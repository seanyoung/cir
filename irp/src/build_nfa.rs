use super::{expression::clone_filter, Expression, Irp, ParameterSpec, RepeatMarker, Vartable};
use log::trace;
use std::{
    collections::HashMap,
    ops::{Add, BitAnd, BitOr, BitXor, Neg, Not, Rem, Shl, Shr, Sub},
    rc::Rc,
};

/**
 * Here we build the decoder nfa (non-deterministic finite automation)
 *
 * TODO
 * - ExtentConstant may be very short. We should calculate minimum length
 * - (..)2 and other repeat markers are not supported
 * - Implement variants
 */

#[derive(PartialEq, Debug, Clone)]
pub(crate) enum Edge {
    Flash(i64, usize),
    Gap(i64, usize),
    FlashVar(String, i64, usize),
    GapVar(String, i64, usize),
    TrailingGap(usize),
    BranchCond {
        expr: Rc<Expression>,
        yes: usize,
        no: usize,
    },
    MayBranchCond {
        expr: Rc<Expression>,
        dest: usize,
    },
    Branch(usize),
    Done(Vec<String>),
}

#[derive(PartialEq, Debug, Clone)]
pub(crate) enum Action {
    Set {
        var: String,
        expr: Rc<Expression>,
    },
    AssertEq {
        left: Rc<Expression>,
        right: Rc<Expression>,
    },
}

#[derive(PartialEq, Default, Clone, Debug)]
pub(crate) struct Vertex {
    pub actions: Vec<Action>,
    pub edges: Vec<Edge>,
}

/// Non-deterministic finite automation for decoding IR. Using this we can
/// match IR and hopefully, one day, create the dfa (deterministic finite
/// automation).
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug)]
pub struct NFA {
    pub(crate) verts: Vec<Vertex>,
}

impl Irp {
    /// Generate an NFA decoder for this IRP. This may fail if it is impossible
    /// to generate a decoder for this Irp.
    pub fn compile(&self) -> Result<NFA, String> {
        let mut builder = Builder::new(self);

        builder.add_constants();

        builder.expression(&self.stream, &[])?;

        builder.add_done()?;

        Ok(NFA {
            verts: builder.complete(),
        })
    }
}

impl NFA {
    /// Generate a GraphViz dot file and write to the given path
    pub fn dotgraphviz(&self, path: &str) {
        crate::graphviz::graphviz(self, &[], path);
    }
}

pub(crate) fn gen_mask(v: i64) -> i64 {
    (1i64 << v) - 1
}

/// track which bits of which variables may be set, builder head, etc.
#[derive(Clone, Debug)]
pub(crate) struct Builder<'a> {
    cur: BuilderLocation,
    saved: Vec<BuilderLocation>,
    verts: Vec<Vertex>,
    constants: Vartable<'a>,
    pub irp: &'a Irp,
}

#[derive(Clone, Debug, Default)]
struct BuilderLocation {
    head: usize,
    seen_edges: bool,
    vars: HashMap<String, i64>,
}

#[allow(dead_code)]
impl<'a> Builder<'a> {
    pub fn new(irp: &'a Irp) -> Self {
        let verts = vec![Vertex::default()];

        Builder {
            cur: BuilderLocation::default(),
            saved: Vec::new(),
            constants: Vartable::new(),
            verts,
            irp,
        }
    }

    fn set(&mut self, name: &str, fields: i64) {
        assert!(fields != 0);

        if let Some(e) = self.cur.vars.get_mut(name) {
            *e |= fields;
        } else {
            self.cur.vars.insert(name.to_owned(), fields);
        }
    }

    pub fn is_set(&self, name: &str, fields: i64) -> bool {
        if let Some(e) = self.cur.vars.get(name) {
            (e & fields) == fields
        } else {
            false
        }
    }

    fn is_any_set(&self, name: &str) -> bool {
        self.cur.vars.contains_key(name)
    }

