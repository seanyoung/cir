use cir::lircd_conf::parse;
use irp::Message;
use liblircd::{lirc_log_set_stdout, LircdConf};
use num_integer::Integer;
use std::fs::read;

fn main() {
    unsafe { lirc_log_set_stdout() };

    let args: Vec<String> = std::env::args().collect();

    let path = &args[1];

    println!("Testing {path}");

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

                let decoded = lircd_remote.decode(&message.raw);

                let mut expect = vec![our_code.code[0]];

                for _ in 0..lircd_remote.min_repeat() {
                    expect.push(our_code.code[0]);
                }

                if decoded != expect {
                    panic!(
                        "DECODE MISMATCH got: {decoded:#x?} expected: {:#x?}",
                        expect
                    );
                } else {
                    println!("LIRCD DECODE {:#x?} OK", decoded);
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
