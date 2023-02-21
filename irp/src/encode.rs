use super::{Expression, GeneralSpec, Irp, Message, Pronto, RepeatMarker, Unit, Vartable};
use bitvec::prelude::*;
use log::warn;
use std::{collections::HashMap, rc::Rc};

impl Irp {
    /// Render it to raw IR with the given variables
    pub fn encode<'a>(&'a self, mut vars: Vartable<'a>, repeats: u64) -> Result<Message, String> {
        self.check_parameters(&mut vars)?;

        let variants = self.split_variants_encode()?;

        let mut encoder = Encoder::new(&self.general_spec, vars, repeats);

        if let Some(down) = &variants.down {
            encoder.encode(down, None)?;

            if encoder.has_trailing_pulse() {
                return Err("stream must end with a gap".into());
            }
        }

        encoder.encode(&variants.repeat, None)?;

        if encoder.has_trailing_pulse() {
            return Err("stream must end with a gap".into());
        }

        if let Some(up) = &variants.up {
            encoder.encode(up, None)?;

            if encoder.has_trailing_pulse() {
                return Err("stream must end with a gap".into());
            }
        }

        Ok(Message {
            carrier: Some(self.general_spec.carrier),
            duty_cycle: self.general_spec.duty_cycle,
            raw: encoder.raw,
        })
    }

    /// Render it to pronto hex with the given variables.
    /// This always produces pronto hex long codes, never the short variant.
    pub fn encode_pronto<'a>(&'a self, mut vars: Vartable<'a>) -> Result<Pronto, String> {
        self.check_parameters(&mut vars)?;

        let carrier = match self.general_spec.carrier {
            // This is the carrier transmogrifier uses for unmodulated signals
            0 => 414514,
            c => c,
        };

        let variants = self.split_variants_encode()?;

        let mut encoder = Encoder::new(&self.general_spec, vars, 1);

        if let Some(down) = &variants.down {
            encoder.encode(down, None)?;

            if encoder.has_trailing_pulse() {
                return Err("stream must end with a gap".into());
            }
        }

        let intro = encoder.raw.iter().map(|v| *v as f64).collect();

        encoder.raw.truncate(0);

        encoder.encode(&variants.repeat, None)?;

        if encoder.has_trailing_pulse() {
            return Err("stream must end with a gap".into());
        }

        let repeat = encoder.raw.iter().map(|v| *v as f64).collect();

        if let Some(up) = &variants.up {
            encoder.raw.truncate(0);

            encoder.encode(up, None)?;

            if !encoder.raw.is_empty() {
                warn!("ending sequence cannot be represented in pronto, dropped");
            }
        }

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

            if val < p.min {
                return Err(format!(
                    "{} is less than minimum value {} for parameter {}",
                    val, p.min, p.name
                ));
            }

            if val > p.max {
                return Err(format!(
                    "{} is more than maximum value {} for parameter {}",
                    val, p.max, p.name
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
struct Encoder<'a, 'b> {
    /// Reference to general spec for lsb/msb etc
    general_spec: &'a GeneralSpec,
    /// Raw output. Even entries are flash, odd are gaps
    raw: Vec<u32>,
    /// Length of IR generated, including leading gap
    total_length: i64,
    /// Extents start from this point
    extent_marker: Vec<i64>,
    /// Number of repeats to encode
    repeats: u64,
    /// The variables
    vars: Vartable<'a>,
    /// bitspec scopes
    bitspec_scope: Vec<BitspecScope<'b>>,
}

/// A single bitscope
struct BitspecScope<'a> {
    /// The bitspec itself
    bit_spec: &'a [Rc<Expression>],
    /// The bitstream. This will be populated bit by bit, and then flushed.
    bitstream: BitVec<usize, LocalBits>,
}

impl<'a, 'b> Encoder<'a, 'b> {
    /// Create a new encoder. One is needed per IRP encode.
    fn new(general_spec: &'a GeneralSpec, vars: Vartable<'a>, repeats: u64) -> Self {
        Encoder {
            general_spec,
            repeats,
            vars,
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
    fn add_extent(&mut self, extent: i64) -> Result<(), String> {
        // remove length of stream generated so far
        let trimmed_extent = extent - (self.total_length - *self.extent_marker.last().unwrap());

        if trimmed_extent > 0 {
            self.add_gap(trimmed_extent)?;
        } else {
            // IrpTransmogrifier will error here with: Argument of extent smaller than actual duration
            // We do this to remain compatible with lircd transmit
            return Err("extend shorter than duration".into());
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

    /// Flush the bitstream for a bitspec scope, which should recurse all the way down the scopes
    fn flush_level(&mut self, level: Option<usize>) -> Result<(), String> {
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

            let bits_step = match max_bit {
                1..=2 => 1,
                3..=4 => 2,
                5..=8 => 3,
                9..=16 => 4,
                _ => unreachable!(),
            };

            if !self.general_spec.lsb {
                for bit in bits.chunks(bits_step as usize) {
                    let bit = bit_to_usize(bit);

                    if bit >= max_bit {
                        return Err(format!("Cannot encode {bit} with current bit_spec"));
                    }

                    self.encode(
                        self.bitspec_scope[level].bit_spec[bit].as_ref(),
                        lower_level,
                    )?;
                }
            } else {
                for bit in bits.chunks(bits_step as usize).rev() {
                    let bit = bit_to_usize(bit);

                    if bit >= max_bit {
                        return Err(format!("Cannot encode {bit} with current bit_spec"));
                    }

                    self.encode(
                        self.bitspec_scope[level].bit_spec[bit].as_ref(),
                        lower_level,
                    )?;
                }
            }
        }

        self.flush_level(lower_level)?;

        Ok(())
    }

    fn encode(&mut self, expr: &'b Expression, level: Option<usize>) -> Result<(), String> {
        match expr {
            Expression::FlashConstant(p, u) => {
                self.flush_level(level)?;
                self.add_flash(u.eval_float(*p, self.general_spec)?)?;
            }
            Expression::FlashIdentifier(id, u) => {
                self.flush_level(level)?;
                self.add_flash(u.eval(self.vars.get(id)?, self.general_spec)?)?;
            }
            Expression::ExtentConstant(p, u) => {
                self.flush_level(level)?;
                self.add_extent(u.eval_float(*p, self.general_spec)?)?;
            }
            Expression::ExtentIdentifier(id, u) => {
                self.flush_level(level)?;
                self.add_extent(u.eval(self.vars.get(id)?, self.general_spec)?)?;
            }
            Expression::GapConstant(p, u) => {
                self.flush_level(level)?;
                self.add_gap(u.eval_float(*p, self.general_spec)?)?;
            }
            Expression::GapIdentifier(id, u) => {
                self.flush_level(level)?;
                self.add_gap(u.eval(self.vars.get(id)?, self.general_spec)?)?;
            }
            Expression::Assignment(id, expr) => {
                self.flush_level(level)?;

                let v = expr.eval(&self.vars)?;

                self.vars.set(id.into(), v);
            }
            Expression::Stream(stream) => {
                let repeats = match stream.repeat {
                    None => 1,
                    Some(RepeatMarker::Any) => self.repeats,
                    Some(RepeatMarker::Count(num)) => num as u64,
                    Some(RepeatMarker::OneOrMore) => 1 + self.repeats,
                    Some(RepeatMarker::CountOrMore(num)) => num as u64 + self.repeats,
                };

                let level = if !stream.bit_spec.is_empty() {
                    self.bitspec_scope.push(BitspecScope {
                        bit_spec: &stream.bit_spec,
                        bitstream: BitVec::new(),
                    });

                    match level {
                        None => Some(0),
                        Some(level) => Some(level + 1),
                    }
                } else {
                    level
                };

                for _ in 0..repeats {
                    self.push_extent_marker();
                    for expr in &stream.stream {
                        if let Expression::List(list) = expr.as_ref() {
                            if list.is_empty() {
                                break;
                            }
                        }
                        self.encode(expr, level)?;
                    }
                    self.pop_extend_marker();
                }

                self.flush_level(level)?;

                if !stream.bit_spec.is_empty() {
                    self.bitspec_scope.pop();
                }
            }
            Expression::BitField { .. } => {
                let (bits, length) = expr.bitfield(&self.vars)?;

                if !(0..64).contains(&length) {
                    return Err("bitfields of {length} not supported".into());
                }

                self.add_bits(bits, length, level)?;
            }
            Expression::List(list) => {
                for expr in list {
                    self.encode(expr, level)?;
                }
            }
            _ => unreachable!(),
        }

        Ok(())
    }

    fn has_trailing_pulse(&self) -> bool {
        (self.raw.len() % 2) != 0
    }
}

impl Unit {
    pub(crate) fn eval(&self, v: i64, spec: &GeneralSpec) -> Result<i64, String> {
        match self {
            Unit::Units if spec.unit == 0.0 => Err("cannot use units when unit set to 0".into()),
            Unit::Units => Ok((v as f64 * spec.unit) as i64),
            Unit::Microseconds => Ok(v),
            Unit::Milliseconds => Ok(v * 1000),
            Unit::Pulses if spec.carrier == 0 => {
                Err("pulses cannot be used with zero carrier".into())
            }
            Unit::Pulses => Ok(v * 1_000_000 / spec.carrier),
        }
    }

    pub(crate) fn eval_float(&self, v: f64, spec: &GeneralSpec) -> Result<i64, String> {
        match self {
            Unit::Units if spec.unit == 0.0 => Err("cannot use units when unit set to 0".into()),
            Unit::Units => Ok((v * spec.unit) as i64),
            Unit::Microseconds => Ok(v as i64),
            Unit::Milliseconds => Ok((v * 1000.0) as i64),
            Unit::Pulses if spec.carrier == 0 => {
                Err("pulses cannot be used with zero carrier".into())
            }
            Unit::Pulses => Ok((v * 1_000_000.0) as i64 / spec.carrier),
        }
    }
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
