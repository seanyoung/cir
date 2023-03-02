use cir::lircd_conf::{parse, Flags, Remote};
use irp::{Irp, Message, Vartable};
use liblircd::LircdConf;
use num_integer::Integer;
use std::{
    fs::{read, read_dir},
    path::{Path, PathBuf},
};

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
        } else if path.to_string_lossy().ends_with(".lircd.conf") {
            lircd_encode(&path);
        }
    }
}

fn lircd_encode(path: &Path) {
    println!("Testing {}", path.display());

    let data = read(path).unwrap();
    let source = String::from_utf8_lossy(&data);

    let lircd_conf = LircdConf::parse(&source).unwrap();

    let our_conf = parse(path).unwrap_or(Vec::new());
    let mut our_conf = our_conf.iter();

    for lircd_remote in lircd_conf.iter() {
        if lircd_remote.is_raw() {
            println!("raw valid: {}", lircd_remote.name());
        } else if lircd_remote.is_serial()
            || lircd_remote.codes_iter().count() == 0
            || lircd_remote.bit(0) == (0, 0)
            || lircd_remote.bit(1) == (0, 0)
        {
            // our rust lircd conf parser will refuse to parse this
            println!("not valid: {}", lircd_remote.name());
            continue;
        } else {
            println!("valid: {}", lircd_remote.name());
        }

        let our_remote = our_conf.next().unwrap();

        if matches!(
            (lircd_remote.bit(0), lircd_remote.bit(1)),
            ((_, 0), (0, _)) | ((0, _), (_, 0))
        ) {
            // TODO: fix either cir or lircd
            println!(
                "SKIP: {} because lircd doesn't encode correctly",
                lircd_remote.name()
            );
            continue;
        } else if lircd_remote.toggle_bit_mask() != 0 && lircd_remote.toggle_bit() == 0 {
            // TODO: fix either cir or lircd
            println!(
                "SKIP: {} because lircd does weird things",
                lircd_remote.name()
            );
            continue;
        }

        if lircd_remote.is_raw() {
            for (our_code, lircd_code) in our_remote.raw_codes.iter().zip(lircd_remote.codes_iter())
            {
                if our_code.dup {
                    continue;
                }

                assert_eq!(our_code.name, lircd_code.name());

                let lircd = match lircd_code.encode() {
                    Some(d) => d,
                    None => {
                        println!(
                            "cannot encode code {} 0x{:}",
                            lircd_code.name(),
                            lircd_code.code()
                        );
                        continue;
                    }
                };

                let mut message = our_remote.encode_raw(our_code, 0).unwrap();

                message.raw.pop();

                if lircd != message.raw {
                    let testdata = Message::from_raw_slice(&lircd);

                    println!("lircd {}", testdata.print_rawir());
                    println!("cir {}", message.print_rawir());
                    panic!("RAW CODE: {}", our_code.name);
                }
            }
        }

        if !our_remote.codes.is_empty() {
            let irp = our_remote.irp();
            println!("remote {} irp:{}", our_remote.name, irp);
            let irp = Irp::parse(&irp).expect("should work");

            for (our_code, lircd_code) in our_remote.codes.iter().zip(lircd_remote.codes_iter()) {
                if our_code.dup {
                    continue;
                }

                assert_eq!(our_code.name, lircd_code.name());
                assert_eq!(our_code.code[0], lircd_code.code());

                let lircd = match lircd_code.encode() {
                    Some(d) => d,
                    None => {
                        println!(
                            "cannot encode code {} 0x{:}",
                            lircd_code.name(),
                            lircd_code.code()
                        );
                        continue;
                    }
                };

                let mut message = Message::new();

                if our_code.code.len() == 2 && our_remote.repeat.0 != 0 && our_remote.repeat.1 != 0
                {
                    // if remote has a repeat parameter and two scancodes, then just repeat the first scancode
                    let mut vars = Vartable::new();
                    vars.set(String::from("CODE"), our_code.code[0] as i64);

                    let m = irp.encode(vars, 1).expect("encode should succeed");

                    message.extend(&m);
                } else {
                    for code in &our_code.code {
                        let mut vars = Vartable::new();
                        vars.set(String::from("CODE"), *code as i64);

                        // lircd does not honour toggle bit in RCMM transmit
                        if our_remote.flags.contains(Flags::RCMM)
                            && our_remote.toggle_bit_mask.count_ones() == 1
                        {
                            vars.set(
                                String::from("T"),
                                ((*code & our_remote.toggle_bit_mask) != 0).into(),
                            );
                        }

                        let m = irp.encode(vars, 0).expect("encode should succeed");

                        message.extend(&m);
                    }
                }

                if message.raw.len().is_even() {
                    message.raw.pop();
                }

                if !compare_output(our_remote, &lircd, &message.raw) {
                    let testdata = Message::from_raw_slice(&lircd);

                    println!("lircd {}", testdata.print_rawir());
                    println!("cir {}", message.print_rawir());
                    panic!("CODE: {} 0x{:x}", our_code.name, our_code.code[0]);
                }
            }
        }
    }
}

fn compare_output(remote: &Remote, lircd: &[u32], our: &[u32]) -> bool {
    if lircd.len() != our.len() {
        println!("length {} {} differ", lircd.len(), our.len());
        return false;
    }

    if lircd == our {
        return true;
    }

    for (no, (lircd, our)) in lircd.iter().zip(our.iter()).enumerate() {
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

        println!("postition:{} {} vs {}", no, lircd, our);

        return false;
    }

    true
}
