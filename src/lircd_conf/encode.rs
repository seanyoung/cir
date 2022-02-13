use super::{Flags, LircRemote};
use crate::log::Log;
use irp::Message;
use num_integer::Integer;

pub fn encode(
    lirc_remotes: &[LircRemote],
    remote: &str,
    codes: &[&str],
    log: &Log,
) -> Option<Message> {
    let mut message: Option<Message> = None;

    for code in codes {
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

            if lirc_remote.flags.contains(Flags::RAW_CODES) {
                for raw_code in &lirc_remote.raw_codes {
                    if raw_code.name == *code {
                        if raw_code.rawir.is_empty() {
                            log.error(&format!(
                                "remote {} code {} is missing raw codes",
                                remote, code
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
                            message = Some(Message {
                                carrier: None,
                                duty_cycle: None,
                                raw,
                            });
                        }
                    }
                }
            } else {
            }
        }
    }

    message
}
