use super::{Expression, Vartable};
use std::fmt;

impl fmt::Display for Expression {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Expression::Number(v) => write!(f, "{}", v),
            Expression::Identifier(id) => write!(f, "{}", id),
            Expression::Add(left, right) => write!(f, "({} + {})", left, right),
            Expression::Subtract(left, right) => write!(f, "({} - {})", left, right),
            Expression::Multiply(left, right) => write!(f, "({} * {})", left, right),
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
