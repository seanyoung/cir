#[derive(PartialEq, Clone, Copy, Debug)]
pub enum Level {
    Error,
    Warning,
    Info,
    Success,
    Trace,
}

impl Default for Log {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct Log {
    level: Level,
}

impl Log {
    pub fn new() -> Self {
        Log {
            level: Level::Warning,
        }
    }

    pub fn quiet(&mut self) {
        self.level = Level::Error;
    }

    pub fn verbose(&mut self, verbose: u64) {
        let mut level = self.level;

        for _ in 0..verbose {
            level = match level {
                Level::Success | Level::Trace => Level::Trace,
                Level::Info => Level::Success,
                Level::Warning => Level::Info,
                Level::Error => Level::Error,
            };
        }

        self.level = level;
    }

    pub fn trace(&self, msg: &str) {
        if self.level == Level::Trace {
            eprintln!("trace: {}", msg);
        }
    }

    pub fn info(&self, msg: &str) {
        match self.level {
            Level::Info | Level::Trace | Level::Success => eprintln!("info: {}", msg),
            _ => (),
        }
    }

    pub fn success(&self, msg: &str) {
        match self.level {
            Level::Trace | Level::Success => eprintln!("success: {}", msg),
            _ => (),
        }
    }

    pub fn warning(&self, msg: &str) {
        match self.level {
            Level::Trace | Level::Success | Level::Warning => eprintln!("warning: {}", msg),
            _ => (),
        }
    }

    pub fn error(&self, msg: &str) {
        eprintln!("error: {}", msg);
    }
}
