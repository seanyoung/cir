pub struct Irp {
    pub general_spec: Vec<GeneralItem>,
    pub bit_spec: Vec<Expression>,
    pub stream: IrStream,
    pub definitions: Vec<Expression>,
    pub parameters: Vec<ParameterSpec>,
}

#[derive(PartialEq, Copy, Clone, Debug)]
pub enum Unit {
    Units,
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

#[derive(PartialEq, Debug)]
pub enum RepeatMarker {
    Any,
    OneOrMore,
    Count(i64),
    CountOrMore(i64),
}

#[derive(PartialEq, Debug)]
pub struct IrStream {
    pub stream: Expression,
    pub repeat: Option<RepeatMarker>,
}

#[derive(PartialEq, Debug)]
pub enum Expression {
    FlashConstant(f64, Unit),
    ExtentConstant(f64, Unit),
    FlashIdentifier(String, Unit),
    ExtentIdentifier(String, Unit),
    Assignment(String, Box<Expression>),
    Number(i64),
    Identifier(String),
    BitField {
        value: Box<Expression>,
        reverse: bool,
        length: Box<Expression>,
        skip: Option<Box<Expression>>,
    },

    Complement(Box<Expression>),
    Not(Box<Expression>),
    Negative(Box<Expression>),
    BitCount(Box<Expression>),

    Power(Box<Expression>, Box<Expression>),
    Multiply(Box<Expression>, Box<Expression>),
    Divide(Box<Expression>, Box<Expression>),
    Modulo(Box<Expression>, Box<Expression>),
    Add(Box<Expression>, Box<Expression>),
    Subtract(Box<Expression>, Box<Expression>),

    ShiftLeft(Box<Expression>, Box<Expression>),
    ShiftRight(Box<Expression>, Box<Expression>),

    LessEqual(Box<Expression>, Box<Expression>),
    Less(Box<Expression>, Box<Expression>),
    More(Box<Expression>, Box<Expression>),
    MoreEqual(Box<Expression>, Box<Expression>),
    Equal(Box<Expression>, Box<Expression>),
    NotEqual(Box<Expression>, Box<Expression>),

    BitwiseAnd(Box<Expression>, Box<Expression>),
    BitwiseOr(Box<Expression>, Box<Expression>),
    BitwiseXor(Box<Expression>, Box<Expression>),
    Or(Box<Expression>, Box<Expression>),
    And(Box<Expression>, Box<Expression>),
    Ternary(Box<Expression>, Box<Expression>, Box<Expression>),
    List(Vec<Expression>),
}

pub struct ParameterSpec {
    pub name: String,
    pub memory: bool,
    pub min: i64,
    pub max: i64,
    pub default: Option<Expression>,
}
