use aya::programs::{Link, LircMode2};
use cir::{
    keymap::{Keymap, LinuxProtocol},
    lirc, lircd_conf,
    rc_maps::parse_rc_maps_file,
    rcdev::{self, enumerate_rc_dev, Rcdev},
};
use evdev::{Device, Key};
use irp::Options;
use itertools::Itertools;
use log::debug;
use std::{
    convert::TryFrom,
    os::fd::AsFd,
    path::{Path, PathBuf},
    str::FromStr,
};

pub fn config(config: &crate::Config) {
    if !config.clear
        && config.keymaps.is_empty()
        && config.delay.is_none()
        && config.period.is_none()
    {
        match rcdev::enumerate_rc_dev() {
            Ok(list) => {
                print_rc_dev(&list, config);
                return;
            }
            Err(err) => {
                eprintln!("error: {err}");
                std::process::exit(1);
            }
        }
    }

    let mut rcdev = find_devices(&config.device, Purpose::Receive);

    if rcdev.inputdev.is_none() {
        eprintln!("error: input device is missing");
        std::process::exit(1);
    }

    if config.delay.is_some() || config.period.is_some() {
        let inputdev = rcdev.inputdev.as_ref().unwrap();

        let mut inputdev = match Device::open(inputdev) {
            Ok(l) => l,
            Err(s) => {
                eprintln!("error: {inputdev}: {s}");
                std::process::exit(1);
            }
        };

        let mut repeat = inputdev
            .get_auto_repeat()
            .expect("auto repeat is supported");

        if let Some(delay) = config.delay {
            repeat.delay = delay;
        }

        if let Some(period) = config.period {
            repeat.period = period;
        }

        if let Err(e) = inputdev.update_auto_repeat(&repeat) {
            eprintln!("error: failed to update autorepeat: {e}");
            std::process::exit(1);
        }
    }

    if !config.keymaps.is_empty() {
        load_keymaps(config.clear, &mut rcdev, Some(config), &config.keymaps);
    }
}

