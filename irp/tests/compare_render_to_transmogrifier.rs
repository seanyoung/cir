use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
pub struct TestData {
    pub protocol: String,
    pub repeats: i64,
    pub params: Vec<Param>,
    pub render: Vec<Vec<u32>>,
}

#[derive(Serialize, Deserialize)]
pub struct Param {
    pub name: String,
    pub value: u64,
}

use irp::protocols;
use irp::render::{render, Vartable};

#[test]
fn go() {
    // load test data
    let data = std::fs::read_to_string("transmogrifier_test_data.json").unwrap();

    let testdata: Vec<TestData> = serde_json::from_str(&data).unwrap();
    let protocols = protocols::read_protocols(&PathBuf::from("IrpProtocols.xml"));

    let mut fails = 0;
    let total_tests = testdata.len();

    for test in testdata {
        let protocol = protocols.iter().find(|p| p.name == test.protocol).unwrap();

        let mut vars = Vartable::new();

        println!("testing {} irp {}", protocol.name, protocol.irp);

        println!("repeats {}", test.repeats);

        if test.repeats != 0 {
            continue;
        }

        for param in test.params {
            println!("{} = {}", param.name, param.value);

            vars.set(param.name, param.value as i64, 8);
        }

        let (_, f) = render(&protocol.irp, vars, test.repeats).unwrap();

        if compare_with_rounding(test.render[0].clone(), f) {
            println!("OK");
        } else {
            println!("FAIL");
            fails += 1;
        }
    }

    println!("tests: {} fails: {}", total_tests, fails);
}

fn compare_with_rounding(l: Vec<u32>, r: Vec<u32>) -> bool {
    if l == r {
        return true;
    }

    let mut success = true;

    if l.len() != r.len() {
        println!(
            "comparing:\n{:?} with\n{:?}\n have different lengths {} and {}",
            l,
            r,
            l.len(),
            r.len()
        );

        success = false;
    }

    for i in 0..std::cmp::min(l.len(), r.len()) {
        let diff = if l[i] > r[i] {
            l[i] - r[i]
        } else {
            r[i] - l[i]
        };
        if diff > 8 {
            println!(
                        "comparing:\nleft:{:?} with\nright:{:?}\nfailed at position {} out of {} found {} expected {}",
                        l,
                        r,
                        i,
                        l.len(),
                        l[i], r[i]
                    );

            success = false;
        }
    }

    success
}
