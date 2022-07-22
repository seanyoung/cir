use super::{Expression, Irp, RepeatMarker, Vartable};
use std::collections::HashMap;

// This is the decoder nfa (non-deterministic finite automation)
//
// From the IRP, we build the nfa
// from the nfa we build the dfa
// from the dfa we build clif
// from clif we the BPF decoder (cranelift does this)

// clif is a compiler IR. This means basic blocks with a single
// flow control instruction at the end of the block. So, we try to model
// the nfa such that it is easy to transform.

#[derive(PartialEq, Debug, Clone)]
pub(crate) enum Edge {
    Flash(i64, usize),
    Gap(i64, usize),
    TrailingGap(usize),
    BranchCond {
        expr: Expression,
        yes: usize,
        no: usize,
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

impl Vertex {
    fn new() -> Self {
        Default::default()
    }
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

        // set all definitions which are constant
        // Note that we support {C=D+1,D=1}; we first D=1 in the first iteration
        // and C=D+1 in the second.
        while {
            let mut changes = false;

            for def in &self.definitions {
                if let Expression::Assignment(name, expr) = def {
                    if builder.is_any_set(name) {
                        continue;
                    }

                    if builder.unknown_var(expr).is_ok() {
                        // FIXME: constant folding/eval here?
                        builder.add_action(Action::Set {
                            var: name.to_owned(),
                            expr: expr.as_ref().clone(),
                        });

                        changes = true;

                        builder.set(name, i64::MAX);
                    }
                }
            }

            changes
        } {}

        self.expression(&self.stream, &mut builder, &[])?;

        if builder.cur.seen_edges && builder.is_done() {
            let res = builder.done_fields();

            builder.add_edge(Edge::Done(res));
        }

        Ok(NFA {
            verts: builder.complete(),
        })
    }

    fn expression_list(
        &self,
        list: &[Expression],
        builder: &mut Builder,
        bit_spec: &[&[Expression]],
    ) -> Result<(), String> {
        let mut pos = 0;

        while pos < list.len() {
            let mut bit_count = 0;
            let mut expr_count = 0;

            while let Some(Expression::BitField { length, .. }) = list.get(pos + expr_count) {
                let (length, _) = length.eval(&Vartable::new())?;

                bit_count += length;
                expr_count += 1;
            }

            if expr_count == 0 {
                self.expression(&list[pos], builder, bit_spec)?;
                pos += 1;
            } else {
                self.bit_field(bit_count, builder, bit_spec)?;

                let mut delayed = Vec::new();

                // now do stuff with bitfields
                let mut offset = if self.general_spec.lsb { 0 } else { bit_count };

                for i in 0..expr_count {
                    if let Expression::BitField {
                        value,
                        length,
                        skip,
                        ..
                    } = &list[i + pos]
                    {
                        let (length, _) = length.eval(&Vartable::new())?;

                        if !self.general_spec.lsb {
                            offset -= length;
                        }

                        let skip = if let Some(skip) = skip {
                            let (skip, _) = skip.eval(&Vartable::new())?;

                            skip
                        } else {
                            0
                        };

                        match value.as_ref() {
                            Expression::Complement(expr) => match expr.as_ref() {
                                Expression::Identifier(name) => {
                                    self.store_bits_in_var(
                                        name,
                                        offset,
                                        true,
                                        length,
                                        skip,
                                        builder,
                                        &mut delayed,
                                    )?;
                                }
                                Expression::Number(_) => self.check_bits_in_var(
                                    value, offset, true, length, skip, builder,
                                )?,
                                _ => {
                                    return Err(format!("expression {} not supported", expr));
                                }
                            },
                            Expression::Identifier(name) => {
                                self.store_bits_in_var(
                                    name,
                                    offset,
                                    false,
                                    length,
                                    skip,
                                    builder,
                                    &mut delayed,
                                )?;
                            }
                            Expression::Number(_) => {
                                self.check_bits_in_var(value, offset, false, length, skip, builder)?
                            }
                            expr => {
                                return Err(format!("expression {} not supported", expr));
                            }
                        }

                        if self.general_spec.lsb {
                            offset += length;
                        }
                    }
                }

                for action in delayed {
                    match &action {
                        Action::AssertEq { left, right } => {
                            builder.unknown_var(left)?;
                            builder.unknown_var(right)?;
                        }
                        Action::Set { expr, .. } => {
                            builder.unknown_var(expr)?;
                        }
                    }
                    builder.add_action(action);
                }

                pos += expr_count;
            }
        }

        Ok(())
    }

    fn store_bits_in_var(
        &self,
        name: &str,
        offset: i64,
        complement: bool,
        length: i64,
        skip: i64,
        builder: &mut Builder,
        delayed: &mut Vec<Action>,
    ) -> Result<(), String> {
        let expr = Expression::Identifier(String::from("$bits"));

        let expr = if complement {
            Expression::Complement(Box::new(expr))
        } else {
            expr
        };

        #[allow(clippy::comparison_chain)]
        let expr = if offset > skip {
            Expression::ShiftRight(Box::new(expr), Box::new(Expression::Number(offset - skip)))
        } else if offset < skip {
            Expression::ShiftLeft(Box::new(expr), Box::new(Expression::Number(skip - offset)))
        } else {
            expr
        };

        let mask = gen_mask(length) << skip;

        let expr = Expression::BitwiseAnd(Box::new(expr), Box::new(Expression::Number(mask)));

        if builder.is_set(name, mask) {
            builder.add_action(Action::AssertEq {
                left: Expression::Identifier(name.to_owned()),
                right: expr,
            });
        } else if let Some(def) = self.definitions.iter().find_map(|def| {
            if let Expression::Assignment(var, expr) = def {
                if name == var {
                    return Some(expr);
                }
            }
            None
        }) {
            let have_it = builder.unknown_var(def).is_ok();

            let action = Action::AssertEq {
                left: def.as_ref().clone(),
                right: expr,
            };

            if have_it {
                builder.add_action(action);
            } else {
                delayed.push(action);
            }
        } else {
            let expr = if builder.is_any_set(name) {
                Expression::BitwiseOr(
                    Box::new(Expression::Identifier(name.to_owned())),
                    Box::new(expr),
                )
            } else {
                expr
            };

            builder.set(name, mask);

            builder.add_action(Action::Set {
                var: name.to_owned(),
                expr,
            });
        }

        Ok(())
    }

    fn check_bits_in_var(
        &self,
        value: &Expression,
        offset: i64,
        complement: bool,
        length: i64,
        skip: i64,
        builder: &mut Builder,
    ) -> Result<(), String> {
        let expr = Expression::Identifier(String::from("$bits"));

        let expr = if complement {
            Expression::Complement(Box::new(expr))
        } else {
            expr
        };

        #[allow(clippy::comparison_chain)]
        let expr = if offset > skip {
            Expression::ShiftRight(Box::new(expr), Box::new(Expression::Number(offset - skip)))
        } else if offset < skip {
            Expression::ShiftLeft(Box::new(expr), Box::new(Expression::Number(skip - offset)))
        } else {
            expr
        };

        let mask = gen_mask(length) << skip;

        let expr = Expression::BitwiseAnd(Box::new(expr), Box::new(Expression::Number(mask)));

        builder.add_action(Action::AssertEq {
            left: expr,
            right: value.clone(),
        });

        Ok(())
    }

    fn bit_field(
        &self,
        length: i64,
        builder: &mut Builder,
        bit_spec: &[&[Expression]],
    ) -> Result<(), String> {
        // TODO: special casing when length == 1
        builder.cur.seen_edges = true;

        let before = builder.cur.head;

        let entry = builder.add_vertex();

        let next = builder.add_vertex();

        let done = builder.add_vertex();

        builder.add_edge(Edge::Branch(entry));

        builder.set_head(entry);

        for (bit, e) in bit_spec[0].iter().enumerate() {
            builder.push_location();

            self.expression(e, builder, &bit_spec[1..])?;

            builder.add_action(Action::Set {
                var: String::from("$v"),
                expr: Expression::Number(bit as i64),
            });

            builder.add_edge(Edge::Branch(next));

            builder.pop_location();
        }

        if !self.general_spec.lsb {
            builder.add_action_at_node(
                before,
                Action::Set {
                    var: String::from("$b"),
                    expr: Expression::Number(length),
                },
            );

            builder.add_action_at_node(
                before,
                Action::Set {
                    var: String::from("$bits"),
                    expr: Expression::Number(0),
                },
            );

            builder.add_action_at_node(
                next,
                Action::Set {
                    var: String::from("$b"),
                    expr: Expression::Subtract(
                        Box::new(Expression::Identifier(String::from("$b"))),
                        Box::new(Expression::Number(1)),
                    ),
                },
            );
            builder.add_action_at_node(
                next,
                Action::Set {
                    var: String::from("$bits"),
                    expr: Expression::BitwiseOr(
                        Box::new(Expression::Identifier(String::from("$bits"))),
                        Box::new(Expression::ShiftLeft(
                            Box::new(Expression::Identifier(String::from("$v"))),
                            Box::new(Expression::Identifier(String::from("$b"))),
                        )),
                    ),
                },
            );
            builder.add_edge_at_node(
                next,
                Edge::BranchCond {
                    expr: Expression::More(
                        Box::new(Expression::Identifier(String::from("$b"))),
                        Box::new(Expression::Number(0)),
                    ),
                    yes: entry,
                    no: done,
                },
            );
        } else {
            builder.add_action_at_node(
                before,
                Action::Set {
                    var: String::from("$b"),
                    expr: Expression::Number(0),
                },
            );

            builder.add_action_at_node(
                before,
                Action::Set {
                    var: String::from("$bits"),
                    expr: Expression::Number(0),
                },
            );

            builder.add_action_at_node(
                next,
                Action::Set {
                    var: String::from("$bits"),
                    expr: Expression::BitwiseOr(
                        Box::new(Expression::Identifier(String::from("$bits"))),
                        Box::new(Expression::ShiftLeft(
                            Box::new(Expression::Identifier(String::from("$v"))),
                            Box::new(Expression::Identifier(String::from("$b"))),
                        )),
                    ),
                },
            );
            builder.add_action_at_node(
                next,
                Action::Set {
                    var: String::from("$b"),
                    expr: Expression::Add(
                        Box::new(Expression::Identifier(String::from("$b"))),
                        Box::new(Expression::Number(1)),
                    ),
                },
            );

            builder.add_edge_at_node(
                next,
                Edge::BranchCond {
                    expr: Expression::Less(
                        Box::new(Expression::Identifier(String::from("$b"))),
                        Box::new(Expression::Number(length)),
                    ),
                    yes: entry,
                    no: done,
                },
            );
        }

        builder.set_head(done);

        Ok(())
    }

    fn expression(
        &self,
        expr: &Expression,
        builder: &mut Builder,
        bit_spec: &[&[Expression]],
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
                    let mut start = if builder.cur.seen_edges {
                        Some(builder.cur.head)
                    } else {
                        None
                    };

                    let done_before = if builder.is_done() {
                        let res = builder.done_fields();

                        builder.add_edge(Edge::Done(res));

                        let node = builder.add_vertex();

                        builder.add_edge(Edge::Branch(node));

                        builder.set_head(node);

                        if start.is_some() {
                            start = Some(node);
                        }

                        builder.add_edge(Edge::Branch(0));

                        true
                    } else {
                        false
                    };

                    self.expression_list(&irstream.stream, builder, &bit_spec)?;

                    builder.add_action(Action::Set {
                        var: "$repeat".to_owned(),
                        expr: Expression::Number(1),
                    });

                    if !done_before && builder.is_done() {
                        let res = builder.done_fields();

                        builder.add_edge(Edge::Done(res));
                    }

                    if let Some(start) = start {
                        builder.add_edge(Edge::Branch(start));
                    }
                } else {
                    self.expression_list(&irstream.stream, builder, &bit_spec)?;
                }
            }
            Expression::List(list) => {
                self.expression_list(list, builder, bit_spec)?;
            }
            Expression::FlashConstant(v, u) => {
                builder.cur.seen_edges = true;

                let len = u.eval_float(*v, &self.general_spec)?;

                let node = builder.add_vertex();

                builder.add_edge(Edge::Flash(len, node));

                builder.set_head(node);
            }
            Expression::GapConstant(v, u) => {
                builder.cur.seen_edges = true;

                let len = u.eval_float(*v, &self.general_spec)?;

                let node = builder.add_vertex();

                builder.add_edge(Edge::Gap(len, node));

                builder.set_head(node);
            }
            Expression::BitField { length, .. } => {
                let (length, _) = length.eval(&Vartable::new())?;

                self.bit_field(length, builder, bit_spec)?;
            }
            Expression::ExtentConstant(_, _) => {
                builder.cur.seen_edges = true;

                let node = builder.add_vertex();

                builder.add_edge(Edge::TrailingGap(node));

                builder.set_head(node);
            }
            _ => println!("expr:{:?}", expr),
        }

        Ok(())
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
        let verts = vec![Vertex::new()];

        Builder {
            cur: Default::default(),
            saved: Vec::new(),
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

        self.verts.push(Vertex::new());

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
                if name.starts_with('$') || self.cur.vars.contains_key(name) {
                    Ok(())
                } else {
                    Err(format!("variable '{}' not known", name))
                }
            }
            Expression::BitField {
                value,
                length,
                skip: Some(skip),
                ..
            } => {
                self.unknown_var(value)?;
                self.unknown_var(length)?;
                self.unknown_var(skip)
            }
            Expression::BitField {
                value,
                length,
                skip: None,
                ..
            } => {
                self.unknown_var(value)?;
                self.unknown_var(length)
            }
            Expression::InfiniteBitField { value, skip } => {
                self.unknown_var(value)?;
                self.unknown_var(skip)
            }
            Expression::Assignment(_, expr)
            | Expression::Complement(expr)
            | Expression::Not(expr)
            | Expression::Negative(expr)
            | Expression::BitCount(expr) => self.unknown_var(expr),
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
}
