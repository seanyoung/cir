//! This library parses IRP, and encodes IR with the provided parameters. This can then be used for IR transmission.
//! You can also use the library to parse and encode pronto hex codes, lirc mode2 pulse / space files, and parse
//! simple raw IR strings.
//!
//! A decoder is in the works but this is still some time away.
//!
//! ## About IRP
//!
//! [IRP Notation](http://hifi-remote.com/wiki/index.php?title=IRP_Notation) is a domain-specific language
//! which describes [Consumer IR](https://en.wikipedia.org/wiki/Consumer_IR) protocols. There is a extensive
//! [library](http://hifi-remote.com/wiki/index.php/DecodeIR) of protocols described using IRP.
//!
//! ## An example of how to encode NEC1
//!
//! This example sets some parameters, encodes and then simply prints the result.
//!
//! ```
//! use irp::Irp;
//
//! let mut vars = irp::Vartable::new();
//! vars.set(String::from("D"), 255, 8);
//! vars.set(String::from("S"), 52, 8);
//! vars.set(String::from("F"), 1, 8);
//! let irp = Irp::parse(r#"
//!     {38.4k,564}<1,-1|1,-3>(16,-8,D:8,S:8,F:8,~F:8,1,^108m,(16,-4,1,^108m)*)
//!     [D:0..255,S:0..255=255-D,F:0..255]"#)
//!     .expect("parse should succeed");
//! let message = irp.encode(vars, 0).expect("encode should succeed");
//! if let Some(carrier) = &message.carrier {
//!     println!("carrier: {}Hz", carrier);
//! }
//! if let Some(duty_cycle) = &message.duty_cycle {
//!     println!("duty cycle: {}%", duty_cycle);
//! }
//! println!("{}", message.print_rawir());
//! ```
//!
//! The output is in raw ir format, which looks like "+9024 -4512 +564 -1692 +564 -1692 +564 -1692 +564 ...". The first
//! entry in this array is *flash*, which means infrared light should be on for N microseconds, and every even entry
//! means *gap*, which means absense of light, i.e. off, for N microseconds. This continues to alternate. The leading
//! + and - also mean *flash* and *gap*.
//!
//! The IRP can also be encoded to pronto hex codes. Pronto hex codes have a repeating part, so no repeat argument is needed.
//!
//! ```
//! use irp::Irp;
//
//! let mut vars = irp::Vartable::new();
//! vars.set(String::from("F"), 1, 8);
//! let irp = Irp::parse("{40k,600}<1,-1|2,-1>(4,-1,F:8,^45m)[F:0..255]")
//!     .expect("parse should succeed");
//! let pronto = irp.encode_pronto(vars).expect("encode should succeed");
//! println!("pronto:{}", pronto);
//! ```
//!
//! ## Parsing pronto hex codes
//!
//! The [Pronto Hex](http://www.hifi-remote.com/wiki/index.php?title=Working_With_Pronto_Hex) is made popular by the
//! Philips Pronto universal remote. The format is a series of 4 digits hex numbers. This library can parse the long
//! codes, there is no support for the short format yet.
//!
//! ```
//! use irp::Pronto;
//
//! let pronto = Pronto::parse(r#"
//!     0000 006C 0000 0022 00AD 00AD 0016 0041 0016 0041 0016 0041 0016 0016 0016
//!     0016 0016 0016 0016 0016 0016 0016 0016 0041 0016 0041 0016 0041 0016 0016
//!     0016 0016 0016 0016 0016 0016 0016 0016 0016 0016 0016 0041 0016 0016 0016
//!     0016 0016 0016 0016 0016 0016 0016 0016 0016 0016 0041 0016 0016 0016 0041
//!     0016 0041 0016 0041 0016 0041 0016 0041 0016 0041 0016 06FB
//!     "#).expect("parse should succeed");
//! let message = pronto.encode(0);
//! if let Some(carrier) = &message.carrier {
//!     println!("carrier: {}Hz", carrier);
//! }
//! println!("{}", message.print_rawir());
//! ```
//!
//! ## Parsing lirc mode2 pulse space files
//!
//! This format was made popular by the [`mode2` tool](https://www.lirc.org/html/mode2.html), which prints a single line
//! for each flash and gap, but then calls them `pulse` and `space`. It looks like so:
//!
//! ```skip
//! carrier 38400
//! pulse 9024
//! space 4512
//! pulse 4512
//! ```
//!
//! This is an example of how to parse this. The result is printed in the more concise raw ir format.
//!
//! ```
//! let message = irp::mode2::parse(r#"
//!     carrier 38400
//!     pulse 9024
//!     space 4512
//!     pulse 4512
//! "#).expect("parse should succeed");
//! if let Some(carrier) = &message.carrier {
//!     println!("carrier: {}Hz", carrier);
//! }
//! if let Some(duty_cycle) = &message.duty_cycle {
//!     println!("duty cycle: {}%", duty_cycle);
//! }
//! println!("{}", message.print_rawir());
//! ```
//!
//! ## Parsing raw ir format
//!
//! The raw ir format looks like "+100 -100 +100". The leading `+` and `-` may be omitted, but if present they are
//! checked for consistency. The parse function returns a `Vec<u32>`.
//!
//! ```
//! let rawir: Vec<u32> = irp::rawir::parse("+100 -100 +100").expect("parse should succeed");
//! println!("{}", irp::rawir::print_to_string(&rawir));
//! ```

