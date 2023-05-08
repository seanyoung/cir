use super::build_nfa::{Action, Edge, Vertex, NFA};
use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
};

/// Non-deterministic finite automation for decoding IR. Using this we can
/// match IR and hopefully, one day, create the dfa (deterministic finite
/// automation).
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
    length: i64,
}

impl NFA {
    /// Build the DFA from the NFA
    pub fn build_dfa(&self) -> DFA {
        let mut builder = Builder {
            verts: Vec::new(),
            nfa_to_dfa: HashMap::new(),
            edges: HashMap::new(),
            nfa: self,
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

struct Builder<'a> {
    nfa: &'a NFA,
    nfa_to_dfa: HashMap<usize, usize>,
    edges: HashMap<DfaEdge, usize>,
    verts: Vec<Vertex>,
}

impl<'a> Builder<'a> {
    fn build(&mut self) {
        let mut visited: HashSet<usize> = HashSet::new();

        let mut pos = vec![0];
        let mut flash = true;

        assert_eq!(self.add_vertex(), 0);

        self.nfa_to_dfa.insert(0, 0);

        while !pos.is_empty() {
            visited.extend(&pos);
            let next = self.closure_set(pos, flash);

            for path in &next {
                self.add_path(flash, path);
            }

            flash = !flash;

            pos = Vec::new();
            for path in next {
                let n = path.last().unwrap();
                if !visited.contains(&n.to) {
                    pos.push(n.to);
                }
            }
        }
    }

    fn add_path(&mut self, flash: bool, path: &[Path]) {
        let from = self.nfa_to_dfa[&path[0].from];
        let nfa_to = path[path.len() - 1].to;
        let length = self.path_length(path);

        #[allow(clippy::map_entry)]
        if !self.nfa_to_dfa.contains_key(&nfa_to) {
            if let Some(to) = self.edges.get(&DfaEdge {
                from,
                flash,
                length,
            }) {
                self.nfa_to_dfa.insert(nfa_to, *to);
                // FIXME: check path matches
            } else {
                let to = self.add_vertex();
                self.nfa_to_dfa.insert(nfa_to, to);

                self.edges.insert(
                    DfaEdge {
                        flash,
                        length,
                        from,
                    },
                    to,
                );

                self.verts[from].edges.push(if flash {
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
                });

                let actions = self.path_actions(path);

                self.verts[to].actions = actions;
            }
        }
    }

    fn path_length(&self, path: &[Path]) -> i64 {
        let mut len = 0;

        for elem in path {
            match self.nfa.verts[elem.from].edges[elem.edge_no] {
                Edge::Gap { length, .. } | Edge::Flash { length, .. } => {
                    len += length;
                }
                //Edge::FlashVar { .. } | Edge::GapVar { .. } => unimplemented!(),
                _ => (),
            }
        }

        len
    }

    fn path_actions(&self, path: &[Path]) -> Vec<Action> {
        let mut res: Vec<Action> = Vec::new();

        for elem in path {
            res.extend(self.nfa.verts[elem.from].actions.iter().cloned());
        }

        res
    }

    fn closure_set(&self, pos: Vec<usize>, flash: bool) -> Vec<Vec<Path>> {
        let mut res = Vec::new();

        for pos in pos {
            self.closure(pos, flash, Vec::new(), &mut res);
        }

        res
    }

    fn closure(&self, pos: usize, flash: bool, current_path: Vec<Path>, res: &mut Vec<Vec<Path>>) {
        for path in self.get_edges(pos, flash) {
            let mut p = current_path.clone();
            p.push(path);
            res.push(p);
        }

        for path in self.get_empty_edges(pos) {
            let mut p = current_path.clone();
            let pos = path.to;
            p.push(path);
            self.closure(pos, flash, p, res);
        }
    }

    fn get_empty_edges(&self, pos: usize) -> Vec<Path> {
        let mut res = Vec::new();
        for (i, edge) in self.nfa.verts[pos].edges.iter().enumerate() {
            match edge {
                Edge::Branch(dest) | Edge::MayBranchCond { dest, .. } => {
                    res.push(Path {
                        to: *dest,
                        from: pos,
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

    fn get_edges(&self, pos: usize, flash: bool) -> Vec<Path> {
        let mut res = Vec::new();
        for (i, edge) in self.nfa.verts[pos].edges.iter().enumerate() {
            match edge {
                Edge::Flash { dest, .. } | Edge::FlashVar { dest, .. } if flash => {
                    res.push(Path {
                        from: pos,
                        to: *dest,
                        edge_no: i,
                    });
                }
                Edge::Gap { dest, .. } | Edge::GapVar { dest, .. } if !flash => {
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

    fn add_vertex(&mut self) -> usize {
        let node = self.verts.len();

        self.verts.push(Vertex::default());

        node
    }
}
