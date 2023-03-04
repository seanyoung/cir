use super::{open_lirc, Purpose};
use cir::lircd_conf;
use irp::{Irp, Message, Pronto, Vartable};
use log::{debug, error, info, warn};
use std::{ffi::OsStr, fs, path::Path, str::FromStr};
use terminal_size::{terminal_size, Width};

pub fn transmit(global_matches: &clap::ArgMatches) {
    let (message, matches) = encode_args(global_matches);
    let dry_run = matches.is_present("DRYRUN");

    let duty_cycle = if let Some(value) = matches.value_of("DUTY_CYCLE") {
        match value.parse() {
            Ok(d @ 1..=99) => Some(d),
            _ => {
                eprintln!("error: ‘{value}’ duty cycle is not valid");

                std::process::exit(1);
            }
        }
    } else {
        message.duty_cycle
    };

    let carrier = if let Some(value) = matches.value_of("CARRIER") {
        match value.parse() {
            Ok(c @ 0..=1_000_000) => Some(c),
            _ => {
                eprintln!("error: ‘{value}’ carrier is not valid");

                std::process::exit(1);
            }
        }
    } else {
        message.carrier
    };

    if let Some(carrier) = &carrier {
        if *carrier == 0 {
            info!("carrier: unmodulated (no carrier)");
        } else {
            info!("carrier: {}Hz", carrier);
        }
    }
    if let Some(duty_cycle) = &duty_cycle {
        info!("duty cycle: {}%", duty_cycle);
    }
    info!("rawir: {}", message.print_rawir());

    if !dry_run {
        let mut lircdev = open_lirc(matches, Purpose::Transmit);

        if let Some(values) = global_matches
            .values_of("TRANSMITTERS")
            .or_else(|| matches.values_of("TRANSMITTERS"))
        {
            let mut transmitters: Vec<u32> = Vec::new();
            for t in values {
                match t.parse() {
                    Ok(0) | Err(_) => {
                        eprintln!("error: ‘{t}’ is not a valid transmitter number");
                        std::process::exit(1);
                    }
                    Ok(v) => transmitters.push(v),
                }
            }

            if !transmitters.is_empty() {
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

                if let Some(t) = transmitters.iter().find(|t| **t > transmitter_count) {
                    eprintln!(
                        "error: transmitter {t} not valid, device has {transmitter_count} transmitters"
                    );

                    std::process::exit(1);
                }

                let mask: u32 = transmitters.iter().fold(0, |acc, t| acc | (1 << (t - 1)));

                info!("debug: setting transmitter mask {:08x}", mask);

                match lircdev.set_transmitter_mask(mask) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("error: {lircdev}: failed to set transmitter mask: {e}");

                        std::process::exit(1);
                    }
                }
            }
        }

        if let Some(duty_cycle) = duty_cycle {
            if lircdev.can_set_send_duty_cycle() {
                debug!("setting {} duty cycle {}", lircdev, duty_cycle);

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

        if let Some(carrier) = carrier {
            if lircdev.can_set_send_carrier() {
                debug!("setting {} send carrier {}", lircdev, carrier);

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

        debug!("transmitting {} data {}", lircdev, message.print_rawir());

        if let Err(s) = lircdev.send(&message.raw) {
            eprintln!("error: {lircdev}: {s}");
            std::process::exit(1);
        }
    }
}

fn encode_args(matches: &clap::ArgMatches) -> (Message, &clap::ArgMatches) {
    match matches.subcommand() {
        Some(("irp", matches)) => {
            let mut vars = irp::Vartable::new();

            let i = matches.value_of("IRP").unwrap();

            if let Some(values) = matches.values_of("FIELD") {
                for fields in values {
                    for f in fields.split(',') {
                        let list: Vec<&str> = f.trim().split('=').collect();

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
                }
            }

            let repeats = match matches.value_of("REPEATS") {
                None => 1,
                Some(s) => match s.parse() {
                    Ok(num) => num,
                    Err(_) => {
                        eprintln!("error: {s} is not numeric");
                        std::process::exit(2);
                    }
                },
            };

            let irp = match Irp::parse(i) {
                Ok(m) => m,
                Err(s) => {
                    eprintln!("unable to parse irp ‘{i}’: {s}");
                    std::process::exit(2);
                }
            };

            if matches.is_present("PRONTO") {
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
                match irp.encode(vars, repeats) {
                    Ok(m) => (m, matches),
                    Err(s) => {
                        eprintln!("error: {s}");
                        std::process::exit(2);
                    }
                }
            }
        }
        Some(("pronto", matches)) => {
            let pronto = matches.value_of("PRONTO").unwrap();

            let repeats = match matches.value_of("REPEATS") {
                None => 0,
                Some(s) => match str::parse(s) {
                    Ok(num) => num,
                    Err(_) => {
                        eprintln!("error: {s} is not numeric");
                        std::process::exit(2);
                    }
                },
            };

            let pronto = match Pronto::parse(pronto) {
                Ok(pronto) => pronto,
                Err(err) => {
                    eprintln!("error: {err}");
                    std::process::exit(2);
                }
            };

            (pronto.encode(repeats), matches)
        }
        Some(("rawir", matches)) => encode_rawir(matches),
        Some(("lircd", matches)) => {
            let filename = matches.value_of_os("CONF").unwrap();

            let remotes = match lircd_conf::parse(filename) {
                Ok(r) => r,
                Err(_) => std::process::exit(2),
            };

            let remote = matches.value_of("REMOTE");
            let repeats = match matches.value_of("REPEATS") {
                None => 0,
                Some(s) => match s.parse() {
                    Ok(num) => num,
                    Err(_) => {
                        eprintln!("error: {s} is not numeric");
                        std::process::exit(2);
                    }
                },
            };

            if let Some(codes) = matches.values_of("CODES") {
                let codes: Vec<&str> = codes.collect();
                let m = lircd_conf::encode(&remotes, remote, &codes, repeats);

                match m {
                    Ok(m) => (m, matches),
                    Err(e) => {
                        error!("{}", e);

                        list_remotes(filename, &remotes, None);

                        std::process::exit(2);
                    }
                }
            } else {
                list_remotes(filename, &remotes, remote);

                std::process::exit(2);
            }
        }
        _ => {
            eprintln!("encode requires a subcommand");
            std::process::exit(2);
        }
    }
}

fn encode_rawir(matches: &clap::ArgMatches) -> (Message, &clap::ArgMatches) {
    enum Part {
        Raw(Message),
        Gap(u32),
    }

    let mut part = Vec::new();

    if let Some(files) = matches.values_of_os("FILE") {
        let mut indices = matches.indices_of("FILE").unwrap();

        for filename in files {
            let input = match fs::read_to_string(filename) {
                Ok(s) => s,
                Err(s) => {
                    error!("{}: {}", Path::new(filename).display(), s);
                    std::process::exit(2);
                }
            };

            match Message::parse(&input) {
                Ok(m) => {
                    part.push((Part::Raw(m), indices.next().unwrap()));
                }
                Err(msg) => match Message::parse_mode2(&input) {
                    Ok(m) => {
                        part.push((Part::Raw(m), indices.next().unwrap()));
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
    }

    if let Some(rawirs) = matches.values_of("RAWIR") {
        let mut indices = matches.indices_of("RAWIR").unwrap();

        for rawir in rawirs {
            match Message::parse(rawir) {
                Ok(m) => {
                    part.push((Part::Raw(m), indices.next().unwrap()));
                }
                Err(msg) => {
                    error!("{}", msg);
                    std::process::exit(2);
                }
            }
        }
    }

    if let Some(scancodes) = matches.values_of("SCANCODE") {
        let mut indices = matches.indices_of("SCANCODE").unwrap();

        for scancode in scancodes {
            if let Some((protocol, code)) = scancode.split_once(':') {
                match encode_scancode(protocol, code) {
                    Ok(m) => {
                        part.push((Part::Raw(m), indices.next().unwrap()));
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
    }

    if let Some(gaps) = matches.values_of("GAP") {
        let mut indices = matches.indices_of("GAP").unwrap();

        for gap in gaps {
            match gap.parse() {
                Ok(0) | Err(_) => {
                    error!("{} is not a valid gap", gap);
                    std::process::exit(2);
                }
                Ok(num) => {
                    part.push((Part::Gap(num), indices.next().unwrap()));
                }
            }
        }
    }

    part.sort_by(|a, b| a.1.cmp(&b.1));

    let mut message = Message::new();
    let mut gap = 125000;

    for (part, _) in part {
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

    (message, matches)
}

fn list_remotes(filename: &OsStr, remotes: &[lircd_conf::Remote], needle: Option<&str>) {
    let size = terminal_size();

    if size.is_some() {
        println!(
            "\nAvailable remotes and codes in {}:\n",
            Path::new(filename).display()
        );
    }

    let mut remote_found = false;

    for remote in remotes {
        if let Some(needle) = needle {
            if remote.name != needle {
                continue;
            }
        }
        remote_found = true;

        let codes = remote
            .codes
            .iter()
            .map(|code| code.name.as_str())
            .chain(remote.raw_codes.iter().map(|code| code.name.as_str()));

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

    let (irp, mask) = match protocol.to_ascii_lowercase().as_str() {
        "rc5" => ("{36k,msb,889}<1,-1|-1,1>(1,~CODE:1:6,T:1,CODE:5:8,CODE:6,^114m)[CODE:0..0x1f7f,T:0..1@CODE:1:6]", 0x1f7f),
        "rc5x_20" => ("{36k,msb,889}<1,-1|-1,1>(1,~CODE:1:6,T:1,CODE:5:16,CODE:4:8,CODE:6,^114m)[CODE:0..0x1f7f,T:0..1@CODE:1:6]", 0x1f7f),
        "rc5_sz" => ("{36k,msb,889}<1,-1|-1,1>(1,CODE:1:13,T:1,CODE:11,^114m)[CODE:0..0x2fff,T:0..1@CODE:1:12]", 0x2fff),
        "nec" => ("{38.4k,564}<1,-1|1,-3>(16,-8,CODE:8:8,CODE:8,~CODE:8:8,~CODE:8,1,^108m) [CODE:0..0xffff]", 0xffff),
        "sony12" => ("{40k,600}<1,-1|2,-1>(4,-1,CODE:7,CODE:5:16,^45m)*[CODE:0..0x1f007f]",0x1f007f),
        "sony15" => ("{40k,600}<1,-1|2,-1>(4,-1,CODE:7,CODE:8:16,^45m)*[CODE:0..0xff007f]",0xff007f),
        "sony20" => ("{40k,600}<1,-1|2,-1>(4,-1,CODE:7,CODE:5:8,CODE:8:16,^45m)*[CODE:0..0x7f1fff]", 0x7f1fff),
        "imon" => ("{416,38k,msb}<last,1u,last=-1|-2u,last=1>(1,CODE:31,^106m){last=1} [CODE:0..0x7fffffff]", 0x7fffffff),
        _ => {
            return Err(format!("protocol {protocol} is not known"));
        }
    };

    let masked = scancode & mask;

    if masked != scancode {
        warn!("error: scancode {scancode:#x} masked to {masked:#x}");
        scancode = masked;
    }

    let irp = Irp::parse(irp).unwrap();

    let mut vars = Vartable::new();

    vars.set("CODE".into(), scancode as i64);

    irp.encode(vars, 1)
}
