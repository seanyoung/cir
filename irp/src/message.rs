use super::{rawir, Message};

impl Message {
    /// Create an empty packet
    pub fn new() -> Self {
        Message::default()
    }

    /// Concatenate to packets
    pub fn extend(&mut self, other: &Message) {
        if self.carrier.is_none() {
            self.carrier = other.carrier;
        }

        if self.duty_cycle.is_none() {
            self.duty_cycle = other.duty_cycle;
        }

        self.raw.extend_from_slice(&other.raw);
    }

    /// Do we have a trailing gap
    pub fn has_trailing_gap(&self) -> bool {
        let len = self.raw.len();

        len > 0 && (len % 2) == 0
    }

    /// Remove any trailing gap
    pub fn remove_trailing_gap(&mut self) {
        if self.has_trailing_gap() {
            self.raw.pop();
        }
    }

    /// Print the flash and gap information as an raw ir string
    pub fn print_rawir(&self) -> String {
        rawir::print_to_string(&self.raw)
    }

    /// Parse a raw IR string of the form `+9000 -45000 +2250`
    pub fn parse(s: &str) -> Result<Self, String> {
        let mut raw = Vec::new();
        let mut flash = true;

        for e in s.split(|c: char| c.is_whitespace() || c == ',') {
            if e.is_empty() {
                continue;
            }

            let mut chars = e.chars().peekable();

            match chars.peek() {
                Some('+') => {
                    if !flash {
                        return Err("unexpected ‘+’ encountered".to_string());
                    }
                    chars.next();
                }
                Some('-') => {
                    if flash {
                        return Err("unexpected ‘-’ encountered".to_string());
                    }
                    chars.next();
                }
                Some(ch) if !ch.is_numeric() => {
                    return Err(format!("unexpected ‘{}’ encountered", ch));
                }
                _ => (),
            }

            let v = chars.collect::<String>();

            let v = v.parse().map_err(|_| format!("invalid number ‘{}’", v))?;

            if v == 0 {
                return Err("nonsensical 0 length".to_string());
            }

            raw.push(v);

            flash = !flash;
        }

        if raw.is_empty() {
            return Err("missing length".to_string());
        }

        Ok(Message {
            raw,
            carrier: None,
            duty_cycle: None,
        })
    }

    /// Parse pulse/space text. This format is produces by lirc's mode2 tool.
    /// Some lirc drivers sometimes produce consecutive pulses or spaces, rather
    /// than alternating. These are automatically folded into one.
    ///
    /// The return value is the line number and the error string.
    pub fn parse_mode2(s: &str) -> Result<Message, (usize, String)> {
        let mut res = Vec::new();
        let mut carrier = None;
        let mut line_no = 0;

        for line in s.lines() {
            line_no += 1;

            let mut words = line.split_whitespace();

            let is_pulse = match words.next() {
                Some("pulse") => true,
                Some("space") => false,
                Some("timeout") => false,
                Some("carrier") => {
                    match words.next() {
                        Some(w) => match w.parse() {
                            Ok(c) => {
                                if carrier.is_some() && carrier != Some(c) {
                                    return Err((
                                        line_no,
                                        String::from("carrier specified more than once"),
                                    ));
                                }

                                if c < 0 {
                                    return Err((
                                        line_no,
                                        format!("negative carrier {} does not make sense", c),
                                    ));
                                }

                                carrier = Some(c);
                            }
                            Err(_) => {
                                return Err((
                                    line_no,
                                    format!("carrier argument ‘{}’ is not a number", w),
                                ));
                            }
                        },
                        None => return Err((line_no, String::from("missing carrier value"))),
                    }

                    if let Some(w) = words.next() {
                        if !w.starts_with('#') && !w.starts_with("//") {
                            return Err((line_no, format!("unexpected ‘{}’", w)));
                        }
                    }

                    continue;
                }
                Some(w) => {
                    if !w.starts_with('#') && !w.starts_with("//") {
                        return Err((line_no, format!("unexpected ‘{}’", w)));
                    }
                    continue;
                }
                None => {
                    continue;
                }
            };

            let value = match words.next() {
                Some(w) => match w.parse() {
                    Ok(0) => {
                        return Err((line_no, "nonsensical 0 duration".to_string()));
                    }
                    Ok(n) => {
                        if n > 0xff_ff_ff {
                            return Err((line_no, format!("duration ‘{}’ too long", w)));
                        }
                        n
                    }
                    Err(_) => {
                        return Err((line_no, format!("invalid duration ‘{}’", w)));
                    }
                },
                None => {
                    return Err((line_no, "missing duration".to_string()));
                }
            };

            if let Some(trailing) = words.next() {
                return Err((line_no, format!("unexpected ‘{}’", trailing)));
            }

            if is_pulse {
                if res.len() % 2 == 1 {
                    // two consecutive pulses should be folded
                    *res.last_mut().unwrap() += value;
                } else {
                    res.push(value);
                }
            } else if res.len() % 2 == 0 {
                // two consecutive spaces should be folded, but leading spaces are ignored
                if let Some(last) = res.last_mut() {
                    *last += value;
                }
            } else {
                res.push(value);
            }
        }

        if res.is_empty() {
            if line_no == 0 {
                line_no = 1;
            }
            return Err((line_no, "missing pulse".to_string()));
        }

        Ok(Message {
            duty_cycle: None,
            carrier,
            raw: res,
        })
    }
}

