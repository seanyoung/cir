use irp::{InfraredData, Irp, Message, NFADecoder, Vartable};
use irptransmogrifier::{create_jvm, IrpTransmogrifierRender};
use itertools::Itertools;
use rand::Rng;
use std::collections::HashMap;

fn main() {
    let jvm = create_jvm("../IrpTransmogrifier");

    let args: Vec<String> = std::env::args().collect();

    let s = &args[1];

    loop {
        let mut rust_ok = true;
        let mut params = HashMap::new();

        match Irp::parse(s) {
            Ok(irp) => {
                let mut vars = Vartable::new();

                let mut rng = rand::thread_rng();

                for param in &irp.parameters {
                    let value = rng.gen_range(param.min..=param.max);
                    println!("{}={}", param.name, value);

                    params.insert(param.name.to_owned(), value);
                    vars.set(param.name.to_owned(), value);
                }

                // encode with irp crate
                match irp.encode(vars.clone()) {
                    Ok(our) => {
                        // encode with transmogrifier
                        let trans = IrpTransmogrifierRender::new(&jvm, s).unwrap();

                        let their = trans.render_raw(params.clone()).unwrap();

                        // compare irptransmogrifier output
                        for i in 0..3 {
                            assert!(compare_with_rounding(&our[i], &their[i]));
                        }

                        match irp.build_nfa() {
                            Ok(nfa) => {
                                let mut decoder = NFADecoder::new(100, 3, 100000);
                                let mut decoded = false;

                                for part in our {
                                    let ir = InfraredData::from_u32_slice(&part);

                                    let mut failed = false;

                                    for i in ir {
                                        decoder.input(i, &nfa, |ev, fields| {
                                            println!(
                                                "decode {i} {ev}: {}",
                                                fields
                                                    .iter()
                                                    .map(|(name, v)| format!("{name}={v}"))
                                                    .join(", ")
                                            );

                                            if fields == params {
                                                decoded = true;
                                            } else {
                                                for (n, v) in fields {
                                                    if params[&n] != v {
                                                        failed = true;
                                                        println!(
                                                            "{n} decoded as {} should be {}",
                                                            v, params[&n]
                                                        );
                                                    }
                                                }
                                            }
                                        });
                                    }

                                    if failed {
                                        panic!("{}", Message::from_raw_slice(&part).print_rawir());
                                    }
                                }

                                assert!(decoded);
                            }
                            Err(e) => {
                                panic!("compile {e}");
                            }
                        }
                    }
                    Err(e) => {
                        println!("encode: {e}");
                        rust_ok = false;
                    }
                }
            }
            Err(e) => {
                println!("parse: {e}");
                rust_ok = false;
            }
        }

        if !rust_ok {
            if let Ok(trans) = IrpTransmogrifierRender::new(&jvm, s) {
                if trans.render_raw(params).is_ok() {
                    // IrpTransmogrifier parsed & rendered what we could not! EH?
                    panic!("we could not parse it, transmogrifier could");
                }
            }
        }
    }
}

fn compare_with_rounding(l: &[u32], r: &[u32]) -> bool {
    if l == r {
        return true;
    }

    if l.len() != r.len() {
        println!(
            "comparing:\n{:?} with\n{:?}\n have different lengths {} and {}",
            l,
            r,
            l.len(),
            r.len()
        );

        return false;
    }

    for i in 0..l.len() {
        let diff = if l[i] > r[i] {
            l[i] - r[i]
        } else {
            r[i] - l[i]
        };
        // is the difference more than 8 and more than 1 promille
        if diff > 8 && (diff * 1000 / l[i]) > 0 {
            println!(
                "comparing:\nleft:{:?} with\nright:{:?}\nfailed at position {} out of {}",
                l,
                r,
                i,
                l.len()
            );

            return false;
        }
    }

    true
}
