extern crate clap;
extern crate num;
extern crate serde;
extern crate serde_derive;
extern crate toml;

mod irp;
mod keymap;
mod mode2;
mod pronto;
mod rawir;

use clap::{App, Arg};
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
        .get_matches();

    let arg = if matches.is_present("FILE") {
        fs::read_to_string(matches.value_of("FILE").unwrap()).unwrap()
    } else {
        matches.value_of("INPUT").unwrap().to_string()
    };
    if matches.is_present("RAWIR") {
        match rawir::parse(&arg) {
            Ok(ir) => println!("{}", rawir::print_to_string(ir)),
            Err(s) => eprintln!("error: {}", s),
        }
    }
    if matches.is_present("MODE2") {
        match mode2::parse(&arg) {
            Ok(ir) => println!("{}", rawir::print_to_string(ir)),
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
