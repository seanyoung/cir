use super::{Keymap, LinuxProtocol, Raw};
use irp::{Irp, Message, Vartable};
use itertools::Itertools;

/// Encode the given codes into raw IR, ready for transmit. This has been validated
/// against the send output of lircd using all the remotes present in the
/// lirc-remotes database.
pub fn encode(
    keymaps: &[Keymap],
    remote: Option<&str>,
    codes: &[&str],
    repeats: u64,
) -> Result<Message, String> {
    let mut message = Message::new();

    let remotes: Vec<&Keymap> = keymaps
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

    for encode_code in codes {
        let remotes: Vec<(&Keymap, usize)> = remotes
            .iter()
            .filter_map(|remote| {
                let count = remote
                    .scancodes
                    .values()
                    .filter(|code| code == encode_code)
                    .count()
                    + remote
                        .raw
                        .iter()
                        .filter(|code| code.keycode == *encode_code)
                        .count();

                if count > 0 {
                    Some((*remote, count))
                } else {
                    None
                }
            })
            .collect();

        if remotes.len() > 1 {
            log::warn!(
                "multiple remotes have a definition of code {}: {}",
                encode_code,
                remotes
                    .iter()
                    .map(|remote| remote.0.name.as_str())
                    .join(", ")
            );
        }

        if remotes.is_empty() {
            return Err(format!("code {encode_code} not found"));
        }

        let (remote, count) = remotes[0];

        if count > 1 {
            log::warn!(
                "remote {} has {} definitions of the code {}",
                remote.name,
                count,
                *encode_code
            );
        }

        for raw_code in &remote.raw {
            if raw_code.keycode == *encode_code {
                let encoded = remote.encode_raw(raw_code, repeats);

                message.extend(&encoded);

                break;
            }
        }

        for (scancode, keycode) in &remote.scancodes {
            if keycode == *encode_code {
                let encoded = remote.encode_scancode(*scancode, repeats)?;

                message.extend(&encoded);

                break;
            }
        }
    }

    Ok(message)
}

impl Keymap {
    pub fn encode(&self, code: &str, repeats: u64) -> Result<Message, String> {
        if let Some((scancode, _)) = self.scancodes.iter().find(|(_, v)| *v == code) {
            self.encode_scancode(*scancode, repeats)
        } else if let Some(raw) = self.raw.iter().find(|e| e.keycode == code) {
            Ok(self.encode_raw(raw, repeats))
        } else {
            Err(format!("{code} not found"))
        }
    }

    pub fn encode_scancode(&self, scancode: u64, repeats: u64) -> Result<Message, String> {
        let irp = if let Some(i) = &self.irp {
            i.as_str()
        } else {
            let protocol = self.variant.as_ref().unwrap_or(&self.protocol);

            if let Some(p) = LinuxProtocol::find_like(protocol) {
                if let Some(i) = p.irp {
                    i
                } else {
                    return Err(format!("unable to encode {protocol}"));
                }
            } else {
                return Err(format!("unknown protocol {protocol}"));
            }
        };

        log::debug!("using irp for encoding: {irp}");

        let irp = Irp::parse(irp)?;

        let mut vars = Vartable::new();

        let mut remaining_bits = 64;
        let mut scancode_bits = scancode;

        for p in irp.parameters.iter().rev() {
            if p.name == "T" {
                continue;
            }
            let bits = p.max.ilog2() + 1;
            if bits > remaining_bits {
                return Err("too many parameters for 64 bit scancode".into());
            }
            vars.set(p.name.clone(), (scancode_bits & gen_mask(bits)) as i64);

            remaining_bits -= bits;
            scancode_bits >>= bits;
        }

        if scancode_bits > 0 {
            log::warn!("IRP did not use all bits in scancode {scancode:#x}");
        }

        irp.encode_raw(vars, repeats)
    }

    pub fn encode_raw(&self, raw: &Raw, repeats: u64) -> Message {
        if let Some(pronto) = &raw.pronto {
            return pronto.encode(repeats as usize);
        }

        let e = raw.raw.as_ref().unwrap();

        let mut m = e.clone();

        if repeats > 0 {
            let rep = raw.repeat.as_ref().unwrap_or(e);

            for _ in 0..repeats {
                m.extend(rep);
            }
        }

        m
    }
}

fn gen_mask(v: u32) -> u64 {
    if v < 64 {
        (1u64 << v) - 1
    } else {
        u64::MAX
    }
}

#[test]
fn ilog2() {
    use rand::RngCore;

    let mut rng = rand::thread_rng();

    for _ in 0..10000 {
        let v = rng.next_u64();

        if v == 0 {
            continue;
        }

        let i = v.ilog2() + 1;

        let leading = 64 - v.leading_zeros();

        assert_eq!(i, leading);
    }
}
