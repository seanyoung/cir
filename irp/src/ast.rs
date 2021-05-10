#[derive(Debug)]
pub struct Irp {
    pub general_spec: GeneralSpec,
    pub stream: Vec<Expression>,
    pub definitions: Vec<Expression>,
    pub parameters: Vec<ParameterSpec>,
}

#[derive(Debug)]
pub struct GeneralSpec {
    pub duty_cycle: Option<u8>,
    pub carrier: Option<i64>,
    pub lsb: bool,
    pub unit: f64,
}

#[derive(PartialEq, Copy, Clone, Debug)]
pub enum Unit {
    Units,
    Microseconds,
    Milliseconds,
    Pulses,
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
    pub bit_spec: Vec<Expression>,
    pub stream: Vec<Expression>,
    pub repeat: Option<RepeatMarker>,
}

#[derive(PartialEq, Debug)]
pub enum Expression {
    FlashConstant(f64, Unit),
    GapConstant(f64, Unit),
    ExtentConstant(f64, Unit),
    FlashIdentifier(String, Unit),
    GapIdentifier(String, Unit),
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
    InfiniteBitField {
        value: Box<Expression>,
        skip: Box<Expression>,
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
    Stream(IrStream),
    Variation(Vec<Vec<Expression>>),
}

#[derive(Debug)]
pub struct ParameterSpec {
    pub name: String,
    pub memory: bool,
    pub min: Expression,
    pub max: Expression,
    pub default: Option<Expression>,
}
