//! Parse lircd.conf files and generate IRP notation for the parsed file.

use bitflags::bitflags;
use std::path::Path;

mod encode;
mod irp;
mod parse;

pub use encode::encode;

/// A button on a remote presented by a scancode
#[derive(Debug)]
pub struct Code {
    /// Name of the button
    pub name: String,
    /// Is this a duplicate entry; different codes may be mapped to the same key
    pub dup: bool,
    /// List of codes. Usually there is only one, sometimes a single button
    /// transmits multiple codes.
    pub code: Vec<u64>,
}

/// A button on a remote presented by raw IR
#[derive(Debug)]
pub struct RawCode {
    /// Name of the button
    pub name: String,
    /// Is this a duplicate entry; different IR may be mapped to the same key
    pub dup: bool,
    /// Raw IR lengths. The first entry is a pulse, followed by gap, pulse, etc.
    pub rawir: Vec<u32>,
}

bitflags! {
    /// Protocol flags
    #[derive(Default)]
    pub struct Flags: u32 {
        /// This remote uses raw codes
        const RAW_CODES = 0x0001;
        /// Uses the rc5 protocol
        const RC5 = 0x0002;
        /// SHIFT_ENC is an alias for RC5
        const SHIFT_ENC = 0x0002;
        /// Uses the rc6 protocol
        const RC6 = 0x0004;
        /// Uses the rc-mm protocol
        const RCMM = 0x0008;
        /// Uses pulse-distance encoding
        const SPACE_ENC = 0x0010;
        /// Bit encoding encodes the space before the pulse
        const SPACE_FIRST = 0x0020;
        /// Grundig protocol
        const GRUNDIG = 0x0040;
        /// B&O protocol
        const BO = 0x0080;
        /// Talk to device over serial port
        const SERIAL = 0x0100;
        /// XMP protocol
        const XMP = 0x0400;
        /// Reverse the bits in the encoding
        const REVERSE = 0x0800;
        /// No header in repeats
        const NO_HEAD_REP = 0x1000;
        /// No footer in repeats
        const NO_FOOT_REP = 0x2000;
        /// Each encoding will always have the same length
        const CONST_LENGTH = 0x4000;
        /// Header is preset in repeats
        const REPEAT_HEADER = 0x8000;
    }
}

/// Lirc remote definition
#[derive(Debug, Default)]
pub struct Remote {
    pub name: String,
    pub driver: String,
    pub serial_mode: String,
    pub flags: Flags,
    pub baud: u64,
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
    pub toggle_mask: u64,
    pub rc6_mask: u64,
    pub header: (u64, u64),
    pub bit: [(u64, u64); 4],
    pub foot: (u64, u64),
    pub repeat: (u64, u64),
    pub pre: (u64, u64),
    pub post: (u64, u64),
    pub gap: u64,
    pub gap2: u64,
    pub repeat_gap: u64,
    pub suppress_repeat: u64,
    pub frequency: u64,
    pub duty_cycle: u64,
    pub min_repeat: u64,
    /// Decoding-only features
    pub manual_sort: u64,
    pub min_code_repeat: u64,
    pub ignore_mask: u64,
    pub codes: Vec<Code>,
    pub raw_codes: Vec<RawCode>,
}

/// Read a lircd.conf file at the path specified. Such a file may contain multiple
/// remotes. Any parse errors or warnings are send to the log.
#[allow(clippy::result_unit_err)]
pub fn parse<P: AsRef<Path>>(path: P) -> Result<Vec<Remote>, ()> {
    parse::LircParser::parse(path.as_ref())
}
