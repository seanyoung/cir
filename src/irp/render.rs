use super::ast::*;
use super::irp;
use super::{general_spec, GeneralSpec};

use bitintr::Popcnt;
use std::collections::HashMap;

// Here we parse an irp lang

pub struct Vartable {
    vars: HashMap<String, (i64, u8)>,
}

impl Vartable {
    pub fn new() -> Self {
        Self {
            vars: HashMap::new(),
        }
    }

    pub fn set(&mut self, id: String, v: i64, l: u8) {
        self.vars.insert(id, (v, l));
    }

    pub fn get(&self, id: &str) -> Result<(i64, u8), String> {
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
                self.add_flash(u.eval(vars.get(id)?.0, &self.general_spec)?)
            }
            Duration::GapIdentifier(id, u) => {
                self.add_gap(u.eval(vars.get(id)?.0, &self.general_spec)?)
            }
            Duration::ExtentConstant(p, u) => {
                self.add_extend(u.eval(*p as i64, &self.general_spec)?)
            }
            Duration::ExtentIdentifier(id, u) => {
                self.add_extend(u.eval(vars.get(id)?.0, &self.general_spec)?)
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
    fn eval(&self, vars: &Vartable) -> Result<(i64, u8), String> {
        match self {
            Expression::Number(n) => Ok((*n, 64)),
            Expression::Identifier(id) => vars.get(id),
            Expression::Negative(e) => {
                let (v, l) = e.eval(vars)?;

                Ok((-v, l))
            }
            Expression::Complement(e) => {
                let (v, l) = e.eval(vars)?;

                Ok((!v, l))
            }
            Expression::Add(l, r) => {
                let (l_val, l_len) = l.eval(vars)?;
                let (r_val, r_len) = r.eval(vars)?;

                Ok(((l_val + r_val), std::cmp::max(l_len, r_len)))
            }
            Expression::Subtract(l, r) => {
                let (l_val, l_len) = l.eval(vars)?;
                let (r_val, r_len) = r.eval(vars)?;

                Ok(((l_val - r_val), std::cmp::max(l_len, r_len)))
            }
            Expression::Multiply(l, r) => {
                let (l_val, l_len) = l.eval(vars)?;
                let (r_val, r_len) = r.eval(vars)?;

                Ok(((l_val * r_val), std::cmp::max(l_len, r_len)))
            }
            Expression::Divide(l, r) => {
                let (l_val, l_len) = l.eval(vars)?;
                let (r_val, r_len) = r.eval(vars)?;

                if r_val == 0 {
                    return Err("divide by zero".to_string());
                }

                Ok(((l_val / r_val), std::cmp::max(l_len, r_len)))
            }
            Expression::Modulo(l, r) => {
                let (l_val, l_len) = l.eval(vars)?;
                let (r_val, r_len) = r.eval(vars)?;

                if r_val == 0 {
                    return Err("divide by zero".to_string());
                }

                Ok(((l_val % r_val), std::cmp::max(l_len, r_len)))
            }
            Expression::BitwiseAnd(l, r) => {
                let (l_val, l_len) = l.eval(vars)?;
                let (r_val, r_len) = r.eval(vars)?;

                Ok(((l_val & r_val), std::cmp::max(l_len, r_len)))
            }
            Expression::BitwiseOr(l, r) => {
                let (l_val, l_len) = l.eval(vars)?;
                let (r_val, r_len) = r.eval(vars)?;

                Ok(((l_val | r_val), std::cmp::max(l_len, r_len)))
            }
            Expression::BitwiseXor(l, r) => {
                let (l_val, l_len) = l.eval(vars)?;
                let (r_val, r_len) = r.eval(vars)?;

                Ok(((l_val ^ r_val), std::cmp::max(l_len, r_len)))
            }
            Expression::Power(l, r) => {
                let (l_val, l_len) = l.eval(vars)?;
                let (r_val, _) = r.eval(vars)?;

                if r_val < 0 {
                    return Err("power to negative not supported".to_string());
                }

                Ok((l_val.pow(r_val as u32), l_len))
            }
            Expression::BitCount(e) => {
                let (mut val, len) = e.eval(vars)?;

                if len < 63 {
                    // mask off any unused bits
                    val &= (1 << len) - 1;
                }

                Ok((val.popcnt(), len))
            }
            Expression::BitField {
                value,
                reverse,
                length,
                skip,
            } => {
                let (mut b, _) = value.eval(&vars)?;

                if let Some(skip) = skip {
                    b >>= skip.eval(&vars)?.0;
                }

                let (l, _) = length.eval(&vars)?;

                if *reverse {
                    b = b.reverse_bits().rotate_left(l as u32);
                }

                Ok((b, l as u8))
            }
            _ => panic!("not implement: {:?}", self),
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
                Some(f) => Ok(v * 1_000_000 / f),
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
        let (v, l) = expr.eval(&vars)?;

        vars.set(name, v, l);
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
                let (v, l) = expr.eval(&vars)?;

                vars.set(id, v, l);
            }
            IrStreamItem::Expression(e) => {
                let (mut v, l) = e.eval(&vars)?;

                if !gs.lsb {
                    v = v.reverse_bits().rotate_left(l as u32);
                }

                for _ in 0..l {
                    for dur in &irp.bit_spec[(v & 1) as usize] {
                        out.add(&dur, &vars)?;
                    }
                    v >>= 1;
                }
            }
        }
    }

    out.pop_extend_marker();

    Ok(out.raw)
}
