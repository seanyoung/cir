use super::{Flags, LircRemote};
use crate::log::Log;
use irp::Message;

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

            if lirc_remote.flags.contains(Flags::RAW_CODES) {
                for raw_code in &lirc_remote.raw_codes {
                    if raw_code.name == *code {
                        if let Some(message) = &mut message {
                            message.raw.extend_from_slice(&raw_code.rawir);
                        } else {
                            message = Some(Message {
                                carrier: None,
                                duty_cycle: None,
                                raw: raw_code.rawir.clone(),
                            });
                        }
                    }
                }
            }
        }
    }

    message
}
