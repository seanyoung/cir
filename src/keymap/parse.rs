//! Parse linux rc keymaps

use super::{Keymap, Raw};
use irp::{Message, Pronto};
use std::{collections::HashMap, ffi::OsStr, fmt::Write, fs::File, io::Read, path::Path};
use toml::{Table, Value};

peg::parser! {
    grammar text_keymap() for str {
        pub rule keymap() -> Vec<Keymap>
        = (_ newline())* first:first_line() lines:lines() _
        {
            let mut scancodes = HashMap::new();

            for (code, name) in lines.into_iter().flatten() {
                let code = string_to_scancode(code).unwrap();
                scancodes.insert(code, name.to_owned());
            }

            let mut protocol = vec![Keymap {
                    name: first.0.to_owned(),
                    protocol: first.1[0].to_owned(),
                    scancodes,
                    ..Default::default()
            }];

            for other in &first.1[1..] {
                protocol.push(Keymap { protocol: other.to_string(), ..Default::default() });
            }

            protocol
        }

        rule first_line() -> (&'input str, Vec<&'input str>)
        = _ "#" _ "table" (":" / "=")? _ name:identifier()  _ "," _ "type" (":" / "=")? _ protocols:protocols() _ newline()
        { (name, protocols) }

        rule identifier() -> &'input str
        = quiet!{$([ 'a'..='z' | 'A'..='Z']['a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-' ]*)}
        / expected!("identifier")

        rule protocols() -> Vec<&'input str>
        = protocols:(identifier() ++ ("," _)) { protocols }

        rule lines() -> Vec<Option<(&'input str, &'input str)>>
        = codes:((scancode() / comment()) ** newline()) { codes }

        rule newline()
        = "\r\n" / "\n"

        rule comment() -> Option<(&'input str, &'input str)>
        = _ "#" [^'\n']* { None }
        / _ { None }

        rule scancode() -> Option<(&'input str, &'input str)>
        = _ hex:hex() _ id:identifier() _ { Some((hex, id)) }

        rule hex() -> &'input str
        = hex:$("0x" ['0'..='9' | 'a'..='f' | 'A'..='F']+) _ { hex }

        rule _ = quiet!{[' ' | '\t']*}
    }
}

fn string_to_scancode(s: &str) -> Result<u64, std::num::ParseIntError> {
    if let Some(hex) = s.strip_prefix("0x") {
        u64::from_str_radix(hex, 16)
    } else {
        str::parse(s)
    }
}
impl Keymap {
    pub fn parse(path: &Path) -> Result<Vec<Keymap>, String> {
        let mut f = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;

        let mut contents = String::new();

        f.read_to_string(&mut contents)
            .map_err(|e| format!("{}: {e}", path.display()))?;

        Keymap::parse_text(&contents, path)
    }

    /// Parse a rc keymap file, either toml or old text format. No validation is done of key codes or protocol names
    pub fn parse_text(contents: &str, filename: &Path) -> Result<Vec<Keymap>, String> {
        if filename.extension() == Some(OsStr::new("toml")) {
            parse_toml(contents, filename)
        } else {
            text_keymap::keymap(contents)
                .map_err(|pos| format!("{}: parse error at {pos}", filename.display()))
        }
    }
}

