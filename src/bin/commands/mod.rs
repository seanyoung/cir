use cir::{
    lirc,
    rcdev::{enumerate_rc_dev, Rcdev},
};
use log::debug;
use std::path::PathBuf;

pub mod config;
pub mod decode;
pub mod test;
pub mod transmit;

pub enum Purpose {
    Receive,
    Transmit,
}

/// Enumerate all rc devices and find the lirc and input devices
pub fn find_devices(matches: &clap::ArgMatches, purpose: Purpose) -> Rcdev {
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

    let entry = if let Some(rcdev) = matches.value_of("RCDEV") {
        if let Some(entry) = list.iter().position(|rc| rc.name == rcdev) {
            entry
        } else {
            eprintln!("error: {rcdev} not found");
            std::process::exit(1);
        }
    } else if let Some(lircdev) = matches.value_of("LIRCDEV") {
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

pub fn open_lirc(matches: &clap::ArgMatches, purpose: Purpose) -> lirc::Lirc {
    let rcdev = find_devices(matches, purpose);

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
