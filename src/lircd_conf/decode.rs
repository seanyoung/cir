use super::Remote;
use irp::{Decoder, InfraredData, Irp, Options, DFA, NFA};
use log::debug;

pub struct LircDecoder<'a> {
    pub remote: &'a Remote,
    pub dfa: DFA,
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
                let message = self.encode_once(raw);
                nfa.add_raw(&message.raw, irp::Event::Down, u32::MAX as i64 + i as i64);
            }

            nfa
        };

        let options = Options {
            name: &self.name,
            aeps: abs_tolerance.unwrap_or(self.aeps as u32),
            eps: rel_tolerance.unwrap_or(self.eps as u32),
            max_gap,
            ..Default::default()
        };

        let dfa = nfa.build_dfa(&options);

        let decoder = Decoder::new(options);

        LircDecoder {
            remote: self,
            dfa,
            decoder,
        }
    }
}

impl<'a> LircDecoder<'a> {
    pub fn input<F>(&mut self, ir: InfraredData, mut callback: F)
    where
        F: FnMut(&'a str, u64),
    {
        self.decoder.dfa_input(ir, &self.dfa, |_, vars| {
            if let Some(decoded) = vars.get("CODE") {
                if self.remote.raw_codes.is_empty() {
                    let (mask, toggle_bit_mask) = if self.remote.toggle_bit_mask.count_ones() == 1 {
                        (!(self.remote.toggle_bit_mask | self.remote.ignore_mask), 0)
                    } else {
                        (!self.remote.ignore_mask, self.remote.toggle_bit_mask)
                    };

                    let decoded = *decoded as u64;
                    if let Some(key_code) = self.remote.codes.iter().find(|code| {
                        let code = code.code[0] & mask;
                        let decoded_masked = decoded & mask;

                        code == decoded_masked
                            || code == (decoded_masked ^ self.remote.repeat_mask)
                            || (code == (decoded_masked ^ toggle_bit_mask))
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
