use super::{Flags, Remote};
use std::fmt::Write;

impl Remote {
    /// Build an IRP representation for the remote. This can be used both for encoding
    /// and decoding.
    pub fn irp(&self) -> String {
        let builder = Builder::new(self);

        builder.build()
    }

    /// How many bits are there in the definition
    pub fn all_bits(&self) -> u64 {
        self.pre_data_bits + self.bits + self.post_data_bits
    }
}

struct Builder<'a> {
    remote: &'a Remote,
    irp: String,
}

#[derive(Clone, Copy)]
enum Stream<'a> {
    Constant(u64),
    Variable(&'a str),
    Toggle,
}

impl<'a> Builder<'a> {
    fn new(remote: &'a Remote) -> Self {
        Builder {
            remote,
            irp: String::new(),
        }
    }

    fn build(mut self) -> String {
        self.irp = "{".into();

        if self.remote.frequency != 0 {
            write!(
                &mut self.irp,
                "{}k,",
                self.remote.frequency as f64 / 1000f64
            )
            .unwrap();
        }

        if self.remote.duty_cycle != 0 {
            write!(&mut self.irp, "{}%,", self.remote.duty_cycle).unwrap();
        }

        self.irp.push_str("msb}<");

        if self.remote.flags.contains(Flags::BO) {
            write!(
                &mut self.irp,
                "{},-zeroGap,zeroGap={},oneGap={}|{},-oneGap,zeroGap={},oneGap={}|",
                self.remote.bit[1].0,
                self.remote.bit[2].1,
                self.remote.bit[3].1,
                self.remote.bit[2].0,
                self.remote.bit[1].1,
                self.remote.bit[2].1,
            )
            .unwrap();
        } else if self.remote.flags.contains(Flags::GRUNDIG) {
            write!(
                &mut self.irp,
                "-{},{}|\
                -{},{},-{},{}|\
                -{},{},-{},{}|\
                -{},{},-{},{}|",
                // bit 0
                self.remote.bit[3].1,
                self.remote.bit[3].0,
                // bit 1
                self.remote.bit[2].1,
                self.remote.bit[2].0,
                self.remote.bit[0].1,
                self.remote.bit[0].0,
                // bit 2
                self.remote.bit[1].1,
                self.remote.bit[1].0,
                self.remote.bit[1].1,
                self.remote.bit[1].0,
                // bit 3
                self.remote.bit[0].1,
                self.remote.bit[0].0,
                self.remote.bit[2].1,
                self.remote.bit[2].0,
            )
            .unwrap();
        } else if self.remote.flags.contains(Flags::XMP) {
            for i in 0..16 {
                write!(
                    &mut self.irp,
                    "{},-{}|",
                    self.remote.bit[0].0,
                    self.remote.bit[0].1 + i * self.remote.bit[1].1
                )
                .unwrap();
            }
        } else {
            for (bit_no, (pulse, space)) in self.remote.bit.iter().enumerate() {
                if *pulse == 0 && *space == 0 {
                    break;
                }

                if (self.remote.flags.intersects(Flags::RC5 | Flags::RC6) && bit_no == 1)
                    || self.remote.flags.contains(Flags::SPACE_FIRST)
                {
                    if *space > 0 {
                        write!(&mut self.irp, "-{space},").unwrap();
                    }

                    if *pulse > 0 {
                        write!(&mut self.irp, "{pulse},").unwrap();
                    }
                } else {
                    if *pulse > 0 {
                        write!(&mut self.irp, "{pulse},").unwrap();
                    }

                    if *space > 0 {
                        write!(&mut self.irp, "-{space},").unwrap();
                    }
                }

                self.irp.pop();
                self.irp.push('|');
            }
        }

        self.irp.pop();
        self.irp.push_str(">(");

        self.add_irp_body(false);

        if self.remote.repeat.0 != 0 && self.remote.repeat.1 != 0 {
            self.irp.push('(');
            if self.remote.flags.contains(Flags::REPEAT_HEADER)
                && self.remote.header.0 != 0
                && self.remote.header.1 != 0
            {
                write!(
                    &mut self.irp,
                    "{},-{},",
                    self.remote.header.0, self.remote.header.1
                )
                .unwrap();
            }
            if self.remote.plead != 0 {
                write!(&mut self.irp, "{},", self.remote.plead).unwrap();
            }
            write!(
                &mut self.irp,
                "{},-{},",
                self.remote.repeat.0, self.remote.repeat.1
            )
            .unwrap();
            if self.remote.ptrail != 0 {
                write!(&mut self.irp, "{},", self.remote.ptrail).unwrap();
            }
            self.add_gap(true);

            self.irp.pop();
            match self.remote.min_repeat {
                0 => self.irp.push_str(")*)"),
                1 => self.irp.push_str(")+)"),
                _ => write!(&mut self.irp, "){}+)", self.remote.min_repeat).unwrap(),
            }
        } else if self
            .remote
            .flags
            .intersects(Flags::NO_HEAD_REP | Flags::NO_FOOT_REP)
        {
            self.irp.push('(');

            self.add_irp_body(true);

            self.irp.pop();
            match self.remote.min_repeat {
                0 => self.irp.push_str(")*)"),
                1 => self.irp.push_str(")+)"),
                _ => write!(&mut self.irp, "){}+)", self.remote.min_repeat).unwrap(),
            }
        } else {
            self.irp.pop();
            if self.remote.min_repeat > 0 {
                write!(&mut self.irp, "){}+", self.remote.min_repeat + 1).unwrap();
            } else {
                self.irp.push_str(")+");
            }
        }

        if self.toggle_post_data() || self.remote.flags.contains(Flags::BO) {
            self.irp.push('{');

            if self.toggle_post_data() {
                write!(&mut self.irp, "POST=0x{:x},", self.remote.post_data).unwrap();
            }

            if self.remote.flags.contains(Flags::BO) {
                write!(
                    &mut self.irp,
                    "zeroGap={},oneGap={},",
                    self.remote.bit[1].1, self.remote.bit[3].1
                )
                .unwrap();
            }

            self.irp.pop();
            self.irp.push('}');
        }

        write!(&mut self.irp, " [CODE:0..{}", gen_mask(self.remote.bits)).unwrap();

        if self.remote.toggle_bit_mask.count_ones() == 1 {
            self.irp.push_str(",T@:0..1=0");
        }

        self.irp.push(']');

        self.irp
    }

