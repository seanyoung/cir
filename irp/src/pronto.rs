use crate::Message;

#[derive(Debug, PartialEq)]
pub enum Pronto {
    LearnedUnmodulated {
        intro: Vec<f64>,
        repeat: Vec<f64>,
    },
    LearnedModulated {
        carrier: f64,
        intro: Vec<f64>,
        repeat: Vec<f64>,
    },
}

fn to_pulses(frequency: u16, pulses: &[u16]) -> Vec<f64> {
    let pulse_time = frequency as f64 * 0.241_246f64;

    pulses.iter().map(|p| *p as f64 * pulse_time).collect()
}

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

    let intro = to_pulses(frequency, &p[4..4 + (2 * intro_length as usize)]);
    let repeat = to_pulses(
        frequency,
        &p[4 + (2 * intro_length as usize)
            ..4 + 2 * (intro_length as usize + repeat_length as usize)],
    );

    match p[0] {
        0 => Ok(Pronto::LearnedModulated {
            carrier: 1_000_000f64 / (frequency as f64 * 0.241_246f64),
            intro,
            repeat,
        }),
        0x100 => Ok(Pronto::LearnedUnmodulated { intro, repeat }),
        _ => Err(format!("unsupport pronto type {:04x}", p[0])),
    }
}

#[test]
fn parse_test() {
    let pronto = parse("0000 006C 0022 0002 015B 00AD 0016 0016 0016 0016 0016 0041 0016 0041 0016 0016 0016 0016 0016 0016 0016 0016 0016 0016 0016 0041 0016 0016 0016 0016 0016 0016 0016 0041 0016 0016 0016 0016 0016 0016 0016 0016 0016 0016 0016 0041 0016 0041 0016 0041 0016 0016 0016 0016 0016 0041 0016 0041 0016 0041 0016 0016 0016 0016 0016 0016 0016 0041 0016 0041 0016 06A4 015B 0057 0016 0E6C");

    if let Ok(Pronto::LearnedModulated {
        carrier,
        intro,
        repeat,
    }) = pronto
    {
        assert_eq!(carrier as u32, 38380);
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
        parse("1000 006C 0000 0000 015B 00AD 0016"),
        Err("inconsistent length".to_string())
    );

    assert_eq!(
        parse("1000 006C 0000 015B 00AD"),
        Err("pronto hex should be at least 6 numbers long".to_string())
    );
}

impl Pronto {
    pub fn encode(&self, repeats: usize) -> Message {
        let raw = match self {
            Pronto::LearnedModulated { intro, repeat, .. }
            | Pronto::LearnedUnmodulated { intro, repeat } => {
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
            Pronto::LearnedModulated { carrier, .. } => Some(*carrier as i64),
            Pronto::LearnedUnmodulated { .. } => None,
        };

        Message {
            duty_cycle: None,
            carrier,
            raw,
        }
    }
}
