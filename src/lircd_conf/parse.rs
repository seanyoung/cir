use super::{Flags, LircCode, LircRawCode, LircRemote};
use crate::log::Log;
use std::{
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Lines},
    path::{Path, PathBuf},
    str::{FromStr, SplitWhitespace},
};

pub struct LircParser<'a> {
    path: PathBuf,
    line_no: u32,
    lines: Lines<BufReader<File>>,
    log: &'a Log,
}

/// We need a custom parser for lircd.conf files, because the parser in lircd itself
/// is custom and permits all sorts which a proper parser would not. For example,
/// garbage is permitted when 'begin remote' is expected, and most lines can have
/// trailing characters after the first two tokens.
impl<'a> LircParser<'a> {
    pub fn new(path: &Path, log: &'a Log) -> Result<Self, ()> {
        let file = OpenOptions::new()
            .read(true)
            .open(path)
            .map_err(|e| log.error(&format!("failed to open '{}': {}", path.display(), e)))?;
        let reader = BufReader::new(file);

        log.info(&format!("parsing '{}' as lircd.conf file", path.display()));

        Ok(LircParser {
            path: PathBuf::from(path),
            line_no: 0,
            lines: reader.lines(),
            log,
        })
    }

    pub fn parse(&mut self) -> Result<Vec<LircRemote>, ()> {
        let mut remotes = Vec::new();

        loop {
            let line = self.next_line()?;

            if line.is_none() {
                return Ok(remotes);
            }

            let line = line.unwrap();

            let mut words = line.split_whitespace();

            let first = words.next();
            let second = words.next();

            if let (Some("begin"), Some("remote")) = (first, second) {
                let mut remote = self.read_remote()?;

                if self.sanity_checks(&mut remote) {
                    remotes.push(remote);
                }
            } else {
                self.log.warning(&format!(
                    "{}:{}: expected 'begin remote', got '{}'",
                    self.path.display(),
                    self.line_no,
                    line
                ));
            }
        }
    }

