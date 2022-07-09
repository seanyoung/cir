use aya::programs::LircMode2;
use cir::{lirc, log::Log, rcdev};
use clap::{Arg, ArgGroup, Command};
use evdev::Device;
use itertools::Itertools;
use std::{convert::TryInto, os::unix::io::AsRawFd, path::PathBuf};

mod commands;

fn main() {
    let matches = Command::new("cir")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Sean Young <sean@mess.org>")
        .about("Consumer Infrared")
        .arg_required_else_help(true)
        .arg(
            Arg::new("verbosity")
                .short('v')
                .long("verbose")
                .global(true)
                .multiple_occurrences(true)
                .help("Increase message verbosity"),
        )
        .arg(
            Arg::new("quiet")
                .short('q')
                .long("quiet")
                .global(true)
                .help("Silence all warnings"),
        )
        .subcommand(
            Command::new("decode")
                .about("Decode IR")
                .arg_required_else_help(true)
                .next_help_heading("INPUT")
                .arg(
                    Arg::new("LIRCDEV")
                        .help("Select device to use by lirc chardev (e.g. /dev/lirc1)")
                        .long("device")
                        .short('d')
                        .takes_value(true)
                        .conflicts_with_all(&["RCDEV", "RAWIR", "FILE"]),
                )
                .arg(
                    Arg::new("RCDEV")
                        .help("Select device to use by rc core device (e.g. rc0)")
                        .long("rcdev")
                        .short('s')
                        .takes_value(true)
                        .conflicts_with_all(&["LIRCDEV", "RAWIR", "FILE"]),
                )
                .arg(
                    Arg::new("LEARNING")
                        .help("Use short-range learning mode")
                        .long("learning-mode")
                        .short('l'),
                )
                .group(ArgGroup::new("DEVICE").args(&["RCDEV", "LIRCDEV"]))
                .arg(
                    Arg::new("FILE")
                        .long("file")
                        .short('f')
                        .help("Read from rawir or mode2 file")
                        .takes_value(true)
                        .allow_invalid_utf8(true)
                        .multiple_occurrences(true)
                        .conflicts_with_all(&["LEARNING", "LIRCDEV", "RCDEV", "RAWIR"]),
                )
                .arg(
                    Arg::new("RAWIR")
                        .long("raw")
                        .short('r')
                        .help("Raw IR text")
                        .takes_value(true)
                        .multiple_occurrences(true)
                        .conflicts_with_all(&["LEARNING", "LIRCDEV", "RCDEV", "FILE"]),
                )
                .next_help_heading("PROTOCOL")
                .arg(
                    Arg::new("IRP")
                        .long("irp")
                        .short('i')
                        .help("Decode using IRP language")
                        .takes_value(true)
                        .required(true)
                        .conflicts_with("LIRCDCONF"),
                )
                .arg(
                    Arg::new("LIRCDCONF")
                        .long("lircd")
                        .short('c')
                        .help("Decode using lircd.conf file and print codes")
                        .allow_invalid_utf8(true)
                        .takes_value(true)
                        .required(true)
                        .conflicts_with("IRP"),
                )
                .arg(
                    Arg::new("GRAPHVIZ")
                        .help("Save the state machine as graphviz dot files")
                        .takes_value(true)
                        .value_parser(["nfa", "nfa-step"])
                        .long("graphviz"),
                ),
        )
        .subcommand(
            Command::new("encode")
                .about("Encode IR and print to stdout")
                .arg_required_else_help(true)
                .subcommand(
                    Command::new("irp")
                        .about("Encode using IRP language")
                        .arg(
                            Arg::new("PRONTO")
                                .help("Encode IRP to pronto hex")
                                .long("pronto")
                                .short('p'),
                        )
                        .arg(
                            Arg::new("REPEATS")
                                .help("Number of IRP repeats to encode")
                                .long("repeats")
                                .short('r')
                                .conflicts_with("PRONTO")
                                .takes_value(true)
                                .default_value("1"),
                        )
                        .arg(
                            Arg::new("FIELD")
                                .help("Set input variable like KEY=VALUE")
                                .long("field")
                                .short('f')
                                .takes_value(true)
                                .multiple_occurrences(true)
                                .conflicts_with("PRONTO"),
                        )
                        .arg(Arg::new("IRP").help("IRP protocol").required(true)),
                )
                .subcommand(
                    Command::new("pronto")
                        .about("Parse pronto hex code and print as raw IR")
                        .arg(
                            Arg::new("REPEATS")
                                .long("repeats")
                                .short('r')
                                .help("Number of times to repeat signal")
                                .takes_value(true)
                                .default_value("1"),
                        )
                        .arg(Arg::new("PRONTO").help("Pronto Hex code").required(true)),
                )
                .subcommand(
                    Command::new("rawir")
                        .about("Parse raw IR and print")
                        .arg(
                            Arg::new("FILE")
                                .long("file")
                                .short('f')
                                .help("Read from rawir or mode2 file")
                                .takes_value(true)
                                .allow_invalid_utf8(true)
                                .multiple_occurrences(true),
                        )
                        .arg(
                            Arg::new("GAP")
                                .long("gap")
                                .short('g')
                                .help("Set gap after each file")
                                .takes_value(true)
                                .multiple_occurrences(true),
                        )
                        .arg(
                            Arg::new("RAWIR")
                                .help("Raw IR text")
                                .multiple_occurrences(true),
                        ),
                )
                .subcommand(
                    Command::new("lircd")
                        .about("Parse lircd.conf file and print codes as raw IR")
                        .arg(
                            Arg::new("CONF")
                                .help("lircd.conf file")
                                .allow_invalid_utf8(true)
                                .required(true),
                        )
                        .arg(
                            Arg::new("REMOTE")
                                .long("remote")
                                .short('m')
                                .help("Use codes from specific remote")
                                .takes_value(true),
                        )
                        .arg(
                            Arg::new("REPEATS")
                                .long("repeats")
                                .short('r')
                                .help("Number of times to repeat signal")
                                .takes_value(true)
                                .default_value("0"),
                        )
                        .arg(
                            Arg::new("CODES")
                                .help("Code to send")
                                .multiple_occurrences(true)
                                .takes_value(true),
                        ),
                ),
        )
        .subcommand(
            Command::new("config")
                .about("Configure IR decoder")
                .arg(
                    Arg::new("LIRCDEV")
                        .help("Select device to use by lirc chardev (e.g. /dev/lirc1)")
                        .long("device")
                        .short('d')
                        .takes_value(true),
                )
                .arg(
                    Arg::new("RCDEV")
                        .help("Select device to use by rc core device (e.g. rc0)")
                        .long("rcdev")
                        .short('s')
                        .takes_value(true),
                )
                .group(ArgGroup::new("DEVICE").args(&["RCDEV", "LIRCDEV"]))
                .arg(Arg::new("DELAY").long("delay").short('D').takes_value(true))
                .arg(
                    Arg::new("PERIOD")
                        .long("period")
                        .short('P')
                        .takes_value(true),
                )
                .arg(
                    Arg::new("KEYMAP")
                        .long("write")
                        .short('w')
                        .takes_value(true)
                        .multiple_occurrences(true),
                )
                .arg(
                    Arg::new("TIMEOUT")
                        .help("Set IR timeout")
                        .long("timeout")
                        .short('t')
                        .takes_value(true),
                )
                .arg(Arg::new("CLEAR").long("clear").short('c'))
                .arg(
                    Arg::new("CFGFILE")
                        .long("auto-load")
                        .short('a')
                        .help("Auto-load keymaps, based on configuration file")
                        .default_value("/etc/rc_maps.cfg")
                        .conflicts_with_all(&["DELAY", "PERIOD", "KEYMAP", "CLEAR", "TIMEOUT"])
                        .requires("DEVICE")
                        .takes_value(true),
                ),
        )
        .subcommand(
            Command::new("transmit")
                .about("Transmit IR")
                .arg_required_else_help(true)
                .arg(
                    Arg::new("LIRCDEV")
                        .help("Select device to use by lirc chardev (e.g. /dev/lirc1)")
                        .long("device")
                        .short('d')
                        .global(true)
                        .takes_value(true)
                        .conflicts_with("RCDEV"),
                )
                .arg(
                    Arg::new("RCDEV")
                        .help("Select device to use by rc core device (e.g. rc0)")
                        .long("rcdev")
                        .short('s')
                        .global(true)
                        .takes_value(true)
                        .conflicts_with("LIRCDEV"),
                )
                .arg(
                    Arg::new("TRANSMITTERS")
                        .help("Comma separated list of transmitters to use, starting from 1")
                        .long("transmitters")
                        .short('e')
                        .global(true)
                        .takes_value(true)
                        .multiple_occurrences(true)
                        .require_value_delimiter(true)
                        .use_value_delimiter(true),
                )
                .subcommand(
                    Command::new("irp")
                        .about("Encode using IRP language and transmit")
                        .arg(Arg::new("PRONTO").long("pronto").hide(true))
                        .arg(
                            Arg::new("CARRIER")
                                .long("carrier")
                                .short('c')
                                .help("Set carrier in Hz, 0 for unmodulated")
                                .takes_value(true)
                                .hide(true),
                        )
                        .arg(
                            Arg::new("DUTY_CYCLE")
                                .long("duty-cycle")
                                .short('u')
                                .help("Set send duty cycle % (1 to 99)")
                                .takes_value(true)
                                .hide(true),
                        )
                        .arg(
                            Arg::new("REPEATS")
                                .help("Number of IRP repeats to encode")
                                .long("repeats")
                                .short('r')
                                .takes_value(true)
                                .default_value("1"),
                        )
                        .arg(
                            Arg::new("FIELD")
                                .help("Set input variable like KEY=VALUE")
                                .long("field")
                                .short('f')
                                .takes_value(true)
                                .multiple_occurrences(true),
                        )
                        .arg(Arg::new("IRP").help("IRP protocol").required(true)),
                )
                .subcommand(
                    Command::new("pronto")
                        .about("Parse pronto hex code and transmit")
                        .arg(
                            Arg::new("REPEATS")
                                .long("repeats")
                                .short('r')
                                .help("Number of times to repeat signal")
                                .takes_value(true)
                                .default_value("1"),
                        )
                        .arg(Arg::new("PRONTO").help("Pronto Hex code").required(true)),
                )
                .subcommand(
                    Command::new("rawir")
                        .about("Parse raw IR and transmit")
                        .arg(
                            Arg::new("FILE")
                                .long("file")
                                .short('f')
                                .help("Read from rawir or mode2 file")
                                .takes_value(true)
                                .allow_invalid_utf8(true)
                                .multiple_occurrences(true),
                        )
                        .arg(
                            Arg::new("GAP")
                                .long("gap")
                                .short('g')
                                .help("Set gap after each file")
                                .takes_value(true)
                                .multiple_occurrences(true),
                        )
                        .arg(
                            Arg::new("RAWIR")
                                .help("Raw IR text")
                                .multiple_occurrences(true),
                        )
                        .arg(
                            Arg::new("CARRIER")
                                .long("carrier")
                                .short('c')
                                .help("Set carrier in Hz, 0 for unmodulated")
                                .takes_value(true),
                        )
                        .arg(
                            Arg::new("DUTY_CYCLE")
                                .long("duty-cycle")
                                .short('u')
                                .help("Set send duty cycle % (1 to 99)")
                                .takes_value(true),
                        ),
                )
                .subcommand(
                    Command::new("lircd")
                        .about("Transmit codes from lircd.conf file")
                        .arg(
                            Arg::new("CARRIER")
                                .long("carrier")
                                .short('c')
                                .help("Override carrier in Hz, 0 for unmodulated")
                                .takes_value(true),
                        )
                        .arg(
                            Arg::new("DUTY_CYCLE")
                                .long("duty-cycle")
                                .short('u')
                                .help("Override duty cycle % (1 to 99)")
                                .takes_value(true),
                        )
                        .arg(
                            Arg::new("CONF")
                                .help("lircd.conf file")
                                .allow_invalid_utf8(true)
                                .required(true),
                        )
                        .arg(
                            Arg::new("REMOTE")
                                .long("remote")
                                .short('m')
                                .help("Use codes from specific remote")
                                .takes_value(true),
                        )
                        .arg(
                            Arg::new("REPEATS")
                                .long("repeats")
                                .short('r')
                                .help("Number of times to repeat signal")
                                .takes_value(true)
                                .default_value("0"),
                        )
                        .arg(
                            Arg::new("CODES")
                                .help("Code to send, leave empty to list codes")
                                .multiple_occurrences(true)
                                .takes_value(true),
                        ),
                ),
        )
        .subcommand(
            Command::new("list")
                .about("List IR and CEC devices")
                .arg(
                    Arg::new("LIRCDEV")
                        .long("device")
                        .short('d')
                        .help("Select device to use by lirc chardev (e.g. /dev/lirc1)")
                        .takes_value(true)
                        .conflicts_with("RCDEV"),
                )
                .arg(
                    Arg::new("RCDEV")
                        .help("Select device to use by rc core device (e.g. rc0)")
                        .long("rcdev")
                        .short('s')
                        .takes_value(true)
                        .conflicts_with("LIRCDEV"),
                )
                .arg(Arg::new("READ").long("read-scancodes").short('l')),
        )
        .subcommand(
            Command::new("receive")
                .about("Receive IR and print to stdout")
                .arg(
                    Arg::new("LIRCDEV")
                        .long("device")
                        .short('d')
                        .help("Select device to use by lirc chardev (e.g. /dev/lirc1)")
                        .takes_value(true)
                        .conflicts_with("RCDEV"),
                )
                .arg(
                    Arg::new("RCDEV")
                        .help("Select device to use by rc core device (e.g. rc0)")
                        .long("rcdev")
                        .short('s')
                        .takes_value(true)
                        .conflicts_with("LIRCDEV"),
                )
                .arg(
                    Arg::new("LEARNING")
                        .help("Use short-range learning mode")
                        .long("learning-mode")
                        .short('l'),
                )
                .arg(
                    Arg::new("TIMEOUT")
                        .help("Set IR timeout")
                        .long("timeout")
                        .short('t')
                        .takes_value(true),
                )
                .arg(
                    Arg::new("ONESHOT")
                        .help("Stop receiving after first timeout message")
                        .long("one-shot")
                        .short('1'),
                ),
        )
        .get_matches();

    let mut log = Log::new();

    log.verbose(matches.occurrences_of("verbosity"));

    if matches.is_present("quiet") {
        log.quiet();
    }

    match matches.subcommand() {
        Some(("decode", matches)) => commands::decode::decode(matches, &log),
        Some(("encode", matches)) => commands::encode::encode(matches, &log),
        Some(("transmit", matches)) => commands::transmit::transmit(matches, &log),
        Some(("list", matches)) => match rcdev::enumerate_rc_dev() {
            Ok(list) => print_rc_dev(&list, matches),
            Err(err) => {
                eprintln!("error: {}", err);
                std::process::exit(1);
            }
        },
        Some(("receive", matches)) => commands::receive::receive(matches),
        Some(("config", matches)) => commands::config::config(matches),
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
