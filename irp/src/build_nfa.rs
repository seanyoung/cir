use super::{
    expression::{clone_filter, inverse},
    Expression, Irp, RepeatMarker, Vartable,
};
use std::{collections::HashMap, rc::Rc};

/**
 * Here we build the decoder nfa (non-deterministic finite automation)
 *
 * TODO
 * - ExtentConstant may be very short. We should calculate minimum length
 * - (..)2 and other repeat markers are not supported
 * - (S-1):4 should produce 16, not 0 (mask in the wrong place)
 */

#[derive(PartialEq, Debug, Clone)]
pub(crate) enum Edge {
    Flash(i64, usize),
    Gap(i64, usize),
    FlashVar(String, i64, usize),
    GapVar(String, i64, usize),
    TrailingGap(usize),
    BranchCond {
        expr: Expression,
        yes: usize,
        no: usize,
    },
    MayBranchCond {
        expr: Expression,
        dest: usize,
    },
    Branch(usize),
    Done(Vec<String>),
}

#[derive(PartialEq, Debug, Clone)]
pub(crate) enum Action {
    Set { var: String, expr: Expression },
    AssertEq { left: Expression, right: Expression },
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

        builder.add_action(Action::Set {
            var: "$repeat".to_owned(),
            expr: Expression::Number(0),
        });

        builder.add_constants();

        builder.expression(&self.stream, &[])?;

        if builder.cur.seen_edges && builder.is_done() {
            let res = builder.done_fields();

            builder.add_edge(Edge::Done(res));
        }

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

fn gen_mask(v: i64) -> i64 {
    (1i64 << v) - 1
}

/// track which
#[derive(Clone, Debug)]
struct Builder<'a> {
    cur: BuilderLocation,
    saved: Vec<BuilderLocation>,
    verts: Vec<Vertex>,
    constants: Vartable<'a>,
    irp: &'a Irp,
}

#[derive(Clone, Debug, Default)]
struct BuilderLocation {
    head: usize,
    seen_edges: bool,
    vars: HashMap<String, i64>,
}

#[allow(dead_code)]
impl<'a> Builder<'a> {
    fn new(irp: &'a Irp) -> Self {
        let verts = vec![Vertex::default()];

        Builder {
            cur: Default::default(),
            saved: Vec::new(),
            constants: Vartable::new(),
            verts,
            irp,
        }
    }

    fn set(&mut self, name: &str, fields: i64) {
        if let Some(e) = self.cur.vars.get_mut(name) {
            *e |= 64;
        } else {
            self.cur.vars.insert(name.to_owned(), fields);
        }
    }

    fn is_set(&self, name: &str, fields: i64) -> bool {
        if let Some(e) = self.cur.vars.get(name) {
            (e & fields) != 0
        } else {
            false
        }
    }

    fn is_any_set(&self, name: &str) -> bool {
        if let Some(e) = self.cur.vars.get(name) {
            *e != 0
        } else {
            false
        }
    }

    fn is_done(&self) -> bool {
        self.irp
            .parameters
            .iter()
            .all(|param| self.cur.vars.contains_key(&param.name))
    }

    fn done_fields(&self) -> Vec<String> {
        self.irp
            .parameters
            .iter()
            .map(|param| param.name.to_owned())
            .collect()
    }