fn parse_toml(contents: &str, filename: &Path) -> Result<Vec<Keymap>, String> {
    let top = contents
        .parse::<Table>()
        .map_err(|e| format!("{}: {e}", filename.display()))?;

    let Some(Value::Array(protocols)) = top.get("protocols") else {
        return Err(format!(
            "{}: missing top level protocols array",
            filename.display()
        ));
    };

    let mut res = Vec::new();

    for entry in protocols {
        let Some(Value::String(name)) = entry.get("name") else {
            return Err(format!("{}: missing name", filename.display()));
        };

        let Some(Value::String(protocol)) = entry.get("protocol") else {
            return Err(format!("{}: missing protocol", filename.display()));
        };

        let mut variant = None;
        if let Some(Value::String(entry)) = entry.get("variant") {
            variant = Some(entry.to_owned());
        }

        let mut rc_protocol = None;
        if let Some(Value::Integer(n)) = entry.get("rc_protocol") {
            if let Ok(n) = (*n).try_into() {
                rc_protocol = Some(n);
            } else {
                return Err(format!(
                    "{}: rc_protocol {n} must be 16 bit value",
                    filename.display()
                ));
            }
        }

        let mut irp = None;
        let mut raw_entries = Vec::new();
        let mut scancodes = HashMap::new();

        if protocol == "raw" {
            // find raw entries
            let Some(Value::Array(e)) = entry.get("raw") else {
                return Err(format!(
                    "{}: raw protocol is misssing raw entries",
                    filename.display()
                ));
            };

            for e in e {
                let Some(Value::String(keycode)) = e.get("keycode") else {
                    return Err(format!("{}: missing keycode", filename.display()));
                };

                let raw = if let Some(Value::String(raw)) = e.get("raw") {
                    let raw =
                        Message::parse(raw).map_err(|e| format!("{}: {e}", filename.display()))?;
                    Some(raw)
                } else {
                    None
                };

                let repeat = if let Some(Value::String(repeat)) = e.get("repeat") {
                    let repeat = Message::parse(repeat)
                        .map_err(|e| format!("{}: {e}", filename.display()))?;
                    Some(repeat)
                } else {
                    None
                };

                let pronto = if let Some(Value::String(pronto)) = e.get("pronto") {
                    let pronto = Pronto::parse(pronto)
                        .map_err(|e| format!("{}: {e}", filename.display()))?;
                    Some(pronto)
                } else {
                    None
                };

                if pronto.is_some() {
                    if raw.is_some() {
                        return Err(format!(
                            "{}: raw entry has both pronto hex code and raw",
                            filename.display()
                        ));
                    }
                    if repeat.is_some() {
                        return Err(format!(
                            "{}: raw entry has both pronto hex code and repeat",
                            filename.display()
                        ));
                    }
                } else if raw.is_none() {
                    return Err(format!(
                        "{}: raw entry has neither pronto hex code nor raw",
                        filename.display()
                    ));
                }

                raw_entries.push(Raw {
                    keycode: keycode.to_owned(),
                    raw,
                    repeat,
                    pronto,
                });
            }
        } else {
            if entry.get("raw").is_some() {
                return Err(format!(
                    "{}: raw entries for non-raw protocol",
                    filename.display()
                ));
            }

            if protocol == "irp" {
                if let Some(Value::String(entry)) = entry.get("irp") {
                    irp = Some(entry.to_owned());
                }
            } else if entry.get("irp").is_some() {
                return Err(format!(
                    "{}: set the protocol to irp when using irp",
                    filename.display()
                ));
            } else {
                irp = bpf_protocol_irp(protocol, entry.as_table().unwrap());
            }

            if let Some(Value::Table(codes)) = entry.get("scancodes") {
                for (scancode, keycode) in codes {
                    let scancode = string_to_scancode(scancode).map_err(|_| {
                        format!("{}: {scancode} is a not valid scancode", filename.display())
                    })?;
                    let Value::String(keycode) = keycode else {
                        return Err(format!("{}: keycode should be string", filename.display()));
                    };

                    scancodes.insert(scancode, keycode.to_owned());
                }
            }
        }

        res.push(Keymap {
            name: name.to_owned(),
            protocol: protocol.to_owned(),
            variant,
            raw: raw_entries,
            rc_protocol,
            scancodes,
            irp,
        });
    }

    Ok(res)
}

