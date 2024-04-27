#[cfg(target_os = "linux")]
use super::config::{open_lirc, Purpose};
use cir::{
    keymap::{Keymap, LinuxProtocol},
    lircd_conf,
};
use irp::{Irp, Message, Pronto, Vartable};
use log::{error, info, warn};
use std::{fs, path::Path, str::FromStr};
use terminal_size::{terminal_size, Width};

pub fn transmit(transmit: &crate::Transmit) {
    let message = encode_args(transmit);

    if let Some(carrier) = &message.carrier {
        if *carrier == 0 {
            info!("carrier: unmodulated (no carrier)");
        } else {
            info!("carrier: {}Hz", carrier);
        }
    }
    if let Some(duty_cycle) = &message.duty_cycle {
        info!("duty cycle: {}%", duty_cycle);
    }
    info!("rawir: {}", message.print_rawir());

    #[cfg(target_os = "linux")]
    if !transmit.dry_run {
        let mut lircdev = open_lirc(&transmit.device, Purpose::Transmit);

        if !transmit.transmitters.is_empty() {
            if !lircdev.can_set_send_transmitter_mask() {
                eprintln!("error: {lircdev}: device does not support setting transmitters");

                std::process::exit(1);
            }

            let transmitter_count = match lircdev.num_transmitters() {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("error: {lircdev}: failed to get transmitter count: {e}");

                    std::process::exit(1);
                }
            };

            if let Some(t) = transmit
                .transmitters
                .iter()
                .find(|t| **t == 0 || **t > transmitter_count)
            {
                eprintln!(
                    "error: transmitter {t} not valid, device has {transmitter_count} transmitters"
                );

                std::process::exit(1);
            }

            let mask: u32 = transmit
                .transmitters
                .iter()
                .fold(0, |acc, t| acc | (1 << (t - 1)));

            info!("debug: setting transmitter mask {:08x}", mask);

            match lircdev.set_transmitter_mask(mask) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("error: {lircdev}: failed to set transmitter mask: {e}");

                    std::process::exit(1);
                }
            }
        }

        if let Some(duty_cycle) = message.duty_cycle {
            if lircdev.can_set_send_duty_cycle() {
                log::debug!("setting {} duty cycle {}", lircdev, duty_cycle);

                if let Err(s) = lircdev.set_send_duty_cycle(duty_cycle as u32) {
                    eprintln!("error: {lircdev}: {s}");

                    std::process::exit(1);
                }
            } else {
                warn!(
                    "warning: {}: device does not support setting send duty cycle",
                    lircdev
                );
            }
        }

        if let Some(carrier) = message.carrier {
            if lircdev.can_set_send_carrier() {
                log::debug!("setting {} send carrier {}", lircdev, carrier);

                if let Err(s) = lircdev.set_send_carrier(carrier as u32) {
                    eprintln!("error: {lircdev}: {s}");

                    if carrier == 0 {
                        eprintln!("info: not all lirc devices can send unmodulated");
                    }
                    std::process::exit(1);
                }
            } else {
                eprintln!("warning: {lircdev}: device does not support setting carrier");
            }
        }

        log::debug!("transmitting {} data {}", lircdev, message.print_rawir());

        if let Err(s) = lircdev.send(&message.raw) {
            eprintln!("error: {lircdev}: {s}");
            std::process::exit(1);
        }
    }
}

fn encode_args(transmit: &crate::Transmit) -> Message {
    match &transmit.commands {
        crate::TransmitCommands::Irp(tx_irp) => {
            let mut vars = irp::Vartable::new();

            for field in &tx_irp.fields {
                let list: Vec<&str> = field.trim().split('=').collect();

                if list.len() != 2 {
                    eprintln!("argument to --field must be X=1");
                    std::process::exit(2);
                }

                let value = match if list[1].starts_with("0x") {
                    i64::from_str_radix(&list[1][2..], 16)
                } else if list[1].starts_with("0o") {
                    i64::from_str_radix(&list[1][2..], 8)
                } else if list[1].starts_with("0b") {
                    i64::from_str_radix(&list[1][2..], 2)
                } else {
                    list[1].parse()
                } {
                    Ok(v) => v,
                    Err(_) => {
                        eprintln!("‘{}’ is not a valid number", list[1]);
                        std::process::exit(2);
                    }
                };

                vars.set(list[0].to_string(), value);
            }

            let irp = match Irp::parse(&tx_irp.irp) {
                Ok(m) => m,
                Err(s) => {
                    eprintln!("unable to parse irp ‘{}’: {s}", tx_irp.irp);
                    std::process::exit(2);
                }
            };

            let mut m = if tx_irp.pronto {
                match irp.encode_pronto(vars) {
                    Ok(p) => {
                        println!("{p}");
                        std::process::exit(0);
                    }
                    Err(s) => {
                        eprintln!("error: {s}");
                        std::process::exit(2);
                    }
                }
            } else {
                match irp.encode_raw(vars, tx_irp.repeats) {
                    Ok(m) => m,
                    Err(s) => {
                        eprintln!("error: {s}");
                        std::process::exit(2);
                    }
                }
            };

            if tx_irp.carrier.is_some() {
                m.carrier = tx_irp.carrier;
            }

            if tx_irp.duty_cycle.is_some() {
                m.duty_cycle = tx_irp.duty_cycle;
            }

            m
        }
        crate::TransmitCommands::Pronto(pronto) => {
            let p = match Pronto::parse(&pronto.pronto) {
                Ok(pronto) => pronto,
                Err(err) => {
                    eprintln!("error: {err}");
                    std::process::exit(2);
                }
            };

            p.encode(pronto.repeats as usize)
        }
        crate::TransmitCommands::RawIR(rawir) => encode_rawir(rawir),
        crate::TransmitCommands::Keymap(args) => {
            if args.keymap.to_string_lossy().ends_with(".lircd.conf") {
                encode_lircd_conf(args)
            } else {
                encode_keymap(args)
            }
        }
    }
}

