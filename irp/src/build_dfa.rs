use super::{
    build_nfa::{Action, Edge, Vertex, NFA},
    Expression, Irp,
};
use std::{collections::HashMap, hash::Hash, rc::Rc};

/// Deterministic finite automation for decoding IR. Using this we can match IR.
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Default)]
pub struct DFA {
    pub(crate) verts: Vec<Vertex>,
}

#[derive(Clone, Debug)]
struct Path {
    from: usize,
    to: usize,
    edge_no: usize,
}

#[derive(Hash, PartialEq, Eq)]

struct DfaEdge {
    from: usize,
    flash: bool,
    length: Option<Rc<Expression>>,
}

impl NFA {
    /// Build the DFA from the NFA
    pub fn build_dfa(&self) -> DFA {
        let mut builder = Builder {
            verts: Vec::new(),
            nfa_to_dfa: HashMap::new(),
            edges: HashMap::new(),
            nfa: self,
            visited: Vec::new(),
        };

        builder.build();

        DFA {
            verts: builder.verts,
        }
    }
}

impl DFA {
    /// Generate a GraphViz dot file and write to the given path
    pub fn dotgraphviz(&self, path: &str) {
        crate::graphviz::graphviz(&self.verts, "DFA", &[], path);
    }
}

impl Irp {
    /// Generate an DFA decoder state machine for this IRP
    pub fn compile(&self) -> Result<DFA, String> {
        let nfa = self.build_nfa()?;

        Ok(nfa.build_dfa())
    }
}

struct Builder<'a> {
    nfa: &'a NFA,
    nfa_to_dfa: HashMap<usize, usize>,
    edges: HashMap<DfaEdge, usize>,
    verts: Vec<Vertex>,
    visited: Vec<usize>,
}

impl<'a> Builder<'a> {
    fn build(&mut self) {
        assert_eq!(self.add_vertex(), 0);

        self.verts[0].actions = self.nfa.verts[0].actions.clone();

        self.nfa_to_dfa.insert(0, 0);

        self.add_path(true, 0);
    }

    /// Recursively add a new path
    fn add_path(&mut self, mut flash: bool, pos: usize) {
        self.visited.push(pos);

        let mut paths = Vec::new();

        self.conditional_closure(pos, Vec::new(), &mut paths);

        let mut next = Vec::new();

        for path in &paths {
            self.add_conditional_path_to_dfa(path);
            let last = path.last().unwrap();
            next.push(last.to);
        }

        for next in next {
            if !self.visited.contains(&next) {
                self.add_path(flash, next);
            }
        }

        let mut paths = Vec::new();

        self.input_closure(pos, flash, Vec::new(), &mut paths);

        let mut next = Vec::new();

        for path in &paths {
            self.add_input_path_to_dfa(flash, path);
            let last = path.last().unwrap();
            next.push(last.to);
        }

        flash = !flash;

        for next in next {
            if !self.visited.contains(&next) {
                self.add_path(flash, next);
            }
        }
    }

    fn add_conditional_path_to_dfa(&mut self, path: &[Path]) {
        for path in path {
            let from = self.nfa_to_dfa[&path.from];
            let to = self.copy_vert(path.to);

            for edge in &self.nfa.verts[path.from].edges {
                match edge {
                    Edge::Branch(dest) if *dest == path.to => {
                        let edge = Edge::Branch(to);

                        if !self.verts[from].edges.contains(&edge) {
                            self.verts[from].edges.push(edge);
                        }
                    }
                    Edge::BranchCond { expr, yes, no } => {
                        let yes = self.copy_vert(*yes);
                        let no = self.copy_vert(*no);
                        let expr = expr.clone();

                        let edge = Edge::BranchCond { expr, yes, no };

                        if !self.verts[from].edges.contains(&edge) {
                            self.verts[from].edges.push(edge);
                        }
                    }
                    _ => (),
                }
            }
        }
    }

    fn copy_vert(&mut self, original: usize) -> usize {
        if let Some(vert_no) = self.nfa_to_dfa.get(&original) {
            *vert_no
        } else {
            let vert_no = self.add_vertex();

            self.nfa_to_dfa.insert(original, vert_no);

            self.verts[vert_no].actions = self.nfa.verts[original].actions.clone();

            vert_no
        }
    }

