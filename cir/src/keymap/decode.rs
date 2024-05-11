use crate::keymap::LinuxProtocol;

use super::Keymap;
use irp::{Decoder, InfraredData, Irp, Options, DFA, NFA};
use log::debug;

pub struct KeymapDecoder<'a> {
    pub remote: &'a Keymap,
    pub dfa: Vec<DFA>,
    pub decoder: Vec<Decoder<'a>>,
}

impl Keymap {
    /// Create DFAs for this remote
    pub fn build_dfa<'b>(&'b self, options: &Options<'b>) -> Result<Vec<DFA>, String> {
        let nfa = if self.raw.is_empty() {
            let mut irps = Vec::new();
            if let Some(irp) = &self.irp {
                irps.push(irp.as_str());
            } else {
                if self.variant.is_none() {
                    if let Some(protocols) = LinuxProtocol::find_decoder(&self.protocol) {
                        // TODO: ideally the decoder tells us which protocol was decoded
                        irps = protocols.iter().filter_map(|p| p.irp).collect();
                    }
                }

                let protocol = self.variant.as_ref().unwrap_or(&self.protocol);

                if irps.is_empty() {
                    if let Some(linux_protocol) = LinuxProtocol::find_like(protocol) {
                        if let Some(irp) = linux_protocol.irp {
                            irps.push(irp);
                        } else {
                            return Err(format!("unable to decode protocol {protocol}"));
                        }
                    } else {
                        return Err(format!("unknown protocol {protocol}"));
                    }
                }
            };

            irps.iter()
                .map(|irp| {
                    debug!("decoding irp {irp} for keymap {}", self.name);

                    let irp = Irp::parse(irp).unwrap();

                    irp.build_nfa().unwrap()
                })
                .collect()
        } else {
            let mut nfa = NFA::default();

            for (i, raw) in self.raw.iter().enumerate() {
                let message = self.encode_raw(raw, 0);
                nfa.add_raw(&message.raw, irp::Event::Down, u32::MAX as i64 + i as i64);
            }

            vec![nfa]
        };

        // TODO: merge NFAs so we end up with on DFA
        Ok(nfa.iter().map(|nfa| nfa.build_dfa(options)).collect())
    }

    /// Create a decoder for this remote
    pub fn decoder<'b>(&'b self, options: Options<'b>) -> Result<KeymapDecoder<'b>, String> {
        let dfa = self.build_dfa(&options)?;

        let decoder = vec![Decoder::new(options); dfa.len()];

        Ok(KeymapDecoder {
            remote: self,
            dfa,
            decoder,
        })
    }
}

impl<'a> KeymapDecoder<'a> {
    pub fn input<F>(&mut self, ir: InfraredData, mut callback: F)
    where
        F: FnMut(&'a str, u64),
    {
        for i in 0..self.dfa.len() {
            self.decoder[i].dfa_input(ir, &self.dfa[i], |_, vars| {
                // FIXME: vars do not have to be called CODE
                if let Some(decoded) = vars.get("CODE") {
                    let decoded = *decoded as u64;
                    if self.remote.raw.is_empty() {
                        if let Some(key_code) = self.remote.scancodes.get(&decoded) {
                            callback(key_code, decoded);
                        }
                    } else {
                        let decoded: usize = decoded as usize - u32::MAX as usize;

                        callback(&self.remote.raw[decoded].keycode, decoded as u64);
                    }
                }
            })
        }
    }

    pub fn reset(&mut self) {
        self.decoder.iter_mut().for_each(|d| d.reset());
    }
}
