use super::{Expression, IrStream, Vartable};
use std::{fmt, rc::Rc};

impl fmt::Display for Expression {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Expression::Number(v) => write!(f, "{}", v),
            Expression::Identifier(id) => write!(f, "{}", id),
            Expression::Add(left, right) => write!(f, "({} + {})", left, right),
            Expression::Subtract(left, right) => write!(f, "({} - {})", left, right),
            Expression::Multiply(left, right) => write!(f, "({} * {})", left, right),
            Expression::Divide(left, right) => write!(f, "({} / {})", left, right),
            Expression::Power(left, right) => write!(f, "({} ** {})", left, right),
            Expression::Modulo(left, right) => write!(f, "({} % {})", left, right),
            Expression::BitwiseOr(left, right) => write!(f, "({} | {})", left, right),
            Expression::BitwiseAnd(left, right) => write!(f, "({} & {})", left, right),
            Expression::BitwiseXor(left, right) => write!(f, "({} ^ {})", left, right),
            Expression::ShiftLeft(left, right) => write!(f, "({} << {})", left, right),
            Expression::ShiftRight(left, right) => write!(f, "({} >> {})", left, right),

            Expression::Equal(left, right) => write!(f, "{} == {}", left, right),
            Expression::NotEqual(left, right) => write!(f, "{} != {}", left, right),
            Expression::More(left, right) => write!(f, "{} > {}", left, right),
            Expression::MoreEqual(left, right) => write!(f, "{} >= {}", left, right),
            Expression::Less(left, right) => write!(f, "{} < {}", left, right),
            Expression::LessEqual(left, right) => write!(f, "{} <= {}", left, right),

            Expression::Or(left, right) => write!(f, "({} || {})", left, right),
            Expression::And(left, right) => write!(f, "({} && {})", left, right),
            Expression::Ternary(cond, left, right) => {
                write!(f, "({} ? {} : {})", cond, left, right)
            }
            Expression::Complement(expr) => write!(f, "~{}", expr),
            Expression::Not(expr) => write!(f, "!{}", expr),
            Expression::Negative(expr) => write!(f, "-{}", expr),
            Expression::BitCount(expr) => write!(f, "#({})", expr),
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
                write!(f, "{}::{}", value, skip)
            }
            Expression::BitReverse(expr, count, skip) => {
                write!(f, "BITREV({},{},{})", expr, count, skip)
            }
            Expression::Assignment(name, expr) => write!(f, "{}={}", name, expr),
            Expression::List(list) => {
                write!(f, "(")?;
                let mut first = true;
                for expr in list {
                    if !first {
                        write!(f, ",")?;
                    }
                    write!(f, "{}", expr)?;
                    first = false;
                }
                write!(f, ")")
            }
            expr => write!(f, "{:?}", expr),
        }
    }
}

impl Expression {
    /// Post-order visit all nodes in an expression
    pub fn visit<T, F>(&self, ctx: &mut T, visit: &F)
    where
        F: Fn(&Expression, &mut T),
    {
        match self {
            Expression::Complement(expr)
            | Expression::Not(expr)
            | Expression::BitCount(expr)
            | Expression::BitReverse(expr, _, _)
            | Expression::Assignment(_, expr) => {
                expr.visit(ctx, visit);
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
                left.visit(ctx, visit);
                right.visit(ctx, visit);
            }
            Expression::Ternary(cond, left, right) => {
                cond.visit(ctx, visit);
                left.visit(ctx, visit);
                right.visit(ctx, visit);
            }
            Expression::BitField {
                value,
                length,
                skip,
                ..
            } => {
                value.visit(ctx, visit);
                length.visit(ctx, visit);
                if let Some(skip) = skip {
                    skip.visit(ctx, visit);
                }
            }
            Expression::InfiniteBitField { value, skip } => {
                value.visit(ctx, visit);
                skip.visit(ctx, visit);
            }
            Expression::List(list) => {
                for e in list {
                    e.visit(ctx, visit);
                }
            }
            Expression::Variation(variants) => {
                for list in variants {
                    for e in list {
                        e.visit(ctx, visit);
                    }
                }
            }
            Expression::Stream(stream) => {
                for bit_spec in &stream.bit_spec {
                    bit_spec.visit(ctx, visit);
                }
                for e in &stream.stream {
                    e.visit(ctx, visit);
                }
            }
            _ => (),
        }
        visit(self, ctx);
    }