    fn add_irp_body(&mut self, repeat: bool) {
        let supress_header = repeat && self.remote.flags.contains(Flags::NO_HEAD_REP);
        let supress_footer = repeat && self.remote.flags.contains(Flags::NO_FOOT_REP);

        if self.remote.flags.contains(Flags::BO) {
            write!(
                &mut self.irp,
                "{},-{},{},-{},",
                self.remote.bit[1].0,
                self.remote.bit[1].1,
                self.remote.bit[1].0,
                self.remote.bit[1].1
            )
            .unwrap();
        }

        if !supress_header && self.remote.header.0 != 0 && self.remote.header.1 != 0 {
            write!(
                &mut self.irp,
                "{},-{},",
                self.remote.header.0, self.remote.header.1
            )
            .unwrap();
        }

        if self.remote.plead != 0 {
            write!(&mut self.irp, "{},", self.remote.plead).unwrap();
        }

        let toggle_bit_mask = if self.remote.toggle_bit_mask.count_ones() == 1 {
            self.remote.toggle_bit_mask
        } else {
            0
        };

        if self.remote.pre_data_bits != 0 {
            self.add_bit_stream(
                Stream::Constant(self.remote.pre_data),
                self.remote.pre_data_bits,
                toggle_bit_mask >> (self.remote.bits + self.remote.post_data_bits),
                self.remote.rc6_mask >> (self.remote.bits + self.remote.post_data_bits),
            );

            if self.remote.pre.0 != 0 && self.remote.pre.1 != 0 {
                write!(
                    &mut self.irp,
                    "{},-{},",
                    self.remote.pre.0, self.remote.pre.1
                )
                .unwrap();
            }
        }

        self.add_bit_stream(
            Stream::Variable("CODE"),
            self.remote.bits,
            toggle_bit_mask >> self.remote.post_data_bits,
            self.remote.rc6_mask >> self.remote.post_data_bits,
        );

        if self.remote.post_data_bits != 0 {
            let stream = if self.toggle_post_data() {
                Stream::Variable("POST")
            } else {
                Stream::Constant(self.remote.post_data)
            };

            self.add_bit_stream(
                stream,
                self.remote.post_data_bits,
                toggle_bit_mask,
                self.remote.rc6_mask,
            );

            if self.remote.post.0 != 0 && self.remote.post.1 != 0 {
                write!(
                    &mut self.irp,
                    "{},-{},",
                    self.remote.post.0, self.remote.post.1
                )
                .unwrap();
            }
        }

        if !supress_footer && self.remote.foot.0 != 0 && self.remote.foot.1 != 0 {
            write!(
                &mut self.irp,
                "{},-{},",
                self.remote.foot.0, self.remote.foot.1
            )
            .unwrap();
        }

        if self.remote.ptrail != 0 {
            write!(&mut self.irp, "{},", self.remote.ptrail).unwrap();
        }

        self.add_gap(repeat);

        if self.remote.toggle_mask != 0 {
            write!(
                &mut self.irp,
                "CODE=CODE^0x{:x},",
                self.remote.toggle_mask >> self.remote.post_data_bits
            )
            .unwrap();
        }

        if self.remote.toggle_bit_mask.count_ones() > 1 {
            write!(
                &mut self.irp,
                "CODE=CODE^0x{:x},",
                self.remote.toggle_bit_mask >> self.remote.post_data_bits
            )
            .unwrap();
        }

        if self.toggle_post_data() {
            write!(
                &mut self.irp,
                "POST=POST^0x{:x},",
                self.remote.toggle_mask & gen_mask(self.remote.post_data_bits)
            )
            .unwrap();
        }
    }

