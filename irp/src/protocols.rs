//! Parsing of IrpTransmogrifier's IrpProtocols.xml.

use serde::Deserialize;
use std::{
    fs::File,
    io::{self, BufReader},
    path::Path,
    str::FromStr,
};
use xml::reader::{EventReader, XmlEvent};

/// Entry in IrpTransmogrifier's IrpProtocols.xml.
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
    pub reject_repeatless: bool,
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

impl Protocol {
    /// Parse IrpTransmogrifier's IrpProtocols.xml.
    pub fn parse(path: &Path) -> io::Result<Vec<Protocol>> {
        let file = File::open(path)?;
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
                            match attr.value.as_ref() {
                                "prefer-over" => {
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
                                "minimum-leadout" => {
                                    element = Element::MinimumLeadout;
                                }
                                "reject-repeatless" => {
                                    element = Element::RejectRepeatLess;
                                }
                                "uei-executor"
                                | "xml"
                                | "frequency-tolerance"
                                | "frequency-lower"
                                | "frequency-upper" => {}
                                elem => {
                                    panic!("parameter {elem} unknown");
                                }
                            }
                        }
                    }
                    _ => (),
                },
                Ok(XmlEvent::Characters(data) | XmlEvent::CData(data)) => {
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
                                protocol.reject_repeatless = bool::from_str(&data).unwrap();
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
                    panic!("Error: {e}");
                }
                _ => {}
            }
        }

        Ok(protocols)
    }
}
