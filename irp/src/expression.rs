use super::{Expression, RepeatMarker, Stream, Unit, Vartable};
use std::{fmt, rc::Rc};

impl fmt::Display for Expression {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Expression::Number(v) => write!(f, "{v}"),
            Expression::Identifier(id) => write!(f, "{id}"),
            Expression::Add(left, right) => write!(f, "({left} + {right})"),
            Expression::Subtract(left, right) => write!(f, "({left} - {right})"),
            Expression::Multiply(left, right) => write!(f, "({left} * {right})"),
            Expression::Divide(left, right) => write!(f, "({left} / {right})"),
            Expression::Power(left, right) => write!(f, "({left} ** {right})"),
            Expression::Modulo(left, right) => write!(f, "({left} % {right})"),
            Expression::BitwiseOr(left, right) => write!(f, "({left} | {right})"),
            Expression::BitwiseAnd(left, right) => write!(f, "({left} & {right})"),
            Expression::BitwiseXor(left, right) => write!(f, "({left} ^ {right})"),
            Expression::ShiftLeft(left, right) => write!(f, "({left} << {right})"),
            Expression::ShiftRight(left, right) => write!(f, "({left} >> {right})"),

            Expression::Equal(left, right) => write!(f, "({left} == {right})"),
            Expression::NotEqual(left, right) => write!(f, "({left} != {right})"),
            Expression::More(left, right) => write!(f, "({left} > {right})"),
            Expression::MoreEqual(left, right) => write!(f, "({left} >= {right})"),
            Expression::Less(left, right) => write!(f, "({left} < {right})"),
            Expression::LessEqual(left, right) => write!(f, "({left} <= {right})"),

            Expression::Or(left, right) => write!(f, "({left} || {right})"),
            Expression::And(left, right) => write!(f, "({left} && {right})"),
            Expression::Conditional(cond, left, right) => {
                write!(f, "({cond} ? {left} : {right})")
            }
            Expression::Complement(expr) => write!(f, "~{expr}"),
            Expression::Not(expr) => write!(f, "!{expr}"),
            Expression::Negative(expr) => write!(f, "-{expr}"),
            Expression::BitCount(expr) => write!(f, "#({expr})"),
            Expression::Log2(expr) => write!(f, "LOG2({expr})"),
            Expression::BitField {
                value,
                reverse,
                length,
                skip: Some(skip),
            } => {
                write!(
                    f,
                    "{}:{}{}:{}",
                    value,
                    if *reverse { "-" } else { "" },
                    length,
                    skip
                )
            }
            Expression::BitField {
                value,
                reverse,
                length,
                skip: None,
            } => {
                write!(f, "{}:{}{}", value, if *reverse { "-" } else { "" }, length,)
            }
            Expression::InfiniteBitField { value, skip } => {
                write!(f, "{value}::{skip}")
            }
            Expression::BitReverse(expr, count, skip) => {
                write!(f, "BITREV({expr},{count},{skip})")
            }
            Expression::Assignment(name, expr) => write!(f, "{name}={expr}"),
            Expression::List(list) => {
                write!(f, "(")?;
                let mut first = true;
                for expr in list {
                    if !first {
                        write!(f, ",")?;
                    }
                    write!(f, "{expr}")?;
                    first = false;
                }
                write!(f, ")")
            }
            Expression::Variation(variants) => {
                for variant in variants {
                    write!(f, "[")?;
                    let mut first = true;
                    for expr in variant {
                        if !first {
                            write!(f, ",")?;
                        }
                        write!(f, "{expr}")?;
                        first = false;
                    }
                    write!(f, "]")?;
                }
                write!(f, "")
            }
            Expression::FlashConstant(v, u) => {
                write!(f, "{v}{u}")
            }
            Expression::GapConstant(v, u) => {
                write!(f, "-{v}{u}")
            }
            Expression::FlashIdentifier(v, u) => {
                write!(f, "{v}{u}")
            }
            Expression::GapIdentifier(v, u) => {
                write!(f, "-{v}{u}")
            }
            Expression::ExtentConstant(v, u) => {
                write!(f, "^{v}{u}")
            }
            Expression::ExtentIdentifier(v, u) => {
                write!(f, "^{v}{u}")
            }
            Expression::Stream(stream) => {
                // bitspec
                if !stream.bit_spec.is_empty() {
                    let mut iter = stream.bit_spec.iter();
                    let mut next = iter.next();
                    write!(f, "<")?;
                    while let Some(expr) = next {
                        write!(f, "{expr}")?;
                        next = iter.next();
                        if next.is_some() {
                            write!(f, "|")?;
                        } else {
                            write!(f, ">")?;
                        }
                    }
                }

                // stream
                write!(f, "(")?;
                let mut first = true;
                for expr in &stream.stream {
                    if !first {
                        write!(f, ",")?;
                    }
                    write!(f, "{expr}")?;
                    first = false;
                }
                write!(f, ")")?;

                // repeat marker
                if let Some(repeat) = &stream.repeat {
                    write!(f, "{repeat}")
                } else {
                    write!(f, "")
                }
            }
        }
    }
}

