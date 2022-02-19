use super::{Flags, LircCode, LircRawCode, LircRemote};
use crate::log::Log;
use irp::{Irp, Message, Vartable};
use num_integer::Integer;

pub fn encode(
    lirc_remotes: &[LircRemote],
    remote: Option<&str>,
    codes: &[&str],
    repeats: u64,
    log: &Log,
) -> Result<Message, String> {
    let mut message = Message::new();
    let mut remote_found = false;

    for send_code in codes {
        let mut code_found = false;

        for lirc_remote in lirc_remotes {
            if let Some(remote) = remote {
                if lirc_remote.name != remote {
                    continue;
                }
            }

            remote_found = true;

            for raw_code in &lirc_remote.raw_codes {
                if raw_code.name == *send_code {
                    if code_found {
                        log.warning(&format!("multiple definitions of code {} found", send_code));
                        break;
                    }

                    let encoded = lirc_remote.encode_raw(raw_code, repeats);

                    message.extend(&encoded);

                    code_found = true;
                }
            }

            for code in &lirc_remote.codes {
                if code.name == *send_code {
                    if code_found {
                        log.warning(&format!("multiple definitions of code {} found", send_code));
                        break;
                    }

                    let encoded = lirc_remote.encode(code, repeats as i64, log);

                    message.extend(&encoded);

                    code_found = true;
                }
            }
        }

        if remote_found && !code_found {
            return Err(format!("code {} not found", send_code));
        }
    }

    if !message.raw.is_empty() {
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

impl LircRemote {
    /// Encode code for this remote, with the given repeats
    pub fn encode(&self, code: &LircCode, repeats: i64, log: &Log) -> Message {
        let irp = self.irp();

        log.info(&format!("irp for remote {}: {}", self.name, irp));

        let mut message = Message::new();

        for code in &code.code {
            let mut vars = Vartable::new();

            vars.set(String::from("CODE"), *code as i64, 32);
            let irp = Irp::parse(&irp).expect("should parse");

            let m = irp.encode(vars, repeats).expect("encode should succeed");

            message.extend(&m);
        }

        message
    }

    /// Encode raw code for this remote, with the given repeats
    pub fn encode_raw(&self, raw_code: &LircRawCode, repeats: u64) -> Message {
        // remove trailing space
        let length = if raw_code.rawir.len().is_even() {
            raw_code.rawir.len() - 1
        } else {
            raw_code.rawir.len()
        };

        let mut raw = raw_code.rawir[..length].to_vec();

        let space = if self.gap == 0 {
            20000
        } else if self.flags.contains(Flags::CONST_LENGTH) {
            let total_length: u32 = raw.iter().sum();

            self.gap as u32 - total_length
        } else {
            self.gap as u32
        };

        raw.push(space);

        if self.min_repeat != 0 || repeats != 0 {
            for _ in 0..(self.min_repeat + repeats) {
                raw.extend(&raw_code.rawir[..length]);
                raw.push(space);
            }
        }

        let carrier = if self.frequency != 0 {
            Some(self.frequency as i64)
        } else {
            None
        };

        let duty_cycle = if self.duty_cycle != 0 {
            Some(self.duty_cycle as u8)
        } else {
            None
        };

        Message {
            carrier,
            duty_cycle,
            raw,
        }
    }
}