fn bpf_protocol_irp(protocol: &str, entry: &Table) -> Option<String> {
    let param = |name: &str, default: i64| -> i64 {
        if let Some(Value::Integer(n)) = entry.get(name) {
            *n
        } else {
            default
        }
    };

    match protocol {
        "pulse_distance" => {
            let mut irp = "{".to_owned();
            let bits = param("bits", 4);

            if param("reverse", 0) == 0 {
                irp.push_str("msb,");
            }

            if entry.contains_key("carrier") {
                write!(irp, "{}Hz,", param("carrier", 0)).unwrap();
            }

            if irp.ends_with(',') {
                irp.pop();
            }

            write!(
                irp,
                "}}<{},-{}|{},-{}>({},-{},CODE:{},{},-40m",
                param("bit_pulse", 625),
                param("bit_0_space", 375),
                param("bit_pulse", 625),
                param("bit_1_space", 1625),
                param("header_pulse", 2125),
                param("header_space", 1875),
                bits,
                param("trailer_pulse", 625),
            )
            .unwrap();

            let header_optional = param("header_optional", 0);

            if header_optional > 0 {
                write!(
                    irp,
                    ",(CODE:{},{},-40m)*",
                    bits,
                    param("trailer_pulse", 625),
                )
                .unwrap();
            } else {
                let repeat_pulse = param("repeat_pulse", 0);
                if repeat_pulse > 0 {
                    write!(
                        irp,
                        ",({},-{},{},-40)*",
                        repeat_pulse,
                        param("repeat_space", 0),
                        param("trailer_pulse", 625)
                    )
                    .unwrap();
                }
            }

            write!(irp, ") [CODE:0..{}]", gen_mask(bits)).unwrap();

            Some(irp)
        }
        "pulse_length" => {
            let mut irp = "{".to_owned();
            let bits = param("bits", 4);

            if param("reverse", 0) == 0 {
                irp.push_str("msb,");
            }

            if entry.contains_key("carrier") {
                write!(irp, "{}Hz,", param("carrier", 0)).unwrap();
            }

            if irp.ends_with(',') {
                irp.pop();
            }

            write!(
                irp,
                "}}<{},-{}|{},-{}>({},-{},CODE:{},-40m",
                param("bit_0_pulse", 375),
                param("bit_space", 625),
                param("bit_1_pulse", 1625),
                param("bit_space", 625),
                param("header_pulse", 2125),
                param("header_space", 1875),
                bits,
            )
            .unwrap();

            let header_optional = param("header_optional", 0);

            if header_optional > 0 {
                write!(irp, ",(CODE:{},-40m)*", bits).unwrap();
            } else {
                let repeat_pulse = param("repeat_pulse", 0);
                if repeat_pulse > 0 {
                    write!(
                        irp,
                        ",({},-{},{},-40)*",
                        repeat_pulse,
                        param("repeat_space", 0),
                        param("trailer_pulse", 625)
                    )
                    .unwrap();
                }
            }

            write!(irp, ") [CODE:0..{}]", gen_mask(bits)).unwrap();

            Some(irp)
        }
        "manchester" => {
            let mut irp = "{msb,".to_owned();
            let bits = param("bits", 14);
            let toggle_bit = param("toggle_bit", 100);

            if entry.contains_key("carrier") {
                write!(irp, "{}Hz,", param("carrier", 0)).unwrap();
            }

            if irp.ends_with(',') {
                irp.pop();
            }

            write!(
                irp,
                "}}<-{},{}|{},-{}>(",
                param("zero_space", 888),
                param("zero_pulse", 888),
                param("one_pulse", 888),
                param("one_space", 888),
            )
            .unwrap();

            let header_pulse = param("header_pulse", 0);
            let header_space = param("header_space", 0);

            if header_pulse > 0 && header_space > 0 {
                write!(irp, "{},-{},", header_pulse, header_space).unwrap();
            }

            if toggle_bit >= bits {
                write!(irp, "CODE:{},-40m", bits,).unwrap();
            } else {
                let leading = bits - toggle_bit;
                if leading > 1 {
                    write!(irp, "CODE:{}:{},", leading - 1, toggle_bit + 1).unwrap();
                }
                write!(irp, "T:1,").unwrap();
                if toggle_bit > 0 {
                    write!(irp, "CODE:{},", toggle_bit).unwrap();
                }
                irp.pop();
            }

            write!(irp, ",-40m) [CODE:0..{}]", gen_mask(bits)).unwrap();

            Some(irp)
        }
        _ => None,
    }
}

fn gen_mask(v: i64) -> u64 {
    if v < 64 {
        (1u64 << v) - 1
    } else {
        u64::MAX
    }
}

#[test]
fn parse_toml_test() {
    let s = r#"
    [[protocols]]
    name = "hauppauge"
    protocol = "rc5"
    variant = "rc5"
    [protocols.scancodes]
    0x1e3b = "KEY_SELECT"
    0x1e3d = "KEY_POWER2"
    0x1e1c = "KEY_TV"
    "#;

    let k = Keymap::parse_text(s, Path::new("x.toml")).unwrap();

    assert_eq!(k[0].name, "hauppauge");
    assert_eq!(k[0].protocol, "rc5");
    assert_eq!(k[0].variant, Some(String::from("rc5")));
    for s in &k[0].scancodes {
        match (s.0, s.1.as_str()) {
            (0x1e3b, "KEY_SELECT") | (0x1e3d, "KEY_POWER2") | (0x1e1c, "KEY_TV") => {}
            _ => panic!("{s:?} not expected"),
        }
    }

    let s = r#"
    [[protocols]]
    name = "hauppauge"
    protocol = "raw"
    [protocols.scancodes]
    0x1e3b = "KEY_SELECT"
    0x1e3d = "KEY_POWER2"
    0x1e1c = "KEY_TV"
    "#;

    assert_eq!(
        Keymap::parse_text(s, Path::new("x.toml")),
        Err("x.toml: raw protocol is misssing raw entries".to_string())
    );

    let s = r#"
    [[protocols]]
    name = "hauppauge"
    protocol = "raw"
    [[protocols.raw]]
    keycode = 'FOO'
    "#;

    assert_eq!(
        Keymap::parse_text(s, Path::new("x.toml")),
        Err("x.toml: raw entry has neither pronto hex code nor raw".to_string())
    );

    let s = r#"
    [[protocols]]
    name = "hauppauge"
    protocol = "raw"
    [[protocols.raw]]
    keycode = 'FOO'
    repeat = '+100'
    "#;

    assert_eq!(
        Keymap::parse_text(s, Path::new("x.toml")),
        Err("x.toml: raw entry has neither pronto hex code nor raw".to_string())
    );
}

