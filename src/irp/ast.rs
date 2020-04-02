pub struct Irp {
    pub general_spec: Vec<GeneralItem>,
    pub bit_spec: Vec<Vec<Duration>>,
    pub stream: IrStream,
    pub definitions: Vec<(String, Expression)>,
}

#[derive(PartialEq, Debug)]
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
pub enum Duration {
    FlashConstant(f64, Unit),
    GapConstant(f64, Unit),
    ExtentConstant(f64, Unit),
    FlashIdentifier(String, Unit),
    GapIdentifier(String, Unit),
    ExtentIdentifier(String, Unit),
}

#[derive(PartialEq, Debug)]
pub enum IrStreamItem {
    Duration(Duration),
    Assignment(String, Expression),
    BitField(bool, Expression, bool, Expression, Option<Expression>),
    InfiniteBitField(bool, Expression, Expression),
}

#[derive(PartialEq)]
pub enum RepeatMarker {
    Any,
    OneOrMore,
    Count(i64),
    CountOrMore(i64),
}

#[derive(PartialEq)]
pub struct IrStream {
    pub stream: Vec<IrStreamItem>,
    pub repeat: Option<RepeatMarker>,
}

#[derive(PartialEq, Debug)]
pub enum Expression {
    Number(i64),
    Identifier(String),
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
}
