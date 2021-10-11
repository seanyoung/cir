use std::fs::File;
use std::io::prelude::*;
use std::path::PathBuf;
use std::process::Command;

include!(concat!(env!("OUT_DIR"), "/output.rs"));

use output::{Node, Rule};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct TestData {
    pub protocol: String,
    pub params: Vec<Param>,
    #[serde(skip_serializing_if = "is_zero")]
    pub repeats: u8,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub pronto: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub render: Vec<Vec<u32>>,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero(num: &u8) -> bool {
    *num == 0
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
        pronto: String::new(),
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
                                value: param.children[2].as_str(input).parse().unwrap(),
                            });
                        }
                    }
                    Rule::render => {
                        if node.children[0].rule == Rule::pronto_out {
                            data.pronto = node.children[0].as_str(input).trim().to_owned();
                        } else {
                            for rawir in collect_rules(node, Rule::rawir) {
                                let mut res = Vec::new();

                                for rawir in collect_rules(rawir, Rule::value) {
                                    res.push(rawir.children[1].as_str(input).parse().unwrap());
                                }

                                data.render.push(res);
                            }
                        }
                    }
                    _ => unimplemented!(),
                }
            }

            walk(&node, input, &mut data);
        }
        Err(pos) => {
            panic!("cannot parse `{}` at  position {}:{}", input, pos.0, pos.1);
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

        let output = Command::new("irptransmogrifier.sh")
            .args(&["render", "--random", "-p", "-P", &protocol.name])
            .output()
            .expect("Failed to execute irptransmogrifier.sh");

        if !output.status.success() {
            continue;
        }

        let result = String::from_utf8(output.stdout).unwrap();

        let data = parse_output(protocol.name.clone(), 0, &result);

        test_data.push(data);
    }

    let test_data = serde_json::to_string(&test_data).unwrap();

    let mut file = File::create("../transmogrifier_test_data.json").unwrap();
    file.write_all(test_data.as_bytes()).unwrap();
}
