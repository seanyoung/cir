use std::fs::File;
use std::io::prelude::*;
use std::path::PathBuf;
use std::process::Command;

mod output;

use output::{Node, Rule};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct TestData {
    pub protocol: String,
    pub params: Vec<Param>,
    pub repeats: u8,
    pub render: Vec<Vec<u32>>,
}

#[derive(Serialize, Deserialize)]
pub struct Param {
    pub name: String,
    pub value: u64,
}

pub fn parse_output(protocol: String, repeats: u8, input: &str) -> TestData {
    let mut parser = output::PEG::new();

    let mut data = TestData {
        protocol,
        repeats,
        params: Vec::new(),
        render: Vec::new(),
    };

    match parser.parse(input) {
        Ok(node) => {
            fn walk(node: &Node, input: &str, data: &mut TestData) {
                match node.rule {
                    Rule::output => {
                        walk(&node.children[0], input, data);
                        walk(&node.children[1], input, data);
                    }
                    Rule::params => {
                        for param in collect_rules(node, Rule::param) {
                            data.params.push(Param {
                                name: param.children[0].as_str(input).to_owned(),
                                value: u64::from_str_radix(param.children[2].as_str(input), 10)
                                    .unwrap(),
                            });
                        }
                    }
                    Rule::render => {
                        for rawir in collect_rules(node, Rule::rawir) {
                            let mut res = Vec::new();

                            for rawir in collect_rules(rawir, Rule::value) {
                                res.push(
                                    u32::from_str_radix(&rawir.children[1].as_str(input), 10)
                                        .unwrap(),
                                );
                            }

                            data.render.push(res);
                        }
                    }
                    _ => unimplemented!(),
                }
            }

            walk(&node, input, &mut data);
        }
        Err(pos) => {
            panic!("cannot parse `{}` at  position {}", input, pos);
        }
    }

    data
}

fn collect_rules(node: &Node, rule: Rule) -> Vec<&Node> {
    let mut list = Vec::new();

    fn recurse<'t>(node: &'t Node, rule: Rule, list: &mut Vec<&'t Node>) {
        if node.rule == rule {
            list.push(node);
        }

        for node in &node.children {
            recurse(node, rule, list);
        }
    }

    recurse(node, rule, &mut list);

    list
}

fn main() {
    let protocols = irp::protocols::read_protocols(&PathBuf::from("../IrpProtocols.xml"));
    let mut test_data = Vec::new();

    for protocol in protocols {
        for n in 0..10 {
            let repeats = if n < 3 {
                n
            } else {
                (rand::random::<u8>() % 20) + n
            };
            let number_repeats = format!("{}", repeats);
            let output = Command::new("irptransmogrifier.sh")
                .args(&[
                    "render",
                    "--random",
                    "-r",
                    "--number-repeats",
                    &number_repeats,
                    "-P",
                    &protocol.name,
                ])
                .output()
                .expect("Failed to execute irptransmogrifier.sh");

            if !output.status.success() {
                continue;
            }

            let result = String::from_utf8(output.stdout).unwrap();

            let data = parse_output(protocol.name.clone(), repeats, &result);

            test_data.push(data);
        }
    }

    let test_data = serde_json::to_string(&test_data).unwrap();

    let mut file = File::create("../transmogrifier_test_data.json").unwrap();
    file.write_all(test_data.as_bytes()).unwrap();
}
