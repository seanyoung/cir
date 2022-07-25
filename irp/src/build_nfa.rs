use super::{
    expression::{clone_filter, inverse},
    Expression, Irp, RepeatMarker, Vartable,
};
use std::{collections::HashMap, rc::Rc};

/**
 * Here we build the decoder nfa (non-deterministic finite automation)
 */

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
                        let (val, len) = expr.eval(&builder.constants).unwrap();

                        builder.constants.set(name.to_owned(), val, len);

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
        list: &[Rc<Expression>],
        builder: &mut Builder,
        bit_spec: &[&[Rc<Expression>]],
    ) -> Result<(), String> {
        let mut pos = 0;

        while pos < list.len() {
            let mut bit_count = 0;
            let mut expr_count = 0;

            while let Some(expr) = list.get(pos + expr_count) {
                if let Expression::BitField { length, .. } = expr.as_ref() {
                    let (length, _) = builder.const_folding(length).eval(&Vartable::new())?;

                    bit_count += length;
                    expr_count += 1;
                } else {
                    break;
                }
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
                        reverse,
                    } = list[i + pos].as_ref()
                    {
                        let (length, _) = builder.const_folding(length).eval(&Vartable::new())?;

                        if !self.general_spec.lsb {
                            offset -= length;
                        }

                        let skip = if let Some(skip) = skip {
                            let (skip, _) = builder.const_folding(skip).eval(&Vartable::new())?;

                            skip
                        } else {
                            0
                        };

                        let value = builder.const_folding(value);

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

                        let bits = if *reverse {
                            Expression::BitReverse(Rc::new(bits), length, skip)
                        } else {
                            bits
                        };

                        match value.as_ref() {
                            Expression::Complement(inner_expr) => {
                                let bits = Expression::Complement(Rc::new(bits));

                                match inner_expr.as_ref() {
                                    Expression::Identifier(name) => {
                                        self.store_bits_in_var(
                                            name,
                                            bits,
                                            length,
                                            skip,
                                            builder,
                                            &mut delayed,
                                        )?;
                                    }
                                    Expression::Number(_) => self.check_bits_in_var(
                                        inner_expr, bits, length, skip, builder,
                                    )?,
                                    _ => {
                                        return Err(format!(
                                            "expression {} not supported",
                                            inner_expr
                                        ));
                                    }
                                }
                            }
                            Expression::Identifier(name) => {
                                self.store_bits_in_var(
                                    name,
                                    bits,
                                    length,
                                    skip,
                                    builder,
                                    &mut delayed,
                                )?;
                            }
                            expr if builder.unknown_var(expr).is_ok() => {
                                self.check_bits_in_var(&value, bits, length, skip, builder)?
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
        bits: Expression,
        length: i64,
        skip: i64,
        builder: &mut Builder,
        delayed: &mut Vec<Action>,
    ) -> Result<(), String> {
        let mask = gen_mask(length) << skip;

        let expr = Expression::BitwiseAnd(Rc::new(bits), Rc::new(Expression::Number(mask)));

        if builder.is_set(name, mask) {
            builder.add_action(Action::AssertEq {
                left: Expression::BitwiseAnd(
                    Rc::new(Expression::Identifier(name.to_owned())),
                    Rc::new(Expression::Number(mask)),
                ),
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
                left: Expression::BitwiseAnd(
                    builder.const_folding(def),
                    Rc::new(Expression::Number(mask)),
                ),
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
                    Rc::new(Expression::Identifier(name.to_owned())),
                    Rc::new(expr),
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
        bits: Expression,
        length: i64,
        skip: i64,
        builder: &mut Builder,
    ) -> Result<(), String> {
        let mask = gen_mask(length) << skip;

        let left = Expression::BitwiseAnd(Rc::new(bits), Rc::new(Expression::Number(mask)));
        let right =
            Expression::BitwiseAnd(Rc::new(value.clone()), Rc::new(Expression::Number(mask)));

        builder.add_action(Action::AssertEq { left, right });

        Ok(())
    }

    fn bit_field(
        &self,
        length: i64,
        builder: &mut Builder,
        bit_spec: &[&[Rc<Expression>]],
    ) -> Result<(), String> {
        // TODO: special casing when length == 1
        builder.cur.seen_edges = true;

        let before = builder.cur.head;

        let entry = builder.add_vertex();

        let next = builder.add_vertex();

        let done = builder.add_vertex();

        builder.add_edge(Edge::Branch(entry));

        builder.set_head(entry);

        let width = match bit_spec[0].len() {
            2 => 1,
            4 => 2,
            8 => 4,
            16 => 8,
            w => {
                return Err(format!("bit spec with {} fields not supported", w));
            }
        };

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
                        Rc::new(Expression::Identifier(String::from("$b"))),
                        Rc::new(Expression::Number(width)),
                    ),
                },
            );
            builder.add_action_at_node(
                next,
                Action::Set {
                    var: String::from("$bits"),
                    expr: Expression::BitwiseOr(
                        Rc::new(Expression::Identifier(String::from("$bits"))),
                        Rc::new(Expression::ShiftLeft(
                            Rc::new(Expression::Identifier(String::from("$v"))),
                            Rc::new(Expression::Identifier(String::from("$b"))),
                        )),
                    ),
                },
            );
            builder.add_edge_at_node(
                next,
                Edge::BranchCond {
                    expr: Expression::More(
                        Rc::new(Expression::Identifier(String::from("$b"))),
                        Rc::new(Expression::Number(0)),
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
                        Rc::new(Expression::Identifier(String::from("$bits"))),
                        Rc::new(Expression::ShiftLeft(
                            Rc::new(Expression::Identifier(String::from("$v"))),
                            Rc::new(Expression::Identifier(String::from("$b"))),
                        )),
                    ),
                },
            );
            builder.add_action_at_node(
                next,
                Action::Set {
                    var: String::from("$b"),
                    expr: Expression::Add(
                        Rc::new(Expression::Identifier(String::from("$b"))),
                        Rc::new(Expression::Number(width)),
                    ),
                },
            );

            builder.add_edge_at_node(
                next,
                Edge::BranchCond {
                    expr: Expression::Less(
                        Rc::new(Expression::Identifier(String::from("$b"))),
                        Rc::new(Expression::Number(length)),
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
        let verts = vec![Vertex::new()];

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
                if name.starts_with('$')
                    || self.cur.vars.contains_key(name)
                    || self.constants.is_defined(name)
                {
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

    fn const_folding(&mut self, expr: &Rc<Expression>) -> Rc<Expression> {
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
