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
                let mut uses_non_strict_extent = false;

                irp.stream.visit(
                    &mut uses_non_strict_extent,
                    &|expr, uses_non_strict_extent| {
                        if matches!(
                            expr,
                            irp::Expression::ExtentConstant(..)
                                | irp::Expression::ExtentIdentifier(..)
                        ) {
                            *uses_non_strict_extent = true;
                        }
                    },
                );

                if uses_non_strict_extent {
                    // Transmogrifier does not support it
                    return;
                }

                let mut vars = Vartable::new();

                let mut rng = rand::thread_rng();

                for param in &irp.parameters {
                    let min = param.min.eval(&vars).unwrap();
                    let max = param.max.eval(&vars).unwrap();

                    let value = rng.gen_range(min..=max);

                    params.insert(param.name.to_owned(), value);
                    vars.set(param.name.to_owned(), value);
                }

                if irp.encode(vars.clone(), 1).is_ok() {
                    let trans = IrpTransmogrifierRender::new(&jvm, s).unwrap();

                    let _m = trans.render_raw(params.clone(), 1).unwrap();

                    // compare irptransmogrifier output with our own
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
