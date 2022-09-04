use super::{Irp, Message, Pronto, Vartable};
use std::{fmt, fmt::Write};

impl Pronto {
    /// Parse a pronto hex string
    pub fn parse(s: &str) -> Result<Pronto, String> {
        let mut p = Vec::new();

        for elem in s.split_whitespace() {
            if elem.len() != 4 {
                return Err(format!("pronto hex expects 4 hex digits, {} found", elem));
            }

            match u16::from_str_radix(elem, 16) {
                Ok(n) => p.push(n),
                Err(_) => {
                    return Err("pronto hex expects 4 hex digits".to_string());
                }
            }
        }

        if p.len() < 6 {
            return Err("pronto hex should be at least 6 numbers long".to_string());
        }

        let intro_length = p[2];
        let repeat_length = p[3];
        let frequency = p[1];

        if p.len() != (4 + 2 * (intro_length as usize + repeat_length as usize)) {
            return Err("inconsistent length".to_string());
        }

        // match short protocols
        #[allow(non_snake_case)]
        match p[0] {
            0x5000 => {
                if intro_length + repeat_length != 1 {
                    return Err("incorrect length".into());
                }
                return Ok(Pronto::Rc5 {
                    D: p[4] as u8,
                    F: p[5] as u8,
                });
            }
            0x5001 => {
                if intro_length + repeat_length != 2 {
                    return Err("incorrect length".into());
                }
                return Ok(Pronto::Rc5x {
                    D: p[4] as u8,
                    S: p[5] as u8,
                    F: p[6] as u8,
                });
            }
            0x6000 => {
                if intro_length + repeat_length != 1 {
                    return Err("incorrect length".into());
                }
                return Ok(Pronto::Rc6 {
                    D: p[4] as u8,
                    F: p[5] as u8,
                });
            }
            0x900a => {
                if intro_length + repeat_length != 1 {
                    return Err("incorrect length".into());
                }
                let D = (p[4] >> 8) as u8;
                let S = p[4] as u8;
                let F = (p[5] >> 8) as u8;
                let chk = p[5] as u8;

                if !chk != F {
                    return Err("checksum incorrect".into());
                }

                return Ok(Pronto::Nec1 { S, D, F });
            }
            _ => (),
        }

        fn to_pulses(frequency: u16, pulses: &[u16]) -> Vec<f64> {
            let pulse_time = frequency as f64 * 0.241_246f64;

            pulses.iter().map(|p| *p as f64 * pulse_time).collect()
        }

        let intro = to_pulses(frequency, &p[4..4 + (2 * intro_length as usize)]);
        let repeat = to_pulses(
            frequency,
            &p[4 + (2 * intro_length as usize)
                ..4 + 2 * (intro_length as usize + repeat_length as usize)],
        );

        let frequency = 1_000_000f64 / (frequency as f64 * 0.241_246f64);

        match p[0] {
            0 => Ok(Pronto::LearnedModulated {
                frequency,
                intro,
                repeat,
            }),
            0x100 => Ok(Pronto::LearnedUnmodulated {
                frequency,
                intro,
                repeat,
            }),
            _ => Err(format!("unsupport pronto type {:04x}", p[0])),
        }
    }