#[test]
fn parse_mode2() {
    assert_eq!(
        Message::parse_mode2("").err(),
        Some((1, "missing pulse".to_string()))
    );
    assert_eq!(
        Message::parse_mode2("pulse 0").err(),
        Some((1, "nonsensical 0 duration".to_string()))
    );
    assert_eq!(
        Message::parse_mode2("pulse").err(),
        Some((1, "missing duration".to_string()))
    );
    assert_eq!(
        Message::parse_mode2("pulse abc").err(),
        Some((1, "invalid duration ‘abc’".to_string()))
    );
    assert_eq!(
        Message::parse_mode2("pulse 1\npulse 2").unwrap().raw,
        vec!(3u32)
    );
    assert_eq!(
        Message::parse_mode2("space 1\r\nspace 2\npulse 1\npulse 2")
            .unwrap()
            .raw,
        vec!(3u32)
    );
    assert_eq!(
        Message::parse_mode2("pulse 100\npulse 21\nspace 10\nspace 50")
            .unwrap()
            .raw,
        vec!(121u32, 60u32)
    );
    assert_eq!(
        Message::parse_mode2("polse 100\nspace 10\nspace 50").err(),
        Some((1, "unexpected ‘polse’".to_string()))
    );
    assert_eq!(
        Message::parse_mode2("pulse 100\nspace 10\npulse 50")
            .unwrap()
            .raw,
        vec!(100u32, 10u32, 50u32)
    );
    assert_eq!(
        Message::parse_mode2("pulse 100\nspace 10\npulse 50\nspace 34134134").err(),
        Some((4, "duration ‘34134134’ too long".to_string()))
    );
    assert_eq!(
        Message::parse_mode2("pulse 100\nspace 10\npulse 50 foobar\nspace 34134134").err(),
        Some((3, "unexpected ‘foobar’".to_string()))
    );
    assert_eq!(
        Message::parse_mode2("pulse 100\nspace 10\ncarrier foobar\nspace 34134134").err(),
        Some((3, "carrier argument ‘foobar’ is not a number".to_string()))
    );
    assert_eq!(
        Message::parse_mode2("pulse 100\nspace 10\ncarrier\nspace 34134134").err(),
        Some((3, "missing carrier value".to_string()))
    );
    assert_eq!(
        Message::parse_mode2("pulse 100\nspace 10\ncarrier 500 x\nspace 34134134").err(),
        Some((3, "unexpected ‘x’".to_string()))
    );
    assert_eq!(
        Message::parse_mode2("pulse 100\nspace 10\npulse 50\ncarrier 500 // hiya\ntimeout 100000")
            .unwrap(),
        Message {
            carrier: Some(500),
            duty_cycle: None,
            raw: vec!(100u32, 10u32, 50u32, 100000u32)
        }
    );
}

#[test]
fn parse_test() {
    assert_eq!(
        Message::parse("+100 +100"),
        Err("unexpected ‘+’ encountered".to_string())
    );

    assert_eq!(
        Message::parse("+100 -100 -1"),
        Err("unexpected ‘-’ encountered".to_string())
    );

    assert_eq!(
        Message::parse("+100 -100"),
        Ok(Message {
            raw: vec!(100, 100),
            duty_cycle: None,
            carrier: None
        })
    );

    assert_eq!(Message::parse(""), Err("missing length".to_string()));

    assert_eq!(Message::parse("+a"), Err("invalid number ‘a’".to_string()));

    assert_eq!(
        Message::parse("+0"),
        Err("nonsensical 0 length".to_string())
    );

    assert_eq!(
        Message::parse("100  \n100\r +1"),
        Ok(Message {
            raw: vec!(100u32, 100u32, 1u32),
            duty_cycle: None,
            carrier: None
        })
    );
    assert_eq!(
        Message::parse("100,100,+1,-20000"),
        Ok(Message {
            raw: vec!(100u32, 100u32, 1u32, 20000u32),
            duty_cycle: None,
            carrier: None
        })
    );
}