#[test]
fn parse_text_test() {
    let s = r#"
    # table hauppauge, type: RC5
    0x1e3b KEY_SELECT
    0x1e3d KEY_POWER2
    0x1e1c KEY_TV
    "#;

    let k = Keymap::parse_text(s, Path::new("hauppauge")).unwrap();

    assert_eq!(k[0].name, "hauppauge");
    assert_eq!(k[0].protocol, "RC5");
    assert_eq!(k[0].variant, None);
    for s in &k[0].scancodes {
        match (s.0, s.1.as_str()) {
            (0x1e3b, "KEY_SELECT") | (0x1e3d, "KEY_POWER2") | (0x1e1c, "KEY_TV") => {}
            _ => panic!("{s:?} not expected"),
        }
    }

    let s = r#"
    # table: rc6_mce, type: RC6, foo
    0x800f0400 KEY_NUMERIC_0
    0x800f0401 KEY_NUMERIC_1
    # foobar
    0x800f0402 KEY_NUMERIC_2

    0x800f0403 KEY_NUMERIC_3
    "#;

    let k = Keymap::parse_text(s, Path::new("hauppauge")).unwrap();

    assert_eq!(k[0].name, "rc6_mce");
    assert_eq!(k[0].protocol, "RC6");
    assert_eq!(k[1].protocol, "foo");
    assert_eq!(k[0].variant, None);
    for s in &k[0].scancodes {
        match (s.0, s.1.as_str()) {
            (0x800f0400, "KEY_NUMERIC_0")
            | (0x800f0401, "KEY_NUMERIC_1")
            | (0x800f0402, "KEY_NUMERIC_2")
            | (0x800f0403, "KEY_NUMERIC_3") => {}
            _ => panic!("{s:?} not expected"),
        }
    }

    let s = r#"
    # table streamzap, type: RC-5-SZ
    0x28c0 KEY_NUMERIC_0
    0x28c1 KEY_NUMERIC_1
    0x28c2 KEY_NUMERIC_2
    "#;

    let k = Keymap::parse_text(s, Path::new("hauppauge")).unwrap();

    assert_eq!(k[0].name, "streamzap");
    assert_eq!(k[0].protocol, "RC-5-SZ");
    assert_eq!(k[0].variant, None);
    for s in &k[0].scancodes {
        match (s.0, s.1.as_str()) {
            (0x28c0, "KEY_NUMERIC_0") | (0x28c1, "KEY_NUMERIC_1") | (0x28c2, "KEY_NUMERIC_2") => {}
            _ => panic!("{s:?} not expected"),
        }
    }
}

#[test]
fn parse_bpf_toml_test() {
    let s = r#"
    [[protocols]]
    name = "meh"
    protocol = "manchester"
    toggle_bit = 12
    [protocols.scancodes]
    0x1e3b = "KEY_SELECT"
    0x1e3d = "KEY_POWER2"
    0x1e1c = "KEY_TV"
    "#;

    let k = Keymap::parse_text(s, Path::new("x.toml")).unwrap();

    assert_eq!(k[0].name, "meh");
    assert_eq!(k[0].protocol, "manchester");
    assert_eq!(
        k[0].irp,
        Some("{msb}<-888,888|888,-888>(CODE:1:13,T:1,CODE:12,-40m) [CODE:0..16383]".into())
    );

    let s = r#"
    [[protocols]]
    name = "meh"
    protocol = "manchester"
    toggle_bit = 1
    carrier = 38000
    header_pulse = 300
    header_space = 350
    [protocols.scancodes]
    0x1e3b = "KEY_SELECT"
    0x1e3d = "KEY_POWER2"
    0x1e1c = "KEY_TV"
    "#;

    let k = Keymap::parse_text(s, Path::new("x.toml")).unwrap();

    assert_eq!(k[0].name, "meh");
    assert_eq!(k[0].protocol, "manchester");
    assert_eq!(
        k[0].irp,
        Some(
            "{msb,38000Hz}<-888,888|888,-888>(300,-350,CODE:12:2,T:1,CODE:1,-40m) [CODE:0..16383]"
                .into()
        )
    );
}