fn load_keymaps(
    clear: bool,
    rcdev: &mut Rcdev,
    config: Option<&crate::Config>,
    keymaps: &[PathBuf],
) {
    let mut protocols = Vec::new();
    let inputdev = rcdev.inputdev.as_ref().unwrap();

    let inputdev = match Device::open(inputdev) {
        Ok(l) => l,
        Err(s) => {
            eprintln!("error: {inputdev}: {s}");
            std::process::exit(1);
        }
    };

    let chdev = if clear || !keymaps.is_empty() {
        clear_scancodes(&inputdev);
        if let Some(lircdev) = &rcdev.lircdev {
            clear_bpf_programs(lircdev)
        } else {
            None
        }
    } else {
        None
    };

    if !keymaps.is_empty() {
        for keymap_filename in keymaps.iter() {
            if keymap_filename.to_string_lossy().ends_with(".lircd.conf") {
                load_lircd(&inputdev, &chdev, config, keymap_filename);
            } else {
                load_keymap(
                    &inputdev,
                    &chdev,
                    config,
                    keymap_filename,
                    &mut protocols,
                    &rcdev.supported_protocols,
                );
            }
        }
    }

    if let Err(e) = rcdev.set_enabled_protocols(&protocols) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

pub fn auto(auto: &crate::Auto) {
    let mut rcdev = find_devices(&auto.device, Purpose::Receive);

    if rcdev.inputdev.is_none() {
        eprintln!("error: input device is missing");
        std::process::exit(1);
    }

    match parse_rc_maps_file(&auto.cfgfile) {
        Ok(keymaps) => {
            for map in keymaps {
                if map.matches(&rcdev) {
                    load_keymaps(true, &mut rcdev, None, &[PathBuf::from(map.file)]);
                    return;
                }
            }

            eprintln!(
                "{}: error: no match for driver ‘{}’ and default keymap ‘{}’",
                auto.cfgfile.display(),
                rcdev.driver,
                rcdev.default_keymap
            );
            std::process::exit(2);
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}

fn clear_bpf_programs(lircdev: &str) -> Option<lirc::Lirc> {
    match lirc::open(PathBuf::from(lircdev)) {
        Ok(fd) => match LircMode2::query(fd.as_fd()) {
            Ok(links) => {
                for link in links {
                    if let Err(e) = link.detach() {
                        eprintln!("error: {lircdev}: unable detach: {e}");
                    }
                }
                Some(fd)
            }
            Err(e) => {
                eprintln!("error: {lircdev}: to query for bpf programs: {e}");
                None
            }
        },
        Err(e) => {
            eprintln!("error: {lircdev}: {e}");
            None
        }
    }
}

fn clear_scancodes(inputdev: &Device) {
    loop {
        match inputdev.update_scancode_by_index(0, Key::KEY_RESERVED, &[]) {
            Ok(_) => (),
            Err(e) if e.kind() == std::io::ErrorKind::InvalidInput => break,
            Err(e) => {
                eprintln!("error: unable to remove scancode entry: {e}");
                std::process::exit(1);
            }
        }
    }
}

fn load_keymap(
    inputdev: &Device,
    chdev: &Option<lirc::Lirc>,
    config: Option<&crate::Config>,
    keymap_filename: &Path,
    protocols: &mut Vec<usize>,
    supported_protocols: &[String],
) {
    let map = match Keymap::parse(keymap_filename) {
        Ok(map) => map,
        Err(e) => {
            eprintln!("error: {}: {e}", keymap_filename.display());
            std::process::exit(1);
        }
    };

    for p in map {
        for (scancode, keycode) in &p.scancodes {
            // TODO: needs some logic to check for KEY_{} etc like load_lircd
            let key = match Key::from_str(keycode) {
                Ok(key) => key,
                Err(_) => {
                    eprintln!("error: ‘{keycode}’ is not a valid keycode");
                    continue;
                }
            };

            // Kernels from before v5.7 want the scancode in 4 bytes; try this if possible
            let scancode = if let Ok(scancode) = u32::try_from(*scancode) {
                scancode.to_ne_bytes().to_vec()
            } else {
                scancode.to_ne_bytes().to_vec()
            };

            match inputdev.update_scancode(key, &scancode) {
                Ok(_) => (),
                Err(e) => {
                    eprintln!(
                            "error: failed to update key mapping from scancode {scancode:x?} to {key:?}: {e}"
                        );
                    std::process::exit(1);
                }
            }
        }

        let Some(chdev) = chdev else {
            if let Some(p) = LinuxProtocol::find_decoder(&p.protocol) {
                for p in p {
                    if let Some(index) = supported_protocols.iter().position(|e| e == p.decoder) {
                        if !protocols.contains(&index) {
                            protocols.push(index);
                        }
                    } else {
                        eprintln!("error: no lirc device found for BPF decoding");
                        std::process::exit(1);
                    }
                }
                continue;
            } else {
                eprintln!("error: no lirc device found for BPF decoding");
                std::process::exit(1);
            }
        };

        let mut max_gap = 100000;

        if let Ok(timeout) = chdev.get_timeout() {
            let dev_max_gap = (timeout * 9) / 10;

            log::trace!(
                "device reports timeout of {}, using 90% of that as {} max_gap",
                timeout,
                dev_max_gap
            );

            max_gap = dev_max_gap;
        }

        let mut options = Options {
            name: &p.name,
            max_gap,
            ..Default::default()
        };

        if let Some(decode) = &config {
            options.nfa = decode.options.save_nfa;
            options.dfa = decode.options.save_dfa;
            options.llvm_ir = decode.save_llvm_ir;
            options.assembly = decode.save_assembly;
            options.object = decode.save_object;
        }

        let dfas = match p.build_dfa(&options) {
            Ok(dfas) => dfas,
            Err(e) => {
                println!("{}: {e}", keymap_filename.display());
                std::process::exit(1);
            }
        };

        for dfa in dfas {
            let bpf = match dfa.compile_bpf(&options) {
                Ok((bpf, _)) => bpf,
                Err(e) => {
                    eprintln!("error: {}: {e}", keymap_filename.display());
                    std::process::exit(1);
                }
            };

            let mut bpf = match aya::Bpf::load(&bpf) {
                Ok(bpf) => bpf,
                Err(e) => {
                    eprintln!("error: {}: {e}", keymap_filename.display());
                    std::process::exit(1);
                }
            };

            let program: &mut LircMode2 = bpf
                .program_mut(&p.name)
                .expect("function missing")
                .try_into()
                .unwrap();

            program.load().unwrap();

            let link = program.attach(chdev.as_fd()).expect("attach");

            program.take_link(link).unwrap();
        }
    }
}

fn load_lircd(
    inputdev: &Device,
    chdev: &Option<lirc::Lirc>,
    config: Option<&crate::Config>,
    keymap_filename: &Path,
) {
    let remotes = match lircd_conf::parse(keymap_filename) {
        Ok(r) => r,
        Err(_) => std::process::exit(2),
    };

    for remote in remotes {
        log::info!("Configuring remote {}", remote.name);

        let Some(chdev) = chdev else {
            eprintln!("error: no lirc device found");
            std::process::exit(1);
        };

        let mut max_gap = 100000;

        if let Ok(timeout) = chdev.get_timeout() {
            let dev_max_gap = (timeout * 9) / 10;

            log::trace!(
                "device reports timeout of {}, using 90% of that as {} max_gap",
                timeout,
                dev_max_gap
            );

            max_gap = dev_max_gap;
        }

        let mut options = remote.default_options(None, None, max_gap);

        options.repeat_mask = remote.repeat_mask;
        if let Some(decode) = &config {
            options.nfa = decode.options.save_nfa;
            options.dfa = decode.options.save_dfa;
            options.llvm_ir = decode.save_llvm_ir;
            options.assembly = decode.save_assembly;
            options.object = decode.save_object;
        }

        let dfa = remote.build_dfa(&options);

        let bpf = match dfa.compile_bpf(&options) {
            Ok((bpf, _)) => bpf,
            Err(e) => {
                eprintln!("error: {}: {e}", keymap_filename.display());
                std::process::exit(1);
            }
        };

        let mut bpf = match aya::Bpf::load(&bpf) {
            Ok(bpf) => bpf,
            Err(e) => {
                eprintln!("error: {}: {e}", keymap_filename.display());
                std::process::exit(1);
            }
        };

        let program: &mut LircMode2 = bpf
            .program_mut(&remote.name)
            .expect("function missing")
            .try_into()
            .unwrap();

        program.load().unwrap();

        let link = program.attach(chdev.as_fd()).expect("attach");

        program.take_link(link).unwrap();

        for code in remote.codes {
            let mut name = code.name.to_uppercase();
            if !name.starts_with("KEY_") {
                name.insert_str(0, "KEY_");
            };
            let key = match Key::from_str(&name) {
                Ok(key) => key,
                Err(_) => {
                    eprintln!(
                        "error: {}:{}: ‘{}’ is not a valid keycode for remote ‘{}’",
                        keymap_filename.display(),
                        code.line_no,
                        code.name,
                        remote.name,
                    );
                    continue;
                }
            };

            // Kernels from before v5.7 want the scancode in 4 bytes; try this if possible
            let scancode = if let Ok(scancode) = u32::try_from(code.code[0]) {
                scancode.to_ne_bytes().to_vec()
            } else {
                code.code[0].to_ne_bytes().to_vec()
            };

            match inputdev.update_scancode(key, &scancode) {
                Ok(_) => (),
                Err(e) => {
                    eprintln!(
                        "error: failed to update key mapping from scancode {scancode:x?} to {key:?}: {e}"
                    );
                    std::process::exit(1);
                }
            }
        }

        // TODO: keycodes for raw codes
    }
}

fn print_rc_dev(list: &[rcdev::Rcdev], config: &crate::Config) {
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

            match lirc::open(PathBuf::from(lircdev)) {
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
                        match LircMode2::query(lircdev.as_file()) {
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
                                            Some(name) => print!("{name}"),
                                            None => print!("{}", info.id()),
                                        },
                                        Err(err) => {
                                            print!("{err}")
                                        }
                                    }
                                }

                                println!();
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

pub enum Purpose {
    Receive,
    Transmit,
}

/// Enumerate all rc devices and find the lirc and input devices
pub fn find_devices(device: &crate::RcDevice, purpose: Purpose) -> Rcdev {
    let list = match enumerate_rc_dev() {
        Ok(list) if list.is_empty() => {
            eprintln!("error: no devices found");
            std::process::exit(1);
        }
        Ok(list) => list,
        Err(err) => {
            eprintln!("error: no devices found: {err}");
            std::process::exit(1);
        }
    };

    let entry = if let Some(rcdev) = &device.rc_dev {
        if let Some(entry) = list.iter().position(|rc| &rc.name == rcdev) {
            entry
        } else {
            eprintln!("error: {rcdev} not found");
            std::process::exit(1);
        }
    } else if let Some(lircdev) = &device.lirc_dev {
        if let Some(entry) = list
            .iter()
            .position(|rc| rc.lircdev == Some(lircdev.to_string()))
        {
            entry
        } else {
            eprintln!("error: {lircdev} not found");
            std::process::exit(1);
        }
    } else if let Some(entry) = list.iter().position(|rc| {
        if rc.lircdev.is_none() {
            false
        } else {
            let lircpath = PathBuf::from(rc.lircdev.as_ref().unwrap());

            let lirc = match lirc::open(&lircpath) {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("error: {}: {}", lircpath.display(), e);
                    std::process::exit(1);
                }
            };

            match purpose {
                Purpose::Receive => lirc.can_receive_raw() || lirc.can_receive_scancodes(),
                Purpose::Transmit => lirc.can_send(),
            }
        }
    }) {
        entry
    } else {
        eprintln!("error: no lirc device found");
        std::process::exit(1);
    };

    list[entry].clone()
}

pub fn open_lirc(device: &crate::RcDevice, purpose: Purpose) -> lirc::Lirc {
    let rcdev = find_devices(device, purpose);

    if let Some(lircdev) = rcdev.lircdev {
        debug!("opening {}", lircdev);

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
