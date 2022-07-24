//! This library parses IRP, and encodes IR with the provided parameters, and can decode
//! using NFA based decoder compiled from IRP.
//! This can then be used for IR transmission and receiving. You can also use the library to
//! parse and encode pronto hex codes, lirc mode2 pulse / space files, and parse
//! simple raw IR strings.
//!
//! ## About IRP
//!
//! [IRP Notation](http://hifi-remote.com/wiki/index.php?title=IRP_Notation) is a domain-specific language
//! which describes [Consumer IR](https://en.wikipedia.org/wiki/Consumer_IR) protocols. There is a extensive
//! [library](http://hifi-remote.com/wiki/index.php/DecodeIR) of protocols described using IRP.
//!
//! ## Decode IR
//!
//! This example decodes some IR using rc5 protocol. First the IRP notation is parsed, and then
//! we compile the NFA state machine for decoding. Then we create a decoder, which
//! needs some matching parameters, and then we can feed it input. The results can be retrieved
//! with the get() function on the decoder.
//!
//! ```
//! use irp::Irp;
//! use irp::decoder_nfa::InfraredData;
//!
//! let irp = Irp::parse(r#"
//!     {36k,msb,889}<1,-1|-1,1>((1,~F:1:6,T:1,D:5,F:6,^114m)*,T=1-T)
//!     [D:0..31,F:0..127,T@:0..1=0]"#)
//!     .expect("parse should succeed");
//! let nfa = irp.compile().expect("build nfa should succeed");
//! // Create a decoder with 100 microsecond tolerance, 30% relative tolerance,
//! // and 20000 microseconds trailing gap.
//! let mut decoder = nfa.decoder(100, 30, 20000);
//! for ir in InfraredData::from_rawir(
//!     "+940 -860 +1790 -1750 +880 -880 +900 -890 +870 -900 +1750
//!      -900 +890 -910 +840 -920 +870 -920 +840 -920 +870 -1810 +840 -125000").unwrap() {
//!     decoder.input(ir);
//! }
//! let res = decoder.get().unwrap();
//! assert_eq!(res["F"], 1);
//! assert_eq!(res["D"], 30);
//! ```
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
//! means *gap*, which means absense of light, i.e. off, for N microseconds. This continues to alternate. The
//! leading + and - also mean *flash* and *gap*.
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
pub mod decoder_nfa;
mod encode;
mod expression;
mod graphviz;
pub mod mode2;
mod parser;
mod pronto;
pub mod protocols;
pub mod rawir;
#[cfg(test)]
mod tests;

include!(concat!(env!("OUT_DIR"), "/irp.rs"));

use std::collections::HashMap;
use std::rc::Rc;

#[derive(Debug, PartialEq, Eq)]
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

    /// Do we have a trailing gap
    pub fn has_trailing_gap(&self) -> bool {
        let len = self.raw.len();

        len > 0 && (len % 2) == 0
    }

    /// Remove any trailing gap
    pub fn remove_trailing_gap(&mut self) {
        if self.has_trailing_gap() {
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
