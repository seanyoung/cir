#![doc = include_str!("../README.md")]

mod build_nfa;
mod decoder_nfa;
mod encode;
mod expression;
mod graphviz;
mod inverse;
mod message;
mod parser;
mod pronto;
pub mod protocols;
pub mod rawir;
#[cfg(test)]
mod tests;

use std::{collections::HashMap, rc::Rc};

#[derive(Debug, PartialEq, Default, Eq)]
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
///
#[derive(Debug)]
pub struct Irp {
    general_spec: GeneralSpec,
    stream: Expression,
    definitions: Vec<Expression>,
    parameters: Vec<ParameterSpec>,
}

#[derive(Debug)]
struct GeneralSpec {
    duty_cycle: Option<u8>,
    carrier: Option<i64>,
    lsb: bool,
    unit: f64,
}

#[derive(PartialEq, Copy, Clone, Debug)]
enum Unit {
    Units,
    Microseconds,
    Milliseconds,
    Pulses,
}

#[derive(PartialEq, Debug, Clone)]
enum RepeatMarker {
    Any,
    OneOrMore,
    Count(i64),
    CountOrMore(i64),
}

#[derive(PartialEq, Debug, Clone)]
struct IrStream {
    bit_spec: Vec<Rc<Expression>>,
    stream: Vec<Rc<Expression>>,
    repeat: Option<RepeatMarker>,
}

#[derive(PartialEq, Debug, Clone)]
enum Expression {
    FlashConstant(f64, Unit),
    GapConstant(f64, Unit),
    ExtentConstant(f64, Unit),
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
        skip: Option<Rc<Expression>>,
    },
    InfiniteBitField {
        value: Rc<Expression>,
        skip: Rc<Expression>,
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
    More(Rc<Expression>, Rc<Expression>),
    MoreEqual(Rc<Expression>, Rc<Expression>),
    Equal(Rc<Expression>, Rc<Expression>),
    NotEqual(Rc<Expression>, Rc<Expression>),

    BitwiseAnd(Rc<Expression>, Rc<Expression>),
    BitwiseOr(Rc<Expression>, Rc<Expression>),
    BitwiseXor(Rc<Expression>, Rc<Expression>),
    Or(Rc<Expression>, Rc<Expression>),
    And(Rc<Expression>, Rc<Expression>),
    Ternary(Rc<Expression>, Rc<Expression>, Rc<Expression>),
    List(Vec<Rc<Expression>>),
    Stream(IrStream),
    Variation(Vec<Vec<Rc<Expression>>>),
    BitReverse(Rc<Expression>, i64, i64),
    Log2(Rc<Expression>),
}

#[derive(Debug)]
struct ParameterSpec {
    pub name: String,
    #[allow(unused)]
    pub memory: bool,
    pub min: Expression,
    pub max: Expression,
    pub default: Option<Expression>,
}

/// During IRP evaluation, variables may change their value
#[derive(Default, Debug, Clone)]
pub struct Vartable<'a> {
    vars: HashMap<String, (i64, u8, Option<&'a Expression>)>,
}

/// Represents input data to the decoder
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum InfraredData {
    Flash(u32),
    Gap(u32),
    Reset,
}

pub use build_nfa::NFA;
pub use decoder_nfa::Decoder;
