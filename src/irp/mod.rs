pub mod ast;
pub mod render;

use ast::*;

#[allow(clippy::all,unused_parens)]
#[cfg_attr(rustfmt, rustfmt_skip)]
pub mod irp;

#[derive(Debug)]
pub struct Irp {
    duty_cycle: Option<u8>,
    carrier: Option<f64>,
    lsb: bool,
    unit: f64,
}

pub fn parse(input: &str) -> Result<Irp, String> {
    let parser = irp::protocolParser::new();

    match parser.parse(input) {
        Ok(s) => {
            let mut res = Irp {
                duty_cycle: None,
                carrier: None,
                lsb: true,
                unit: 1.0,
            };

            let mut unit = None;
            let mut lsb = None;

            for i in s.general_spec {
                match i {
                    ast::GeneralItem::DutyCycle(d) => {
                        if d < 1.0 {
                            return Err("duty cycle less than 1% not valid".to_string());
                        }
                        if d > 99.0 {
                            return Err("duty cycle larger than 99% not valid".to_string());
                        }
                        if res.duty_cycle.is_some() {
                            return Err("duty cycle specified twice".to_string());
                        }

                        res.duty_cycle = Some(d as u8);
                    }
                    ast::GeneralItem::Frequency(f) => {
                        if res.carrier.is_some() {
                            return Err("carrier frequency specified twice".to_string());
                        }

                        res.carrier = Some(f);
                    }
                    ast::GeneralItem::OrderLsb | ast::GeneralItem::OrderMsb => {
                        if lsb.is_some() {
                            return Err("bit order (lsb,msb) specified twice".to_string());
                        }

                        lsb = Some(i == ast::GeneralItem::OrderLsb);
                    }
                    ast::GeneralItem::Unit(p, u) => {
                        if unit.is_some() {
                            return Err("unit specified twice".to_string());
                        }

                        unit = Some((p, u));
                    }
                }
            }

            if let Some((p, u)) = unit {
                res.unit = match u {
                    Unit::Pulses => {
                        if let Some(f) = res.carrier {
                            p * 1000.0 / f
                        } else {
                            return Err(
                                "pulse unit specified without carrier frequency".to_string()
                            );
                        }
                    }
                    Unit::Milliseconds => p * 1000.0,
                    Unit::Microseconds => p,
                }
            }

            if Some(false) == lsb {
                res.lsb = false;
            }
            Ok(res)
        }
        Err(r) => Err(r.to_string()),
    }
}
