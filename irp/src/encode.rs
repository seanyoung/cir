use super::{Expression, GeneralSpec, Irp, Message, Pronto, RepeatMarker, Unit, Vartable};

use bitvec::prelude::*;
use std::{collections::HashMap, rc::Rc};

impl Irp {
    /// Render it to raw IR with the given variables
    pub fn encode<'a>(&'a self, mut vars: Vartable<'a>, repeats: u64) -> Result<Message, String> {
        self.check_parameters(&mut vars)?;

        let mut encoder = Encoder::new(&self.general_spec);

        let stream = &[Rc::new(self.stream.clone())];

        eval_stream(
            stream,
            &mut encoder,
            None,
            &mut vars,
            &self.general_spec,
            repeats,
            0,
        )?;

        Ok(Message {
            carrier: self.general_spec.carrier,
            duty_cycle: self.general_spec.duty_cycle,
            raw: encoder.raw,
        })
    }

    /// Render it to pronto hex with the given variables. Any trailing part after the repeating section
    /// cannot be represented in pronto hex, so this is dropped. This part of the IR is commonly use for
    /// a "key up" type message.
    pub fn encode_pronto<'a>(&'a self, mut vars: Vartable<'a>) -> Result<Pronto, String> {
        self.check_parameters(&mut vars)?;

        let mut encoder = Encoder::new(&self.general_spec);

        let stream = &[Rc::new(self.stream.clone())];

        eval_stream(
            stream,
            &mut encoder,
            None,
            &mut vars,
            &self.general_spec,
            1,
            0,
        )?;

        let carrier = match self.general_spec.carrier {
            // This is the carrier transmogrifier uses for unmodulated signals
            Some(0) => 414514,
            None => 38000,
            Some(c) => c,
        };

        let intro_length = encoder.repeats_starts.unwrap_or(encoder.raw.len());
        let repeat_length = encoder.ending_starts.unwrap_or(encoder.raw.len());

        let intro = encoder.raw[0..intro_length]
            .iter()
            .map(|v| *v as f64)
            .collect();

        let repeat = encoder.raw[intro_length..repeat_length]
            .iter()
            .map(|v| *v as f64)
            .collect();

        if self.general_spec.carrier != Some(0) {
            Ok(Pronto::LearnedModulated {
                frequency: carrier as f64,
                intro,
                repeat,
            })
        } else {
            Ok(Pronto::LearnedUnmodulated {
                frequency: carrier as f64,
                intro,
                repeat,
            })
        }
    }

    fn check_parameters<'a>(&'a self, vars: &mut Vartable<'a>) -> Result<(), String> {
        for p in &self.parameters {
            let val = if let Ok((val, _)) = vars.get(&p.name) {
                val
            } else if let Some(e) = &p.default {
                let (v, l) = e.eval(vars)?;

                vars.set(p.name.to_owned(), v, l);

                v
            } else {
                return Err(format!("missing value for {}", p.name));
            };

            let (min, _) = p.min.eval(vars)?;
            if val < min {
                return Err(format!(
                    "{} is less than minimum value {} for parameter {}",
                    val, min, p.name
                ));
            }

            let (max, _) = p.max.eval(vars)?;
            if val > max {
                return Err(format!(
                    "{} is more than maximum value {} for parameter {}",
                    val, max, p.name
                ));
            }
        }

        // if parameters are defined, only allow parameters to be set
        if !self.parameters.is_empty() {
            for name in vars.vars.keys() {
                if !self.parameters.iter().any(|p| &p.name == name) {
                    return Err(format!("no parameter called {}", name));
                }
            }
        }

        for e in &self.definitions {
            if let Expression::Assignment(name, expr) = e {
                vars.set_definition(name.clone(), expr.as_ref());
            } else {
                panic!("definition not correct expression: {:?}", e);
            }
        }

        Ok(())
    }
}

impl<'a> Vartable<'a> {
    pub fn new() -> Self {
        Vartable {
            vars: HashMap::new(),
        }
    }

