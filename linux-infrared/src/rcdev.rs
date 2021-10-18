// Get a list of rc devices from sysfs

use std::path::Path;
use std::{fs, io};

#[derive(Debug, Default)]
pub struct Rcdev {
    pub name: String,
    pub device_name: String,
    pub driver: String,
    pub default_keymap: String,
    pub lircdev: Option<String>,
    pub inputdev: Option<String>,
    pub supported_protocols: Vec<String>,
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
                        for entry in fs::read_dir(&entry.path())? {
                            let entry = entry?;
                            if let Some(file_name) = entry.file_name().to_str() {
                                if file_name.starts_with("event") {
                                    let uevent = read_uevent(&entry.path())?;

                                    inputdev = Some(format!("/dev/{}", uevent.dev_name));
                                }
                            }
                        }
                    } else if file_name == "protocols" {
                        for protocol in fs::read_to_string(&entry.path())?.split_whitespace() {
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
                name = value.to_owned();
            }
            Some(("DRV_NAME", value)) => {
                drv_name = value.to_owned();
            }
            Some(("DEVNAME", value)) | Some(("DEV_NAME", value)) => {
                dev_name = value.to_owned();
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