    /// Evaluate an arithmetic expression
    pub fn eval(&self, vars: &Vartable) -> Result<(i64, u8), String> {
        match self {
            Expression::Number(n) => Ok((*n, 64)),
            Expression::Identifier(id) => vars.get(id),
            Expression::Negative(e) => {
                let (v, l) = e.eval(vars)?;

                Ok((-v, l))
            }
            Expression::Complement(e) => {
                let (v, l) = e.eval(vars)?;

                Ok((!v, l))
            }
            Expression::Add(l, r) => {
                let (l_val, l_len) = l.eval(vars)?;
                let (r_val, r_len) = r.eval(vars)?;

                Ok(((l_val + r_val), std::cmp::max(l_len, r_len)))
            }
            Expression::Subtract(l, r) => {
                let (l_val, l_len) = l.eval(vars)?;
                let (r_val, r_len) = r.eval(vars)?;

                Ok(((l_val - r_val), std::cmp::max(l_len, r_len)))
            }
            Expression::Multiply(l, r) => {
                let (l_val, l_len) = l.eval(vars)?;
                let (r_val, r_len) = r.eval(vars)?;

                Ok(((l_val * r_val), std::cmp::max(l_len, r_len)))
            }
            Expression::Divide(l, r) => {
                let (l_val, l_len) = l.eval(vars)?;
                let (r_val, r_len) = r.eval(vars)?;

                if r_val == 0 {
                    return Err("divide by zero".to_string());
                }

                Ok(((l_val / r_val), std::cmp::max(l_len, r_len)))
            }
            Expression::Modulo(l, r) => {
                let (l_val, l_len) = l.eval(vars)?;
                let (r_val, r_len) = r.eval(vars)?;

                if r_val == 0 {
                    return Err("divide by zero".to_string());
                }

                Ok(((l_val % r_val), std::cmp::max(l_len, r_len)))
            }
            Expression::BitwiseAnd(l, r) => {
                let (l_val, l_len) = l.eval(vars)?;
                let (r_val, r_len) = r.eval(vars)?;

                Ok(((l_val & r_val), std::cmp::max(l_len, r_len)))
            }
            Expression::BitwiseOr(l, r) => {
                let (l_val, l_len) = l.eval(vars)?;
                let (r_val, r_len) = r.eval(vars)?;

                Ok(((l_val | r_val), std::cmp::max(l_len, r_len)))
            }
            Expression::BitwiseXor(l, r) => {
                let (l_val, l_len) = l.eval(vars)?;
                let (r_val, r_len) = r.eval(vars)?;

                Ok(((l_val ^ r_val), std::cmp::max(l_len, r_len)))
            }
            Expression::Power(l, r) => {
                let (l_val, l_len) = l.eval(vars)?;
                let (r_val, _) = r.eval(vars)?;

                if r_val < 0 {
                    return Err("power to negative not supported".to_string());
                }

                Ok((l_val.pow(r_val as u32), l_len))
            }
            Expression::BitCount(e) => {
                let (mut val, len) = e.eval(vars)?;

                if len < 63 {
                    // mask off any unused bits
                    val &= (1 << len) - 1;
                }

                Ok((val.count_ones().into(), len))
            }
            Expression::BitReverse(e, count, skip) => {
                let (val, len) = e.eval(vars)?;

                let mut new_val = 0;

                for i in 0..*count {
                    if (val & (1 << (i + skip))) != 0 {
                        new_val |= 1 << (skip + count - 1 - i);
                    }
                }

                Ok((new_val, len))
            }
            Expression::ShiftLeft(value, r) => {
                let (value, len) = value.eval(vars)?;
                let (r, _) = r.eval(vars)?;

                Ok((value.wrapping_shl(r as u32), len))
            }
            Expression::ShiftRight(value, r) => {
                let (value, len) = value.eval(vars)?;
                let (r, _) = r.eval(vars)?;

                Ok((value.wrapping_shr(r as u32), len))
            }
            Expression::Equal(left, right) => {
                let (left, _) = left.eval(vars)?;
                let (right, _) = right.eval(vars)?;

                Ok(((left == right) as i64, 1))
            }
            Expression::More(left, right) => {
                let (left, _) = left.eval(vars)?;
                let (right, _) = right.eval(vars)?;

                Ok(((left > right) as i64, 1))
            }
            Expression::MoreEqual(left, right) => {
                let (left, _) = left.eval(vars)?;
                let (right, _) = right.eval(vars)?;

                Ok(((left >= right) as i64, 1))
            }
            Expression::Less(left, right) => {
                let (left, _) = left.eval(vars)?;
                let (right, _) = right.eval(vars)?;

                Ok(((left < right) as i64, 1))
            }
            Expression::LessEqual(left, right) => {
                let (left, _) = left.eval(vars)?;
                let (right, _) = right.eval(vars)?;

                Ok(((left <= right) as i64, 1))
            }
            Expression::BitField {
                value,
                reverse,
                length,
                skip,
            } => {
                let (mut b, _) = value.eval(vars)?;

                if let Some(skip) = skip {
                    b >>= skip.eval(vars)?.0;
                }

                let (l, _) = length.eval(vars)?;

                if *reverse {
                    b = b.reverse_bits().rotate_left(l as u32);
                }

                if l < 64 {
                    b &= (1 << l) - 1;
                }

                Ok((b, l as u8))
            }
            Expression::InfiniteBitField { value, skip } => {
                let (mut b, _) = value.eval(vars)?;

                b >>= skip.eval(vars)?.0;

                Ok((b, 8))
            }
            Expression::List(v) if v.len() == 1 => {
                let (v, l) = v[0].eval(vars)?;

                Ok((v, l))
            }
            _ => panic!("not implemented: {:?}", self),
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
        Expression::Ternary(cond, left, right) => {
            let cond1 = clone_filter(cond, filter);
            let left1 = clone_filter(left, filter);
            let right1 = clone_filter(right, filter);

            if cond1.is_some() || left1.is_some() || right1.is_some() {
                let cond = cond1.unwrap_or_else(|| cond.clone());
                let left = left1.unwrap_or_else(|| left.clone());
                let right = right1.unwrap_or_else(|| right.clone());

                let new_expr = Rc::new(Expression::Ternary(cond, left, right));
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

                let new_expr = Rc::new(Expression::Stream(IrStream {
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

    assert_eq!(format!("{}", expr), "(B + S)");
    assert_eq!(format!("{}", expr2), "(B + 8)");
}

/// Calculate inverse expression. For example:
/// X=~(B-1) (B) => ~X=B-1 => ~X+1=B
/// X=(102+B)-C (B) => X+C=102+B => X+C-102=B
pub(crate) fn inverse(
    left: Rc<Expression>,
    right: Rc<Expression>,
    var: &str,
) -> Option<Rc<Expression>> {
    match right.as_ref() {
        Expression::Identifier(id) if id == var => Some(left),
        Expression::Complement(expr) => {
            inverse(Rc::new(Expression::Complement(left)), expr.clone(), var)
        }
        Expression::Negative(expr) => {
            inverse(Rc::new(Expression::Negative(left)), expr.clone(), var)
        }
        Expression::Not(expr) => inverse(Rc::new(Expression::Not(left)), expr.clone(), var),
        Expression::Add(left1, right1) => {
            let left2 = inverse(
                Rc::new(Expression::Subtract(left.clone(), right1.clone())),
                left1.clone(),
                var,
            );

            if left2.is_some() {
                left2
            } else {
                inverse(
                    Rc::new(Expression::Subtract(left, left1.clone())),
                    right1.clone(),
                    var,
                )
            }
        }
        Expression::Subtract(left1, right1) => {
            let left2 = inverse(
                Rc::new(Expression::Add(left.clone(), right1.clone())),
                left1.clone(),
                var,
            );

            if left2.is_some() {
                left2
            } else {
                inverse(
                    Rc::new(Expression::Negative(Rc::new(Expression::Subtract(
                        left,
                        left1.clone(),
                    )))),
                    right1.clone(),
                    var,
                )
            }
        }
        Expression::Multiply(left1, right1) => {
            let left2 = inverse(
                Rc::new(Expression::Divide(left.clone(), right1.clone())),
                left1.clone(),
                var,
            );

            if left2.is_some() {
                left2
            } else {
                inverse(
                    Rc::new(Expression::Divide(left, left1.clone())),
                    right1.clone(),
                    var,
                )
            }
        }
        Expression::Divide(left1, right1) => {
            let left2 = inverse(
                Rc::new(Expression::Multiply(left.clone(), right1.clone())),
                left1.clone(),
                var,
            );

            if left2.is_some() {
                left2
            } else {
                inverse(
                    Rc::new(Expression::Divide(left1.clone(), left)),
                    right1.clone(),
                    var,
                )
            }
        }
        _ => None,
    }
}

#[test]
fn inverse1() {
    // first
    let left = Rc::new(Expression::Identifier("X".to_owned()));

    let right = Rc::new(Expression::Complement(Rc::new(Expression::Subtract(
        Rc::new(Expression::Identifier("B".to_owned())),
        Rc::new(Expression::Number(1)),
    ))));

    let inv = inverse(left.clone(), right.clone(), "B").unwrap();

    assert_eq!(format!("{}", left), "X");
    assert_eq!(format!("{}", right), "~(B - 1)");
    assert_eq!(format!("{}", inv), "(~X + 1)");

    // second
    let left = Rc::new(Expression::Identifier("X".to_owned()));

    let right = Rc::new(Expression::Complement(Rc::new(Expression::Subtract(
        Rc::new(Expression::Number(1)),
        Rc::new(Expression::Identifier("B".to_owned())),
    ))));

    let inv = inverse(left.clone(), right.clone(), "B").unwrap();

    assert_eq!(format!("{}", left), "X");
    assert_eq!(format!("{}", right), "~(1 - B)");
    assert_eq!(format!("{}", inv), "-(~X - 1)");

    // third
    let left = Rc::new(Expression::Identifier("X".to_owned()));

    let right = Rc::new(Expression::Negative(Rc::new(Expression::Add(
        Rc::new(Expression::Identifier("B".to_owned())),
        Rc::new(Expression::Number(1)),
    ))));

    let inv = inverse(left.clone(), right.clone(), "B").unwrap();

    assert_eq!(format!("{}", left), "X");
    assert_eq!(format!("{}", right), "-(B + 1)");
    assert_eq!(format!("{}", inv), "(-X - 1)");

    // fourth
    let left = Rc::new(Expression::Identifier("X".to_owned()));

    let right = Rc::new(Expression::Negative(Rc::new(Expression::Add(
        Rc::new(Expression::Number(1)),
        Rc::new(Expression::Identifier("B".to_owned())),
    ))));

    let inv = inverse(left.clone(), right.clone(), "B").unwrap();

    assert_eq!(format!("{}", left), "X");
    assert_eq!(format!("{}", right), "-(1 + B)");
    assert_eq!(format!("{}", inv), "(-X - 1)");

    // fifth
    let left = Rc::new(Expression::Identifier("X".to_owned()));

    let right = Rc::new(Expression::Multiply(
        Rc::new(Expression::Number(3)),
        Rc::new(Expression::Negative(Rc::new(Expression::Identifier(
            "B".to_owned(),
        )))),
    ));

    let inv = inverse(left.clone(), right.clone(), "B").unwrap();

    assert_eq!(format!("{}", left), "X");
    assert_eq!(format!("{}", right), "(3 * -B)");
    assert_eq!(format!("{}", inv), "-(X / 3)");

    // sixth
    let left = Rc::new(Expression::Identifier("X".to_owned()));

    let right = Rc::new(Expression::Multiply(
        Rc::new(Expression::Negative(Rc::new(Expression::Identifier(
            "B".to_owned(),
        )))),
        Rc::new(Expression::Number(3)),
    ));

    let inv = inverse(left.clone(), right.clone(), "B").unwrap();

    assert_eq!(format!("{}", left), "X");
    assert_eq!(format!("{}", right), "(-B * 3)");
    assert_eq!(format!("{}", inv), "-(X / 3)");

    // seventh
    let left = Rc::new(Expression::Identifier("X".to_owned()));

    let right = Rc::new(Expression::Divide(
        Rc::new(Expression::Negative(Rc::new(Expression::Identifier(
            "B".to_owned(),
        )))),
        Rc::new(Expression::Number(3)),
    ));

    let inv = inverse(left.clone(), right.clone(), "B").unwrap();

    assert_eq!(format!("{}", left), "X");
    assert_eq!(format!("{}", right), "(-B / 3)");
    assert_eq!(format!("{}", inv), "-(X * 3)");

    // 8th
    let left = Rc::new(Expression::Identifier("X".to_owned()));

    let right = Rc::new(Expression::Divide(
        Rc::new(Expression::Number(3)),
        Rc::new(Expression::Negative(Rc::new(Expression::Identifier(
            "B".to_owned(),
        )))),
    ));

    let inv = inverse(left.clone(), right.clone(), "B").unwrap();

    assert_eq!(format!("{}", left), "X");
    assert_eq!(format!("{}", right), "(3 / -B)");
    assert_eq!(format!("{}", inv), "-(3 / X)");

    // nothing to do
    let left = Rc::new(Expression::Identifier("X".to_owned()));
    let right = Rc::new(Expression::Identifier("B".to_owned()));

    let inv = inverse(left.clone(), right.clone(), "B").unwrap();

    assert_eq!(format!("{}", left), "X");
    assert_eq!(format!("{}", right), "B");
    assert_eq!(format!("{}", inv), "X");
}
