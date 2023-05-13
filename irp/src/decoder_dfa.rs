use super::{
    build_dfa::DFA,
    build_nfa::{Action, Edge},
    InfraredData, Vartable,
};
use crate::Event;
use log::trace;
use std::collections::HashMap;

/// NFA Decoder state
#[derive(Debug)]
pub struct DFADecoder<'a> {
    pos: usize,
    vartab: Vartable<'a>,
    abs_tolerance: u32,
    rel_tolerance: u32,
    max_gap: u32,
}

impl<'a> DFADecoder<'a> {
    /// Create a decoder with parameters. abs_tolerance is microseconds, rel_tolerance is in percentage,
    /// and trailing gap is the minimum gap in microseconds which must follow.
    pub fn new(abs_tolerance: u32, rel_tolerance: u32, max_gap: u32) -> DFADecoder<'a> {
        DFADecoder {
            pos: 0,
            vartab: Vartable::new(),
            abs_tolerance,
            rel_tolerance,
            max_gap,
        }
    }
}

impl<'a> DFADecoder<'a> {
    /// Reset decoder state
    pub fn reset(&mut self) {
        self.pos = 0;
    }

    fn tolerance_eq(&self, expected: u32, received: u32) -> bool {
        let diff = expected.abs_diff(received);

        if diff <= self.abs_tolerance {
            true
        } else {
            // upcast to u64 since diff * 100 may overflow
            ((diff as u64 * 100) / expected as u64) <= self.rel_tolerance as u64
        }
    }

    /// Feed infrared data to the decoder
    pub fn input<F>(&mut self, ir: InfraredData, dfa: &DFA, mut callback: F)
    where
        F: FnMut(Event, HashMap<String, i64>),
    {
        if ir == InfraredData::Reset {
            trace!("decoder reset");
            self.reset();
            return;
        }

        if self.pos == 0 {
            let success = self.run_actions(0, dfa, &mut callback);

            self.vartab.set("$down".into(), 0);

            assert!(success);
        }

        let mut input = Some(ir);

        loop {
            let mut stuff_to_do = false;
            let edges = &dfa.verts[self.pos].edges;

            trace!("pos:{} ir:{:?} vars:{}", self.pos, input, self.vartab);

            for edge in edges {
                //trace!("edge:{:?}", edge);

                match edge {
                    Edge::Flash {
                        length: expected,
                        complete,
                        dest,
                    } => {
                        let expected = expected.eval(&self.vartab).unwrap();

                        if let Some(ir @ InfraredData::Flash(received)) = input {
                            if self.tolerance_eq(expected as u32, received) {
                                trace!(
                                    "matched flash {} (expected {}) => {}",
                                    received,
                                    expected,
                                    dest
                                );

                                let success = self.run_actions(*dest, dfa, &mut callback);
                                if success {
                                    self.pos = *dest;
                                    input = None;
                                    stuff_to_do = true;
                                } else {
                                    self.reset();
                                }
                            } else if !complete && received > expected as u32 {
                                trace!(
                                    "matched flash {} (expected {}) (incomplete consume) => {}",
                                    received,
                                    expected,
                                    dest
                                );

                                let success = self.run_actions(*dest, dfa, &mut callback);
                                if success {
                                    self.pos = *dest;
                                    input = Some(ir.consume(expected as u32));
                                    stuff_to_do = true;
                                } else {
                                    self.reset();
                                }
                            }
                        }
                    }
                    Edge::Gap {
                        length: expected,
                        complete,
                        dest,
                    } => {
                        let expected = expected.eval(&self.vartab).unwrap();

                        if let Some(ir @ InfraredData::Gap(received)) = input {
                            if expected >= self.max_gap as i64 {
                                if received >= self.max_gap {
                                    trace!(
                                        "large gap matched gap {} (expected {}) => {}",
                                        received,
                                        expected,
                                        dest
                                    );

                                    let success = self.run_actions(*dest, dfa, &mut callback);
                                    if success {
                                        self.pos = *dest;
                                        stuff_to_do = true;
                                    } else {
                                        self.reset();
                                    }
                                }
                            } else if self.tolerance_eq(expected as u32, received) {
                                trace!(
                                    "matched gap {} (expected {}) => {}",
                                    received,
                                    expected,
                                    dest
                                );

                                let success = self.run_actions(*dest, dfa, &mut callback);
                                if success {
                                    self.pos = *dest;
                                    stuff_to_do = true;
                                    input = None;
                                } else {
                                    self.reset();
                                }
                            } else if !complete && received > expected as u32 {
                                trace!(
                                    "matched gap {} (expected {}) (incomplete consume) => {}",
                                    received,
                                    expected,
                                    dest
                                );

                                let success = self.run_actions(*dest, dfa, &mut callback);
                                if success {
                                    self.pos = *dest;
                                    stuff_to_do = true;
                                    input = Some(ir.consume(expected as u32));
                                } else {
                                    self.reset();
                                }
                            }
                        }
                    }
                    Edge::Branch(dest) => {
                        let success = self.run_actions(*dest, dfa, &mut callback);

                        if success {
                            self.pos = *dest;
                            stuff_to_do = true;
                        } else {
                            self.reset();
                        }
                    }
                    Edge::BranchCond { expr, yes, no } => {
                        let cond = expr.eval(&self.vartab).unwrap();

                        let dest = if cond != 0 { *yes } else { *no };

                        let success = self.run_actions(dest, dfa, &mut callback);

                        if success {
                            trace!(
                                "conditional branch {}: {}: destination {}",
                                expr,
                                cond != 0,
                                dest
                            );

                            self.pos = dest;
                            stuff_to_do = true;
                        } else {
                            self.reset();
                        }
                    }
                    Edge::MayBranchCond { expr, dest } => {
                        let cond = expr.eval(&self.vartab).unwrap();

                        if cond != 0 {
                            let success = self.run_actions(*dest, dfa, &mut callback);

                            if success {
                                self.pos = *dest;

                                trace!(
                                    "conditional branch {}: {}: destination {}",
                                    expr,
                                    cond != 0,
                                    dest
                                );
                            }
                            stuff_to_do = true;
                        } else {
                            self.reset();
                        }
                    }
                }
            }
            if !stuff_to_do {
                break;
            }
        }
    }

