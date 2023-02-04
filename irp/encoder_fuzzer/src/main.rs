use afl::fuzz;
use irp::{Irp, Vartable};
use rand::Rng;
use std::collections::HashMap;

fn main() {
    fuzz!(|data: &[u8]| {
        if let Ok(s) = std::str::from_utf8(data) {
            if let Ok(irp) = Irp::parse(s) {
                let mut vars = Vartable::new();

                let mut params = HashMap::new();

                let mut rng = rand::thread_rng();

                for param in &irp.parameters {
                    let min = param.min.eval(&vars).unwrap().0;
                    let max = param.max.eval(&vars).unwrap().0;

                    if min > max {
                        continue;
                    }

                    let value = rng.gen_range(min..=max);

                    params.insert(param.name.to_owned(), value);
                    vars.set(param.name.to_owned(), value, 32);
                }

                let _ = irp.encode(vars.clone(), 1);
            }
        }
    });
}
