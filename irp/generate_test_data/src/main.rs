use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::prelude::*;
use std::path::PathBuf;
use std::process::Command;

enum Render {
    Pronto(String),
    Rawir(Vec<Vec<u32>>),
}

peg::parser! {
    grammar output() for str {
        pub(super) rule output() -> (Vec<Param>, Render)
        = p:params() r:render() _ { (p, r) }

        rule params() -> Vec<Param>
        = "{" params:(param() ++ ",") "}" _ { params }

        rule param() -> Param
        = id:identifier() _ "=" _ value:number() _ {  Param { name: id.to_owned(), value } }

        rule render() -> Render
        = p:pronto_out() { Render::Pronto(p.to_owned()) }
        / r:rawir_out() { Render::Rawir(r) }

        rule pronto_out() -> &'input str
        = $(hex() ++ _)

        rule rawir_out() -> (Vec<Vec<u32>>)
        = frequency()? raw:rawir()+ { raw }

        rule frequency() -> u64
        = "Freq=" n:number() "Hz" { n }

        rule rawir() -> Vec<u32>
        = "[" values:(value() ** ",") "]" { values }

        rule value() -> u32
        = ("+" / "-") n:$(['0'..='9']+)
        {? match n.parse() { Ok(n) => Ok(n), Err(_) => Err("u32")} }

        rule hex()
        = ['0'..='9' | 'a'..='f' | 'A'..='F']+

        rule number() -> u64
        =  n:$(['0'..='9']+)
        {? match n.parse() { Ok(n) => Ok(n), Err(_) => Err("u64")} }

        rule identifier() -> &'input str
        = quiet!{$([ 'a'..='z' | 'A'..='Z']['a'..='z' | 'A'..='Z' | '0'..='9' ]*)}
        / expected!("identifier")

        rule _ = quiet!{[' ' | '\t' | '\r' | '\n']*}

    }
}

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
    let mut data = TestData {
        protocol,
        repeats,
        params: Vec::new(),
        pronto: String::new(),
        render: Vec::new(),
    };

    match output::output(input) {
        Ok((params, render)) => {
            data.params = params;

            match render {
                Render::Pronto(pronto) => data.pronto = pronto,
                Render::Rawir(raw) => data.render = raw,
            }
        }
        Err(pos) => {
            panic!("cannot parse `{}` at  position {}", input, pos);
        }
    }

    data
}

fn main() {
    let protocols = irp::protocols::parse(&PathBuf::from("../IrpProtocols.xml"));
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
