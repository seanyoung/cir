/*!
 * Reading lirc mode2 style input files
 */

use super::Message;

/// Parse pulse/space text. This format is produces by lirc's mode2 tool.
/// Some lirc drivers sometimes produce consecutive pulses or spaces, rather
/// than alternating. These are automatically folded into one.
///
/// The return value is the line number and the error string.
pub fn parse(s: &str) -> Result<Message, (usize, String)> {
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

#[test]
fn test_parse() {
    assert_eq!(parse("").err(), Some((1, "missing pulse".to_string())));
    assert_eq!(
        parse("pulse 0").err(),
        Some((1, "nonsensical 0 duration".to_string()))
    );
    assert_eq!(
        parse("pulse").err(),
        Some((1, "missing duration".to_string()))
    );
    assert_eq!(
        parse("pulse abc").err(),
        Some((1, "invalid duration ‘abc’".to_string()))
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
        Some((1, "unexpected ‘polse’".to_string()))
    );
    assert_eq!(
        parse("pulse 100\nspace 10\npulse 50").unwrap().raw,
        vec!(100u32, 10u32, 50u32)
    );
    assert_eq!(
        parse("pulse 100\nspace 10\npulse 50\nspace 34134134").err(),
        Some((4, "duration ‘34134134’ too long".to_string()))
    );
    assert_eq!(
        parse("pulse 100\nspace 10\npulse 50 foobar\nspace 34134134").err(),
        Some((3, "unexpected ‘foobar’".to_string()))
    );
    assert_eq!(
        parse("pulse 100\nspace 10\ncarrier foobar\nspace 34134134").err(),
        Some((3, "carrier argument ‘foobar’ is not a number".to_string()))
    );
    assert_eq!(
        parse("pulse 100\nspace 10\ncarrier\nspace 34134134").err(),
        Some((3, "missing carrier value".to_string()))
    );
    assert_eq!(
        parse("pulse 100\nspace 10\ncarrier 500 x\nspace 34134134").err(),
        Some((3, "unexpected ‘x’".to_string()))
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
