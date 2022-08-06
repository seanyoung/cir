use super::{Code, Flags, RawCode, Remote};
use log::{debug, error, warn};
use std::num::ParseIntError;
use std::str::Lines;
use std::{
    fs::OpenOptions,
    io::Read,
    path::{Path, PathBuf},
    str::{FromStr, SplitWhitespace},
};

pub struct LircParser<'a> {
    path: PathBuf,
    line_no: u32,
    lines: Lines<'a>,
}

/// We need a custom parser for lircd.conf files, because the parser in lircd itself
/// is custom and permits all sorts which a proper parser would not. For example,
/// garbage is permitted when 'begin remote' is expected, and most lines can have
/// trailing characters after the first two tokens.
impl<'a> LircParser<'a> {
    pub fn parse(path: &Path) -> Result<Vec<Remote>, ()> {
        let mut file = OpenOptions::new()
            .read(true)
            .open(path)
            .map_err(|e| error!("failed to open ‘{}’: {}", path.display(), e))?;

        let mut buf = Vec::new();
        file.read_to_end(&mut buf)
            .map_err(|e| error!("failed to read ‘{}’: {}", path.display(), e))?;

        let contents = String::from_utf8_lossy(&buf);

        // strip bom
        let contents = if let Some(contents) = contents.strip_prefix('\u{feff}') {
            contents
        } else {
            &contents
        };

        let lines = contents.lines();

        debug!("parsing ‘{}’ as lircd.conf file", path.display());

        let mut parser = LircParser {
            path: PathBuf::from(path),
            line_no: 0,
            lines,
        };

        parser.read()
    }

    fn read(&mut self) -> Result<Vec<Remote>, ()> {
        let mut remotes = Vec::new();

        loop {
            let line = self.next_line();

            if line.is_none() {
                return if remotes.is_empty() {
                    error!("{}: no remote definitions found", self.path.display());
                    Err(())
                } else {
                    Ok(remotes)
                };
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
            } else if let Some(first) = first {
                if !first.starts_with('#') {
                    warn!(
                        "{}:{}: expected 'begin remote', got ‘{}’",
                        self.path.display(),
                        self.line_no,
                        line
                    );
                }
            }
        }
    }

    fn read_remote(&mut self) -> Result<Remote, ()> {
        let mut remote = Remote {
            frequency: 38000,
            ..Default::default()
        };

        loop {
            let line = self.next_line();

            if line.is_none() {
                error!(
                    "{}:{}: unexpected end of file",
                    self.path.display(),
                    self.line_no
                );
                return Err(());
            }

            let line = line.unwrap();

            let mut words = line.split_whitespace();

            let first = words.next();
            let second = words.next();

            match first {
                Some("name") => {
                    if second.is_none() {
                        error!(
                            "{}:{}: missing name argument",
                            self.path.display(),
                            self.line_no
                        );
                        return Err(());
                    }

                    remote.name = second.unwrap().to_owned();
                }
                Some("driver") => {
                    if second.is_none() {
                        error!(
                            "{}:{}: missing driver argument",
                            self.path.display(),
                            self.line_no
                        );
                        return Err(());
                    }

                    remote.driver = second.unwrap().to_owned();
                }
                Some("serial_mode") => {
                    if second.is_none() {
                        error!(
                            "{}:{}: missing serial_mode argument",
                            self.path.display(),
                            self.line_no
                        );
                        return Err(());
                    }

                    remote.serial_mode = second.unwrap().to_owned();
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
                | Some(name @ "frequency")
                | Some(name @ "duty_cycle")
                | Some(name @ "min_repeat")
                | Some(name @ "toggle_bit")
                | Some(name @ "repeat_bit")
                | Some(name @ "toggle_bit_mask")
                | Some(name @ "toggle_mask")
                | Some(name @ "rc6_mask")
                | Some(name @ "baud")
                | Some(name @ "repeat_gap")
                | Some(name @ "suppress_repeat")
                | Some(name @ "manual_sort")
                | Some(name @ "min_code_repeat")
                | Some(name @ "ignore_mask") => {
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
                        "frequency" => remote.frequency = val,
                        "duty_cycle" => remote.duty_cycle = val,
                        "min_repeat" => remote.min_repeat = val,
                        "toggle_bit_mask" => remote.toggle_bit_mask = val,
                        "toggle_bit" => remote.toggle_bit = val,
                        "repeat_bit" => remote.toggle_bit = val,
                        "toggle_mask" => remote.toggle_mask = val,
                        "rc6_mask" => remote.rc6_mask = val,
                        "baud" => remote.baud = val,
                        "repeat_gap" => remote.repeat_gap = val,
                        "suppress_repeat" => remote.suppress_repeat = val,
                        "manual_sort" => remote.manual_sort = val,
                        "min_code_repeat" => remote.min_code_repeat = val,
                        "ignore_mask" => remote.ignore_mask = val,
                        _ => unreachable!(),
                    }
                }
                Some(name @ "gap") => {
                    remote.gap = self.parse_number_arg(name, second)?;

                    let gap2 = words.next();
                    if gap2.is_some() {
                        remote.gap2 = self.parse_number_arg(name, gap2)?;
                    }
                }
                Some(name @ "header")
                | Some(name @ "three")
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
                        "three" => remote.bit[3] = (first, second),
                        "two" => remote.bit[2] = (first, second),
                        "one" => remote.bit[1] = (first, second),
                        "zero" => remote.bit[0] = (first, second),
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
                                    error!(
                                        "{}:{}: unknown flag {}",
                                        self.path.display(),
                                        self.line_no,
                                        flag
                                    );
                                    return Err(());
                                }
                            }
                        }

