use super::ast::*;
use super::parse;

use bitintr::Popcnt;
use bitvec::prelude::*;
use std::collections::HashMap;

// Here we parse an irp lang
#[derive(Default)]
pub struct Vartable {
    vars: HashMap<String, (i64, u8, Option<Expression>)>,
}

impl Vartable {
    pub fn new() -> Self {
        Vartable {
            vars: HashMap::new(),
        }
    }

    pub fn set_definition(&mut self, name: String, expr: Expression) {
        self.vars.insert(name, (0, 0, Some(expr)));
    }

    pub fn set(&mut self, id: String, name: i64, length: u8) {
        self.vars.insert(id, (name, length, None));
    }

    pub fn is_defined(&self, id: &str) -> bool {
        self.vars.contains_key(id)
    }

    pub fn get(&self, id: &str) -> Result<(i64, u8), String> {
        match self.vars.get(id) {
            Some((val, length, None)) => Ok((*val, *length)),
            Some((_, _, Some(expr))) => expr.eval(self),
            None => Err(format!("variable `{}´ not defined", id)),
        }
    }
}

struct Output<'a> {
    general_spec: &'a GeneralSpec,
    raw: Vec<u32>,
    total: i64,
    extent_marker: Vec<i64>,
    levels: Vec<BitspecLevel<'a>>,
}

struct BitspecLevel<'a> {
    bit_spec: &'a [Expression],
    res: BitVec<LocalBits, usize>,
}

impl<'a> Output<'a> {
    fn new(gs: &'a GeneralSpec) -> Self {
        Self {
            general_spec: gs,
            raw: Vec::new(),
            total: 0,
            extent_marker: Vec::new(),
            levels: Vec::new(),
        }
    }

    fn push_extent_marker(&mut self) {
        self.extent_marker.push(self.total);
    }

    fn pop_extend_marker(&mut self) {
        self.extent_marker.pop();
    }

    fn add_flash(&mut self, n: i64) {
        assert!(n > 0);

        self.total += n;

        if (self.raw.len() % 2) == 1 {
            *self.raw.last_mut().unwrap() += n as u32;
        } else {
            self.raw.push(n as u32);
        }
    }

    fn add_bits(&mut self, bits: i64, length: u8, level: Option<usize>) -> Result<(), String> {
        match level {
            Some(level) => {
                let mut bv = BitVec::<LocalBits, usize>::from_element(bits as usize);

                bv.truncate(length as usize);

                bv.reverse();

                let level = &mut self.levels[level];

                if self.general_spec.lsb {
                    bv.append(&mut level.res);
                    level.res = bv;
                } else {
                    level.res.append(&mut bv);
                }

                Ok(())
            }
            None => Err(String::from("bits not permitted")),
        }
    }

    fn push_bitspec(&mut self, bit_spec: &'a [Expression]) {
        self.levels.push(BitspecLevel {
            bit_spec,
            res: BitVec::new(),
        })
    }

    fn flush_level(&mut self, level: Option<usize>, vars: &mut Vartable) -> Result<(), String> {
        let level = match level {
            Some(level) => level,
            None => {
                return Ok(());
            }
        };

        let x = &self.levels[level];

        let lower_level = if level > 0 { Some(level - 1) } else { None };

        if x.res.is_empty() {
            self.flush_level(lower_level, vars)?;

            return Ok(());
        }

        let mut bits = BitVec::new();

        std::mem::swap(&mut bits, &mut self.levels[level].res);

        let len = self.levels[level].bit_spec.len();
        // 2 => 1, 4 => 2, 8 => 3, 16 => 4
        let bits_step = len.trailing_zeros();

        if (bits.len() % bits_step as usize) != 0 {
            return Err(format!(
                "{} bits found, not multiple of {}",
                self.levels[level].res.len(),
                bits_step
            ));
        }

        debug_assert_eq!(1 << bits_step, len);

        if !self.general_spec.lsb {
            for bit in bits.chunks(bits_step as usize) {
                let bit = bit_to_usize(bit);

                if let Expression::List(v) = &self.levels[level].bit_spec[bit] {
                    eval_stream(v, self, lower_level, vars, self.general_spec, 0, 0)?;
                }
            }
        } else {
            for bit in bits.chunks(bits_step as usize).rev() {
                let bit = bit_to_usize(bit);

                if let Expression::List(v) = &self.levels[level].bit_spec[bit] {
                    eval_stream(v, self, lower_level, vars, self.general_spec, 0, 0)?;
                }
            }
        }

        self.flush_level(lower_level, vars)?;

        Ok(())
    }

