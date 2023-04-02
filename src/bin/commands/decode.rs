use super::{find_devices, Purpose};
use cir::{lirc, lircd_conf::parse};
use irp::{Decoder, InfraredData, Irp, Message};
use itertools::Itertools;
use log::{error, info, trace};
use std::{
    fs,
    path::{Path, PathBuf},
    str,
};

pub fn decode(matches: &clap::ArgMatches) {
    match matches.subcommand() {
        Some(("irp", matches)) => decode_irp(matches),
        Some(("lircd", matches)) => decode_lircd(matches),
        _ => unreachable!(),
    }
}

fn decode_irp(matches: &clap::ArgMatches) {
    let graphviz_step = matches.value_of("GRAPHVIZ") == Some("nfa-step");
    let graphviz = matches.value_of("GRAPHVIZ") == Some("nfa");

    let mut abs_tolerance = str::parse(matches.value_of("AEPS").unwrap()).expect("number expected");
    let rel_tolerance = str::parse(matches.value_of("EPS").unwrap()).expect("number expected");
    let mut max_gap = 100000;

    let i = matches.value_of("IRP").unwrap();

    let irp = match Irp::parse(i) {
        Ok(m) => m,
        Err(s) => {
            eprintln!("unable to parse irp ‘{i}’: {s}");
            std::process::exit(2);
        }
    };

    let nfa = match irp.compile() {
        Ok(nfa) => nfa,
        Err(s) => {
            eprintln!("unable to compile irp ‘{i}’: {s}");
            std::process::exit(2);
        }
    };

    if graphviz {
        let filename = "irp_nfa.dot";
        info!("saving nfa as {}", filename);

        nfa.dotgraphviz(filename);
    }

    let input_on_cli = matches.get_count("FILE") != 0 || matches.get_count("RAWIR") != 0;

    let lircdev = if !input_on_cli {
        // open lirc
        let rcdev = find_devices(matches, Purpose::Receive);

        if let Some(lircdev) = rcdev.lircdev {
            let lircpath = PathBuf::from(lircdev);

            trace!("opening lirc device: {}", lircpath.display());

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
                    if resolution > abs_tolerance {
                        info!(
                            "{} resolution is {}, using absolute tolerance {} rather than {}",
                            lircdev, resolution, resolution, abs_tolerance
                        );

                        abs_tolerance = resolution;
                    }
                }

                if let Ok(timeout) = lircdev.get_timeout() {
                    let dev_max_gap = (timeout * 9) / 10;

                    trace!(
                        "device reports timeout of {}, using 90% of that as {} max_gap",
                        timeout,
                        dev_max_gap
                    );

                    max_gap = dev_max_gap;
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
    } else {
        None
    };

    let mut decoder = Decoder::new(abs_tolerance, rel_tolerance, max_gap);

    let mut feed_decoder = |raw: &[InfraredData]| {
        for (index, ir) in raw.iter().enumerate() {
            decoder.input(*ir, &nfa, |event, var| {
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

            if graphviz_step {
                let filename = format!("irp_nfa_step_{:04}.dot", index);

                info!("saving nfa at step {} as {}", index, filename);

                decoder.dotgraphviz(&filename, &nfa);
            }
        }
    };

    if let Some(files) = matches.values_of_os("FILE") {
        for filename in files {
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
    }

    if let Some(rawirs) = matches.values_of("RAWIR") {
        for rawir in rawirs {
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
    }

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

            trace!("decoding: {}", raw.iter().join(" "));

            feed_decoder(&raw);
        }
    }
}

fn decode_lircd(matches: &clap::ArgMatches) {
    let graphviz_step = matches.value_of("GRAPHVIZ") == Some("nfa-step");
    let graphviz = matches.value_of("GRAPHVIZ") == Some("nfa");

    let mut abs_tolerance = str::parse(matches.value_of("AEPS").unwrap()).expect("number expected");
    let rel_tolerance = str::parse(matches.value_of("EPS").unwrap()).expect("number expected");
    let mut max_gap = 100000;

    let conf = matches.value_of_os("LIRCDCONF").unwrap();

    let remotes = match parse(conf) {
        Ok(r) => r,
        Err(_) => std::process::exit(2),
    };

    let input_on_cli = matches.is_present("FILE") || matches.is_present("RAWIR");

    let lircdev = if !input_on_cli {
        // open lirc
        let rcdev = find_devices(matches, Purpose::Receive);

        if let Some(lircdev) = rcdev.lircdev {
            let lircpath = PathBuf::from(lircdev);

            trace!("opening lirc device: {}", lircpath.display());

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
                    if resolution > abs_tolerance {
                        info!(
                            "{} resolution is {}, using absolute tolerance {} rather than {}",
                            lircdev, resolution, resolution, abs_tolerance
                        );

                        abs_tolerance = resolution;
                    }
                }

                if let Ok(timeout) = lircdev.get_timeout() {
                    let dev_max_gap = (timeout * 9) / 10;

                    trace!(
                        "device reports timeout of {}, using 90% of that as {} max_gap",
                        timeout,
                        dev_max_gap
                    );

                    max_gap = dev_max_gap;
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
    } else {
        None
    };

    let mut decoders = remotes
        .iter()
        .map(|remote| {
            let decoder = remote.decoder(abs_tolerance, rel_tolerance, max_gap);

            if graphviz {
                let filename = format!("{}_nfa.dot", remote.name);
                info!("saving nfa as {}", filename);

                decoder.nfa.dotgraphviz(&filename);
            }

            decoder
        })
        .collect::<Vec<_>>();

    let mut feed_decoder = |raw: &[InfraredData]| {
        for (index, ir) in raw.iter().enumerate() {
            for decoder in &mut decoders {
                decoder.input(*ir, |bits, code| {
                    if let Some(code) = code {
                        println!(
                            "decoded: remote:{} value:{:#x} code:{}",
                            decoder.remote.name, bits, code.name
                        );
                    } else {
                        println!("decoded: remote:{} value:{:#x}", decoder.remote.name, bits,);
                    }
                });

                if graphviz_step {
                    let filename = format!("{}_nfa_step_{:04}.dot", decoder.remote.name, index);

                    info!("saving nfa at step {} as {}", index, filename);

                    decoder.nfa.dotgraphviz(&filename);
                }
            }
        }
    };

    if let Some(files) = matches.values_of_os("FILE") {
        for filename in files {
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
    }

    if let Some(rawirs) = matches.values_of("RAWIR") {
        for rawir in rawirs {
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
    }

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

            trace!("decoding: {}", raw.iter().join(" "));

            feed_decoder(&raw);
        }
    }
}