impl fmt::Display for Unit {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Unit::Pulses => write!(f, "p"),
            Unit::Microseconds => write!(f, "u"),
            Unit::Milliseconds => write!(f, "m"),
            Unit::Units => write!(f, ""),
        }
    }
}

impl fmt::Display for RepeatMarker {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RepeatMarker::Any => write!(f, "*"),
            RepeatMarker::OneOrMore => write!(f, "+"),
            RepeatMarker::Count(n) => write!(f, "{n}"),
            RepeatMarker::CountOrMore(n) => write!(f, "{n}+"),
        }
    }
}

impl Expression {
    /// Post-order visit all nodes in an expression
    pub fn visit<'a, T, F>(&'a self, ctx: &mut T, repeats: bool, visit: &F)
    where
        F: Fn(&'a Expression, &mut T),
    {
        match self {
            Expression::Complement(expr)
            | Expression::Not(expr)
            | Expression::Negative(expr)
            | Expression::BitCount(expr)
            | Expression::BitReverse(expr, _, _)
            | Expression::Log2(expr)
            | Expression::Assignment(_, expr) => {
                expr.visit(ctx, repeats, visit);
            }
            Expression::Add(left, right)
            | Expression::Subtract(left, right)
            | Expression::Multiply(left, right)
            | Expression::Divide(left, right)
            | Expression::Modulo(left, right)
            | Expression::Power(left, right)
            | Expression::ShiftLeft(left, right)
            | Expression::ShiftRight(left, right)
            | Expression::BitwiseAnd(left, right)
            | Expression::BitwiseOr(left, right)
            | Expression::BitwiseXor(left, right)
            | Expression::More(left, right)
            | Expression::MoreEqual(left, right)
            | Expression::Less(left, right)
            | Expression::LessEqual(left, right)
            | Expression::Equal(left, right)
            | Expression::NotEqual(left, right)
            | Expression::And(left, right)
            | Expression::Or(left, right) => {
                left.visit(ctx, repeats, visit);
                right.visit(ctx, repeats, visit);
            }
            Expression::Conditional(cond, left, right) => {
                cond.visit(ctx, repeats, visit);
                left.visit(ctx, repeats, visit);
                right.visit(ctx, repeats, visit);
            }
            Expression::BitField {
                value,
                length,
                skip,
                ..
            } => {
                value.visit(ctx, repeats, visit);
                length.visit(ctx, repeats, visit);
                if let Some(skip) = skip {
                    skip.visit(ctx, repeats, visit);
                }
            }
            Expression::InfiniteBitField { value, skip } => {
                value.visit(ctx, repeats, visit);
                skip.visit(ctx, repeats, visit);
            }
            Expression::List(list) => {
                for e in list {
                    e.visit(ctx, repeats, visit);
                }
            }
            Expression::Variation(variants) => {
                for list in variants {
                    for e in list {
                        e.visit(ctx, repeats, visit);
                    }
                }
            }
            Expression::Stream(stream) => {
                if !repeats {
                    for bit_spec in &stream.bit_spec {
                        bit_spec.visit(ctx, repeats, visit);
                    }
                    for e in &stream.stream {
                        e.visit(ctx, repeats, visit);
                    }
                } else {
                    let rep = match stream.repeat {
                        Some(RepeatMarker::Count(n)) => n,
                        // FIXME: This should be None, other repeats should not occur but they do
                        _ => 1,
                    };

                    for _ in 0..rep {
                        for bit_spec in &stream.bit_spec {
                            bit_spec.visit(ctx, repeats, visit);
                        }
                        for e in &stream.stream {
                            e.visit(ctx, repeats, visit);
                        }
                    }
                };
            }
            Expression::Number(..)
            | Expression::Identifier(..)
            | Expression::FlashConstant(..)
            | Expression::FlashIdentifier(..)
            | Expression::GapConstant(..)
            | Expression::GapIdentifier(..)
            | Expression::ExtentConstant(..)
            | Expression::ExtentIdentifier(..) => (),
        }
        visit(self, ctx);
    }

    /// Evaluate an bit field
    pub fn bitfield(&self, vars: &Vartable) -> Result<(i64, i64), String> {
        match self {
            Expression::BitField {
                value,
                reverse,
                length,
                skip,
            } => {
                let mut value = value.eval(vars)?;

                if let Some(skip) = skip {
                    value = value.wrapping_shr(skip.eval(vars)? as u32);
                }

                let length = length.eval(vars)?;

                if *reverse {
                    value = value.reverse_bits().rotate_left(length as u32);
                }

                if !(0..64).contains(&length) {
                    return Err(format!("bitfields of {length} not supported"));
                }

                if (0..63).contains(&length) {
                    value &= (1 << length) - 1;
                }

                Ok((value, length))
            }
            _ => Err("not a bitfield".into()),
        }
    }

    /// Evaluate an arithmetic expression
    pub fn eval(&self, vars: &Vartable) -> Result<i64, String> {
        match self {
            Expression::Number(n) => Ok(*n),
            Expression::Identifier(id) => vars.get(id),
            Expression::Negative(e) => {
                let v = e.eval(vars)?;

                Ok(-v)
            }
            Expression::Complement(e) => {
                let v = e.eval(vars)?;

                Ok(!v)
            }
            Expression::Log2(e) => {
                let v = e.eval(vars)?;

                let i = v.ilog2();

                if (1 << i) == v {
                    Ok(i.into())
                } else {
                    Ok(0)
                }
            }
            Expression::Add(l, r) => {
                let l_val = l.eval(vars)?;
                let r_val = r.eval(vars)?;

                Ok(l_val.overflowing_add(r_val).0)
            }
            Expression::Subtract(l, r) => {
                let l_val = l.eval(vars)?;
                let r_val = r.eval(vars)?;

                Ok(l_val.overflowing_sub(r_val).0)
            }
            Expression::Multiply(l, r) => {
                let l_val = l.eval(vars)?;
                let r_val = r.eval(vars)?;

                Ok(l_val.overflowing_mul(r_val).0)
            }
            Expression::Divide(l, r) => {
                let l_val = l.eval(vars)?;
                let r_val = r.eval(vars)?;

                if r_val == 0 {
                    return Err("divide by zero".to_string());
                }

                Ok(l_val / r_val)
            }
            Expression::Modulo(l, r) => {
                let l_val = l.eval(vars)?;
                let r_val = r.eval(vars)?;

                if r_val == 0 {
                    return Err("divide by zero".to_string());
                }

                Ok(l_val % r_val)
            }
            Expression::BitwiseAnd(l, r) => {
                let l_val = l.eval(vars)?;
                let r_val = r.eval(vars)?;

                Ok(l_val & r_val)
            }
            Expression::BitwiseOr(l, r) => {
                let l_val = l.eval(vars)?;
                let r_val = r.eval(vars)?;

                Ok(l_val | r_val)
            }
            Expression::BitwiseXor(l, r) => {
                let l_val = l.eval(vars)?;
                let r_val = r.eval(vars)?;

                Ok(l_val ^ r_val)
            }
            Expression::Power(l, r) => {
                let l_val = l.eval(vars)?;
                let r_val = r.eval(vars)?;

                if r_val < 0 {
                    return Err("power to negative not supported".to_string());
                }

                Ok(l_val.overflowing_pow(r_val as u32).0)
            }
            Expression::BitCount(e) => {
                let val = e.eval(vars)?;

                Ok(val.count_ones().into())
            }
            Expression::BitReverse(e, count, skip) => {
                let val = e.eval(vars)?;

                let mut new_val = 0;

                for i in 0..*count {
                    if (val & (1 << (i + skip))) != 0 {
                        new_val |= 1 << (skip + count - 1 - i);
                    }
                }

                Ok(new_val)
            }
            Expression::ShiftLeft(value, r) => {
                let value = value.eval(vars)?;
                let r = r.eval(vars)?;

                Ok(value.wrapping_shl(r as u32))
            }
            Expression::ShiftRight(value, r) => {
                let value = value.eval(vars)?;
                let r = r.eval(vars)?;

                Ok(value.wrapping_shr(r as u32))
            }
            Expression::NotEqual(left, right) => {
                let left = left.eval(vars)?;
                let right = right.eval(vars)?;

                Ok((left != right) as i64)
            }
            Expression::Equal(left, right) => {
                let left = left.eval(vars)?;
                let right = right.eval(vars)?;

                Ok((left == right) as i64)
            }
            Expression::More(left, right) => {
                let left = left.eval(vars)?;
                let right = right.eval(vars)?;

                Ok((left > right) as i64)
            }
            Expression::MoreEqual(left, right) => {
                let left = left.eval(vars)?;
                let right = right.eval(vars)?;

                Ok((left >= right) as i64)
            }
            Expression::Less(left, right) => {
                let left = left.eval(vars)?;
                let right = right.eval(vars)?;

                Ok((left < right) as i64)
            }
            Expression::LessEqual(left, right) => {
                let left = left.eval(vars)?;
                let right = right.eval(vars)?;

                Ok((left <= right) as i64)
            }
            Expression::BitField { .. } => Ok(self.bitfield(vars)?.0),
            Expression::InfiniteBitField { value, skip } => {
                let mut b = value.eval(vars)?;

                b = b.wrapping_shr(skip.eval(vars)? as u32);

                Ok(b)
            }
            Expression::Not(expr) => {
                let v = expr.eval(vars)?;

                Ok((v == 0) as i64)
            }
            Expression::And(left, right) => {
                let left = left.eval(vars)?;

                if left == 0 {
                    Ok(0)
                } else {
                    right.eval(vars)
                }
            }
            Expression::Or(left, right) => {
                let left = left.eval(vars)?;

                if left != 0 {
                    Ok(left)
                } else {
                    right.eval(vars)
                }
            }
            Expression::Conditional(cond, left, right) => {
                let cond = cond.eval(vars)?;

                if cond != 0 {
                    left.eval(vars)
                } else {
                    right.eval(vars)
                }
            }
            Expression::FlashConstant(..)
            | Expression::FlashIdentifier(..)
            | Expression::GapConstant(..)
            | Expression::GapIdentifier(..)
            | Expression::ExtentConstant(..)
            | Expression::ExtentIdentifier(..)
            | Expression::Assignment(..)
            | Expression::List(..)
            | Expression::Variation(..)
            | Expression::Stream(..) => unreachable!(),
        }
    }
}