    fn add_done(&mut self) -> Result<bool, String> {
        if self.cur.seen_edges
            && self
                .irp
                .parameters
                .iter()
                .all(|param| self.cur.vars.contains_key(&param.name))
        {
            let res = self
                .irp
                .parameters
                .iter()
                .map(|param| param.name.to_owned())
                .collect();

            self.add_edge(Edge::Done(res));
            self.mask_results()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn add_vertex(&mut self) -> usize {
        let node = self.verts.len();

        self.verts.push(Vertex::default());

        node
    }

    fn set_head(&mut self, head: usize) {
        self.cur.head = head;
    }

    fn add_action(&mut self, action: Action) {
        self.verts[self.cur.head].actions.push(action);
    }

    fn add_edge(&mut self, edge: Edge) {
        self.verts[self.cur.head].edges.push(edge);
    }

    fn add_action_at_node(&mut self, node: usize, action: Action) {
        self.verts[node].actions.push(action);
    }

    fn add_edge_at_node(&mut self, node: usize, edge: Edge) {
        self.verts[node].edges.push(edge);
    }

    /// Once done building, return the completed vertices
    fn complete(self) -> Vec<Vertex> {
        self.verts
    }

    fn push_location(&mut self) {
        self.saved.push(self.cur.clone());
    }

    fn pop_location(&mut self) {
        self.cur = self.saved.pop().unwrap();
    }

    /// The list of definitions may contain variables which are constants and
    /// can be evaluated now. Some of those will be modified at a later point;
    /// and will end up as variables with initializers, else if the variables
    /// are truly constant then they end up in the constants map.
    fn add_constants(&mut self) {
        // first add the true constants
        while {
            let mut changes = false;

            for def in &self.irp.definitions {
                if let Expression::Assignment(name, expr) = def {
                    if self.is_any_set(name) {
                        continue;
                    }

                    let mut modified_anywhere = false;

                    // is this variable modified anywhere
                    self.irp.stream.visit(
                        &mut modified_anywhere,
                        &|expr: &Expression, modified: &mut bool| {
                            if let Expression::Assignment(var_name, _) = expr {
                                if var_name == name {
                                    *modified = true;
                                }
                            }
                        },
                    );

                    if modified_anywhere {
                        continue;
                    }

                    if self.expression_available(expr).is_ok() {
                        let (val, len) = expr.eval(&self.constants).unwrap();

                        self.constants.set(name.to_owned(), val, len);

                        changes = true;

                        self.set(name, !0);
                    }
                }
            }

            changes
        } {}

        // any remaining definitions which are available are either modified or
        // depend on a variable which will be modified at some point
        while {
            let mut changes = false;

            for def in &self.irp.definitions {
                if let Expression::Assignment(name, expr) = def {
                    if self.is_any_set(name) {
                        continue;
                    }

                    if self.expression_available(expr).is_ok() {
                        // just set an initial value
                        self.add_action(Action::Set {
                            var: name.to_owned(),
                            expr: self.const_folding(expr),
                        });

                        changes = true;

                        self.set(name, !0);
                    }
                }
            }

            changes
        } {}

        self.add_action(Action::Set {
            var: "$repeat".to_owned(),
            expr: Rc::new(Expression::Number(0)),
        });
    }

    fn expression_list(
        &mut self,
        list: &[Rc<Expression>],
        bit_spec: &[&[Rc<Expression>]],
    ) -> Result<(), String> {
        let mut pos = 0;

        while pos < list.len() {
            let mut bit_count = 0;
            let mut expr_count = 0;

            while let Some(expr) = list.get(pos + expr_count) {
                if let Expression::BitField { length, .. } = expr.as_ref() {
                    let (min_len, max_len, _) = self.bit_field_length(length)?;

                    // if variable length bit field is preceded by a constant length fields, process
                    // those before this one
                    if expr_count != 0 && max_len != min_len {
                        break;
                    }

                    if max_len > 64 {
                        return Err(format!(
                            "bitfield of length {} longer than the 64 maximum",
                            max_len
                        ));
                    }

                    if bit_count + max_len > 64 {
                        break;
                    }

                    bit_count += min_len;
                    expr_count += 1;

                    // variable length bitfields should be processed by one
                    if max_len != min_len {
                        break;
                    }
                } else {
                    break;
                }
            }

            if expr_count == 0 {
                // not a bit field
                self.expression(&list[pos], bit_spec)?;
                pos += 1;
                continue;
            }

            // if it is a single constant bitfield, just expand it - no loops needed
            if expr_count == 1 && bit_count <= 8 {
                if let Expression::BitField {
                    value,
                    skip: None,
                    reverse,
                    ..
                } = list[pos].as_ref()
                {
                    if let Expression::Number(value) = self.const_folding(value).as_ref() {
                        if self.irp.general_spec.lsb ^ reverse {
                            for bit in 0..bit_count {
                                let e = &bit_spec[0][((value >> bit) & 1) as usize];

                                self.expression(e, &bit_spec[1..])?;
                            }
                        } else {
                            for bit in (0..bit_count).rev() {
                                let e = &bit_spec[0][((value >> bit) & 1) as usize];

                                self.expression(e, &bit_spec[1..])?;
                            }
                        }

                        pos += 1;
                        continue;
                    }
                }
            }

            let do_reverse = if expr_count == 1 {
                if let Expression::BitField {
                    value,
                    skip,
                    reverse,
                    length,
                } = list[pos].as_ref()
                {
                    let (min_len, max_len, store_length) = self.bit_field_length(length)?;

                    if min_len != max_len {
                        self.decode_bits(Some(min_len), max_len, *reverse, store_length, bit_spec)?;

                        let skip = if let Some(skip) = skip {
                            let (skip, _) = self.const_folding(skip).eval(&Vartable::new())?;

                            skip
                        } else {
                            0
                        };

                        let bits = Expression::Identifier(String::from("$bits"));

                        #[allow(clippy::comparison_chain)]
                        let bits = if skip > 0 {
                            Expression::ShiftLeft(Rc::new(bits), Rc::new(Expression::Number(skip)))
                        } else {
                            bits
                        };

                        if let Expression::Identifier(name) = value.as_ref() {
                            self.add_action(Action::Set {
                                var: name.to_owned(),
                                expr: self.const_folding(&Rc::new(bits)),
                            });

                            self.set(name, !0);
                        } else {
                            return Err(format!(
                                "expression {} not supported for variable length bitfield",
                                value
                            ));
                        }

                        pos += 1;
                        continue;
                    } else {
                        *reverse
                    }
                } else {
                    unreachable!();
                }
            } else {
                false
            };

            self.decode_bits(None, bit_count, do_reverse, None, bit_spec)?;

            let mut delayed = Vec::new();

            // now do stuff with bitfields
            let mut offset = if self.irp.general_spec.lsb {
                0
            } else {
                bit_count
            };

            for i in 0..expr_count {
                let expr = list[i + pos].as_ref();

                if let Expression::BitField {
                    value,
                    length,
                    skip,
                    reverse,
                } = expr
                {
                    let (length, _) = self.const_folding(length).eval(&Vartable::new())?;

                    if !self.irp.general_spec.lsb {
                        offset -= length;
                    }

                    let skip = if let Some(skip) = skip {
                        let (skip, _) = self.const_folding(skip).eval(&Vartable::new())?;

                        skip
                    } else {
                        0
                    };

                    let mut value = self.const_folding(value);

                    let bits = Rc::new(Expression::Identifier(String::from("$bits")));

                    #[allow(clippy::comparison_chain)]
                    let mut bits = if offset > skip {
                        Rc::new(Expression::ShiftRight(
                            bits,
                            Rc::new(Expression::Number(offset - skip)),
                        ))
                    } else if offset < skip {
                        Rc::new(Expression::ShiftLeft(
                            bits,
                            Rc::new(Expression::Number(skip - offset)),
                        ))
                    } else {
                        bits
                    };

                    if *reverse && !do_reverse {
                        bits = Rc::new(Expression::BitReverse(bits, length, skip));
                    }

                    let mask = gen_mask(length) << skip;

                    // F:4 => ($bits & 15) = (F & 15)
                    // ~F:4 => (~$bits & 15) = (F & 15)
                    // (F-1):4 => ($bits & 15) + 1 = F
                    // ~(F-1):4 => ~($bits & 15) + 1 = F
                    match value.as_ref() {
                        Expression::Complement(comp) => {
                            if matches!(comp.as_ref(), Expression::Identifier(..)) {
                                value = comp.clone();
                                bits = Rc::new(Expression::BitwiseAnd(
                                    Rc::new(Expression::Complement(bits)),
                                    Rc::new(Expression::Number(mask)),
                                ));
                            }
                        }
                        _ => {
                            bits = Rc::new(Expression::BitwiseAnd(
                                bits,
                                Rc::new(Expression::Number(mask)),
                            ));
                        }
                    };

                    match self.expression_available(expr) {
                        Ok(_) => {
                            // We know all the variables in here or its constant
                            self.check_bits_in_var(value, bits, mask)?;
                        }
                        Err(name) => match self.inverse(bits, value.clone(), &name) {
                            Some((bits, actions, _)) => {
                                actions.into_iter().for_each(|act| self.add_action(act));

                                self.use_decode_bits(&name, bits, mask, &mut delayed)?;
                            }
                            None => {
                                return Err(format!("expression {} not supported", value));
                            }
                        },
                    }

                    if self.irp.general_spec.lsb {
                        offset += length;
                    }
                }
            }

            for action in delayed {
                match &action {
                    Action::AssertEq { left, right } => {
                        self.have_definitions(left)?;
                        self.have_definitions(right)?;
                    }
                    Action::Set { expr, .. } => {
                        self.have_definitions(expr)?;
                    }
                }
                self.add_action(action);
            }

            pos += expr_count;
        }

        Ok(())
    }

    /// Fetch the length for a bitfield. If the length is not a constant, it has
    /// to be a parameter with constant min and max.
    fn bit_field_length(
        &self,
        length: &Rc<Expression>,
    ) -> Result<(i64, i64, Option<String>), String> {
        let length = self.const_folding(length);

        match length.as_ref() {
            Expression::Number(v) => Ok((*v, *v, None)),
            Expression::Identifier(name) => {
                if let Some(param) = self.irp.parameters.iter().find(|def| def.name == *name) {
                    let min = param.min.eval(&self.constants)?.0;
                    let max = param.max.eval(&self.constants)?.0;

                    if min > max {
                        Err(format!(
                            "parameter {} has min > max ({} > {})",
                            name, min, max
                        ))
                    } else {
                        Ok((
                            param.min.eval(&self.constants)?.0,
                            param.max.eval(&self.constants)?.0,
                            Some(name.to_owned()),
                        ))
                    }
                } else {
                    Err(format!("bit field length {} is not a parameter", name))
                }
            }
            expr => Err(format!("bit field length {} not known", expr)),
        }
    }

    fn use_decode_bits(
        &mut self,
        name: &str,
        mut bits: Rc<Expression>,
        mask: i64,
        delayed: &mut Vec<Action>,
    ) -> Result<(), String> {
        if self.is_set(name, mask) {
            let left = self.const_folding(&Rc::new(Expression::BitwiseAnd(
                Rc::new(Expression::Identifier(name.to_owned())),
                Rc::new(Expression::Number(mask)),
            )));

            self.add_action(Action::AssertEq {
                left,
                right: self.const_folding(&bits),
            });
        } else if let Some(def) = self.irp.definitions.iter().find_map(|def| {
            if let Expression::Assignment(var, expr) = def {
                if name == var {
                    return Some(expr);
                }
            }
            None
        }) {
            let expr = if self.is_any_set(name) {
                Rc::new(Expression::BitwiseOr(
                    Rc::new(Expression::Identifier(name.to_owned())),
                    bits.clone(),
                ))
            } else {
                bits.clone()
            };

            self.add_action(Action::Set {
                var: name.to_owned(),
                expr: self.const_folding(&expr),
            });

            if !self.is_any_set(name) {
                bits = Rc::new(Expression::Identifier(name.to_owned()));
            }

            self.set(name, mask);

            let def = self.const_folding(def);

            let left = self.const_folding(&Rc::new(Expression::BitwiseAnd(
                def.clone(),
                Rc::new(Expression::Number(mask)),
            )));

            let action = Action::AssertEq {
                left,
                right: self.const_folding(&bits),
            };

            if self.have_definitions(&def).is_ok() {
                self.add_action(action);
            } else {
                delayed.push(action);
            }
        } else {
            let expr = if self.is_any_set(name) {
                Rc::new(Expression::BitwiseOr(
                    Rc::new(Expression::Identifier(name.to_owned())),
                    bits,
                ))
            } else {
                bits
            };

            self.set(name, mask);

            self.add_action(Action::Set {
                var: name.to_owned(),
                expr: self.const_folding(&expr),
            });
        }

        Ok(())
    }

    fn check_bits_in_var(
        &mut self,
        value: Rc<Expression>,
        bits: Rc<Expression>,
        mask: i64,
    ) -> Result<(), String> {
        let left = bits;
        let right = self.const_folding(&Rc::new(Expression::BitwiseAnd(
            value,
            Rc::new(Expression::Number(mask)),
        )));
        self.add_action(Action::AssertEq { left, right });

        Ok(())
    }

    /// Look for a definition of name in the list of definitions. If found,
    /// add it to the actions. This function works recursively, so if a definition
    /// requires a further definition, then that definition will be included too
    fn add_definition(&mut self, name: &str) -> bool {
        if let Some(def) = self.irp.definitions.iter().find_map(|def| {
            if let Expression::Assignment(var, expr) = def {
                if name == var {
                    return Some(expr);
                }
            }
            None
        }) {
            for _ in 0..self.irp.definitions.len() {
                match self.expression_available(def) {
                    Ok(_) => {
                        trace!("found definition {} = {}", name, def);

                        self.add_action(Action::Set {
                            var: name.to_owned(),
                            expr: self.const_folding(def),
                        });

                        self.set(name, !0);

                        return true;
                    }
                    Err(name) => {
                        if !self.add_definition(&name) {
                            return false;
                        }
                    }
                }
            }
        }

        let mut found = false;

        for def in &self.irp.definitions {
            if let Expression::Assignment(var, expr) = def {
                let expr = self.const_folding(expr);

                if let Some((expr, actions, mask)) =
                    self.inverse(Rc::new(Expression::Identifier(var.to_string())), expr, name)
                {
                    if self.expression_available(&expr).is_err() {
                        continue;
                    }

                    if let Some(mask) = mask {
                        if self.is_set(name, mask) {
                            continue;
                        }
                    }

                    let mut expr = self.const_folding(&expr);

                    if self.is_any_set(name) {
                        expr = Rc::new(Expression::BitwiseOr(
                            Rc::new(Expression::Identifier(name.to_owned())),
                            expr,
                        ));
                    }

                    trace!("found definition {} = {}", name, expr);

                    self.add_action(Action::Set {
                        var: name.to_owned(),
                        expr,
                    });

                    self.set(name, mask.unwrap_or(!0));

                    actions.into_iter().for_each(|act| self.add_action(act));

                    found = true;
                }
            }
        }

        found
    }

    /// For given expression, add any required definitions or return an error if
    /// no definitions can be found
    fn have_definitions(&mut self, expr: &Expression) -> Result<(), String> {
        loop {
            match self.expression_available(expr) {
                Ok(_) => {
                    return Ok(());
                }
                Err(name) => {
                    if !self.add_definition(&name) {
                        return Err(format!("variable ‘{}’ not known", name));
                    }
                }
            }
        }
    }

    /// Add the bit specs for bitfields. max specifies the maximum number of
    /// bits, min specifies a minimum if the number of bits is not fixed at max.
    /// The length can be stored in the variable name store_length if set.
    fn decode_bits(
        &mut self,
        min: Option<i64>,
        max: i64,
        reverse: bool,
        store_length: Option<String>,
        bit_spec: &[&[Rc<Expression>]],
    ) -> Result<(), String> {
        self.cur.seen_edges = true;

        let width = match bit_spec[0].len() {
            2 => 1,
            4 => 2,
            8 => 3,
            16 => 4,
            w => {
                return Err(format!("bit spec with {} fields not supported", w));
            }
        };

        if max == 1 && min.is_none() {
            let next = self.add_vertex();

            for (bit, e) in bit_spec[0].iter().enumerate() {
                self.push_location();

                self.expression(e, &bit_spec[1..])?;

                self.add_action(Action::Set {
                    var: String::from("$bits"),
                    expr: Rc::new(Expression::Number(bit as i64)),
                });

                self.add_edge(Edge::Branch(next));

                self.pop_location();
            }

            self.set_head(next);

            return Ok(());
        }

        let before = self.cur.head;

        let entry = self.add_vertex();

        let next = self.add_vertex();

        let done = self.add_vertex();

        self.add_edge(Edge::Branch(entry));

        self.set_head(entry);

        if let Some(length) = &store_length {
            self.set(length, !0);
        }

        let length = store_length.unwrap_or_else(|| String::from("$b"));

        for (bit, e) in bit_spec[0].iter().enumerate() {
            self.push_location();

            self.expression(e, &bit_spec[1..])?;

            self.add_action(Action::Set {
                var: String::from("$v"),
                expr: Rc::new(Expression::Number(bit as i64)),
            });

            self.add_edge(Edge::Branch(next));

            self.pop_location();
        }

        self.add_action_at_node(
            before,
            Action::Set {
                var: length.to_owned(),
                expr: Rc::new(Expression::Number(0)),
            },
        );

        self.add_action_at_node(
            before,
            Action::Set {
                var: String::from("$bits"),
                expr: Rc::new(Expression::Number(0)),
            },
        );

        if !(self.irp.general_spec.lsb ^ reverse) {
            self.add_action_at_node(
                next,
                Action::Set {
                    var: String::from("$bits"),
                    expr: Rc::new(Expression::BitwiseOr(
                        Rc::new(Expression::ShiftLeft(
                            Rc::new(Expression::Identifier(String::from("$bits"))),
                            Rc::new(Expression::Number(width)),
                        )),
                        Rc::new(Expression::Identifier(String::from("$v"))),
                    )),
                },
            );
        } else {
            self.add_action_at_node(
                next,
                Action::Set {
                    var: String::from("$bits"),
                    expr: Rc::new(Expression::BitwiseOr(
                        Rc::new(Expression::Identifier(String::from("$bits"))),
                        Rc::new(Expression::ShiftLeft(
                            Rc::new(Expression::Identifier(String::from("$v"))),
                            Rc::new(Expression::Identifier(length.to_owned())),
                        )),
                    )),
                },
            );
        }

        self.add_action_at_node(
            next,
            Action::Set {
                var: length.to_owned(),
                expr: Rc::new(Expression::Add(
                    Rc::new(Expression::Identifier(length.to_owned())),
                    Rc::new(Expression::Number(width)),
                )),
            },
        );

        self.add_edge_at_node(
            next,
            Edge::BranchCond {
                expr: Rc::new(Expression::Less(
                    Rc::new(Expression::Identifier(length.to_owned())),
                    Rc::new(Expression::Number(max)),
                )),
                yes: entry,
                no: done,
            },
        );

        if let Some(min) = min {
            self.add_edge_at_node(
                next,
                Edge::MayBranchCond {
                    expr: Rc::new(Expression::MoreEqual(
                        Rc::new(Expression::Identifier(length)),
                        Rc::new(Expression::Number(min)),
                    )),
                    dest: done,
                },
            );
        }

        self.set_head(done);

        Ok(())
    }

    fn expression(
        &mut self,
        expr: &Expression,
        bit_spec: &[&[Rc<Expression>]],
    ) -> Result<(), String> {
        match expr {
            Expression::Stream(irstream) => {
                let mut bit_spec = bit_spec.to_vec();

                if !irstream.bit_spec.is_empty() {
                    bit_spec.insert(0, &irstream.bit_spec);
                }

                if irstream.repeat == Some(RepeatMarker::Any)
                    || irstream.repeat == Some(RepeatMarker::OneOrMore)
                {
                    let mut start = self.cur.head;

                    let done_before = if self.add_done()? {
                        let node = self.add_vertex();

                        self.add_edge(Edge::Branch(node));

                        self.set_head(node);

                        start = node;

                        self.add_edge(Edge::Branch(0));

                        true
                    } else {
                        false
                    };

                    self.expression_list(&irstream.stream, &bit_spec)?;

                    self.add_action(Action::Set {
                        var: "$repeat".to_owned(),
                        expr: Rc::new(Expression::Number(1)),
                    });

                    if !done_before {
                        self.add_done()?;
                    }

                    self.add_edge(Edge::Branch(start));
                } else {
                    self.expression_list(&irstream.stream, &bit_spec)?;
                }
            }
            Expression::List(list) => {
                self.expression_list(list, bit_spec)?;
            }
            Expression::FlashConstant(v, u) => {
                self.cur.seen_edges = true;

                let len = u.eval_float(*v, &self.irp.general_spec)?;

                let node = self.add_vertex();

                self.add_edge(Edge::Flash(len, node));

                self.set_head(node);
            }
            Expression::GapConstant(v, u) => {
                self.cur.seen_edges = true;

                let len = u.eval_float(*v, &self.irp.general_spec)?;

                let node = self.add_vertex();

                self.add_edge(Edge::Gap(len, node));

                self.set_head(node);
            }
            Expression::FlashIdentifier(var, unit) => {
                if !self.is_any_set(var) {
                    return Err(format!("variable ‘{}’ is not set", var));
                }

                let unit = unit.eval(1, &self.irp.general_spec)?;

                let node = self.add_vertex();

                self.add_edge(Edge::FlashVar(var.to_owned(), unit, node));

                self.set_head(node);
            }
            Expression::GapIdentifier(var, unit) => {
                if !self.is_any_set(var) {
                    return Err(format!("variable ‘{}’ is not set", var));
                }

                let unit = unit.eval(1, &self.irp.general_spec)?;

                let node = self.add_vertex();

                self.add_edge(Edge::GapVar(var.to_owned(), unit, node));

                self.set_head(node);
            }
            Expression::BitField { length, .. } => {
                let (length, _) = length.eval(&Vartable::new())?;

                self.decode_bits(None, length, false, None, bit_spec)?;
            }
            Expression::ExtentConstant(_, _) => {
                self.cur.seen_edges = true;

                let node = self.add_vertex();

                self.add_edge(Edge::TrailingGap(node));

                self.set_head(node);
            }
            Expression::Assignment(var, expr) => {
                if var == "T" {
                    return Ok(());
                }

                self.have_definitions(expr)?;

                self.add_action(Action::Set {
                    var: var.to_owned(),
                    expr: self.const_folding(expr),
                })
            }
            _ => println!("expr:{:?}", expr),
        }

        Ok(())
    }

    /// Do we have all the vars to evaluate an expression, i.e. can this
    /// expression be evaluated now.
    fn expression_available(&self, expr: &Expression) -> Result<(), String> {
        match expr {
            Expression::FlashConstant(..)
            | Expression::GapConstant(..)
            | Expression::ExtentConstant(..)
            | Expression::Number(..) => Ok(()),
            Expression::FlashIdentifier(name, ..)
            | Expression::GapIdentifier(name, ..)
            | Expression::ExtentIdentifier(name, ..)
            | Expression::Identifier(name) => {
                if name.starts_with('$')
                    || self.cur.vars.contains_key(name)
                    || self.constants.is_defined(name)
                {
                    Ok(())
                } else {
                    Err(name.to_owned())
                }
            }
            Expression::BitField {
                value,
                length,
                skip,
                ..
            } => {
                if let Some(res) = self.bitfield_known(value, length, skip) {
                    res
                } else {
                    if let Some(skip) = &skip {
                        self.expression_available(skip)?;
                    }
                    self.expression_available(value)?;
                    self.expression_available(length)
                }
            }
            Expression::InfiniteBitField { value, skip } => {
                self.expression_available(value)?;
                self.expression_available(skip)
            }
            Expression::Assignment(_, expr)
            | Expression::Complement(expr)
            | Expression::Not(expr)
            | Expression::Negative(expr)
            | Expression::BitCount(expr)
            | Expression::Log2(expr)
            | Expression::BitReverse(expr, ..) => self.expression_available(expr),
            Expression::Add(left, right)
            | Expression::Subtract(left, right)
            | Expression::Multiply(left, right)
            | Expression::Power(left, right)
            | Expression::Divide(left, right)
            | Expression::Modulo(left, right)
            | Expression::ShiftLeft(left, right)
            | Expression::ShiftRight(left, right)
            | Expression::LessEqual(left, right)
            | Expression::Less(left, right)
            | Expression::More(left, right)
            | Expression::MoreEqual(left, right)
            | Expression::NotEqual(left, right)
            | Expression::Equal(left, right)
            | Expression::BitwiseAnd(left, right)
            | Expression::BitwiseOr(left, right)
            | Expression::BitwiseXor(left, right)
            | Expression::Or(left, right)
            | Expression::And(left, right) => {
                self.expression_available(left)?;
                self.expression_available(right)
            }
            Expression::Ternary(cond, left, right) => {
                self.expression_available(cond)?;
                self.expression_available(left)?;
                self.expression_available(right)
            }
            Expression::List(list) => {
                for expr in list {
                    self.expression_available(expr)?;
                }
                Ok(())
            }
            Expression::Variation(list) => {
                for expr in list.iter().flatten() {
                    self.expression_available(expr)?;
                }
                Ok(())
            }
            Expression::Stream(stream) => {
                for expr in &stream.bit_spec {
                    self.expression_available(expr)?;
                }
                for expr in &stream.stream {
                    self.expression_available(expr)?;
                }
                Ok(())
            }
        }
    }

    fn bitfield_known(
        &self,
        value: &Rc<Expression>,
        length: &Rc<Expression>,
        skip: &Option<Rc<Expression>>,
    ) -> Option<Result<(), String>> {
        let name = match value.as_ref() {
            Expression::Identifier(name) => name,
            Expression::Complement(expr) => {
                if let Expression::Identifier(name) = expr.as_ref() {
                    name
                } else {
                    return None;
                }
            }
            _ => {
                return None;
            }
        };

        let length = if let Expression::Number(v) = self.const_folding(length).as_ref() {
            *v
        } else {
            return None;
        };

        let skip = if let Some(skip) = skip {
            if let Expression::Number(v) = self.const_folding(skip).as_ref() {
                *v
            } else {
                return None;
            }
        } else {
            0
        };

        let mask = gen_mask(length) << skip;

        if self.is_set(name, mask) {
            Some(Ok(()))
        } else {
            Some(Err(name.to_string()))
        }
    }

    /// For the given parameter, get the mask
    pub fn param_to_mask(&self, param: &ParameterSpec) -> Result<i64, String> {
        let max = param.max.eval(&self.constants)?.0 as u64;

        Ok((max + 1).next_power_of_two() as i64 - 1)
    }

    /// Mask results
    fn mask_results(&mut self) -> Result<(), String> {
        for param in &self.irp.parameters {
            let mask = self.param_to_mask(param)?;

            if let Some(fields) = self.cur.vars.get(&param.name) {
                if (fields & !mask) != 0 {
                    self.add_action(Action::Set {
                        var: param.name.to_owned(),
                        expr: Rc::new(Expression::BitwiseAnd(
                            Rc::new(Expression::Identifier(param.name.to_owned())),
                            Rc::new(Expression::Number(mask)),
                        )),
                    });
                }
            }
        }

        Ok(())
    }

    /// Constanting folding of expressions. This simply folds constants values and a few
    /// expression. More to be done.
    pub fn const_folding(&self, expr: &Rc<Expression>) -> Rc<Expression> {
        macro_rules! unary {
            ($expr:expr, $op:ident) => {{
                if let Expression::Number(expr) = $expr.as_ref() {
                    Some(Rc::new(Expression::Number(expr.$op().into())))
                } else {
                    None
                }
            }};
        }

        macro_rules! binary {
            ($left:expr, $right:expr, $op:ident) => {{
                if let (Expression::Number(left), Expression::Number(right)) =
                    ($left.as_ref(), $right.as_ref())
                {
                    Some(Rc::new(Expression::Number(left.$op(right))))
                } else {
                    None
                }
            }};
        }

        let new = clone_filter(expr, &|expr| match expr.as_ref() {
            Expression::Identifier(name) => {
                let (val, _) = self.constants.get(name).ok()?;
                Some(Rc::new(Expression::Number(val)))
            }
            Expression::Complement(expr) => unary!(expr, not),
            Expression::BitCount(expr) => unary!(expr, count_ones),
            Expression::Negative(expr) => unary!(expr, neg),

            Expression::Add(left, right) => binary!(left, right, add),
            Expression::Subtract(left, right) => binary!(left, right, sub),
            Expression::Modulo(left, right) => binary!(left, right, rem),
            Expression::Multiply(left, right) => {
                // fold multiply by power-of-two into shift
                match (left.as_ref(), right.as_ref()) {
                    (Expression::Number(left), Expression::Number(right)) => {
                        Some(Rc::new(Expression::Number(left * right)))
                    }
                    (Expression::Number(no), _) if (*no as u64).is_power_of_two() => {
                        Some(Rc::new(Expression::ShiftLeft(
                            right.clone(),
                            Rc::new(Expression::Number((*no).trailing_zeros() as i64)),
                        )))
                    }
                    (_, Expression::Number(no)) if (*no as u64).is_power_of_two() => {
                        Some(Rc::new(Expression::ShiftLeft(
                            left.clone(),
                            Rc::new(Expression::Number((*no).trailing_zeros() as i64)),
                        )))
                    }
                    _ => None,
                }
            }
            Expression::Divide(left, right) => {
                // fold divide by power-of-two into shift
                match (left.as_ref(), right.as_ref()) {
                    (Expression::Number(left), Expression::Number(right)) => {
                        Some(Rc::new(Expression::Number(left / right)))
                    }
                    (_, Expression::Number(no)) if (*no as u64).is_power_of_two() => {
                        Some(Rc::new(Expression::ShiftRight(
                            left.clone(),
                            Rc::new(Expression::Number((*no).trailing_zeros() as i64)),
                        )))
                    }
                    _ => None,
                }
            }
            Expression::BitwiseAnd(left, right) => binary!(left, right, bitand),
            Expression::BitwiseOr(left, right) => binary!(left, right, bitor),
            Expression::BitwiseXor(left, right) => binary!(left, right, bitxor),
            Expression::ShiftLeft(left, right) => binary!(left, right, shl),
            Expression::ShiftRight(left, right) => binary!(left, right, shr),
            // TODO: logical (Not, And, Or, Equal, More, Ternary)
            _ => None,
        });

        new.unwrap_or_else(|| expr.clone())
    }
}
