use crate::{lircd_conf::parse, log::Log};
use irp::{Irp, Vartable};
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsStr,
    fs::{read_dir, File},
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct Remote {
    name: String,
    codes: Vec<Code>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Code {
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
    let mut file = File::open(testdata).unwrap();
    let mut data = String::new();
    file.read_to_string(&mut data).unwrap();

    let testdata: Vec<Remote> = serde_json::from_str(&data).expect("failed to deserialize");

    let lircd_conf = parse(conf, log).expect("parse should work");

    for (remote_no, remote) in lircd_conf.iter().enumerate() {
        let irp = remote.irp(log).expect("should work");
        let irp = Irp::parse(&irp).expect("should work");

        for (code_no, code) in remote.codes.iter().enumerate() {
            let mut vars = Vartable::new();
            vars.set(String::from("CODE"), code.code as i64, 32);

            // FIXME: should be possible to test repeats
            let mut message = irp.encode(vars, 0).expect("encode should succeed");

            message.raw.pop();

            let testdata = &testdata[remote_no].codes[code_no];

            assert_eq!(testdata.name, code.name);

            if testdata.rawir == message.raw {
                println!("MATCH for {}", conf.display());
            } else {
                println!("NO MATCH for {}", conf.display());
            }
        }
    }
}
