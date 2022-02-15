use super::log::Log;
use bitflags::bitflags;
use std::path::Path;

mod encode;
mod irp;
mod parse;

pub use encode::encode;

#[derive(Debug)]
pub struct LircCode {
    pub name: String,
    pub code: u64,
}

#[derive(Debug)]
pub struct LircRawCode {
    pub name: String,
    pub rawir: Vec<u32>,
}

bitflags! {
    #[derive(Default)]
    pub struct Flags: u32 {
        const RAW_CODES = 0x0001;
        const RC5 = 0x0002;
        const SHIFT_ENC = 0x0002;
        const RC6 = 0x0004;
        const RCMM = 0x0008;
        const SPACE_ENC = 0x0010;
        const SPACE_FIRST = 0x0020;
        const GRUNDIG = 0x0040;
        const BO = 0x0080;
        const SERIAL = 0x0100;
        const XMP = 0x0400;
        const REVERSE = 0x0800;
        const NO_HEAD_REP = 0x1000;
        const NO_FOOT_REP = 0x2000;
        const CONST_LENGTH = 0x4000;
        const REPEAT_HEADER = 0x8000;
        const COMPAT_REVERSE = 0x10000;
    }
}

#[derive(Debug, Default)]
pub struct LircRemote {
    pub name: String,
    pub driver: String,
    pub flags: Flags,
    pub eps: u64,
    pub aeps: u64,
    pub bits: u64,
    pub plead: u64,
    pub ptrail: u64,
    pub pre_data_bits: u64,
    pub pre_data: u64,
    pub post_data_bits: u64,
    pub post_data: u64,
    pub toggle_bit_mask: u64,
    pub toggle_bit: u64,
    pub rc6_mask: u64,
    pub header: (u64, u64),
    pub bit: [(u64, u64); 4],
    pub foot: (u64, u64),
    pub repeat: (u64, u64),
    pub pre: (u64, u64),
    pub post: (u64, u64),
    pub gap: u64,
    pub frequency: u64,
    pub duty_cycle: u64,
    pub codes: Vec<LircCode>,
    pub raw_codes: Vec<LircRawCode>,
}

/// Read a lircd.conf file at the path specified. Such a file may contain multiple
/// remotes. Any parse errors or warnings are send to the log.
#[allow(clippy::result_unit_err)]
pub fn parse<P: AsRef<Path>>(path: P, log: &Log) -> Result<Vec<LircRemote>, ()> {
    parse::LircParser::parse(path.as_ref(), log)
}
