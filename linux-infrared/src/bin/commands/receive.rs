use evdev::{Device, InputEventKind};
use linux_infrared::lirc;
use mio::{unix::SourceFd, Events, Interest, Poll, Token};
use nix::fcntl::{FcntlArg, OFlag};
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;
use std::time::Duration;

use super::{find_devices, open_lirc, Purpose};

// Clippy comparison_chain doesn't make any sense. It make the code _worse_
#[allow(clippy::comparison_chain)]
pub fn receive(matches: &clap::ArgMatches) {
    let rcdev = find_devices(matches, Purpose::Receive);
    let raw_token: Token = Token(0);
    let scancodes_token: Token = Token(1);
    let input_token: Token = Token(2);

    let mut poll = Poll::new().expect("failed to create poll");
    let mut scandev = None;
    let mut rawdev = None;
    let mut eventdev = None;

    if let Some(lircdev) = rcdev.lircdev {
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
                        "error: {}: failed to enable measure carrier: {}",
                        lircdev, err
                    );
                    std::process::exit(1);
                }
                learning_mode = true;
            }

            if lircdev.can_use_wideband_receiver() {
                if let Err(err) = lircdev.set_wideband_receiver(true) {
                    eprintln!(
                        "error: {}: failed to enable wideband receiver: {}",
                        lircdev, err
                    );
                    std::process::exit(1);
                }
                learning_mode = true;
            }

            if !learning_mode {
                eprintln!(
                    "error: {}: lirc device does not support learning mode",
                    lircdev
                );
                std::process::exit(1);
            }
        } else {
            if lircdev.can_measure_carrier() {
                if let Err(err) = lircdev.set_measure_carrier(false) {
                    eprintln!(
                        "error: {}: failed to disable measure carrier: {}",
                        lircdev, err
                    );
                    std::process::exit(1);
                }
            }

            if lircdev.can_use_wideband_receiver() {
                if let Err(err) = lircdev.set_wideband_receiver(false) {
                    eprintln!(
                        "error: {}: failed to disable wideband receiver: {}",
                        lircdev, err
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
                                eprintln!("error: {}: {}", lircdev, err);
                                std::process::exit(1);
                            }
                        }
                        Ok(range) => {
                            eprintln!(
                                "error: {} not in the supported range {}-{}",
                                timeout, range.start, range.end
                            );
                            std::process::exit(1);
                        }
                        Err(err) => {
                            eprintln!("error: {}: {}", lircdev, err);
                            std::process::exit(1);
                        }
                    }
                } else {
                    eprintln!("error: {}: changing timeout not supported", lircdev);
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
                let mut lircdev = open_lirc(matches, Purpose::Receive);

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

    if let Some(inputdev) = rcdev.inputdev {
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
                if err.kind() != std::io::ErrorKind::WouldBlock {
                    eprintln!("error: {}", err);
                    std::process::exit(1);
                }
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
                if err.kind() != std::io::ErrorKind::WouldBlock {
                    eprintln!("error: {}", err);
                    std::process::exit(1);
                }
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
                    eprintln!("error: {}", e);
                    std::process::exit(1);
                }
            }
        }

        poll.poll(&mut events, None).expect("poll should not fail");
    }
}
