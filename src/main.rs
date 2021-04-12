mod keymap;

use clap::{App, Arg, SubCommand};
use std::fs;

fn main() {
    let matches = App::new("ir-ctl")
        .version("0.1")
        .author("Sean Young")
        .about("Linux Infrared Control")
        .subcommand(
            SubCommand::with_name("render")
                .about("render raw IR")
                .subcommand(
                    SubCommand::with_name("irp")
                        .about("Render using IRP langauge")
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
                        )
                        .arg(Arg::with_name("IRP").help("IRP protocol").required(true)),
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
        .get_matches();

    if let ("render", Some(matches)) = matches.subcommand() {
        match matches.subcommand() {
            ("irp", Some(matches)) => {
                let mut vars = irp::render::Vartable::new();

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

                match irp::render::render(i, vars, repeats) {
                    Ok(ir) => println!("{}", irp::rawir::print_to_string(&ir)),
                    Err(s) => eprintln!("error: {}", s),
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

                let pronto = match irp::pronto::parse(&pronto) {
                    Ok(pronto) => pronto,
                    Err(err) => {
                        eprintln!("error: {}", err);
                        std::process::exit(2);
                    }
                };

                let ir = pronto.render(repeats);

                println!("{}", irp::rawir::print_to_string(&ir));
            }
            ("rawir", Some(matches)) => {
                let rawir = matches.value_of("RAWIR").unwrap();

                match irp::rawir::parse(&rawir) {
                    Ok(ir) => println!("{}", irp::rawir::print_to_string(&ir)),
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
                    Ok(ir) => println!("{}", irp::rawir::print_to_string(&ir)),
                    Err(s) => {
                        eprintln!("error: {}", s);
                        std::process::exit(2);
                    }
                }
            }
            _ => (),
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
