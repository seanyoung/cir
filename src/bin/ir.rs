use aya::programs::LircMode2;
use clap::{App, AppSettings, Arg, SubCommand};
use evdev::{Device, InputEventKind, Key};
use irp::{Irp, Message, Pronto};
use itertools::Itertools;
use linux_infrared::{keymap, lirc, rcdev};
use mio::{unix::SourceFd, Events, Interest, Poll, Token};
use nix::fcntl::{FcntlArg, OFlag};
use std::convert::{TryFrom, TryInto};
use std::fs;
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

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
        .subcommand(
            SubCommand::with_name("decoder")
                .about("Configure IR decoder")
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
                .arg(
                    Arg::with_name("DELAY")
                        .long("delay")
                        .short("D")
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("PERIOD")
                        .long("period")
                        .short("P")
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("KEYMAP")
                        .long("write")
                        .short("w")
                        .takes_value(true)
                        .multiple(true),
                )
                .arg(Arg::with_name("CLEAR").long("clear").short("c"))
                .arg(Arg::with_name("VERBOSE").long("verbose").short("v")),
        )
        .subcommand(
            SubCommand::with_name("transmit")
                .about("Transmit IR")
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
                        .about("Encode using IRP langauge and transmit")
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
                        .about("Parse pronto hex code and transmit")
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
                        .about("Parse mode2 pulse space file and transmit")
                        .arg(
                            Arg::with_name("FILE")
                                .help("File to load and parse")
                                .required(true),
                        ),
                )
                .subcommand(
                    SubCommand::with_name("rawir")
                        .about("Parse raw IR and transmit")
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
        .subcommand(
            SubCommand::with_name("list")
                .about("List IR and CEC devices")
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
                .arg(Arg::with_name("READ").long("read-scancodes").short("l")),
        )
        .subcommand(
            SubCommand::with_name("receive")
                .about("Receive IR and print to stdout")
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
        ("transmit", Some(matches)) => {
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
        ("list", Some(matches)) => match rcdev::enumerate_rc_dev() {
            Ok(list) => print_rc_dev(&list, matches),
            Err(err) => {
                eprintln!("error: {}", err.to_string());
                std::process::exit(1);
            }
        },
        ("receive", Some(matches)) => {
            receive(matches);
        }
        ("decoder", Some(matches)) => {
            decoder(matches);
        }
        _ => unreachable!(),
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

fn print_rc_dev(list: &[rcdev::Rcdev], matches: &clap::ArgMatches) {
    let mut printed = 0;

    for rcdev in list {
        if let Some(needlelircdev) = matches.value_of("LIRCDEV") {
            if let Some(lircdev) = &rcdev.lircdev {
                if lircdev == needlelircdev {
                    // ok
                } else {
                    continue;
                }
            } else {
                continue;
            }
        } else if let Some(needlercdev) = matches.value_of("RCDEV") {
            if needlercdev != rcdev.name {
                continue;
            }
        }

        println!("{}:", rcdev.name);

        println!("\tDevice Name\t\t: {}", rcdev.device_name);
        println!("\tDriver\t\t\t: {}", rcdev.driver);
        if !rcdev.default_keymap.is_empty() {
            println!("\tDefault Keymap\t\t: {}", rcdev.default_keymap);
        }
        if let Some(inputdev) = &rcdev.inputdev {
            println!("\tInput Device\t\t: {}", inputdev);

            match Device::open(&inputdev) {
                Ok(inputdev) => {
                    let id = inputdev.input_id();

                    println!("\tBus\t\t\t: {}", id.bus_type());

                    println!(
                        "\tVendor/product\t\t: {:04x}:{:04x} version 0x{:04x}",
                        id.product(),
                        id.vendor(),
                        id.version()
                    );

                    if let Some(repeat) = inputdev.get_auto_repeat() {
                        println!(
                            "\tRepeat\t\t\t: delay {} ms, period {} ms",
                            repeat.delay, repeat.period
                        );
                    }

                    if matches.is_present("READ") {
                        let mut index = 0;

                        loop {
                            match inputdev.get_scancode_by_index(index) {
                                Ok((keycode, scancode)) => {
                                    match scancode.len() {
                                        8 => {
                                            // kernel v5.7 and later give 64 bit scancodes
                                            let scancode =
                                                u64::from_ne_bytes(scancode.try_into().unwrap());
                                            let keycode = evdev::Key::new(keycode as u16);

                                            println!(
                                                "\tScancode\t\t: 0x{:08x} => {:?}",
                                                scancode, keycode
                                            );
                                        }
                                        4 => {
                                            // kernel v5.6 and earlier give 32 bit scancodes
                                            let scancode =
                                                u32::from_ne_bytes(scancode.try_into().unwrap());
                                            let keycode = evdev::Key::new(keycode as u16);

                                            println!(
                                                "\tScancode\t\t: 0x{:08x} => {:?}",
                                                scancode, keycode
                                            )
                                        }
                                        len => panic!(
                                            "scancode should be 4 or 8 bytes long, not {}",
                                            len
                                        ),
                                    }

                                    index += 1;
                                }
                                Err(err) if err.kind() == std::io::ErrorKind::InvalidInput => break,
                                Err(err) => {
                                    eprintln!("error: {}", err);
                                    std::process::exit(1);
                                }
                            }
                        }
                    }
                }
                Err(err) => {
                    println!("\tInput properties\t: {}", err);
                }
            };
        }
        if let Some(lircdev) = &rcdev.lircdev {
            println!("\tLIRC Device\t\t: {}", lircdev);

            match lirc::open(&PathBuf::from(lircdev)) {
                Ok(mut lircdev) => {
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
                                        format!("{} to {} microseconds", range.start, range.end),
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

                        match LircMode2::query(lircdev.as_raw_fd()) {
                            Ok(list) => {
                                print!("\tBPF protocols\t\t: ");

                                let mut first = true;

                                for e in list {
                                    if first {
                                        first = false;
                                    } else {
                                        print!(", ")
                                    }

                                    match e.info() {
                                        Ok(info) => match info.name_as_str() {
                                            Some(name) => print!("{}", name),
                                            None => print!("{}", info.id()),
                                        },
                                        Err(err) => {
                                            print!("{}", err.to_string())
                                        }
                                    }
                                }

                                println!();
                            }
                            Err(err) => {
                                println!("\tBPF protocols\t\t: {}", err.to_string())
                            }
                        }
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

                        if lircdev.can_set_send_transmitter_mask() {
                            println!(
                                "\tLIRC Transmitters\t: {}",
                                match lircdev.num_transmitters() {
                                    Ok(count) => format!("{}", count),
                                    Err(err) => err.to_string(),
                                }
                            );
                        } else {
                            println!("\tLIRC Transmitters\t: unknown");
                        }
                    } else {
                        println!("\tLIRC Transmitter\t: no");
                    }
                }
                Err(err) => {
                    println!("\tLIRC Features\t\t: {}", err.to_string());
                }
            }
        }

        if !rcdev.supported_protocols.is_empty() {
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

        printed += 1;
    }

    if printed == 0 {
        if let Some(lircdev) = matches.value_of("LIRCDEV") {
            eprintln!("error: no lirc device named {}", lircdev);
        } else if let Some(rcdev) = matches.value_of("RCDEV") {
            eprintln!("error: no rc device named {}", rcdev);
        } else {
            eprintln!("error: no devices found");
        }
        std::process::exit(1);
    }
}

/// Enumerate all rc devices and find the lirc and input devices
fn find_devices(matches: &clap::ArgMatches) -> (Option<String>, Option<String>) {
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

fn open_lirc(matches: &clap::ArgMatches) -> lirc::Lirc {
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

// Clippy comparison_chain doesn't make any sense. It make the code _worse_
#[allow(clippy::comparison_chain)]
fn receive(matches: &clap::ArgMatches) {
    let (lircdev, inputdev) = find_devices(matches);
    let raw_token: Token = Token(0);
    let scancodes_token: Token = Token(1);
    let input_token: Token = Token(2);

    let mut poll = Poll::new().expect("failed to create poll");
    let mut scandev = None;
    let mut rawdev = None;
    let mut eventdev = None;

    if let Some(lircdev) = lircdev {
        let lircpath = PathBuf::from(lircdev);

        let mut lircdev = match lirc::open(&lircpath) {
            Ok(l) => l,
            Err(s) => {
                eprintln!("error: {}: {}", lircpath.display(), s);
                std::process::exit(1);
            }
        };

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

        if lircdev.can_receive_raw() {
            poll.registry()
                .register(
                    &mut SourceFd(&lircdev.as_raw_fd()),
                    raw_token,
                    Interest::READABLE,
                )
                .expect("failed to add raw poll");

            if lircdev.can_receive_scancodes() {
                let mut lircdev = open_lirc(matches);

                lircdev
                    .scancode_mode()
                    .expect("should be able to switch to scancode mode");

                let raw_fd = lircdev.as_raw_fd();

                nix::fcntl::fcntl(raw_fd, FcntlArg::F_SETFL(OFlag::O_NONBLOCK))
                    .expect("should be able to set non-blocking");

                poll.registry()
                    .register(&mut SourceFd(&raw_fd), scancodes_token, Interest::READABLE)
                    .expect("failed to add scancodes poll");

                scandev = Some(lircdev);
            }

            rawdev = Some(lircdev);
        } else if lircdev.can_receive_scancodes() {
            let raw_fd = lircdev.as_raw_fd();

            nix::fcntl::fcntl(raw_fd, FcntlArg::F_SETFL(OFlag::O_NONBLOCK))
                .expect("should be able to set non-blocking");

            poll.registry()
                .register(&mut SourceFd(&raw_fd), scancodes_token, Interest::READABLE)
                .expect("failed to add scancodes poll");

            scandev = Some(lircdev);
        }
    }

    if let Some(inputdev) = inputdev {
        let inputdev = match Device::open(&inputdev) {
            Ok(l) => l,
            Err(s) => {
                eprintln!("error: {}: {}", inputdev, s);
                std::process::exit(1);
            }
        };

        let raw_fd = inputdev.as_raw_fd();

        nix::fcntl::fcntl(raw_fd, FcntlArg::F_SETFL(OFlag::O_NONBLOCK))
            .expect("should be able to set non-blocking");

        poll.registry()
            .register(&mut SourceFd(&raw_fd), input_token, Interest::READABLE)
            .expect("failed to add scancodes poll");

        eventdev = Some(inputdev);
    }

    let mut rawbuf = Vec::with_capacity(1024);
    let mut carrier = None;
    let mut leading_space = true;
    let mut scanbuf = Vec::with_capacity(1024);
    let mut events = Events::with_capacity(4);
    let mut last_event_time = None;
    let mut last_lirc_time = None;

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

                let timestamp = Duration::new(
                    entry.timestamp / 1_000_000_000,
                    (entry.timestamp % 1_000_000_000) as u32,
                );

                if let Some(last) = last_lirc_time {
                    if timestamp > last {
                        print!(
                            "lirc: later: {}, ",
                            humantime::format_duration(timestamp - last)
                        );
                    } else if timestamp < last {
                        print!(
                            "lirc: earlier: {}, ",
                            humantime::format_duration(last - timestamp)
                        );
                    } else {
                        print!("lirc: same time, ");
                    }
                } else {
                    print!("lirc: ");
                };

                last_lirc_time = Some(timestamp);

                println!(
                    "scancode={:x} keycode={:?}{}{}",
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

        if let Some(eventdev) = &mut eventdev {
            match eventdev.fetch_events() {
                Ok(iterator) => {
                    for ev in iterator {
                        let timestamp = ev
                            .timestamp()
                            .elapsed()
                            .expect("input time should never exceed system time");

                        if let Some(last) = last_event_time {
                            if timestamp > last {
                                print!(
                                    "event: later: {}, type: ",
                                    humantime::format_duration(timestamp - last)
                                );
                            } else if timestamp < last {
                                print!(
                                    "event: earlier: {}, type: ",
                                    humantime::format_duration(last - timestamp)
                                );
                            } else {
                                print!("event: same time, type: ");
                            }
                        } else {
                            print!("event: type: ");
                        };

                        last_event_time = Some(timestamp);

                        let ty = ev.event_type();
                        let value = ev.value();

                        match ev.kind() {
                            InputEventKind::Misc(misc) => {
                                println!("{:?}: {:?} = {:#010x}", ty, misc, value);
                            }
                            InputEventKind::Synchronization(sync) => {
                                println!("{:?}", sync);
                            }
                            InputEventKind::Key(key) if value == 1 => {
                                println!("KEY_DOWN: {:?} ", key);
                            }
                            InputEventKind::Key(key) if value == 0 => {
                                println!("KEY_UP: {:?}", key);
                            }
                            InputEventKind::Key(key) => {
                                println!("{:?} {:?} {}", ty, key, value);
                            }
                            InputEventKind::RelAxis(rel) => {
                                println!("{:?} {:?} {:#08x}", ty, rel, value);
                            }
                            InputEventKind::AbsAxis(abs) => {
                                println!("{:?} {:?} {:#08x}", ty, abs, value);
                            }
                            InputEventKind::Switch(switch) => {
                                println!("{:?} {:?} {:#08x}", ty, switch, value);
                            }
                            InputEventKind::Led(led) => {
                                println!("{:?} {:?} {:#08x}", ty, led, value);
                            }
                            InputEventKind::Sound(sound) => {
                                println!("{:?} {:?} {:#08x}", ty, sound, value);
                            }
                            InputEventKind::Other => {
                                println!("other");
                            }
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => (),
                Err(e) => {
                    eprintln!("error: {}", e.to_string());
                    std::process::exit(1);
                }
            }
        }

        poll.poll(&mut events, None).expect("poll should not fail");
    }
}

fn decoder(matches: &clap::ArgMatches) {
    let (_, inputdev) = find_devices(matches);

    if inputdev.is_none() {
        eprintln!("error: input device is missing");
        std::process::exit(1);
    }

    let inputdev = inputdev.unwrap();

    let mut inputdev = match Device::open(&inputdev) {
        Ok(l) => l,
        Err(s) => {
            eprintln!("error: {}: {}", inputdev, s);
            std::process::exit(1);
        }
    };

    if matches.is_present("DELAY") || matches.is_present("PERIOD") {
        let mut repeat = inputdev
            .get_auto_repeat()
            .expect("auto repeat is supported");

        if let Some(delay) = matches.value_of("DELAY") {
            repeat.delay = match delay.parse() {
                Ok(d) => d,
                Err(_) => {
                    eprintln!("error: ‘{}’ is not a valid delay", delay);
                    std::process::exit(1);
                }
            }
        }

        if let Some(period) = matches.value_of("PERIOD") {
            repeat.period = match period.parse() {
                Ok(d) => d,
                Err(_) => {
                    eprintln!("error: ‘{}’ is not a valid period", period);
                    std::process::exit(1);
                }
            }
        }

        if let Err(e) = inputdev.update_auto_repeat(&repeat) {
            eprintln!("error: failed to update autorepeat: {}", e);
            std::process::exit(1);
        }
    }

    if matches.is_present("CLEAR") {
        loop {
            match inputdev.update_scancode_by_index(0, Key::KEY_RESERVED, &[]) {
                Ok(_) => (),
                Err(e) if e.kind() == std::io::ErrorKind::InvalidInput => break,
                Err(e) => {
                    eprintln!("error: unable to remove scancode entry: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }

    if let Some(keymaps) = matches.values_of("KEYMAP") {
        for keymap_filename in keymaps {
            let keymap_contents = fs::read_to_string(keymap_filename).unwrap();

            let map = match keymap::parse(&keymap_contents, keymap_filename) {
                Ok(map) => map,
                Err(e) => {
                    eprintln!("error: {}: {}", keymap_filename, e);
                    std::process::exit(1);
                }
            };

            for p in map.protocols {
                if let Some(scancodes) = p.scancodes {
                    for (scancode, keycode) in scancodes {
                        let key = match Key::from_str(&keycode) {
                            Ok(key) => key,
                            Err(_) => {
                                eprintln!("error: ‘{}’ is not a valid keycode", keycode);
                                continue;
                            }
                        };

                        let scancode =
                            match u64::from_str_radix(scancode.trim_start_matches("0x"), 16) {
                                Ok(scancode) => scancode,
                                Err(_) => {
                                    eprintln!("error: ‘{}’ is not a valid scancode", scancode);
                                    continue;
                                }
                            };

                        // Kernels from before v5.7 want the scancode in 4 bytes; try this if possible
                        let scancode = if let Ok(scancode) = u32::try_from(scancode) {
                            scancode.to_ne_bytes().to_vec()
                        } else {
                            scancode.to_ne_bytes().to_vec()
                        };

                        match inputdev.update_scancode(key, &scancode) {
                            Ok(_) => (),
                            Err(e) => {
                                eprintln!(
                                    "error: failed to update key mapping from scancode {:x?} to {:?}: {}",
                                    scancode,
                                    key,
                                    e
                                );
                                std::process::exit(1);
                            }
                        }
                    }
                }
            }
        }
    }
}
