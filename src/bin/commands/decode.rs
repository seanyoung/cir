#[cfg(target_os = "linux")]
use super::config::{find_devices, Purpose};
#[cfg(target_os = "linux")]
use cir::lirc::Lirc;
use cir::{keymap::Keymap, lircd_conf::parse};
use irp::{Decoder, InfraredData, Irp, Message, Options};
use itertools::Itertools;
use log::{error, info};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn decode_irp(decode: &crate::Decode, irp_str: &String) {
    #[allow(unused_mut)]
    let mut abs_tolerance = decode.options.aeps;
    let rel_tolerance = decode.options.eps;
    #[allow(unused_mut)]
    let mut max_gap = 100000;

    let irp = match Irp::parse(irp_str) {
        Ok(m) => m,
        Err(s) => {
            eprintln!("unable to parse irp ‘{irp_str}’: {s}");
            std::process::exit(2);
        }
    };

    let input_on_cli = !decode.file.is_empty() || !decode.rawir.is_empty();

    #[cfg(target_os = "linux")]
    let lircdev = open_lirc(input_on_cli, decode, &mut abs_tolerance, &mut max_gap);

    #[cfg(not(target_os = "linux"))]
    if !input_on_cli {
        eprintln!("no infrared input provided");
        std::process::exit(2);
    }

    let mut options = Options {
        name: "irp",
        aeps: abs_tolerance,
        eps: rel_tolerance,
        max_gap,
        ..Default::default()
    };

    options.nfa = decode.options.save_nfa;
    options.dfa = decode.options.save_dfa;
    let dfa = match irp.compile(&options) {
        Ok(dfa) => dfa,
        Err(s) => {
            eprintln!("unable to compile irp ‘{irp_str}’: {s}");
            std::process::exit(2);
        }
    };

    let mut decoder = Decoder::new(options);

    let mut feed_decoder = |raw: &[InfraredData]| {
        for ir in raw {
            decoder.dfa_input(*ir, &dfa, |event, var| {
                let mut var: Vec<(String, i64)> = var.into_iter().collect();
                var.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
                println!(
                    "decoded: {} {}",
                    event,
                    var.iter()
                        .map(|(name, val)| format!("{name}={val}"))
                        .join(", ")
                );
            });
        }
    };

    for filename in &decode.file {
        let input = match fs::read_to_string(filename) {
            Ok(s) => s,
            Err(s) => {
                error!("{}: {}", Path::new(filename).display(), s);
                std::process::exit(2);
            }
        };

        info!("parsing ‘{}’ as rawir", filename.to_string_lossy());

        match Message::parse(&input) {
            Ok(raw) => {
                info!("decoding: {}", raw.print_rawir());
                feed_decoder(&InfraredData::from_u32_slice(&raw.raw));
            }
            Err(msg) => {
                info!("parsing ‘{}’ as mode2", filename.to_string_lossy());

                match Message::parse_mode2(&input) {
                    Ok(m) => {
                        info!("decoding: {}", m.print_rawir());
                        feed_decoder(&InfraredData::from_u32_slice(&m.raw));
                    }
                    Err((line_no, error)) => {
                        error!("{}: parse as rawir: {}", Path::new(filename).display(), msg);
                        error!(
                            "{}:{}: parse as mode2: {}",
                            Path::new(filename).display(),
                            line_no,
                            error
                        );
                        std::process::exit(2);
                    }
                }
            }
        }
    }

    for rawir in &decode.rawir {
        match Message::parse(rawir) {
            Ok(raw) => {
                info!("decoding: {}", raw.print_rawir());
                feed_decoder(&InfraredData::from_u32_slice(&raw.raw));
            }
            Err(msg) => {
                error!("parsing ‘{}’: {}", rawir, msg);
                std::process::exit(2);
            }
        }
    }

    #[cfg(target_os = "linux")]
    if let Some(mut lircdev) = lircdev {
        let mut rawbuf = Vec::with_capacity(1024);

        loop {
            if let Err(err) = lircdev.receive_raw(&mut rawbuf) {
                eprintln!("error: {err}");
                std::process::exit(1);
            }

            let raw: Vec<_> = rawbuf
                .iter()
                .filter_map(|raw| {
                    if raw.is_pulse() {
                        Some(InfraredData::Flash(raw.value()))
                    } else if raw.is_space() || raw.is_timeout() {
                        Some(InfraredData::Gap(raw.value()))
                    } else if raw.is_overflow() {
                        Some(InfraredData::Reset)
                    } else {
                        None
                    }
                })
                .collect();

            log::trace!("decoding: {}", raw.iter().join(" "));

            feed_decoder(&raw);
        }
    }
}

