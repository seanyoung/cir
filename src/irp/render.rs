use super::ast::*;
use super::irp;
use crate::rawir;

use std::collections::HashMap;

// Here we parse an irp lang

#[derive(Debug)]
pub struct GeneralSpec {
    duty_cycle: Option<u8>,
    carrier: Option<i64>,
    lsb: bool,
    unit: f64,
}

pub struct Vartable {
    vars: HashMap<String, i64>,
}

impl Vartable {
    pub fn new() -> Self {
        Self {
            vars: HashMap::new(),
        }
    }

    pub fn set(&mut self, id: String, v: i64) {
        self.vars.insert(id, v);
    }

    pub fn get(&self, id: &str) -> Result<i64, String> {
        match self.vars.get(id) {
            Some(n) => Ok(*n),
            None => Err(format!("variable {} not defined", id)),
        }
    }
}

struct Output<'a> {
    general_spec: &'a GeneralSpec,
    raw: Vec<u32>,
    extent_marker: Vec<u32>,
}

impl<'a> Output<'a> {
    fn new(gs: &'a GeneralSpec) -> Self {
        Self {
            general_spec: gs,
            raw: Vec::new(),
            extent_marker: Vec::new(),
        }
    }

    fn push_extent_marker(&mut self) {
        self.extent_marker.push(0);
    }

    fn pop_extend_marker(&mut self) {
        self.extent_marker.pop();
    }

    fn add(&mut self, dur: &Duration, vars: &Vartable) -> Result<(), String> {
        match dur {
            Duration::FlashConstant(p, u) => self.add_flash(u.eval(*p as i64, &self.general_spec)?),
            Duration::GapConstant(p, u) => self.add_gap(u.eval(*p as i64, &self.general_spec)?),
            Duration::FlashIdentifier(id, u) => {
                self.add_flash(u.eval(vars.get(id)?, &self.general_spec)?)
            }
            Duration::GapIdentifier(id, u) => {
                self.add_gap(u.eval(vars.get(id)?, &self.general_spec)?)
            }
            Duration::ExtentConstant(p, u) => {
                self.add_extend(u.eval(*p as i64, &self.general_spec)?)
            }
            Duration::ExtentIdentifier(id, u) => {
                self.add_extend(u.eval(vars.get(id)?, &self.general_spec)?)
            }
        }

        Ok(())
    }

    fn add_flash(&mut self, n: i64) {
        assert!(n > 0);

        *self.extent_marker.last_mut().unwrap() += n as u32;

        if (self.raw.len() % 2) == 1 {
            *self.raw.last_mut().unwrap() += n as u32;
        } else {
            self.raw.push(n as u32);
        }
    }

    fn add_gap(&mut self, n: i64) {
        assert!(n > 0);

        *self.extent_marker.last_mut().unwrap() += n as u32;

        let len = self.raw.len();

        if len == 0 {
            // ignore leading gaps
        } else if (len % 2) == 0 {
            *self.raw.last_mut().unwrap() += n as u32;
        } else {
            self.raw.push(n as u32);
        }
    }

    fn add_extend(&mut self, mut extent: i64) {
        extent -= *self.extent_marker.last().unwrap() as i64;

        if extent > 0 {
            self.add_gap(extent);
        }
    }
}

impl Expression {
    fn eval(&self, vars: &Vartable) -> Result<i64, String> {
        match self {
            Expression::Number(n) => Ok(*n),
            Expression::Identifier(id) => vars.get(id),
            Expression::Negative(e) => Ok(-e.eval(vars)?),
            Expression::Complement(e) => Ok(!e.eval(vars)?),
            Expression::Add(l, r) => Ok(l.eval(vars)? + r.eval(vars)?),
            Expression::Subtract(l, r) => Ok(l.eval(vars)? - r.eval(vars)?),
            Expression::Multiply(l, r) => Ok(l.eval(vars)? * r.eval(vars)?),
            Expression::Divide(l, r) => Ok(l.eval(vars)? / r.eval(vars)?),
            Expression::Modulo(l, r) => Ok(l.eval(vars)? % r.eval(vars)?),
            Expression::BitwiseAnd(l, r) => Ok(l.eval(vars)? & r.eval(vars)?),
            Expression::BitwiseOr(l, r) => Ok(l.eval(vars)? | r.eval(vars)?),
            Expression::BitwiseXor(l, r) => Ok(l.eval(vars)? ^ r.eval(vars)?),
            Expression::Power(l, r) => Ok(l.eval(vars)?.pow(r.eval(vars)? as u32)),
            _ => unimplemented!(),
        }
    }
}

impl Unit {
    fn eval(&self, v: i64, spec: &GeneralSpec) -> Result<i64, String> {
        match self {
            Unit::Units => Ok((v as f64 * spec.unit) as i64),
            Unit::Microseconds => Ok(v),
            Unit::Milliseconds => Ok(v * 1000),
            Unit::Pulses => match spec.carrier {
                Some(f) => Ok(v * 1000 / f),
                None => Err("pulses specified but no carrier given".to_string()),
            },
        }
    }
}

