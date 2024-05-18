use cir::{
    keymap::{Keymap, LinuxProtocol},
    lirc::Lirc,
    lircd_conf,
    rc_maps::parse_rc_maps_file,
    rcdev::Rcdev,
};
use evdev::KeyCode;
use irp::{Irp, Options};
use log::debug;
use std::{
    path::{Path, PathBuf},
    str::FromStr,
};

pub fn keymap(args: &crate::Keymap) {
    let mut rcdev = find_devices(&args.device, Purpose::Receive);

    if args.delay.is_some() || args.period.is_some() {
        let inputdev = match rcdev.open_input() {
            Ok(dev) => dev,
            Err(e) => {
                eprintln!("error: input: {e}");
                std::process::exit(1);
            }
        };

        let mut repeat = inputdev
            .get_auto_repeat()
            .expect("auto repeat is supported");

        if let Some(delay) = args.delay {
            repeat.delay = delay;
        }

        if let Some(period) = args.period {
            repeat.period = period;
        }

        if let Err(e) = inputdev.update_auto_repeat(&repeat) {
            eprintln!("error: failed to update autorepeat: {e}");
            std::process::exit(1);
        }
    }

    if args.clear {
        if let Err(e) = rcdev.clear_scancodes() {
            eprintln!("error: input: {e}");
            std::process::exit(1);
        }

        if let Some(lircdev) = &rcdev.lircdev {
            let lirc = match Lirc::open(PathBuf::from(lircdev)) {
                Ok(fd) => fd,
                Err(e) => {
                    eprintln!("error: {lircdev}: {e}");
                    std::process::exit(1);
                }
            };

            if let Err(e) = lirc.clear_bpf() {
                eprintln!("error: {lircdev}: {e}");
                std::process::exit(1);
            }
        }
    }

    if let Some(timeout) = args.timeout {
        if let Some(lircdev) = &rcdev.lircdev {
            let mut lirc = match Lirc::open(PathBuf::from(lircdev)) {
                Ok(fd) => fd,
                Err(e) => {
                    eprintln!("error: {lircdev}: {e}");
                    std::process::exit(1);
                }
            };

            if let Err(e) = lirc.set_timeout(timeout) {
                eprintln!("error: {lircdev}: {e}");
                std::process::exit(1);
            }
        } else {
            eprintln!("error: {}: no lirc device", rcdev.name);
            std::process::exit(1);
        }
    }

    if let Some(cfgfile) = &args.auto_load {
        match parse_rc_maps_file(cfgfile) {
            Ok(keymaps) => {
                let keymaps: Vec<_> = keymaps
                    .iter()
                    .filter_map(|map| {
                        if map.matches(&rcdev) {
                            Some(PathBuf::from(&map.file))
                        } else {
                            None
                        }
                    })
                    .collect();

                if keymaps.is_empty() {
                    eprintln!(
                        "{}: error: no match for driver ‘{}’ and default keymap ‘{}’",
                        cfgfile.display(),
                        rcdev.driver,
                        rcdev.default_keymap
                    );
                    std::process::exit(2);
                } else {
                    load_keymaps(true, &mut rcdev, None, None, &keymaps);
                }
            }
            Err(e) => {
                eprintln!("error: {}: {e}", cfgfile.display());
                std::process::exit(1);
            }
        }
    }

    if !args.scankey.is_empty() {
        for (scancode, keycode) in &args.scankey {
            let key = match KeyCode::from_str(keycode) {
                Ok(key) => key,
                Err(_) => {
                    eprintln!("error: ‘{keycode}’ is not a valid keycode");
                    continue;
                }
            };

            match rcdev.update_scancode(key, *scancode) {
                Ok(_) => (),
                Err(e) => {
                    eprintln!(
                            "error: failed to update key mapping from scancode {scancode:x?} to {key:?}: {e}"
                        );
                    std::process::exit(1);
                }
            }
        }
    }

    if !args.protocol.is_empty() {
        let mut res = Vec::new();

        for name in &args.protocol {
            if name.is_empty() {
                // nothing to do
            } else if name == "all" {
                for pos in 0..rcdev.supported_protocols.len() {
                    if !res.contains(&pos) {
                        res.push(pos);
                    }
                }
            } else if let Some(pos) = rcdev.supported_protocols.iter().position(|e| e == name) {
                if !res.contains(&pos) {
                    res.push(pos);
                }
            } else {
                eprintln!("error: {}: does not support protocol {name}", rcdev.name);
                std::process::exit(1);
            }
        }

        if let Err(e) = rcdev.set_enabled_protocols(&res) {
            eprintln!("error: {}: {e}", rcdev.name);
            std::process::exit(1);
        }
    }

    if let Some(irp_notation) = &args.irp {
        let irp = match Irp::parse(irp_notation) {
            Ok(irp) => irp,
            Err(e) => {
                eprintln!("error: {irp_notation}: {e}");
                std::process::exit(1);
            }
        };

        let mut max_gap = 100000;

        let chdev = if let Some(lircdev) = &rcdev.lircdev {
            let lirc = match Lirc::open(PathBuf::from(lircdev)) {
                Ok(fd) => fd,
                Err(e) => {
                    eprintln!("error: {lircdev}: {e}");
                    std::process::exit(1);
                }
            };

            if !lirc.can_receive_raw() {
                eprintln!("error: {}: not a raw receiver, irp not supported", lircdev);
                std::process::exit(1);
            }

            match lirc.query_bpf() {
                Ok(Some(_)) => (),
                Ok(None) => {
                    eprintln!("error: {}: no kernel BPF support, rebuild kernel with CONFIG_BPF_LIRC_MODE2", lircdev);
                    std::process::exit(1);
                }
                Err(e) => {
                    eprintln!("error: {}: {e}", lircdev);
                    std::process::exit(1);
                }
            }

            lirc
        } else {
            eprintln!("error: {}: no lirc device, irp not supported", rcdev.name);
            std::process::exit(1);
        };

        if let Some(timeout) = args.timeout {
            max_gap = timeout;
        } else if let Ok(timeout) = chdev.get_timeout() {
            let dev_max_gap = (timeout * 9) / 10;

            log::trace!(
                "device reports timeout of {}, using 90% of that as {} max_gap",
                timeout,
                dev_max_gap
            );

            max_gap = dev_max_gap;
        }

        let mut options = Options {
            name: "irp",
            max_gap,
            ..Default::default()
        };

        options.nfa = args.options.save_nfa;
        options.dfa = args.options.save_dfa;
        options.aeps = args.options.aeps.unwrap_or(100);
        options.eps = args.options.eps.unwrap_or(3);

        options.llvm_ir = args.bpf_options.save_llvm_ir;
        options.assembly = args.bpf_options.save_assembly;
        options.object = args.bpf_options.save_object;

        let dfa = match irp.compile(&options) {
            Ok(dfa) => dfa,
            Err(e) => {
                println!("error: irp: {e}");
                std::process::exit(1);
            }
        };

        let bpf = match dfa.compile_bpf(&options) {
            Ok((bpf, _)) => bpf,
            Err(e) => {
                eprintln!("error: irp: {e}");
                std::process::exit(1);
            }
        };

        if let Err(e) = chdev.attach_bpf(&bpf) {
            eprintln!("error: attach bpf: {e}",);
            std::process::exit(1);
        }
    }

    if !args.write.is_empty() {
        load_keymaps(
            true,
            &mut rcdev,
            Some(&args.options),
            Some(&args.bpf_options),
            &args.write,
        )
    }
}