    /// Create raw IR with given number of repeats
    pub fn encode(&self, repeats: usize) -> Message {
        match self {
            Pronto::LearnedModulated { intro, repeat, .. }
            | Pronto::LearnedUnmodulated { intro, repeat, .. } => {
                let mut raw: Vec<u32> = Vec::with_capacity(intro.len() + repeats * repeat.len());

                for v in intro {
                    raw.push(*v as u32);
                }

                for _ in 0..repeats {
                    for v in repeat {
                        raw.push(*v as u32);
                    }
                }

                let carrier = match self {
                    Pronto::LearnedModulated { frequency, .. } => Some(*frequency as i64),
                    Pronto::LearnedUnmodulated { .. } => None,
                    _ => unreachable!(),
                };

                Message {
                    duty_cycle: None,
                    carrier,
                    raw,
                }
            }
            Pronto::Rc5 { D, F } => {
                let irp = Irp::parse("{36k,msb,889}<1,-1|-1,1>((1,~F:1:6,T:1,D:5,F:6,^114m)*,T=1-T)[D:0..31,F:0..127,T@:0..1=0]").unwrap();

                let mut vars = Vartable::new();
                vars.set("D".into(), *D as i64, 8);
                vars.set("F".into(), *F as i64, 8);

                irp.encode(vars, repeats as u64).unwrap()
            }
            Pronto::Rc5x { D, S, F } => {
                let irp = Irp::parse("{36k,msb,889}<1,-1|-1,1>((1,~S:1:6,T:1,D:5,-4,S:6,F:6,^114m)*,T=1-T)[D:0..31,S:0..127,F:0..63,T@:0..1=0]").unwrap();

                let mut vars = Vartable::new();
                vars.set("D".into(), *D as i64, 8);
                vars.set("S".into(), *S as i64, 8);
                vars.set("F".into(), *F as i64, 8);

                irp.encode(vars, repeats as u64).unwrap()
            }
            Pronto::Rc6 { D, F } => {
                let irp = Irp::parse("{36k,444,msb}<-1,1|1,-1>((6,-2,1:1,0:3,<-2,2|2,-2>(T:1),D:8,F:8,^107m)*,T=1-T)[D:0..255,F:0..255,T@:0..1=0]").unwrap();

                let mut vars = Vartable::new();
                vars.set("D".into(), *D as i64, 8);
                vars.set("F".into(), *F as i64, 8);

                irp.encode(vars, repeats as u64).unwrap()
            }
            Pronto::Nec1 { D, S, F } => {
                let irp = Irp::parse("{38.4k,564}<1,-1|1,-3>(16,-8,D:8,S:8,F:8,~F:8,1,-78,(16,-4,1,-173)*) [D:0..255,S:0..255=255-D,F:0..255]").unwrap();

                let mut vars = Vartable::new();
                vars.set("D".into(), *D as i64, 8);
                vars.set("S".into(), *S as i64, 8);
                vars.set("F".into(), *F as i64, 8);

                irp.encode(vars, repeats as u64).unwrap()
            }
        }
    }
}

impl fmt::Display for Pronto {
    /// Produce pronto hex string
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut codes = Vec::new();

        match self {
            Pronto::LearnedModulated {
                intro,
                repeat,
                frequency,
            }
            | Pronto::LearnedUnmodulated {
                intro,
                repeat,
                frequency,
            } => {
                // modulated or not
                if matches!(self, Pronto::LearnedModulated { .. }) {
                    codes.push(0);
                } else {
                    codes.push(0x100);
                }

                let frequency = 1_000_000f64 / (*frequency as f64 * 0.241_246f64);
                // carrier
                codes.push((frequency + 0.5) as usize);

                // lengths
                codes.push(intro.len() / 2);
                codes.push(repeat.len() / 2);

                fn to_units(frequency: f64, units: &[f64]) -> Vec<usize> {
                    let pulse_time = frequency as f64 * 0.241_246f64;

                    units.iter().map(|p| (*p / pulse_time) as usize).collect()
                }

                // the lengths
                codes.extend(to_units(frequency, intro));
                codes.extend(to_units(frequency, repeat));
            }
            Pronto::Rc5 { D, F } => {
                codes.extend([0x5000, 115, 0, 1, *D as usize, *F as usize]);
            }
            Pronto::Rc5x { D, S, F } => {
                codes.extend([0x5001, 115, 0, 2, *D as usize, *S as usize, *F as usize]);
            }
            Pronto::Rc6 { D, F } => {
                codes.extend([0x6000, 115, 0, 1, *D as usize, *F as usize]);
            }
            Pronto::Nec1 { D, S, F } => {
                let mut code1 = if *S > 0 { *S } else { !D } as usize;
                code1 |= (*D as usize) << 8;
                let mut code2 = !F as usize;
                code2 |= (*F as usize) << 8;

                codes.extend([0x900a, 108, 0, 1, code1, code2]);
            }
        }

        let mut s = String::new();

        for c in codes {
            write!(s, "{:04X} ", c).unwrap();
        }

        // return last space
        s.pop();

        write!(f, "{}", s)
    }
}

