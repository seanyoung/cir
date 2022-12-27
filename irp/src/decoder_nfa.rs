use super::{
    build_nfa::{Action, Edge, NFA},
    Expression, InfraredData, Vartable,
};
use crate::{Event, Message};
use log::trace;
use std::{
    collections::{HashMap, VecDeque},
    fmt,
    fmt::Write,
};

/// NFA Decoder state
#[derive(Debug)]
pub struct Decoder<'a> {
    pos: Vec<(usize, Vartable<'a>)>,
    abs_tolerance: u32,
    rel_tolerance: u32,
    max_gap: u32,
    nfa: &'a NFA,
    decoded: VecDeque<(Event, HashMap<String, i64>)>,
}

impl NFA {
    /// Create a decoder with parameters. abs_tolerance is microseconds, rel_tolerance is in percentage,
    /// and trailing gap is the minimum gap in microseconds which must follow.
    pub fn decoder(&self, abs_tolerance: u32, rel_tolerance: u32, max_gap: u32) -> Decoder {
        Decoder {
            pos: Vec::new(),
            abs_tolerance,
            rel_tolerance,
            max_gap,
            nfa: self,
            decoded: VecDeque::new(),
        }
    }
}

impl InfraredData {
    /// Create from a slice of alternating flash and gap
    pub fn from_u32_slice(data: &[u32]) -> Vec<InfraredData> {
        data.iter()
            .enumerate()
            .map(|(index, data)| {
                if index % 2 == 0 {
                    InfraredData::Flash(*data)
                } else {
                    InfraredData::Gap(*data)
                }
            })
            .collect()
    }

    /// Create from a rawir string
    pub fn from_rawir(data: &str) -> Result<Vec<InfraredData>, String> {
        Ok(Message::parse(data)?
            .raw
            .iter()
            .enumerate()
            .map(|(index, data)| {
                if index % 2 == 0 {
                    InfraredData::Flash(*data)
                } else {
                    InfraredData::Gap(*data)
                }
            })
            .collect())
    }

    #[must_use]
    fn consume(&self, v: u32) -> Self {
        match self {
            InfraredData::Flash(dur) => InfraredData::Flash(*dur - v),
            InfraredData::Gap(dur) => InfraredData::Gap(*dur - v),
            _ => unreachable!(),
        }
    }
}

impl fmt::Display for InfraredData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            InfraredData::Flash(dur) => write!(f, "+{}", dur),
            InfraredData::Gap(dur) => write!(f, "-{}", dur),
            InfraredData::Reset => write!(f, "!"),
        }
    }
}

impl<'a> fmt::Display for Vartable<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut s = String::new();
        for (name, (val, _, expr)) in &self.vars {
            if let Some(expr) = expr {
                write!(s, " {} = {}", name, expr).unwrap();
            } else {
                write!(s, " {} = {}", name, val).unwrap();
            }
        }

        write!(f, "{}", s)
    }
}

impl<'a> Decoder<'a> {
    /// Reset decoder state
    pub fn reset(&mut self) {
        self.pos.truncate(0);
    }

    fn tolerance_eq(&self, expected: u32, received: u32) -> bool {
        let diff = if expected > received {
            expected - received
        } else {
            received - expected
        };

        if diff <= self.abs_tolerance {
            true
        } else {
            ((diff * 100) / expected) <= self.rel_tolerance
        }
    }

