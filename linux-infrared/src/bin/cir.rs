use aya::programs::LircMode2;
use clap::{App, AppSettings, Arg, ArgGroup, SubCommand};
use evdev::Device;
use itertools::Itertools;
use linux_infrared::{lirc, rcdev};
use std::convert::TryInto;
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;

mod commands;

fn main() {
    let matches = App::new("cir")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Sean Young <sean@mess.org>")
        .about("Consumer Infrared")
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
                                .conflicts_with("PRONTO")
                                .number_of_values(1),
                        )
                        .arg(
                            Arg::with_name("IRP")
                                .help("IRP protocol")
                                .required(true)
                                .last(true),
                        ),
                )
                .subcommand(
                    SubCommand::with_name("pronto")
                        .about("Parse pronto hex code and print as raw IR")
                        .arg(
                            Arg::with_name("REPEATS")
                                .long("repeats")
                                .short("r")
                                .help("Number of times to repeat signal")
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
                    SubCommand::with_name("rawir").about("Parse raw IR").arg(
                        Arg::with_name("RAWIR")
                            .help("Raw IR to parse")
                            .required(true),
                    ),
                ),
        )
        .subcommand(
            SubCommand::with_name("config")
                .about("Configure IR decoder")
                .arg(
                    Arg::with_name("LIRCDEV")
                        .long("device")
                        .short("d")
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("RCDEV")
                        .long("rcdev")
                        .short("s")
                        .takes_value(true),
                )
                .group(ArgGroup::with_name("DEVICE").args(&["RCDEV", "LIRCDEV"]))
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
                .arg(
                    Arg::with_name("TIMEOUT")
                        .help("Set IR timeout")
                        .long("timeout")
                        .short("t")
                        .takes_value(true),
                )
                .arg(Arg::with_name("CLEAR").long("clear").short("c"))
                .arg(Arg::with_name("VERBOSE").long("verbose").short("v"))
                .arg(
                    Arg::with_name("CFGFILE")
                        .long("auto-load")
                        .short("a")
                        .help("Auto-load keymaps, based on configuration file")
                        .default_value("/etc/rc_maps.cfg")
                        .conflicts_with_all(&["DELAY", "PERIOD", "KEYMAP", "CLEAR", "TIMEOUT"])
                        .requires("DEVICE")
                        .takes_value(true),
                ),
        )
        .subcommand(
            SubCommand::with_name("transmit")
                .about("Transmit IR")
                .setting(AppSettings::SubcommandRequiredElseHelp)
                .arg(
                    Arg::with_name("LIRCDEV")
                        .help("Select device to use by lirc chardev (e.g. /dev/lirc1)")
                        .long("device")
                        .short("d")
                        .global(true)
                        .takes_value(true)
                        .conflicts_with("RCDEV"),
                )
                .arg(
                    Arg::with_name("RCDEV")
                        .help("Select device to use by rc core device (e.g. rc0)")
                        .long("rcdev")
                        .short("s")
                        .global(true)
                        .takes_value(true)
                        .conflicts_with("LIRCDEV"),
                )
                .arg(
                    Arg::with_name("VERBOSE")
                        .long("verbose")
                        .short("v")
                        .global(true)
                        .help("verbose output"),
                )
                .arg(
                    Arg::with_name("TRANSMITTERS")
                        .help("Comma separated list of transmitters to use, starting from 1")
                        .long("transmitters")
                        .short("e")
                        .global(true)
                        .takes_value(true)
                        .multiple(true)
                        .require_delimiter(true),
                )
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
                                .help("Number of times to repeat signal")
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
                            Arg::with_name("CARRIER")
                                .long("carrier")
                                .short("c")
                                .help("Set carrier in Hz, 0 for unmodulated")
                                .takes_value(true),
                        )
                        .arg(
                            Arg::with_name("DUTY_CYCLE")
                                .long("duty-cycle")
                                .short("u")
                                .help("Set send duty cycle % (1 to 99)")
                                .takes_value(true),
                        )
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
                                .help("Set carrier in Hz, 0 for unmodulated")
                                .takes_value(true),
                        )
                        .arg(
                            Arg::with_name("DUTY_CYCLE")
                                .long("duty-cycle")
                                .short("u")
                                .help("Set send duty cycle % (1 to 99)")
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
                        .help("Select device to use by lirc chardev (e.g. /dev/lirc1)")
                        .takes_value(true)
                        .conflicts_with("RCDEV"),
                )
                .arg(
                    Arg::with_name("RCDEV")
                        .help("Select device to use by rc core device (e.g. rc0)")
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
                        .help("Select device to use by lirc chardev (e.g. /dev/lirc1)")
                        .takes_value(true)
                        .conflicts_with("RCDEV"),
                )
                .arg(
                    Arg::with_name("RCDEV")
                        .help("Select device to use by rc core device (e.g. rc0)")
                        .long("rcdev")
                        .short("s")
                        .takes_value(true)
                        .conflicts_with("LIRCDEV"),
                )
                .arg(
                    Arg::with_name("LEARNING")
                        .help("Use short-range learning mode")
                        .long("learning-mode")
                        .short("l"),
                ),
        )
        .get_matches();

    match matches.subcommand() {
        ("encode", Some(matches)) => commands::encode::encode(matches),
        ("transmit", Some(matches)) => commands::transmit::transmit(matches),
        ("list", Some(matches)) => match rcdev::enumerate_rc_dev() {
            Ok(list) => print_rc_dev(&list, matches),
            Err(err) => {
                eprintln!("error: {}", err);
                std::process::exit(1);
            }
        },
        ("receive", Some(matches)) => commands::receive::receive(matches),
        ("config", Some(matches)) => commands::config::config(matches),
        _ => unreachable!(),
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
                                            print!("{}", err)
                                        }
                                    }
                                }

                                println!();
                            }
                            Err(err) => {
                                println!("\tBPF protocols\t\t: {}", err)
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
                    println!("\tLIRC Features\t\t: {}", err);
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