    fn clear(&mut self) {
        self.cur.vars.clear();
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
    /// those will have their values set now.
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

                    if self.unknown_var(expr).is_ok() {
                        if modified_anywhere {
                            // just set an initial value
                            self.add_action(Action::Set {
                                var: name.to_owned(),
                                expr: self.const_folding(expr).as_ref().clone(),
                            });
                        } else {
                            let (val, len) = expr.eval(&self.constants).unwrap();

                            self.constants.set(name.to_owned(), val, len);
                        }
                        changes = true;

                        self.set(name, i64::MAX);
                    }
                }
            }

            changes
        } {}
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

                    // if this bit field is preceded by a constant length fields, process
                    // those separately
                    if expr_count != 0 && max_len != min_len {
                        break;
                    }

                    if min_len > 64 || max_len > 64 {
                        return Err(format!(
                            "bitfields of {}..{} longer than the 64 maximum",
                            min_len, max_len
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
                self.expression(&list[pos], bit_spec)?;
                pos += 1;
                continue;
            }

            // if it is a single constant bitfield, just expand it - no loops needed
            if expr_count == 1 && bit_count < 4 {
                if let Expression::BitField {
                    value,
                    reverse: false,
                    skip: None,
                    ..
                } = list[pos].as_ref()
                {
                    if let Expression::Number(value) = self.const_folding(value).as_ref() {
                        if self.irp.general_spec.lsb {
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
                        if let Some(length) = &store_length {
                            self.set(length, !0);
                        }

                        self.add_bit_specs(
                            Some(min_len),
                            max_len,
                            *reverse,
                            store_length,
                            bit_spec,
                        )?;

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
                                expr: bits,
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

            self.add_bit_specs(None, bit_count, do_reverse, None, bit_spec)?;

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

                    let value = self.const_folding(value);

                    let bits = Expression::Identifier(String::from("$bits"));

                    #[allow(clippy::comparison_chain)]
                    let bits = if offset > skip {
                        Expression::ShiftRight(
                            Rc::new(bits),
                            Rc::new(Expression::Number(offset - skip)),
                        )
                    } else if offset < skip {
                        Expression::ShiftLeft(
                            Rc::new(bits),
                            Rc::new(Expression::Number(skip - offset)),
                        )
                    } else {
                        bits
                    };

                    let bits = if *reverse && !do_reverse {
                        Expression::BitReverse(Rc::new(bits), length, skip)
                    } else {
                        bits
                    };

                    match self.unknown_var(expr) {
                        Ok(_) => {
                            // We know all the variables in here or its constant
                            self.check_bits_in_var(&value, bits, length, skip)?;
                        }
                        Err(name) => match inverse(Rc::new(bits), value.clone(), &name) {
                            Some(bits) => {
                                self.store_bits_in_var(&name, bits, length, skip, &mut delayed)?;
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
                    Ok((
                        param.min.eval(&self.constants)?.0,
                        param.max.eval(&self.constants)?.0,
                        Some(name.to_owned()),
                    ))
                } else {
                    Err(format!("bit field length {} is not a parameter", name))
                }
            }
            expr => Err(format!("bit field length {} not known", expr)),
        }
    }

    fn store_bits_in_var(
        &mut self,
        name: &str,
        bits: Rc<Expression>,
        length: i64,
        skip: i64,
        delayed: &mut Vec<Action>,
    ) -> Result<(), String> {
        let mask = gen_mask(length) << skip;

        let expr = Expression::BitwiseAnd(bits, Rc::new(Expression::Number(mask)));

        if self.is_set(name, mask) {
            self.add_action(Action::AssertEq {
                left: Expression::BitwiseAnd(
                    Rc::new(Expression::Identifier(name.to_owned())),
                    Rc::new(Expression::Number(mask)),
                ),
                right: expr,
            });
        } else if let Some(def) = self.irp.definitions.iter().find_map(|def| {
            if let Expression::Assignment(var, expr) = def {
                if name == var {
                    return Some(expr);
                }
            }
            None
        }) {
            let have_it = match self.unknown_var(def) {
                Ok(_) => true,
                Err(name) => self.add_definition(&name),
            };

            let action = Action::AssertEq {
                left: Expression::BitwiseAnd(
                    self.const_folding(def),
                    Rc::new(Expression::Number(mask)),
                ),
                right: expr,
            };

            if have_it {
                self.add_action(action);
            } else {
                delayed.push(action);
            }
        } else {
            let expr = if self.is_any_set(name) {
                Expression::BitwiseOr(
                    Rc::new(Expression::Identifier(name.to_owned())),
                    Rc::new(expr),
                )
            } else {
                expr
            };

            self.set(name, mask);

            self.add_action(Action::Set {
                var: name.to_owned(),
                expr,
            });
        }

        Ok(())
    }

    fn check_bits_in_var(
        &mut self,
        value: &Expression,
        bits: Expression,
        length: i64,
        skip: i64,
    ) -> Result<(), String> {
        let mask = gen_mask(length) << skip;

        let left = Expression::BitwiseAnd(Rc::new(bits), Rc::new(Expression::Number(mask)));
        let right =
            Expression::BitwiseAnd(Rc::new(value.clone()), Rc::new(Expression::Number(mask)));

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
            loop {
                match self.unknown_var(def) {
                    Ok(_) => {
                        self.add_action(Action::Set {
                            var: name.to_owned(),
                            expr: def.as_ref().clone(),
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
        } else {
            false
        }
    }

    /// For given expression, add any required definitions or return an error if
    /// no definitions can be found
    fn have_definitions(&mut self, expr: &Expression) -> Result<(), String> {
        loop {
            match self.unknown_var(expr) {
                Ok(_) => {
                    return Ok(());
                }
                Err(name) => {
                    if !self.add_definition(&name) {
                        return Err(format!("variable '{}' not known", name));
                    }
                }
            }
        }
    }

    /// Add the bit specs for bitfields. max specifies the maximum number of
    /// bits, min specifies a minimum if the number of bits is not fixed at max.
    /// The length can be stored in the variable name store_length if set.
    fn add_bit_specs(
        &mut self,
        min: Option<i64>,
        max: i64,
        reverse: bool,
        store_length: Option<String>,
        bit_spec: &[&[Rc<Expression>]],
    ) -> Result<(), String> {
        // TODO: special casing when length == 1
        self.cur.seen_edges = true;

        let before = self.cur.head;

        let entry = self.add_vertex();

        let next = self.add_vertex();

        let done = self.add_vertex();

        self.add_edge(Edge::Branch(entry));

        self.set_head(entry);

        let width = match bit_spec[0].len() {
            2 => 1,
            4 => 2,
            8 => 4,
            16 => 8,
            w => {
                return Err(format!("bit spec with {} fields not supported", w));
            }
        };

        let length = store_length.unwrap_or_else(|| String::from("$b"));

        for (bit, e) in bit_spec[0].iter().enumerate() {
            self.push_location();

            self.expression(e, &bit_spec[1..])?;

            self.add_action(Action::Set {
                var: String::from("$v"),
                expr: Expression::Number(bit as i64),
            });

            self.add_edge(Edge::Branch(next));

            self.pop_location();
        }

        self.add_action_at_node(
            before,
            Action::Set {
                var: length.to_owned(),
                expr: Expression::Number(0),
            },
        );

        self.add_action_at_node(
            before,
            Action::Set {
                var: String::from("$bits"),
                expr: Expression::Number(0),
            },
        );

        if !(self.irp.general_spec.lsb ^ reverse) {
            self.add_action_at_node(
                next,
                Action::Set {
                    var: String::from("$bits"),
                    expr: Expression::BitwiseOr(
                        Rc::new(Expression::ShiftLeft(
                            Rc::new(Expression::Identifier(String::from("$bits"))),
                            Rc::new(Expression::Number(width)),
                        )),
                        Rc::new(Expression::Identifier(String::from("$v"))),
                    ),
                },
            );
        } else {
            self.add_action_at_node(
                next,
                Action::Set {
                    var: String::from("$bits"),
                    expr: Expression::BitwiseOr(
                        Rc::new(Expression::Identifier(String::from("$bits"))),
                        Rc::new(Expression::ShiftLeft(
                            Rc::new(Expression::Identifier(String::from("$v"))),
                            Rc::new(Expression::Identifier(length.to_owned())),
                        )),
                    ),
                },
            );
        }

        self.add_action_at_node(
            next,
            Action::Set {
                var: length.to_owned(),
                expr: Expression::Add(
                    Rc::new(Expression::Identifier(length.to_owned())),
                    Rc::new(Expression::Number(width)),
                ),
            },
        );

        self.add_edge_at_node(
            next,
            Edge::BranchCond {
                expr: Expression::Less(
                    Rc::new(Expression::Identifier(length.to_owned())),
                    Rc::new(Expression::Number(max)),
                ),
                yes: entry,
                no: done,
            },
        );

        if let Some(min) = min {
            self.add_edge_at_node(
                next,
                Edge::MayBranchCond {
                    expr: Expression::MoreEqual(
                        Rc::new(Expression::Identifier(length)),
                        Rc::new(Expression::Number(min)),
                    ),
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
                };

                if irstream.repeat == Some(RepeatMarker::Any)
                    || irstream.repeat == Some(RepeatMarker::OneOrMore)
                {
                    let mut start = if self.cur.seen_edges {
                        Some(self.cur.head)
                    } else {
                        None
                    };

                    let done_before = if self.is_done() {
                        let res = self.done_fields();

                        self.add_edge(Edge::Done(res));

                        let node = self.add_vertex();

                        self.add_edge(Edge::Branch(node));

                        self.set_head(node);

                        if start.is_some() {
                            start = Some(node);
                        }

                        self.add_edge(Edge::Branch(0));

                        true
                    } else {
                        false
                    };

                    self.expression_list(&irstream.stream, &bit_spec)?;

                    self.add_action(Action::Set {
                        var: "$repeat".to_owned(),
                        expr: Expression::Number(1),
                    });

                    if !done_before && self.is_done() {
                        let res = self.done_fields();

                        self.add_edge(Edge::Done(res));
                    }

                    if let Some(start) = start {
                        self.add_edge(Edge::Branch(start));
                    }
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
                    return Err(format!("variable '{}' is not set", var));
                }

                let unit = unit.eval(1, &self.irp.general_spec)?;

                let node = self.add_vertex();

                self.add_edge(Edge::FlashVar(var.to_owned(), unit, node));

                self.set_head(node);
            }
            Expression::GapIdentifier(var, unit) => {
                if !self.is_any_set(var) {
                    return Err(format!("variable '{}' is not set", var));
                }

                let unit = unit.eval(1, &self.irp.general_spec)?;

                let node = self.add_vertex();

                self.add_edge(Edge::GapVar(var.to_owned(), unit, node));

                self.set_head(node);
            }
            Expression::BitField { length, .. } => {
                let (length, _) = length.eval(&Vartable::new())?;

                self.add_bit_specs(None, length, false, None, bit_spec)?;
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
                    expr: self.const_folding(expr).as_ref().clone(),
                })
            }
            _ => println!("expr:{:?}", expr),
        }

        Ok(())
    }

    /// Do we have all the vars to evaluate an expression
    fn unknown_var(&self, expr: &Expression) -> Result<(), String> {
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
                        self.unknown_var(skip)?;
                    }
                    self.unknown_var(value)?;
                    self.unknown_var(length)
                }
            }
            Expression::InfiniteBitField { value, skip } => {
                self.unknown_var(value)?;
                self.unknown_var(skip)
            }
            Expression::Assignment(_, expr)
            | Expression::Complement(expr)
            | Expression::Not(expr)
            | Expression::Negative(expr)
            | Expression::BitCount(expr)
            | Expression::BitReverse(expr, ..) => self.unknown_var(expr),
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
                self.unknown_var(left)?;
                self.unknown_var(right)
            }
            Expression::Ternary(cond, left, right) => {
                self.unknown_var(cond)?;
                self.unknown_var(left)?;
                self.unknown_var(right)
            }
            Expression::List(list) => {
                for expr in list {
                    self.unknown_var(expr)?;
                }
                Ok(())
            }
            Expression::Variation(list) => {
                for expr in list.iter().flatten() {
                    self.unknown_var(expr)?;
                }
                Ok(())
            }
            Expression::Stream(stream) => {
                for expr in &stream.bit_spec {
                    self.unknown_var(expr)?;
                }
                for expr in &stream.stream {
                    self.unknown_var(expr)?;
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

        if self.is_set(name, gen_mask(length) << skip) {
            Some(Ok(()))
        } else {
            Some(Err(name.to_string()))
        }
    }

    fn const_folding(&self, expr: &Rc<Expression>) -> Rc<Expression> {
        let new = clone_filter(expr, &|expr| match expr.as_ref() {
            Expression::Identifier(name) => {
                let (val, _) = self.constants.get(name).ok()?;
                Some(Rc::new(Expression::Number(val)))
            }
            _ => None,
        });

        new.unwrap_or_else(|| expr.clone())
    }
}
