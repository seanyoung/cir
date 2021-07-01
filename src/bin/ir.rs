use clap::{App, AppSettings, Arg, SubCommand};
use ir::{keymap, lirc, rcdev};
use irp::{Irp, Message, Pronto};
use itertools::Itertools;
use mio::{unix::SourceFd, Events, Interest, Poll, Token};
use std::fs;
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;

fn main() {
    let matches = App::new("ir")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Sean Young <sean@mess.org>")
        .about("Linux Infrared Control")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommand(
            SubCommand::with_name("encode")
                .about("Encode IR and print to stdout")
                .setting(AppSettings::SubcommandRequiredElseHelp)
                .subcommand(
                    SubCommand::with_name("irp")
                        .about("Encode using IRP langauge")
                        .arg(
                            Arg::with_name("PRONTO")
                                .help("Encode IRP to pronto hex")
                                .long("pronto")
                                .short("p"),
                        )
                        .arg(
                            Arg::with_name("REPEATS")
                                .help("Number of IRP repeats to encode")
                                .long("repeats")
                                .short("r")
                                .conflicts_with("PRONTO")
                                .takes_value(true),
                        )
                        .arg(
                            Arg::with_name("FIELD")
                                .help("Set input variable like KEY=VALUE")
                                .long("field")
                                .short("f")
                                .takes_value(true)
                                .multiple(true)
                                .number_of_values(1),
                        )
                        .arg(
                            Arg::with_name("IRP")
                                .help("IRP protocol")
                                .required(true)
                                .index(1),
                        ),
                )
                .subcommand(
                    SubCommand::with_name("pronto")
                        .arg(
                            Arg::with_name("REPEATS")
                                .long("repeats")
                                .short("r")
                                .takes_value(true)
                                .default_value("1"),
                        )
                        .arg(
                            Arg::with_name("PRONTO")
                                .help("Pronto Hex code")
                                .required(true),
                        ),
                )
                .subcommand(
                    SubCommand::with_name("mode2")
                        .about("Parse mode2 pulse space file and print as raw IR")
                        .arg(
                            Arg::with_name("FILE")
                                .help("File to load and parse")
                                .required(true),
                        ),
                )
                .subcommand(
                    SubCommand::with_name("rawir")
                        .arg(Arg::with_name("RAWIR").help("Raw IR").required(true)),
                ),
        )
        .subcommand(SubCommand::with_name("keymap").arg(Arg::with_name("FILE").long("keymap")))
        .subcommand(
            SubCommand::with_name("send")
                .about("Encode IR and transmit")
                .setting(AppSettings::SubcommandRequiredElseHelp)
                .arg(
                    Arg::with_name("LIRCDEV")
                        .long("device")
                        .short("d")
                        .takes_value(true)
                        .conflicts_with("RCDEV"),
                )
                .arg(
                    Arg::with_name("RCDEV")
                        .long("rcdev")
                        .short("s")
                        .takes_value(true)
                        .conflicts_with("LIRCDEV"),
                )
                .arg(Arg::with_name("VERBOSE").long("verbose").short("v"))
                .subcommand(
                    SubCommand::with_name("irp")
                        .about("Encode using IRP langauge")
                        .arg(
                            Arg::with_name("IRP")
                                .help("IRP protocol")
                                .required(true)
                                .last(true),
                        )
                        .arg(
                            Arg::with_name("REPEATS")
                                .long("repeats")
                                .short("r")
                                .takes_value(true)
                                .default_value("1"),
                        )
                        .arg(
                            Arg::with_name("FIELD")
                                .long("field")
                                .short("f")
                                .takes_value(true),
                        ),
                )
                .subcommand(
                    SubCommand::with_name("pronto")
                        .arg(
                            Arg::with_name("REPEATS")
                                .long("repeats")
                                .short("r")
                                .takes_value(true)
                                .default_value("1"),
                        )
                        .arg(
                            Arg::with_name("PRONTO")
                                .help("Pronto Hex code")
                                .required(true),
                        ),
                )
                .subcommand(
                    SubCommand::with_name("mode2")
                        .about("Parse mode2 pulse space file and print as raw IR")
                        .arg(
                            Arg::with_name("FILE")
                                .help("File to load and parse")
                                .required(true),
                        ),
                )
                .subcommand(
                    SubCommand::with_name("rawir")
                        .arg(Arg::with_name("RAWIR").help("Raw IR").required(true))
                        .arg(
                            Arg::with_name("CARRIER")
                                .long("carrier")
                                .short("c")
                                .takes_value(true),
                        )
                        .arg(
                            Arg::with_name("DUTY_CYCLE")
                                .long("duty-cycle")
                                .short("u")
                                .takes_value(true),
                        ),
                ),
        )
        .subcommand(SubCommand::with_name("list").about("List IR devices"))
        .subcommand(
            SubCommand::with_name("receive")
                .about("Receive IR")
                .arg(
                    Arg::with_name("LIRCDEV")
                        .long("device")
                        .short("d")
                        .takes_value(true)
                        .conflicts_with("RCDEV"),
                )
                .arg(
                    Arg::with_name("RCDEV")
                        .long("rcdev")
                        .short("s")
                        .takes_value(true)
                        .conflicts_with("LIRCDEV"),
                )
                .arg(Arg::with_name("LEARNING").long("learning-mode").short("l"))
                .arg(
                    Arg::with_name("TIMEOUT")
                        .long("timeout")
                        .short("t")
                        .takes_value(true),
                ),
        )
        .get_matches();

    match matches.subcommand() {
        ("encode", Some(matches)) => {
            let message = encode_args(matches);

            if let Some(carrier) = &message.carrier {
                if *carrier == 0 {
                    println!("carrier: unmodulated (no carrier)");
                } else {
                    println!("carrier: {}Hz", carrier);
                }
            }
            if let Some(duty_cycle) = &message.duty_cycle {
                println!("duty cycle: {}%", duty_cycle);
            }
            println!("rawir: {}", message.print_rawir());
        }
        ("send", Some(matches)) => {
            let message = encode_args(matches);

            if matches.is_present("VERBOSE") {
                if let Some(carrier) = &message.carrier {
                    if *carrier == 0 {
                        println!("carrier: unmodulated (no carrier)");
                    } else {
                        println!("carrier: {}Hz", carrier);
                    }
                }
                if let Some(duty_cycle) = &message.duty_cycle {
                    println!("duty cycle: {}%", duty_cycle);
                }
                println!("rawir: {}", message.print_rawir());
            }

            let mut lircdev = open_lirc(matches);

            if let Some(duty_cycle) = message.duty_cycle {
                if lircdev.can_set_send_duty_cycle() {
                    if let Err(s) = lircdev.set_send_duty_cycle(duty_cycle as u32) {
                        eprintln!("error: {}", s.to_string());

                        std::process::exit(1);
                    }
                } else {
                    eprintln!("warning: device does not support setting send duty cycle");
                }
            }

            if let Some(carrier) = message.carrier {
                if lircdev.can_set_send_carrier() {
                    if let Err(s) = lircdev.set_send_carrier(carrier as u32) {
                        eprintln!("error: {}", s.to_string());

                        if carrier == 0 {
                            eprintln!("info: not all lirc devices can send unmodulated");
                        }
                        std::process::exit(1);
                    }
                } else {
                    eprintln!("warning: device does not support setting carrier");
                }
            }

            if let Err(s) = lircdev.send(&message.raw) {
                eprintln!("error: {}", s.to_string());
                std::process::exit(1);
            }
        }
        ("list", Some(_)) => match rcdev::enumerate_rc_dev() {
            Ok(list) => print_rc_dev(&list),
            Err(err) => {
                eprintln!("error: {}", err.to_string());
                std::process::exit(1);
            }
        },
        ("receive", Some(matches)) => {
            receive(matches);
        }
        _ => unreachable!(),
    }

    if let ("keymap", Some(matches)) = matches.subcommand() {
        let arg = if matches.is_present("FILE") {
            fs::read_to_string(matches.value_of("FILE").unwrap()).unwrap()
        } else {
            matches.value_of("INPUT").unwrap().to_string()
        };

        match keymap::parse(&arg) {
            Ok(ir) => println!("{:?}", ir),
            Err(s) => eprintln!("error: {}", s),
        }
    }
}