    fn add_gap(&mut self, repeat: bool) {
        if self.remote.gap != 0 || (self.remote.repeat_gap != 0 && repeat) {
            let gap = if repeat && self.remote.repeat_gap != 0 {
                self.remote.repeat_gap
            } else if !repeat
                && self
                    .remote
                    .flags
                    .contains(Flags::NO_HEAD_REP | Flags::CONST_LENGTH)
            {
                self.remote.gap + self.remote.header.0 + self.remote.header.1
            } else {
                self.remote.gap
            };

            self.irp
                .push_str(if self.remote.flags.contains(Flags::CONST_LENGTH) {
                    "^"
                } else {
                    "-"
                });

            if self.remote.gap % 1000 == 0 {
                write!(&mut self.irp, "{}m,", gap / 1000).unwrap();
            } else {
                write!(&mut self.irp, "{gap},").unwrap();
            }
        }
    }

    fn toggle_post_data(&self) -> bool {
        self.remote.toggle_mask != 0
            && (self.remote.toggle_mask & gen_mask(self.remote.post_data_bits)) != 0
    }

    fn add_bit_stream(&mut self, stream: Stream, bits: u64, toggle_mask: u64, rc6_mask: u64) {
        let mut edges = mask_edges(rc6_mask, bits);
        edges.extend_from_slice(&mask_edges(toggle_mask, bits));
        edges.sort_by(|a, b| b.partial_cmp(a).unwrap());
        edges.dedup();
        edges.push(0);

        let mut highest_bit = bits;

        for bit in edges {
            let is_toggle = (toggle_mask & (1 << bit)) != 0;
            let is_rc6 = (rc6_mask & (1 << bit)) != 0;

            if is_rc6 {
                write!(
                    &mut self.irp,
                    "<{},-{}|-{},{}>(",
                    self.remote.bit[0].0 * 2,
                    self.remote.bit[0].1 * 2,
                    self.remote.bit[1].1 * 2,
                    self.remote.bit[1].0 * 2
                )
                .unwrap();
            }

            let stream = if is_toggle { Stream::Toggle } else { stream };

            let bit_count = highest_bit - bit;
            let offset = bit;

            match stream {
                Stream::Constant(v) => {
                    let v = (v >> offset) & gen_mask(bit_count);

                    if v <= 9 {
                        write!(&mut self.irp, "{v}:{bit_count},").unwrap();
                    } else {
                        write!(&mut self.irp, "0x{v:x}:{bit_count},").unwrap();
                    }
                }
                Stream::Variable(v) if offset == 0 => {
                    write!(&mut self.irp, "{v}:{bit_count},").unwrap();
                }
                Stream::Variable(v) => {
                    write!(&mut self.irp, "{v}:{bit_count}:{offset},").unwrap();
                }
                Stream::Toggle => {
                    assert_eq!(bit_count, 1);

                    self.irp.push_str("T:1,");
                }
            }

            if is_rc6 {
                self.irp.pop();
                self.irp.push_str("),");
            }

            highest_bit = bit;
        }
    }
}

fn gen_mask(v: u64) -> u64 {
    (1u64 << v) - 1
}

fn highest_bit(v: u64) -> u64 {
    63u64 - v.leading_zeros() as u64
}

/// For given bitmask, produce an array of edges of bit numbers where the mask changes
fn mask_edges(mask: u64, bits: u64) -> Vec<u64> {
    let mut v = mask & gen_mask(bits);
    let mut edges = Vec::new();

    while v != 0 {
        let bit = highest_bit(v) + 1;

        edges.push(bit);

        v = !v & gen_mask(bit);
    }

    edges
}

#[test]
fn test_edges() {
    assert_eq!(mask_edges(0, 32), vec![]);
    assert_eq!(mask_edges(1, 32), vec![1]);
    assert_eq!(mask_edges(2, 32), vec![2, 1]);
    assert_eq!(mask_edges(8, 32), vec![4, 3]);
    assert_eq!(mask_edges(0xf0, 32), vec![8, 4]);
}
