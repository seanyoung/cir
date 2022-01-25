use super::log::Log;
use std::{
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Lines},
    path::{Path, PathBuf},
    str::{FromStr, SplitWhitespace},
};
mod encode;

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

#[derive(Debug, Default)]
pub struct LircRemote {
    pub name: String,
    pub driver: String,
    pub eps: u32,
    pub aeps: u32,
    pub bits: u32,
    pub plead: u32,
    pub ptrail: u32,
    pub pre_data_bits: u32,
    pub pre_data: u32,
    pub post_data_bits: u32,
    pub post_data: u32,
    pub header: (u32, u32),
    pub one: (u32, u32),
    pub zero: (u32, u32),
    pub gap: u32,
    pub codes: Vec<LircCode>,
    pub raw_codes: Vec<LircRawCode>,
}

struct LircParser<'a> {
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
    fn new(path: &Path, log: &'a Log) -> Result<Self, ()> {
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

    fn parse(&mut self) -> Result<Vec<LircRemote>, ()> {
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
                remotes.push(self.read_remote()?);
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
        let mut remote: LircRemote = Default::default();

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
                Some("eps")
                | Some("aeps")
                | Some("bits")
                | Some("plead")
                | Some("ptrail")
                | Some("pre_data_bits")
                | Some("pre_data")
                | Some("post_data_bits")
                | Some("post_data") => match second {
                    Some(val) => {
                        if let Ok(val) = u32::from_str(val) {
                            match first {
                                Some("eps") => remote.eps = val,
                                Some("aeps") => remote.aeps = val,
                                Some("bits") => remote.bits = val,
                                Some("plead") => remote.plead = val,
                                Some("ptrail") => remote.ptrail = val,
                                Some("pre_data_bits") => remote.pre_data_bits = val,
                                Some("pre_data") => remote.pre_data = val,
                                Some("post_data_bits") => remote.post_data_bits = val,
                                Some("post_data") => remote.post_data = val,
                                _ => unreachable!(),
                            }
                            remote.bits = val;
                        } else {
                            self.log.error(&format!(
                                "{}:{}: bits argument '{}' not valid",
                                self.path.display(),
                                self.line_no,
                                val
                            ));
                        }
                    }
                    None => {
                        self.log.error(&format!(
                            "{}:{}: missing bits argument",
                            self.path.display(),
                            self.line_no
                        ));
                        return Err(());
                    }
                },
                Some("end") => {
                    if let Some("remote") = second {
                        // TODO: sanity check
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

            if !trimmed.is_empty() && !trimmed.starts_with('#') {
                return Ok(Some(trimmed.to_owned()));
            }
        }
    }
}

/// Read a lircd.conf file at the path specified. Such a file may contain multiple
/// remotes. Any parse errors or warnings are send to the log crate.
#[allow(clippy::result_unit_err)]
pub fn parse<P: AsRef<Path>>(path: P, log: &Log) -> Result<Vec<LircRemote>, ()> {
    let mut parser = LircParser::new(path.as_ref(), log)?;

    parser.parse()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parse_all_of_them() {
        let log = Log::new();

        println!(
            "{:?}",
            parse("testdata/lircd_conf/pioneer/CU-VSX107.lircd.conf", &log)
        );
    }
}
