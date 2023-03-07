use irp::{Irp, Vartable};
use irptransmogrifier::{create_jvm, IrpTransmogrifierRender};
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

                match irp.encode(vars.clone()) {
                    Ok(our) => {
                        let trans = IrpTransmogrifierRender::new(&jvm, s).unwrap();

                        let their = trans.render_raw(params.clone()).unwrap();

                        // compare irptransmogrifier output with our own
                        for i in 0..3 {
                            assert!(compare_with_rounding(&our[i], &their[i]));
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
