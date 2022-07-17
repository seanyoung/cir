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
    /// Generate an NFA decoder for this IRP
    pub fn build_nfa(&self) -> Result<NFA, String> {
        let mut verts: Vec<Vertex> = vec![Vertex::new()];
        let mut heads: Vec<BuilderPos> = vec![BuilderPos::new()];

        self.expression(&self.stream, &mut verts, &mut heads, &[])?;

        for pos in heads {
            let res: Vec<String> = pos
                .vars
                .into_keys()
                .filter(|v| !v.starts_with('$'))
                .collect();

            if !res.is_empty() && pos.seen_edges {
                verts[pos.head].edges.push(Edge::Done(res));
            }
        }

        Ok(NFA { verts })
    }

    fn expression_list(
        &self,
        list: &[Expression],
        verts: &mut Vec<Vertex>,
        heads: &mut Vec<BuilderPos>,
        bit_spec: &[Expression],
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
                self.expression(&list[pos], verts, heads, bit_spec)?;
                pos += 1;
            } else {
                self.bit_field(bit_count, verts, heads, bit_spec)?;

                // now do stuff with bitfields
                let mut offset = bit_count;
                for i in 0..expr_count {
                    if let Expression::BitField {
                        value,
                        length,
                        skip,
                        ..
                    } = &list[i + pos]
                    {
                        let (length, _) = length.eval(&Vartable::new())?;

                        offset -= length;

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
                                        name, offset, true, length, skip, verts, heads,
                                    )?;
                                }
                                Expression::Number(_) => self.check_bits_in_var(
                                    value, offset, true, length, skip, verts, heads,
                                )?,
                                _ => unimplemented!("{:?}", expr),
                            },
                            Expression::Identifier(name) => {
                                self.store_bits_in_var(
                                    name, offset, false, length, skip, verts, heads,
                                )?;
                            }
                            Expression::Number(_) => self.check_bits_in_var(
                                value, offset, false, length, skip, verts, heads,
                            )?,
                            _ => unimplemented!("{:?}", value),
                        }
                    }
                }

                pos += expr_count;
            }
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments, clippy::ptr_arg)]
    fn store_bits_in_var(
        &self,
        name: &str,
        offset: i64,
        complement: bool,
        length: i64,
        skip: i64,
        verts: &mut Vec<Vertex>,
        heads: &mut Vec<BuilderPos>,
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

        for head in heads {
            let expr = expr.clone();

            if head.is_set(name, mask) {
                verts[head.head].actions.push(Action::AssertEq {
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
                verts[head.head].actions.push(Action::AssertEq {
                    left: def.as_ref().clone(),
                    right: expr,
                });
            } else {
                let expr = if head.is_any_set(name) {
                    Expression::BitwiseOr(
                        Box::new(Expression::Identifier(name.to_owned())),
                        Box::new(expr),
                    )
                } else {
                    expr
                };

                head.set(name, mask);

                verts[head.head].actions.push(Action::Set {
                    var: name.to_owned(),
                    expr,
                });
            }
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments, clippy::ptr_arg)]
    fn check_bits_in_var(
        &self,
        value: &Expression,
        offset: i64,
        complement: bool,
        length: i64,
        skip: i64,
        verts: &mut Vec<Vertex>,
        heads: &mut Vec<BuilderPos>,
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

        for head in heads {
            let expr = expr.clone();

            verts[head.head].actions.push(Action::AssertEq {
                left: expr,
                right: value.clone(),
            });
        }

        Ok(())
    }

    fn bit_field(
        &self,
        length: i64,
        verts: &mut Vec<Vertex>,
        heads: &mut Vec<BuilderPos>,
        bit_spec: &[Expression],
    ) -> Result<(), String> {
        // TODO: special casing when length == 1
        for head in heads {
            head.seen_edges = true;

            let before = head.head;

            let entry = verts.len();

            verts.push(Vertex::new());

            let next = verts.len();

            verts.push(Vertex::new());

            let done = verts.len();

            verts.push(Vertex::new());

            verts[head.head].edges.push(Edge::Branch(entry));

            head.head = entry;

            for (bit, e) in bit_spec.iter().enumerate() {
                let mut new_heads = vec![head.clone()];

                self.expression(e, verts, &mut new_heads, bit_spec)?;

                for head in new_heads {
                    verts[head.head].actions.push(Action::Set {
                        var: String::from("$v"),
                        expr: Expression::Number(bit as i64),
                    });

                    verts[head.head].edges.push(Edge::Branch(next));
                }
            }

            if !self.general_spec.lsb {
                verts[before].actions.push(Action::Set {
                    var: String::from("$b"),
                    expr: Expression::Number(length),
                });

                verts[before].actions.push(Action::Set {
                    var: String::from("$bits"),
                    expr: Expression::Number(0),
                });

                verts[next] = Vertex {
                    actions: vec![
                        Action::Set {
                            var: String::from("$b"),
                            expr: Expression::Subtract(
                                Box::new(Expression::Identifier(String::from("$b"))),
                                Box::new(Expression::Number(1)),
                            ),
                        },
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
                    ],
                    edges: vec![Edge::BranchCond {
                        expr: Expression::More(
                            Box::new(Expression::Identifier(String::from("$b"))),
                            Box::new(Expression::Number(0)),
                        ),
                        yes: entry,
                        no: done,
                    }],
                };
            } else {
                verts[before].actions.push(Action::Set {
                    var: String::from("$b"),
                    expr: Expression::Number(0),
                });

                verts[before].actions.push(Action::Set {
                    var: String::from("$bits"),
                    expr: Expression::Number(0),
                });

                verts[next] = Vertex {
                    actions: vec![
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
                        Action::Set {
                            var: String::from("$b"),
                            expr: Expression::Add(
                                Box::new(Expression::Identifier(String::from("$b"))),
                                Box::new(Expression::Number(1)),
                            ),
                        },
                    ],
                    edges: vec![Edge::BranchCond {
                        expr: Expression::Less(
                            Box::new(Expression::Identifier(String::from("$b"))),
                            Box::new(Expression::Number(length)),
                        ),
                        yes: entry,
                        no: done,
                    }],
                };
            }

            head.head = done;
        }

        Ok(())
    }

    fn expression(
        &self,
        expr: &Expression,
        verts: &mut Vec<Vertex>,
        heads: &mut Vec<BuilderPos>,
        bit_spec: &[Expression],
    ) -> Result<(), String> {
        match expr {
            Expression::Stream(irstream) => {
                let bit_spec = if irstream.bit_spec.is_empty() {
                    bit_spec
                } else {
                    &irstream.bit_spec
                };

                if irstream.repeat == Some(RepeatMarker::Any) {
                    let before_heads = heads.clone();

                    self.expression_list(&irstream.stream, verts, heads, bit_spec)?;

                    for mut head in before_heads.into_iter() {
                        let pos = verts.len();

                        verts.push(Vertex::new());

                        verts[head.head].edges.push(Edge::Branch(pos));

                        head.head = pos;

                        heads.push(head);
                    }
                } else {
                    self.expression_list(&irstream.stream, verts, heads, bit_spec)?;
                }
            }
            Expression::List(list) => {
                self.expression_list(list, verts, heads, bit_spec)?;
            }
            Expression::FlashConstant(v, u) => {
                for head in heads {
                    head.seen_edges = true;

                    let len = u.eval_float(*v, &self.general_spec)?;

                    let pos = verts.len();

                    verts.push(Vertex::new());

                    verts[head.head].edges.push(Edge::Flash(len, pos));

                    head.head = pos;
                }
            }
            Expression::GapConstant(v, u) => {
                for head in heads {
                    head.seen_edges = true;

                    let len = u.eval_float(*v, &self.general_spec)?;

                    let pos = verts.len();

                    verts.push(Vertex::new());

                    verts[head.head].edges.push(Edge::Gap(len, pos));

                    head.head = pos;
                }
            }
            Expression::BitField { length, .. } => {
                let (length, _) = length.eval(&Vartable::new())?;

                self.bit_field(length, verts, heads, bit_spec)?;
            }
            Expression::ExtentConstant(_, _) => {
                for head in heads {
                    head.seen_edges = true;

                    let pos = verts.len();

                    verts.push(Vertex::new());

                    verts[head.head].edges.push(Edge::TrailingGap(pos));

                    head.head = pos;
                }
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
#[derive(Clone, Default, Debug)]
struct BuilderPos {
    head: usize,
    seen_edges: bool,
    vars: HashMap<String, i64>,
}

#[allow(dead_code)]
impl BuilderPos {
    fn new() -> Self {
        Default::default()
    }

    fn set(&mut self, name: &str, fields: i64) {
        if let Some(e) = self.vars.get_mut(name) {
            *e |= 64;
        } else {
            self.vars.insert(name.to_owned(), fields);
        }
    }

    fn is_set(&self, name: &str, fields: i64) -> bool {
        if let Some(e) = self.vars.get(name) {
            (e & fields) != 0
        } else {
            false
        }
    }

    fn is_any_set(&self, name: &str) -> bool {
        if let Some(e) = self.vars.get(name) {
            *e != 0
        } else {
            false
        }
    }

    fn clear(&mut self) {
        self.vars.clear();
    }
}
