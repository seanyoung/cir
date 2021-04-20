use super::Message;

/// parse pulse/space type input. This format is produces by lirc's mode2 tool.
/// Some lirc drivers sometimes produce consecutive pulses or spaces, rather
/// than alternating. These have to be folded.
pub fn parse(s: &str) -> Result<Message, String> {
    let mut res = Vec::new();
    let mut carrier = None;

    for line in s.lines() {
        let mut words = line.split_whitespace();

        let is_pulse = match words.next() {
            Some("pulse") => true,
            Some("space") => false,
            Some("timeout") => false,
            Some("carrier") => {
                match words.next() {
                    Some(w) => match i64::from_str_radix(w, 10) {
                        Ok(c) => {
                            if carrier.is_some() && carrier != Some(c) {
                                return Err(String::from("carrier specified more than once"));
                            }

                            if c < 0 {
                                return Err(format!("negative carrier {} does not make sense", c));
                            }

                            carrier = Some(c);
                        }
                        Err(_) => {
                            return Err(format!("carrier argument ‘{}’ is not a number", w));
                        }
                    },
                    None => return Err(String::from("missing carrier value")),
                }

                if let Some(w) = words.next() {
                    if !w.starts_with('#') && !w.starts_with("//") {
                        return Err(format!("unexpected ‘{}’", w));
                    }
                }

                continue;
            }
            Some(w) => {
                if !w.starts_with('#') && !w.starts_with("//") {
                    return Err(format!("unexpected ‘{}’", w));
                }
                continue;
            }
            None => {
                continue;
            }
        };

        let value = match words.next() {
            Some(w) => match u32::from_str_radix(w, 10) {
                Ok(0) => {
                    return Err("nonsensical 0 duration".to_string());
                }
                Ok(n) => {
                    if n > 0xff_ff_ff {
                        return Err(format!("duration ‘{}’ too long", w));
                    }
                    n
                }
                Err(_) => {
                    return Err(format!("invalid duration ‘{}’", w));
                }
            },
            None => {
                return Err("missing duration".to_string());
            }
        };

        if let Some(trailing) = words.next() {
            return Err(format!("unexpected ‘{}’", trailing));
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
        return Err("missing pulse".to_string());
    }

    Ok(Message {
        duty_cycle: None,
        carrier,
        raw: res,
    })
}

#[test]
fn test_parse() {
    assert_eq!(parse("").err(), Some("missing pulse".to_string()));
    assert_eq!(
        parse("pulse 0").err(),
        Some("nonsensical 0 duration".to_string())
    );
    assert_eq!(parse("pulse").err(), Some("missing duration".to_string()));
    assert_eq!(
        parse("pulse abc").err(),
        Some("invalid duration ‘abc’".to_string())
    );
    assert_eq!(parse("pulse 1\npulse 2").unwrap().raw, vec!(3u32));
    assert_eq!(
        parse("space 1\r\nspace 2\npulse 1\npulse 2").unwrap().raw,
        vec!(3u32)
    );
    assert_eq!(
        parse("pulse 100\npulse 21\nspace 10\nspace 50")
            .unwrap()
            .raw,
        vec!(121u32, 60u32)
    );
    assert_eq!(
        parse("polse 100\nspace 10\nspace 50").err(),
        Some("unexpected ‘polse’".to_string())
    );
    assert_eq!(
        parse("pulse 100\nspace 10\npulse 50").unwrap().raw,
        vec!(100u32, 10u32, 50u32)
    );
    assert_eq!(
        parse("pulse 100\nspace 10\npulse 50\nspace 34134134").err(),
        Some("duration ‘34134134’ too long".to_string())
    );
    assert_eq!(
        parse("pulse 100\nspace 10\npulse 50 foobar\nspace 34134134").err(),
        Some("unexpected ‘foobar’".to_string())
    );
    assert_eq!(
        parse("pulse 100\nspace 10\ncarrier foobar\nspace 34134134").err(),
        Some("carrier argument ‘foobar’ is not a number".to_string())
    );
    assert_eq!(
        parse("pulse 100\nspace 10\ncarrier\nspace 34134134").err(),
        Some("missing carrier value".to_string())
    );
    assert_eq!(
        parse("pulse 100\nspace 10\ncarrier 500 x\nspace 34134134").err(),
        Some("unexpected ‘x’".to_string())
    );
    assert_eq!(
        parse("pulse 100\nspace 10\npulse 50\ncarrier 500 // hiya\ntimeout 100000").unwrap(),
        Message {
            carrier: Some(500),
            duty_cycle: None,
            raw: vec!(100u32, 10u32, 50u32, 100000u32)
        }
    );
}
