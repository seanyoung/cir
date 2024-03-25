use super::Remote;
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
        let nfa = if self.raw_codes.is_empty() {
            let irp = self.decode_irp();

            debug!("decoding irp {irp} for remote {}", self.name);

            let irp = Irp::parse(&irp).unwrap();

            irp.build_nfa().unwrap()
        } else {
            let mut nfa = NFA::default();

            for (i, raw) in self.raw_codes.iter().enumerate() {
                nfa.add_raw(&raw.rawir, irp::Event::Down, u32::MAX as i64 + i as i64);
            }

            nfa
        };

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
        F: FnMut(&'a str, u64),
    {
        self.decoder.nfa_input(ir, &self.nfa, |_, vars| {
            if let Some(decoded) = vars.get("CODE") {
                if self.remote.raw_codes.is_empty() {
                    // TODO: ignore mask, toggle_bit_mask with many bits set
                    let mask = if self.remote.toggle_bit_mask.count_ones() == 1 {
                        !self.remote.toggle_bit_mask
                    } else {
                        !0
                    };

                    let decoded = *decoded as u64;
                    if let Some(key_code) = self.remote.codes.iter().find(|code| {
                        let code = code.code[0] & mask;
                        let decoded_masked = decoded & mask;

                        code == decoded_masked || code == (decoded_masked ^ self.remote.repeat_mask)
                    }) {
                        callback(&key_code.name, decoded);
                    }
                } else {
                    let decoded: usize = *decoded as usize - u32::MAX as usize;

                    callback(&self.remote.raw_codes[decoded].name, decoded as u64);
                }
            }
        })
    }

    pub fn reset(&mut self) {
        self.decoder.reset();
    }
}