    /// IRP definitions are evaluated each time when they are referenced
    fn set_definition(&mut self, name: String, expr: &'a Expression) {
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
    /// Raw output. Even entries are flash, odd are gaps
    raw: Vec<u32>,
    /// Length of IR generated, including leading gap
    total_length: i64,
    /// Extents start from this point
    extent_marker: Vec<i64>,
    /// Nested bitspec scopes
    bitspec_scope: Vec<BitspecScope<'a>>,
    /// At which point that the repeating section start
    repeats_starts: Option<usize>,
    /// At this point the trailing section starts
    ending_starts: Option<usize>,
}

/// A single bitscope
struct BitspecScope<'a> {
    /// The bitspec itself
    bit_spec: &'a [Rc<Expression>],
    /// The bitstream. This will be populated bit by bit, and then flushed.
    bitstream: BitVec<usize, LocalBits>,
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
            repeats_starts: None,
            ending_starts: None,
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
                let mut bv = BitVec::<usize, LocalBits>::from_element(bits as usize);

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
    fn enter_bitspec_scope(&mut self, bit_spec: &'a [Rc<Expression>]) {
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

                    if let Expression::List(v) = self.bitspec_scope[level].bit_spec[bit].as_ref() {
                        eval_stream(v, self, lower_level, vars, self.general_spec, 0, 0)?;
                    }
                }
            } else {
                for bit in bits.chunks(bits_step as usize).rev() {
                    let bit = bit_to_usize(bit);

                    if let Expression::List(v) = self.bitspec_scope[level].bit_spec[bit].as_ref() {
                        eval_stream(v, self, lower_level, vars, self.general_spec, 0, 0)?;
                    }
                }
            }
        }

        self.flush_level(lower_level, vars)?;

        Ok(())
    }
}

impl Unit {
    pub fn eval(&self, v: i64, spec: &GeneralSpec) -> Result<i64, String> {
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

    pub fn eval_float(&self, v: f64, spec: &GeneralSpec) -> Result<i64, String> {
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
    stream: &'a [Rc<Expression>],
    encoder: &mut Encoder<'a>,
    level: Option<usize>,
    vars: &mut Vartable,
    gs: &GeneralSpec,
    repeats: u64,
    alternative: usize,
) -> Result<(), String> {
    for expr in stream {
        match expr.as_ref() {
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

                let (v, l) = expr.eval(vars)?;

                vars.set(id.to_string(), v, l);
            }
            Expression::Stream(stream) => {
                let variant_count = stream
                    .stream
                    .iter()
                    .filter_map(|e| {
                        if let Expression::Variation(list) = e.as_ref() {
                            Some(list.len())
                        } else {
                            None
                        }
                    })
                    .max();

                let (indefinite, count, set_marker) = match stream.repeat {
                    None if variant_count.is_some() => {
                        return Err(String::from("cannot have variant without repeat"));
                    }
                    None => (1, 0, false),
                    Some(RepeatMarker::Any) => {
                        if variant_count.is_some() {
                            // if the first variant is empty, then Any is permitted
                            let mut first_variant_empty = false;

                            if let Expression::Variation(list) = stream.stream[0].as_ref() {
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

                        (0, repeats, true)
                    }
                    Some(RepeatMarker::Count(num)) => (num, 0, false),
                    Some(RepeatMarker::OneOrMore) => (1, repeats, true),
                    Some(RepeatMarker::CountOrMore(num)) => (num, repeats, true),
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

                if count > 0 {
                    if set_marker && encoder.repeats_starts.is_none() {
                        encoder.repeats_starts = Some(encoder.raw.len());
                    }

                    for _ in 0..count {
                        encoder.push_extent_marker();
                        eval_stream(&stream.stream, encoder, level, vars, gs, repeats, 1)?;
                        encoder.pop_extend_marker();
                    }
                }

                if set_marker {
                    encoder.ending_starts = Some(encoder.raw.len());
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
                let (bits, length) = expr.eval(vars)?;

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
