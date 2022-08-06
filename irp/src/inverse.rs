use super::{
    build_nfa::{gen_mask, Action, Builder},
    Expression,
};
use std::rc::Rc;

/// Calculate inverse expression. For example:
/// X=~(B-1) (B) => ~X=B-1 => ~X+1=B
/// X=(102+B)-C (B) => X+C=102+B => X+C-102=B
impl<'a> Builder<'a> {
    pub fn expression_bits(&self, expr: &Rc<Expression>) -> Option<i64> {
        // println!("expr_bits:{}", expr);
        match expr.as_ref() {
            Expression::Identifier(id) => {
                if let Some(param) = self.irp.parameters.iter().find(|param| &param.name == id) {
                    self.param_to_mask(param).ok()
                } else {
                    None
                }
            }
            Expression::BitwiseOr(left, right) | Expression::BitwiseXor(left, right) => {
                if let (Some(left), Some(right)) =
                    (self.expression_bits(left), self.expression_bits(right))
                {
                    Some(left | right)
                } else {
                    None
                }
            }
            Expression::BitwiseAnd(left, right) => {
                if let (Some(left), Some(right)) =
                    (self.expression_bits(left), self.expression_bits(right))
                {
                    Some(left & right)
                } else {
                    None
                }
            }
            Expression::ShiftLeft(left, right) => {
                if let (Expression::Number(no), Some(mask)) =
                    (right.as_ref(), self.expression_bits(left))
                {
                    Some(mask << no)
                } else {
                    None
                }
            }
            Expression::ShiftRight(left, right) => {
                if let (Expression::Number(no), Some(mask)) =
                    (right.as_ref(), self.expression_bits(left))
                {
                    Some(mask >> no)
                } else {
                    None
                }
            }
            Expression::BitField { length, .. } => {
                let length = if let Expression::Number(length) = self.const_folding(length).as_ref()
                {
                    *length
                } else {
                    return None;
                };

                Some(gen_mask(length))
            }
            Expression::Add(left, right) => {
                if let (Some(left), Some(right)) =
                    (self.expression_bits(left), self.expression_bits(right))
                {
                    let mut mask = left | right;
                    mask |= (mask as u64).next_power_of_two() as i64;

                    Some(mask)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub fn inverse(
        &self,
        left: Rc<Expression>,
        right: Rc<Expression>,
        var: &str,
    ) -> Option<(Rc<Expression>, Vec<Action>, Option<i64>)> {
        // println!("inverse left:{} right:{} var:{}", left, right, var);
        match right.as_ref() {
            Expression::Identifier(id) if id == var => Some((left, vec![], None)),
            Expression::Complement(expr) => {
                self.inverse(Rc::new(Expression::Complement(left)), expr.clone(), var)
            }
            Expression::Negative(expr) => {
                self.inverse(Rc::new(Expression::Negative(left)), expr.clone(), var)
            }
            Expression::Not(expr) => {
                self.inverse(Rc::new(Expression::Not(left)), expr.clone(), var)
            }
            Expression::Add(left1, right1) => {
                if let (Some(left_bits), Some(right_bits)) =
                    (self.expression_bits(left1), self.expression_bits(right1))
                {
                    // if there is no overlap between the bits.. (must be broken)
                    if (left_bits & right_bits) == 0 {
                        let skip = left_bits.trailing_zeros();
                        let length = i64::BITS - left_bits.leading_zeros() - skip;

                        //println!("mask:{} {} {}", left_bits, skip, length);
                        let left2 = Rc::new(Expression::BitField {
                            value: left.clone(),
                            reverse: false,
                            length: Rc::new(Expression::Number(length.into())),
                            skip: Some(Rc::new(Expression::Number(skip.into()))),
                        });
                        let right2 = if skip > 0 {
                            Rc::new(Expression::ShiftRight(
                                left1.clone(),
                                Rc::new(Expression::Number(skip.into())),
                            ))
                        } else {
                            left1.clone()
                        };

                        //println!("left:{} {}", left, left1);
                        let v = self.inverse(left2, right2, var);

                        if v.is_some() {
                            return v;
                        }
                        //println!("left:{} {}", left, right1);

                        let skip = right_bits.trailing_zeros();
                        let length = i64::BITS - right_bits.leading_zeros() - skip;

                        //println!("mask:{} {} {}", right_bits, skip, length);
                        let left2 = Rc::new(Expression::BitField {
                            value: left.clone(),
                            reverse: false,
                            length: Rc::new(Expression::Number(length.into())),
                            skip: Some(Rc::new(Expression::Number(skip.into()))),
                        });
                        let right2 = if skip > 0 {
                            Rc::new(Expression::ShiftRight(
                                left1.clone(),
                                Rc::new(Expression::Number(skip.into())),
                            ))
                        } else {
                            left1.clone()
                        };

                        let v = self.inverse(left2, right2, var);

                        if v.is_some() {
                            return v;
                        }
                    }
                }

                let left2 = self.inverse(
                    Rc::new(Expression::Subtract(left.clone(), right1.clone())),
                    left1.clone(),
                    var,
                );

                if left2.is_some() {
                    left2
                } else {
                    self.inverse(
                        Rc::new(Expression::Subtract(left, left1.clone())),
                        right1.clone(),
                        var,
                    )
                }
            }
            Expression::Subtract(left1, right1) => {
                let left2 = self.inverse(
                    Rc::new(Expression::Add(left.clone(), right1.clone())),
                    left1.clone(),
                    var,
                );

                if left2.is_some() {
                    left2
                } else {
                    self.inverse(
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
                let left2 = self.inverse(
                    Rc::new(Expression::Divide(left.clone(), right1.clone())),
                    left1.clone(),
                    var,
                );

                if left2.is_some() {
                    left2
                } else {
                    self.inverse(
                        Rc::new(Expression::Divide(left, left1.clone())),
                        right1.clone(),
                        var,
                    )
                }
            }
            Expression::ShiftRight(left1, right1) => self.inverse(
                Rc::new(Expression::ShiftLeft(left, right1.clone())),
                left1.clone(),
                var,
            ),
            Expression::ShiftLeft(left1, right1) => {
                if let Expression::Number(shift) = self.const_folding(right1).as_ref() {
                    let minimum = 1i64 << *shift;

                    match left.as_ref() {
                        Expression::Add(left2, right2)
                        | Expression::Subtract(left2, right2)
                        | Expression::BitwiseOr(left2, right2)
                        | Expression::BitwiseAnd(left2, right2)
                        | Expression::BitwiseXor(left2, right2) => {
                            if let Some(left_bits) = self.expression_bits(left2) {
                                // println!("left_bits:{:b} {}", left_bits, left2);
                                if left_bits < minimum {
                                    // left term can be eleminated
                                    return if matches!(left.as_ref(), Expression::Subtract(..)) {
                                        self.inverse(
                                            Rc::new(Expression::ShiftRight(
                                                Rc::new(Expression::Negative(right2.clone())),
                                                right1.clone(),
                                            )),
                                            left1.clone(),
                                            var,
                                        )
                                    } else {
                                        self.inverse(
                                            Rc::new(Expression::ShiftRight(
                                                right2.clone(),
                                                right1.clone(),
                                            )),
                                            left1.clone(),
                                            var,
                                        )
                                    };
                                }
                            }

                            if let Some(right_bits) = self.expression_bits(right2) {
                                // println!("right_bits:{:b} {}", right_bits, right2);
                                if right_bits < minimum {
                                    // right term can be eleminated
                                    return self.inverse(
                                        Rc::new(Expression::ShiftRight(
                                            left2.clone(),
                                            right1.clone(),
                                        )),
                                        left1.clone(),
                                        var,
                                    );
                                }
                            }
                        }
                        _ => (),
                    }
                }

                self.inverse(
                    Rc::new(Expression::ShiftRight(left, right1.clone())),
                    left1.clone(),
                    var,
                )
            }
            Expression::Divide(left1, right1) => {
                let left2 = self.inverse(
                    Rc::new(Expression::Multiply(left.clone(), right1.clone())),
                    left1.clone(),
                    var,
                );

                if left2.is_some() {
                    left2
                } else {
                    self.inverse(
                        Rc::new(Expression::Divide(left1.clone(), left)),
                        right1.clone(),
                        var,
                    )
                }
            }
            Expression::BitwiseXor(left1, right1) => {
                let left2 = self.inverse(
                    Rc::new(Expression::BitwiseXor(left.clone(), right1.clone())),
                    left1.clone(),
                    var,
                );

                if left2.is_some() {
                    left2
                } else {
                    self.inverse(
                        Rc::new(Expression::BitwiseXor(left, left1.clone())),
                        right1.clone(),
                        var,
                    )
                }
            }
            Expression::Power(left1, right1) => {
                if matches!(left1.as_ref(), Expression::Number(2)) {
                    if let Some(mut res) =
                        self.inverse(Rc::new(Expression::Log2(left.clone())), right1.clone(), var)
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
}

#[test]
fn inverse1() {
    use crate::Irp;

    let irp = Irp::parse(
        "{36k,msb,889}<1,-1|-1,1>((1,~F:1:6,T:1,D:5,F:6,^114m)*,T=1-T)[D:0..31,F:0..127,T@:0..1=0]",
    )
    .unwrap();

    let builder = Builder::new(&irp);
    // first
    let left = Rc::new(Expression::Identifier("X".to_owned()));

    let right = Rc::new(Expression::Complement(Rc::new(Expression::Subtract(
        Rc::new(Expression::Identifier("B".to_owned())),
        Rc::new(Expression::Number(1)),
    ))));

    let inv = builder.inverse(left.clone(), right.clone(), "B").unwrap();

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

    let inv = builder.inverse(left.clone(), right.clone(), "B").unwrap();

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

    let inv = builder.inverse(left.clone(), right.clone(), "B").unwrap();

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

    let inv = builder.inverse(left.clone(), right.clone(), "B").unwrap();

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

    let inv = builder.inverse(left.clone(), right.clone(), "B").unwrap();

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

    let inv = builder.inverse(left.clone(), right.clone(), "B").unwrap();

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

    let inv = builder.inverse(left.clone(), right.clone(), "B").unwrap();

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

    let inv = builder.inverse(left.clone(), right.clone(), "B").unwrap();

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

    let inv = builder.inverse(left.clone(), right.clone(), "B").unwrap();

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

    let inv = builder.inverse(left.clone(), right.clone(), "B").unwrap();

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

    let inv = builder.inverse(left.clone(), right.clone(), "B").unwrap();

    assert_eq!(format!("{}", left), "X");
    assert_eq!(format!("{}", right), "(2 ** B)");
    assert_eq!(format!("{}", inv.0), "LOG2(X)");
    assert_eq!(inv.1, vec![Action::AssertEq { left, right }]);
    assert_eq!(inv.2, None);

    // nothing to do
    let left = Rc::new(Expression::Identifier("X".to_owned()));
    let right = Rc::new(Expression::Identifier("B".to_owned()));

    let inv = builder.inverse(left.clone(), right.clone(), "B").unwrap();

    assert_eq!(format!("{}", left), "X");
    assert_eq!(format!("{}", right), "B");
    assert_eq!(format!("{}", inv.0), "X");
    assert_eq!(inv.1.len(), 0);
    assert_eq!(inv.2, None);
}

#[test]
fn inverse2() {
    use crate::Irp;

    let irp = Irp::parse(
        "{36k,msb,889}<1,-1|-1,1>((1,~F:1:6,T:1,D:5,F:6,^114m)*,T=1-T)[D:0..31,F:0..127,T@:0..1=0]",
    )
    .unwrap();

    let builder = Builder::new(&irp);
    // first
    let left = Rc::new(Expression::Identifier("X".to_owned()));

    let right = Rc::new(Expression::Add(
        Rc::new(Expression::Multiply(
            Rc::new(Expression::Identifier("D".to_owned())),
            Rc::new(Expression::Number(16)),
        )),
        Rc::new(Expression::Identifier("T".to_owned())),
    ));

    assert_eq!(format!("{}", left), "X");
    assert_eq!(format!("{}", right), "((D * 16) + T)");
    let inv = builder
        .inverse(left, builder.const_folding(&right), "D")
        .unwrap();
    assert_eq!(format!("{}", inv.0), "((X:5:4 << 4) >> 4)");
    assert_eq!(inv.1.len(), 0);
    assert_eq!(inv.2, None);

    let left = Rc::new(Expression::Identifier("X".to_owned()));

    let right = Rc::new(Expression::Add(
        Rc::new(Expression::Multiply(
            Rc::new(Expression::Identifier("D".to_owned())),
            Rc::new(Expression::Number(8)),
        )),
        Rc::new(Expression::BitField {
            value: Rc::new(Expression::Identifier("D".to_owned())),
            reverse: false,
            length: Rc::new(Expression::Number(3)),
            skip: Some(Rc::new(Expression::Number(8))),
        }),
    ));

    assert_eq!(format!("{}", left), "X");
    assert_eq!(format!("{}", right), "((D * 8) + D:3:8)");
    let inv = builder
        .inverse(left, builder.const_folding(&right), "D")
        .unwrap();
    assert_eq!(format!("{}", inv.0), "((X:5:3 << 3) >> 3)");
    assert_eq!(inv.1.len(), 0);
    assert_eq!(inv.2, None);
}
