//! Parse linux rc keymaps

use serde_derive::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, PartialEq, Eq, Debug, Default)]
pub struct Protocol {
    pub name: String,
    pub protocol: String,
    pub variant: Option<String>,
    pub raw: Option<Vec<Raw>>,
    pub scancodes: Option<HashMap<String, String>>,
}

#[derive(Deserialize, PartialEq, Eq, Debug)]
pub struct Raw {
    pub keycode: String,
    pub raw: Option<String>,
    pub repeat: Option<String>,
    pub pronto: Option<String>,
}

#[derive(Deserialize, PartialEq, Eq, Debug)]
pub struct Keymap {
    pub protocols: Vec<Protocol>,
}

peg::parser! {
    grammar text_keymap() for str {
        pub rule keymap() -> Vec<Protocol>
        = (_ newline())* first:first_line() lines:lines() _
        {
            let mut scancodes = HashMap::new();

            for (code, name) in lines.into_iter().flatten() {
                scancodes.insert(code.to_owned(), name.to_owned());
            }

            let mut protocol = vec![Protocol {
                    name: first.0.to_owned(),
                    protocol: first.1[0].to_owned(),
                    scancodes: Some(scancodes),
                    ..Default::default()
            }];

            for other in &first.1[1..] {
                protocol.push(Protocol { protocol: other.to_string(), ..Default::default() });
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

/// Parse a rc keymap file, either toml or old text format. No validation is done of key codes or protocol names
pub fn parse(contents: &str, filename: &str) -> Result<Keymap, String> {
    if filename.ends_with(".toml") {
        let keymap: Keymap = toml::from_str(contents).map_err(|e| e.to_string())?;

        for p in &keymap.protocols {
            if p.protocol == "raw" {
                match &p.raw {
                    None => {
                        return Err("raw protocol is misssing raw entries".to_string());
                    }
                    Some(raw) => {
                        for r in raw {
                            if r.pronto.is_some() {
                                if r.raw.is_some() {
                                    return Err(
                                        "raw entry has both pronto hex code and raw".to_string()
                                    );
                                }
                                if r.repeat.is_some() {
                                    return Err(
                                        "raw entry has both pronto hex code and repeat".to_string()
                                    );
                                }
                            } else if r.raw.is_none() {
                                return Err(
                                    "raw entry has neither pronto hex code nor raw".to_string()
                                );
                            }
                        }
                    }
                }
            } else if p.raw.is_some() {
                return Err("raw entries for non-raw protocol".to_string());
            }
        }

        Ok(keymap)
    } else {
        match text_keymap::keymap(contents) {
            Ok(protocols) => Ok(Keymap { protocols }),
            Err(pos) => Err(format!("parse error at {pos}")),
        }
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

    let k = parse(s, "x.toml").unwrap();

    assert_eq!(k.protocols[0].name, "hauppauge");
    assert_eq!(k.protocols[0].protocol, "rc5");
    assert_eq!(k.protocols[0].variant, Some(String::from("rc5")));
    if let Some(scancodes) = &k.protocols[0].scancodes {
        for s in scancodes {
            match (s.0.as_str(), s.1.as_str()) {
                ("0x1e3b", "KEY_SELECT") | ("0x1e3d", "KEY_POWER2") | ("0x1e1c", "KEY_TV") => {}
                _ => panic!("{s:?} not expected"),
            }
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
        parse(s, "x.toml"),
        Err("raw protocol is misssing raw entries".to_string())
    );

    let s = r#"
    [[protocols]]
    name = "hauppauge"
    protocol = "raw"
    [[protocols.raw]]
    keycode = 'FOO'
    "#;

    assert_eq!(
        parse(s, "x.toml"),
        Err("raw entry has neither pronto hex code nor raw".to_string())
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
        parse(s, "x.toml"),
        Err("raw entry has neither pronto hex code nor raw".to_string())
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

    let k = parse(s, "hauppauge").unwrap();

    assert_eq!(k.protocols[0].name, "hauppauge");
    assert_eq!(k.protocols[0].protocol, "RC5");
    assert_eq!(k.protocols[0].variant, None);
    if let Some(scancodes) = &k.protocols[0].scancodes {
        for s in scancodes {
            match (s.0.as_str(), s.1.as_str()) {
                ("0x1e3b", "KEY_SELECT") | ("0x1e3d", "KEY_POWER2") | ("0x1e1c", "KEY_TV") => {}
                _ => panic!("{s:?} not expected"),
            }
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

    let k = parse(s, "hauppauge").unwrap();

    assert_eq!(k.protocols[0].name, "rc6_mce");
    assert_eq!(k.protocols[0].protocol, "RC6");
    assert_eq!(k.protocols[1].protocol, "foo");
    assert_eq!(k.protocols[0].variant, None);
    if let Some(scancodes) = &k.protocols[0].scancodes {
        for s in scancodes {
            match (s.0.as_str(), s.1.as_str()) {
                ("0x800f0400", "KEY_NUMERIC_0")
                | ("0x800f0401", "KEY_NUMERIC_1")
                | ("0x800f0402", "KEY_NUMERIC_2")
                | ("0x800f0403", "KEY_NUMERIC_3") => {}
                _ => panic!("{s:?} not expected"),
            }
        }
    }

    let s = r#"
    # table streamzap, type: RC-5-SZ
    0x28c0 KEY_NUMERIC_0
    0x28c1 KEY_NUMERIC_1
    0x28c2 KEY_NUMERIC_2
    "#;

    let k = parse(s, "hauppauge").unwrap();

    assert_eq!(k.protocols[0].name, "streamzap");
    assert_eq!(k.protocols[0].protocol, "RC-5-SZ");
    assert_eq!(k.protocols[0].variant, None);
    if let Some(scancodes) = &k.protocols[0].scancodes {
        for s in scancodes {
            match (s.0.as_str(), s.1.as_str()) {
                ("0x28c0", "KEY_NUMERIC_0")
                | ("0x28c1", "KEY_NUMERIC_1")
                | ("0x28c2", "KEY_NUMERIC_2") => {}
                _ => panic!("{s:?} not expected"),
            }
        }
    }
}
