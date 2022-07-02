use super::{find_devices, Purpose};
use cir::{
    lirc,
    lircd_conf::{parse, Remote},
    log::Log,
};
use irp::{InfraredData, Irp, Matcher, NFA};
use itertools::Itertools;
use num_integer::Integer;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn decode(matches: &clap::ArgMatches, log: &Log) {
    let remotes;

    let irps: Vec<(Option<&Remote>, NFA)> = match matches.subcommand() {
        Some(("irp", matches)) => {
            let i = matches.value_of("IRP").unwrap();

            let irp = match Irp::parse(i) {
                Ok(m) => m,
                Err(s) => {
                    eprintln!("unable to parse irp ‘{}’: {}", i, s);
                    std::process::exit(2);
                }
            };

            vec![(None, irp.build_nfa().unwrap())]
        }
        Some(("lircd", matches)) => {
            let filename = matches.value_of_os("CONF").unwrap();

            remotes = match parse(filename, log) {
                Ok(r) => r,
                Err(_) => std::process::exit(2),
            };

            remotes
                .iter()
                .map(|remote| {
                    let irp = remote.irp();

                    log.info(&format!("found remote {}", remote.name));
                    log.info(&format!("IRP {}", irp));

                    let irp = Irp::parse(&irp).unwrap();

                    (Some(remote), irp.build_nfa().unwrap())
                })
                .collect()
        }
        _ => unreachable!(),
    };

    let mut input_on_cli = false;

    if let Some(files) = matches.values_of_os("FILE") {
        input_on_cli = true;

        for filename in files {
            let input = match fs::read_to_string(filename) {
                Ok(s) => s,
                Err(s) => {
                    log.error(&format!("{}: {}", Path::new(filename).display(), s));
                    std::process::exit(2);
                }
            };

            match irp::rawir::parse(&input) {
                Ok(raw) => {
                    process(&raw, &irps);
                }
                Err(msg) => match irp::mode2::parse(&input) {
                    Ok(m) => {
                        process(&m.raw, &irps);
                    }
                    Err((line_no, error)) => {
                        log.error(&format!(
                            "{}: parse as rawir: {}",
                            Path::new(filename).display(),
                            msg
                        ));
                        log.error(&format!(
                            "{}:{}: parse as mode2: {}",
                            Path::new(filename).display(),
                            line_no,
                            error
                        ));
                        std::process::exit(2);
                    }
                },
            }
        }
    }

    if let Some(rawirs) = matches.values_of("RAWIR") {
        input_on_cli = true;

        for rawir in rawirs {
            match irp::rawir::parse(rawir) {
                Ok(raw) => {
                    process(&raw, &irps);
                }
                Err(msg) => {
                    log.error(&msg);
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
            }

            if lircdev.can_receive_raw() {
                let mut rawbuf = Vec::with_capacity(1024);
                let resolution = lircdev.receiver_resolution().unwrap_or(100);

                let mut matchers = irps
                    .iter()
                    .map(|(remote, nfa)| (remote, nfa.matcher(resolution, 100)))
                    .collect::<Vec<(&Option<&Remote>, Matcher)>>();

                loop {
                    if let Err(err) = lircdev.receive_raw(&mut rawbuf) {
                        eprintln!("error: {}", err);
                        std::process::exit(1);
                    }

                    for raw in &rawbuf {
                        let data = if raw.is_pulse() {
                            InfraredData::Flash(raw.value())
                        } else if raw.is_space() || raw.is_timeout() {
                            InfraredData::Gap(raw.value())
                        } else if raw.is_overflow() {
                            InfraredData::Reset
                        } else {
                            continue;
                        };

                        for (remote, matcher) in &mut matchers {
                            if let Some(var) = matcher.input(data) {
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
                                    println!(
                                        "decoded: {}",
                                        var.iter()
                                            .map(|(name, val)| format!("{}={:x}", name, val))
                                            .join(", ")
                                    );
                                }
                            }
                        }
                    }
                }
            } else {
                log.error(&format!("{}: device cannot receive raw", lircdev));
                std::process::exit(1);
            }
        }
    }
}

fn process(raw: &[u32], irps: &[(Option<&Remote>, NFA)]) {
    for (remote, nfa) in irps {
        let mut matcher = nfa.matcher(100, 100);

        for (index, raw) in raw.iter().enumerate() {
            let data = if index.is_odd() {
                InfraredData::Gap(*raw)
            } else {
                InfraredData::Flash(*raw)
            };

            if let Some(var) = matcher.input(data) {
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
                    println!(
                        "decoded: {}",
                        var.iter()
                            .map(|(name, val)| format!("{}={:x}", name, val))
                            .join(", ")
                    );
                }
            }
        }
    }
}
