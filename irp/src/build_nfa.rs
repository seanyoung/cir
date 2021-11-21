use super::{Expression, Irp, Vartable};
use std::{char, fs::File, io::Write, path::Path};

// This is the decoder nfa (non-deterministic finite automation)
//
// From the IRP, we build the nfa
// from the nfa we build the dfa
// from the dfa we build clif
// from clif we the BPF decoder (cranelift does this)

// clif is a compiler IR. This means basic blocks with a single
// flow control instruction at the end of the block. So, we try to model
// the nfa such this is easy to transform.

#[derive(PartialEq, Debug)]
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

#[derive(PartialEq, Debug)]
pub enum Action {
    Set { var: String, expr: Expression },
}

#[derive(PartialEq, Default, Debug)]
pub struct Vertex {
    pub actions: Vec<Action>,
    pub edges: Vec<Edge>,
}

impl Vertex {
    fn new() -> Self {
        Default::default()
    }
}

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug)]
pub struct NFA {
    pub verts: Vec<Vertex>,
}

impl Irp {
    // Generate a decoder for this IRP
    pub fn build_nfa(&self) -> Result<NFA, String> {
        let mut verts: Vec<Vertex> = vec![Vertex::new()];
        let mut last = 0;

        for expr in &self.stream {
            self.expression(expr, &mut verts, &mut last, &[])?;
        }

        Ok(NFA { verts })
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
                for expr in &irstream.stream {
                    self.expression(expr, verts, last, &irstream.bit_spec)?;
                }
            }
            Expression::List(list) => {
                for expr in list {
                    self.expression(expr, verts, last, bit_spec)?;
                }
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

                let entry = verts.len();

                verts.push(Vertex {
                    edges: vec![],
                    actions: vec![],
                });

                let next = verts.len();

                verts.push(Vertex::new());

                let done = verts.len();

                verts.push(Vertex {
                    edges: vec![Edge::Done],
                    actions: vec![],
                });

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
                String::from("done")
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
