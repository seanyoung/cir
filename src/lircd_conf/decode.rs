use super::{Code, Remote};
use irp::{Decoder, InfraredData, Irp, NFA};
use log::info;

pub struct LircDecoder<'a> {
    pub remote: &'a Remote,
    pub nfa: NFA,
    pub decoder: Decoder<'a>,
}

impl Remote {
    /// Create a decoder for this remote
    pub fn decoder(&self, abs_tolerance: u32, rel_tolerance: u32, max_gap: u32) -> LircDecoder {
        let irp = self.decode_irp();

        info!("decoding irp {irp} for remote {}", self.name);

        let irp = Irp::parse(&irp).unwrap();

        let nfa = irp.compile().unwrap();

        let decoder = Decoder::new(
            abs_tolerance.max(self.aeps as u32),
            rel_tolerance.max(self.eps as u32),
            max_gap,
        );

        LircDecoder {
            remote: self,
            nfa,
            decoder,
        }
    }
}

impl<'a> LircDecoder<'a> {
    pub fn input<F>(&mut self, ir: InfraredData, mut callback: F)
    where
        F: FnMut(u64, Option<&'a Code>),
    {
        self.decoder.input(ir, &self.nfa, |_, vars| {
            let decoded = vars["CODE"] as u64;

            callback(
                decoded,
                self.remote
                    .codes
                    .iter()
                    .find(|code| code.code[0] == decoded),
            );
        })
    }
}