use super::{find_devices, Purpose};
use cir::{
    lirc,
    lircd_conf::{parse, Remote},
};
use irp::{Decoder, InfraredData, Irp, Message, NFA};
use itertools::Itertools;
use log::{error, info, trace};
use num_integer::Integer;
use std::{
    fs,
    path::{Path, PathBuf},
    str,
};

pub fn decode(matches: &clap::ArgMatches) {
    let remotes;
    let nfa_graphviz = matches.value_of("GRAPHVIZ") == Some("nfa");

    let abs_tolerance = str::parse(matches.value_of("AEPS").unwrap()).expect("number expected");
    let rel_tolerance = str::parse(matches.value_of("EPS").unwrap()).expect("number expected");

    let irps = if let Some(i) = matches.value_of("IRP") {
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

        if nfa_graphviz {
            let filename = "irp_nfa.dot";
            info!("saving nfa as {}", filename);

            nfa.dotgraphviz(filename);
        }

        vec![(None, nfa)]
    } else if let Some(filename) = matches.value_of_os("LIRCDCONF") {
        remotes = match parse(filename) {
            Ok(r) => r,
            Err(_) => std::process::exit(2),
        };

        remotes
            .iter()
            .map(|remote| {
                let irp = remote.irp();

                info!("found remote {}", remote.name);
                info!("IRP {}", irp);

                let irp = Irp::parse(&irp).unwrap();

                let nfa = irp.compile().unwrap();

                if nfa_graphviz {
                    let filename = format!("{}_nfa.dot", remote.name);
                    info!("saving nfa as {}", filename);

                    nfa.dotgraphviz(&filename);
                }

                (Some(remote), nfa)
            })
            .collect()
    } else {
        unreachable!();
    };

    let mut input_on_cli = false;

    if let Some(files) = matches.values_of_os("FILE") {
        input_on_cli = true;

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
                    process(&raw.raw, &irps, matches, abs_tolerance, rel_tolerance);
                }
                Err(msg) => {
                    info!("parsing ‘{}’ as mode2", filename.to_string_lossy());

                    match Message::parse_mode2(&input) {
                        Ok(m) => {
                            info!("decoding: {}", m.print_rawir());
                            process(&m.raw, &irps, matches, abs_tolerance, rel_tolerance);
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
        input_on_cli = true;

        for rawir in rawirs {
            match Message::parse(rawir) {
                Ok(raw) => {
                    info!("decoding: {}", raw.print_rawir());
                    process(&raw.raw, &irps, matches, abs_tolerance, rel_tolerance);
                }
                Err(msg) => {
                    error!("parsing ‘{}’: {}", rawir, msg);
                    std::process::exit(2);
                }
            }
        }
    }

    if !input_on_cli {
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
                let mut rawbuf = Vec::with_capacity(1024);

                let abs_tolerance = if let Ok(resolution) = lircdev.receiver_resolution() {
                    if resolution > abs_tolerance {
                        info!(
                            "{} resolution is {}, using absolute tolerance {} rather than {}",
                            lircdev, resolution, resolution, abs_tolerance
                        );

                        resolution
                    } else {
                        abs_tolerance
                    }
                } else {
                    abs_tolerance
                };

                let max_gap = if let Ok(timeout) = lircdev.get_timeout() {
                    let max_gap = (timeout * 9) / 10;

                    trace!(
                        "device reports timeout of {}, using 90% of that as {} max_gap",
                        timeout,
                        max_gap
                    );

                    max_gap
                } else {
                    20000
                };

                // TODO: for each remote, use eps/aeps from lircd.conf if it was NOT specified on the command line
                let mut matchers = irps
                    .iter()
                    .map(|(remote, nfa)| {
                        (remote, nfa.decoder(abs_tolerance, rel_tolerance, max_gap))
                    })
                    .collect::<Vec<(&Option<&Remote>, Decoder)>>();

                loop {
                    if let Err(err) = lircdev.receive_raw(&mut rawbuf) {
                        eprintln!("error: {err}");
                        std::process::exit(1);
                    }

                    for raw in &rawbuf {
                        let ir = if raw.is_pulse() {
                            InfraredData::Flash(raw.value())
                        } else if raw.is_space() || raw.is_timeout() {
                            InfraredData::Gap(raw.value())
                        } else if raw.is_overflow() {
                            InfraredData::Reset
                        } else {
                            continue;
                        };

                        trace!("decoding: {}", ir);

                        for (remote, matcher) in &mut matchers {
                            matcher.input(ir);

                            while let Some((event, var)) = matcher.get() {
                                if let Some(remote) = remote {
                                    // lirc
                                    let decoded_code = var["CODE"] as u64;

                                    // TODO: raw codes
                                    if let Some(code) = remote
                                        .codes
                                        .iter()
                                        .find(|code| code.code[0] == decoded_code)
                                    {
                                        println!("remote:{} code:{}", remote.name, code.name);
                                    } else {
                                        println!(
                                            "remote:{} unmapped code:{:x}",
                                            remote.name, decoded_code
                                        );
                                    }
                                } else {
                                    // lirc remote
                                    let mut var: Vec<(String, i64)> = var.into_iter().collect();
                                    var.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
                                    println!(
                                        "decoded: {} {}",
                                        event,
                                        var.iter()
                                            .map(|(name, val)| format!("{name}={val}"))
                                            .join(", ")
                                    );
                                }
                            }
                        }
                    }
                }
            } else {
                error!("{}: device cannot receive raw", lircdev);
                std::process::exit(1);
            }
        }
    }
}

fn process(
    raw: &[u32],
    irps: &[(Option<&Remote>, NFA)],
    matches: &clap::ArgMatches,
    abs_tolerance: u32,
    rel_tolerance: u32,
) {
    let graphviz = matches.value_of("GRAPHVIZ") == Some("nfa-step");

    for (remote, nfa) in irps {
        let mut matcher = nfa.decoder(abs_tolerance, rel_tolerance, 20000);

        for (index, raw) in raw.iter().enumerate() {
            let ir = if index.is_odd() {
                InfraredData::Gap(*raw)
            } else {
                InfraredData::Flash(*raw)
            };

            matcher.input(ir);

            while let Some((event, var)) = matcher.get() {
                if let Some(remote) = remote {
                    // lirc
                    let decoded_code = var["CODE"] as u64;

                    // TODO: raw codes
                    if let Some(code) = remote
                        .codes
                        .iter()
                        .find(|code| code.code[0] == decoded_code)
                    {
                        println!("remote:{} code:{}", remote.name, code.name);
                    } else {
                        println!("remote:{} unmapped code:{:x}", remote.name, decoded_code);
                    }
                } else {
                    // lirc remote
                    let mut var: Vec<(String, i64)> = var.into_iter().collect();
                    var.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
                    println!(
                        "decoded: {} {}",
                        event,
                        var.iter()
                            .map(|(name, val)| format!("{name}={val}"))
                            .join(", ")
                    );
                }
            }

            if graphviz {
                let filename = format!(
                    "{}_nfa_step_{:04}.dot",
                    if let Some(remote) = remote {
                        &remote.name
                    } else {
                        "irp"
                    },
                    index
                );

                info!("saving nfa at step {} as {}", index, filename);

                matcher.dotgraphviz(&filename);
            }
        }
    }
}