    fn run_actions<F>(&mut self, pos: usize, dfa: &DFA, callback: &mut F) -> bool
    where
        F: FnMut(Event, HashMap<String, i64>),
    {
        for a in &dfa.verts[pos].actions {
            match a {
                Action::Set { var, expr } => {
                    let val = expr.eval(&self.vartab).unwrap();
                    trace!("set {} = {} = {}", var, expr, val);
                    self.vartab.vars.insert(var.to_string(), (val, None));
                }
                Action::AssertEq { left, right } => {
                    if let (Ok(left_val), Ok(right_val)) =
                        (left.eval(&self.vartab), right.eval(&self.vartab))
                    {
                        if left_val != right_val {
                            trace!(
                                "assert FAIL {} != {} ({} != {})",
                                left,
                                right,
                                left_val,
                                right_val
                            );
                            return false;
                        } else {
                            trace!(
                                "assert  {} == {} ({} == {})",
                                left,
                                right,
                                left_val,
                                right_val
                            );
                        }
                    } else {
                        return false;
                    }
                }
                Action::Done(event, include) => {
                    let mut res: HashMap<String, i64> = HashMap::new();

                    for (name, (val, _)) in &self.vartab.vars {
                        if include.contains(name) {
                            trace!("done {}", event);

                            res.insert(name.to_owned(), *val);
                        }
                    }

                    (callback)(*event, res);
                }
            }
        }

        true
    }

    /// Generate a GraphViz dot file and write to the given path
    pub fn dotgraphviz(&self, path: &str, dfa: &DFA) {
        crate::graphviz::graphviz(&dfa.verts, "DFA", &[(self.pos, self.vartab.clone())], path);
    }
}

#[cfg(test)]
mod test {
    use super::{DFADecoder, InfraredData};
    use crate::{Event, Irp};
    use std::collections::HashMap;

    #[test]
    #[ignore]
    fn sony8() {
        // sony 8
        let irp = Irp::parse("{40k,600}<1,-1|2,-1>(4,-1,F:8,^45m)[F:0..255]").unwrap();

        let nfa = irp.build_nfa().unwrap();
        let dfa = nfa.build_dfa();

        let mut res: Vec<(Event, HashMap<String, i64>)> = Vec::new();

        let mut matcher = DFADecoder::new(100, 3, 20000);

        for ir in InfraredData::from_rawir(
            "+2400 -600 +600 -600 +600 -600 +1200 -600 +600 -600 +600 -600 +600 -600 +1200 -600 +1200 -31200").unwrap() {
            matcher.input(ir, &dfa, |ev, vars| res.push((ev, vars)));
        }

        assert_eq!(res.len(), 1);

        let (event, res) = &res[0];

        assert_eq!(*event, Event::Down);
        assert_eq!(res["F"], 196);
    }