    fn add_input_path_to_dfa(&mut self, flash: bool, path: &[Path]) {
        let from = self.nfa_to_dfa[&path[0].from];
        let nfa_to = path[path.len() - 1].to;
        let length = self.path_length(path);

        let dfa_edge = DfaEdge {
            from,
            flash,
            length: length.clone(),
        };

        if let Some(to) = self.edges.get(&dfa_edge) {
            self.nfa_to_dfa.insert(nfa_to, *to);
            // FIXME: check path matches
        } else {
            let to = if let Some(vert_no) = self.nfa_to_dfa.get(&nfa_to) {
                *vert_no
            } else {
                self.add_vertex()
            };

            self.nfa_to_dfa.insert(nfa_to, to);

            self.edges.insert(dfa_edge, to);

            self.verts[from].edges.push(if let Some(length) = length {
                if flash {
                    Edge::Flash {
                        length,
                        complete: true,
                        dest: to,
                    }
                } else {
                    Edge::Gap {
                        length,
                        complete: true,
                        dest: to,
                    }
                }
            } else {
                Edge::Branch(to)
            });

            let actions = self.path_actions(path);

            self.verts[to].actions = actions;
        }
    }

    fn path_length(&self, path: &[Path]) -> Option<Rc<Expression>> {
        let mut len: Option<Rc<Expression>> = None;

        for elem in path {
            match &self.nfa.verts[elem.from].edges[elem.edge_no] {
                Edge::Gap { length, .. } | Edge::Flash { length, .. } => {
                    if let Some(prev) = len {
                        if let (Expression::Number(left), Expression::Number(right)) =
                            (length.as_ref(), prev.as_ref())
                        {
                            // TODO: proper const folding
                            len = Some(Rc::new(Expression::Number(left + right)));
                        } else {
                            len = Some(Rc::new(Expression::Add(length.clone(), prev)));
                        }
                    } else {
                        len = Some(length.clone());
                    }
                }
                _ => (),
            }
        }

        len
    }

    fn path_actions(&self, path: &[Path]) -> Vec<Action> {
        let mut res: Vec<Action> = Vec::new();

        for elem in path {
            res.extend(self.nfa.verts[elem.to].actions.iter().cloned());
        }

        res
    }

    fn input_closure(
        &self,
        pos: usize,
        flash: bool,
        current_path: Vec<Path>,
        res: &mut Vec<Vec<Path>>,
    ) {
        for path in self.get_input_edges(pos, flash) {
            let mut p = current_path.clone();
            p.push(path);
            res.push(p);
        }

        for path in self.get_unconditional_edges(pos) {
            let mut p = current_path.clone();
            let pos = path.to;
            p.push(path);
            self.input_closure(pos, flash, p, res);
        }
    }

    fn get_unconditional_edges(&self, pos: usize) -> Vec<Path> {
        let mut res = Vec::new();
        for (i, edge) in self.nfa.verts[pos].edges.iter().enumerate() {
            match edge {
                Edge::Branch(dest) => {
                    res.push(Path {
                        to: *dest,
                        from: pos,
                        edge_no: i,
                    });
                }
                Edge::MayBranchCond { .. } | Edge::BranchCond { .. } => {
                    return Vec::new();
                }
                _ => (),
            }
        }
        res
    }

    fn get_input_edges(&self, pos: usize, flash: bool) -> Vec<Path> {
        let mut res = Vec::new();
        for (i, edge) in self.nfa.verts[pos].edges.iter().enumerate() {
            match edge {
                Edge::Flash { dest, .. } if flash => {
                    res.push(Path {
                        from: pos,
                        to: *dest,
                        edge_no: i,
                    });
                }
                Edge::Gap { dest, .. } if !flash => {
                    res.push(Path {
                        from: pos,
                        to: *dest,
                        edge_no: i,
                    });
                }
                _ => (),
            }
        }
        res
    }

    fn get_conditional_edges(&self, pos: usize) -> Vec<Path> {
        let mut res = Vec::new();
        for (i, edge) in self.nfa.verts[pos].edges.iter().enumerate() {
            match edge {
                Edge::MayBranchCond { dest, .. } => {
                    res.push(Path {
                        from: pos,
                        to: *dest,
                        edge_no: i,
                    });
                }
                Edge::BranchCond { yes, no, .. } => {
                    res.push(Path {
                        from: pos,
                        to: *yes,
                        edge_no: i,
                    });
                    res.push(Path {
                        from: pos,
                        to: *no,
                        edge_no: i,
                    });
                }
                _ => (),
            }
        }
        res
    }

    fn conditional_closure(&self, pos: usize, current_path: Vec<Path>, res: &mut Vec<Vec<Path>>) {
        for path in self.get_conditional_edges(pos) {
            let mut p = current_path.clone();
            p.push(path);
            res.push(p);
        }

        for path in self.get_unconditional_edges(pos) {
            let mut p = current_path.clone();
            let pos = path.to;
            p.push(path);
            self.conditional_closure(pos, p, res);
        }
    }

    fn add_vertex(&mut self) -> usize {
        let node = self.verts.len();

        self.verts.push(Vertex::default());

        node
    }
}
