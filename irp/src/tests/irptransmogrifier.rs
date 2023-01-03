use crate::Message;
use j4rs::{ClasspathEntry, Instance, InvocationArg, JavaClass, Jvm, JvmBuilder};
use std::collections::HashMap;

pub fn create_jvm() -> Jvm {
    JvmBuilder::new()
        .classpath_entry(ClasspathEntry::new(
            "IrpTransmogrifier/target/IrpTransmogrifier-1.2.12-SNAPSHOT-jar-with-dependencies.jar",
        ))
        .build()
        .unwrap()
}

pub struct IrpTransmogrifierRender<'a> {
    protocol: Instance,
    jvm: &'a Jvm,
}

impl<'a> IrpTransmogrifierRender<'a> {
    pub fn new(jvm: &'a Jvm, irp: &str) -> Self {
        let irp = jvm
            .create_instance("java.lang.String", &[InvocationArg::try_from(irp).unwrap()])
            .unwrap();

        let protocol = jvm
            .create_instance("org.harctoolbox.irp.Protocol", &[irp.into()])
            .unwrap();

        IrpTransmogrifierRender { protocol, jvm }
    }

    fn render(&self, param: HashMap<String, i64>) -> Instance {
        let jparam = self
            .jvm
            .java_map(JavaClass::String, JavaClass::Long, param)
            .unwrap();

        self.jvm
            .invoke(&self.protocol, "toIrSignal", &[jparam.into()])
            .unwrap()
    }

    pub fn render_raw(&self, param: HashMap<String, i64>, repeats: usize) -> Message {
        let res = self.render(param);

        let frequency: f64 = self
            .jvm
            .to_rust(self.jvm.invoke(&res, "getFrequency", &[]).unwrap())
            .unwrap();
        let duty_cycle: Option<f64> = self
            .jvm
            .to_rust(self.jvm.invoke(&res, "getDutyCycle", &[]).unwrap())
            .unwrap();

        let intro: Vec<u32> = self
            .jvm
            .to_rust(self.jvm.invoke(&res, "getIntroInts", &[]).unwrap())
            .unwrap();

        let repeat: Vec<u32> = self
            .jvm
            .to_rust(self.jvm.invoke(&res, "getRepeatInts", &[]).unwrap())
            .unwrap();

        let ending: Vec<u32> = self
            .jvm
            .to_rust(self.jvm.invoke(&res, "getEndingInts", &[]).unwrap())
            .unwrap();

        let mut raw = intro;

        for _ in 0..repeats {
            raw.extend(&repeat);
        }

        raw.extend(ending);

        Message {
            carrier: Some(frequency as i64),
            duty_cycle: duty_cycle.map(|d| (d * 100.0) as u8),
            raw,
        }
    }

    pub fn render_pronto(&self, param: HashMap<String, i64>) -> String {
        let res = self.render(param);

        let res = self
            .jvm
            .invoke_static("org.harctoolbox.ircore.Pronto", "toString", &[res.into()])
            .unwrap();

        self.jvm.to_rust(res).unwrap()
    }
}
