use super::{
    build_nfa::{Action, Edge, Length, Vertex, NFA},
    expression::clone_filter,
    Expression, Irp,
};
use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    rc::Rc,
};

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
    length: Length,
    actions: Vec<Action>,
}

impl NFA {
    /// Build the DFA from the NFA
    pub fn build_dfa(&self, aeps: u32, eps: u32) -> DFA {
        let mut builder = Builder {
            verts: Vec::new(),
            nfa_to_dfa: HashMap::new(),
            edges: HashMap::new(),
            nfa: self,
            visited: HashSet::new(),
            aeps,
            eps,
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
    pub fn compile(&self, aeps: u32, eps: u32) -> Result<DFA, String> {
        let nfa = self.build_nfa()?;

        Ok(nfa.build_dfa(aeps, eps))
    }
}

struct Builder<'a> {
    nfa: &'a NFA,
    nfa_to_dfa: HashMap<usize, usize>,
    edges: HashMap<DfaEdge, usize>,
    verts: Vec<Vertex>,
    visited: HashSet<usize>,
    aeps: u32,
    eps: u32,
}

impl<'a> Builder<'a> {
    fn build(&mut self) {
        assert_eq!(self.add_vertex(), 0);

        self.verts[0].entry = self.nfa.verts[0].entry.clone();

        self.nfa_to_dfa.insert(0, 0);

        self.recurse_nfa_path(true, vec![0]);
    }

    // Recursively add a new path
    fn recurse_nfa_path(&mut self, flash: bool, pos: Vec<usize>) {
        struct UniqueEdge {
            length: Length,
            actions: Vec<Action>,
            nfa_edges: Vec<(usize, usize)>,
        }

        let mut paths = Vec::new();

        for pos in pos {
            if !self.visited.contains(&pos) {
                self.input_closure(pos, flash, Vec::new(), &mut paths);
                self.visited.insert(pos);
            }
        }

        let mut edges: Vec<UniqueEdge> = Vec::new();

        for path in &paths {
            let length = self.path_length(path);
            let actions = self.path_actions(path);
            let from = path[0].from;
            let to = path.last().unwrap().to;

            // If an edge overlaps with an existing edge, merge it
            if let Some(e) = edges
                .iter_mut()
                .find(|e| e.length.overlaps(&length) && e.actions == actions)
            {
                e.length = e.length.merge(&length);
                e.nfa_edges.push((from, to));
            } else {
                edges.push(UniqueEdge {
                    length,
                    actions,
                    nfa_edges: vec![(from, to)],
                });
            }
        }

        for UniqueEdge {
            length,
            actions,
            nfa_edges,
        } in edges
        {
            let pos = nfa_edges.iter().map(|(_, nfa_to)| *nfa_to).collect();
            self.add_path_to_dfa(flash, actions, length, nfa_edges);

            self.recurse_nfa_path(!flash, pos);
        }
    }

    fn add_path_to_dfa(
        &mut self,
        flash: bool,
        mut actions: Vec<Action>,
        length: Length,
        nfa_edges: Vec<(usize, usize)>,
    ) {
        let from = self.nfa_to_dfa[&nfa_edges[0].0];

        actions.insert(
            0,
            if flash {
                Action::Flash {
                    length: length.clone(),
                    complete: true,
                }
            } else {
                Action::Gap {
                    length: length.clone(),
                    complete: true,
                }
            },
        );

        let dfa_edge = DfaEdge {
            from,
            flash,
            length,
            actions: actions.clone(),
        };

        if let Some(to) = self.edges.get(&dfa_edge) {
            if self.verts[from]
                .edges
                .iter()
                .any(|edge| edge.dest == *to && edge.actions == actions)
            {
                for (nfa_from, nfa_to) in nfa_edges {
                    self.nfa_to_dfa.insert(nfa_from, from);
                    self.nfa_to_dfa.insert(nfa_to, *to);
                }
                return;
            }
        }

        let to = if let Some(vert_no) = self.nfa_to_dfa.get(&nfa_edges[0].1) {
            *vert_no
        } else {
            self.add_vertex()
        };

        for (nfa_from, nfa_to) in nfa_edges {
            self.nfa_to_dfa.insert(nfa_from, from);
            self.nfa_to_dfa.insert(nfa_to, to);
        }

        self.edges.insert(dfa_edge, to);

        self.verts[from].edges.push(Edge { dest: to, actions });
    }

    fn path_length(&self, path: &[Path]) -> Length {
        let mut len: Option<Rc<Expression>> = None;

        let mut vars: HashMap<&str, Rc<Expression>> = HashMap::new();

        for elem in path {
            for action in self.nfa.verts[elem.from].edges[elem.edge_no]
                .actions
                .iter()
                .chain(&self.nfa.verts[elem.to].entry)
            {
                match action {
                    Action::Gap { length, .. } | Action::Flash { length, .. } => {
                        let length = match length {
                            Length::Expression(expr) => expr,
                            Length::Range(..) => unreachable!(),
                        };

                        let length = replace_vars(length, &vars);

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
                    Action::Set { var, expr } => {
                        let expr = replace_vars(expr, &vars);

                        vars.insert(var, expr);
                    }
                    _ => (),
                }
            }
        }

        let length = len.unwrap();

        if let Expression::Number(length) = length.as_ref() {
            let length = *length as u32;
            let min = std::cmp::min(
                length.saturating_sub(self.aeps),
                (length * (100 - self.eps)) / 100,
            );

            let max = std::cmp::min(length + self.aeps, (length * (100 + self.eps)) / 100);

            Length::Range(min, max)
        } else {
            Length::Expression(length)
        }
    }

    fn path_actions(&self, path: &[Path]) -> Vec<Action> {
        let mut res: Vec<Action> = Vec::new();

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

fn replace_vars(expr: &Rc<Expression>, vars: &HashMap<&str, Rc<Expression>>) -> Rc<Expression> {
    clone_filter(expr, &|e| {
        if let Expression::Identifier(id) = e.as_ref() {
            if let Some(expr) = vars.get(&id.as_str()) {
                return Some(expr.clone());
            }
        }
        None
    })
    .unwrap_or(expr.clone())
}

impl Length {
    fn overlaps(&self, other: &Self) -> bool {
        if let (Length::Range(min1, max1), Length::Range(min2, max2)) = (self, other) {
            !(max1 < min2 || max2 < min1)
        } else {
            false
        }
    }

    fn merge(&self, other: &Self) -> Length {
        debug_assert!(self.overlaps(other));

        if let (Length::Range(min1, max1), Length::Range(min2, max2)) = (self, other) {
            Length::Range(std::cmp::min(*min1, *min2), std::cmp::max(*max1, *max2))
        } else {
            unreachable!();
        }
    }
}

#[test]
fn overlaps() {
    assert!(!Length::Range(1, 10).overlaps(&Length::Range(11, 20)));
    assert!(!Length::Range(11, 20).overlaps(&Length::Range(1, 10)));

    assert!(Length::Range(1, 11).overlaps(&Length::Range(11, 20)));
    assert!(Length::Range(11, 20).overlaps(&Length::Range(1, 11)));

    assert!(Length::Range(11, 20).overlaps(&Length::Range(11, 20)));
    assert!(Length::Range(5, 25).overlaps(&Length::Range(11, 20)));
    assert!(Length::Range(11, 20).overlaps(&Length::Range(5, 25)));
}