    fn pop_bitspec(&mut self) {
        self.levels.pop();
    }

    fn add_gap(&mut self, n: i64) {
        assert!(n > 0);

        // Leading gaps must be added to the totals
        self.total += n;

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
        extent -= self.total - *self.extent_marker.last().unwrap();

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
            Expression::ShiftLeft(value, r) => {
                let (value, len) = value.eval(vars)?;
                let (r, _) = r.eval(vars)?;

                Ok((value << r, len))
            }
            Expression::ShiftRight(value, r) => {
                let (value, len) = value.eval(vars)?;
                let (r, _) = r.eval(vars)?;

                Ok((value >> r, len))
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

                b &= (1 << l) - 1;

                Ok((b, l as u8))
            }
            Expression::InfiniteBitField { value, skip } => {
                let (mut b, _) = value.eval(&vars)?;

                b >>= skip.eval(&vars)?.0;

                Ok((b, 8))
            }
            Expression::List(v) if v.len() == 1 => {
                let (v, l) = v[0].eval(vars)?;

                Ok((v, l))
            }
            _ => panic!("not implemented: {:?}", self),
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

    fn eval_float(&self, v: f64, spec: &GeneralSpec) -> Result<i64, String> {
        match self {
            Unit::Units => Ok((v * spec.unit) as i64),
            Unit::Microseconds => Ok(v as i64),
            Unit::Milliseconds => Ok((v * 1000.0) as i64),
            Unit::Pulses => match spec.carrier {
                Some(f) => Ok((v * 1_000_000.0) as i64 / f),
                None => Err("pulses specified but no carrier given".to_string()),
            },
        }
    }
}

pub fn render(
    input: &str,
    mut vars: Vartable,
    repeats: i64,
) -> Result<(Option<i64>, Vec<u32>), String> {
    let irp = parse(input)?;

    for p in &irp.parameters {
        let val = if let Ok((val, _)) = vars.get(&p.name) {
            val
        } else if let Some(e) = &p.default {
            let (v, l) = e.eval(&vars)?;

            vars.set(p.name.to_owned(), v, l);

            v
        } else {
            return Err(format!("missing value for {}", p.name));
        };

        let (min, _) = p.min.eval(&vars)?;
        if val < min {
            return Err(format!(
                "{} is less than minimum value {} for parameter {}",
                val, min, p.name
            ));
        }

        let (max, _) = p.max.eval(&vars)?;
        if val > max {
            return Err(format!(
                "{} is more than maximum value {} for parameter {}",
                val, max, p.name
            ));
        }
    }

    // if parameters are defined, only allow parameters to be set
    if !irp.parameters.is_empty() {
        for name in vars.vars.keys() {
            if !irp.parameters.iter().any(|p| &p.name == name) {
                return Err(format!("no parameter called {}", name));
            }
        }
    }

    for e in irp.definitions {
        if let Expression::Assignment(name, expr) = e {
            vars.set_definition(name, *expr);
        }
    }

    let mut out = Output::new(&irp.general_spec);

    let stream = vec![irp.stream];

    eval_stream(
        &stream,
        &mut out,
        None,
        &mut vars,
        &irp.general_spec,
        repeats,
        0,
    )?;

    Ok((irp.general_spec.carrier, out.raw))
}

fn eval_stream<'a>(
    stream: &'a [Expression],
    out: &mut Output<'a>,
    level: Option<usize>,
    vars: &mut Vartable,
    gs: &GeneralSpec,
    repeats: i64,
    alternative: usize,
) -> Result<(), String> {
    for expr in stream {
        match expr {
            Expression::Number(v) => {
                out.flush_level(level, vars)?;

                out.add_flash(Unit::Units.eval(*v, gs)?);
            }
            Expression::Negative(e) => {
                out.flush_level(level, vars)?;
                match e.as_ref() {
                    Expression::Number(v) => out.add_gap(Unit::Units.eval(*v, gs)?),
                    Expression::FlashConstant(v, u) => out.add_gap(u.eval_float(*v, gs)?),
                    _ => unreachable!(),
                }
            }
            Expression::FlashConstant(p, u) => {
                out.flush_level(level, vars)?;
                out.add_flash(u.eval_float(*p, gs)?);
            }
            Expression::FlashIdentifier(id, u) => {
                out.flush_level(level, vars)?;
                out.add_flash(u.eval(vars.get(id)?.0, gs)?);
            }
            Expression::ExtentConstant(p, u) => {
                out.flush_level(level, vars)?;
                out.add_extend(u.eval_float(*p, gs)?);
            }
            Expression::ExtentIdentifier(id, u) => {
                out.flush_level(level, vars)?;
                out.add_extend(u.eval(vars.get(id)?.0, gs)?);
            }
            Expression::GapConstant(p, u) => {
                out.flush_level(level, vars)?;
                out.add_gap(u.eval_float(*p, gs)?);
            }
            Expression::GapIdentifier(id, u) => {
                out.flush_level(level, vars)?;
                out.add_gap(u.eval(vars.get(id)?.0, gs)?);
            }
            Expression::Assignment(id, expr) => {
                out.flush_level(level, vars)?;

                let (v, l) = expr.eval(&vars)?;

                vars.set(id.to_string(), v, l);
            }
            Expression::Stream(stream) => {
                let (indefinite, count) = match stream.repeat {
                    None => {
                        // If a stream starts with variation, then it is implicitly repeating
                        if let Expression::Variation(_) = &stream.stream[0] {
                            (1, repeats)
                        } else {
                            (1, 0)
                        }
                    }
                    Some(RepeatMarker::Any) => (0, repeats),
                    Some(RepeatMarker::Count(num)) => (num, 0),
                    Some(RepeatMarker::OneOrMore) => (1, repeats),
                    Some(RepeatMarker::CountOrMore(num)) => (num, repeats),
                };

                let level = if !stream.bit_spec.is_empty() {
                    out.push_bitspec(&stream.bit_spec);

                    match level {
                        None => Some(0),
                        Some(level) => Some(level + 1),
                    }
                } else {
                    level
                };

                for _ in 0..indefinite {
                    out.push_extent_marker();
                    eval_stream(&stream.stream, out, level, vars, gs, repeats, 0)?;
                    out.pop_extend_marker();
                }

                for _ in 0..count {
                    out.push_extent_marker();
                    eval_stream(&stream.stream, out, level, vars, gs, repeats, 1)?;
                    out.pop_extend_marker();
                }

                if let Expression::Variation(list) = &stream.stream[0] {
                    if list.len() == 3 {
                        out.push_extent_marker();
                        eval_stream(&stream.stream, out, level, vars, gs, repeats, 2)?;
                        out.pop_extend_marker();
                    }
                }

                if !stream.bit_spec.is_empty() {
                    out.pop_bitspec();
                }
            }
            Expression::Variation(list) => {
                let variation = &list[alternative];

                if variation.is_empty() {
                    break;
                }

                eval_stream(variation, out, level, vars, gs, repeats, alternative)?;
            }
            _ => {
                let (bits, length) = expr.eval(&vars)?;

                out.add_bits(bits, length, level)?;
            }
        }
    }

    out.flush_level(level, vars)?;

    Ok(())
}

fn bit_to_usize(bit: &BitSlice) -> usize {
    let mut v = 0;

    for i in 0..bit.len() {
        if bit[i] {
            v |= 1 << (bit.len() - 1 - i);
        }
    }

    v
}
