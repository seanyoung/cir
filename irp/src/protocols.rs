use serde::Deserialize;
use std::{fs::File, io::BufReader, path::Path, str::FromStr};
use xml::reader::{EventReader, XmlEvent};

#[derive(Debug, Deserialize, PartialEq, Default)]
pub struct Protocol {
    pub name: String,
    pub alt_name: Vec<String>,
    pub irp: String,
    pub prefer_over: Vec<String>,
    pub absolute_tolerance: u32,
    pub relative_tolerance: f32,
    pub minimum_leadout: u32,
    pub decode_only: bool,
    pub decodable: bool,
    pub reject_repeatess: bool,
}

enum Element {
    None,
    Irp,
    AbsoluteTolerance,
    RelativeTolerance,
    AlternateName,
    DecodeOnly,
    Decodable,
    PreferOver,
    MinimumLeadout,
    RejectRepeatLess,
}

#[allow(dead_code)]
pub fn read_protocols(path: &Path) -> Vec<Protocol> {
    let file = File::open(path).unwrap();
    let file = BufReader::new(file);

    let parser = EventReader::new(file);
    let mut protocols: Vec<Protocol> = Vec::new();
    let mut protocol = None;
    let mut element = Element::None;

    for e in parser {
        match e {
            Ok(XmlEvent::StartElement {
                name, attributes, ..
            }) => match name.local_name.as_ref() {
                "protocol" => {
                    if attributes.len() == 1 && attributes[0].name.local_name == "name" {
                        protocol = Some(Protocol {
                            name: attributes[0].value.to_owned(),
                            decodable: true,
                            absolute_tolerance: 100,
                            relative_tolerance: 0.3,
                            minimum_leadout: 20000,
                            ..Default::default()
                        });
                    } else {
                        panic!("missing name attribute");
                    }
                }
                "irp" => {
                    element = Element::Irp;
                }
                "parameter" => {
                    for attr in attributes {
                        match attr.name.local_name.as_ref() {
                            "prefer_over" => {
                                element = Element::PreferOver;
                            }
                            "absolute-tolerance" => {
                                element = Element::AbsoluteTolerance;
                            }
                            "relative-tolerance" => {
                                element = Element::RelativeTolerance;
                            }
                            "decodable" => {
                                element = Element::Decodable;
                            }
                            "decode-only" => {
                                element = Element::DecodeOnly;
                            }
                            "alt_name" => {
                                element = Element::AlternateName;
                            }
                            "min-leadout" | "minimum-leadout" => {
                                element = Element::MinimumLeadout;
                            }
                            "reject_repeatless" => {
                                element = Element::RejectRepeatLess;
                            }
                            _ => (),
                        }
                    }
                }
                _ => (),
            },
            Ok(XmlEvent::CData(data)) => {
                if let Some(protocol) = &mut protocol {
                    match element {
                        Element::Irp => {
                            protocol.irp = data;
                        }
                        Element::AlternateName => {
                            protocol.alt_name.push(data);
                        }
                        Element::PreferOver => {
                            protocol.prefer_over.push(data);
                        }
                        Element::Decodable => {
                            protocol.decodable = bool::from_str(&data).unwrap();
                        }
                        Element::DecodeOnly => {
                            protocol.decode_only = bool::from_str(&data).unwrap();
                        }
                        Element::RejectRepeatLess => {
                            protocol.reject_repeatess = bool::from_str(&data).unwrap();
                        }
                        Element::AbsoluteTolerance => {
                            protocol.absolute_tolerance = u32::from_str(&data).unwrap();
                        }
                        Element::RelativeTolerance => {
                            protocol.relative_tolerance = f32::from_str(&data).unwrap();
                        }
                        Element::MinimumLeadout => {
                            protocol.minimum_leadout = u32::from_str(&data).unwrap();
                        }
                        Element::None => (),
                    }
                }

                element = Element::None;
            }
            Ok(XmlEvent::EndElement { name }) => {
                if name.local_name == "protocol" {
                    if let Some(protocol) = protocol {
                        protocols.push(protocol);
                    }
                    protocol = None;
                }
            }
            Err(e) => {
                panic!("Error: {}", e);
            }
            _ => {}
        }
    }

    protocols
}