    fn read_remote(&mut self) -> Result<LircRemote, ()> {
        let mut remote = LircRemote {
            frequency: 38000,
            ..Default::default()
        };

        loop {
            let line = self.next_line()?;

            if line.is_none() {
                self.log.error(&format!(
                    "{}:{}: unexpected end of file",
                    self.path.display(),
                    self.line_no
                ));
                return Err(());
            }

            let line = line.unwrap();

            let mut words = line.split_whitespace();

            let first = words.next();
            let second = words.next();

            match first {
                Some("name") => {
                    if second.is_none() {
                        self.log.error(&format!(
                            "{}:{}: missing name argument",
                            self.path.display(),
                            self.line_no
                        ));
                        return Err(());
                    }

                    remote.name = second.unwrap().to_owned();
                }
                Some("driver") => {
                    if second.is_none() {
                        self.log.error(&format!(
                            "{}:{}: missing driver argument",
                            self.path.display(),
                            self.line_no
                        ));
                        return Err(());
                    }

                    remote.driver = second.unwrap().to_owned();
                }
                Some(name @ "eps")
                | Some(name @ "aeps")
                | Some(name @ "bits")
                | Some(name @ "plead")
                | Some(name @ "ptrail")
                | Some(name @ "pre_data_bits")
                | Some(name @ "pre_data")
                | Some(name @ "post_data_bits")
                | Some(name @ "post_data")
                | Some(name @ "gap")
                | Some(name @ "frequency")
                | Some(name @ "duty_cycle")
                | Some(name @ "toggle_bit")
                | Some(name @ "repeat_bit")
                | Some(name @ "toggle_bit_mask")
                | Some(name @ "rc6_mask") => {
                    let val = self.parse_number_arg(name, second)?;
                    match name {
                        "eps" => remote.eps = val,
                        "aeps" => remote.aeps = val,
                        "bits" => remote.bits = val,
                        "plead" => remote.plead = val,
                        "ptrail" => remote.ptrail = val,
                        "pre_data_bits" => remote.pre_data_bits = val,
                        "pre_data" => remote.pre_data = val,
                        "post_data_bits" => remote.post_data_bits = val,
                        "post_data" => remote.post_data = val,
                        "gap" => remote.gap = val,
                        "frequency" => remote.frequency = val,
                        "duty_cycle" => remote.duty_cycle = val,
                        "toggle_bit_mask" => remote.toggle_bit_mask = val,
                        "toggle_bit" => remote.toggle_bit = val,
                        "repeat_bit" => remote.toggle_bit = val,
                        "rc6_mask" => remote.rc6_mask = val,
                        _ => unreachable!(),
                    }
                }
                Some(name @ "header")
                | Some(name @ "three")
                | Some(name @ "four")
                | Some(name @ "two")
                | Some(name @ "one")
                | Some(name @ "zero")
                | Some(name @ "foot")
                | Some(name @ "repeat")
                | Some(name @ "pre")
                | Some(name @ "post") => {
                    let first = self.parse_number_arg(name, second)?;
                    let second = self.parse_number_arg(name, words.next())?;

                    match name {
                        "header" => remote.header = (first, second),
                        "three" => remote.three = (first, second),
                        "four" => remote.four = (first, second),
                        "two" => remote.two = (first, second),
                        "one" => remote.one = (first, second),
                        "zero" => remote.zero = (first, second),
                        "foot" => remote.foot = (first, second),
                        "repeat" => remote.repeat = (first, second),
                        "pre" => remote.pre = (first, second),
                        "post" => remote.post = (first, second),
                        _ => unreachable!(),
                    }
                }
                Some("flags") => match second {
                    Some(val) => {
                        let mut flags = Flags::empty();

                        for flag in val.split('|') {
                            match flag {
                                "RAW_CODES" => {
                                    flags |= Flags::RAW_CODES;
                                }
                                "RC5" => {
                                    flags |= Flags::RC5;
                                }
                                "SHIFT_ENC" => {
                                    flags |= Flags::SHIFT_ENC;
                                }
                                "RC6" => {
                                    flags |= Flags::RC6;
                                }
                                "RCMM" => {
                                    flags |= Flags::RCMM;
                                }
                                "SPACE_ENC" => {
                                    flags |= Flags::SPACE_ENC;
                                }
                                "SPACE_FIRST" => {
                                    flags |= Flags::SPACE_FIRST;
                                }
                                "GRUNDIG" => {
                                    flags |= Flags::GRUNDIG;
                                }
                                "BO" => {
                                    flags |= Flags::BO;
                                }
                                "SERIAL" => {
                                    flags |= Flags::SERIAL;
                                }
                                "XMP" => {
                                    flags |= Flags::XMP;
                                }
                                "REVERSE" => {
                                    flags |= Flags::REVERSE;
                                }
                                "NO_HEAD_REP" => {
                                    flags |= Flags::NO_HEAD_REP;
                                }
                                "NO_FOOT_REP" => {
                                    flags |= Flags::NO_FOOT_REP;
                                }
                                "CONST_LENGTH" => {
                                    flags |= Flags::CONST_LENGTH;
                                }
                                "REPEAT_HEADER" => {
                                    flags |= Flags::REPEAT_HEADER;
                                }
                                _ => {
                                    self.log.error(&format!(
                                        "{}:{}: unknown flag {}",
                                        self.path.display(),
                                        self.line_no,
                                        flag
                                    ));
                                    return Err(());
                                }
                            }
                        }

                        remote.flags = flags;
                    }
                    None => {
                        self.log.error(&format!(
                            "{}:{}: missing flags argument",
                            self.path.display(),
                            self.line_no
                        ));
                        return Err(());
                    }
                },
                Some("end") => {
                    if let Some("remote") = second {
                        return Ok(remote);
                    }

                    self.log.error(&format!(
                        "{}:{}: expected 'end remote', got '{}'",
                        self.path.display(),
                        self.line_no,
                        line
                    ));

                    return Err(());
                }
                Some("begin") => match second {
                    Some("codes") => {
                        remote.codes = self.read_codes()?;
                    }
                    Some("raw_codes") => {
                        remote.raw_codes = self.read_raw_codes()?;
                    }
                    _ => {
                        self.log.error(&format!(
                            "{}:{}: expected 'begin codes' or 'begin raw_codes', got '{}'",
                            self.path.display(),
                            self.line_no,
                            line
                        ));

                        return Err(());
                    }
                },
                Some(key) => {
                    self.log.error(&format!(
                        "{}:{}: '{}' unexpected",
                        self.path.display(),
                        self.line_no,
                        key
                    ));
                }
                None => (),
            }
        }
    }

