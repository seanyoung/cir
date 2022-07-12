use super::{open_lirc, Purpose};
use cir::lircd_conf;
use irp::{rawir, Irp, Message, Pronto};
use log::{debug, error, info, warn};
use std::{ffi::OsStr, fs, path::Path};
use terminal_size::{terminal_size, Width};

pub fn transmit(global_matches: &clap::ArgMatches) {
    let (message, matches) = encode_args(global_matches);
    let dry_run = matches.is_present("DRYRUN");

    let duty_cycle = if let Some(value) = matches.value_of("DUTY_CYCLE") {
        match value.parse() {
            Ok(d @ 1..=99) => Some(d),
            _ => {
                eprintln!("error: ‘{}’ duty cycle is not valid", value);

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
                eprintln!("error: ‘{}’ carrier is not valid", value);

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
                        eprintln!("error: ‘{}’ is not a valid transmitter number", t);
                        std::process::exit(1);
                    }
                    Ok(v) => transmitters.push(v),
                }
            }

            if !transmitters.is_empty() {
                if !lircdev.can_set_send_transmitter_mask() {
                    eprintln!(
                        "error: {}: device does not support setting transmitters",
                        lircdev
                    );

                    std::process::exit(1);
                }

                let transmitter_count = match lircdev.num_transmitters() {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("error: {}: failed to get transmitter count: {}", lircdev, e);

                        std::process::exit(1);
                    }
                };

                if let Some(t) = transmitters.iter().find(|t| **t > transmitter_count) {
                    eprintln!(
                        "error: transmitter {} not valid, device has {} transmitters",
                        t, transmitter_count
                    );

                    std::process::exit(1);
                }

                let mask: u32 = transmitters.iter().fold(0, |acc, t| acc | (1 << (t - 1)));

                info!("debug: setting transmitter mask {:08x}", mask);

                match lircdev.set_transmitter_mask(mask) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("error: {}: failed to set transmitter mask: {}", lircdev, e);

                        std::process::exit(1);
                    }
                }
            }
        }

        if let Some(duty_cycle) = duty_cycle {
            if lircdev.can_set_send_duty_cycle() {
                debug!("setting {} duty cycle {}", lircdev, duty_cycle);

                if let Err(s) = lircdev.set_send_duty_cycle(duty_cycle as u32) {
                    eprintln!("error: {}: {}", lircdev, s);

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
                    eprintln!("error: {}: {}", lircdev, s);

                    if carrier == 0 {
                        eprintln!("info: not all lirc devices can send unmodulated");
                    }
                    std::process::exit(1);
                }
            } else {
                eprintln!(
                    "warning: {}: device does not support setting carrier",
                    lircdev
                );
            }
        }

        debug!(
            "transmitting {} data {}",
            lircdev,
            rawir::print_to_string(&message.raw)
        );

        if let Err(s) = lircdev.send(&message.raw) {
            eprintln!("error: {}: {}", lircdev, s);
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
                for f in values {
                    let list: Vec<&str> = f.split('=').collect();

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
                            eprintln!("'{}' is not a valid number", list[1]);
                            std::process::exit(2);
                        }
                    };

                    vars.set(list[0].to_string(), value, 8);
                }
            }

            let repeats = match matches.value_of("REPEATS") {
                None => 1,
                Some(s) => match s.parse() {
                    Ok(num) => num,
                    Err(_) => {
                        eprintln!("error: {} is not numeric", s);
                        std::process::exit(2);
                    }
                },
            };

            let irp = match Irp::parse(i) {
                Ok(m) => m,
                Err(s) => {
                    eprintln!("unable to parse irp ‘{}’: {}", i, s);
                    std::process::exit(2);
                }
            };

            if matches.is_present("PRONTO") {
                match irp.encode_pronto(vars) {
                    Ok(p) => {
                        println!("{}", p);
                        std::process::exit(0);
                    }
                    Err(s) => {
                        eprintln!("error: {}", s);
                        std::process::exit(2);
                    }
                }
            } else {
                match irp.encode(vars, repeats) {
                    Ok(m) => (m, matches),
                    Err(s) => {
                        eprintln!("error: {}", s);
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
                        eprintln!("error: {} is not numeric", s);
                        std::process::exit(2);
                    }
                },
            };

            let pronto = match Pronto::parse(pronto) {
                Ok(pronto) => pronto,
                Err(err) => {
                    eprintln!("error: {}", err);
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
                        eprintln!("error: {} is not numeric", s);
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

            match irp::rawir::parse(&input) {
                Ok(raw) => {
                    part.push((
                        Part::Raw(Message {
                            carrier: None,
                            duty_cycle: None,
                            raw,
                        }),
                        indices.next().unwrap(),
                    ));
                }
                Err(msg) => match irp::mode2::parse(&input) {
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
            match irp::rawir::parse(rawir) {
                Ok(raw) => {
                    part.push((
                        Part::Raw(Message {
                            carrier: None,
                            duty_cycle: None,
                            raw,
                        }),
                        indices.next().unwrap(),
                    ));
                }
                Err(msg) => {
                    error!("{}", msg);
                    std::process::exit(2);
                }
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
                if !message.raw.is_empty() && !message.has_trailing_space() {
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

    if !message.has_trailing_space() {
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
                println!("{}", code);
            }
        }
    }

    if !remote_found {
        error!("not remote found");
    }
}
