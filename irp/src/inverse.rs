use super::build_nfa::{gen_mask, Action};
use super::Expression;
use std::rc::Rc;

/// Calculate inverse expression. For example:
/// X=~(B-1) (B) => ~X=B-1 => ~X+1=B
/// X=(102+B)-C (B) => X+C=102+B => X+C-102=B
pub(crate) fn inverse(
    left: Rc<Expression>,
    right: Rc<Expression>,
    var: &str,
) -> Option<(Rc<Expression>, Vec<Action>, Option<i64>)> {
    match right.as_ref() {
        Expression::Identifier(id) if id == var => Some((left, vec![], None)),
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
        Expression::BitwiseXor(left1, right1) => {
            let left2 = inverse(
                Rc::new(Expression::BitwiseXor(left.clone(), right1.clone())),
                left1.clone(),
                var,
            );

            if left2.is_some() {
                left2
            } else {
                inverse(
                    Rc::new(Expression::BitwiseXor(left, left1.clone())),
                    right1.clone(),
                    var,
                )
            }
        }
        Expression::Power(left1, right1) => {
            if matches!(left1.as_ref(), Expression::Number(2)) {
                if let Some(mut res) =
                    inverse(Rc::new(Expression::Log2(left.clone())), right1.clone(), var)
                {
                    res.1.push(Action::AssertEq { left, right });

                    Some(res)
                } else {
                    None
                }
            } else {
                None
            }
        }
        Expression::BitField {
            value,
            reverse: false,
            length,
            skip,
        } => {
            let mut complement = false;

            match value.as_ref() {
                Expression::Identifier(id) if id == var => (),
                Expression::Complement(expr) => {
                    complement = true;

                    match expr.as_ref() {
                        Expression::Identifier(id) if id == var => (),
                        _ => {
                            return None;
                        }
                    }
                }
                _ => {
                    return None;
                }
            }

            let length = if let Expression::Number(length) = length.as_ref() {
                *length
            } else {
                return None;
            };

            let skip = if let Some(skip) = skip {
                if let Expression::Number(skip) = skip.as_ref() {
                    *skip
                } else {
                    return None;
                }
            } else {
                0
            };

            let left = if complement {
                Rc::new(Expression::Complement(left))
            } else {
                left
            };

            Some((
                Rc::new(Expression::ShiftLeft(
                    Rc::new(Expression::BitwiseAnd(
                        left,
                        Rc::new(Expression::Number(gen_mask(length))),
                    )),
                    Rc::new(Expression::Number(skip)),
                )),
                vec![],
                Some(gen_mask(length) << skip),
            ))
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
    assert_eq!(format!("{}", inv.0), "(~X + 1)");
    assert_eq!(inv.1.len(), 0);
    assert_eq!(inv.2, None);

    // second
    let left = Rc::new(Expression::Identifier("X".to_owned()));

    let right = Rc::new(Expression::Complement(Rc::new(Expression::Subtract(
        Rc::new(Expression::Number(1)),
        Rc::new(Expression::Identifier("B".to_owned())),
    ))));

    let inv = inverse(left.clone(), right.clone(), "B").unwrap();

    assert_eq!(format!("{}", left), "X");
    assert_eq!(format!("{}", right), "~(1 - B)");
    assert_eq!(format!("{}", inv.0), "-(~X - 1)");
    assert_eq!(inv.1.len(), 0);
    assert_eq!(inv.2, None);

    // third
    let left = Rc::new(Expression::Identifier("X".to_owned()));

    let right = Rc::new(Expression::Negative(Rc::new(Expression::Add(
        Rc::new(Expression::Identifier("B".to_owned())),
        Rc::new(Expression::Number(1)),
    ))));

    let inv = inverse(left.clone(), right.clone(), "B").unwrap();

    assert_eq!(format!("{}", left), "X");
    assert_eq!(format!("{}", right), "-(B + 1)");
    assert_eq!(format!("{}", inv.0), "(-X - 1)");
    assert_eq!(inv.1.len(), 0);
    assert_eq!(inv.2, None);

    // fourth
    let left = Rc::new(Expression::Identifier("X".to_owned()));

    let right = Rc::new(Expression::Negative(Rc::new(Expression::Add(
        Rc::new(Expression::Number(1)),
        Rc::new(Expression::Identifier("B".to_owned())),
    ))));

    let inv = inverse(left.clone(), right.clone(), "B").unwrap();

    assert_eq!(format!("{}", left), "X");
    assert_eq!(format!("{}", right), "-(1 + B)");
    assert_eq!(format!("{}", inv.0), "(-X - 1)");
    assert_eq!(inv.1.len(), 0);
    assert_eq!(inv.2, None);

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
    assert_eq!(format!("{}", inv.0), "-(X / 3)");
    assert_eq!(inv.1.len(), 0);
    assert_eq!(inv.2, None);

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
    assert_eq!(format!("{}", inv.0), "-(X / 3)");
    assert_eq!(inv.1.len(), 0);
    assert_eq!(inv.2, None);

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
    assert_eq!(format!("{}", inv.0), "-(X * 3)");
    assert_eq!(inv.1.len(), 0);
    assert_eq!(inv.2, None);

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
    assert_eq!(format!("{}", inv.0), "-(3 / X)");
    assert_eq!(inv.1.len(), 0);
    assert_eq!(inv.2, None);

    // xor
    let left = Rc::new(Expression::Identifier("X".to_owned()));

    let right = Rc::new(Expression::BitwiseXor(
        Rc::new(Expression::Number(3)),
        Rc::new(Expression::Negative(Rc::new(Expression::Identifier(
            "B".to_owned(),
        )))),
    ));

    let inv = inverse(left.clone(), right.clone(), "B").unwrap();

    assert_eq!(format!("{}", left), "X");
    assert_eq!(format!("{}", right), "(3 ^ -B)");
    assert_eq!(format!("{}", inv.0), "-(X ^ 3)");
    assert_eq!(inv.1.len(), 0);
    assert_eq!(inv.2, None);

    // bitfield
    let left = Rc::new(Expression::Identifier("X".to_owned()));

    let right = Rc::new(Expression::BitField {
        value: Rc::new(Expression::Identifier("B".to_owned())),
        reverse: false,
        length: Rc::new(Expression::Number(3)),
        skip: Some(Rc::new(Expression::Number(1))),
    });

    let inv = inverse(left.clone(), right.clone(), "B").unwrap();

    assert_eq!(format!("{}", left), "X");
    assert_eq!(format!("{}", right), "B:3:1");
    assert_eq!(format!("{}", inv.0), "((X & 7) << 1)");
    assert_eq!(inv.1.len(), 0);
    assert_eq!(inv.2, Some(0b1110));

    // 2**n
    let left = Rc::new(Expression::Identifier("X".to_owned()));

    let right = Rc::new(Expression::Power(
        Rc::new(Expression::Number(2)),
        Rc::new(Expression::Identifier("B".to_owned())),
    ));

    let inv = inverse(left.clone(), right.clone(), "B").unwrap();

    assert_eq!(format!("{}", left), "X");
    assert_eq!(format!("{}", right), "(2 ** B)");
    assert_eq!(format!("{}", inv.0), "LOG2(X)");
    assert_eq!(inv.1, vec![Action::AssertEq { left, right }]);
    assert_eq!(inv.2, None);

    // nothing to do
    let left = Rc::new(Expression::Identifier("X".to_owned()));
    let right = Rc::new(Expression::Identifier("B".to_owned()));

    let inv = inverse(left.clone(), right.clone(), "B").unwrap();

    assert_eq!(format!("{}", left), "X");
    assert_eq!(format!("{}", right), "B");
    assert_eq!(format!("{}", inv.0), "X");
    assert_eq!(inv.1.len(), 0);
    assert_eq!(inv.2, None);
}
