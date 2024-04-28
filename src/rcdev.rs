//! Get a list of remote controller devices from sysfs on linux. A remote
//! controller is either an infrared receiver/transmitter or a cec interface.

use crate::rc_maps::KeymapTable;
use itertools::Itertools;
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::Path,
};

/// Single remote controller device on linux (either infrared or cec device)
#[derive(Debug, Default, Clone)]
pub struct Rcdev {
    /// Name of rc. This is usually "rc" followed by a number
    pub name: String,
    /// Name of the actual device. Human readable
    pub device_name: String,
    /// Name of the driver
    pub driver: String,
    /// Default keymap name for this device
    pub default_keymap: String,
    /// Path to lirc device, if any. Device may be cec or kernel can be
    /// compiled without lirc chardevs
    pub lircdev: Option<String>,
    /// Path to input device. Transmitters do not have an input device attached
    pub inputdev: Option<String>,
    /// Supported protocols. Will be a single "cec" entry for cec devices
    pub supported_protocols: Vec<String>,
    /// Which protocols are enabled. This indexes into supported_protocols
    pub enabled_protocols: Vec<usize>,
}

/// Get a list of rc devices attached to the system. If none are present, not found error maybe returned
pub fn enumerate_rc_dev() -> io::Result<Vec<Rcdev>> {
    let mut rcdev = Vec::new();

    for entry in fs::read_dir("/sys/class/rc")? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let uevent = read_uevent(&path)?;
            let mut lircdev = None;
            let mut inputdev = None;
            let mut supported_protocols = Vec::new();
            let mut enabled_protocols = Vec::new();

            for entry in fs::read_dir(path)? {
                let entry = entry?;
                if let Some(file_name) = entry.file_name().to_str() {
                    if file_name.starts_with("lirc") {
                        let uevent = read_uevent(&entry.path())?;

                        lircdev = Some(format!("/dev/{}", uevent.dev_name));
                    } else if file_name.starts_with("input") {
                        for entry in fs::read_dir(entry.path())? {
                            let entry = entry?;
                            if let Some(file_name) = entry.file_name().to_str() {
                                if file_name.starts_with("event") {
                                    let uevent = read_uevent(&entry.path())?;

                                    inputdev = Some(format!("/dev/{}", uevent.dev_name));
                                }
                            }
                        }
                    } else if file_name == "protocols" {
                        for protocol in fs::read_to_string(entry.path())?.split_whitespace() {
                            if protocol.starts_with('[') && protocol.ends_with(']') {
                                let protocol = &protocol[1..protocol.len() - 1];
                                if protocol == "lirc" {
                                    // The kernel always outputs this entry for compatibility
                                    continue;
                                }
                                enabled_protocols.push(supported_protocols.len());
                                supported_protocols.push(protocol.to_owned());
                            } else {
                                supported_protocols.push(protocol.to_owned());
                            }
                        }
                    }
                }
            }

            rcdev.push(Rcdev {
                name: entry.file_name().to_str().unwrap().to_owned(),
                device_name: uevent.dev_name,
                driver: uevent.drv_name,
                default_keymap: uevent.name,
                inputdev,
                lircdev,
                enabled_protocols,
                supported_protocols,
            })
        }
    }

    // Sort the list by name
    rcdev.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(rcdev)
}

struct UEvent {
    name: String,
    drv_name: String,
    dev_name: String,
}

fn read_uevent(path: &Path) -> io::Result<UEvent> {
    let mut name = String::new();
    let mut drv_name = String::new();
    let mut dev_name = String::new();

    for line in fs::read_to_string(path.join("uevent"))?.lines() {
        match line.split_once('=') {
            Some(("NAME", value)) => {
                value.clone_into(&mut name);
            }
            Some(("DRV_NAME", value)) => {
                value.clone_into(&mut drv_name);
            }
            Some(("DEVNAME", value)) | Some(("DEV_NAME", value)) => {
                value.clone_into(&mut dev_name);
            }
            _ => (),
        }
    }

    Ok(UEvent {
        name,
        drv_name,
        dev_name,
    })
}

impl KeymapTable {
    pub fn matches(&self, rcdev: &Rcdev) -> bool {
        (self.driver == "*" || self.driver == rcdev.driver)
            && (self.table == "*" || self.table == rcdev.default_keymap)
    }
}

impl Rcdev {
    pub fn set_enabled_protocols(&mut self, protocols: &[usize]) -> Result<(), String> {
        let string = if protocols.is_empty() {
            "none".into()
        } else {
            protocols
                .iter()
                .map(|i| self.supported_protocols[*i].as_str())
                .join(" ")
        };

        let path = format!("/sys/class/rc/{}/protocols", self.name);

        let mut f = OpenOptions::new()
            .write(true)
            .open(&path)
            .map_err(|e| format!("{path}: {e}"))?;

        f.write_all(string.as_bytes())
            .map_err(|e| format!("{path}: {e}"))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::rc_maps::parse_rc_maps_file;
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
