use irp::{Irp, Message, Pronto};
use linux_infrared::{lirc, rcdev};

use std::{fs, path::PathBuf};

pub mod config;
pub mod encode;
pub mod receive;
pub mod transmit;

/// Enumerate all rc devices and find the lirc and input devices
pub fn find_devices(matches: &clap::ArgMatches) -> (Option<String>, Option<String>) {
    let list = match rcdev::enumerate_rc_dev() {
        Ok(list) if list.is_empty() => {
            eprintln!("error: no devices found");
            std::process::exit(1);
        }
        Ok(list) => list,
        Err(err) => {
            eprintln!("error: no devices found: {}", err.to_string());
            std::process::exit(1);
        }
    };

    if let Some(lircdev) = matches.value_of("LIRCDEV") {
        (Some(lircdev.to_owned()), None)
    } else {
        let entry = if let Some(rcdev) = matches.value_of("RCDEV") {
            if let Some(entry) = list.iter().position(|rc| rc.name == rcdev) {
                entry
            } else {
                eprintln!("error: {} not found", rcdev);
                std::process::exit(1);
            }
        } else if let Some(entry) = list.iter().position(|rc| rc.lircdev.is_some()) {
            entry
        } else {
            eprintln!("error: no lirc device found");
            std::process::exit(1);
        };

        (list[entry].lircdev.clone(), list[entry].inputdev.clone())
    }
}

pub fn open_lirc(matches: &clap::ArgMatches) -> lirc::Lirc {
    let (lircdev, _) = find_devices(matches);

    if let Some(lircdev) = lircdev {
        let lircpath = PathBuf::from(lircdev);

        match lirc::open(&lircpath) {
            Ok(l) => l,
            Err(s) => {
                eprintln!("error: {}: {}", lircpath.display(), s);
                std::process::exit(1);
            }
        }
    } else {
        eprintln!("error: no lirc device found");
        std::process::exit(1);
    }
}

pub fn encode_args(matches: &clap::ArgMatches) -> Message {
    match matches.subcommand() {
        ("irp", Some(matches)) => {
            let mut vars = irp::Vartable::new();

            let i = matches.value_of("IRP").unwrap();

            if let Some(values) = matches.values_of("FIELD") {
                for f in values {
                    let list: Vec<&str> = f.split('=').collect();

                    if list.len() != 2 {
                        eprintln!("argument to --field must be X=1");
                        std::process::exit(2);
                    }

                    let value = if list[1].starts_with("0x") {
                        i64::from_str_radix(&list[1][2..], 16).unwrap()
                    } else {
                        list[1].parse().unwrap()
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
                    eprintln!("parse error: {}", s);
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
                    Ok(m) => m,
                    Err(s) => {
                        eprintln!("error: {}", s);
                        std::process::exit(2);
                    }
                }
            }
        }
        ("pronto", Some(matches)) => {
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

            pronto.encode(repeats)
        }
        ("rawir", Some(matches)) => {
            let rawir = matches.value_of("RAWIR").unwrap();

            match irp::rawir::parse(rawir) {
                Ok(raw) => Message {
                    carrier: None,
                    duty_cycle: None,
                    raw,
                },
                Err(s) => {
                    eprintln!("error: {}", s);
                    std::process::exit(2);
                }
            }
        }
        ("mode2", Some(matches)) => {
            let input = match fs::read_to_string(matches.value_of("FILE").unwrap()) {
                Ok(s) => s,
                Err(s) => {
                    eprintln!("error: {}", s.to_string());
                    std::process::exit(2);
                }
            };

            match irp::mode2::parse(&input) {
                Ok(m) => m,
                Err(s) => {
                    eprintln!("error: {}", s);
                    std::process::exit(2);
                }
            }
        }
        _ => {
            eprintln!("encode requires a subcommand");
            std::process::exit(2);
        }
    }
}