pub fn render(input: &str, mut vars: Vartable) -> Result<Vec<u32>, String> {
    let parser = irp::protocolParser::new();

    let irp = parser.parse(input).map_err(|e| e.to_string())?;

    let gs = general_spec(&irp.general_spec)?;

    for (name, expr) in irp.definitions {
        vars.set(name, expr.eval(&vars)?);
    }

    let mut out = Output::new(&gs);

    if irp.bit_spec.len() != 2 {
        println!("bit_spec {:?}", irp.bit_spec);
        return Err("bit spec should have two entries".to_string());
    }

    out.push_extent_marker();

    for i in irp.stream.stream {
        match i {
            IrStreamItem::Duration(d) => {
                out.add(&d, &vars)?;
            }
            IrStreamItem::Assignment(id, expr) => {
                vars.set(id, expr.eval(&vars)?);
            }
            IrStreamItem::BitField(complement, e, reverse, length, skip) => {
                let mut b = e.eval(&vars)?;

                if let Some(skip) = skip {
                    b >>= skip.eval(&vars)?;
                }

                if complement {
                    b = !b;
                }

                let l = length.eval(&vars)?;

                // a tricksy way of say !gs.lsb logical xor reverse
                if gs.lsb == reverse {
                    b = b.reverse_bits().rotate_left(l as u32);
                }

                for _ in 0..l {
                    for dur in &irp.bit_spec[(b & 1) as usize] {
                        out.add(&dur, &vars)?;
                    }
                    b >>= 1;
                }
            }
            _ => {
                println!(
                    "i:{:?} before we go away:{}",
                    i,
                    rawir::print_to_string(&out.raw)
                );
                unimplemented!();
            }
        }
    }

    out.pop_extend_marker();

    Ok(out.raw)
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
                    p * 1000.0 / f as f64
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

#[test]
fn test() {
    let mut vars = Vartable::new();

    vars.set("F".to_string(), 1);
    vars.set("D".to_string(), 0xe9);
    vars.set("S".to_string(), 0xfe);

    let res = render(
        "{38.0k,564}<1,-1|1,-3>(16,-8,D:8,S:8,F:8,~F:8,1,^108m)+",
        vars,
    );

    // irptransmogrifier.sh  --irp "{38.0k,564}<1,-1|1,-3>(16,-8,D:8,S:8,F:8,~F:8,1,^108m)+" render -r -n F=1,D=0xe9,S=0xfe
    assert_eq!(
        res,
        Ok(rawir::parse("+9024,-4512,+564,-1692,+564,-564,+564,-564,+564,-1692,+564,-564,+564,-1692,+564,-1692,+564,-1692,+564,-564,+564,-1692,+564,-1692,+564,-1692,+564,-1692,+564,-1692,+564,-1692,+564,-1692,+564,-1692,+564,-564,+564,-564,+564,-564,+564,-564,+564,-564,+564,-564,+564,-564,+564,-564,+564,-1692,+564,-1692,+564,-1692,+564,-1692,+564,-1692,+564,-1692,+564,-1692,+564,-35244").unwrap())
    );

    let mut vars = Vartable::new();

    vars.set("F".to_string(), 1);
    vars.set("D".to_string(), 0xe9);
    vars.set("T".to_string(), 0);

    let res = render(
        "{36k,msb,889}<1,-1|-1,1>(1:1,~F:1:6,T:1,D:5,F:6,^114m)+",
        vars,
    );

    // irptransmogrifier.sh  --irp "{36k,msb,889}<1,-1|-1,1>(1:1,~F:1:6,T:1,D:5,F:6,^114m)+" render -r -n F=1,T=0,D=0xe9

    assert_eq!(
        res,
        Ok(rawir::parse("+889,-889,+1778,-889,+889,-1778,+1778,-889,+889,-1778,+1778,-889,+889,-889,+889,-889,+889,-889,+889,-1778,+889,-89108").unwrap())
    );

    let mut vars = Vartable::new();

    vars.set("F".to_string(), 1);
    vars.set("D".to_string(), 0xe9);
    vars.set("S".to_string(), 0x88);

    let res = render(
        "{38k,400}<1,-1|1,-3>(8,-4,170:8,90:8,15:4,D:4,S:8,F:8,E:4,C:4,1,-48)+ {E=1,C=D^S:4:0^S:4:4^F:4:0^F:4:4^E:4}",
        vars,
    );

    assert_eq!(
        res,
        Ok(rawir::parse("+3200,-1600,+400,-400,+400,-1200,+400,-400,+400,-1200,+400,-400,+400,-1200,+400,-400,+400,-1200,+400,-400,+400,-1200,+400,-400,+400,-1200,+400,-1200,+400,-400,+400,-1200,+400,-400,+400,-1200,+400,-1200,+400,-1200,+400,-1200,+400,-1200,+400,-400,+400,-400,+400,-1200,+400,-400,+400,-400,+400,-400,+400,-1200,+400,-400,+400,-400,+400,-400,+400,-1200,+400,-1200,+400,-400,+400,-400,+400,-400,+400,-400,+400,-400,+400,-400,+400,-400,+400,-1200,+400,-400,+400,-400,+400,-400,+400,-1200,+400,-400,+400,-400,+400,-1200,+400,-19200").unwrap())
    );
}