    fn parse_number_arg(&self, arg_name: &str, arg: Option<&str>) -> Result<u64, ()> {
        if let Some(val) = arg {
            let no = if let Some(hex) = val.strip_prefix("0x") {
                u64::from_str_radix(hex, 16)
            } else if let Some(hex) = val.strip_prefix("0X") {
                u64::from_str_radix(hex, 16)
            } else {
                u64::from_str(val)
            };

            if let Ok(val) = no {
                Ok(val)
            } else {
                self.log.error(&format!(
                    "{}:{}: {} argument {} is not a number",
                    self.path.display(),
                    self.line_no,
                    arg_name,
                    val
                ));
                Err(())
            }
        } else {
            self.log.error(&format!(
                "{}:{}: missing {} argument",
                self.path.display(),
                self.line_no,
                arg_name
            ));
            Err(())
        }
    }

    fn read_codes(&mut self) -> Result<Vec<LircCode>, ()> {
        let mut codes = Vec::new();

        loop {
            let line = self.next_line()?;

            if line.is_none() {
                self.log.error(&format!(
                    "{}:{}: unexpected end of file",
                    self.path.display(),
                    self.line_no
                ));
                return Err(());
            }

            let line = line.unwrap();

            let mut words = line.split_whitespace();

            let first = words.next();
            let second = words.next();

            match first {
                Some("end") => {
                    if let Some("codes") = second {
                        return Ok(codes);
                    }

                    self.log.error(&format!(
                        "{}:{}: expected 'end codes', got '{}'",
                        self.path.display(),
                        self.line_no,
                        line
                    ));

                    return Err(());
                }
                Some(name) => {
                    if let Some(scancode) = second {
                        match if let Some(hex_scancode) = scancode.strip_prefix("0x") {
                            u64::from_str_radix(hex_scancode, 16)
                        } else if let Some(hex_scancode) = scancode.strip_prefix("0X") {
                            u64::from_str_radix(hex_scancode, 16)
                        } else {
                            u64::from_str(scancode)
                        } {
                            Ok(scancode) => {
                                codes.push(LircCode {
                                    name: name.to_owned(),
                                    code: scancode,
                                });
                            }
                            Err(_) => {
                                self.log.error(&format!(
                                    "{}:{}: scancode '{}' is not valid",
                                    self.path.display(),
                                    self.line_no,
                                    scancode,
                                ));
                                return Err(());
                            }
                        }
                    } else {
                        self.log.error(&format!(
                            "{}:{}: missing scancode",
                            self.path.display(),
                            self.line_no
                        ));
                        return Err(());
                    }
                }
                None => (),
            }
        }
    }

    fn read_raw_codes(&mut self) -> Result<Vec<LircRawCode>, ()> {
        let mut raw_codes = Vec::new();
        let mut raw_code = None;

        loop {
            let line = self.next_line()?;

            if line.is_none() {
                self.log.error(&format!(
                    "{}:{}: unexpected end of file",
                    self.path.display(),
                    self.line_no
                ));
                return Err(());
            }

            let line = line.unwrap();

            let mut words = line.split_whitespace();

            match words.next() {
                Some("end") => {
                    if let Some("raw_codes") = words.next() {
                        if let Some(raw_code) = raw_code {
                            raw_codes.push(raw_code);
                        }
                        return Ok(raw_codes);
                    }

                    self.log.error(&format!(
                        "{}:{}: expected 'end raw_codes', got '{}'",
                        self.path.display(),
                        self.line_no,
                        line,
                    ));

                    return Err(());
                }
                Some("name") => {
                    if let Some(name) = words.next() {
                        if let Some(raw_code) = raw_code {
                            raw_codes.push(raw_code);
                        }

                        raw_code = Some(LircRawCode {
                            name: name.to_owned(),
                            rawir: self.read_lengths(words)?,
                        });
                    } else {
                        self.log.error(&format!(
                            "{}:{}: missing name",
                            self.path.display(),
                            self.line_no
                        ));
                        return Err(());
                    }
                }
                Some(v) => {
                    if let Some(raw_code) = &mut raw_code {
                        let codes = self.read_lengths(line.split_whitespace())?;

                        raw_code.rawir.extend(codes);
                    } else {
                        self.log.error(&format!(
                            "{}:{}: '{}' not expected",
                            self.path.display(),
                            self.line_no,
                            v
                        ));
                        return Err(());
                    }
                }
                None => (),
            }
        }
    }

