use super::{Code, Flags, RawCode, Remote};
use irp::{Irp, Message, Vartable};
use itertools::Itertools;
use log::{debug, warn};
use num_integer::Integer;

/// Encode the given codes into raw IR, ready for transmit. This has been validated
/// against the send output of lircd using all the remotes present in the
/// lirc-remotes database.
pub fn encode(
    lirc_remotes: &[Remote],
    remote: Option<&str>,
    code_name: &str,
    repeats: u64,
) -> Result<Message, String> {
    let mut message = Message::new();

    let remotes: Vec<&Remote> = lirc_remotes
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
            return Err(format!("remote {needle} not found"));
        } else {
            return Err(String::from("no remote found"));
        }
    }

    let remotes: Vec<(&Remote, usize)> = remotes
        .iter()
        .filter_map(|remote| {
            let count = remote
                .codes
                .iter()
                .filter(|code| code.name == code_name)
                .count()
                + remote
                    .raw_codes
                    .iter()
                    .filter(|code| code.name == code_name)
                    .count();

            if count > 0 {
                Some((*remote, count))
            } else {
                None
            }
        })
        .collect();

    if remotes.len() > 1 {
        warn!(
            "multiple remotes have a definition of code {}: {}",
            code_name,
            remotes
                .iter()
                .map(|remote| remote.0.name.as_str())
                .join(", ")
        );
    }

    if remotes.is_empty() {
        return Err(format!("code {code_name} not found"));
    }

    let (remote, count) = remotes[0];

    if count > 1 {
        warn!(
            "remote {} has {} definitions of the code {}",
            remote.name, count, code_name
        );
    }

    for raw_code in &remote.raw_codes {
        if raw_code.name == code_name {
            let encoded = remote.encode_raw(raw_code, repeats)?;

            message.extend(&encoded);

            break;
        }
    }

    for code in &remote.codes {
        if code.name == code_name {
            let encoded = remote.encode(code, repeats)?;

            message.extend(&encoded);

            break;
        }
    }

    Ok(message)
}

impl Remote {
    /// Encode code for this remote, with the given repeats
    pub fn encode(&self, code: &Code, repeats: u64) -> Result<Message, String> {
        let irp = self.encode_irp();

        debug!("irp for remote {}: {}", self.name, irp);

        let irp = Irp::parse(&irp).expect("should parse");

        let name = &code.name;

        if code.code.len() == 1 {
            // We can use irp repeats
            let mut vars = Vartable::new();

            debug!("encoding name={} code={}", name, code.code[0]);

            vars.set(String::from("CODE"), code.code[0] as i64);

            irp.encode_raw(vars, repeats)
        } else {
            // We need to repeat all the scancodes for each repeat
            let mut single = Message::new();

            for code in &code.code {
                let mut vars = Vartable::new();

                debug!("encoding name={} code={}", name, code);

                vars.set(String::from("CODE"), *code as i64);

                let m = irp.encode_raw(vars, 0)?;

                single.extend(&m);
            }

            let mut message = Message::new();

            for _ in 0..=repeats {
                message.extend(&single);
            }

            Ok(message)
        }
    }

    pub fn encode_once(&self, raw_code: &RawCode) -> Message {
        // remove trailing space
        let length = if raw_code.rawir.len().is_even() {
            raw_code.rawir.len() - 1
        } else {
            raw_code.rawir.len()
        };

        let mut raw = raw_code.rawir[..length].to_vec();

        let gap = if self.gap2 != 0 && self.gap2 < self.gap {
            self.gap2
        } else {
            self.gap
        };

        let mut gap = if self.flags.contains(Flags::CONST_LENGTH) {
            let total_length: u32 = raw.iter().sum();

            if (total_length as u64) < gap {
                gap as u32 - total_length
            } else {
                gap as u32
            }
        } else {
            gap as u32
        };

        if gap == 0 {
            gap = 20000;
        }

        raw.push(gap);

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

    /// Encode raw code for this remote, with the given repeats
    pub fn encode_raw(&self, raw_code: &RawCode, repeats: u64) -> Result<Message, String> {
        debug!("encoding name={}", raw_code.name);

        // remove trailing space
        let length = if raw_code.rawir.len().is_even() {
            raw_code.rawir.len() - 1
        } else {
            raw_code.rawir.len()
        };

        let mut raw = raw_code.rawir[..length].to_vec();

        let gap = if self.gap2 != 0 && self.gap2 < self.gap {
            self.gap2
        } else {
            self.gap
        };

        let mut gap = if self.flags.contains(Flags::CONST_LENGTH) {
            let total_length: u32 = raw.iter().sum();

            if (total_length as u64) < gap {
                gap as u32 - total_length
            } else {
                return Err(format!(
                    "const length gap is too short, gap is {} but signal is {}",
                    self.gap, total_length
                ));
            }
        } else {
            gap as u32
        };

        if gap != 0 {
            raw.push(gap);
        }

        if self.min_repeat != 0 || repeats != 0 {
            if gap == 0 {
                return Err("cannot repeat without a gap".into());
            }

            if self.repeat_gap != 0 {
                gap = self.repeat_gap as u32;
            }

            for _ in 0..(self.min_repeat + repeats) {
                raw.extend(&raw_code.rawir[..length]);
                raw.push(gap);
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

        Ok(Message {
            carrier,
            duty_cycle,
            raw,
        })
    }
}
