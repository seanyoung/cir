use super::{Expression, Irp, Vartable};
#[allow(unused_imports)]
use std::{
    char,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};

// This is the decoder nfa (non-deterministic finite automation)
#[derive(PartialEq, Debug)]
pub enum Edge {
    Flash(i64, usize),
    Gap(i64, usize),
    Repeat(usize),
    Empty(usize),
}

#[derive(PartialEq, Debug)]
pub enum Action {
    Set {
        var: String,
        expr: Expression,
    },
    AddBit {
        var: String,
        expr: Expression,
        count: u8,
        lsb: bool,
    },
    Done,
}

#[derive(PartialEq, Debug)]
pub struct Vertex {
    pub edges: Vec<Edge>,
    pub actions: Vec<Action>,
}

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug)]
pub struct NFA {
    pub verts: Vec<Vertex>,
}

impl Vertex {
    fn new() -> Self {
        Vertex {
            edges: vec![],
            actions: vec![],
        }
    }

    pub fn get_repeat_edge(&self) -> usize {
        for e in &self.edges {
            if let Edge::Repeat(dest) = e {
                return *dest;
            }
        }

        panic!("no repeat edge found");
    }

    pub fn get_empty_edge(&self) -> Option<usize> {
        for e in &self.edges {
            if let Edge::Empty(dest) = e {
                return Some(*dest);
            }
        }

        None
    }
}

impl Irp {
    // Generate a decoder for this IRP
    pub fn build_nfa(&self) -> Result<NFA, String> {
        let mut verts: Vec<Vertex> = vec![Vertex::new()];
        let mut last = 0;

        for expr in &self.stream {
            self.expression(expr, &mut verts, &mut last, &[])?;
        }

        let pos = verts.len();

        verts.push(Vertex {
            edges: vec![],
            actions: vec![Action::Done],
        });

        verts[last].edges.push(Edge::Empty(pos));

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
                let start = *last;

                let (length, _) = length.eval(&Vartable::new())?;

                let end = verts.len();

                verts.push(Vertex {
                    actions: vec![Action::AddBit {
                        var: String::from("F"),
                        expr: Expression::Identifier(String::from("v")),
                        lsb: self.general_spec.lsb,
                        count: length as u8,
                    }],
                    edges: vec![Edge::Repeat(start)],
                });

                for (bit, e) in bit_spec.iter().enumerate() {
                    let mut n = start;

                    self.expression(e, verts, &mut n, bit_spec)?;

                    verts[n].actions.push(Action::Set {
                        var: String::from("v"),
                        expr: Expression::Number(bit as i64),
                    });

                    verts[n].edges.push(Edge::Empty(end));
                }

                *last = end;
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
            let name = if v.actions.iter().any(|a| matches!(a, Action::Done)) {
                String::from("done")
            } else {
                format!("\"{} ({})\"", no_to_name(vert_names.len()), no)
            };

            let labels: Vec<String> = v
                .actions
                .iter()
                .filter_map(|a| match a {
                    Action::Set { var, expr } => Some(format!("{} = {}", var, expr)),
                    Action::AddBit {
                        var,
                        count,
                        expr,
                        lsb,
                    } => Some(format!(
                        "bit {} = {} count:{} lsb:{}",
                        var, expr, count, lsb
                    )),
                    Action::Done => None,
                })
                .collect::<Vec<String>>();

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
                    Edge::Repeat(dest) => writeln!(
                        &mut file,
                        "\t{} -> {} [label=repeat]",
                        vert_names[i], vert_names[*dest]
                    )
                    .unwrap(),
                    Edge::Empty(dest) => {
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
