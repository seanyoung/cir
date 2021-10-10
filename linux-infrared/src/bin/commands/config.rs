use evdev::{Device, Key};
use linux_infrared::keymap;
use std::convert::TryFrom;
use std::fs;
use std::str::FromStr;

use super::{find_devices, Purpose};

pub fn config(matches: &clap::ArgMatches) {
    let (_, inputdev) = find_devices(matches, Purpose::Receive);

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
