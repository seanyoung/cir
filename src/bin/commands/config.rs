use cir::keymap;
use evdev::{Device, Key};
use std::{convert::TryFrom, fs, path::PathBuf, str::FromStr};

use super::{find_devices, Purpose};
use cir::rc_maps::parse_rc_maps_file;

pub fn config(matches: &clap::ArgMatches) {
    let rcdev = find_devices(matches, Purpose::Receive);

    if rcdev.inputdev.is_none() {
        eprintln!("error: input device is missing");
        std::process::exit(1);
    }

    let inputdev = rcdev.inputdev.as_ref().unwrap();

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
        clear_scancodes(&inputdev);
    }

    if matches.occurrences_of("CFGFILE") > 0 {
        let cfgfile = PathBuf::from(matches.value_of("CFGFILE").unwrap());

        match parse_rc_maps_file(&cfgfile) {
            Ok(keymaps) => {
                for map in keymaps {
                    if map.matches(&rcdev) {
                        clear_scancodes(&inputdev);
                        load_keymap(&inputdev, &map.file);
                        return;
                    }
                }

                eprintln!(
                    "{}: error: no match for driver ‘{}’ and default keymap ‘{}’",
                    cfgfile.display(),
                    rcdev.driver,
                    rcdev.default_keymap
                );
                std::process::exit(2);
            }
            Err(e) => {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
    }

    if let Some(keymaps) = matches.values_of("KEYMAP") {
        for keymap_filename in keymaps {
            load_keymap(&inputdev, keymap_filename);
        }
    }
}

fn clear_scancodes(inputdev: &Device) {
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

fn load_keymap(inputdev: &Device, keymap_filename: &str) {
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

                let scancode = match u64::from_str_radix(scancode.trim_start_matches("0x"), 16) {
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
                            scancode, key, e
                        );
                        std::process::exit(1);
                    }
                }
            }
        }
    }
}
