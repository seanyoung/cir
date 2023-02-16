use super::{Expression, GeneralSpec, Irp, Message, Pronto, RepeatMarker, Unit, Vartable};
use bitvec::prelude::*;
use log::warn;
use std::{collections::HashMap, rc::Rc};

impl Irp {
    /// Render it to raw IR with the given variables
    pub fn encode<'a>(&'a self, mut vars: Vartable<'a>, repeats: u64) -> Result<Message, String> {
        self.check_parameters(&mut vars)?;

        let variants = self.split_variants_encode()?;

        let mut encoder = Encoder::new(&self.general_spec);
        let stream;

        if let Some(down) = variants.down {
            stream = [down];

            eval_stream(
                &stream,
                &mut encoder,
                None,
                &mut vars,
                &self.general_spec,
                repeats,
            )?;

            encoder.flush_level(None, &mut vars)?;

            if (encoder.raw.len() % 2) != 0 {
                return Err("stream must end with a gap".into());
            }
        }

        let stream = [variants.repeat];

        eval_stream(
            &stream,
            &mut encoder,
            None,
            &mut vars,
            &self.general_spec,
            repeats,
        )?;

        encoder.flush_level(None, &mut vars)?;

        if (encoder.raw.len() % 2) != 0 {
            return Err("stream must end with a gap".into());
        }

        let stream;

        if let Some(up) = variants.up {
            stream = [up];

            eval_stream(
                &stream,
                &mut encoder,
                None,
                &mut vars,
                &self.general_spec,
                repeats,
            )?;

            encoder.flush_level(None, &mut vars)?;

            if (encoder.raw.len() % 2) != 0 {
                return Err("stream must end with a gap".into());
            }
        }