fn encode_keymap(args: &crate::TransmitKeymap) -> Message {
    let remotes = match Keymap::parse(&args.keymap) {
        Ok(r) => r,
        Err(_) => std::process::exit(2),
    };

    if !args.codes.is_empty() {
        let codes: Vec<&str> = args.codes.iter().map(|v| v.as_str()).collect();
        let m = cir::keymap::encode(&remotes, args.remote.as_deref(), &codes, args.repeats);

        match m {
            Ok(mut m) => {
                if args.carrier.is_some() {
                    m.carrier = args.carrier;
                }

                if args.duty_cycle.is_some() {
                    m.duty_cycle = args.duty_cycle;
                }

                m
            }
            Err(e) => {
                error!("{}", e);

                list_keymap_remotes(&args.keymap, &remotes, None);

                std::process::exit(2);
            }
        }
    } else {
        list_keymap_remotes(&args.keymap, &remotes, args.remote.as_deref());

        std::process::exit(2);
    }
}

fn encode_lircd_conf(lircd: &crate::TransmitKeymap) -> Message {
    let remotes = match lircd_conf::parse(&lircd.keymap) {
        Ok(r) => r,
        Err(_) => std::process::exit(2),
    };

    if !lircd.codes.is_empty() {
        let codes: Vec<&str> = lircd.codes.iter().map(|v| v.as_str()).collect();
        let m = lircd_conf::encode(&remotes, lircd.remote.as_deref(), &codes, lircd.repeats);

        match m {
            Ok(mut m) => {
                if lircd.carrier.is_some() {
                    m.carrier = lircd.carrier;
                }

                if lircd.duty_cycle.is_some() {
                    m.duty_cycle = lircd.duty_cycle;
                }

                m
            }
            Err(e) => {
                error!("{}", e);

                list_lircd_remotes(&lircd.keymap, &remotes, None);

                std::process::exit(2);
            }
        }
    } else {
        list_lircd_remotes(&lircd.keymap, &remotes, lircd.remote.as_deref());

        std::process::exit(2);
    }
}

fn encode_rawir(transmit: &crate::TransmitRawIR) -> Message {
    enum Part {
        Raw(Message),
        Gap(u32),
    }

    let mut part = Vec::new();

    for tx in &transmit.transmitables {
        match tx {
            crate::Transmitables::File(filename) => {
                let input = match fs::read_to_string(filename) {
                    Ok(s) => s,
                    Err(s) => {
                        error!("{}: {}", Path::new(filename).display(), s);
                        std::process::exit(2);
                    }
                };

                match Message::parse(&input) {
                    Ok(m) => {
                        part.push(Part::Raw(m));
                    }
                    Err(msg) => match Message::parse_mode2(&input) {
                        Ok(m) => {
                            part.push(Part::Raw(m));
                        }
                        Err((line_no, error)) => {
                            error!("{}: parse as rawir: {}", Path::new(filename).display(), msg);
                            error!(
                                "{}:{}: parse as mode2: {}",
                                Path::new(filename).display(),
                                line_no,
                                error
                            );
                            std::process::exit(2);
                        }
                    },
                }
            }
            crate::Transmitables::RawIR(rawir) => match Message::parse(rawir) {
                Ok(m) => {
                    part.push(Part::Raw(m));
                }
                Err(msg) => {
                    error!("{}", msg);
                    std::process::exit(2);
                }
            },
            crate::Transmitables::Scancode(scancode) => {
                if let Some((protocol, code)) = scancode.split_once(':') {
                    match encode_scancode(protocol, code) {
                        Ok(m) => {
                            part.push(Part::Raw(m));
                        }
                        Err(msg) => {
                            error!("{}", msg);
                            std::process::exit(2);
                        }
                    }
                } else {
                    error!(
                        "{} is not a valid protocol, should be protocol:scancode",
                        scancode
                    );
                    std::process::exit(2);
                }
            }
            crate::Transmitables::Gap(gap) => {
                part.push(Part::Gap(*gap));
            }
        }
    }

    let mut message = Message::new();
    let mut gap = 125000;

    for part in part {
        match part {
            Part::Gap(v) => {
                gap = v;
            }
            Part::Raw(raw) => {
                if !message.raw.is_empty() && !message.has_trailing_gap() {
                    message.raw.push(gap);
                }

                message.extend(&raw);
            }
        }
    }

    if message.raw.is_empty() {
        error!("nothing to send");
        std::process::exit(2);
    }

    if !message.has_trailing_gap() {
        message.raw.push(gap);
    }

    message
}

