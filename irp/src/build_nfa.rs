use super::{Expression, Irp, RepeatMarker, Vartable};
use std::{char, collections::HashMap, fs::File, io::Write, path::Path};

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
pub enum Edge {
    Flash(i64, usize),
    Gap(i64, usize),
    BranchCond {
        expr: Expression,
        yes: usize,
        no: usize,
    },
    Branch(usize),
    Done,
}

#[derive(PartialEq, Debug, Clone)]
pub enum Action {
    Set { var: String, expr: Expression },
}

#[derive(PartialEq, Default, Clone, Debug)]
pub struct Vertex {
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
    pub verts: Vec<Vertex>,
}

impl Irp {
    /// Generate an NFA decoder for this IRP
    pub fn build_nfa(&self) -> Result<NFA, String> {
        let mut verts: Vec<Vertex> = vec![Vertex::new()];
        let mut last = 0;

        for expr in &self.stream {
            self.expression(expr, &mut verts, &mut last, &[])?;
        }

        verts[last].edges.push(Edge::Done);

        Ok(NFA { verts })
    }

    fn expression_list(
        &self,
        list: &[Expression],
        verts: &mut Vec<Vertex>,
        last: &mut usize,
        bit_spec: &[Expression],
    ) -> Result<(), String> {
        let mut pos = 0;
        let mut tracker = VarTracker::new();

        while pos < list.len() {
            let mut bit_count = 0;
            let mut expr_count = 0;

            while let Some(Expression::BitField { length, .. }) = list.get(pos + expr_count) {
                let (length, _) = length.eval(&Vartable::new())?;

                bit_count += length;
                expr_count += 1;
            }

            if expr_count == 0 {
                self.expression(&list[pos], verts, last, bit_spec)?;
                pos += 1;
            } else {
                self.bit_field(bit_count, verts, last, bit_spec)?;

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

                        match value.as_ref() {
                            Expression::Complement(expr) => match expr.as_ref() {
                                Expression::Identifier(name) => {
                                    self.store_bits_in_var(
                                        name,
                                        offset,
                                        true,
                                        length,
                                        skip,
                                        &mut verts[*last],
                                        &mut tracker,
                                    )?;
                                }
                                _ => unimplemented!(),
                            },
                            Expression::Identifier(name) => {
                                self.store_bits_in_var(
                                    name,
                                    offset,
                                    false,
                                    length,
                                    skip,
                                    &mut verts[*last],
                                    &mut tracker,
                                )?;
                            }
                            _ => unimplemented!(""),
                        }

                        offset -= length;
                    }
                }

                pos += expr_count;
            }
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn store_bits_in_var(
        &self,
        name: &str,
        offset: i64,
        complement: bool,
        length: i64,
        skip: &Option<Box<Expression>>,
        vert: &mut Vertex,
        tracker: &mut VarTracker,
    ) -> Result<(), String> {
        let skip = if let Some(skip) = skip {
            let (skip, _) = skip.eval(&Vartable::new())?;

            skip
        } else {
            0
        };

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

        let expr = if tracker.is_any_set(name) {
            Expression::BitwiseOr(
                Box::new(Expression::Identifier(name.to_owned())),
                Box::new(expr),
            )
        } else {
            expr
        };

        tracker.set(name, mask);

        vert.actions.push(Action::Set {
            var: name.to_owned(),
            expr: Expression::BitwiseAnd(Box::new(expr), Box::new(Expression::Number(mask))),
        });

        Ok(())
    }

