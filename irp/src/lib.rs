#![doc = include_str!("../README.md")]
#![allow(clippy::comparison_chain)]

#[cfg(feature = "bpf")]
mod build_bpf;
mod build_dfa;
mod build_nfa;
mod decoder;
mod encode;
mod expression;
mod graphviz;
mod inverse;
mod message;
mod parser;
mod pronto;
mod protocols;
mod variants;

pub use build_dfa::DFA;
pub use build_nfa::NFA;
pub use decoder::Decoder;
pub use protocols::Protocol;

use num_rational::Rational64;
use std::{collections::HashMap, fmt, rc::Rc};

#[derive(Debug, PartialEq, Default, Clone, Eq)]
/// An encoded raw infrared message
pub struct Message {
    /// The carrier for the message. None means unknown, Some(0) means unmodulated
    pub carrier: Option<i64>,
    /// The duty cycle if known. Between 1% and 99%
    pub duty_cycle: Option<u8>,
    /// The actual flash and gap information in microseconds. All even entries are flash, odd are gap
    pub raw: Vec<u32>,
}

/// A parsed or generated pronto hex code
#[derive(Debug, PartialEq)]
#[allow(non_snake_case)]
pub enum Pronto {
    LearnedUnmodulated {
        frequency: f64,
        intro: Vec<f64>,
        repeat: Vec<f64>,
    },
    LearnedModulated {
        frequency: f64,
        intro: Vec<f64>,
        repeat: Vec<f64>,
    },
    Rc5 {
        D: u8,
        F: u8,
    },
    Rc5x {
        D: u8,
        S: u8,
        F: u8,
    },
    Rc6 {
        D: u8,
        F: u8,
    },
    Nec1 {
        D: u8,
        S: u8,
        F: u8,
    },
}

/// A parsed IRP notation, which can be used for encoding and decoding
#[derive(Debug)]
pub struct Irp {
    general_spec: GeneralSpec,
    stream: Rc<Expression>,
    definitions: Vec<Expression>,
    pub parameters: Vec<ParameterSpec>,
    variants: [Option<Rc<Expression>>; 3],
}

/// The general spec for an IRP
#[derive(Debug)]
struct GeneralSpec {
    duty_cycle: Option<u8>,
    carrier: Rational64,
    lsb: bool,
    unit: Rational64,
}

/// Unit suffix for a duration
#[derive(PartialEq, Copy, Hash, Eq, Clone, Debug)]
enum Unit {
    Units,
    Microseconds,
    Milliseconds,
    Pulses,
}

/// The repeat marker for a stream within an IRP
#[derive(PartialEq, Eq, Debug, Hash, Clone)]
enum RepeatMarker {
    Any,
    OneOrMore,
    Count(i64),
    CountOrMore(i64),
}

/// A stream within an IRP
#[derive(PartialEq, Eq, Debug, Hash, Clone)]
struct Stream {
    bit_spec: Vec<Rc<Expression>>,
    stream: Vec<Rc<Expression>>,
    repeat: Option<RepeatMarker>,
}

/// An expression within an IRP
#[derive(PartialEq, Eq, Debug, Hash, Clone)]
enum Expression {
    FlashConstant(Rational64, Unit),
    GapConstant(Rational64, Unit),
    ExtentConstant(Rational64, Unit),
    FlashIdentifier(String, Unit),
    GapIdentifier(String, Unit),
    ExtentIdentifier(String, Unit),
    Assignment(String, Rc<Expression>),
    Number(i64),
    Identifier(String),
    BitField {
        value: Rc<Expression>,
        reverse: bool,
        length: Rc<Expression>,
        offset: Option<Rc<Expression>>,
    },
    InfiniteBitField {
        value: Rc<Expression>,
        offset: Rc<Expression>,
    },
    Complement(Rc<Expression>),
    Not(Rc<Expression>),
    Negative(Rc<Expression>),
    BitCount(Rc<Expression>),

