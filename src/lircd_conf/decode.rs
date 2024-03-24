use super::{Code, Remote};
use irp::{Decoder, InfraredData, Irp, NFA};
use log::debug;

pub struct LircDecoder<'a> {
    pub remote: &'a Remote,
    pub nfa: NFA,
    pub decoder: Decoder<'a>,
}

impl Remote {
    /// Create a decoder for this remote
    pub fn decoder(
        &self,
        abs_tolerance: Option<u32>,
        rel_tolerance: Option<u32>,
        max_gap: u32,
    ) -> LircDecoder {
        let irp = self.decode_irp();

        debug!("decoding irp {irp} for remote {}", self.name);

        let irp = Irp::parse(&irp).unwrap();

        let nfa = irp.build_nfa().unwrap();

        let decoder = Decoder::new(
            abs_tolerance.unwrap_or(self.aeps as u32),
            rel_tolerance.unwrap_or(self.eps as u32),
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
        F: FnMut(&'a Code),
    {
        self.decoder.nfa_input(ir, &self.nfa, |_, vars| {
            if let Some(decoded) = vars.get("CODE") {
                let decoded = *decoded as u64;
                if let Some(key_code) = self.remote.codes.iter().find(|code| {
                    code.code[0] == decoded || code.code[0] == (decoded ^ self.remote.repeat_mask)
                }) {
                    callback(key_code);
                }
            }
        })
    }

    pub fn reset(&mut self) {
        self.decoder.reset();
    }
}
