use crate::Message;
use j4rs::{
    errors::J4RsError, ClasspathEntry, Instance, InvocationArg, JavaClass, Jvm, JvmBuilder,
};
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