    #[test]
    fn nec() {
        let irp = Irp::parse("{38.4k,564}<1,-1|1,-3>(16,-8,D:8,S:8,F:8,~F:8,1,^108m,(16,-4,1,^108m)*) [D:0..255,S:0..255=255-D,F:0..255]").unwrap();

        let nfa = irp.build_nfa().unwrap();
        let dfa = nfa.build_dfa();

        let mut res: Vec<(Event, HashMap<String, i64>)> = Vec::new();

        let mut matcher = DFADecoder::new(100, 3, 20000);

        for ir in InfraredData::from_rawir(
            "+9024 -4512 +564 -564 +564 -564 +564 -564 +564 -564 +564 -564 +564 -564 +564 -1692 +564 -564 +564 -1692 +564 -1692 +564 -1692 +564 -1692 +564 -1692 +564 -1692 +564 -564 +564 -1692 +564 -564 +564 -564 +564 -1692 +564 -564 +564 -564 +564 -564 +564 -1692 +564 -1692 +564 -1692 +564 -1692 +564 -564 +564 -1692 +564 -1692 +564 -1692 +564 -564 +564 -564 +564 -39756").unwrap() {

            matcher.input(ir, &dfa, |ev, vars| res.push((ev, vars)));
        }

        assert_eq!(res.len(), 1);

        let (event, vars) = &res[0];

        assert_eq!(*event, Event::Down);
        assert_eq!(vars["F"], 196);
        assert_eq!(vars["D"], 64);
        assert_eq!(vars["S"], 191);

        for ir in InfraredData::from_rawir("+9024 -2256 +564 -96156").unwrap() {
            matcher.input(ir, &dfa, |ev, vars| res.push((ev, vars)));
        }

        assert_eq!(res.len(), 2);

        let (event, vars) = &res[1];

        assert_eq!(*event, Event::Repeat);
        assert_eq!(vars["F"], 196);
        assert_eq!(vars["D"], 64);
        assert_eq!(vars["S"], 191);

        for ir in InfraredData::from_rawir("+9024 -2256 +564 -96156").unwrap() {
            matcher.input(ir, &dfa, |ev, vars| res.push((ev, vars)));
        }

        assert_eq!(res.len(), 3);

        let (event, vars) = &res[2];

        assert_eq!(*event, Event::Repeat);
        assert_eq!(vars["F"], 196);
        assert_eq!(vars["D"], 64);
        assert_eq!(vars["S"], 191);

        for ir in InfraredData::from_rawir(
            "+9024 -4512 +564 -1692 +564 -1692 +564 -564 +564 -1692 +564 -1692 +564 -1692 +564 -564 +564 -564 +564 -564 +564 -564 +564 -1692 +564 -564 +564 -564 +564 -564 +564 -1692 +564 -1692 +564 -1692 +564 -1692 +564 -1692 +564 -1692 +564 -1692 +564 -1692 +564 -564 +564 -1692 +564 -564 +564 -564 +564 -564 +564 -564 +564 -564 +564 -564 +564 -1692 +564 -564 +564 -39756").unwrap() {

                matcher.input(ir, &dfa, |ev, vars| res.push((ev, vars)));
            }

        assert_eq!(res.len(), 4);

        let (event, vars) = &res[3];

        assert_eq!(*event, Event::Down);
        // not quite
        assert_eq!(vars["F"], 191);
        assert_eq!(vars["D"], 59);
        assert_eq!(vars["S"], 196);
    }

    #[test]
    #[ignore]
    fn rc5() {
        // RC5
        let irp = Irp::parse("{36k,msb,889}<1,-1|-1,1>((1,~F:1:6,T:1,D:5,F:6,^114m)*,T=1-T)[D:0..31,F:0..127,T@:0..1=0]").unwrap();

        let nfa = irp.build_nfa().unwrap();
        let dfa = nfa.build_dfa();

        let mut res: Vec<(Event, HashMap<String, i64>)> = Vec::new();

        let mut matcher = DFADecoder::new(100, 3, 20000);

        for ir in InfraredData::from_rawir(
            "+889 -889 +1778 -1778 +889 -889 +889 -889 +889 -889 +1778 -889 +889 -889 +889 -889 +889 -889 +889 -889 +889 -1778 +889 -89997").unwrap() {

            matcher.input(ir, &dfa, |ev, vars| res.push((ev, vars)));
        }

        let (event, vars) = &res[0];

        assert_eq!(*event, Event::Repeat);
        assert_eq!(vars["F"], 1);
        assert_eq!(vars["D"], 30);
        assert_eq!(vars["T"], 0);
    }
}