    /// Feed intrared data to the decoder
    pub fn input(&mut self, ir: InfraredData) {
        if ir == InfraredData::Reset {
            trace!("decoder reset");
            self.reset();
            return;
        }

        if self.pos.is_empty() {
            let (success, mut vartab) = self.run_actions(0, &Vartable::new());

            vartab.set("down".into(), 0, 8);

            assert!(success);

            self.pos.push((0, vartab));
        }

        let mut work = Vec::new();

        for (pos, vartab) in &self.pos {
            work.push((Some(ir), *pos, vartab.clone()));
        }

        let mut new_pos = Vec::new();

        while let Some((ir, pos, vartab)) = work.pop() {
            let edges = &self.nfa.verts[pos].edges;

            trace!("pos:{} ir:{:?} vars:{}", pos, ir, vartab);

            for edge in edges {
                //trace!(&format!("edge:{:?}", edge));

                match edge {
                    Edge::Flash(expected, dest) => {
                        if let Some(ir @ InfraredData::Flash(received)) = ir {
                            if self.tolerance_eq(*expected as u32, received) {
                                trace!(
                                    "matched flash {} (expected {}) => {}",
                                    received,
                                    *expected,
                                    dest
                                );

                                let (success, vartab) = self.run_actions(*dest, &vartab);
                                if success {
                                    work.push((None, *dest, vartab));
                                }
                            } else if received > *expected as u32 {
                                trace!(
                                    "matched flash {} (expected {}) (incomplete consume) => {}",
                                    received,
                                    *expected,
                                    dest
                                );

                                let (success, vartab) = self.run_actions(*dest, &vartab);
                                if success {
                                    work.push((Some(ir.consume(*expected as u32)), *dest, vartab));
                                }
                            }
                        } else if ir.is_none() && new_pos.iter().all(|(n, _)| *n != pos) {
                            new_pos.push((pos, vartab.clone()));
                        }
                    }
                    Edge::FlashVar(var, unit, dest) => {
                        let (res, _) = Expression::Identifier(var.to_owned())
                            .eval(&vartab)
                            .unwrap();
                        let expected = res * unit;

                        if let Some(ir @ InfraredData::Flash(received)) = ir {
                            if self.tolerance_eq(expected as u32, received) {
                                trace!(
                                    "matched flash {} (expected {}) => {}",
                                    received,
                                    expected,
                                    dest
                                );

                                let (success, vartab) = self.run_actions(*dest, &vartab);
                                if success {
                                    work.push((None, *dest, vartab));
                                }
                            } else if received > expected as u32 {
                                trace!(
                                    "matched flash {} (expected {}) (incomplete consume) => {}",
                                    received,
                                    expected,
                                    dest
                                );

                                let (success, vartab) = self.run_actions(*dest, &vartab);
                                if success {
                                    work.push((Some(ir.consume(expected as u32)), *dest, vartab));
                                }
                            }
                        } else if ir.is_none() && new_pos.iter().all(|(n, _)| *n != pos) {
                            new_pos.push((pos, vartab.clone()));
                        }
                    }
                    Edge::Gap(expected, dest) => {
                        if let Some(ir @ InfraredData::Gap(received)) = ir {
                            if *expected >= self.max_gap as i64 {
                                if received >= self.max_gap {
                                    trace!(
                                        "large gap matched gap {} (expected {}) => {}",
                                        received,
                                        *expected,
                                        dest
                                    );

                                    let (success, vartab) = self.run_actions(*dest, &vartab);
                                    if success {
                                        work.push((None, *dest, vartab));
                                    }
                                }
                            } else if self.tolerance_eq(*expected as u32, received) {
                                trace!(
                                    "matched gap {} (expected {}) => {}",
                                    received,
                                    *expected,
                                    dest
                                );

                                let (success, vartab) = self.run_actions(*dest, &vartab);
                                if success {
                                    work.push((None, *dest, vartab));
                                }
                            } else if received > *expected as u32 {
                                trace!(
                                    "matched gap {} (expected {}) (incomplete consume) => {}",
                                    received,
                                    *expected,
                                    dest
                                );

                                let (success, vartab) = self.run_actions(*dest, &vartab);
                                if success {
                                    work.push((Some(ir.consume(*expected as u32)), *dest, vartab));
                                }
                            }
                        } else if ir.is_none() && new_pos.iter().all(|(n, _)| *n != pos) {
                            new_pos.push((pos, vartab.clone()));
                        }
                    }
                    Edge::GapVar(var, unit, dest) => {
                        let (res, _) = Expression::Identifier(var.to_owned())
                            .eval(&vartab)
                            .unwrap();
                        let expected = res * unit;

                        if let Some(ir @ InfraredData::Gap(received)) = ir {
                            if expected >= self.max_gap as i64 {
                                if received >= self.max_gap {
                                    trace!(
                                        "large gap matched gap {} (expected {}) => {}",
                                        received,
                                        expected,
                                        dest
                                    );

                                    let (success, vartab) = self.run_actions(*dest, &vartab);
                                    if success {
                                        work.push((None, *dest, vartab));
                                    }
                                }
                            } else if self.tolerance_eq(expected as u32, received) {
                                trace!(
                                    "matched gap {} (expected {}) => {}",
                                    received,
                                    expected,
                                    dest
                                );

                                let (success, vartab) = self.run_actions(*dest, &vartab);
                                if success {
                                    work.push((None, *dest, vartab));
                                }
                            } else if received > expected as u32 {
                                trace!(
                                    "matched gap {} (expected {}) (incomplete consume) => {}",
                                    received,
                                    expected,
                                    dest
                                );

                                let (success, vartab) = self.run_actions(*dest, &vartab);
                                if success {
                                    work.push((Some(ir.consume(expected as u32)), *dest, vartab));
                                }
                            }
                        } else if ir.is_none() && new_pos.iter().all(|(n, _)| *n != pos) {
                            new_pos.push((pos, vartab.clone()));
                        }
                    }
                    Edge::Branch(dest) => {
                        let (success, vartab) = self.run_actions(*dest, &vartab);

                        if success {
                            work.push((ir, *dest, vartab));
                        }
                    }
                    Edge::BranchCond { expr, yes, no } => {
                        let (cond, _) = expr.eval(&vartab).unwrap();

                        let dest = if cond != 0 { *yes } else { *no };

                        let (success, vartab) = self.run_actions(dest, &vartab);

                        if success {
                            trace!(
                                "conditional branch {}: {}: destination {}",
                                expr,
                                cond != 0,
                                dest
                            );

                            work.push((ir, dest, vartab));
                        }
                    }
                    Edge::MayBranchCond { expr, dest } => {
                        let (cond, _) = expr.eval(&vartab).unwrap();

                        if cond != 0 {
                            let (success, vartab) = self.run_actions(*dest, &vartab);

                            if success {
                                let dest = *dest;

                                trace!(
                                    "conditional branch {}: {}: destination {}",
                                    expr,
                                    cond != 0,
                                    dest
                                );

                                work.push((ir, dest, vartab));
                            }
                        }
                    }
                }
            }
        }

        self.pos = new_pos;
    }

