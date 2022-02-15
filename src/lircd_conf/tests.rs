use crate::{
    lircd_conf::{parse, Flags},
    log::Log,
};
use irp::{rawir, Irp, Vartable};
use num_integer::Integer;
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

    let mut all_pass = true;

    for remote in &lircd_conf {
        let testdata = if let Some(testdata) = testdata
            .iter()
            .find(|testdata| testdata.name == remote.name)
        {
            testdata
        } else {
            println!("cannot find testdata for remote {}", remote.name);
            continue;
        };

        if remote.flags.contains(Flags::RAW_CODES) {
            for code in &remote.raw_codes {
                let testdata = if let Some(testdata) = testdata
                    .codes
                    .iter()
                    .find(|testdata| testdata.name == code.name)
                {
                    testdata
                } else {
                    println!("cannot find testdata for code {}", code.name);
                    continue;
                };

                if testdata.rawir != code.rawir {
                    all_pass = false;
                    println!("RAW CODE: {}", code.name);
                    println!("lircd {}", rawir::print_to_string(&testdata.rawir));
                    println!("cir {}", rawir::print_to_string(&code.rawir));
                }
            }
        } else {
            let irp = remote.irp();
            println!("remote {} irp:{}", remote.name, irp);
            let irp = Irp::parse(&irp).expect("should work");

            for code in &remote.codes {
                let mut vars = Vartable::new();
                vars.set(String::from("CODE"), code.code as i64, 32);

                // FIXME: should be possible to test repeats
                let mut message = irp.encode(vars, 0).expect("encode should succeed");

                if message.raw.len().is_even() {
                    message.raw.pop();
                }

                let testdata = if let Some(testdata) = testdata.codes.iter().find(|testdata| {
                    let scancode = u64::from_str_radix(&testdata.code, 16).unwrap();

                    code.name == testdata.name && code.code == scancode
                }) {
                    testdata
                } else {
                    println!(
                        "cannot find testdata for code {} 0x{:}",
                        code.name, code.code
                    );
                    continue;
                };

                if testdata.rawir != message.raw {
                    all_pass = false;
                    println!("CODE: {} {:x}", code.name, code.code);
                    println!("lircd {}", rawir::print_to_string(&testdata.rawir));
                    println!("cir {}", rawir::print_to_string(&message.raw));
                }
            }
        }
    }

    println!("ALL PASS: {}", all_pass);
}