    Power(Rc<Expression>, Rc<Expression>),
    Multiply(Rc<Expression>, Rc<Expression>),
    Divide(Rc<Expression>, Rc<Expression>),
    Modulo(Rc<Expression>, Rc<Expression>),
    Add(Rc<Expression>, Rc<Expression>),
    Subtract(Rc<Expression>, Rc<Expression>),

    ShiftLeft(Rc<Expression>, Rc<Expression>),
    ShiftRight(Rc<Expression>, Rc<Expression>),

    LessEqual(Rc<Expression>, Rc<Expression>),
    Less(Rc<Expression>, Rc<Expression>),
    Greater(Rc<Expression>, Rc<Expression>),
    GreaterEqual(Rc<Expression>, Rc<Expression>),
    Equal(Rc<Expression>, Rc<Expression>),
    NotEqual(Rc<Expression>, Rc<Expression>),

    BitwiseAnd(Rc<Expression>, Rc<Expression>),
    BitwiseOr(Rc<Expression>, Rc<Expression>),
    BitwiseXor(Rc<Expression>, Rc<Expression>),
    Or(Rc<Expression>, Rc<Expression>),
    And(Rc<Expression>, Rc<Expression>),
    Conditional(Rc<Expression>, Rc<Expression>, Rc<Expression>),
    List(Vec<Rc<Expression>>),
    Stream(Stream),
    Variation(Vec<Vec<Rc<Expression>>>),
    BitReverse(Rc<Expression>, i64, i64),
    Log2(Rc<Expression>),
}

/// An IRP parameter specification
#[derive(Debug)]
pub struct ParameterSpec {
    pub name: String,
    /// Retain value, see <http://www.harctoolbox.org/IrpTransmogrifier.html#Persistency+of+variables>
    pub persistent: bool,
    pub min: i64,
    pub max: i64,
    default: Option<Expression>,
}

impl ParameterSpec {
    /// Does this parameter have a default value?s
    pub fn has_default(&self) -> bool {
        self.default.is_some()
    }
}

/// During IRP evaluation, variables may change their value
#[derive(Default, Debug, Clone, PartialEq)]
pub struct Vartable<'a> {
    vars: HashMap<String, (i64, Option<&'a Expression>)>,
}

/// Represents input data to the decoder
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum InfraredData {
    Flash(u32),
    Gap(u32),
    Reset,
}

/// Decoded key event
#[derive(PartialEq, Eq, Debug, Hash, Clone, Copy)]
pub enum Event {
    Down,
    Repeat,
    Up,
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Event::Down => write!(f, "down"),
            Event::Repeat => write!(f, "repeat"),
            Event::Up => write!(f, "up"),
        }
    }
}

/// Options for the decoder
#[derive(Default, Debug, Clone)]
pub struct Options<'a> {
    /// Name of the decoder
    pub name: &'a str,
    // Name of the source file
    pub source: &'a str,
    /// Absolute tolerance in microseconds
    pub aeps: u32,
    /// Relative tolerance in percentage
    pub eps: u32,
    /// Maximum gap the input will contains
    pub max_gap: u32,
    /// If non-zero, repeated presses can be XORed with this value
    #[cfg(feature = "bpf")]
    pub repeat_mask: u64,
    /// Protocol no which will be passed to bpf_rc_keydown()
    pub protocol: u32,
    /// If Some(path) the nfa intermediate file will be saved
    pub nfa: bool,
    /// If Some(path) the dfa intermediate file will be saved
    pub dfa: bool,
    /// If Some(path) the llvm IR intermediate file will be saved
    #[cfg(feature = "bpf")]
    pub llvm_ir: bool,
    /// If Some(path) the assembly intermediate file will be saved
    #[cfg(feature = "bpf")]
    pub assembly: bool,
    /// If Some(path) the object intermediate file will be saved
    #[cfg(feature = "bpf")]
    pub object: bool,
}
