//! Parse linux rc keymaps

use serde_derive::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, PartialEq, Eq, Debug)]
pub struct Protocol {
    #[serde(default = "String::new")]
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

include!(concat!(env!("OUT_DIR"), "/text_keymap.rs"));

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
        let mut parser = text_keymap::PEG::new();

        match parser.parse(contents) {
            Ok(node) => {
                let first_line = &node.children[0];

                let table = first_line.children[6].as_str(contents).to_owned();

                let mut protocols: Vec<Protocol> =
                    collect_rules(&first_line.children[13], text_keymap::Rule::identifier)
                        .iter()
                        .map(|node| Protocol {
                            name: String::new(),
                            protocol: node.as_str(contents).to_owned(),
                            raw: None,
                            scancodes: None,
                            variant: None,
                        })
                        .collect();

                let scancodes: HashMap<String, String> =
                    collect_rules(&node, text_keymap::Rule::scancode)
                        .iter()
                        .map(|node| {
                            (
                                node.children[0].as_str(contents).to_owned(),
                                node.children[2].as_str(contents).to_owned(),
                            )
                        })
                        .collect();

                protocols[0].name = table;
                protocols[0].scancodes = Some(scancodes);

                Ok(Keymap { protocols })
            }
            Err(pos) => Err(format!("parse error at {}:{}", pos.0, pos.1)),
        }
    }
}

fn collect_rules(node: &text_keymap::Node, rule: text_keymap::Rule) -> Vec<&text_keymap::Node> {
    let mut list = Vec::new();

    fn recurse<'t>(
        node: &'t text_keymap::Node,
        rule: text_keymap::Rule,
        list: &mut Vec<&'t text_keymap::Node>,
    ) {
        if node.rule == rule {
            list.push(node);
        } else {
            for node in &node.children {
                recurse(node, rule, list);
            }
        }
    }

    recurse(node, rule, &mut list);

    list
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
                _ => panic!("{:?} not expected", s),
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
                _ => panic!("{:?} not expected", s),
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
                _ => panic!("{:?} not expected", s),
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
                _ => panic!("{:?} not expected", s),
            }
        }
    }
}
