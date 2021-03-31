use quick_xml::de::from_str;
use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
pub struct Protocols {
    protocol: Vec<Protocol>,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct Protocol {
    pub name: String,
    pub irp: String,
}

#[allow(dead_code)]
pub fn read_protocols() -> Vec<Protocol> {
    let protocols_xml = std::fs::read_to_string("IrpProtocols.xml").expect("file not found!");

    let protocols: Protocols = from_str(&protocols_xml).expect("unexpected xml");

    protocols.protocol
}
