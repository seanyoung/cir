use cir::{
    lirc::{Lirc, LIRC_SCANCODE_FLAG_REPEAT, LIRC_SCANCODE_FLAG_TOGGLE},
    rcdev::Rcdev,
};
use evdev::Device;
use nix::{
    errno::Errno,
    fcntl::{fcntl, FcntlArg, OFlag},
    poll::{poll, PollFd, PollFlags, PollTimeout},
};
use std::os::{fd::AsFd, unix::io::AsRawFd};
use std::path::PathBuf;
use std::time::Duration;

use super::config::{find_devices, open_lirc, Purpose};

// Clippy comparison_chain doesn't make any sense. It make the code _worse_
#[allow(clippy::comparison_chain)]
pub fn test(test: &crate::Test) {
    let rcdev = find_devices(&test.device, Purpose::Receive);

    if test.raw {
        test_raw(rcdev, test);
    } else {
        test_all(rcdev, test);
    }
}

fn test_all(rcdev: Rcdev, test: &crate::Test) {
    let mut scandev = None;
    let mut rawdev = None;
    let mut eventdev = None;

    if let Some(lircdev) = rcdev.lircdev {
        let lircdev = open(lircdev, test);

        if lircdev.can_receive_raw() {
            if lircdev.can_receive_scancodes() {
                let mut lircdev = open_lirc(&test.device, Purpose::Receive);

                lircdev
                    .scancode_mode()
                    .expect("should be able to switch to scancode mode");

                fcntl(lircdev.as_raw_fd(), FcntlArg::F_SETFL(OFlag::O_NONBLOCK))
                    .expect("should be able to set non-blocking");

                scandev = Some(lircdev);
            }

            fcntl(lircdev.as_raw_fd(), FcntlArg::F_SETFL(OFlag::O_NONBLOCK))
                .expect("should be able to set non-blocking");

            rawdev = Some(lircdev);
        } else if lircdev.can_receive_scancodes() {
            fcntl(lircdev.as_raw_fd(), FcntlArg::F_SETFL(OFlag::O_NONBLOCK))
                .expect("should be able to set non-blocking");

            scandev = Some(lircdev);
        } else {
            eprintln!("error: {lircdev}: device does not support receiving");
            std::process::exit(1);
        }
    }

    if let Some(inputdev) = &rcdev.inputdev {
        let inputdev = match Device::open(inputdev) {
            Ok(l) => l,
            Err(s) => {
                eprintln!("error: {inputdev}: {s}");
                std::process::exit(1);
            }
        };

        fcntl(inputdev.as_raw_fd(), FcntlArg::F_SETFL(OFlag::O_NONBLOCK))
            .expect("should be able to set non-blocking");

        eventdev = Some(inputdev);
    }

    let mut rawbuf = Vec::with_capacity(1024);
    let mut carrier = None;
    let mut leading_space = true;
    let mut scanbuf = Vec::with_capacity(1024);

    println!("Testing events. Press Ctrl+C to abort");

    'outer: loop {
        if let Some(lircdev) = &mut rawdev {
            if let Err(err) = lircdev.receive_raw(&mut rawbuf) {
                if err.kind() != std::io::ErrorKind::WouldBlock {
                    eprintln!("error: {err}");
                    std::process::exit(1);
                }
            }

            print!("{lircdev}: raw: ");

            for entry in &rawbuf {
                if entry.is_space() {
                    if !leading_space {
                        print!("-{} ", entry.value());
                    }
                } else if entry.is_pulse() {
                    if leading_space {
                        leading_space = false;
                    }
                    print!("+{} ", entry.value());
                } else if entry.is_frequency() {
                    carrier = Some(entry.value());
                } else if entry.is_overflow() {
                    if let Some(freq) = carrier {
                        println!(" # receiver overflow, carrier {freq}Hz");
                        carrier = None;
                    } else {
                        println!(" # receiver overflow");
                    }
                    if test.one_shot {
                        break 'outer;
                    }
                    leading_space = true;
                } else if entry.is_timeout() {
                    if let Some(freq) = carrier {
                        println!(" # timeout {}, carrier {}Hz", entry.value(), freq);
                        carrier = None;
                    } else {
                        println!(" # timeout {}", entry.value());
                    }
                    if test.one_shot {
                        break 'outer;
                    }
                    leading_space = true;
                }
            }

            println!();
        }

        if let Some(lircdev) = &mut scandev {
            if let Err(err) = lircdev.receive_scancodes(&mut scanbuf) {
                if err.kind() != std::io::ErrorKind::WouldBlock {
                    eprintln!("error: {err}");
                    std::process::exit(1);
                }
            }

            for entry in &scanbuf {
                let keycode = evdev::KeyCode::new(entry.keycode as u16);

                let timestamp = Duration::new(
                    entry.timestamp / 1_000_000_000,
                    (entry.timestamp % 1_000_000_000) as u32,
                );

                println!(
                    "{lircdev}: scancode: timestamp={timestamp:?} scancode={:x} keycode={:?}{}{}",
                    entry.scancode,
                    keycode,
                    if (entry.flags & LIRC_SCANCODE_FLAG_REPEAT) != 0 {
                        " repeat"
                    } else {
                        ""
                    },
                    if (entry.flags & LIRC_SCANCODE_FLAG_TOGGLE) != 0 {
                        " toggle"
                    } else {
                        ""
                    },
                );
            }
        }

        if let Some(eventdev) = &mut eventdev {
            let display_name = rcdev.inputdev.as_ref().unwrap();

            match eventdev.fetch_events() {
                Ok(iterator) => {
                    for ev in iterator {
                        let timestamp = ev
                            .timestamp()
                            .elapsed()
                            .expect("input time should never exceed system time");

                        let ty = ev.event_type();

                        let summary = ev.destructure();

                        println!(
                            "{display_name}: event: timestamp={timestamp:?} {ty:?} {summary:?}"
                        );
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => (),
                Err(e) => {
                    eprintln!("error: {e}");
                    std::process::exit(1);
                }
            }
        }

        let mut polls = Vec::with_capacity(3);

        if let Some(dev) = &scandev {
            polls.push(PollFd::new(dev.as_fd(), PollFlags::POLLIN));
        }

        if let Some(dev) = &rawdev {
            polls.push(PollFd::new(dev.as_fd(), PollFlags::POLLIN));
        }

        if let Some(dev) = &eventdev {
            polls.push(PollFd::new(dev.as_fd(), PollFlags::POLLIN));
        }

        if let Err(e) = poll(&mut polls, PollTimeout::NONE) {
            if e != Errno::EINTR && e != Errno::EAGAIN {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
    }
}

fn test_raw(rcdev: Rcdev, test: &crate::Test) {
    if let Some(lircdev) = rcdev.lircdev {
        let mut lircdev = open(lircdev, test);

        if !lircdev.can_receive_raw() {
            eprintln!("error: {lircdev} does not support raw mode");
        }

        let mut rawbuf = Vec::with_capacity(1024);
        let mut leading_space = true;
        let mut carrier = None;

        'outer: loop {
            if let Err(err) = lircdev.receive_raw(&mut rawbuf) {
                if err.kind() != std::io::ErrorKind::WouldBlock {
                    eprintln!("error: {err}");
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
                    }
                    print!("+{} ", entry.value());
                } else if entry.is_frequency() {
                    carrier = Some(entry.value());
                } else if entry.is_overflow() {
                    if let Some(freq) = carrier {
                        println!(" # receiver overflow, carrier {freq}Hz");
                        carrier = None;
                    } else {
                        println!(" # receiver overflow");
                    }
                    if test.one_shot {
                        break 'outer;
                    }
                    leading_space = true;
                } else if entry.is_timeout() {
                    if let Some(freq) = carrier {
                        println!(" # timeout {}, carrier {}Hz", entry.value(), freq);
                        carrier = None;
                    } else {
                        println!(" # timeout {}", entry.value());
                    }
                    if test.one_shot {
                        break 'outer;
                    }
                    leading_space = true;
                }
            }
        }
    } else {
        eprintln!("error: {}: has no lirc device", rcdev.name);
        std::process::exit(1);
    }
}

fn open(lircdev: String, test: &crate::Test) -> Lirc {
    let lircpath = PathBuf::from(lircdev);

    let mut lircdev = match Lirc::open(&lircpath) {
        Ok(l) => l,
        Err(s) => {
            eprintln!("error: {}: {}", lircpath.display(), s);
            std::process::exit(1);
        }
    };

    if test.learning {
        let mut learning_mode = false;

        if lircdev.can_measure_carrier() {
            if let Err(err) = lircdev.set_measure_carrier(true) {
                eprintln!("error: {lircdev}: failed to enable measure carrier: {err}");
                std::process::exit(1);
            }
            learning_mode = true;
        }

        if lircdev.can_use_wideband_receiver() {
            if let Err(err) = lircdev.set_wideband_receiver(true) {
                eprintln!("error: {lircdev}: failed to enable wideband receiver: {err}");
                std::process::exit(1);
            }
            learning_mode = true;
        }

        if !learning_mode {
            eprintln!("error: {lircdev}: lirc device does not support learning mode");
            std::process::exit(1);
        }
    } else {
        if lircdev.can_measure_carrier() {
            if let Err(err) = lircdev.set_measure_carrier(false) {
                eprintln!("error: {lircdev}: failed to disable measure carrier: {err}");
                std::process::exit(1);
            }
        }

        if lircdev.can_use_wideband_receiver() {
            if let Err(err) = lircdev.set_wideband_receiver(false) {
                eprintln!("error: {lircdev}: failed to disable wideband receiver: {err}");
                std::process::exit(1);
            }
        }
    }

    if let Some(timeout) = test.timeout {
        if lircdev.can_set_timeout() {
            match lircdev.get_min_max_timeout() {
                Ok(range) if range.contains(&timeout) => {
                    if let Err(err) = lircdev.set_timeout(timeout) {
                        eprintln!("error: {lircdev}: {err}");
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
                    eprintln!("error: {lircdev}: {err}");
                    std::process::exit(1);
                }
            }
        } else {
            eprintln!("error: {lircdev}: changing timeout not supported");
            std::process::exit(1);
        }
    }
    lircdev
}
