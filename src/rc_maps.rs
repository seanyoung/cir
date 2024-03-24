//! Parse /etc/rc_maps.cfg for Linux. This file configures the default keymap
//! to load on Linux.

use std::fs::File;
use std::io::{BufRead, BufReader, Error, ErrorKind};
use std::path::Path;

/// Entry for keymap mapping
#[derive(Debug)]
pub struct KeymapTable {
    /// Name the driver to match ("*" for any)
    pub driver: String,
    /// Name of the default keymap to match ("*" for any)
    pub table: String,
    /// Path of keymap to load
    pub file: String,
}

/// Parse /etc/rc_maps.cfg
pub fn parse_rc_maps_file(path: &Path) -> Result<Vec<KeymapTable>, Error> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut res = Vec::new();

    for (line_no, line) in reader.lines().enumerate() {
        let line = line?;

        let line = line.trim_start();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let elements: Vec<_> = line.split_whitespace().collect();

        if elements.len() != 3 {
            return Err(Error::new(
                ErrorKind::Other,
                format!(
                    "{}:{}: error: invalid parameters",
                    path.display(),
                    line_no + 1
                ),
            ));
        }

        let driver = elements[0].to_owned();
        let table = elements[1].to_owned();
        let file = elements[2].to_owned();

        res.push(KeymapTable {
            driver,
            table,
            file,
        });
    }

    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::parse_rc_maps_file;
    use crate::rcdev::Rcdev;
    use std::path::PathBuf;

    #[test]
    fn parse_bad() {
        let e = parse_rc_maps_file(&PathBuf::from("testdata/rc_maps_cfg/bad.cfg")).unwrap_err();

        assert_eq!(
            format!("{e}"),
            "testdata/rc_maps_cfg/bad.cfg:4: error: invalid parameters"
        );
    }

    #[test]
    fn parse_good() {
        let t = parse_rc_maps_file(&PathBuf::from("testdata/rc_maps_cfg/ttusbir.cfg")).unwrap();

        assert_eq!(t.len(), 2);

        let rc = Rcdev {
            driver: String::from("ttusbir"),
            default_keymap: String::from("rc-empty"),
            ..Default::default()
        };

        assert!(t[0].matches(&rc));
        assert!(t[1].matches(&rc));

        let rc = Rcdev {
            driver: String::from("ttusbi"),
            default_keymap: String::from("rc-empty"),
            ..Default::default()
        };

        assert!(!t[0].matches(&rc));
        assert!(t[1].matches(&rc));
    }
}
