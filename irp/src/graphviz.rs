use super::{
    build_nfa::{Action, Vertex},
    Vartable,
};
use itertools::Itertools;
use log::error;
use std::{
    char,
    fs::File,
    io::{BufWriter, Write},
    path::PathBuf,
};

/// Generate a GraphViz dot file and write to the given path
pub(crate) fn graphviz(verts: &[Vertex], name: &str, states: &[(usize, Vartable)], path: &str) {
    let path = PathBuf::from(path);
    let file = match File::create(&path) {
        Ok(file) => file,
        Err(e) => {
            error!("unable to write file '{}': {e}", path.display());
            return;
        }
    };

    let mut file = BufWriter::new(file);

    writeln!(&mut file, "strict digraph {name} {{").unwrap();

    let mut vert_names = Vec::new();

    for (no, v) in verts.iter().enumerate() {
        let name = if v.entry.iter().any(|a| matches!(a, Action::Done(..))) {
            format!("done ({no})")
        } else {
            format!("{} ({})", no_to_name(vert_names.len()), no)
        };

        let mut labels = actions(&v.entry);

        let color = if let Some((_, vars)) = states.iter().find(|(node, _)| *node == no) {
            let values = vars
                .vars
                .iter()
                .map(|(name, (val, _))| format!("{name}={val}"))
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
            writeln!(&mut file, "\t\"{name}\"{color}").unwrap();
        }

        vert_names.push(name);
    }

    for (i, v) in verts.iter().enumerate() {
        for edge in &v.edges {
            let labels = actions(&edge.actions);

            if !labels.is_empty() {
                writeln!(
                    &mut file,
                    "\t\"{}\" -> \"{}\" [label=\"{}\"]",
                    vert_names[i],
                    vert_names[edge.dest],
                    labels.join("\\n"),
                )
                .unwrap();
            } else {
                writeln!(
                    &mut file,
                    "\t\"{}\" -> \"{}\"",
                    vert_names[i], vert_names[edge.dest]
                )
                .unwrap();
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

fn actions(actions: &[Action]) -> Vec<String> {
    actions
        .iter()
        .map(|a| match a {
            Action::Flash { length, complete } => {
                format!("flash {length} {}", if *complete { "complete" } else { "" })
            }
            Action::Gap { length, complete } => {
                format!("gap {length} {}", if *complete { "complete" } else { "" })
            }
            Action::Set { var, expr } => format!("{var} = {expr}"),
            Action::AssertEq { left, right } => format!("assert {left} = {right}",),
            Action::Done(event, res) => format!("{} ({})", event, res.iter().join(", ")),
        })
        .collect::<Vec<String>>()
}
