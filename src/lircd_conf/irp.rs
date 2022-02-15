use super::{Flags, LircRemote};

impl LircRemote {
    /// Build an IRP representation for the remote. This can be used both for encoding
    /// and decoding.
    pub fn irp(&self) -> String {
        let mut irp = String::from("{");

        if self.frequency != 0 {
            irp.push_str(&format!("{}k,", self.frequency as f64 / 1000f64));
        }

        if self.duty_cycle != 0 {
            irp.push_str(&format!("{}%,", self.duty_cycle));
        }

        irp.push_str("msb}<");

        if self.flags.contains(Flags::XMP) {
            for i in 0..16 {
                irp.push_str(&format!(
                    "{},-{}|",
                    self.bit[0].0,
                    self.bit[0].1 + i * self.bit[1].1
                ));
            }
        } else {
            for (bit_no, (pulse, space)) in self.bit.iter().enumerate() {
                if *pulse == 0 && *space == 0 {
                    break;
                }

                if (self.flags.intersects(Flags::RC5 | Flags::RC6) && bit_no == 1)
                    || self.flags.contains(Flags::SPACE_FIRST)
                {
                    if *space > 0 {
                        irp.push_str(&format!("-{},", space))
                    }

                    if *pulse > 0 {
                        irp.push_str(&format!("{},", pulse))
                    }
                } else {
                    if *pulse > 0 {
                        irp.push_str(&format!("{},", pulse))
                    }

                    if *space > 0 {
                        irp.push_str(&format!("-{},", space))
                    }
                }

                irp.pop();
                irp.push('|');
            }
        }

        irp.pop();
        irp.push_str(">(");

        if self.header.0 != 0 && self.header.1 != 0 {
            irp.push_str(&format!("{},-{},", self.header.0, self.header.1));
        }

        if self.plead != 0 {
            irp.push_str(&format!("{},", self.plead));
        }

        if self.pre_data_bits != 0 {
            add_stream_with_rc6_mask(
                self,
                Stream::Constant(self.pre_data),
                self.pre_data_bits,
                self.rc6_mask >> (self.bits + self.post_data_bits),
                &mut irp,
            );

            if self.pre.0 != 0 && self.pre.1 != 0 {
                irp.push_str(&format!("{},-{},", self.pre.0, self.pre.1));
            }
        }

        add_stream_with_rc6_mask(
            self,
            Stream::Variable("CODE"),
            self.bits,
            self.rc6_mask >> self.post_data_bits,
            &mut irp,
        );

        if self.post_data_bits != 0 {
            add_stream_with_rc6_mask(
                self,
                Stream::Constant(self.post_data),
                self.post_data_bits,
                self.rc6_mask,
                &mut irp,
            );

            if self.post.0 != 0 && self.post.1 != 0 {
                irp.push_str(&format!("{},-{},", self.post.0, self.post.1));
            }
        }

        if self.ptrail != 0 {
            irp.push_str(&format!("{},", self.ptrail));
        }

        if self.foot.0 != 0 && self.foot.1 != 0 {
            irp.push_str(&format!("{},-{},", self.foot.0, self.foot.1));
        }

        if self.gap != 0 {
            irp.push_str(&format!("^{},", self.gap));
        }

        if self.repeat.0 != 0 && self.repeat.1 != 0 {
            irp.push_str(&format!("({},-{},", self.repeat.0, self.repeat.1));
            if self.ptrail != 0 {
                irp.push_str(&format!("{},", self.ptrail));
            }
            irp.pop();
            irp.push_str(")*)");
        } else {
            irp.pop();
            irp.push_str(")+");
        }

        irp.push_str(&format!(" [CODE:0..{}]", (1u64 << self.bits) - 1));

        irp
    }

    /// How many bits are there in the definition
    pub fn all_bits(&self) -> u64 {
        self.pre_data_bits + self.bits + self.post_data_bits
    }
}

#[derive(Clone, Copy)]
enum Stream<'a> {
    Constant(u64),
    Variable(&'a str),
}

fn add_stream_with_rc6_mask(
    remote: &LircRemote,
    stream: Stream,
    bits: u64,
    mask: u64,
    irp: &mut String,
) {
    let mask = mask & gen_mask(bits);

    if mask == 0 {
        add_bits(irp, stream, bits, 0);
        return;
    }

    let leading_bits = bits - highest_bit(mask) - 1;

    if leading_bits > 0 {
        add_bits(irp, stream, leading_bits, bits - leading_bits);
    }

    irp.push_str(&format!(
        "<{},-{}|-{},{}>(",
        remote.bit[0].0 * 2,
        remote.bit[0].1 * 2,
        remote.bit[1].1 * 2,
        remote.bit[1].0 * 2
    ));

    let trailing_bits = mask.trailing_zeros() as u64;

    add_bits(irp, stream, mask.count_ones() as u64, leading_bits);

    irp.pop();
    irp.push_str("),");

    if trailing_bits > 0 {
        add_bits(irp, stream, trailing_bits, 0);
    }
}

fn add_bits(irp: &mut String, stream: Stream, bits: u64, offset: u64) {
    match stream {
        Stream::Constant(v) => {
            let v = (v >> offset) & gen_mask(bits);

            if v <= 9 {
                irp.push_str(&format!("{}:{},", v, bits));
            } else {
                irp.push_str(&format!("0x{:x}:{},", v, bits));
            }
        }
        Stream::Variable(v) if offset == 0 => {
            irp.push_str(&format!("{}:{},", v, bits));
        }
        Stream::Variable(v) => {
            irp.push_str(&format!("{}:{}:{},", v, bits, offset));
        }
    }
}

fn gen_mask(v: u64) -> u64 {
    (1u64 << v) - 1
}

fn highest_bit(v: u64) -> u64 {
    63u64 - v.leading_zeros() as u64
}