    fn bit_field(
        &self,
        length: i64,
        verts: &mut Vec<Vertex>,
        last: &mut usize,
        bit_spec: &[Expression],
    ) -> Result<(), String> {
        let entry = verts.len();

        verts.push(Vertex::new());

        let next = verts.len();

        verts.push(Vertex::new());

        let done = verts.len();

        verts.push(Vertex::new());

        verts[*last].edges.push(Edge::Branch(entry));

        for (bit, e) in bit_spec.iter().enumerate() {
            let mut n = entry;

            self.expression(e, verts, &mut n, bit_spec)?;

            verts[n].actions.push(Action::Set {
                var: String::from("$v"),
                expr: Expression::Number(bit as i64),
            });

            verts[n].edges.push(Edge::Branch(next));
        }

        if !self.general_spec.lsb {
            verts[*last].actions.push(Action::Set {
                var: String::from("$b"),
                expr: Expression::Number(length),
            });

            verts[*last].actions.push(Action::Set {
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
            verts[*last].actions.push(Action::Set {
                var: String::from("$b"),
                expr: Expression::Number(0),
            });

            verts[*last].actions.push(Action::Set {
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

        *last = done;

        Ok(())
    }

    fn expression(
        &self,
        expr: &Expression,
        verts: &mut Vec<Vertex>,
        last: &mut usize,
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
                    let begin = *last;

                    self.expression_list(&irstream.stream, verts, last, bit_spec)?;

                    verts[begin].edges.push(Edge::Branch(*last));
                } else {
                    self.expression_list(&irstream.stream, verts, last, bit_spec)?;
                }
            }
            Expression::List(list) => {
                self.expression_list(list, verts, last, bit_spec)?;
            }
            Expression::FlashConstant(v, u) => {
                let len = u.eval_float(*v, &self.general_spec)?;

                let pos = verts.len();

                verts.push(Vertex::new());

                verts[*last].edges.push(Edge::Flash(len, pos));

                *last = pos;
            }
            Expression::GapConstant(v, u) => {
                let len = u.eval_float(*v, &self.general_spec)?;

                let pos = verts.len();

                verts.push(Vertex::new());

                verts[*last].edges.push(Edge::Gap(len, pos));

                *last = pos;
            }
            Expression::BitField { length, .. } => {
                let (length, _) = length.eval(&Vartable::new())?;

                self.bit_field(length, verts, last, bit_spec)?;
            }
            Expression::ExtentConstant(_, _) => {
                // should really check this is the last entry
            }
            _ => println!("expr:{:?}", expr),
        }

        Ok(())
    }
}

impl NFA {
    /// Generate a GraphViz dot file and write to the given path
    pub fn dotgraphviz(&self, path: &Path) {
        let mut file = File::create(path).expect("create file");

        writeln!(&mut file, "strict digraph NFA {{").unwrap();

        let mut vert_names = Vec::new();

        for (no, v) in self.verts.iter().enumerate() {
            let name = if v.edges.iter().any(|a| matches!(a, Edge::Done)) {
                format!("\"done ({})\"", no)
            } else {
                format!("\"{} ({})\"", no_to_name(vert_names.len()), no)
            };

            let mut labels: Vec<String> = v
                .actions
                .iter()
                .map(|a| match a {
                    Action::Set { var, expr } => format!("{} = {}", var, expr),
                })
                .collect::<Vec<String>>();

            if let Some(Edge::BranchCond { expr, .. }) = v
                .edges
                .iter()
                .find(|e| matches!(e, Edge::BranchCond { .. }))
            {
                labels.push(format!("cond: {}", expr));
            }

            if !labels.is_empty() {
                writeln!(&mut file, "\t{} [label=\"{}\"]", name, labels.join("\\n")).unwrap();
            }

            vert_names.push(name);
        }

        for (i, v) in self.verts.iter().enumerate() {
            for edge in &v.edges {
                match edge {
                    Edge::Flash(len, dest) => writeln!(
                        &mut file,
                        "\t{} -> {} [label=\"flash {}μs\"]",
                        vert_names[i], vert_names[*dest], len
                    )
                    .unwrap(),
                    Edge::Gap(len, dest) => writeln!(
                        &mut file,
                        "\t{} -> {} [label=\"gap {}μs\"]",
                        vert_names[i], vert_names[*dest], len
                    )
                    .unwrap(),
                    Edge::BranchCond { yes, no, .. } => {
                        writeln!(
                            &mut file,
                            "\t{} -> {} [label=\"cond: true\"]",
                            vert_names[i], vert_names[*yes]
                        )
                        .unwrap();
                        //

                        writeln!(
                            &mut file,
                            "\t{} -> {} [label=\"cond: false\"]",
                            vert_names[i], vert_names[*no]
                        )
                        .unwrap();
                    }
                    Edge::Done => (),
                    Edge::Branch(dest) => {
                        writeln!(&mut file, "\t{} -> {}", vert_names[i], vert_names[*dest]).unwrap()
                    }
                }
            }
        }

        writeln!(&mut file, "}}").unwrap();
    }
}

fn no_to_name(no: usize) -> String {
    let mut no = no;
    let mut res = String::new();

    loop {
        let ch = char::from_u32((65 + no % 26) as u32).unwrap();

        res.insert(0, ch);

        no /= 26;
        if no == 0 {
            return res;
        }
    }
}

fn gen_mask(v: i64) -> i64 {
    (1i64 << v) - 1
}

/// track which
struct VarTracker {
    vars: HashMap<String, i64>,
}

#[allow(dead_code)]
impl VarTracker {
    fn new() -> Self {
        VarTracker {
            vars: HashMap::new(),
        }
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
