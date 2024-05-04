use j4rs::{
    errors::J4RsError, ClasspathEntry, Instance, InvocationArg, JavaClass, Jvm, JvmBuilder,
};
use std::collections::HashMap;

pub fn create_jvm(base: &str) -> Jvm {
    let mut base = base.to_owned();

    base.push_str("/target/IrpTransmogrifier-1.2.14-SNAPSHOT-jar-with-dependencies.jar");

    JvmBuilder::new()
        .classpath_entry(ClasspathEntry::new(&base))
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

        let protocol =
            jvm.create_instance("org.harctoolbox.irp.Protocol", &[InvocationArg::from(irp)])?;

        Ok(IrpTransmogrifierRender { protocol, jvm })
    }

    fn render(&self, param: HashMap<String, i64>) -> Result<Instance, J4RsError> {
        let jparam = self
            .jvm
            .java_map(JavaClass::String, JavaClass::Long, param)?;

        self.jvm
            .invoke(&self.protocol, "toIrSignal", &[InvocationArg::from(jparam)])
    }

    pub fn render_raw(&self, param: HashMap<String, i64>) -> Result<[Vec<u32>; 3], J4RsError> {
        let res = self.render(param)?;

        let intro: Vec<u32> = self.jvm.to_rust(self.jvm.invoke(
            &res,
            "getIntroInts",
            InvocationArg::empty(),
        )?)?;

        let repeat: Vec<u32> = self.jvm.to_rust(self.jvm.invoke(
            &res,
            "getRepeatInts",
            InvocationArg::empty(),
        )?)?;

        let ending: Vec<u32> = self.jvm.to_rust(self.jvm.invoke(
            &res,
            "getEndingInts",
            InvocationArg::empty(),
        )?)?;

        Ok([intro, repeat, ending])
    }

    pub fn render_pronto(&self, param: HashMap<String, i64>) -> Result<String, J4RsError> {
        let res = self.render(param)?;

        let res = self.jvm.invoke_static(
            "org.harctoolbox.ircore.Pronto",
            "toString",
            &[InvocationArg::from(res)],
        )?;

        self.jvm.to_rust(res)
    }
}
