//! Parse, encode and decode linux rc keymaps

use irp::{Message, Pronto};
use std::collections::HashMap;

mod decode;
mod encode;
mod parse;
mod protocol;

pub use encode::encode;
pub use protocol::LINUX_PROTOCOLS;

/// A Linux keymap, either toml or text format used by ir-keytable
#[derive(PartialEq, Debug, Default)]
pub struct Keymap {
    pub name: String,
    pub protocol: String,
    pub variant: Option<String>,
    pub irp: Option<String>,
    pub rc_protocol: Option<u16>,
    pub raw: Vec<Raw>,
    pub scancodes: HashMap<u64, String>,
}

#[derive(PartialEq, Debug)]
pub struct Raw {
    pub keycode: String,
    pub raw: Option<Message>,
    pub repeat: Option<Message>,
    pub pronto: Option<Pronto>,
}

pub struct LinuxProtocol {
    pub name: &'static str,
    pub decoder: &'static str,
    pub irp: Option<&'static str>,
    pub scancode_mask: u32,
    pub protocol_no: u32,
}
