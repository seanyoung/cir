use ir_ctl::irp;
use ir_ctl::keymap;
use ir_ctl::mode2;
use ir_ctl::pronto;
use ir_ctl::rawir;

use clap::{App, Arg, SubCommand};
use std::fs;

fn main() {
    let matches = App::new("ir-ctl")
        .version("0.1")
        .author("Sean Young")
        .about("Linux Infrared Control")
        .arg(Arg::with_name("INPUT").help("IR to send"))
        .arg(Arg::with_name("MODE2").long("mode2"))
        .arg(Arg::with_name("IRP").long("irp"))
        .arg(Arg::with_name("RAWIR").long("rawir"))
        .arg(Arg::with_name("PRONTO").long("pronto"))
        .arg(Arg::with_name("KEYMAP").long("keymap"))
        .arg(Arg::with_name("FILE").long("file").takes_value(true))
        .subcommand(
            SubCommand::with_name("render")
                .about("render raw IR")
                .arg(
                    Arg::with_name("IRP")
                        .long("irp")
                        .takes_value(true)
                        .required(true),
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
        .get_matches();

    if let ("render", Some(matches)) = matches.subcommand() {
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
            Ok(ir) => println!("{}", rawir::print_to_string(&ir)),
            Err(s) => eprintln!("error: {}", s),
        }
    } else {
        let arg = if matches.is_present("FILE") {
            fs::read_to_string(matches.value_of("FILE").unwrap()).unwrap()
        } else {
            matches.value_of("INPUT").unwrap().to_string()
        };
        if matches.is_present("RAWIR") {
            match rawir::parse(&arg) {
                Ok(ir) => println!("{}", rawir::print_to_string(&ir)),
                Err(s) => eprintln!("error: {}", s),
            }
        }
        if matches.is_present("MODE2") {
            match mode2::parse(&arg) {
                Ok(ir) => println!("{}", rawir::print_to_string(&ir)),
                Err(s) => eprintln!("error: {}", s),
            }
        }
        if matches.is_present("PRONTO") {
            match pronto::parse(&arg) {
                Ok(ir) => println!("{:?}", ir),
                Err(s) => eprintln!("error: {}", s),
            }
        }
        if matches.is_present("IRP") {
            match irp::parse(&arg) {
                Ok(ir) => println!("{:?}", ir),
                Err(s) => eprintln!("error: {}", s),
            }
        }
        if matches.is_present("KEYMAP") {
            match keymap::parse(&arg) {
                Ok(ir) => println!("{:?}", ir),
                Err(s) => eprintln!("error: {}", s),
            }
        }
    }
}