    fn run_actions<'v>(&mut self, pos: usize, vartab: &Vartable<'v>) -> (bool, Vartable<'v>) {
        let mut vartable = vartab.clone();

        for a in &self.nfa.verts[pos].actions {
            match a {
                Action::Set { var, expr } => {
                    let (val, len) = expr.eval(&vartable).unwrap();
                    trace!("set {} = {} = {}", var, expr, val);
                    vartable.vars.insert(var.to_string(), (val, len, None));
                }
                Action::AssertEq { left, right } => {
                    if let (Ok((left_val, _)), Ok((right_val, _))) =
                        (left.eval(&vartable), right.eval(&vartable))
                    {
                        if left_val != right_val {
                            trace!(
                                "assert FAIL {} != {} ({} != {})",
                                left,
                                right,
                                left_val,
                                right_val
                            );
                            return (false, vartable);
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
                        return (false, vartable);
                    }
                }
                Action::Done(event, include) => {
                    let mut res: HashMap<String, i64> = HashMap::new();

                    for (name, (val, _, _)) in &vartable.vars {
                        if include.contains(name) || name == "$repeat" {
                            trace!("done {}", event);

                            res.insert(name.to_owned(), *val);
                        }
                    }

                    self.decoded.push_back((*event, res));
                }
            }
        }

        (true, vartable)
    }

    /// Generate a GraphViz dot file and write to the given path
    pub fn dotgraphviz(&self, path: &str) {
        crate::graphviz::graphviz(self.nfa, &self.pos, path);
    }

    /// Get the decoded result
    pub fn get(&mut self) -> Option<(Event, HashMap<String, i64>)> {
        self.decoded.pop_front()
    }
}

#[cfg(test)]
mod test {
    use super::{Decoder, InfraredData};
    use crate::{Event, Irp, Message};
    use num::Integer;
    use std::collections::HashMap;

