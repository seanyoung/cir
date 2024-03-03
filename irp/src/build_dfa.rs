use super::{
    build_nfa::{Action, Edge, Vertex, NFA},
    Expression, Irp,
};
use std::{collections::HashMap, hash::Hash, rc::Rc};

/// Deterministic finite automation for decoding IR. Using this we can match IR.
#[derive(Debug, Default)]
#[allow(clippy::upper_case_acronyms)]
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

        self.verts[0].entry = self.nfa.verts[0].entry.clone();

        self.nfa_to_dfa.insert(0, 0);

        self.add_path(true, 0);
    }

    // Recursively add a new path
    fn add_path(&mut self, mut flash: bool, pos: usize) {
        self.visited.push(pos);

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

            let actions = self.path_actions(path, flash, length);

            self.verts[from].edges.push(Edge { dest: to, actions });
        }
    }

    fn path_length(&self, path: &[Path]) -> Option<Rc<Expression>> {
        let mut len: Option<Rc<Expression>> = None;

        for elem in path {
            for action in &self.nfa.verts[elem.from].edges[elem.edge_no].actions {
                match action {
                    Action::Gap { length, .. } | Action::Flash { length, .. } => {
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
        }

        len
    }

    fn path_actions(
        &self,
        path: &[Path],
        flash: bool,
        length: Option<Rc<Expression>>,
    ) -> Vec<Action> {
        let mut res: Vec<Action> = Vec::new();

        if let Some(length) = length {
            res.push(if flash {
                Action::Flash {
                    length,
                    complete: true,
                }
            } else {
                Action::Gap {
                    length,
                    complete: true,
                }
            });
        }

        for elem in path {
            res.extend(self.nfa.verts[elem.to].entry.iter().cloned());
            res.extend(
                self.nfa.verts[elem.from].edges[elem.edge_no]
                    .actions
                    .iter()
                    .filter(|action| !matches!(action, Action::Flash { .. } | Action::Gap { .. }))
                    .cloned(),
            );
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
            let pos = path.to;
            if current_path.iter().all(|p| p.from != pos) {
                p.push(path);
                res.push(p.clone());
                self.input_closure(pos, flash, p, res);
            }
        }

        for path in self.get_non_input_edges(pos) {
            let mut p = current_path.clone();
            let pos = path.to;
            if current_path.iter().all(|p| p.from != pos) {
                p.push(path);
                self.input_closure(pos, flash, p, res);
            }
        }
    }

    fn get_input_edges(&self, pos: usize, flash: bool) -> Vec<Path> {
        let mut res = Vec::new();
        for (edge_no, edge) in self.nfa.verts[pos].edges.iter().enumerate() {
            for action in &edge.actions {
                match action {
                    Action::Flash { .. } if flash => {
                        res.push(Path {
                            from: pos,
                            to: edge.dest,
                            edge_no,
                        });
                        break;
                    }
                    Action::Gap { .. } if !flash => {
                        res.push(Path {
                            from: pos,
                            to: edge.dest,
                            edge_no,
                        });
                        break;
                    }
                    _ => (),
                }
            }
        }
        res
    }

    fn get_non_input_edges(&self, pos: usize) -> Vec<Path> {
        let mut res = Vec::new();
        for (i, edge) in self.nfa.verts[pos].edges.iter().enumerate() {
            if !edge
                .actions
                .iter()
                .any(|action| matches!(action, Action::Flash { .. } | Action::Gap { .. }))
            {
                res.push(Path {
                    from: pos,
                    to: edge.dest,
                    edge_no: i,
                });
            }
        }

        res
    }

    fn add_vertex(&mut self) -> usize {
        let node = self.verts.len();

        self.verts.push(Vertex::default());

        node
    }
}
