use super::{
    build_nfa::{Action, Edge, NFA},
    Vartable,
};
use std::{char, fs::File, io::Write, path::PathBuf};

/// Generate a GraphViz dot file and write to the given path
pub fn graphviz(nfa: &NFA, states: &[(usize, Vartable)], path: &str) {
    let path = PathBuf::from(path);
    let mut file = File::create(path).expect("create file");

    writeln!(&mut file, "strict digraph NFA {{").unwrap();

    let mut vert_names = Vec::new();

    for (no, v) in nfa.verts.iter().enumerate() {
        let name = if v.edges.iter().any(|a| matches!(a, Edge::Done(_))) {
            format!("done ({})", no)
        } else {
            format!("{} ({})", no_to_name(vert_names.len()), no)
        };

        let mut labels: Vec<String> = v
            .actions
            .iter()
            .map(|a| match a {
                Action::Set { var, expr } => format!("{} = {}", var, expr),
                Action::Assert { var, expr } => format!("assert {} = {}", var, expr),
            })
            .collect::<Vec<String>>();

        if let Some(Edge::BranchCond { expr, .. }) = v
            .edges
            .iter()
            .find(|e| matches!(e, Edge::BranchCond { .. }))
        {
            labels.push(format!("cond: {}", expr));
        }

        let color = if let Some((_, vars)) = states.iter().find(|(node, _)| *node == no) {
            let values = vars
                .vars
                .iter()
                .map(|(name, (val, _, _))| format!("{}={}", name, val))
                .collect::<Vec<String>>();

            labels.push(format!("state: {}", values.join(", ")));

            " [color=red]"
        } else {
            ""
        };

        if !labels.is_empty() {
            writeln!(
                &mut file,
                "\t\"{}\" [label=\"{}\\n{}\"]{}",
                name,
                name,
                labels.join("\\n"),
                color
            )
            .unwrap();
        } else if !color.is_empty() {
            writeln!(&mut file, "\t\"{}\"{}", name, color).unwrap();
        }

        vert_names.push(name);
    }

    for (i, v) in nfa.verts.iter().enumerate() {
        for edge in &v.edges {
            match edge {
                Edge::Flash(len, dest) => writeln!(
                    &mut file,
                    "\t\"{}\" -> \"{}\" [label=\"flash {}μs\"]",
                    vert_names[i], vert_names[*dest], len
                )
                .unwrap(),
                Edge::Gap(len, dest) => writeln!(
                    &mut file,
                    "\t\"{}\" -> \"{}\" [label=\"gap {}μs\"]",
                    vert_names[i], vert_names[*dest], len
                )
                .unwrap(),
                Edge::BranchCond { yes, no, .. } => {
                    writeln!(
                        &mut file,
                        "\t\"{}\" -> \"{}\" [label=\"cond: true\"]",
                        vert_names[i], vert_names[*yes]
                    )
                    .unwrap();
                    //

                    writeln!(
                        &mut file,
                        "\t\"{}\" -> \"{}\" [label=\"cond: false\"]",
                        vert_names[i], vert_names[*no]
                    )
                    .unwrap();
                }
                Edge::Done(_) => (),
                Edge::Branch(dest) => writeln!(
                    &mut file,
                    "\t\"{}\" -> \"{}\"",
                    vert_names[i], vert_names[*dest]
                )
                .unwrap(),
            }
        }
    }

    writeln!(&mut file, "}}").unwrap();
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