#[test]
fn long_test() {
    let pronto_hex_code = "0000 006C 0022 0002 015B 00AD 0016 0016 0016 0016 0016 0041 0016 0041 0016 0016 0016 0016 0016 0016 0016 0016 0016 0016 0016 0041 0016 0016 0016 0016 0016 0016 0016 0041 0016 0016 0016 0016 0016 0016 0016 0016 0016 0016 0016 0041 0016 0041 0016 0041 0016 0016 0016 0016 0016 0041 0016 0041 0016 0041 0016 0016 0016 0016 0016 0016 0016 0041 0016 0041 0016 06A4 015B 0057 0016 0E6C";
    let pronto = Pronto::parse(pronto_hex_code).expect("parse should succeed");

    assert_eq!(pronto.to_string(), pronto_hex_code);

    if let Pronto::LearnedModulated {
        frequency,
        intro,
        repeat,
    } = pronto
    {
        assert_eq!(frequency as u32, 38380);
        assert_eq!(
            intro.into_iter().map(|p| p as u32).collect::<Vec<u32>>(),
            vec![
                9040, 4507, 573, 573, 573, 573, 573, 1693, 573, 1693, 573, 573, 573, 573, 573, 573,
                573, 573, 573, 573, 573, 1693, 573, 573, 573, 573, 573, 573, 573, 1693, 573, 573,
                573, 573, 573, 573, 573, 573, 573, 573, 573, 1693, 573, 1693, 573, 1693, 573, 573,
                573, 573, 573, 1693, 573, 1693, 573, 1693, 573, 573, 573, 573, 573, 573, 573, 1693,
                573, 1693, 573, 44292
            ]
        );
        assert_eq!(
            repeat.into_iter().map(|p| p as u32).collect::<Vec<u32>>(),
            [9040, 2266, 573, 96193]
        );
    }

    assert_eq!(
        Pronto::parse("1000 006C 0000 0000 015B 00AD 0016"),
        Err("inconsistent length".to_string())
    );

    assert_eq!(
        Pronto::parse("1000 006C 0000 015B 00AD"),
        Err("pronto hex should be at least 6 numbers long".to_string())
    );
}

#[test]
fn short_test() {
    let pronto_hex_rc5 = "5000 0073 0000 0001 0001 0001";
    let pronto = Pronto::parse(pronto_hex_rc5).unwrap();

    assert_eq!(pronto.to_string(), pronto_hex_rc5);

    let raw = pronto.encode(1);
    assert_eq!(raw.print_rawir(), "+889 -889 +1778 -889 +889 -889 +889 -889 +889 -889 +889 -1778 +1778 -889 +889 -889 +889 -889 +889 -889 +889 -1778 +889 -89997");

    let pronto_hex_rc5x = "5001 0073 0000 0002 0001 0002 0003 0000";
    let pronto = Pronto::parse(pronto_hex_rc5x).unwrap();

    assert_eq!(pronto.to_string(), "5001 0073 0000 0002 0001 0002 0003");

    let raw = pronto.encode(1);
    assert_eq!(raw.print_rawir(), "+889 -889 +1778 -889 +889 -889 +889 -889 +889 -889 +889 -1778 +889 -3556 +889 -889 +889 -889 +889 -889 +889 -1778 +1778 -889 +889 -889 +889 -889 +889 -889 +889 -1778 +889 -889 +889 -75773");

    let pronto_hex_rc6 = "6000 0073 0000 0001 0001 0003";
    let pronto = Pronto::parse(pronto_hex_rc6).unwrap();

    assert_eq!(pronto.to_string(), pronto_hex_rc6);

    let raw = pronto.encode(1);
    assert_eq!(raw.print_rawir(), "+2664 -888 +444 -888 +444 -444 +444 -444 +444 -888 +888 -444 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +888 -888 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +888 -444 +444 -84356");

    let pronto_hex_nec1 = "900A 006C 0000 0001 0CF3 38C7";
    let pronto = Pronto::parse(pronto_hex_nec1).unwrap();

    assert_eq!(pronto.to_string(), pronto_hex_nec1);

    let raw = pronto.encode(1);
    assert_eq!(raw.print_rawir(), "+9024 -4512 +564 -564 +564 -564 +564 -1692 +564 -1692 +564 -564 +564 -564 +564 -564 +564 -564 +564 -1692 +564 -1692 +564 -564 +564 -564 +564 -1692 +564 -1692 +564 -1692 +564 -1692 +564 -564 +564 -564 +564 -564 +564 -1692 +564 -1692 +564 -1692 +564 -564 +564 -564 +564 -1692 +564 -1692 +564 -1692 +564 -564 +564 -564 +564 -564 +564 -1692 +564 -1692 +564 -43992 +9024 -2256 +564 -97572");
}
