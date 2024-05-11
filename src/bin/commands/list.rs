use cir::{lirc::Lirc, rcdev};
use evdev::Device;
use itertools::Itertools;
use std::path::PathBuf;

pub fn list(args: &crate::List) {
    match rcdev::enumerate_rc_dev() {
        Ok(list) => {
            print_rc_dev(&list, args);
        }
        Err(err) => {
            eprintln!("error: {err}");
            std::process::exit(1);
        }
    }
}

fn print_rc_dev(list: &[rcdev::Rcdev], config: &crate::List) {
    let mut printed = 0;

    for rcdev in list {
        if let Some(needlelircdev) = &config.device.lirc_dev {
            if let Some(lircdev) = &rcdev.lircdev {
                if lircdev == needlelircdev {
                    // ok
                } else {
                    continue;
                }
            } else {
                continue;
            }
        } else if let Some(needlercdev) = &config.device.rc_dev {
            if needlercdev != &rcdev.name {
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
            println!("\tInput Device\t\t: {inputdev}");

            match Device::open(inputdev) {
                Ok(inputdev) => {
                    let id = inputdev.input_id();

                    println!("\tBus\t\t\t: {}", id.bus_type());

                    println!(
                        "\tVendor/product\t\t: {:04x}:{:04x} version 0x{:04x}",
                        id.vendor(),
                        id.product(),
                        id.version()
                    );

                    if let Some(repeat) = inputdev.get_auto_repeat() {
                        println!(
                            "\tRepeat\t\t\t: delay {} ms, period {} ms",
                            repeat.delay, repeat.period
                        );
                    }

                    if config.mapping {
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
                                                "\tScancode\t\t: 0x{scancode:08x} => {keycode:?}"
                                            );
                                        }
                                        4 => {
                                            // kernel v5.6 and earlier give 32 bit scancodes
                                            let scancode =
                                                u32::from_ne_bytes(scancode.try_into().unwrap());
                                            let keycode = evdev::Key::new(keycode as u16);

                                            println!(
                                                "\tScancode\t\t: 0x{scancode:08x} => {keycode:?}"
                                            )
                                        }
                                        len => panic!(
                                            "scancode should be 4 or 8 bytes long, not {len}"
                                        ),
                                    }

                                    index += 1;
                                }
                                Err(err) if err.kind() == std::io::ErrorKind::InvalidInput => break,
                                Err(err) => {
                                    eprintln!("error: {err}");
                                    std::process::exit(1);
                                }
                            }
                        }
                    }
                }
                Err(err) => {
                    println!("\tInput properties\t: {err}");
                }
            };
        }
        if let Some(lircdev) = &rcdev.lircdev {
            println!("\tLIRC Device\t\t: {lircdev}");

            match Lirc::open(PathBuf::from(lircdev)) {
                Ok(mut lircdev) => {
                    if lircdev.can_receive_raw() {
                        println!("\tLIRC Receiver\t\t: raw receiver");

                        if lircdev.can_get_rec_resolution() {
                            println!(
                                "\tLIRC Resolution\t\t: {}",
                                match lircdev.receiver_resolution() {
                                    Ok(res) => format!("{res} microseconds"),
                                    Err(err) => err.to_string(),
                                }
                            );
                        } else {
                            println!("\tLIRC Resolution\t\t: unknown");
                        }

                        println!(
                            "\tLIRC Timeout\t\t: {}",
                            match lircdev.get_timeout() {
                                Ok(timeout) => format!("{timeout} microseconds"),
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
                                    Ok(count) => format!("{count}"),
                                    Err(err) => err.to_string(),
                                }
                            );
                        } else {
                            println!("\tLIRC Transmitters\t: unknown");
                        }
                    } else {
                        println!("\tLIRC Transmitter\t: no");
                    }

                    if lircdev.can_receive_raw() {
                        match lircdev.query_bpf() {
                            Ok(Some(links)) => {
                                println!("\tBPF protocols\t\t: {}", links.iter().join(" "));
                            }
                            Ok(None) => {
                                println!("\tBPF protocols\t\t: No kernel support")
                            }
                            Err(err) => {
                                println!("\tBPF protocols\t\t: {err}")
                            }
                        }
                    }
                }
                Err(err) => {
                    println!("\tLIRC Features\t\t: {err}");
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
        if let Some(lircdev) = &config.device.lirc_dev {
            eprintln!("error: no lirc device named {lircdev}");
        } else if let Some(rcdev) = &config.device.rc_dev {
            eprintln!("error: no rc device named {rcdev}");
        } else {
            eprintln!("error: no devices found");
        }
        std::process::exit(1);
    }
}
