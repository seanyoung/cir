use irp::{rawir, Irp, Vartable};
use linux_infrared::{
    lircd_conf::{parse, LircRemote},
    log::Log,
};
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
        } else if path.extension() == Some(OsStr::new("testdata")) {
            let mut conf = path.clone();

            let filename = path.file_name().unwrap().to_string_lossy();
            let filename = filename.strip_suffix(".testdata").unwrap();

            conf.set_file_name(OsStr::new(filename));

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

        for code in &remote.raw_codes {
            if code.dup {
                continue;
            }

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

            let mut message = remote.encode_raw(code, 0);

            message.raw.pop();

            if testdata.rawir != message.raw {
                all_pass = false;
                println!("RAW CODE: {}", code.name);
                println!("lircd {}", rawir::print_to_string(&testdata.rawir));
                println!("cir {}", rawir::print_to_string(&message.raw));
            }
        }

        if !remote.codes.is_empty() {
            let irp = remote.irp();
            println!("remote {} irp:{}", remote.name, irp);
            let irp = Irp::parse(&irp).expect("should work");

            for code in &remote.codes {
                if code.dup {
                    continue;
                }

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

                if !compare_output(remote, &testdata.rawir, &message.raw) {
                    all_pass = false;
                    println!("CODE: {} 0x{:x}", code.name, code.code);
                    println!("lircd {}", rawir::print_to_string(&testdata.rawir));
                    println!("cir {}", rawir::print_to_string(&message.raw));
                }
            }
        }
    }

    println!("ALL PASS: {}", all_pass);
}

fn compare_output(remote: &LircRemote, lircd: &[u32], our: &[u32]) -> bool {
    if lircd.len() != our.len() {
        return false;
    }

    if lircd == our {
        return true;
    }

    for (lircd, our) in lircd.iter().zip(our.iter()) {
        let lircd = *lircd;
        let our = *our;

        if lircd == our {
            continue;
        }

        if lircd > 10000 && (lircd - remote.bit[0].1 as u32) == our {
            continue;
        }

        if lircd > 10000 && (lircd - remote.bit[1].1 as u32) == our {
            continue;
        }

        return false;
    }

    true
}
