use std::collections::HashMap;

mod parse;
mod protocol;

/// A Linux keymap, either toml or text format used by ir-keytable
#[derive(PartialEq, Eq, Debug, Default)]
pub struct Keymap {
    pub name: String,
    pub protocol: String,
    pub variant: Option<String>,
    pub irp: Option<String>,
    pub rc_protocol: Option<u16>,
    pub raw: Option<Vec<Raw>>,
    pub scancodes: Option<HashMap<String, String>>,
}

#[derive(PartialEq, Eq, Debug)]
pub struct Raw {
    pub keycode: String,
    pub raw: Option<String>,
    pub repeat: Option<String>,
    pub pronto: Option<String>,
}

pub struct LinuxProtocol {
    pub name: &'static str,
    pub decoder: &'static str,
    pub irp: Option<&'static str>,
    pub scancode_mask: u32,
    pub protocol_no: u32,
}