mod build_nfa;
mod encode;
mod graphviz;
mod match_nfa;
pub mod mode2;
mod parser;
mod pronto;
pub mod protocols;
pub mod rawir;
#[cfg(test)]
mod tests;

include!(concat!(env!("OUT_DIR"), "/irp.rs"));

use std::collections::HashMap;
use std::fmt;

#[derive(Debug, PartialEq)]
/// An encoded raw infrared message
pub struct Message {
    /// The carrier for the message. None means unknown, Some(0) means unmodulated
    pub carrier: Option<i64>,
    /// The duty cycle if known. Between 1% and 99%
    pub duty_cycle: Option<u8>,
    /// The actual flash and gap information in microseconds. All even entries are flash, odd are gap
    pub raw: Vec<u32>,
}

impl Message {
    /// Create an empty packet
    pub fn new() -> Self {
        Message {
            carrier: None,
            duty_cycle: None,
            raw: Vec::new(),
        }
    }

    /// Concatenate to packets
    pub fn extend(&mut self, other: &Message) {
        if self.carrier.is_none() {
            self.carrier = other.carrier;
        }

        if self.duty_cycle.is_none() {
            self.duty_cycle = other.duty_cycle;
        }

        self.raw.extend_from_slice(&other.raw);
    }

    /// Do we have a trailing space
    pub fn has_trailing_space(&self) -> bool {
        let len = self.raw.len();

        len > 0 && (len % 2) == 0
    }

    /// Remove any trailing space
    pub fn remove_trailing_space(&mut self) {
        if self.has_trailing_space() {
            self.raw.pop();
        }
    }

    /// Print the flash and gap information as an raw ir string
    pub fn print_rawir(&self) -> String {
        rawir::print_to_string(&self.raw)
    }
}

impl Default for Message {
    fn default() -> Self {
        Message::new()
    }
}

/// A parsed or generated pronto hex code
#[derive(Debug, PartialEq)]
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
}

/// A parsed IRP notation, which can be used for encoding (and decoding in the future)
pub struct Irp {
    general_spec: GeneralSpec,
    stream: Expression,
    definitions: Vec<Expression>,
    parameters: Vec<ParameterSpec>,
}

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
    bit_spec: Vec<Expression>,
    stream: Vec<Expression>,
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
    Assignment(String, Box<Expression>),
    Number(i64),
    Identifier(String),
    BitField {
        value: Box<Expression>,
        reverse: bool,
        length: Box<Expression>,
        skip: Option<Box<Expression>>,
    },
    InfiniteBitField {
        value: Box<Expression>,
        skip: Box<Expression>,
    },
    Complement(Box<Expression>),
    Not(Box<Expression>),
    Negative(Box<Expression>),
    BitCount(Box<Expression>),

    Power(Box<Expression>, Box<Expression>),
    Multiply(Box<Expression>, Box<Expression>),
    Divide(Box<Expression>, Box<Expression>),
    Modulo(Box<Expression>, Box<Expression>),
    Add(Box<Expression>, Box<Expression>),
    Subtract(Box<Expression>, Box<Expression>),

    ShiftLeft(Box<Expression>, Box<Expression>),
    ShiftRight(Box<Expression>, Box<Expression>),

    LessEqual(Box<Expression>, Box<Expression>),
    Less(Box<Expression>, Box<Expression>),
    More(Box<Expression>, Box<Expression>),
    MoreEqual(Box<Expression>, Box<Expression>),
    Equal(Box<Expression>, Box<Expression>),
    NotEqual(Box<Expression>, Box<Expression>),

    BitwiseAnd(Box<Expression>, Box<Expression>),
    BitwiseOr(Box<Expression>, Box<Expression>),
    BitwiseXor(Box<Expression>, Box<Expression>),
    Or(Box<Expression>, Box<Expression>),
    And(Box<Expression>, Box<Expression>),
    Ternary(Box<Expression>, Box<Expression>, Box<Expression>),
    List(Vec<Expression>),
    Stream(IrStream),
    Variation(Vec<Vec<Expression>>),
}

impl fmt::Display for Expression {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Expression::Number(v) => write!(f, "{}", v),
            Expression::Identifier(id) => write!(f, "{}", id),
            Expression::Add(left, right) => write!(f, "({} + {})", left, right),
            Expression::Subtract(left, right) => write!(f, "({} - {})", left, right),
            Expression::Multiply(left, right) => write!(f, "({} * {})", left, right),
            Expression::Modulo(left, right) => write!(f, "({} % {})", left, right),
            Expression::BitwiseOr(left, right) => write!(f, "({} | {})", left, right),
            Expression::BitwiseAnd(left, right) => write!(f, "({} & {})", left, right),
            Expression::ShiftLeft(left, right) => write!(f, "({} << {})", left, right),
            Expression::ShiftRight(left, right) => write!(f, "({} >> {})", left, right),

            Expression::Equal(left, right) => write!(f, "{} == {}", left, right),
            Expression::More(left, right) => write!(f, "{} > {}", left, right),
            Expression::MoreEqual(left, right) => write!(f, "{} >= {}", left, right),
            Expression::Less(left, right) => write!(f, "{} < {}", left, right),
            Expression::LessEqual(left, right) => write!(f, "{} <= {}", left, right),

            Expression::Complement(expr) => write!(f, "~{}", expr),
            Expression::BitCount(expr) => write!(f, "#({})", expr),

            _ => panic!("{:?}", self),
        }
    }
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

pub use build_nfa::NFA;
pub use match_nfa::InfraredData;
pub use match_nfa::Matcher;
