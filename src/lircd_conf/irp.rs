use super::LircRemote;
use crate::log::Log;

/// Build an IRP representation for the remote. This can be used both for encoding
/// and decoding.
pub fn convert_to_irp(remote: &LircRemote, log: &Log) -> Result<String, ()> {
    let mut irp = String::from("{");
    if remote.frequency != 0 {
        irp.push_str(&format!("{}k,", remote.frequency as f64 / 1000f64));
    }
    if remote.duty_cycle != 0 {
        if remote.duty_cycle >= 99 {
            log.error(&format!(
                "remote {} duty_cycle {} is invalid",
                remote.name, remote.duty_cycle
            ));
        } else {
            irp.push_str(&format!("{}%,", remote.duty_cycle));
        }
    }
    irp.push_str("lsb}");

    if remote.header.0 != 0 && remote.header.1 != 0 {
        irp.push_str(&format!("({},-{})", remote.header.0, remote.header.1));
    }

    Ok(irp)
}