    fn read_lengths(&self, words: SplitWhitespace) -> Result<Vec<u32>, ()> {
        let mut rawir = Vec::new();

        for no in words {
            if no.starts_with('#') {
                break;
            }

            match u32::from_str(no) {
                Ok(v) => rawir.push(v),
                Err(_) => {
                    self.log.error(&format!(
                        "{}:{}: invalid duration '{}'",
                        self.path.display(),
                        self.line_no,
                        no
                    ));
                    return Err(());
                }
            }
        }

        Ok(rawir)
    }

    fn next_line(&mut self) -> Result<Option<String>, ()> {
        loop {
            let line = self.lines.next();

            if line.is_none() {
                return Ok(None);
            }

            let line = line.unwrap();

            if let Err(err) = line {
                self.log.error(&format!(
                    "failed to read '{}' line {}: {}",
                    self.path.display(),
                    self.line_no,
                    err
                ));
                return Err(());
            }

            self.line_no += 1;

            let line = line.unwrap();

            let trimmed = line.trim();

            if !trimmed.is_empty() && !line.starts_with('#') {
                return Ok(Some(trimmed.to_owned()));
            }
        }
    }

    /// Do some sanity checks and cleanups. Returns false for invalid
    fn sanity_checks(&self, remote: &mut LircRemote) -> bool {
        if remote.name.is_empty() {
            self.log.error(&format!(
                "{}:{}: missing remote name",
                self.path.display(),
                self.line_no,
            ));
            return false;
        }

        if remote.gap == 0 {
            self.log.warning(&format!(
                "{}:{}: missing gap",
                self.path.display(),
                self.line_no,
            ));
        }

        if remote.flags.contains(Flags::RAW_CODES) {
            if remote.raw_codes.is_empty() {
                self.log.error(&format!(
                    "{}:{}: missing raw codes",
                    self.path.display(),
                    self.line_no,
                ));
                return false;
            }

            if !remote.codes.is_empty() {
                self.log.error(&format!(
                    "{}:{}: non-raw codes specified for raw remote",
                    self.path.display(),
                    self.line_no,
                ));
                return false;
            }

            return true;
        }

        if !remote.raw_codes.is_empty() {
            self.log.error(&format!(
                "{}:{}: raw codes specified for non-raw remote",
                self.path.display(),
                self.line_no,
            ));
            return false;
        }

        if remote.codes.is_empty() {
            self.log.error(&format!(
                "{}:{}: missing codes",
                self.path.display(),
                self.line_no,
            ));
            return false;
        }

        // Can we generate a sensible irp for this remote
        if (remote.zero.0 == 0 && remote.zero.1 == 0) || (remote.one.0 == 0 && remote.one.1 == 0) {
            self.log.error(&format!(
                "{}:{}: no bit encoding provided",
                self.path.display(),
                self.line_no,
            ));
            return false;
        }

        if (remote.pre_data & !gen_mask(remote.pre_data_bits)) != 0 {
            self.log.warning(&format!(
                "{}:{}: invalid pre_data",
                self.path.display(),
                self.line_no,
            ));
            remote.pre_data &= gen_mask(remote.pre_data_bits);
        }

        if (remote.post_data & !gen_mask(remote.post_data_bits)) != 0 {
            self.log.warning(&format!(
                "{}:{}: invalid post_data",
                self.path.display(),
                self.line_no,
            ));
            remote.post_data &= gen_mask(remote.post_data_bits);
        }

        for code in &mut remote.codes {
            if (code.code & !gen_mask(remote.bits)) != 0 {
                self.log.warning(&format!(
                    "{}:{}: invalid code {:x}",
                    self.path.display(),
                    self.line_no,
                    code.code
                ));
                code.code &= gen_mask(remote.bits);
            }
        }

        true
    }
}

fn gen_mask(v: u64) -> u64 {
    (1u64 << v) - 1
}
