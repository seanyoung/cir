use num::Integer;

/// Parse a raw IR string of the form "+9000 -45000 +2250"
pub fn parse(s: &str) -> Result<Vec<u32>, String> {
    let mut res = Vec::new();

    for (i, e) in s.split(|c: char| c.is_whitespace() || c == ',').enumerate() {
        let mut chars = e.chars().peekable();

        match chars.peek() {
            Some('+') => {
                if i.is_odd() {
                    return Err("unexpected ‘+’ encountered".to_string());
                }
                chars.next();
            }
            Some('-') => {
                if i.is_even() {
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

        res.push(v);
    }

    if res.is_empty() {
        return Err("missing length".to_string());
    }

    Ok(res)
}

/// Convert a Vec<u32> to raw IR string
pub fn print_to_string(ir: &[u32]) -> String {
    ir.iter()
        .enumerate()
        .map(|(i, v)| format!("{}{}", if i.is_even() { "+" } else { "-" }, v))
        .collect::<Vec<String>>()
        .join(" ")
}

#[test]
fn parse_test() {
    assert_eq!(
        parse("+100 +100"),
        Err("unexpected ‘+’ encountered".to_string())
    );

    assert_eq!(
        parse("+100 -100 -1"),
        Err("unexpected ‘-’ encountered".to_string())
    );

    assert_eq!(parse("+100 -100"), Ok(vec!(100, 100)));

    assert_eq!(parse(""), Err("invalid number ‘’".to_string()));

    assert_eq!(parse("+a"), Err("invalid number ‘a’".to_string()));

    assert_eq!(parse("+0"), Err("nonsensical 0 length".to_string()));

    assert_eq!(parse("100 100 +1"), Ok(vec!(100u32, 100u32, 1u32)));
    assert_eq!(
        parse("100,100,+1,-20000"),
        Ok(vec!(100u32, 100u32, 1u32, 20000u32))
    );
}

#[test]
fn print_test() {
    assert_eq!(print_to_string(&[100, 50, 75]), "+100 -50 +75");
}
