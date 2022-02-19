use super::{Flags, LircRemote};
use crate::log::Log;
use irp::{Irp, Message, Vartable};
use num_integer::Integer;

pub fn encode(
    lirc_remotes: &[LircRemote],
    remote: Option<&str>,
    codes: &[&str],
    log: &Log,
) -> Result<Message, String> {
    let mut message: Option<Message> = None;
    let mut remote_found = false;

    for send_code in codes {
        let mut code_found = false;

        for lirc_remote in lirc_remotes {
            if let Some(remote) = remote {
                if lirc_remote.name != remote {
                    continue;
                }
                remote_found = true;
            }

            for raw_code in &lirc_remote.raw_codes {
                if raw_code.name == *send_code {
                    if code_found {
                        log.warning(&format!("multiple definitions of code {} found", send_code));
                        break;
                    }

                    if raw_code.rawir.is_empty() {
                        log.error(&format!(
                            "remote {} code {} is missing raw codes",
                            lirc_remote.name, send_code
                        ));
                        continue;
                    }
                    let length = if raw_code.rawir.len().is_even() {
                        raw_code.rawir.len() - 1
                    } else {
                        raw_code.rawir.len()
                    };

                    let mut raw = raw_code.rawir[..length].to_vec();

                    let space = if lirc_remote.gap == 0 {
                        log.error(&format!(
                            "remote {} does not a specify a gap",
                            lirc_remote.name
                        ));
                        20000
                    } else if lirc_remote.flags.contains(Flags::CONST_LENGTH) {
                        let total_length: u32 = raw.iter().sum();

                        lirc_remote.gap as u32 - total_length
                    } else {
                        lirc_remote.gap as u32
                    };

                    raw.push(space);

                    if lirc_remote.min_repeat != 0 {
                        for _ in 0..lirc_remote.min_repeat {
                            raw.extend(&raw_code.rawir[..length]);
                            raw.push(space);
                        }
                    }

                    if let Some(message) = &mut message {
                        message.raw.extend_from_slice(&raw);
                    } else {
                        let carrier = if lirc_remote.frequency != 0 {
                            Some(lirc_remote.frequency as i64)
                        } else {
                            None
                        };

                        let duty_cycle = if lirc_remote.duty_cycle != 0 {
                            if lirc_remote.duty_cycle < 99 {
                                Some(lirc_remote.duty_cycle as u8)
                            } else {
                                log.error(&format!(
                                    "remote {} duty_cycle {} is invalid",
                                    lirc_remote.name, lirc_remote.duty_cycle
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
                    code_found = true;
                }
            }

            for code in &lirc_remote.codes {
                if code.name == *send_code {
                    if code_found {
                        log.warning(&format!("multiple definitions of code {} found", send_code));
                        break;
                    }

                    let irp = lirc_remote.irp();

                    log.info(&format!("irp for remote {}: {}", lirc_remote.name, irp));

                    let mut vars = Vartable::new();
                    vars.set(String::from("CODE"), code.code as i64, 32);
                    let irp = Irp::parse(&irp).expect("should parse");

                    // FIXME: concatenate multiple messages (impl on Message?)
                    // FIXME: should be possible to specify repeats
                    message = Some(irp.encode(vars, 0).expect("encode should succeed"));

                    code_found = true;
                }
            }
        }

        if !code_found {
            return Err(format!("code {} not found", send_code));
        }
    }

    if let Some(message) = message {
        Ok(message)
    } else {
        if let Some(remote) = remote {
            if !remote_found {
                return Err(format!("remote {} not found", remote));
            }
        }

        Err(String::from("Nothing to send"))
    }
}
