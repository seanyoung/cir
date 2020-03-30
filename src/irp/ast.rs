pub struct Irp {
    pub general_spec: Vec<GeneralItem>,
    pub bit_spec: Vec<Vec<Duration>>,
}

#[derive(PartialEq)]
pub enum Unit {
    Microseconds,
    Milliseconds,
    Pulses,
}

#[derive(PartialEq)]
pub enum GeneralItem {
    Frequency(f64),
    DutyCycle(f64),
    OrderMsb,
    OrderLsb,
    Unit(f64, Unit),
}

#[derive(PartialEq)]
pub enum Duration {
    Flash(f64, Unit),
    Gap(f64, Unit),
    Extent(f64, Unit),
}
