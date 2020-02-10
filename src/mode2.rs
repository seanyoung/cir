use num::Integer;

/// parse pulse/space
pub fn parse(s: &str) -> Result<Vec<u32>, String> {
    let mut res = Vec::new();

    for line in s.lines() {
        let mut words = line.split_whitespace();

        match words.next() {
            Some("pulse") => {
                if res.len().is_odd() {
                    return Err("pulse encountered while expecting space".to_string());
                }
            }
            Some("space") => {
                if res.len().is_even() {
                    return Err("space encountered while expecting pulse".to_string());
                }
            }
            Some("timeout") => {
                continue;
            }
            Some(w) => {
                if !w.starts_with('#') {
                    return Err(format!("unexpected ‘{}’", w));
                }
            }
            None => {}
        }

        match words.next() {
            Some(w) => match u32::from_str_radix(w, 10) {
                Ok(0) => {
                    return Err("nonsensical 0 length".to_string());
                }
                Ok(n) => res.push(n),
                Err(_) => {
                    return Err(format!("invalid number ‘{}’", w));
                }
            },
            None => {
                return Err("missing number".to_string());
            }
        }
    }

    if res.is_empty() {
        return Err("missing pulse".to_string());
    }

    if res.len().is_even() {
        res.pop();
    }

    Ok(res)
}

#[test]
fn test_parse() {
    assert_eq!(parse(""), Err("missing pulse".to_string()));
    assert_eq!(parse("pulse 0"), Err("nonsensical 0 length".to_string()));
    assert_eq!(parse("pulse"), Err("missing number".to_string()));
    assert_eq!(parse("pulse abc"), Err("invalid number ‘abc’".to_string()));
    assert_eq!(
        parse("pulse 1\npulse 2"),
        Err("pulse encountered while expecting space".to_string())
    );
    assert_eq!(
        parse("pulse 100\nspace 10\nspace 50"),
        Err("space encountered while expecting pulse".to_string())
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
        Ok(vec!(100u32, 10u32, 50u32))
    );
    assert_eq!(
        parse("pulse 100\nspace 10\npulse 50\ntimeout 100000"),
        Ok(vec!(100u32, 10u32, 50u32))
    );
}
