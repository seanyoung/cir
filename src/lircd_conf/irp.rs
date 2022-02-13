use super::LircRemote;
use crate::log::Log;
/// Build an IRP representation for the remote. This can be used both for encoding
/// and decoding.
impl LircRemote {
    #[allow(clippy::result_unit_err)]
    pub fn irp(&self, log: &Log) -> Result<String, ()> {
        let mut irp = String::from("{");

        if self.frequency != 0 {
            irp.push_str(&format!("{}k,", self.frequency as f64 / 1000f64));
        }

        if self.duty_cycle != 0 {
            if self.duty_cycle >= 99 {
                log.error(&format!(
                    "remote {} duty_cycle {} is invalid",
                    self.name, self.duty_cycle
                ));
            } else {
                irp.push_str(&format!("{}%,", self.duty_cycle));
            }
        }

        irp.push_str("msb}");

        irp.push_str(&format!(
            "<{},-{}|{},-{}>(",
            self.zero.0, self.zero.1, self.one.0, self.one.1
        ));

        if self.header.0 != 0 && self.header.1 != 0 {
            irp.push_str(&format!("{},-{},", self.header.0, self.header.1));
        }

        if self.plead != 0 {
            irp.push_str(&format!("{},", self.plead));
        }

        if self.pre_data_bits != 0 {
            irp.push_str(&format!("{}:{},", self.pre_data, self.pre_data_bits));

            if self.pre.0 != 0 && self.pre.1 != 0 {
                irp.push_str(&format!("{},-{},", self.pre.0, self.pre.1));
            }
        }

        irp.push_str(&format!("CODE:{},", self.bits));

        if self.post_data_bits != 0 {
            irp.push_str(&format!("{}:{},", self.post_data, self.post_data_bits));

            if self.post.0 != 0 && self.post.1 != 0 {
                irp.push_str(&format!("{},-{},", self.post.0, self.post.1));
            }
        }

        if self.ptrail != 0 {
            irp.push_str(&format!("{},", self.ptrail));
        }

        if self.foot.0 != 0 && self.foot.1 != 0 {
            irp.push_str(&format!("{},-{},", self.foot.0, self.foot.1));
        }

        if self.gap != 0 {
            irp.push_str(&format!("^{},", self.gap));
        }

        if self.repeat.0 != 0 && self.repeat.1 != 0 {
            irp.push_str(&format!("({},-{},", self.repeat.0, self.repeat.1));
            if self.ptrail != 0 {
                irp.push_str(&format!("{},", self.ptrail));
            }
            irp.pop();
            irp.push_str(")*)");
        } else {
            irp.pop();
            irp.push_str(")*");
        }

        irp.push_str(&format!(" [CODE:0..{}]", (1u64 << self.bits) - 1));

        Ok(irp)
    }
}
