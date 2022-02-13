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

    irp.push_str("msb}");

    irp.push_str(&format!(
        "<{},-{}|{},-{}>(",
        remote.zero.0, remote.zero.1, remote.one.0, remote.one.1
    ));

    if remote.header.0 != 0 && remote.header.1 != 0 {
        irp.push_str(&format!("{},-{},", remote.header.0, remote.header.1));
    }

    if remote.plead != 0 {
        irp.push_str(&format!("{},", remote.plead));
    }

    if remote.pre_data_bits != 0 {
        irp.push_str(&format!("{}:{},", remote.pre_data, remote.pre_data_bits));

        if remote.pre.0 != 0 && remote.pre.1 != 0 {
            irp.push_str(&format!("{},-{},", remote.pre.0, remote.pre.1));
        }
    }

    irp.push_str(&format!("CODE:{},", remote.bits));

    if remote.post_data_bits != 0 {
        irp.push_str(&format!("{}:{},", remote.post_data, remote.post_data_bits));

        if remote.post.0 != 0 && remote.post.1 != 0 {
            irp.push_str(&format!("{},-{},", remote.post.0, remote.post.1));
        }
    }

    if remote.ptrail != 0 {
        irp.push_str(&format!("{},", remote.ptrail));
    }

    if remote.foot.0 != 0 && remote.foot.1 != 0 {
        irp.push_str(&format!("{},-{},", remote.foot.0, remote.foot.1));
    }

    if remote.gap != 0 {
        irp.push_str(&format!("^{},", remote.gap));
    }

    if remote.repeat.0 != 0 && remote.repeat.1 != 0 {
        irp.push_str(&format!("({},-{},", remote.repeat.0, remote.repeat.1));
        if remote.ptrail != 0 {
            irp.push_str(&format!("{},", remote.ptrail));
        }
        irp.pop();
        irp.push_str(")*)");
    } else {
        irp.pop();
        irp.push_str(")*");
    }

    irp.push_str(&format!(" [CODE:0..{}]", (1 << remote.bits) - 1));

    Ok(irp)
}
