/// parse pulse/space type input. This format is produces by lirc's mode2 tool.
/// Some lirc drivers sometimes produce consecutive pulses or spaces, rather
/// than alternating. These have to be folded.
pub fn parse(s: &str) -> Result<Vec<u32>, String> {
    let mut res = Vec::new();

    for line in s.lines() {
        let mut words = line.split_whitespace();

        let is_pulse = match words.next() {
            Some("pulse") => true,
            Some("space") => false,
            Some("timeout") => false,
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
        } else {
            if res.len() % 2 == 0 {
                // two consecutive spaces should be folded, but leading spaces are ignored
                if let Some(last) = res.last_mut() {
                    *last += value;
                }
            } else {
                res.push(value);
            }
        }
    }

    if res.is_empty() {
        return Err("missing pulse".to_string());
    }

    Ok(res)
}

#[test]
fn test_parse() {
    assert_eq!(parse(""), Err("missing pulse".to_string()));
    assert_eq!(parse("pulse 0"), Err("nonsensical 0 duration".to_string()));
    assert_eq!(parse("pulse"), Err("missing duration".to_string()));
    assert_eq!(
        parse("pulse abc"),
        Err("invalid duration ‘abc’".to_string())
    );
    assert_eq!(parse("pulse 1\npulse 2"), Ok(vec!(3u32)));
    assert_eq!(
        parse("space 1\r\nspace 2\npulse 1\npulse 2"),
        Ok(vec!(3u32))
    );
    assert_eq!(
        parse("pulse 100\npulse 21\nspace 10\nspace 50"),
        Ok(vec!(121u32, 60u32))
    );
    assert_eq!(
        parse("polse 100\nspace 10\nspace 50"),
        Err("unexpected ‘polse’".to_string())
    );
    assert_eq!(
        parse("pulse 100\nspace 10\npulse 50"),
        Ok(vec!(100u32, 10u32, 50u32))
    );
    assert_eq!(
        parse("pulse 100\nspace 10\npulse 50\nspace 34134134"),
        Err("duration ‘34134134’ too long".to_string())
    );
    assert_eq!(
        parse("pulse 100\nspace 10\npulse 50 foobar\nspace 34134134"),
        Err("unexpected ‘foobar’".to_string())
    );
    assert_eq!(
        parse("pulse 100\nspace 10\npulse 50\ntimeout 100000"),
        Ok(vec!(100u32, 10u32, 50u32, 100000u32))
    );
}
