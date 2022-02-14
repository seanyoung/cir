use crate::{lircd_conf::parse, log::Log};
use irp::{rawir, Irp, Vartable};
use serde::Deserialize;
use std::{
    ffi::OsStr,
    fs::{read_dir, File},
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Deserialize)]
struct Remote {
    name: String,
    codes: Vec<Code>,
}

#[derive(Deserialize)]
#[allow(unused)]
struct Code {
    name: String,
    code: String,
    rawir: Vec<u32>,
}

#[test]
fn lircd_testdata() {
    let log = Log::new();

    recurse(&PathBuf::from("testdata/lircd_conf"), &log);

    panic!("meh");
}

fn recurse(path: &Path, log: &Log) {
    for entry in read_dir(path).unwrap() {
        let e = entry.unwrap();
        let path = e.path();
        if e.metadata().unwrap().file_type().is_dir() {
            recurse(&path, log);
        } else if path.extension() != Some(OsStr::new("testdata")) {
            let conf = path.clone();
            let mut testdata = path;

            testdata.set_extension("testdata");

            if testdata.exists() {
                lircd_encode(&conf, &testdata, log);
            }
        }
    }
}

fn lircd_encode(conf: &Path, testdata: &Path, log: &Log) {
    println!("Testing {} {}", conf.display(), testdata.display());

    let mut file = File::open(testdata).unwrap();
    let mut data = String::new();
    file.read_to_string(&mut data).unwrap();

    let testdata: Vec<Remote> = serde_json::from_str(&data).expect("failed to deserialize");

    let lircd_conf = parse(conf, log).expect("parse should work");

    for remote in &lircd_conf {
        let irp = remote.irp(log).expect("should work");
        let irp = Irp::parse(&irp).expect("should work");

        let testdata = if let Some(testdata) = testdata
            .iter()
            .find(|testdata| testdata.name == remote.name)
        {
            testdata
        } else {
            println!("cannot find testdata for {}", remote.name);
            continue;
        };

        for (code_no, code) in remote.codes.iter().enumerate() {
            let mut vars = Vartable::new();
            vars.set(String::from("CODE"), code.code as i64, 32);

            // FIXME: should be possible to test repeats
            let mut message = irp.encode(vars, 0).expect("encode should succeed");

            message.raw.pop();

            if testdata.codes.len() <= code_no {
                println!("testdata no missing {}", code.name);
                continue;
            }
            let testdata = &testdata.codes[code_no];

            if testdata.name != code.name {
                println!("testdata no matchy {} {}", testdata.name, code.name);
                continue;
            }

            if testdata.rawir != message.raw {
                println!("lircd {}", rawir::print_to_string(&testdata.rawir));
                println!("cir {}", rawir::print_to_string(&message.raw));
            }
        }
    }
}
