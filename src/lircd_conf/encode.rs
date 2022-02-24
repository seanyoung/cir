use super::{Flags, LircCode, LircRawCode, LircRemote};
use crate::log::Log;
use irp::{Irp, Message, Vartable};
use itertools::Itertools;
use num_integer::Integer;

pub fn encode(
    lirc_remotes: &[LircRemote],
    remote: Option<&str>,
    codes: &[&str],
    repeats: u64,
    log: &Log,
) -> Result<Message, String> {
    let mut message = Message::new();

    let remotes: Vec<&LircRemote> = lirc_remotes
        .iter()
        .filter(|r| {
            if let Some(needle) = remote {
                needle == r.name
            } else {
                true
            }
        })
        .collect();

    if remotes.is_empty() {
        if let Some(needle) = remote {
            return Err(format!("remote {} not found", needle));
        } else {
            return Err(String::from("no remote found"));
        }
    }

    for send_code in codes {
        let remotes: Vec<(&LircRemote, usize)> = remotes
            .iter()
            .filter_map(|r| {
                let count = r
                    .codes
                    .iter()
                    .filter(|code| code.name == *send_code)
                    .count()
                    + r.raw_codes
                        .iter()
                        .filter(|code| code.name == *send_code)
                        .count();

                if count > 0 {
                    Some((*r, count))
                } else {
                    None
                }
            })
            .collect();

        if remotes.len() > 1 {
            log.warning(&format!(
                "multiple remotes have a definition of code {}: {}",
                send_code,
                remotes
                    .iter()
                    .map(|remote| remote.0.name.as_str())
                    .join(", ")
            ));
        }

        if remotes.is_empty() {
            return Err(format!("code {} not found", send_code));
        }

        let (lirc_remote, count) = remotes[0];

        if count > 1 {
            log.warning(&format!(
                "remote {} has {} definitions of the code {}",
                lirc_remote.name, count, *send_code
            ));
        }

        for raw_code in &lirc_remote.raw_codes {
            if raw_code.name == *send_code {
                let encoded = lirc_remote.encode_raw(raw_code, repeats);

                message.extend(&encoded);

                break;
            }
        }

        for code in &lirc_remote.codes {
            if code.name == *send_code {
                let encoded = lirc_remote.encode(code, repeats, log);

                message.extend(&encoded);

                break;
            }
        }
    }

    Ok(message)
}

impl LircRemote {
    /// Encode code for this remote, with the given repeats
    pub fn encode(&self, code: &LircCode, repeats: u64, log: &Log) -> Message {
        let irp = self.irp();

        log.info(&format!("irp for remote {}: {}", self.name, irp));

        let irp = Irp::parse(&irp).expect("should parse");

        let mut message = Message::new();

        for code in &code.code {
            let mut vars = Vartable::new();

            vars.set(String::from("CODE"), *code as i64, 32);

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
            // TODO: is this right?
            20000
        } else if self.flags.contains(Flags::CONST_LENGTH) {
            let total_length: u32 = raw.iter().sum();

            self.gap as u32 - total_length
        } else {
            self.gap as u32
        };

        raw.push(space);

        // what about repeat_gap?
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
