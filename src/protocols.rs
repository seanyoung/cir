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
    let foo = std::fs::read_to_string("/home/sean/git/IrpTransmogrifier/target/IrpProtocols.xml")
        .expect("file not found!");

    let protocols: Protocols = from_str(&foo).expect("unexpected xml");

    return protocols.protocol;
}
