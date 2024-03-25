use cir::lircd_conf::parse;
use irp::{InfraredData, Message};
use liblircd::LircdConf;
use num_integer::Integer;
use std::{
    ffi::OsStr,
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
            lircd_encode_decode(&path);
        }
    }
}

fn lircd_encode_decode(path: &Path) {
    println!("Testing {}", path.display());

    let data = read(path).unwrap();
    let source = String::from_utf8_lossy(&data);

    let lircd_conf = LircdConf::parse(&source).unwrap();

    let our_conf = parse(path).unwrap_or_default();
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

        let mut decoder = our_remote.decoder(Some(10), Some(1), 200000);

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

                let mut decoded = Vec::new();

                decoder.reset();

                for ir in InfraredData::from_u32_slice(&message.raw) {
                    decoder.input(ir, |name, _| {
                        decoded.push(name);
                    });
                }

                assert!(decoded.contains(&our_code.name.as_str()));
            }
        }

        if !our_remote.codes.is_empty() {
            let irp = our_remote.encode_irp();
            println!("remote {} irp:{}", our_remote.name, irp);

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

                let mut message = our_remote
                    .encode(our_code, 0)
                    .expect("encode should succeed");

                if message.raw.len().is_even() {
                    message.raw.pop();
                }

                if !compare_output(&lircd, &message.raw) {
                    let testdata = Message::from_raw_slice(&lircd);

                    println!("lircd {}", testdata.print_rawir());
                    println!("cir {}", message.print_rawir());
                    panic!("CODE: {} {:#x}", our_code.name, our_code.code[0]);
                }

                // so now we know that cir and lircd agree on the exact transmit encoding

                // let's see if lircd can decode its own creation

                if path == OsStr::new("testdata/lircd_conf/creative/livedrive.lircd.conf") {
                    // not decodable, missing ptrail
                    continue;
                }

                if path == OsStr::new("testdata/lircd_conf/meridian/MSR.lircd.conf") {
                    // not decodable, missing plead/header
                    continue;
                }

                if path == OsStr::new("testdata/lircd_conf/logitech/logitech.lircd.conf")
                    || path == OsStr::new("testdata/lircd_conf/pcmak/pcmak.lircd.conf")
                    || path == OsStr::new("testdata/lircd_conf/pixelview/remotemaster.lircd.conf")
                {
                    // not decodable, leading space (both lircd and irp crate)
                    continue;
                }

                let mut decoded = lircd_remote.decode(&lircd);

                decoded.iter_mut().for_each(|v| {
                    *v &= !lircd_remote.toggle_bit_mask();
                });

                let mut expect = Vec::new();

                let mut min_repeat = lircd_remote.min_repeat();

                if lircd_remote.toggle_mask() != 0 {
                    if min_repeat > 1 {
                        min_repeat /= 2;
                    }
                    for _ in 0..min_repeat {
                        expect.push(our_code.code[0] & !lircd_remote.toggle_bit_mask());
                    }
                } else {
                    expect = vec![our_code.code[0] & !lircd_remote.toggle_bit_mask()];

                    for _ in 0..min_repeat {
                        expect.push(our_code.code[0] & !lircd_remote.toggle_bit_mask());
                    }
                }

                if decoded != expect {
                    // is decoded and expected all the same value?
                    let all_the_same = if !decoded.is_empty() && !expect.is_empty() {
                        decoded
                            .iter()
                            .chain(expect.iter())
                            .all(|v| *v == decoded[0])
                    } else {
                        false
                    };

                    if !all_the_same {
                        panic!(
                            "DECODE MISMATCH got: {decoded:#x?} expected: {:#x?}",
                            expect
                        );
                    }
                }

                let mut decoded = Vec::new();

                decoder.reset();

                // needs trailing space
                let message = our_remote
                    .encode(our_code, 0)
                    .expect("encode should succeed");

                for ir in InfraredData::from_u32_slice(&message.raw) {
                    decoder.input(ir, |_, code| {
                        decoded.push(code & !lircd_remote.toggle_bit_mask());
                    });
                }

                if decoded != expect && !expect.is_empty() {
                    // is decoded and expected all the same value?
                    let all_the_same = if !decoded.is_empty() && !expect.is_empty() {
                        decoded
                            .iter()
                            .chain(expect.iter())
                            .all(|v| *v == decoded[0])
                    } else {
                        false
                    };

                    if !all_the_same {
                        println!("{}", message.print_rawir());
                        println!("irp: {}", our_remote.decode_irp());
                        panic!(
                            "DECODE MISMATCH got: {decoded:#x?} expected: {:#x?}",
                            expect
                        );
                    }
                }
            }
        }
    }
}

fn compare_output(lircd: &[u32], our: &[u32]) -> bool {
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

        println!("postition:{} lircd {} vs our {}", no, lircd, our);

        return false;
    }

    true
}