fn load_keymaps(
    clear: bool,
    rcdev: &mut Rcdev,
    decode_options: Option<&crate::DecodeOptions>,
    bpf_decode_options: Option<&crate::BpfDecodeOptions>,
    keymaps: &[PathBuf],
) {
    let mut protocols = Vec::new();

    let chdev = if clear || !keymaps.is_empty() {
        if let Err(e) = rcdev.clear_scancodes() {
            eprintln!("error: {e}");
            std::process::exit(1);
        }

        if let Some(lircdev) = &rcdev.lircdev {
            let lirc = match Lirc::open(PathBuf::from(lircdev)) {
                Ok(fd) => fd,
                Err(e) => {
                    eprintln!("error: {lircdev}: {e}");
                    std::process::exit(1);
                }
            };

            if let Err(e) = lirc.clear_bpf() {
                eprintln!("error: {lircdev}: {e}");
                std::process::exit(1);
            }

            Some(lirc)
        } else {
            None
        }
    } else {
        None
    };

    for keymap_filename in keymaps.iter() {
        if keymap_filename.to_string_lossy().ends_with(".lircd.conf") {
            load_lircd(
                rcdev,
                &chdev,
                decode_options,
                bpf_decode_options,
                keymap_filename,
            );
        } else {
            load_keymap(
                rcdev,
                &chdev,
                decode_options,
                bpf_decode_options,
                keymap_filename,
                &mut protocols,
            );
        }
    }

    if let Err(e) = rcdev.set_enabled_protocols(&protocols) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

fn load_keymap(
    rcdev: &mut Rcdev,
    chdev: &Option<Lirc>,
    decode_options: Option<&crate::DecodeOptions>,
    bpf_decode_options: Option<&crate::BpfDecodeOptions>,
    keymap_filename: &Path,
    protocols: &mut Vec<usize>,
) {
    let keymaps = match Keymap::parse_file(keymap_filename) {
        Ok(map) => map,
        Err(e) => {
            eprintln!("error: {}: {e}", keymap_filename.display());
            std::process::exit(1);
        }
    };

    for keymap in keymaps {
        for (scancode, keycode) in &keymap.scancodes {
            // TODO: needs some logic to check for KEY_{} etc like load_lircd
            let key = match KeyCode::from_str(keycode) {
                Ok(key) => key,
                Err(_) => {
                    eprintln!("error: ‘{keycode}’ is not a valid keycode");
                    continue;
                }
            };

            match rcdev.update_scancode(key, *scancode) {
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
            if let Some(p) = LinuxProtocol::find_decoder(&keymap.protocol) {
                for p in p {
                    if let Some(index) = rcdev
                        .supported_protocols
                        .iter()
                        .position(|e| e == p.decoder)
                    {
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
            name: &keymap.name,
            max_gap,
            ..Default::default()
        };

        if let Some(decode) = &decode_options {
            options.nfa = decode.save_nfa;
            options.dfa = decode.save_dfa;
            options.aeps = decode.aeps.unwrap_or(100);
            options.eps = decode.eps.unwrap_or(3);
        }

        if let Some(decode) = &bpf_decode_options {
            options.llvm_ir = decode.save_llvm_ir;
            options.assembly = decode.save_assembly;
            options.object = decode.save_object;
        }

        let dfas = match keymap.build_dfa(&options) {
            Ok(dfas) => dfas,
            Err(e) => {
                println!("{}: {e}", keymap_filename.display());
                std::process::exit(1);
            }
        };

        for (dfa, _) in dfas {
            let bpf = match dfa.compile_bpf(&options) {
                Ok((bpf, _)) => bpf,
                Err(e) => {
                    eprintln!("error: {}: {e}", keymap_filename.display());
                    std::process::exit(1);
                }
            };

            if !chdev.can_receive_raw() {
                eprintln!("error: {}: not a raw receiver, irp not supported", chdev);
                std::process::exit(1);
            }

            match chdev.query_bpf() {
                Ok(Some(_)) => (),
                Ok(None) => {
                    eprintln!(
                    "error: {}: no kernel BPF support, rebuild kernel with CONFIG_BPF_LIRC_MODE2",
                    chdev
                );
                    std::process::exit(1);
                }
                Err(e) => {
                    eprintln!("error: {}: {e}", chdev);
                    std::process::exit(1);
                }
            }

            log::debug!(
                "attaching bpf program for {} to {}",
                keymap_filename.display(),
                chdev
            );

            if let Err(e) = chdev.attach_bpf(&bpf) {
                eprintln!("error: {}: attach bpf: {e}", keymap_filename.display());
                std::process::exit(1);
            }
        }
    }
}

fn load_lircd(
    rcdev: &mut Rcdev,
    chdev: &Option<Lirc>,
    decode_options: Option<&crate::DecodeOptions>,
    bpf_decode_options: Option<&crate::BpfDecodeOptions>,
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

        let mut options = remote.default_options(
            decode_options.and_then(|decode| decode.aeps),
            decode_options.and_then(|decode| decode.eps),
            max_gap,
        );

        options.repeat_mask = remote.repeat_mask;

        if let Some(decode) = &decode_options {
            options.nfa = decode.save_nfa;
            options.dfa = decode.save_dfa;
        }

        if let Some(decode) = &bpf_decode_options {
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

        if !chdev.can_receive_raw() {
            eprintln!("error: {}: not a raw receiver, irp not supported", chdev);
            std::process::exit(1);
        }

        match chdev.query_bpf() {
            Ok(Some(_)) => (),
            Ok(None) => {
                eprintln!(
                    "error: {}: no kernel BPF support, rebuild kernel with CONFIG_BPF_LIRC_MODE2",
                    chdev
                );
                std::process::exit(1);
            }
            Err(e) => {
                eprintln!("error: {}: {e}", chdev);
                std::process::exit(1);
            }
        }

        if let Err(e) = chdev.attach_bpf(&bpf) {
            eprintln!("error: {}: attach bpf: {e}", keymap_filename.display());
            std::process::exit(1);
        }

        log::debug!("attaching bpf program for {} to {}", remote.name, chdev);

        for code in remote.codes {
            let mut name = code.name.to_uppercase();
            if !name.starts_with("KEY_") {
                name.insert_str(0, "KEY_");
            };
            let key = match KeyCode::from_str(&name) {
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

            match rcdev.update_scancode(key, code.code[0]) {
                Ok(_) => (),
                Err(e) => {
                    eprintln!(
                        "error: failed to update key mapping from scancode {:x?} to {key:?}: {e}",
                        code.code[0]
                    );
                    std::process::exit(1);
                }
            }
        }

        // TODO: keycodes for raw codes
    }
}

pub enum Purpose {
    Receive,
    Transmit,
}

/// Enumerate all rc devices and find the lirc and input devices
pub fn find_devices(device: &crate::RcDevice, purpose: Purpose) -> Rcdev {
    let mut list = match Rcdev::enumerate_devices() {
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

            let lirc = match Lirc::open(&lircpath) {
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

    list.remove(entry)
}

pub fn open_lirc(device: &crate::RcDevice, purpose: Purpose) -> Lirc {
    let rcdev = find_devices(device, purpose);

    if let Some(lircdev) = rcdev.lircdev {
        debug!("opening {}", lircdev);

        let lircpath = PathBuf::from(lircdev);

        match Lirc::open(&lircpath) {
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