#[cfg(target_os = "linux")]
fn open_lirc(
    input_on_cli: bool,
    decode: &crate::Decode,
    abs_tolerance: &mut u32,
    max_gap: &mut u32,
) -> Option<Lirc> {
    if input_on_cli {
        return None;
    }

    // open lirc
    let rcdev = find_devices(&decode.device, Purpose::Receive);

    if let Some(lircdev) = rcdev.lircdev {
        let lircpath = std::path::PathBuf::from(lircdev);

        log::trace!("opening lirc device: {}", lircpath.display());

        let mut lircdev = match Lirc::open(&lircpath) {
            Ok(l) => l,
            Err(s) => {
                eprintln!("error: {}: {}", lircpath.display(), s);
                std::process::exit(1);
            }
        };

        if decode.learning {
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
        }

        if lircdev.can_receive_raw() {
            if let Ok(resolution) = lircdev.receiver_resolution() {
                if resolution > *abs_tolerance {
                    log::info!(
                        "{} resolution is {}, using absolute tolerance {} rather than {}",
                        lircdev,
                        resolution,
                        resolution,
                        abs_tolerance
                    );

                    *abs_tolerance = resolution;
                }
            }

            if let Ok(timeout) = lircdev.get_timeout() {
                let dev_max_gap = (timeout * 9) / 10;

                log::trace!(
                    "device reports timeout of {}, using 90% of that as {} max_gap",
                    timeout,
                    dev_max_gap
                );

                *max_gap = dev_max_gap;
            }

            Some(lircdev)
        } else {
            error!("{}: device cannot receive raw", lircdev);
            std::process::exit(1);
        }
    } else {
        error!("{}: no lirc device found", rcdev.name);
        std::process::exit(1);
    }
}

pub fn decode_keymap(decode: &crate::Decode, path: &Path) {
    #[allow(unused_mut)]
    let mut abs_tolerance = decode.options.aeps;
    let rel_tolerance = decode.options.eps;
    #[allow(unused_mut)]
    let mut max_gap = 100000;

    let keymaps = match Keymap::parse(path) {
        Ok(r) => r,
        Err(e) => {
            log::error!("{e}");
            std::process::exit(2);
        }
    };

    let input_on_cli = !decode.file.is_empty() || !decode.rawir.is_empty();

    #[cfg(target_os = "linux")]
    let lircdev = open_lirc(input_on_cli, decode, &mut abs_tolerance, &mut max_gap);

    #[cfg(not(target_os = "linux"))]
    if !input_on_cli {
        eprintln!("no infrared input provided");
        std::process::exit(2);
    }

    let mut decoders = keymaps
        .iter()
        .map(|keymap| {
            let mut options = Options {
                name: &keymap.name,
                max_gap,
                aeps: abs_tolerance,
                eps: rel_tolerance,
                ..Default::default()
            };

            options.nfa = decode.options.save_nfa;
            options.dfa = decode.options.save_dfa;

            match keymap.decoder(options) {
                Ok(decoder) => decoder,
                Err(e) => {
                    log::error!("{}: {e}", path.display());
                    std::process::exit(2);
                }
            }
        })
        .collect::<Vec<_>>();

    let mut feed_decoder = |raw: &[InfraredData]| {
        for ir in raw {
            for decoder in &mut decoders {
                decoder.input(*ir, |name, _| {
                    println!("decoded: keymap:{} code:{}", decoder.remote.name, name);
                });
            }
        }
    };

    for filename in &decode.file {
        let input = match fs::read_to_string(filename) {
            Ok(s) => s,
            Err(s) => {
                error!("{}: {}", Path::new(filename).display(), s);
                std::process::exit(2);
            }
        };

        info!("parsing ‘{}’ as rawir", filename.to_string_lossy());

        match Message::parse(&input) {
            Ok(raw) => {
                info!("decoding: {}", raw.print_rawir());
                feed_decoder(&InfraredData::from_u32_slice(&raw.raw));
            }
            Err(msg) => {
                info!("parsing ‘{}’ as mode2", filename.to_string_lossy());

                match Message::parse_mode2(&input) {
                    Ok(m) => {
                        info!("decoding: {}", m.print_rawir());
                        feed_decoder(&InfraredData::from_u32_slice(&m.raw));
                    }
                    Err((line_no, error)) => {
                        error!("{}: parse as rawir: {}", Path::new(filename).display(), msg);
                        error!(
                            "{}:{}: parse as mode2: {}",
                            Path::new(filename).display(),
                            line_no,
                            error
                        );
                        std::process::exit(2);
                    }
                }
            }
        }
    }

    for rawir in &decode.rawir {
        match Message::parse(rawir) {
            Ok(raw) => {
                info!("decoding: {}", raw.print_rawir());
                feed_decoder(&InfraredData::from_u32_slice(&raw.raw));
            }
            Err(msg) => {
                error!("parsing ‘{}’: {}", rawir, msg);
                std::process::exit(2);
            }
        }
    }

    #[cfg(target_os = "linux")]
    if let Some(mut lircdev) = lircdev {
        let mut rawbuf = Vec::with_capacity(1024);

        loop {
            if let Err(err) = lircdev.receive_raw(&mut rawbuf) {
                eprintln!("error: {err}");
                std::process::exit(1);
            }

            let raw: Vec<_> = rawbuf
                .iter()
                .filter_map(|raw| {
                    if raw.is_pulse() {
                        Some(InfraredData::Flash(raw.value()))
                    } else if raw.is_space() || raw.is_timeout() {
                        Some(InfraredData::Gap(raw.value()))
                    } else if raw.is_overflow() {
                        Some(InfraredData::Reset)
                    } else {
                        None
                    }
                })
                .collect();

            log::trace!("decoding: {}", raw.iter().join(" "));

            feed_decoder(&raw);
        }
    }
}

