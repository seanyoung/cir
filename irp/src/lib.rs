//! This library parses IRP, and encodes IR with the provided parameters. This can then be used for IR transmission.
//! You can also use the library to parse and encode pronto hex codes, lirc mode2 pulse / space files, and parse
//! simple raw IR strings.
//!
//! A decoder is in the works but this is still some time away.
//!
//! ## About IRP
//!
//! [IRP Notation](http://hifi-remote.com/wiki/index.php?title=IRP_Notation) is a mini-language
//! which describes [Consumer IR](https://en.wikipedia.org/wiki/Consumer_IR) protocols. There is a extensive
//! [library](http://hifi-remote.com/wiki/index.php/DecodeIR) of protocols described using IRP.
//!
//! ## An example of how to encode NEC1
//!
//! This example sets some parameters, encodes and then simply prints the result.
//!
//!     let mut vars = irp::encode::Vartable::new();
//!     vars.set(String::from("D"), 255, 8);
//!     vars.set(String::from("S"), 52, 8);
//!     vars.set(String::from("F"), 1, 8);
//!     let message = irp::encode::encode(
//!         "{38.4k,564}<1,-1|1,-3>(16,-8,D:8,S:8,F:8,~F:8,1,^108m,(16,-4,1,^108m)*) [D:0..255,S:0..255=255-D,F:0..255]",
//!         vars,
//!         0).expect("encode should succeed");
//!     if let Some(carrier) = &message.carrier {
//!         println!("carrier: {}Hz", carrier);
//!     }
//!     if let Some(duty_cycle) = &message.duty_cycle {
//!         println!("duty cycle: {}%", duty_cycle);
//!     }
//!     println!("{}", message.print_rawir());
//!
//! The output is in raw ir format, which looks like "+9024 -4512 +564 -1692 +564 -1692 +564 -1692 +564 ...". The first
//! entry in this array is *flash*, which means infrared light should be on for N microseconds, and every even entry
//! means *gap*, which means absense of light, i.e. off, for N microseconds. This continues to alternate. The leading
//! + and - also mean *flash* and *gap*.
//!
//! ## Parsing pronto hex codes
//!
//! The [Pronto Hex](http://www.hifi-remote.com/wiki/index.php?title=Working_With_Pronto_Hex) is made popular by the
//! Philips Pronto universal remote. The format is a series of 4 digits hex numbers. This library can parse the long
//! codes, there is no support for the short format yet.
//!
//!     let pronto = irp::pronto::parse(r#"
//!         0000 006C 0000 0022 00AD 00AD 0016 0041 0016 0041 0016 0041 0016 0016 0016
//!         0016 0016 0016 0016 0016 0016 0016 0016 0041 0016 0041 0016 0041 0016 0016
//!         0016 0016 0016 0016 0016 0016 0016 0016 0016 0016 0016 0041 0016 0016 0016
//!         0016 0016 0016 0016 0016 0016 0016 0016 0016 0016 0041 0016 0016 0016 0041
//!         0016 0041 0016 0041 0016 0041 0016 0041 0016 0041 0016 06FB
//!         "#).expect("parse should succeed");
//!     let message = pronto.encode(0);
//!     if let Some(carrier) = &message.carrier {
//!         println!("carrier: {}Hz", carrier);
//!     }
//!     println!("{}", message.print_rawir());
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
//!     let message = irp::mode2::parse(r#"
//!         carrier 38400
//!         pulse 9024
//!         space 4512
//!         pulse 4512
//!     "#).expect("parse should succeed");
//!     if let Some(carrier) = &message.carrier {
//!         println!("carrier: {}Hz", carrier);
//!     }
//!     if let Some(duty_cycle) = &message.duty_cycle {
//!         println!("duty cycle: {}%", duty_cycle);
//!     }
//!     println!("{}", message.print_rawir());
//!
//! ## Parsing raw ir format
//!
//! The raw ir format looks like "+100 -100 +100". The leading `+` and `-` may be omitted, but if present they are
//! checked for consistency. The parse function returns a `Vec<u32>`.
//!
//!     let rawir: Vec<u32> = irp::rawir::parse("+100 -100 +100").expect("parse should succeed");
//!     println!("{}", irp::rawir::print_to_string(&rawir));
//!

mod ast;
#[rustfmt::skip]
mod irp;
pub mod encode;
pub mod mode2;
mod parser;
pub mod pronto;
pub mod protocols;
pub mod rawir;
#[cfg(test)]
mod tests;

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
    /// Print the flash and gap information as an raw ir string
    pub fn print_rawir(&self) -> String {
        rawir::print_to_string(&self.raw)
    }
}
