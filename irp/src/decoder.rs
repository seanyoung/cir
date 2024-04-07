use super::{
    build_dfa::DFA,
    build_nfa::{Action, Length, NFA},
    InfraredData, Options, Vartable,
};
use crate::{build_nfa::Vertex, Event, Message};
use log::trace;
use std::{collections::HashMap, fmt, fmt::Write};

/// NFA Decoder state
#[derive(Debug)]
pub struct Decoder<'a> {
    pos: Vec<(usize, Vartable<'a>)>,
    options: Options<'a>,
    dfa: bool,
}

impl<'a> Decoder<'a> {
    /// Create a decoder with parameters.
    pub fn new(options: Options<'a>) -> Decoder<'a> {
        Decoder {
            options,
            pos: Vec::new(),
            dfa: false,
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
}

impl fmt::Display for InfraredData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            InfraredData::Flash(dur) => write!(f, "+{dur}"),
            InfraredData::Gap(dur) => write!(f, "-{dur}"),
            InfraredData::Reset => write!(f, "!"),
        }
    }
}

impl<'a> fmt::Display for Vartable<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut s = String::new();
        for (name, (val, expr)) in &self.vars {
            if let Some(expr) = expr {
                write!(s, " {name} = {expr}").unwrap();
            } else {
                write!(s, " {name} = {val}").unwrap();
            }
        }

        write!(f, "{s}")
    }
}

