use cir::lircd_conf::{parse, Flags, Remote};
use irp::{Irp, Message, Vartable};
use num_integer::Integer;
use serde::Deserialize;
use std::{
    ffi::OsStr,
    fs::{read_dir, File},
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Deserialize)]
struct RemoteTestData {
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
    recurse(&PathBuf::from("testdata/lircd_conf"));
}

fn recurse(path: &Path) {
    for entry in read_dir(path).unwrap() {
        let e = entry.unwrap();
        let path = e.path();
        if e.metadata().unwrap().file_type().is_dir() {
            recurse(&path);
        } else if path.extension() == Some(OsStr::new("testdata")) {
            let mut conf = path.clone();

            let filename = path.file_name().unwrap().to_string_lossy();
            let filename = filename.strip_suffix(".testdata").unwrap();

            conf.set_file_name(OsStr::new(filename));

            let mut testdata = path;

            testdata.set_extension("testdata");

            if testdata.exists() {
                lircd_encode(&conf, &testdata);
            }
        }
    }
}

fn lircd_encode(conf: &Path, testdata: &Path) {
    println!("Testing {} {}", conf.display(), testdata.display());

    let mut file = File::open(testdata).unwrap();
    let mut data = String::new();
    file.read_to_string(&mut data).unwrap();

    let testdata: Vec<RemoteTestData> = serde_json::from_str(&data).expect("failed to deserialize");

    let lircd_conf = parse(conf).expect("parse should work");

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
                let testdata = Message::from_raw_slice(&testdata.rawir);

                println!("lircd {}", testdata.print_rawir());
                println!("cir {}", message.print_rawir());
                panic!("RAW CODE: {}", code.name);
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

                let mut message = Message::new();

                if code.code.len() == 2 && remote.repeat.0 != 0 && remote.repeat.1 != 0 {
                    // if remote has a repeat parameter and two scancodes, then just repeat the first scancode
                    let mut vars = Vartable::new();
                    vars.set(String::from("CODE"), code.code[0] as i64);

                    let m = irp.encode(vars, 1).expect("encode should succeed");

                    message.extend(&m);
                } else {
                    for code in &code.code {
                        let mut vars = Vartable::new();
                        vars.set(String::from("CODE"), *code as i64);

                        // lircd does not honour toggle bit in RCMM transmit
                        if remote.flags.contains(Flags::RCMM)
                            && remote.toggle_bit_mask.count_ones() == 1
                        {
                            vars.set(
                                String::from("T"),
                                ((*code & remote.toggle_bit_mask) != 0).into(),
                            );
                        }

                        let m = irp.encode(vars, 0).expect("encode should succeed");

                        message.extend(&m);
                    }
                }

                if message.raw.len().is_even() {
                    message.raw.pop();
                }

                let testdata = if let Some(testdata) = testdata.codes.iter().find(|testdata| {
                    let scancode = u64::from_str_radix(&testdata.code, 16).unwrap();

                    code.name == testdata.name && code.code[0] == scancode
                }) {
                    testdata
                } else {
                    println!(
                        "cannot find testdata for code {} 0x{:}",
                        code.name, code.code[0]
                    );
                    continue;
                };

                if !compare_output(remote, &testdata.rawir, &message.raw) {
                    let testdata = Message::from_raw_slice(&testdata.rawir);

                    println!("lircd {}", testdata.print_rawir());
                    println!("cir {}", message.print_rawir());
                    panic!("CODE: {} 0x{:x}", code.name, code.code[0]);
                }
            }
        }
    }
}

fn compare_output(remote: &Remote, lircd: &[u32], our: &[u32]) -> bool {
    if lircd.len() < our.len() {
        let len = lircd.len();
        if our[len] > 100000 && lircd == &our[..len] {
            return true;
        }
    }
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

        if lircd > 4500 && (lircd - remote.bit[0].1 as u32) == our {
            continue;
        }

        if lircd > 4500 && (lircd - remote.bit[1].1 as u32) == our {
            continue;
        }

        return false;
    }

    true
}