fn encode_args(matches: &clap::ArgMatches) -> Message {
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

            let pronto = match Pronto::parse(&pronto) {
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

            match irp::rawir::parse(&rawir) {
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

fn print_rc_dev(list: &[rcdev::Rcdev]) {
    if list.is_empty() {
        eprintln!("error: no devices found");
        std::process::exit(1);
    }

    for rcdev in list {
        println!("{}:", rcdev.name);

        println!("\tDevice Name\t\t: {}", rcdev.device_name);
        println!("\tDriver\t\t\t: {}", rcdev.driver);
        println!("\tDefault Keymap\t\t: {}", rcdev.default_keymap);
        if let Some(inputdev) = &rcdev.inputdev {
            println!("\tInput Device\t\t: {}", inputdev);
        }
        if let Some(lircdev) = &rcdev.lircdev {
            println!("\tLIRC Device\t\t: {}", lircdev);

            match lirc::lirc_open(&PathBuf::from(lircdev)) {
                Ok(lircdev) => {
                    if lircdev.can_receive_raw() {
                        println!("\tLIRC Receiver\t\t: raw receiver");

                        if lircdev.can_get_rec_resolution() {
                            println!(
                                "\tLIRC Resolution\t\t: {}",
                                match lircdev.receiver_resolution() {
                                    Ok(res) => format!("{} microseconds", res),
                                    Err(err) => err.to_string(),
                                }
                            );
                        } else {
                            println!("\tLIRC Resolution\t\t: unknown");
                        }

                        println!(
                            "\tLIRC Timeout\t\t: {}",
                            match lircdev.get_timeout() {
                                Ok(timeout) => format!("{} microseconds", timeout),
                                Err(err) => err.to_string(),
                            }
                        );

                        if lircdev.can_set_timeout() {
                            println!(
                                "\tLIRC Timeout Range\t: {}",
                                match lircdev.get_min_max_timeout() {
                                    Ok(range) =>
                                        format!("{} - {} microseconds", range.start, range.end),
                                    Err(err) => err.to_string(),
                                }
                            );
                        } else {
                            println!("\tLIRC Receiver Timeout Range\t: none");
                        }

                        println!(
                            "\tLIRC Wideband Receiver\t: {}",
                            if lircdev.can_use_wideband_receiver() {
                                "yes"
                            } else {
                                "no"
                            }
                        );

                        println!(
                            "\tLIRC Measure Carrier\t: {}",
                            if lircdev.can_measure_carrier() {
                                "yes"
                            } else {
                                "no"
                            }
                        );
                    } else if lircdev.can_receive_scancodes() {
                        println!("\tLIRC Receiver\t\t: scancode");
                    } else {
                        println!("\tLIRC Receiver\t\t: none");
                    }

                    if lircdev.can_send() {
                        println!("\tLIRC Transmitter\t: yes");

                        println!(
                            "\tLIRC Set Tx Carrier\t: {}",
                            if lircdev.can_set_send_carrier() {
                                "yes"
                            } else {
                                "no"
                            }
                        );

                        println!(
                            "\tLIRC Set Tx Duty Cycle\t: {}",
                            if lircdev.can_set_send_duty_cycle() {
                                "yes"
                            } else {
                                "no"
                            }
                        );
                    } else {
                        println!("\tLIRC Transmitter\t: no");
                    }
                }
                Err(err) => {
                    println!("\tLIRC Features: {}", err.to_string());
                }
            }
        }
        println!(
            "\tSupported Protocols\t: {}",
            rcdev.supported_protocols.join(" ")
        );

        println!(
            "\tEnabled Protocols\t: {}",
            rcdev
                .enabled_protocols
                .iter()
                .map(|p| &rcdev.supported_protocols[*p])
                .join(" ")
        );
    }
}

fn open_lirc(matches: &clap::ArgMatches) -> lirc::Lirc {
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

    let lircdev = if let Some(lircdev) = matches.value_of("LIRCDEV") {
        lircdev
    } else {
        let entry = if let Some(rcdev) = matches.value_of("RCDEV") {
            if let Some(entry) = list.iter().position(|rc| rc.name == rcdev) {
                entry
            } else {
                eprintln!("error: {} not found", rcdev);
                std::process::exit(1);
            }
        } else {
            if let Some(entry) = list.iter().position(|rc| rc.lircdev.is_some()) {
                entry
            } else {
                eprintln!("error: no lirc device found");
                std::process::exit(1);
            }
        };

        if let Some(lircdev) = &list[entry].lircdev {
            lircdev
        } else {
            eprintln!("error: {} has no lirc device", list[entry].name);
            std::process::exit(1);
        }
    };

    let lircpath = PathBuf::from(lircdev);

    match lirc::lirc_open(&lircpath) {
        Ok(l) => l,
        Err(s) => {
            eprintln!("error: {}: {}", lircpath.display(), s);
            std::process::exit(1);
        }
    }
}

fn receive(matches: &clap::ArgMatches) {
    let mut lircdev = open_lirc(matches);
    let raw_token: Token = Token(0);
    let scancodes_token: Token = Token(1);

    let mut poll = Poll::new().expect("failed to create poll");

    if matches.is_present("LEARNING") {
        let mut learning_mode = false;

        if lircdev.can_measure_carrier() {
            if let Err(err) = lircdev.set_measure_carrier(true) {
                eprintln!(
                    "error: failed to enable measure carrier: {}",
                    err.to_string()
                );
                std::process::exit(1);
            }
            learning_mode = true;
        }

        if lircdev.can_use_wideband_receiver() {
            if let Err(err) = lircdev.set_wideband_receiver(true) {
                eprintln!(
                    "error: failed to enable wideband receiver: {}",
                    err.to_string()
                );
                std::process::exit(1);
            }
            learning_mode = true;
        }

        if !learning_mode {
            eprintln!("error: lirc device does not support learning mode");
            std::process::exit(1);
        }
    } else {
        if lircdev.can_measure_carrier() {
            if let Err(err) = lircdev.set_measure_carrier(false) {
                eprintln!(
                    "error: failed to disable measure carrier: {}",
                    err.to_string()
                );
                std::process::exit(1);
            }
        }

        if lircdev.can_use_wideband_receiver() {
            if let Err(err) = lircdev.set_wideband_receiver(false) {
                eprintln!(
                    "error: failed to disable wideband receiver: {}",
                    err.to_string()
                );
                std::process::exit(1);
            }
        }
    }

    if let Some(timeout) = matches.value_of("TIMEOUT") {
        if let Ok(timeout) = timeout.parse() {
            if lircdev.can_set_timeout() {
                match lircdev.get_min_max_timeout() {
                    Ok(range) if range.contains(&timeout) => {
                        if let Err(err) = lircdev.set_timeout(timeout) {
                            eprintln!("error: {}", err.to_string());
                            std::process::exit(1);
                        }
                    }
                    Ok(range) => {
                        eprintln!(
                            "error: {} not in the range {}-{}",
                            timeout, range.start, range.end
                        );
                        std::process::exit(1);
                    }
                    Err(err) => {
                        eprintln!("error: {}", err.to_string());
                        std::process::exit(1);
                    }
                }
            } else {
                eprintln!("error: cannot set timeout");
                std::process::exit(1);
            }
        } else {
            eprintln!("error: timeout {} not valid", timeout);
            std::process::exit(1);
        }
    }

    let mut scandev = None;
    let mut rawdev = None;

    if lircdev.can_receive_raw() {
        poll.registry()
            .register(
                &mut SourceFd(&lircdev.file.as_raw_fd()),
                raw_token,
                Interest::READABLE,
            )
            .expect("failed to add raw poll");

        if lircdev.can_receive_scancodes() {
            let lircdev = open_lirc(matches);

            poll.registry()
                .register(
                    &mut SourceFd(&lircdev.file.as_raw_fd()),
                    scancodes_token,
                    Interest::READABLE,
                )
                .expect("failed to add scancodes poll");

            scandev = Some(lircdev);
        }

        rawdev = Some(lircdev);
    } else {
        if lircdev.can_receive_scancodes() {
            poll.registry()
                .register(
                    &mut SourceFd(&lircdev.file.as_raw_fd()),
                    scancodes_token,
                    Interest::READABLE,
                )
                .expect("failed to add scancodes poll");

            scandev = Some(lircdev);
        }
    };

    let mut rawbuf = Vec::with_capacity(1024);
    let mut carrier = None;
    let mut leading_space = true;
    let mut scanbuf = Vec::with_capacity(1024);
    let mut events = Events::with_capacity(4);

    loop {
        if let Some(lircdev) = &mut rawdev {
            if let Err(err) = lircdev.receive_raw(&mut rawbuf) {
                eprintln!("error: {}", err.to_string());
                std::process::exit(1);
            }

            for entry in &rawbuf {
                if entry.is_space() {
                    if !leading_space {
                        print!("-{} ", entry.value());
                    }
                } else if entry.is_pulse() {
                    if leading_space {
                        leading_space = false;
                        print!("raw ir: ")
                    }
                    print!("+{} ", entry.value());
                } else if entry.is_frequency() {
                    carrier = Some(entry.value());
                } else if entry.is_timeout() {
                    if let Some(freq) = carrier {
                        println!(" # timeout {}, carrier {}Hz", entry.value(), freq);
                        carrier = None;
                    } else {
                        println!(" # timeout {}", entry.value());
                    }
                    leading_space = true;
                }
            }
        }

        if let Some(lircdev) = &mut scandev {
            if let Err(err) = lircdev.receive_scancodes(&mut scanbuf) {
                eprintln!("error: {}", err.to_string());
                std::process::exit(1);
            }

            for entry in &scanbuf {
                let keycode = evdev::Key::new(entry.keycode as u16);

                println!(
                    "scancode: {}.{:#09}: scancode={:x} keycode={:?}{}{}",
                    entry.timestamp / 1_000_000_000,
                    entry.timestamp % 1_000_000_000,
                    entry.scancode,
                    keycode,
                    if (entry.flags & lirc::LIRC_SCANCODE_FLAG_REPEAT) != 0 {
                        " repeat"
                    } else {
                        ""
                    },
                    if (entry.flags & lirc::LIRC_SCANCODE_FLAG_TOGGLE) != 0 {
                        " toggle"
                    } else {
                        ""
                    },
                );
            }
        }

        poll.poll(&mut events, None).expect("poll should not fail");
    }
}
