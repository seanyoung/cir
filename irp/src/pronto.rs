use crate::Message;
use std::fmt;

#[derive(Debug, PartialEq)]
pub enum Pronto {
    LearnedUnmodulated {
        frequency: f64,
        intro: Vec<f64>,
        repeat: Vec<f64>,
    },
    LearnedModulated {
        frequency: f64,
        intro: Vec<f64>,
        repeat: Vec<f64>,
    },
}

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
        let raw = match self {
            Pronto::LearnedModulated { intro, repeat, .. }
            | Pronto::LearnedUnmodulated { intro, repeat, .. } => {
                let mut res: Vec<u32> = Vec::with_capacity(intro.len() + repeats * repeat.len());

                for v in intro {
                    res.push(*v as u32);
                }

                for _ in 0..repeats {
                    for v in repeat {
                        res.push(*v as u32);
                    }
                }

                res
            }
        };

        let carrier = match self {
            Pronto::LearnedModulated { frequency, .. } => Some(*frequency as i64),
            Pronto::LearnedUnmodulated { .. } => None,
        };

        Message {
            duty_cycle: None,
            carrier,
            raw,
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
        }

        let mut s = String::new();

        for c in codes {
            s.push_str(&format!("{:04X} ", c));
        }

        // return last space
        s.pop();

        write!(f, "{}", s)
    }
}

#[test]
fn parse_test() {
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