    fn munge<'a>(matcher: &'a mut Decoder, s: &str) -> (Event, HashMap<String, i64>) {
        let mut res = None;

        for ir in Message::parse(s)
            .unwrap()
            .raw
            .iter()
            .enumerate()
            .map(|(no, len)| {
                if no.is_odd() {
                    InfraredData::Gap(*len)
                } else {
                    InfraredData::Flash(*len)
                }
            })
        {
            matcher.input(ir);

            if let Some(r) = matcher.get() {
                if res.is_some() {
                    panic!("double result: {:?} and {:?}", res, r);
                }

                res = Some(r.clone());
            }
        }

        res.unwrap()
    }

    #[test]
    fn sony8() {
        // sony 8
        let irp = Irp::parse("{40k,600}<1,-1|2,-1>(4,-1,F:8,^45m)[F:0..255]").unwrap();

        let nfa = irp.compile().unwrap();

        let mut matcher = nfa.decoder(100, 3, 20000);

        let (event, res) = munge(&mut matcher,
            "+2400 -600 +600 -600 +600 -600 +1200 -600 +600 -600 +600 -600 +600 -600 +1200 -600 +1200 -31200");

        assert_eq!(event, Event::Down);
        assert_eq!(res["F"], 196);
    }

    #[test]
    fn nec() {
        let irp = Irp::parse("{38.4k,564}<1,-1|1,-3>(16,-8,D:8,S:8,F:8,~F:8,1,^108m,(16,-4,1,^108m)*) [D:0..255,S:0..255=255-D,F:0..255]").unwrap();

        let nfa = irp.compile().unwrap();

        let mut matcher = nfa.decoder(100, 3, 20000);

        let (event, res) = munge(&mut matcher,
            "+9024 -4512 +564 -564 +564 -564 +564 -564 +564 -564 +564 -564 +564 -564 +564 -1692 +564 -564 +564 -1692 +564 -1692 +564 -1692 +564 -1692 +564 -1692 +564 -1692 +564 -564 +564 -1692 +564 -564 +564 -564 +564 -1692 +564 -564 +564 -564 +564 -564 +564 -1692 +564 -1692 +564 -1692 +564 -1692 +564 -564 +564 -1692 +564 -1692 +564 -1692 +564 -564 +564 -564 +564 -39756");

        assert_eq!(event, Event::Down);
        assert_eq!(res["F"], 196);
        assert_eq!(res["D"], 64);
        assert_eq!(res["S"], 191);

        println!("matcher:{:?}", matcher);

        let (event, res) = munge(&mut matcher, "+9024 -2256 +564 -96156");

        assert_eq!(event, Event::Repeat);
        assert!(res.is_empty());

        let (event, res) = munge(&mut matcher, "+9024 -2256 +564 -96156");

        assert_eq!(event, Event::Repeat);
        assert!(res.is_empty());

        let (event, res) = munge(&mut matcher,
            "9024 -4512 +564 -1692 +564 -1692 +564 -564 +564 -1692 +564 -1692 +564 -1692 +564 -564 +564 -564 +564 -564 +564 -564 +564 -1692 +564 -564 +564 -564 +564 -564 +564 -1692 +564 -1692 +564 -1692 +564 -1692 +564 -1692 +564 -1692 +564 -1692 +564 -1692 +564 -564 +564 -1692 +564 -564 +564 -564 +564 -564 +564 -564 +564 -564 +564 -564 +564 -1692 +564 -564 +564 -39756");

        assert_eq!(event, Event::Down);
        // not quite
        assert_eq!(res["F"], 191);
        assert_eq!(res["D"], 59);
        assert_eq!(res["S"], 196);
    }

    #[test]
    fn rc5() {
        // RC5
        let irp = Irp::parse("{36k,msb,889}<1,-1|-1,1>((1,~F:1:6,T:1,D:5,F:6,^114m)*,T=1-T)[D:0..31,F:0..127,T@:0..1=0]").unwrap();

        let nfa = irp.compile().unwrap();

        let mut matcher = nfa.decoder(100, 3, 20000);

        let  (event, res) = munge(&mut matcher,
            "+889 -889 +1778 -1778 +889 -889 +889 -889 +889 -889 +1778 -889 +889 -889 +889 -889 +889 -889 +889 -889 +889 -1778 +889 -89997");

        assert_eq!(event, Event::Down);
        assert_eq!(res["F"], 1);
        assert_eq!(res["D"], 30);
        assert_eq!(res["T"], 0);
    }
}
