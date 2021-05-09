mod keymap;

use clap::{App, Arg, SubCommand};
use irp::pronto::Pronto;
use irp::Message;
use lirc::lirc_open;
use std::fs;
use std::path::PathBuf;

fn main() {
    let matches = App::new("ir")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Sean Young <sean@mess.org>")
        .about("Linux Infrared Control")
        .subcommand(
            SubCommand::with_name("encode")
                .about("Encode IR and print to stdout")
                .subcommand(
                    SubCommand::with_name("irp")
                        .about("Encode using IRP langauge")
                        .arg(
                            Arg::with_name("IRP")
                                .help("IRP protocol")
                                .required(true)
                                .index(1),
                        )
                        .arg(
                            Arg::with_name("REPEATS")
                                .long("repeats")
                                .short("r")
                                .takes_value(true),
                        )
                        .arg(
                            Arg::with_name("FIELD")
                                .long("field")
                                .short("f")
                                .takes_value(true)
                                .multiple(true),
                        ),
                )
                .subcommand(
                    SubCommand::with_name("pronto")
                        .arg(
                            Arg::with_name("REPEATS")
                                .long("repeats")
                                .short("r")
                                .takes_value(true),
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
                .arg(
                    Arg::with_name("LIRCDEV")
                        .long("device")
                        .short("d")
                        .default_value("/dev/lirc0")
                        .takes_value(true),
                )
                .arg(Arg::with_name("VERBOSE").long("verbose").short("v"))
                .about("Encode IR and print")
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
                                .takes_value(true)
                                .multiple(true),
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

            let lircpath = PathBuf::from(matches.value_of("LIRCDEV").unwrap());

            let mut lircdev = match lirc_open(&lircpath) {
                Ok(l) => l,
                Err(s) => {
                    eprintln!("error: {}: {}", lircpath.display(), s);
                    std::process::exit(1);
                }
            };

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
        _ => {
            eprintln!("command required");
            std::process::exit(2);
        }
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
            let mut vars = irp::encode::Vartable::new();

            let i = matches.value_of("IRP").unwrap();

            if let Some(values) = matches.values_of("FIELD") {
                for f in values {
                    let list: Vec<&str> = f.split('=').collect();

                    if list.len() != 2 {
                        eprintln!("argument to --field must be X=1");
                    }

                    let value = if list[1].starts_with("0x") {
                        i64::from_str_radix(&list[1][2..], 16).unwrap()
                    } else {
                        i64::from_str_radix(list[1], 10).unwrap()
                    };

                    vars.set(list[0].to_string(), value, 8);
                }
            }

            let repeats = match matches.value_of("REPEATS") {
                None => 0,
                Some(s) => match i64::from_str_radix(s, 10) {
                    Ok(num) => num,
                    Err(_) => {
                        eprintln!("error: {} is not numeric", s);
                        std::process::exit(2);
                    }
                },
            };

            match irp::encode::encode(i, vars, repeats) {
                Ok(m) => m,
                Err(s) => {
                    eprintln!("error: {}", s);
                    std::process::exit(2);
                }
            }
        }
        ("pronto", Some(matches)) => {
            let pronto = matches.value_of("PRONTO").unwrap();

            let repeats = match matches.value_of("REPEATS") {
                None => 0,
                Some(s) => match usize::from_str_radix(s, 10) {
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
