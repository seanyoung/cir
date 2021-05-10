use super::ast::*;
use super::parser::parse;
use super::Message;

use bitintr::Popcnt;
use bitvec::prelude::*;
use std::collections::HashMap;

/// Parse an IRP expression and encode it to raw IR with the given variables
pub fn encode(input: &str, mut vars: Vartable, repeats: i64) -> Result<Message, String> {
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
        } else {
            panic!("definition not correct expression: {:?}", e);
        }
    }

    let mut encoder = Encoder::new(&irp.general_spec);

    let stream = vec![irp.stream];

    eval_stream(
        &stream,
        &mut encoder,
        None,
        &mut vars,
        &irp.general_spec,
        repeats,
        0,
    )?;

    Ok(Message {
        carrier: irp.general_spec.carrier,
        duty_cycle: irp.general_spec.duty_cycle,
        raw: encoder.raw,
    })
}

/// During IRP evaluation, variables may change their value
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

    /// IRP definitions are evaluated each time when they are referenced
    fn set_definition(&mut self, name: String, expr: Expression) {
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

/// Encoder. This can be used to add flash, gap, extents and deals with nested bitspec scopes.
struct Encoder<'a> {
    /// Reference to general spec for lsb/msb etc
    general_spec: &'a GeneralSpec,
    /// Raw output. Even entries are flash, odd are spaces
    raw: Vec<u32>,
    /// Length of IR generated, including leading space
    total_length: i64,
    /// Extents start from this point
    extent_marker: Vec<i64>,
    /// Nested bitspec scopes
    bitspec_scope: Vec<BitspecScope<'a>>,
}

/// A single bitscope
struct BitspecScope<'a> {
    /// The bitspec itself
    bit_spec: &'a [Expression],
    /// The bitstream. This will be populated bit by bit, and then flushed.
    bitstream: BitVec<LocalBits, usize>,
}

impl<'a> Encoder<'a> {
    /// Create a new encoder. One is needed per IRP encode.
    fn new(gs: &'a GeneralSpec) -> Self {
        Encoder {
            general_spec: gs,
            raw: Vec::new(),
            total_length: 0,
            extent_marker: Vec::new(),
            bitspec_scope: Vec::new(),
        }
    }

    /// When we enter an IR stream, we should mark reference point for extents
    fn push_extent_marker(&mut self) {
        self.extent_marker.push(self.total_length);
    }

    /// When we are done with an IR stream, the last extent reference point is no longer needed
    fn pop_extend_marker(&mut self) {
        self.extent_marker.pop();
    }

    /// Add a flash of length microseconds
    fn add_flash(&mut self, length: i64) {
        assert!(length > 0);

        self.total_length += length;

        if (self.raw.len() % 2) == 1 {
            *self.raw.last_mut().unwrap() += length as u32;
        } else {
            self.raw.push(length as u32);
        }
    }

    /// Add a gap of length microseconds
    fn add_gap(&mut self, length: i64) {
        assert!(length > 0);

        // Leading gaps must be added to the totals
        self.total_length += length;

        let len = self.raw.len();

        if len == 0 {
            // ignore leading gaps
        } else if (len % 2) == 0 {
            *self.raw.last_mut().unwrap() += length as u32;
        } else {
            self.raw.push(length as u32);
        }
    }

    /// Add an extent.
    fn add_extend(&mut self, mut extent: i64) {
        extent -= self.total_length - *self.extent_marker.last().unwrap();

        if extent > 0 {
            self.add_gap(extent);
        }
    }

    /// Add some bits after evaluating a bitfield.
    fn add_bits(&mut self, bits: i64, length: u8, level: Option<usize>) -> Result<(), String> {
        match level {
            Some(level) => {
                let mut bv = BitVec::<LocalBits, usize>::from_element(bits as usize);

                bv.truncate(length as usize);

                bv.reverse();

                let level = &mut self.bitspec_scope[level];

                if self.general_spec.lsb {
                    bv.append(&mut level.bitstream);
                    level.bitstream = bv;
                } else {
                    level.bitstream.append(&mut bv);
                }

                Ok(())
            }
            None => Err(String::from("bits not permitted")),
        }
    }

    /// When entering an IR stream with a bitspec, enter a new scope
    fn enter_bitspec_scope(&mut self, bit_spec: &'a [Expression]) {
        self.bitspec_scope.push(BitspecScope {
            bit_spec,
            bitstream: BitVec::new(),
        })
    }

    /// When leaving an IR stream with a bitspec, leave this scope
    fn leave_bitspec_scope(&mut self) {
        self.bitspec_scope.pop();
    }

    /// Flush the bitstream for a bitspec scope, which should recurse all the way down the scopes
    fn flush_level(&mut self, level: Option<usize>, vars: &mut Vartable) -> Result<(), String> {
        let level = match level {
            Some(level) => level,
            None => {
                return Ok(());
            }
        };

        let lower_level = if level > 0 { Some(level - 1) } else { None };

        if !self.bitspec_scope[level].bitstream.is_empty() {
            let mut bits = BitVec::new();

            // Swap in a new empty bitvec, we will consume the enter stream and then we
            // don't need a mutable reference.
            std::mem::swap(&mut bits, &mut self.bitspec_scope[level].bitstream);

            let len = self.bitspec_scope[level].bit_spec.len();
            // 2 => 1, 4 => 2, 8 => 3, 16 => 4
            let bits_step = len.trailing_zeros();

            if (bits.len() % bits_step as usize) != 0 {
                return Err(format!(
                    "{} bits found, not multiple of {}",
                    self.bitspec_scope[level].bitstream.len(),
                    bits_step
                ));
            }

            debug_assert_eq!(1 << bits_step, len);

            if !self.general_spec.lsb {
                for bit in bits.chunks(bits_step as usize) {
                    let bit = bit_to_usize(bit);

                    if let Expression::List(v) = &self.bitspec_scope[level].bit_spec[bit] {
                        eval_stream(v, self, lower_level, vars, self.general_spec, 0, 0)?;
                    }
                }
            } else {
                for bit in bits.chunks(bits_step as usize).rev() {
                    let bit = bit_to_usize(bit);

                    if let Expression::List(v) = &self.bitspec_scope[level].bit_spec[bit] {
                        eval_stream(v, self, lower_level, vars, self.general_spec, 0, 0)?;
                    }
                }
            }
        }

        self.flush_level(lower_level, vars)?;

        Ok(())
    }
}