                        remote.flags = flags;
                    }
                    None => {
                        error!(
                            "{}:{}: missing flags argument",
                            self.path.display(),
                            self.line_no
                        );
                        return Err(());
                    }
                },
                Some("end") => {
                    if let Some("remote") = second {
                        return Ok(remote);
                    }

                    error!(
                        "{}:{}: expected 'end remote', got ‘{}’",
                        self.path.display(),
                        self.line_no,
                        line
                    );

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
                        error!(
                            "{}:{}: expected 'begin codes' or 'begin raw_codes', got ‘{}’",
                            self.path.display(),
                            self.line_no,
                            line
                        );

                        return Err(());
                    }
                },
                Some(key) => {
                    error!(
                        "{}:{}: ‘{}’ unexpected",
                        self.path.display(),
                        self.line_no,
                        key
                    );
                }
                None => (),
            }
        }
    }

    fn parse_number_arg(&self, arg_name: &str, arg: Option<&str>) -> Result<u64, ()> {
        if let Some(val) = arg {
            if let Ok(val) = parse_number(val) {
                Ok(val)
            } else {
                error!(
                    "{}:{}: {} argument {} is not a number",
                    self.path.display(),
                    self.line_no,
                    arg_name,
                    val
                );
                Err(())
            }
        } else {
            error!(
                "{}:{}: missing {} argument",
                self.path.display(),
                self.line_no,
                arg_name
            );
            Err(())
        }
    }

    fn read_codes(&mut self) -> Result<Vec<Code>, ()> {
        let mut codes = Vec::new();

        loop {
            let line = self.next_line();

            if line.is_none() {
                error!(
                    "{}:{}: unexpected end of file",
                    self.path.display(),
                    self.line_no
                );
                return Err(());
            }

            let line = line.unwrap();

            let mut words = line.split_whitespace();

            match words.next() {
                Some("end") => {
                    if let Some("codes") = words.next() {
                        return Ok(codes);
                    }

                    error!(
                        "{}:{}: expected 'end codes', got ‘{}’",
                        self.path.display(),
                        self.line_no,
                        line
                    );

                    return Err(());
                }
                Some(name) => {
                    let mut values = Vec::new();

                    for code in words {
                        if code.starts_with('#') {
                            break;
                        }

                        match parse_number(code) {
                            Ok(code) => {
                                values.push(code);
                            }
                            Err(_) => {
                                error!(
                                    "{}:{}: code ‘{}’ is not valid",
                                    self.path.display(),
                                    self.line_no,
                                    code,
                                );
                                return Err(());
                            }
                        }
                    }

                    if values.is_empty() {
                        error!("{}:{}: missing code", self.path.display(), self.line_no);
                        return Err(());
                    }

                    let dup = codes.iter().any(|c| c.name == name);

                    codes.push(Code {
                        name: name.to_owned(),
                        dup,
                        code: values,
                    });
                }
                None => (),
            }
        }
    }

    fn read_raw_codes(&mut self) -> Result<Vec<RawCode>, ()> {
        let mut raw_codes = Vec::new();
        let mut raw_code = None;

        loop {
            let line = self.next_line();

            if line.is_none() {
                error!(
                    "{}:{}: unexpected end of file",
                    self.path.display(),
                    self.line_no
                );
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

                    error!(
                        "{}:{}: expected 'end raw_codes', got ‘{}’",
                        self.path.display(),
                        self.line_no,
                        line,
                    );

                    return Err(());
                }
                Some("name") => {
                    if let Some(name) = words.next() {
                        if let Some(raw_code) = raw_code {
                            raw_codes.push(raw_code);
                        }

                        let dup = raw_codes.iter().any(|c| c.name == name);

                        raw_code = Some(RawCode {
                            name: name.to_owned(),
                            dup,
                            rawir: self.read_lengths(words)?,
                        });
                    } else {
                        error!("{}:{}: missing name", self.path.display(), self.line_no);
                        return Err(());
                    }
                }
                Some(v) => {
                    if let Some(raw_code) = &mut raw_code {
                        let codes = self.read_lengths(line.split_whitespace())?;

                        raw_code.rawir.extend(codes);
                    } else {
                        error!(
                            "{}:{}: ‘{}’ not expected",
                            self.path.display(),
                            self.line_no,
                            v
                        );
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

            match parse_number(no) {
                Ok(v) if v <= u32::MAX as u64 => rawir.push(v as u32),
                _ => {
                    error!(
                        "{}:{}: invalid duration ‘{}’",
                        self.path.display(),
                        self.line_no,
                        no
                    );
                    return Err(());
                }
            }
        }

        Ok(rawir)
    }

    fn next_line(&mut self) -> Option<String> {
        loop {
            let line = self.lines.next()?;

            self.line_no += 1;

            let trimmed = line.trim();

            if !trimmed.is_empty() && !line.starts_with('#') {
                return Some(trimmed.to_owned());
            }
        }
    }

    /// Do some sanity checks and cleanups. Returns false for invalid
    fn sanity_checks(&self, remote: &mut Remote) -> bool {
        if remote.name.is_empty() {
            error!(
                "{}:{}: missing remote name",
                self.path.display(),
                self.line_no,
            );
            return false;
        }

        if remote.gap == 0 {
            warn!("{}:{}: missing gap", self.path.display(), self.line_no,);
        }

        if remote.duty_cycle > 99 {
            warn!(
                "{}:{}: duty_cycle {} is not valid",
                self.path.display(),
                self.line_no,
                remote.duty_cycle
            );
            remote.duty_cycle = 0;
        }

        if !remote.raw_codes.is_empty() {
            remote.flags.set(Flags::RAW_CODES, true);

            if !remote.codes.is_empty() {
                error!(
                    "{}:{}: non-raw codes specified for raw remote",
                    self.path.display(),
                    self.line_no,
                );
                return false;
            }

            return true;
        }

        if remote.flags.contains(Flags::RAW_CODES) {
            error!(
                "{}:{}: missing raw codes",
                self.path.display(),
                self.line_no,
            );
            return false;
        }

        if remote.codes.is_empty() {
            error!("{}:{}: missing codes", self.path.display(), self.line_no,);
            return false;
        }

        // Can we generate a sensible irp for this remote
        if (remote.bit[0].0 == 0 && remote.bit[0].1 == 0)
            || (remote.bit[1].0 == 0 && remote.bit[1].1 == 0)
        {
            error!(
                "{}:{}: no bit encoding provided",
                self.path.display(),
                self.line_no,
            );
            return false;
        }

        if (remote.pre_data & !gen_mask(remote.pre_data_bits)) != 0 {
            warn!("{}:{}: invalid pre_data", self.path.display(), self.line_no,);
            remote.pre_data &= gen_mask(remote.pre_data_bits);
        }

        if (remote.post_data & !gen_mask(remote.post_data_bits)) != 0 {
            warn!(
                "{}:{}: invalid post_data",
                self.path.display(),
                self.line_no,
            );
            remote.post_data &= gen_mask(remote.post_data_bits);
        }

        for code in &mut remote.codes {
            for code in &mut code.code {
                if (*code & !gen_mask(remote.bits)) != 0 {
                    warn!(
                        "{}:{}: invalid code 0x{:x} truncated",
                        self.path.display(),
                        self.line_no,
                        code
                    );
                    *code &= gen_mask(remote.bits);
                }
            }
        }

        if remote.flags.contains(Flags::REVERSE) {
            if remote.pre_data_bits > 0 {
                remote.pre_data = reverse(remote.pre_data, remote.pre_data_bits);
            }

            if remote.post_data_bits > 0 {
                remote.post_data = reverse(remote.post_data, remote.post_data_bits);
            }

            for code in &mut remote.codes {
                for code in &mut code.code {
                    *code = reverse(*code, remote.bits)
                }
            }
        }

        if remote.flags.contains(Flags::RC6) && remote.rc6_mask == 0 && remote.toggle_bit > 0 {
            remote.rc6_mask = 1u64 << (remote.all_bits() - remote.toggle_bit);
        }

        if remote.toggle_bit > 0 {
            if remote.toggle_bit_mask > 0 {
                warn!(
                    "{}:{}: remote {} uses both toggle_bit and toggle_bit_mask",
                    self.path.display(),
                    self.line_no,
                    remote.name
                );
            } else {
                remote.toggle_bit_mask = 1u64 << (remote.all_bits() - remote.toggle_bit);
            }
            remote.toggle_bit = 0;
        }

        true
    }
}

fn gen_mask(v: u64) -> u64 {
    (1u64 << v) - 1
}

fn reverse(val: u64, bits: u64) -> u64 {
    let mut res = 0u64;
    let mut val = val;

    for _ in 0..bits {
        res <<= 1;
        res |= val & 1;
        val >>= 1;
    }

    res
}

/// Parse a number like strtoull in lirc daemon
fn parse_number(val: &str) -> Result<u64, ParseIntError> {
    if let Some(hex) = val.strip_prefix("0x") {
        u64::from_str_radix(hex, 16)
    } else if let Some(hex) = val.strip_prefix("0X") {
        u64::from_str_radix(hex, 16)
    } else if let Some(oct) = val.strip_prefix('0') {
        if oct.is_empty() {
            Ok(0)
        } else {
            u64::from_str_radix(oct, 8)
        }
    } else {
        u64::from_str(val)
    }
}