        Ok(Message {
            carrier: Some(self.general_spec.carrier),
            duty_cycle: self.general_spec.duty_cycle,
            raw: encoder.raw,
        })
    }

    /// Render it to pronto hex with the given variables. Any trailing part after the repeating section
    /// cannot be represented in pronto hex, so this is dropped. This part of the IR is commonly use for
    /// a "key up" type message.
    /// This always produces pronto hex long codes, never the short variant.
    pub fn encode_pronto<'a>(&'a self, mut vars: Vartable<'a>) -> Result<Pronto, String> {
        self.check_parameters(&mut vars)?;

        let carrier = match self.general_spec.carrier {
            // This is the carrier transmogrifier uses for unmodulated signals
            0 => 414514,
            c => c,
        };

        let variants = self.split_variants_encode()?;

        let mut encoder = Encoder::new(&self.general_spec);
        let stream;

        if let Some(down) = variants.down {
            stream = [down];

            eval_stream(
                &stream,
                &mut encoder,
                None,
                &mut vars,
                &self.general_spec,
                1,
            )?;

            encoder.flush_level(None, &mut vars)?;
        }

        let intro = encoder.raw.iter().map(|v| *v as f64).collect();

        encoder.raw.truncate(0);

        let stream = &[variants.repeat];

        eval_stream(stream, &mut encoder, None, &mut vars, &self.general_spec, 1)?;

        encoder.flush_level(None, &mut vars)?;

        let repeat = encoder.raw.iter().map(|v| *v as f64).collect();

        if self.general_spec.carrier != 0 {
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
            let val = if let Ok(val) = vars.get(&p.name) {
                val
            } else if let Some(e) = &p.default {
                let v = e.eval(vars)?;

                vars.set(p.name.to_owned(), v);

                v
            } else {
                return Err(format!("missing value for {}", p.name));
            };

            let min = p.min.eval(vars)?;
            if val < min {
                return Err(format!(
                    "{} is less than minimum value {} for parameter {}",
                    val, min, p.name
                ));
            }

            let max = p.max.eval(vars)?;
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
                    return Err(format!("no parameter called {name}"));
                }
            }
        }

        for e in &self.definitions {
            if let Expression::Assignment(name, expr) = e {
                vars.set_definition(name.clone(), expr.as_ref());
            } else {
                panic!("definition not correct expression: {e:?}");
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
    fn set_definition(&mut self, id: String, expr: &'a Expression) {
        self.vars.insert(id, (0, Some(expr)));
    }

    pub fn set(&mut self, id: String, value: i64) {
        self.vars.insert(id, (value, None));
    }

    pub fn is_defined(&self, id: &str) -> bool {
        self.vars.contains_key(id)
    }

    pub fn get(&self, id: &str) -> Result<i64, String> {
        match self.vars.get(id) {
            Some((val, None)) => Ok(*val),
            Some((_, Some(expr))) => expr.eval(self),
            None => Err(format!("variable `{id}´ not defined")),
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
    fn new(general_spec: &'a GeneralSpec) -> Self {
        Encoder {
            general_spec,
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
    fn add_flash(&mut self, length: i64) -> Result<(), String> {
        if length <= 0 {
            warn!("length should be non-zero");
            return Ok(());
        }

        if let Some(v) = self.total_length.checked_add(length) {
            self.total_length = v;
        } else {
            return Err("length overflow".into());
        }

        if (self.raw.len() % 2) == 1 {
            let raw = self.raw.last_mut().unwrap();

            if let Some(v) = raw.checked_add(length as u32) {
                *raw = v;
            } else {
                return Err("length overflow".into());
            }
        } else {
            self.raw.push(length as u32);
        }
        Ok(())
    }

    /// Add a gap of length microseconds
    fn add_gap(&mut self, length: i64) -> Result<(), String> {
        if length <= 0 {
            warn!("length should be non-zero");
            return Ok(());
        }

        // Leading gaps must be added to the totals
        if let Some(v) = self.total_length.checked_add(length) {
            self.total_length = v;
        } else {
            return Err("length overflow".into());
        }

        let len = self.raw.len();

        if len == 0 {
            // ignore leading gaps
        } else if (len % 2) == 0 {
            let raw = self.raw.last_mut().unwrap();

            if let Some(v) = raw.checked_add(length as u32) {
                *raw = v;
            } else {
                return Err("length overflow".into());
            }
        } else {
            self.raw.push(length as u32);
        }
        Ok(())
    }

    /// Add an extent.
    fn add_extent(&mut self, extent: i64, strict: bool) -> Result<(), String> {
        // remove length of stream generated so far
        let trimmed_extent = extent - (self.total_length - *self.extent_marker.last().unwrap());

        if trimmed_extent > 0 {
            self.add_gap(trimmed_extent)?;
        } else if strict {
            // IrpTransmogrifier will error here with: Argument of extent smaller than actual duration
            // We do this to remain compatible with lircd transmit
            return Err("extend shorter than duration".into());
        } else {
            self.add_gap(extent)?;
        }
        Ok(())
    }

    /// Add some bits after evaluating a bitfield.
    fn add_bits(&mut self, bits: i64, length: i64, level: Option<usize>) -> Result<(), String> {
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

            let max_bit = self.bitspec_scope[level].bit_spec.len();

            // 1,2 => 1
            // 3,4 => 2
            // 5,6,7,8 => 4
            // 9,10,11,12,13,14,15,16 => 4
            let bits_step = if max_bit <= 2 {
                1
            } else {
                max_bit.next_power_of_two().ilog2()
            };

            if !self.general_spec.lsb {
                for bit in bits.chunks(bits_step as usize) {
                    let bit = bit_to_usize(bit);

                    if bit >= max_bit {
                        return Err(format!("Cannot encode {bit} with current bit_spec"));
                    }

                    if let Expression::List(v) = self.bitspec_scope[level].bit_spec[bit].as_ref() {
                        eval_stream(v, self, lower_level, vars, self.general_spec, 0)?;
                    }
                }
            } else {
                for bit in bits.chunks(bits_step as usize).rev() {
                    let bit = bit_to_usize(bit);

                    if bit >= max_bit {
                        return Err(format!("Cannot encode {bit} with current bit_spec"));
                    }

                    if let Expression::List(v) = self.bitspec_scope[level].bit_spec[bit].as_ref() {
                        eval_stream(v, self, lower_level, vars, self.general_spec, 0)?;
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
                0 => Err("pulses cannot be used with zero carrier".into()),
                f => Ok(v * 1_000_000 / f),
            },
        }
    }

    pub fn eval_float(&self, v: f64, spec: &GeneralSpec) -> Result<i64, String> {
        match self {
            Unit::Units => Ok((v * spec.unit) as i64),
            Unit::Microseconds => Ok(v as i64),
            Unit::Milliseconds => Ok((v * 1000.0) as i64),
            Unit::Pulses => match spec.carrier {
                0 => Err("pulses cannot be used with zero carrier".into()),
                f => Ok((v * 1_000_000.0) as i64 / f),
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
) -> Result<(), String> {
    for expr in stream {
        match expr.as_ref() {
            Expression::FlashConstant(p, u) => {
                encoder.flush_level(level, vars)?;
                encoder.add_flash(u.eval_float(*p, gs)?)?;
            }
            Expression::FlashIdentifier(id, u) => {
                encoder.flush_level(level, vars)?;
                encoder.add_flash(u.eval(vars.get(id)?, gs)?)?;
            }
            Expression::ExtentConstant(p, u) => {
                encoder.flush_level(level, vars)?;
                encoder.add_extent(u.eval_float(*p, gs)?, false)?;
            }
            Expression::StrictExtentConstant(p, u) => {
                encoder.flush_level(level, vars)?;
                encoder.add_extent(u.eval_float(*p, gs)?, true)?;
            }
            Expression::ExtentIdentifier(id, u) => {
                encoder.flush_level(level, vars)?;
                encoder.add_extent(u.eval(vars.get(id)?, gs)?, false)?;
            }
            Expression::StrictExtentIdentifier(id, u) => {
                encoder.flush_level(level, vars)?;
                encoder.add_extent(u.eval(vars.get(id)?, gs)?, true)?;
            }
            Expression::GapConstant(p, u) => {
                encoder.flush_level(level, vars)?;
                encoder.add_gap(u.eval_float(*p, gs)?)?;
            }
            Expression::GapIdentifier(id, u) => {
                encoder.flush_level(level, vars)?;
                encoder.add_gap(u.eval(vars.get(id)?, gs)?)?;
            }
            Expression::Assignment(id, expr) => {
                encoder.flush_level(level, vars)?;

                let v = expr.eval(vars)?;

                vars.set(id.into(), v);
            }
            Expression::Stream(stream) => {
                let repeats = match stream.repeat {
                    None => 1,
                    Some(RepeatMarker::Any) => repeats,
                    Some(RepeatMarker::Count(num)) => num as u64,
                    Some(RepeatMarker::OneOrMore) => 1 + repeats,
                    Some(RepeatMarker::CountOrMore(num)) => num as u64 + repeats,
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

                for _ in 0..repeats {
                    encoder.push_extent_marker();
                    eval_stream(&stream.stream, encoder, level, vars, gs, repeats)?;
                    encoder.pop_extend_marker();
                }

                encoder.flush_level(level, vars)?;

                if !stream.bit_spec.is_empty() {
                    encoder.leave_bitspec_scope();
                }
            }
            Expression::BitField { .. } => {
                let (bits, length) = expr.bitfield(vars)?;

                if !(0..64).contains(&length) {
                    return Err("bitfields of {length} not supported".into());
                }

                encoder.add_bits(bits, length, level)?;
            }
            Expression::List(list) if list.is_empty() => break,
            Expression::List(list) => {
                eval_stream(list, encoder, level, vars, gs, repeats)?;
            }
            _ => unreachable!(),
        }
    }

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