impl Expression {
    /// Evaluate an arithmetic expression
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

fn eval_stream<'a>(
    stream: &'a [Expression],
    encoder: &mut Encoder<'a>,
    level: Option<usize>,
    vars: &mut Vartable,
    gs: &GeneralSpec,
    repeats: i64,
    alternative: usize,
) -> Result<(), String> {
    for expr in stream {
        match expr {
            Expression::Number(v) => {
                encoder.flush_level(level, vars)?;

                encoder.add_flash(Unit::Units.eval(*v, gs)?);
            }
            Expression::Negative(e) => {
                encoder.flush_level(level, vars)?;
                match e.as_ref() {
                    Expression::Number(v) => encoder.add_gap(Unit::Units.eval(*v, gs)?),
                    Expression::FlashConstant(v, u) => encoder.add_gap(u.eval_float(*v, gs)?),
                    _ => unreachable!(),
                }
            }
            Expression::FlashConstant(p, u) => {
                encoder.flush_level(level, vars)?;
                encoder.add_flash(u.eval_float(*p, gs)?);
            }
            Expression::FlashIdentifier(id, u) => {
                encoder.flush_level(level, vars)?;
                encoder.add_flash(u.eval(vars.get(id)?.0, gs)?);
            }
            Expression::ExtentConstant(p, u) => {
                encoder.flush_level(level, vars)?;
                encoder.add_extend(u.eval_float(*p, gs)?);
            }
            Expression::ExtentIdentifier(id, u) => {
                encoder.flush_level(level, vars)?;
                encoder.add_extend(u.eval(vars.get(id)?.0, gs)?);
            }
            Expression::GapConstant(p, u) => {
                encoder.flush_level(level, vars)?;
                encoder.add_gap(u.eval_float(*p, gs)?);
            }
            Expression::GapIdentifier(id, u) => {
                encoder.flush_level(level, vars)?;
                encoder.add_gap(u.eval(vars.get(id)?.0, gs)?);
            }
            Expression::Assignment(id, expr) => {
                encoder.flush_level(level, vars)?;

                let (v, l) = expr.eval(&vars)?;

                vars.set(id.to_string(), v, l);
            }
            Expression::Stream(stream) => {
                let variant_count = stream
                    .stream
                    .iter()
                    .filter_map(|e| {
                        if let Expression::Variation(list) = e {
                            Some(list.len())
                        } else {
                            None
                        }
                    })
                    .max();

                let (indefinite, count) = match stream.repeat {
                    None if variant_count.is_some() => {
                        return Err(String::from("cannot have variant without repeat"));
                    }
                    None => (1, 0),
                    Some(RepeatMarker::Any) => {
                        if variant_count.is_some() {
                            // if the first variant is empty, then Any is permitted
                            let mut first_variant_empty = false;

                            if let Expression::Variation(list) = &stream.stream[0] {
                                if list[0].is_empty() {
                                    first_variant_empty = true;
                                }
                            }

                            if !first_variant_empty {
                                return Err(String::from(
                                    "cannot have variant with '*' repeat, use '+' instead",
                                ));
                            }
                        }

                        (0, repeats)
                    }
                    Some(RepeatMarker::Count(num)) => (num, 0),
                    Some(RepeatMarker::OneOrMore) => (1, repeats),
                    Some(RepeatMarker::CountOrMore(num)) => (num, repeats),
                };

                let level = if !stream.bit_spec.is_empty() {
                    encoder.enter_bitspec_scope(&stream.bit_spec);

                    match level {
                        None => Some(0),
                        Some(level) => Some(level + 1),
                    }
                } else {
                    level
                };

                for _ in 0..indefinite {
                    encoder.push_extent_marker();
                    eval_stream(&stream.stream, encoder, level, vars, gs, repeats, 0)?;
                    encoder.pop_extend_marker();
                }

                for _ in 0..count {
                    encoder.push_extent_marker();
                    eval_stream(&stream.stream, encoder, level, vars, gs, repeats, 1)?;
                    encoder.pop_extend_marker();
                }

                if variant_count == Some(3) {
                    encoder.push_extent_marker();
                    eval_stream(&stream.stream, encoder, level, vars, gs, repeats, 2)?;
                    encoder.pop_extend_marker();
                }

                if !stream.bit_spec.is_empty() {
                    encoder.leave_bitspec_scope();
                }
            }
            Expression::Variation(list) => {
                let variation = &list[alternative];

                if variation.is_empty() {
                    break;
                }

                eval_stream(variation, encoder, level, vars, gs, repeats, alternative)?;
            }
            _ => {
                let (bits, length) = expr.eval(&vars)?;

                encoder.add_bits(bits, length, level)?;
            }
        }
    }

    encoder.flush_level(level, vars)?;

    Ok(())
}

// See https://github.com/bitvecto-rs/bitvec/issues/119
fn bit_to_usize(bit: &BitSlice) -> usize {
    let mut v = 0;

    for i in 0..bit.len() {
        if bit[i] {
            v |= 1 << (bit.len() - 1 - i);
        }
    }

    v
}
