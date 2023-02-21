use afl::fuzz;
use irp::{Irp, Message, Vartable};
use j4rs::{
    errors::J4RsError, ClasspathEntry, Instance, InvocationArg, JavaClass, Jvm, JvmBuilder,
};
use rand::Rng;
use std::collections::HashMap;

pub fn create_jvm() -> Jvm {
    JvmBuilder::new()
        .classpath_entry(ClasspathEntry::new(
            "../IrpTransmogrifier/target/IrpTransmogrifier-1.2.12-SNAPSHOT-jar-with-dependencies.jar",
        ))
        .build()
        .unwrap()
}

pub struct IrpTransmogrifierRender<'a> {
    protocol: Instance,
    jvm: &'a Jvm,
}

impl<'a> IrpTransmogrifierRender<'a> {
    pub fn new(jvm: &'a Jvm, irp: &str) -> Result<Self, J4RsError> {
        let irp = jvm.create_instance("java.lang.String", &[InvocationArg::try_from(irp)?])?;

        let protocol = jvm.create_instance("org.harctoolbox.irp.Protocol", &[irp.into()])?;

        Ok(IrpTransmogrifierRender { protocol, jvm })
    }

    fn render(&self, param: HashMap<String, i64>) -> Result<Instance, J4RsError> {
        let jparam = self
            .jvm
            .java_map(JavaClass::String, JavaClass::Long, param)?;

        self.jvm
            .invoke(&self.protocol, "toIrSignal", &[jparam.into()])
    }

    pub fn render_raw(
        &self,
        param: HashMap<String, i64>,
        repeats: usize,
    ) -> Result<Message, J4RsError> {
        let res = self.render(param)?;

        let frequency: f64 = self
            .jvm
            .to_rust(self.jvm.invoke(&res, "getFrequency", &[])?)?;
        let duty_cycle: Option<f64> =
            self.jvm
                .to_rust(self.jvm.invoke(&res, "getDutyCycle", &[])?)?;

        let intro: Vec<u32> = self
            .jvm
            .to_rust(self.jvm.invoke(&res, "getIntroInts", &[])?)?;

        let repeat: Vec<u32> = self
            .jvm
            .to_rust(self.jvm.invoke(&res, "getRepeatInts", &[])?)?;

        let ending: Vec<u32> = self
            .jvm
            .to_rust(self.jvm.invoke(&res, "getEndingInts", &[])?)?;

        let mut raw = intro;

        for _ in 0..repeats {
            raw.extend(&repeat);
        }

        raw.extend(ending);

        Ok(Message {
            carrier: Some(frequency as i64),
            duty_cycle: duty_cycle.map(|d| (d * 100.0) as u8),
            raw,
        })
    }

    pub fn render_pronto(&self, param: HashMap<String, i64>) -> Result<String, J4RsError> {
        let res = self.render(param)?;

        let res =
            self.jvm
                .invoke_static("org.harctoolbox.ircore.Pronto", "toString", &[res.into()])?;

        self.jvm.to_rust(res)
    }
}

fn main() {
    let jvm = create_jvm();

    fuzz!(|data: &[u8]| {
        if let Ok(s) = std::str::from_utf8(data) {
            let mut rust_ok = true;
            let mut params = HashMap::new();

            if let Ok(irp) = Irp::parse(s) {
                let mut vars = Vartable::new();

                let mut rng = rand::thread_rng();

                for param in &irp.parameters {
                    let value = rng.gen_range(param.min..=param.max);

                    params.insert(param.name.to_owned(), value);
                    vars.set(param.name.to_owned(), value);
                }

                if let Ok(our) = irp.encode(vars.clone(), 1) {
                    let trans = IrpTransmogrifierRender::new(&jvm, s).unwrap();

                    let their = trans.render_raw(params.clone(), 1).unwrap();

                    // compare irptransmogrifier output with our own
                    assert!(compare_with_rounding(&our.raw, &their.raw));
                } else {
                    rust_ok = false;
                }
            } else {
                rust_ok = false;
            }

            if !rust_ok {
                if let Ok(trans) = IrpTransmogrifierRender::new(&jvm, s) {
                    if trans.render_raw(params, 1).is_ok() {
                        // IrpTransmogrifier parsed & rendered what we could not! EH?
                        panic!();
                    }
                }
            }
        }
    });
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
