pub struct Irp {
    pub general_spec: Vec<GeneralItem>,
}

#[derive(PartialEq)]
pub enum GeneralItem {
    Frequency(f64),
    DutyCycle(f64),
    OrderMsb,
    OrderLsb,
    Unit(f64),
    UnitPulse(f64),
}