pub fn decode_lircd(decode: &crate::Decode, conf: &PathBuf) {
    #[allow(unused_mut)]
    let mut abs_tolerance = decode.options.aeps;
    let rel_tolerance = decode.options.eps;
    #[allow(unused_mut)]
    let mut max_gap = 100000;

    let remotes = match parse(conf) {
        Ok(r) => r,
        Err(_) => std::process::exit(2),
    };

    let input_on_cli = !decode.file.is_empty() || !decode.rawir.is_empty();

    #[cfg(target_os = "linux")]
    let lircdev = open_lirc(input_on_cli, decode, &mut abs_tolerance, &mut max_gap);

    #[cfg(not(target_os = "linux"))]
    if !input_on_cli {
        eprintln!("no infrared input provided");
        std::process::exit(2);
    }

    let mut decoders = remotes
        .iter()
        .map(|remote| {
            let mut options =
                remote.default_options(Some(abs_tolerance), Some(rel_tolerance), max_gap);

            options.nfa = decode.options.save_nfa;
            options.dfa = decode.options.save_dfa;

            remote.decoder(options)
        })
        .collect::<Vec<_>>();

    let mut feed_decoder = |raw: &[InfraredData]| {
        for ir in raw {
            for decoder in &mut decoders {
                decoder.input(*ir, |name, _| {
                    println!("decoded: remote:{} code:{}", decoder.remote.name, name);
                });
            }
        }
    };

    for filename in &decode.file {
        let input = match fs::read_to_string(filename) {
            Ok(s) => s,
            Err(s) => {
                error!("{}: {}", Path::new(filename).display(), s);
                std::process::exit(2);
            }
        };

        info!("parsing ‘{}’ as rawir", filename.to_string_lossy());

        match Message::parse(&input) {
            Ok(raw) => {
                info!("decoding: {}", raw.print_rawir());
                feed_decoder(&InfraredData::from_u32_slice(&raw.raw));
            }
            Err(msg) => {
                info!("parsing ‘{}’ as mode2", filename.to_string_lossy());

                match Message::parse_mode2(&input) {
                    Ok(m) => {
                        info!("decoding: {}", m.print_rawir());
                        feed_decoder(&InfraredData::from_u32_slice(&m.raw));
                    }
                    Err((line_no, error)) => {
                        error!("{}: parse as rawir: {}", Path::new(filename).display(), msg);
                        error!(
                            "{}:{}: parse as mode2: {}",
                            Path::new(filename).display(),
                            line_no,
                            error
                        );
                        std::process::exit(2);
                    }
                }
            }
        }
    }

    for rawir in &decode.rawir {
        match Message::parse(rawir) {
            Ok(raw) => {
                info!("decoding: {}", raw.print_rawir());
                feed_decoder(&InfraredData::from_u32_slice(&raw.raw));
            }
            Err(msg) => {
                error!("parsing ‘{}’: {}", rawir, msg);
                std::process::exit(2);
            }
        }
    }

    #[cfg(target_os = "linux")]
    if let Some(mut lircdev) = lircdev {
        let mut rawbuf = Vec::with_capacity(1024);

        loop {
            if let Err(err) = lircdev.receive_raw(&mut rawbuf) {
                eprintln!("error: {err}");
                std::process::exit(1);
            }

            let raw: Vec<_> = rawbuf
                .iter()
                .filter_map(|raw| {
                    if raw.is_pulse() {
                        Some(InfraredData::Flash(raw.value()))
                    } else if raw.is_space() || raw.is_timeout() {
                        Some(InfraredData::Gap(raw.value()))
                    } else if raw.is_overflow() {
                        Some(InfraredData::Reset)
                    } else {
                        None
                    }
                })
                .collect();

            log::trace!("decoding: {}", raw.iter().join(" "));

            feed_decoder(&raw);
        }
    }
}
