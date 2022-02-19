use irp::{Irp, Message, Pronto};
use linux_infrared::{
    lirc,
    lircd_conf::{self, LircRemote},
    log::Log,
    rcdev::{enumerate_rc_dev, Rcdev},
};
use std::{ffi::OsStr, fs, path::PathBuf};
use terminal_size::{terminal_size, Width};

pub mod config;
pub mod encode;
pub mod receive;
pub mod transmit;

pub enum Purpose {
    Receive,
    Transmit,
}

/// Enumerate all rc devices and find the lirc and input devices
pub fn find_devices(matches: &clap::ArgMatches, purpose: Purpose) -> Rcdev {
    let list = match enumerate_rc_dev() {
        Ok(list) if list.is_empty() => {
            eprintln!("error: no devices found");
            std::process::exit(1);
        }
        Ok(list) => list,
        Err(err) => {
            eprintln!("error: no devices found: {}", err);
            std::process::exit(1);
        }
    };

    let entry = if let Some(rcdev) = matches.value_of("RCDEV") {
        if let Some(entry) = list.iter().position(|rc| rc.name == rcdev) {
            entry
        } else {
            eprintln!("error: {} not found", rcdev);
            std::process::exit(1);
        }
    } else if let Some(lircdev) = matches.value_of("LIRCDEV") {
        if let Some(entry) = list
            .iter()
            .position(|rc| rc.lircdev == Some(lircdev.to_string()))
        {
            entry
        } else {
            eprintln!("error: {} not found", lircdev);
            std::process::exit(1);
        }
    } else if let Some(entry) = list.iter().position(|rc| {
        if rc.lircdev.is_none() {
            false
        } else {
            let lircpath = PathBuf::from(rc.lircdev.as_ref().unwrap());

            let lirc = match lirc::open(&lircpath) {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("error: {}: {}", lircpath.display(), e);
                    std::process::exit(1);
                }
            };

            match purpose {
                Purpose::Receive => lirc.can_receive_raw() || lirc.can_receive_scancodes(),
                Purpose::Transmit => lirc.can_send(),
            }
        }
    }) {
        entry
    } else {
        eprintln!("error: no lirc device found");
        std::process::exit(1);
    };

    list[entry].clone()
}

pub fn open_lirc(matches: &clap::ArgMatches, purpose: Purpose) -> lirc::Lirc {
    let rcdev = find_devices(matches, purpose);

    if let Some(lircdev) = rcdev.lircdev {
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

pub fn encode_args<'a>(
    matches: &'a clap::ArgMatches,
    log: &Log,
) -> (Message, &'a clap::ArgMatches) {
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
        Some(("rawir", matches)) => {
            let rawir = matches.value_of("RAWIR").unwrap();

            match irp::rawir::parse(rawir) {
                Ok(raw) => (
                    Message {
                        carrier: None,
                        duty_cycle: None,
                        raw,
                    },
                    matches,
                ),
                Err(s) => {
                    eprintln!("error: {}", s);
                    std::process::exit(2);
                }
            }
        }
        Some(("mode2", matches)) => {
            let filename = matches.value_of("FILE").unwrap();
            let input = match fs::read_to_string(filename) {
                Ok(s) => s,
                Err(s) => {
                    eprintln!("error: {}", s);
                    std::process::exit(2);
                }
            };

            match irp::mode2::parse(&input) {
                Ok(m) => (m, matches),
                Err((line_no, error)) => {
                    eprintln!("{}:{}: error: {}", filename, line_no, error);
                    std::process::exit(2);
                }
            }
        }
        Some(("lircd", matches)) => {
            let filename = matches.value_of_os("CONF").unwrap();

            let remotes = match lircd_conf::parse(filename, log) {
                Ok(r) => r,
                Err(_) => std::process::exit(2),
            };

            let remote = matches.value_of("REMOTE");
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

            if let Some(codes) = matches.values_of("CODES") {
                let codes: Vec<&str> = codes.collect();
                let m = lircd_conf::encode(&remotes, remote, &codes, repeats, log);

                match m {
                    Ok(m) => (m, matches),
                    Err(e) => {
                        log.error(&e);

                        list_remotes(filename, &remotes, None, log);

                        std::process::exit(2);
                    }
                }
            } else {
                list_remotes(filename, &remotes, remote, log);

                std::process::exit(2);
            }
        }
        _ => {
            eprintln!("encode requires a subcommand");
            std::process::exit(2);
        }
    }
}

fn list_remotes(filename: &OsStr, remotes: &[LircRemote], needle: Option<&str>, log: &Log) {
    let size = terminal_size();

    if size.is_some() {
        println!(
            "\nAvailable remotes and codes in {}:\n",
            filename.to_string_lossy()
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
        log.error("not remote found");
    }
}