fn list_keymap_remotes(filename: &Path, remotes: &[Keymap], needle: Option<&str>) {
    let size = terminal_size();

    if size.is_some() {
        println!("\nAvailable remotes and codes in {}:\n", filename.display());
    }

    let mut remote_found = false;

    for remote in remotes {
        if let Some(needle) = needle {
            if remote.name != needle {
                continue;
            }
        }
        remote_found = true;

        let mut codes: Vec<_> = remote
            .scancodes
            .values()
            .map(|code| code.as_str())
            .chain(remote.raw.iter().map(|code| code.keycode.as_str()))
            .collect();

        codes.sort();

        if let Some((Width(term_witdh), _)) = size {
            let mut pos = 2;
            let mut res = String::new();
            let mut first = true;

            for code in codes {
                if first {
                    first = false
                } else {
                    res.push_str(", ");
                }

                if pos + code.len() + 2 < term_witdh as usize {
                    res.push_str(code);
                    pos += code.len() + 2;
                } else {
                    res.push_str("\n  ");
                    res.push_str(code);
                    pos = code.len() + 4;
                }
            }

            println!("Remote:\n  {}\nCodes:\n  {}", remote.name, res);
        } else {
            for code in codes {
                println!("{code}");
            }
        }
    }

    if !remote_found {
        error!("not remote found");
    }
}

fn list_lircd_remotes(filename: &Path, remotes: &[lircd_conf::Remote], needle: Option<&str>) {
    let size = terminal_size();

    if size.is_some() {
        println!("\nAvailable remotes and codes in {}:\n", filename.display());
    }

    let mut remote_found = false;

    for remote in remotes {
        if let Some(needle) = needle {
            if remote.name != needle {
                continue;
            }
        }
        remote_found = true;

        let mut codes: Vec<_> = remote
            .codes
            .iter()
            .map(|code| code.name.as_str())
            .chain(remote.raw_codes.iter().map(|code| code.name.as_str()))
            .collect();

        codes.sort();

        if let Some((Width(term_witdh), _)) = size {
            let mut pos = 2;
            let mut res = String::new();
            let mut first = true;

            for code in codes {
                if first {
                    first = false
                } else {
                    res.push_str(", ");
                }

                if pos + code.len() + 2 < term_witdh as usize {
                    res.push_str(code);
                    pos += code.len() + 2;
                } else {
                    res.push_str("\n  ");
                    res.push_str(code);
                    pos = code.len() + 4;
                }
            }

            println!("Remote:\n  {}\nCodes:\n  {}", remote.name, res);
        } else {
            for code in codes {
                println!("{code}");
            }
        }
    }

    if !remote_found {
        error!("not remote found");
    }
}

fn encode_scancode(protocol: &str, code: &str) -> Result<Message, String> {
    let mut scancode = if let Ok(code) = if let Some(hex) = code.strip_prefix("0x") {
        u64::from_str_radix(hex, 16)
    } else {
        u64::from_str(code)
    } {
        code
    } else {
        return Err(format!("invalid scancode {code}"));
    };

    let Some(linux) = LinuxProtocol::find_like(protocol) else {
        return Err(format!("protocol {protocol} is not known"));
    };

    if linux.irp.is_none() {
        return Err(format!("protocol {protocol} is cannot be encoded"));
    }

    let masked = scancode & linux.scancode_mask as u64;

    if masked != scancode {
        warn!("error: scancode {scancode:#x} masked to {masked:#x}");
        scancode = masked;
    }

    let irp = Irp::parse(linux.irp.unwrap()).unwrap();

    let mut vars = Vartable::new();

    vars.set("CODE".into(), scancode as i64);

    irp.encode_raw(vars, 1)
}
