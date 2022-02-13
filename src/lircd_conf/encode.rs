use super::{Flags, LircRemote};
use crate::log::Log;
use irp::{Irp, Message, Vartable};
use num_integer::Integer;

pub fn encode(
    lirc_remotes: &[LircRemote],
    remote: &str,
    codes: &[&str],
    log: &Log,
) -> Option<Message> {
    let mut message: Option<Message> = None;

    for send_code in codes {
        for lirc_remote in lirc_remotes {
            if lirc_remote.name != remote {
                continue;
            }

            if !lirc_remote.driver.is_empty() {
                log.error(&format!(
                    "remote {} is for a specific lirc driver {} and cannot be encoded",
                    remote, lirc_remote.driver
                ));
                continue;
            }

            if lirc_remote.flags.contains(Flags::SERIAL) {
                log.error(&format!(
                    "remote {} is for a specific serial driver and cannot be encoded",
                    remote
                ));
                continue;
            }

            // FIXME: should check for multiple definitions of the same code
            if lirc_remote.flags.contains(Flags::RAW_CODES) {
                for raw_code in &lirc_remote.raw_codes {
                    if raw_code.name == *send_code {
                        if raw_code.rawir.is_empty() {
                            log.error(&format!(
                                "remote {} code {} is missing raw codes",
                                remote, send_code
                            ));
                            continue;
                        }
                        let length = if raw_code.rawir.len().is_even() {
                            raw_code.rawir.len() - 1
                        } else {
                            raw_code.rawir.len()
                        };
                        let mut raw = raw_code.rawir[..length].to_vec();

                        let total_length: u32 = raw.iter().sum();

                        let space = if lirc_remote.gap == 0 {
                            log.error(&format!("remote {} does not a specify a gap", remote));
                            20000
                        } else if total_length >= lirc_remote.gap {
                            log.error(&format!("remote {} has a gap of {} which is smaller than the length of the IR {}", remote, lirc_remote.gap, total_length));
                            20000
                        } else {
                            lirc_remote.gap - total_length
                        };

                        raw.push(space);

                        if let Some(message) = &mut message {
                            message.raw.extend_from_slice(&raw);
                        } else {
                            let carrier = if lirc_remote.frequency != 0 {
                                Some(lirc_remote.frequency.into())
                            } else {
                                None
                            };

                            let duty_cycle = if lirc_remote.duty_cycle != 0 {
                                if lirc_remote.duty_cycle < 99 {
                                    Some(lirc_remote.duty_cycle as u8)
                                } else {
                                    log.error(&format!(
                                        "remote {} duty_cycle {} is invalid",
                                        remote, lirc_remote.duty_cycle
                                    ));
                                    None
                                }
                            } else {
                                None
                            };
                            message = Some(Message {
                                carrier,
                                duty_cycle,
                                raw,
                            });
                        }
                    }
                }
            } else {
                // FIXME: should check for multiple definitions of the same code
                for code in &lirc_remote.codes {
                    if code.name == *send_code {
                        let irp = match lirc_remote.irp(log) {
                            Ok(s) => s,
                            Err(_) => continue,
                        };

                        log.info(&format!("irp for remote {}: {}", lirc_remote.name, irp));

                        let mut vars = Vartable::new();
                        vars.set(String::from("CODE"), code.code as i64, 32);
                        let irp = Irp::parse(&irp).expect("should parse");

                        // FIXME: concatenate multiple messages (impl on Message?)
                        // FIXME: should be possible to specify repeats
                        message = Some(irp.encode(vars, 0).expect("encode should succeed"));
                    }
                }
            }
        }
    }

    message
}