/// Clone a reference counted expression with a filter
pub(crate) fn clone_filter<F>(expr: &Rc<Expression>, filter: &F) -> Option<Rc<Expression>>
where
    F: Fn(&Rc<Expression>) -> Option<Rc<Expression>>,
{
    macro_rules! unary {
        ($expr:expr, $ty:ident) => {{
            let expr1 = clone_filter($expr, filter);

            if expr1.is_some() {
                let expr = expr1.unwrap_or_else(|| $expr.clone());
                let new_expr = Rc::new(Expression::$ty(expr));
                let filtered = filter(&new_expr);

                if filtered.is_some() {
                    filtered
                } else {
                    Some(new_expr)
                }
            } else {
                filter(expr)
            }
        }};
    }

    macro_rules! binary {
        ($left:expr, $right:expr, $ty:ident) => {{
            let left1 = clone_filter($left, filter);
            let right1 = clone_filter($right, filter);

            if left1.is_some() || right1.is_some() {
                let left = left1.unwrap_or_else(|| $left.clone());
                let right = right1.unwrap_or_else(|| $right.clone());
                let new_expr = Rc::new(Expression::$ty(left, right));
                let filtered = filter(&new_expr);

                if filtered.is_some() {
                    filtered
                } else {
                    Some(new_expr)
                }
            } else {
                filter(expr)
            }
        }};
    }

    match expr.as_ref() {
        // unary
        Expression::Complement(expr) => unary!(expr, Complement),
        Expression::Not(expr) => unary!(expr, Not),
        Expression::BitCount(expr) => unary!(expr, BitCount),
        Expression::BitReverse(expr, count, skip) => {
            let expr1 = clone_filter(expr, filter);

            if expr1.is_some() {
                let expr = expr1.unwrap_or_else(|| expr.clone());
                let new_expr = Rc::new(Expression::BitReverse(expr, *count, *skip));
                let filtered = filter(&new_expr);

                if filtered.is_some() {
                    filtered
                } else {
                    Some(new_expr)
                }
            } else {
                filter(expr)
            }
        }

        Expression::Assignment(dest, expr) => {
            let expr1 = clone_filter(expr, filter);

            if expr1.is_some() {
                let expr = expr1.unwrap_or_else(|| expr.clone());
                let new_expr = Rc::new(Expression::Assignment(dest.to_owned(), expr));
                let filtered = filter(&new_expr);

                if filtered.is_some() {
                    filtered
                } else {
                    Some(new_expr)
                }
            } else {
                filter(expr)
            }
        }
        // binary
        Expression::Add(left, right) => binary!(left, right, Add),
        Expression::Subtract(left, right) => binary!(left, right, Subtract),
        Expression::Multiply(left, right) => binary!(left, right, Multiply),
        Expression::Divide(left, right) => binary!(left, right, Divide),
        Expression::Modulo(left, right) => binary!(left, right, Modulo),
        Expression::Power(left, right) => binary!(left, right, Power),

        Expression::ShiftLeft(left, right) => binary!(left, right, ShiftLeft),
        Expression::ShiftRight(left, right) => binary!(left, right, ShiftRight),
        Expression::BitwiseAnd(left, right) => binary!(left, right, BitwiseAnd),
        Expression::BitwiseOr(left, right) => binary!(left, right, BitwiseOr),
        Expression::BitwiseXor(left, right) => binary!(left, right, BitwiseXor),

        Expression::More(left, right) => binary!(left, right, More),
        Expression::MoreEqual(left, right) => binary!(left, right, MoreEqual),
        Expression::Less(left, right) => binary!(left, right, Less),
        Expression::LessEqual(left, right) => binary!(left, right, LessEqual),
        Expression::Equal(left, right) => binary!(left, right, Equal),
        Expression::NotEqual(left, right) => binary!(left, right, NotEqual),

        Expression::And(left, right) => binary!(left, right, And),
        Expression::Or(left, right) => binary!(left, right, Or),

        // Ternary
        Expression::Conditional(cond, left, right) => {
            let cond1 = clone_filter(cond, filter);
            let left1 = clone_filter(left, filter);
            let right1 = clone_filter(right, filter);

            if cond1.is_some() || left1.is_some() || right1.is_some() {
                let cond = cond1.unwrap_or_else(|| cond.clone());
                let left = left1.unwrap_or_else(|| left.clone());
                let right = right1.unwrap_or_else(|| right.clone());

                let new_expr = Rc::new(Expression::Conditional(cond, left, right));
                let filtered = filter(&new_expr);

                if filtered.is_some() {
                    filtered
                } else {
                    Some(new_expr)
                }
            } else {
                filter(expr)
            }
        }

        // others with sub expression
        Expression::BitField {
            value,
            reverse,
            length,
            skip: Some(skip),
        } => {
            let value1 = clone_filter(value, filter);
            let length1 = clone_filter(length, filter);
            let skip1 = clone_filter(skip, filter);

            if value1.is_some() || length1.is_some() || skip1.is_some() {
                let value = value1.unwrap_or_else(|| value.clone());
                let length = length1.unwrap_or_else(|| length.clone());
                let skip = Some(skip1.unwrap_or_else(|| skip.clone()));
                let reverse = *reverse;
                let new_expr = Rc::new(Expression::BitField {
                    value,
                    reverse,
                    length,
                    skip,
                });
                let filtered = filter(&new_expr);

                if filtered.is_some() {
                    filtered
                } else {
                    Some(new_expr)
                }
            } else {
                filter(expr)
            }
        }
        Expression::BitField {
            value,
            reverse,
            length,
            skip: None,
        } => {
            let value1 = clone_filter(value, filter);
            let length1 = clone_filter(length, filter);

            if value1.is_some() || length1.is_some() {
                let value = value1.unwrap_or_else(|| value.clone());
                let length = length1.unwrap_or_else(|| length.clone());
                let skip = None;
                let reverse = *reverse;

                let new_expr = Rc::new(Expression::BitField {
                    value,
                    reverse,
                    length,
                    skip,
                });
                let filtered = filter(&new_expr);

                if filtered.is_some() {
                    filtered
                } else {
                    Some(new_expr)
                }
            } else {
                filter(expr)
            }
        }

        Expression::InfiniteBitField { value, skip } => {
            let value1 = clone_filter(value, filter);
            let skip1 = clone_filter(skip, filter);

            if value1.is_some() || skip1.is_some() {
                let value = value1.unwrap_or_else(|| value.clone());
                let skip = skip1.unwrap_or_else(|| skip.clone());

                let new_expr = Rc::new(Expression::InfiniteBitField { value, skip });
                let filtered = filter(&new_expr);

                if filtered.is_some() {
                    filtered
                } else {
                    Some(new_expr)
                }
            } else {
                filter(expr)
            }
        }
        Expression::List(list) => {
            let new_list: Vec<Option<Rc<Expression>>> =
                list.iter().map(|expr| clone_filter(expr, filter)).collect();

            if new_list.iter().any(Option::is_some) {
                let list = new_list
                    .into_iter()
                    .enumerate()
                    .map(|(index, expr)| expr.unwrap_or_else(|| list[index].clone()))
                    .collect();

                let new_expr = Rc::new(Expression::List(list));
                let filtered = filter(&new_expr);

                if filtered.is_some() {
                    filtered
                } else {
                    Some(new_expr)
                }
            } else {
                filter(expr)
            }
        }
        Expression::Variation(list) => {
            let new_list = list
                .iter()
                .map(|list| {
                    list.iter()
                        .map(|expr| clone_filter(expr, filter))
                        .collect::<Vec<Option<Rc<Expression>>>>()
                })
                .collect::<Vec<Vec<Option<Rc<Expression>>>>>();

            if new_list.iter().flatten().any(Option::is_some) {
                let list = new_list
                    .into_iter()
                    .enumerate()
                    .map(|(index0, variant)| {
                        variant
                            .into_iter()
                            .enumerate()
                            .map(|(index1, expr)| {
                                expr.unwrap_or_else(|| list[index0][index1].clone())
                            })
                            .collect()
                    })
                    .collect();

                let new_expr = Rc::new(Expression::Variation(list));
                let filtered = filter(&new_expr);

                if filtered.is_some() {
                    filtered
                } else {
                    Some(new_expr)
                }
            } else {
                filter(expr)
            }
        }
        Expression::Stream(ir_stream) => {
            let new_bit_spec: Vec<Option<Rc<Expression>>> = ir_stream
                .bit_spec
                .iter()
                .map(|expr| clone_filter(expr, filter))
                .collect();

            let new_stream: Vec<Option<Rc<Expression>>> = ir_stream
                .stream
                .iter()
                .map(|expr| clone_filter(expr, filter))
                .collect();
            if new_bit_spec.iter().any(Option::is_some) || new_stream.iter().any(Option::is_some) {
                let bit_spec = new_bit_spec
                    .into_iter()
                    .enumerate()
                    .map(|(index, expr)| expr.unwrap_or_else(|| ir_stream.bit_spec[index].clone()))
                    .collect();

                let stream = new_stream
                    .into_iter()
                    .enumerate()
                    .map(|(index, expr)| expr.unwrap_or_else(|| ir_stream.stream[index].clone()))
                    .collect();

                let new_expr = Rc::new(Expression::Stream(Stream {
                    bit_spec,
                    stream,
                    repeat: ir_stream.repeat.clone(),
                }));

                let filtered = filter(&new_expr);

                if filtered.is_some() {
                    filtered
                } else {
                    Some(new_expr)
                }
            } else {
                filter(expr)
            }
        }
        _ => filter(expr),
    }
}

#[test]
fn clone1() {
    let expr = Rc::new(Expression::Add(
        Rc::new(Expression::Identifier("B".to_owned())),
        Rc::new(Expression::Identifier("S".to_owned())),
    ));

    let expr2 = clone_filter(&expr, &|expr: &Rc<Expression>| match expr.as_ref() {
        Expression::Identifier(var) if var == "S" => Some(Rc::new(Expression::Number(8))),
        _ => None,
    })
    .unwrap();

    assert_eq!(format!("{expr}"), "(B + S)");
    assert_eq!(format!("{expr2}"), "(B + 8)");
}
