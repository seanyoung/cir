use super::{Keymap, LinuxProtocol};
use irp::{Irp, Message, Vartable};

impl Keymap {
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

        let irp = Irp::parse(irp)?;

        let mut vars = Vartable::new();

        let mut remaining_bits = 64;
        let mut scancode = scancode;

        for p in irp.parameters.iter().rev() {
            if p.name == "T" {
                continue;
            }
            let bits = p.max.ilog2() + 1;
            if bits > remaining_bits {
                return Err("too many parameters for 64 bit scancode".into());
            }
            vars.set(p.name.clone(), (scancode & gen_mask(bits)) as i64);

            remaining_bits -= bits;
            scancode >>= bits;
        }

        irp.encode_raw(vars, repeats)
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
