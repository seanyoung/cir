use serde_derive::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
pub struct Protocol {
    name: String,
    protocol: String,
    variant: Option<String>,
    raw: Option<Vec<Raw>>,
    scancodes: Option<HashMap<String, String>>,
}

#[derive(Deserialize)]
pub struct Raw {
    keycode: String,
    raw: Option<String>,
    repeat: Option<String>,
    pronto: Option<String>,
}

#[derive(Deserialize)]
pub struct Keymap {
    protocols: Vec<Protocol>,
}

pub fn parse(s: &str) -> Result<Keymap, String> {
    let keymap: Keymap = toml::from_str(s).map_err(|e| e.to_string())?;

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
                        }
                    }
                }
            }
        } else if p.raw.is_some() {
            return Err("raw entries for non-raw protocol".to_string());
        }
    }

    Ok(keymap)
}

#[test]
fn parse_test() {
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

    let k = parse(s).unwrap();

    assert_eq!(k.protocols[0].name, "hauppauge");
    assert_eq!(k.protocols[0].protocol, "rc5");
    assert_eq!(k.protocols[0].variant, Some(String::from("rc5")));
    if let Some(scancodes) = &k.protocols[0].scancodes {
        for s in scancodes {
            match (s.0.as_str(), s.1.as_str()) {
                ("0x1e3b", "KEY_SELECT") | ("0x1e3d", "KEY_POWER2") | ("0x1e1c", "KEY_TV") => {}
                _ => panic!(format!("{:?} not expected", s)),
            }
        }
    }
}
