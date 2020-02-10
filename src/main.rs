extern crate clap;
extern crate num;

mod mode2;
mod pronto;
mod rawir;

use clap::{App, Arg};

fn main() {
    let matches = App::new("ir-ctl")
        .version("0.1")
        .author("Sean Young")
        .about("Linux Infrared Control")
        .arg(Arg::with_name("INPUT").help("IR to send").required(true))
        .arg(Arg::with_name("MODE2").long("mode2"))
        .arg(Arg::with_name("RAWIR").long("rawir"))
        .arg(Arg::with_name("PRONTO").long("pronto"))
        .get_matches();

    if matches.is_present("RAWIR") {
        match rawir::parse(&matches.value_of("INPUT").unwrap()) {
            Ok(ir) => println!("{}", rawir::print_to_string(ir)),
            Err(s) => eprintln!("error: {}", s),
        }
    }
    if matches.is_present("MODE2") {
        match mode2::parse(&matches.value_of("INPUT").unwrap()) {
            Ok(ir) => println!("{}", rawir::print_to_string(ir)),
            Err(s) => eprintln!("error: {}", s),
        }
    }
    if matches.is_present("PRONTO") {
        match pronto::parse(&matches.value_of("INPUT").unwrap()) {
            Ok(ir) => println!("{:?}", ir),
            Err(s) => eprintln!("error: {}", s),
        }
    }
}
