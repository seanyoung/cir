pub mod ast;
pub mod render;

#[cfg(test)]
mod tests;

use ast::*;

#[allow(clippy::all,unused_parens)]
#[cfg_attr(rustfmt, rustfmt_skip)]
pub mod irp;

pub fn box_option<T>(o: Option<T>) -> Option<Box<T>> {
    match o {
        None => None,
        Some(x) => Some(Box::new(x)),
    }
}

pub fn parse(input: &str) -> Result<GeneralSpec, String> {
    let parser = irp::protocolParser::new();

    match parser.parse(input) {
        Ok(s) => general_spec(&s.general_spec),
        Err(r) => Err(r.to_string()),
    }
}

#[derive(Debug)]
pub struct GeneralSpec {
    duty_cycle: Option<u8>,
    carrier: Option<i64>,
    lsb: bool,
    unit: f64,
}

fn general_spec(general_spec: &[GeneralItem]) -> Result<GeneralSpec, String> {
    let mut res = GeneralSpec {
        duty_cycle: None,
        carrier: None,
        lsb: true,
        unit: 1.0,
    };

    let mut unit = None;
    let mut lsb = None;

    for i in general_spec {
        match i {
            GeneralItem::DutyCycle(d) => {
                if *d < 1.0 {
                    return Err("duty cycle less than 1% not valid".to_string());
                }
                if *d > 99.0 {
                    return Err("duty cycle larger than 99% not valid".to_string());
                }
                if res.duty_cycle.is_some() {
                    return Err("duty cycle specified twice".to_string());
                }

                res.duty_cycle = Some(*d as u8);
            }
            GeneralItem::Frequency(f) => {
                if res.carrier.is_some() {
                    return Err("carrier frequency specified twice".to_string());
                }

                res.carrier = Some((*f * 1000.0) as i64);
            }
            GeneralItem::OrderLsb | GeneralItem::OrderMsb => {
                if lsb.is_some() {
                    return Err("bit order (lsb,msb) specified twice".to_string());
                }

                lsb = Some(*i == GeneralItem::OrderLsb);
            }
            GeneralItem::Unit(p, u) => {
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
                    p * 1_000_000.0 / f as f64
                } else {
                    return Err("pulse unit specified without carrier frequency".to_string());
                }
            }
            Unit::Milliseconds => p * 1000.0,
            Unit::Units | Unit::Microseconds => *p,
        }
    }

    if Some(false) == lsb {
        res.lsb = false;
    }
    Ok(res)
}