enum ActionResult<'v> {
    Fail,
    Retry(Vartable<'v>),
    Match(Option<InfraredData>, Vartable<'v>),
}

impl<'a> Decoder<'a> {
    /// Reset decoder state
    pub fn reset(&mut self) {
        self.pos.truncate(0);
    }

    pub fn add_pos(&mut self, pos: usize, vartab: Vartable<'a>) {
        let entry = (pos, vartab);
        if self.dfa {
            self.pos = vec![entry];
        } else if !self.pos.contains(&entry) {
            self.pos.push(entry);
        }
    }

    fn tolerance_eq(&self, expected: u32, received: u32) -> bool {
        let diff = expected.abs_diff(received);

        if diff <= self.options.aeps {
            true
        } else {
            // upcast to u64 since diff * 100 may overflow
            (diff as u64 * 100) <= self.options.eps as u64 * expected as u64
        }
    }

    pub(crate) fn consume_flash(
        &self,
        ir: &mut Option<InfraredData>,
        expected: i64,
        complete: bool,
    ) -> bool {
        match ir {
            Some(InfraredData::Flash(received)) => {
                if self.tolerance_eq(expected as u32, *received) {
                    trace!("matched flash {} (expected {})", received, expected);
                    *ir = None;
                    true
                } else if !complete && *received as i64 > expected {
                    trace!(
                        "matched flash {} (expected {}) (incomplete consume)",
                        received,
                        expected,
                    );
                    *ir = Some(InfraredData::Flash(*received - expected as u32));
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    pub(crate) fn consume_flash_range(
        &self,
        ir: &mut Option<InfraredData>,
        min: i64,
        max: i64,
        complete: bool,
    ) -> bool {
        match ir {
            Some(InfraredData::Flash(received)) => {
                let received = *received as i64;
                if received >= min && received <= max {
                    trace!("matched flash {} (range {}..{})", received, min, max);
                    *ir = None;
                    true
                } else if !complete && received > min {
                    trace!(
                        "matched flash {} (range {}..{}) (incomplete consume)",
                        received,
                        min,
                        max
                    );
                    *ir = Some(InfraredData::Flash((received - min) as u32));
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    pub(crate) fn consume_gap(
        &self,
        ir: &mut Option<InfraredData>,
        expected: i64,
        complete: bool,
    ) -> bool {
        match ir {
            Some(InfraredData::Gap(received)) => {
                if self.options.max_gap > 0
                    && expected > self.options.max_gap as i64
                    && *received >= self.options.max_gap
                {
                    trace!("large gap matched gap {} (expected {})", received, expected,);
                    *ir = None;
                    true
                } else if self.tolerance_eq(expected as u32, *received) {
                    trace!("matched gap {} (expected {})", received, expected);
                    *ir = None;
                    true
                } else if !complete && *received as i64 > expected {
                    trace!(
                        "matched gap {} (expected {}) (incomplete consume)",
                        received,
                        expected,
                    );
                    *ir = Some(InfraredData::Gap(*received - expected as u32));
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    pub(crate) fn consume_gap_range(
        &self,
        ir: &mut Option<InfraredData>,
        min: i64,
        max: i64,
        complete: bool,
    ) -> bool {
        match ir {
            Some(InfraredData::Gap(received)) => {
                let received = *received as i64;

                if max > self.options.max_gap as i64 && received >= self.options.max_gap as i64 {
                    trace!(
                        "large gap matched gap {} (range {}..{})",
                        received,
                        min,
                        max
                    );
                    *ir = None;
                    true
                } else if received >= min && received <= max {
                    trace!("matched gap {} (range {}..{})", received, min, max);
                    *ir = None;
                    true
                } else if !complete && received > min {
                    trace!(
                        "matched gap {} (range {}..{}) (incomplete consume)",
                        received,
                        min,
                        max,
                    );
                    *ir = Some(InfraredData::Gap((received - min) as u32));
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    pub fn nfa_input<F>(&mut self, ir: InfraredData, nfa: &NFA, callback: F)
    where
        F: FnMut(Event, HashMap<String, i64>),
    {
        self.dfa = false;
        self.input(ir, &nfa.verts, callback)
    }

    pub fn dfa_input<F>(&mut self, ir: InfraredData, dfa: &DFA, callback: F)
    where
        F: FnMut(Event, HashMap<String, i64>),
    {
        self.dfa = true;
        self.input(ir, &dfa.verts, callback)
    }

    /// Feed infrared data to the decoder
    fn input<F>(&mut self, ir: InfraredData, verts: &[Vertex], mut callback: F)
    where
        F: FnMut(Event, HashMap<String, i64>),
    {
        if ir == InfraredData::Reset {
            trace!("decoder reset");
            self.reset();
            return;
        }

        let ir = if self.pos.is_empty() {
            let mut vartable = Vartable::new();
            vartable.set("$down".into(), 0);

            match self.run_actions(&verts[0].entry, &vartable, Some(ir), &mut callback) {
                ActionResult::Match(ir, vartab) => {
                    self.add_pos(0, vartab);
                    ir
                }
                ActionResult::Retry(vartab) => {
                    self.add_pos(0, vartab);
                    Some(ir)
                }
                ActionResult::Fail => {
                    return;
                }
            }
        } else {
            Some(ir)
        };

        let mut work = Vec::new();

        for (pos, vartab) in &self.pos {
            work.push((ir, *pos, vartab.clone()));
        }

        self.pos.truncate(0);

        while let Some((ir, pos, vartab)) = work.pop() {
            let edges = &verts[pos].edges;

            trace!("pos:{} ir:{:?} vars:{}", pos, ir, vartab);

            for (edge_no, edge) in edges.iter().enumerate() {
                //trace!(&format!("edge:{:?}", edge));

                match self.run_actions(&edge.actions, &vartab, ir, &mut callback) {
                    ActionResult::Match(ir, vartab) => {
                        match self.run_actions(&verts[edge.dest].entry, &vartab, ir, &mut callback)
                        {
                            ActionResult::Match(ir, vartab) => {
                                trace!("pos {pos}: edge: {edge_no} match");
                                work.push((ir, edge.dest, vartab));
                                if self.dfa {
                                    break;
                                }
                            }
                            ActionResult::Retry(..) => {
                                panic!("no flash/gap on entry actions allowed");
                            }
                            ActionResult::Fail => (),
                        }
                    }
                    ActionResult::Retry(vartab) => {
                        self.add_pos(pos, vartab);
                    }
                    ActionResult::Fail => {
                        trace!("pos {pos}: edge: {edge_no} no match");
                    }
                }
            }
        }
    }

    fn run_actions<'v, F>(
        &self,
        actions: &[Action],
        vartab: &Vartable<'v>,
        mut ir: Option<InfraredData>,
        callback: &mut F,
    ) -> ActionResult<'v>
    where
        F: FnMut(Event, HashMap<String, i64>),
    {
        let mut vartable = vartab.clone();

        for a in actions {
            match a {
                Action::Flash {
                    length: Length::Expression(expected),
                    complete,
                } => {
                    let expected = expected.eval(vartab).unwrap();

                    if ir.is_none() {
                        return ActionResult::Retry(vartable);
                    } else if self.consume_flash(&mut ir, expected, *complete) {
                        continue;
                    }

                    return ActionResult::Fail;
                }
                Action::Flash {
                    length: Length::Range(min, max),
                    complete,
                } => {
                    if ir.is_none() {
                        return ActionResult::Retry(vartable);
                    } else if self.consume_flash_range(
                        &mut ir,
                        (*min).into(),
                        max.unwrap_or(u32::MAX).into(),
                        *complete,
                    ) {
                        continue;
                    }

                    return ActionResult::Fail;
                }
                Action::Gap {
                    length: Length::Expression(expected),
                    complete,
                } => {
                    let expected = expected.eval(vartab).unwrap();

                    if ir.is_none() {
                        return ActionResult::Retry(vartable);
                    } else if self.consume_gap(&mut ir, expected, *complete) {
                        continue;
                    }

                    return ActionResult::Fail;
                }
                Action::Gap {
                    length: Length::Range(min, max),
                    complete,
                } => {
                    if ir.is_none() {
                        return ActionResult::Retry(vartable);
                    } else if self.consume_gap_range(
                        &mut ir,
                        (*min).into(),
                        max.unwrap_or(u32::MAX).into(),
                        *complete,
                    ) {
                        continue;
                    }

                    return ActionResult::Fail;
                }
                Action::Set { var, expr } => {
                    let val = expr.eval(&vartable).unwrap();
                    trace!("set {} = {} = {}", var, expr, val);
                    vartable.vars.insert(var.to_string(), (val, None));
                }
                Action::AssertEq { left, right } => {
                    if let (Ok(left_val), Ok(right_val)) =
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
                            return ActionResult::Fail;
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
                        return ActionResult::Fail;
                    }
                }
                Action::Done(event, include) => {
                    let mut res: HashMap<String, i64> = HashMap::new();

                    for (name, (val, _)) in &vartable.vars {
                        if include.contains(name) {
                            trace!("done {}", event);

                            res.insert(name.to_owned(), *val);
                        }
                    }

                    (callback)(*event, res);
                }
            }
        }

        ActionResult::Match(ir, vartable)
    }

    /// Generate a GraphViz dot file and write to the given path
    pub fn nfa_dotgraphviz(&self, path: &str, nfa: &NFA) {
        crate::graphviz::graphviz(&nfa.verts, "NFA", &self.pos, path);
    }

    pub fn dfa_dotgraphviz(&self, path: &str, dfa: &DFA) {
        crate::graphviz::graphviz(&dfa.verts, "DFA", &self.pos, path);
    }
}

#[cfg(test)]
mod test {
    use super::{Decoder, InfraredData};
    use crate::{Event, Irp, Options};
    use std::collections::HashMap;

    #[test]
    fn sony8() {
        // sony 8
        let irp = Irp::parse("{40k,600}<1,-1|2,-1>(4,-1,F:8,^45m)[F:0..255]").unwrap();

        let nfa = irp.build_nfa().unwrap();

        let mut res: Vec<(Event, HashMap<String, i64>)> = Vec::new();

        let mut matcher = Decoder::new(Options {
            aeps: 100,
            eps: 3,
            max_gap: 20000,
            ..Default::default()
        });

        for ir in InfraredData::from_rawir(
            "+2400 -600 +600 -600 +600 -600 +1200 -600 +600 -600 +600 -600 +600 -600 +1200 -600 +1200 -31200").unwrap() {
            matcher.nfa_input(ir, &nfa, |ev, vars| res.push((ev, vars)));
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

        let mut res: Vec<(Event, HashMap<String, i64>)> = Vec::new();

        let mut matcher = Decoder::new(Options {
            aeps: 100,
            eps: 3,
            max_gap: 20000,
            ..Default::default()
        });

        for ir in InfraredData::from_rawir(
            "+9024 -4512 +564 -564 +564 -564 +564 -564 +564 -564 +564 -564 +564 -564 +564 -1692 +564 -564 +564 -1692 +564 -1692 +564 -1692 +564 -1692 +564 -1692 +564 -1692 +564 -564 +564 -1692 +564 -564 +564 -564 +564 -1692 +564 -564 +564 -564 +564 -564 +564 -1692 +564 -1692 +564 -1692 +564 -1692 +564 -564 +564 -1692 +564 -1692 +564 -1692 +564 -564 +564 -564 +564 -39756").unwrap() {

            matcher.nfa_input(ir, &nfa, |ev, vars| res.push((ev, vars)));
        }

        assert_eq!(res.len(), 1);

        let (event, vars) = &res[0];

        assert_eq!(*event, Event::Down);
        assert_eq!(vars["F"], 196);
        assert_eq!(vars["D"], 64);
        assert_eq!(vars["S"], 191);

        for ir in InfraredData::from_rawir("+9024 -2256 +564 -96156").unwrap() {
            matcher.nfa_input(ir, &nfa, |ev, vars| res.push((ev, vars)));
        }

        assert_eq!(res.len(), 2);

        let (event, vars) = &res[1];

        assert_eq!(*event, Event::Repeat);
        assert_eq!(vars["F"], 196);
        assert_eq!(vars["D"], 64);
        assert_eq!(vars["S"], 191);

        for ir in InfraredData::from_rawir("+9024 -2256 +564 -96156").unwrap() {
            matcher.nfa_input(ir, &nfa, |ev, vars| res.push((ev, vars)));
        }

        assert_eq!(res.len(), 3);

        let (event, vars) = &res[2];

        assert_eq!(*event, Event::Repeat);
        assert_eq!(vars["F"], 196);
        assert_eq!(vars["D"], 64);
        assert_eq!(vars["S"], 191);

        for ir in InfraredData::from_rawir(
            "+9024 -4512 +564 -1692 +564 -1692 +564 -564 +564 -1692 +564 -1692 +564 -1692 +564 -564 +564 -564 +564 -564 +564 -564 +564 -1692 +564 -564 +564 -564 +564 -564 +564 -1692 +564 -1692 +564 -1692 +564 -1692 +564 -1692 +564 -1692 +564 -1692 +564 -1692 +564 -564 +564 -1692 +564 -564 +564 -564 +564 -564 +564 -564 +564 -564 +564 -564 +564 -1692 +564 -564 +564 -39756").unwrap() {

                matcher.nfa_input(ir, &nfa, |ev, vars| res.push((ev, vars)));
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
    fn rc5() {
        // RC5
        let irp = Irp::parse("{36k,msb,889}<1,-1|-1,1>((1,~F:1:6,T:1,D:5,F:6,^114m)*,T=1-T)[D:0..31,F:0..127,T@:0..1=0]").unwrap();

        let nfa = irp.build_nfa().unwrap();

        let mut res: Vec<(Event, HashMap<String, i64>)> = Vec::new();

        let mut matcher = Decoder::new(Options {
            aeps: 100,
            eps: 3,
            max_gap: 20000,
            ..Default::default()
        });

        for ir in InfraredData::from_rawir(
            "+889 -889 +1778 -1778 +889 -889 +889 -889 +889 -889 +1778 -889 +889 -889 +889 -889 +889 -889 +889 -889 +889 -1778 +889 -89997").unwrap() {

            matcher.nfa_input(ir, &nfa, |ev, vars| res.push((ev, vars)));
        }

        let (event, vars) = &res[0];

        assert_eq!(*event, Event::Repeat);
        assert_eq!(vars["F"], 1);
        assert_eq!(vars["D"], 30);
        assert_eq!(vars["T"], 0);
    }
}
