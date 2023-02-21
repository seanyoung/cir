use crate::{expression::clone_filter, Expression, Irp, RepeatMarker, Stream};
use std::rc::Rc;

#[derive(Debug)]
pub(crate) struct Variants {
    pub down: Option<Rc<Expression>>,
    pub repeat: Rc<Expression>,
    pub up: Option<Rc<Expression>>,
}

impl Irp {
    pub(crate) fn split_variants(&self) -> Result<Variants, String> {
        let expr = &self.stream;

        if let Expression::Stream(stream) = self.stream.as_ref() {
            for expr in &stream.bit_spec {
                // bitspec should never have repeats
                check_no_repeats(expr)?;
            }

            // do we have a top-level repeat?
            if expr.is_repeating() {
                expr.check_no_contained_repeats()?;

                let mut stream = stream.clone();

                stream.repeat = None;

                let variant_count = stream
                    .stream
                    .iter()
                    .filter_map(|e| {
                        if let Expression::Variation(list) = e.as_ref() {
                            Some(list.len())
                        } else {
                            None
                        }
                    })
                    .max();

                if let Some(variant_count) = variant_count {
                    let expr = Rc::new(expr.clone());

                    let variants: Vec<_> = (0..variant_count)
                        .map(|variant| {
                            clone_filter(&expr, &|e| {
                                if let Expression::Variation(list) = e.as_ref() {
                                    if let Some(variant) = list.get(variant) {
                                        if variant.len() == 1 {
                                            Some(variant[0].clone())
                                        } else {
                                            Some(Rc::new(Expression::List(variant.clone())))
                                        }
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            })
                            .unwrap()
                        })
                        .collect();

                    let up = if variants.len() == 3 {
                        if let Expression::Stream(stream) = variants[2].as_ref() {
                            let mut stream = stream.clone();
                            stream.repeat = None;
                            Some(Rc::new(Expression::Stream(stream)))
                        } else {
                            panic!("stream expected");
                        }
                    } else {
                        None
                    };

                    let repeat = variants[1].clone();

                    let down = if let Expression::Stream(stream) = variants[0].as_ref() {
                        let mut stream = stream.clone();
                        stream.repeat = None;
                        Some(Rc::new(Expression::Stream(stream)))
                    } else {
                        panic!("stream expected");
                    };

                    Ok(Variants { down, repeat, up })
                } else {
                    Ok(Variants {
                        down: None,
                        repeat: expr.clone(),
                        up: None,
                    })
                }
            } else {
                let mut down = Vec::new();
                let mut repeats = Vec::new();
                let mut up = Vec::new();

                let mut seen_repeat = None;

                for expr in &stream.stream {
                    expr.check_no_contained_repeats()?;

                    if let Expression::Stream(stream) = expr.as_ref() {
                        if stream.is_repeating() {
                            if seen_repeat.is_some() {
                                return Err("multiple repeat markers in IRP".into());
                            } else {
                                repeats = stream.stream.clone();
                                seen_repeat = stream.repeat.clone();
                                continue;
                            }
                        }
                    }

                    if seen_repeat.is_some() {
                        add_flatten(&mut up, expr);
                    } else {
                        add_flatten(&mut down, expr);
                    }
                }

                let down = if down.is_empty() || all_assignments(&down) {
                    if any_assignments(&repeats) {
                        Some(Rc::new(Expression::Stream(Stream {
                            repeat: None,
                            stream: repeats.clone(),
                            bit_spec: stream.bit_spec.clone(),
                        })))
                    } else {
                        None
                    }
                } else {
                    if repeats.is_empty() {
                        repeats = down.clone();
                    }

                    if match seen_repeat {
                        Some(RepeatMarker::OneOrMore) => true,
                        Some(RepeatMarker::CountOrMore(n)) => n > 0,
                        _ => false,
                    } {
                        // if both the original stream and the added stream contain an extend,
                        // puth the original in stream
                        if has_extent(&repeats) && has_extent(&down) {
                            down = vec![
                                Rc::new(Expression::Stream(Stream {
                                    bit_spec: Vec::new(),
                                    stream: down,
                                    repeat: None,
                                })),
                                Rc::new(Expression::Stream(Stream {
                                    bit_spec: Vec::new(),
                                    stream: repeats.clone(),
                                    repeat: None,
                                })),
                            ];
                        } else {
                            for expr in &repeats {
                                add_flatten(&mut down, expr);
                            }
                        }
                    }

                    Some(Rc::new(Expression::Stream(Stream {
                        repeat: None,
                        stream: down,
                        bit_spec: stream.bit_spec.clone(),
                    })))
                };

                let up = if up.is_empty() || all_assignments(&up) {
                    None
                } else {
                    Some(Rc::new(Expression::Stream(Stream {
                        repeat: None,
                        stream: up,
                        bit_spec: stream.bit_spec.clone(),
                    })))
                };

                let repeat = Rc::new(Expression::Stream(Stream {
                    repeat: seen_repeat,
                    stream: repeats,
                    bit_spec: stream.bit_spec.clone(),
                }));

                Ok(Variants { down, repeat, up })
            }
        } else {
            Err("expected stream expression".into())
        }
    }

    pub(crate) fn split_variants_encode(&self) -> Result<Variants, String> {
        let expr = &self.stream;

        if let Expression::Stream(stream) = self.stream.as_ref() {
            for expr in &stream.bit_spec {
                // bitspec should never have repeats
                check_no_repeats(expr)?;
            }

            // do we have a top-level repeat?
            if expr.is_repeating() {
                expr.check_no_contained_repeats()?;

                let mut stream = stream.clone();

                stream.repeat = None;

                let mut ctx = Vec::new();

                expr.visit(&mut ctx, &|expr, ctx| {
                    if let Expression::Variation(list) = &expr {
                        ctx.push((list[0].is_empty(), list.len()));
                    };
                });

                let mut down_variant_empty = true;

                let variant_count = ctx
                    .into_iter()
                    .map(|(down_empty, count)| {
                        if !down_empty {
                            down_variant_empty = false;
                        }

                        count
                    })
                    .max();

                if let Some(variant_count) = variant_count {
                    let variants: Vec<_> = (0..variant_count)
                        .map(|variant| {
                            clone_filter(expr, &|e| {
                                if let Expression::Variation(list) = e.as_ref() {
                                    if let Some(variant) = list.get(variant) {
                                        if variant.len() == 1 {
                                            Some(variant[0].clone())
                                        } else {
                                            Some(Rc::new(Expression::List(variant.clone())))
                                        }
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            })
                            .unwrap()
                        })
                        .collect();

                    let up = if variants.len() == 3 {
                        if let Expression::Stream(stream) = variants[2].as_ref() {
                            let mut stream = stream.clone();
                            stream.repeat = None;
                            Some(Rc::new(Expression::Stream(stream)))
                        } else {
                            panic!("stream expected");
                        }
                    } else {
                        None
                    };

                    let repeat = if let Expression::Stream(stream) = variants[1].as_ref() {
                        let mut stream = stream.clone();
                        // (foo)* / (foo)0+ not permitted with variantion
                        stream.repeat = match stream.repeat {
                            Some(RepeatMarker::Any) | Some(RepeatMarker::CountOrMore(0)) => {
                                if !down_variant_empty {
                                    return Err(
                                        "cannot have variant with '*' repeat, use '+' instead"
                                            .into(),
                                    );
                                }
                                Some(RepeatMarker::Any)
                            }
                            Some(RepeatMarker::OneOrMore) => Some(RepeatMarker::Any),
                            Some(RepeatMarker::CountOrMore(n)) => {
                                Some(RepeatMarker::CountOrMore(n - 1))
                            }
                            Some(RepeatMarker::Count(_)) | None => unreachable!(),
                        };
                        Rc::new(Expression::Stream(stream))
                    } else {
                        panic!("stream expected");
                    };

                    let down = if let Expression::Stream(stream) = variants[0].as_ref() {
                        let mut stream = stream.clone();
                        stream.repeat = None;
                        Some(Rc::new(Expression::Stream(stream)))
                    } else {
                        panic!("stream expected");
                    };

                    Ok(Variants { down, repeat, up })
                } else {
                    Ok(Variants {
                        down: None,
                        repeat: expr.clone(),
                        up: None,
                    })
                }
            } else {
                if let Some(expr) = expr.find_variant() {
                    return Err(format!("variant {expr} found without repeat marker"));
                }

                let mut down = Vec::new();
                let mut repeats = Vec::new();
                let mut up = Vec::new();

                let mut seen_repeat = None;
                let top_level_repeat = stream.repeat.clone();

                if stream.max_repeat() != Some(0) {
                    for expr in &stream.stream {
                        expr.check_no_contained_repeats()?;

                        if let Expression::Stream(stream) = expr.as_ref() {
                            if stream.is_repeating() {
                                if seen_repeat.is_some() {
                                    return Err("multiple repeat markers in IRP".into());
                                } else {
                                    repeats = stream.stream.clone();
                                    seen_repeat = stream.repeat.clone();
                                    continue;
                                }
                            }
                        }

                        if seen_repeat.is_some() {
                            add_flatten(&mut up, expr);
                        } else {
                            add_flatten(&mut down, expr);
                        }
                    }
                }

                if seen_repeat.is_some() {
                    if let Some(n) = stream.max_repeat() {
                        if n > 1 {
                            return Err("multiple repeat markers in IRP".into());
                        }
                    }
                }

                if match seen_repeat {
                    Some(RepeatMarker::OneOrMore) => true,
                    Some(RepeatMarker::CountOrMore(n)) => n > 0,
                    _ => false,
                } {
                    // if both the original stream and the added stream contain an extend,
                    // puth the original in stream
                    if has_extent(&repeats) && has_extent(&down) {
                        down = vec![
                            Rc::new(Expression::Stream(Stream {
                                bit_spec: Vec::new(),
                                stream: down,
                                repeat: None,
                            })),
                            Rc::new(Expression::Stream(Stream {
                                bit_spec: Vec::new(),
                                stream: repeats.clone(),
                                repeat: None,
                            })),
                        ];
                    } else {
                        for expr in &repeats {
                            add_flatten(&mut down, expr);
                        }
                    }

                    seen_repeat = if let Some(RepeatMarker::CountOrMore(n)) = seen_repeat {
                        match n {
                            1 => Some(RepeatMarker::Any),
                            2 => Some(RepeatMarker::OneOrMore),
                            _ => Some(RepeatMarker::CountOrMore(n - 1)),
                        }
                    } else {
                        Some(RepeatMarker::Any)
                    };
                }

                let down = if down.is_empty() {
                    None
                } else {
                    Some(Rc::new(Expression::Stream(Stream {
                        repeat: top_level_repeat.clone(),
                        stream: down,
                        bit_spec: stream.bit_spec.clone(),
                    })))
                };

                let up = if up.is_empty() {
                    None
                } else {
                    Some(Rc::new(Expression::Stream(Stream {
                        repeat: top_level_repeat,
                        stream: up,
                        bit_spec: stream.bit_spec.clone(),
                    })))
                };

                let repeat = Rc::new(Expression::Stream(Stream {
                    repeat: seen_repeat,
                    stream: repeats,
                    bit_spec: stream.bit_spec.clone(),
                }));

                Ok(Variants { down, repeat, up })
            }
        } else {
            Err("expected stream expression".into())
        }
    }
}

fn check_no_repeats(expr: &Expression) -> Result<(), String> {
    let mut repeats = false;

    expr.visit(&mut repeats, &|e, repeats| *repeats |= e.is_repeating());

    if repeats {
        Err("multiple repeat markers in IRP".into())
    } else {
        Ok(())
    }
}

// Add stream item to the list, flattening unnecessary lists/streams
fn add_flatten(expr: &mut Vec<Rc<Expression>>, elem: &Rc<Expression>) {
    match elem.as_ref() {
        Expression::List(list) => {
            for elem in list {
                expr.push(elem.clone());
            }
        }
        Expression::Stream(stream)
            if stream.bit_spec.is_empty()
                && !has_extent(&stream.stream)
                && stream.repeat.is_none() =>
        {
            for elem in &stream.stream {
                expr.push(elem.clone());
            }
        }
        _ => {
            expr.push(elem.clone());
        }
    }
}

/// Do we only have assignments - nothing to do here
fn all_assignments(list: &Vec<Rc<Expression>>) -> bool {
    for expr in list {
        match expr.as_ref() {
            Expression::Assignment(..) => (),
            Expression::List(list) if !all_assignments(list) => return false,
            Expression::Stream(stream) if !all_assignments(&stream.stream) => return false,
            _ => return false,
        }
    }

    true
}

/// Do we have any assignments
fn any_assignments(list: &Vec<Rc<Expression>>) -> bool {
    for expr in list {
        let mut changes = false;
        expr.visit(&mut changes, &|e, changes| {
            *changes |= matches!(e, Expression::Assignment(..) | Expression::Identifier(..));
        });

        if changes {
            return true;
        }
    }

    false
}

/// Do we only have assignments - nothing to do here
fn has_extent(list: &Vec<Rc<Expression>>) -> bool {
    for expr in list {
        if matches!(
            expr.as_ref(),
            Expression::ExtentConstant(..) | Expression::ExtentIdentifier(..)
        ) {
            return true;
        }
    }

    false
}

impl Stream {
    /// Every IRP expresion should have at most a single repeat marker. Is this it?
    fn is_repeating(&self) -> bool {
        !matches!(self.repeat, None | Some(crate::RepeatMarker::Count(_)))
    }

    #[allow(unused)]
    fn min_repeat(&self) -> i64 {
        match self.repeat {
            Some(RepeatMarker::OneOrMore) | None => 1,
            Some(RepeatMarker::Count(n) | RepeatMarker::CountOrMore(n)) => n,
            Some(RepeatMarker::Any) => 0,
        }
    }

    fn max_repeat(&self) -> Option<i64> {
        match self.repeat {
            None => Some(1),
            Some(RepeatMarker::Count(n)) => Some(n),
            _ => None,
        }
    }
}

impl Expression {
    /// Every IRP expresion should have at most a single repeat marker. Is this it?
    fn is_repeating(&self) -> bool {
        if let Expression::Stream(stream) = self {
            stream.is_repeating()
        } else {
            false
        }
    }

    /// Make sure there are no repeats anywhere within the expression (although the expression itself may be
    /// a stream with repeats)
    fn check_no_contained_repeats(&self) -> Result<(), String> {
        if let Expression::Stream(stream) = &self {
            for expr in &stream.stream {
                check_no_repeats(expr)?;
            }

            for expr in &stream.bit_spec {
                // bitspec should never have repeats
                check_no_repeats(expr)?;
            }
        }

        Ok(())
    }

    fn find_variant(&self) -> Option<&Expression> {
        let mut found = None;
        self.visit(
            &mut found,
            &|expr: &Expression, found: &mut Option<&Expression>| {
                if matches!(expr, Expression::Variation(..)) {
                    *found = Some(expr);
                }
            },
        );
        found
    }
}

#[test]
fn variants() -> Result<(), String> {
    let irp = Irp::parse("{}<1,-1|1,-3>([11][22],-100)*").unwrap();

    let variants = irp.split_variants()?;

    assert_eq!(
        format!("{}", variants.down.unwrap()),
        "<(1,-1)|(1,-3)>(11,-100)"
    );

    assert_eq!(format!("{}", variants.repeat), "<(1,-1)|(1,-3)>(22,-100)*");

    assert_eq!(variants.up, None);

    let irp = Irp::parse("{}<1,-1|1,-3>([11][22][33,44],-100)*").unwrap();

    let variants = irp.split_variants()?;

    assert_eq!(
        format!("{}", variants.down.unwrap()),
        "<(1,-1)|(1,-3)>(11,-100)"
    );

    assert_eq!(format!("{}", variants.repeat), "<(1,-1)|(1,-3)>(22,-100)*");

    assert_eq!(
        format!("{}", variants.up.unwrap()),
        "<(1,-1)|(1,-3)>((33,44),-100)"
    );

    let irp = Irp::parse("{38.4k,577}<2,-1|1,-2|1,-1|2,-2>((4,-1,D:8,T1:2,OBC:6,T2:2,S:8,1,-75m)*,(4,-1,D:8,~F1:2,OBC:6,~F2:2,S:8,1,-250m))
    [D:0..255,S:0..255,OBC:0..63,T1:0..3,T2:0..3,F1:0..3,F2:0..3]").unwrap();

    let variants = irp.split_variants()?;

    assert_eq!(
        format!("{}", variants.down.unwrap()),
        "<(2,-1)|(1,-2)|(1,-1)|(2,-2)>(4,-1,D:8,T1:2,OBC:6,T2:2,S:8,1,-75ms)"
    );

    assert_eq!(
        format!("{}", variants.repeat),
        "<(2,-1)|(1,-2)|(1,-1)|(2,-2)>(4,-1,D:8,T1:2,OBC:6,T2:2,S:8,1,-75ms)*"
    );

    assert_eq!(
        format!("{}", variants.up.unwrap()),
        "<(2,-1)|(1,-2)|(1,-1)|(2,-2)>(4,-1,D:8,~F1:2,OBC:6,~F2:2,S:8,1,-250ms)"
    );

    let irp = Irp::parse("{30.3k,512}<-1,1|1,-1>(1,-5,1023:10, -44, (1,-5,1:1,F:6,D:3,-236)+ ,1,-5,1023:10,-44)[F:0..63,D:0..7]").unwrap();

    let variants = irp.split_variants()?;

    assert_eq!(
        format!("{}", variants.down.unwrap()),
        "<(-1,1)|(1,-1)>(1,-5,1023:10,-44,1,-5,1:1,F:6,D:3,-236)"
    );

    assert_eq!(
        format!("{}", variants.repeat),
        "<(-1,1)|(1,-1)>(1,-5,1:1,F:6,D:3,-236)+"
    );

    assert_eq!(
        format!("{}", variants.up.unwrap()),
        "<(-1,1)|(1,-1)>(1,-5,1023:10,-44)"
    );

    let irp =
        Irp::parse("{}<-1,1|1,-1>(1,-5,^100m,(1,-5,1:1,F:6,D:3,^100m)+)[F:0..63,D:0..7]").unwrap();

    let variants = irp.split_variants()?;

    assert_eq!(
        format!("{}", variants.down.unwrap()),
        "<(-1,1)|(1,-1)>((1,-5,^100ms),(1,-5,1:1,F:6,D:3,^100ms))"
    );

    let irp = Irp::parse("{40k,600}<1,-1|2,-1>(4,-1,F:8,^45m)[F:0..255]").unwrap();

    let variants = irp.split_variants()?;

    assert_eq!(
        format!("{}", variants.down.unwrap()),
        "<(1,-1)|(2,-1)>(4,-1,F:8,^45ms)"
    );

    assert_eq!(
        format!("{}", variants.repeat),
        "<(1,-1)|(2,-1)>(4,-1,F:8,^45ms)"
    );

    assert_eq!(variants.up, None);

    Ok(())
}

#[test]
fn encode_variants() {
    use crate::Vartable;

    let irp = Irp::parse("{}<1,-1|1,-3>([11][22][33],-100)*").unwrap();

    assert_eq!(
        irp.split_variants_encode().unwrap_err(),
        "cannot have variant with '*' repeat, use '+' instead"
    );

    let irp = Irp::parse("{}<1,-1|1,-3>(100,-10,([11][22][33])2,-100)*").unwrap();

    assert_eq!(
        irp.split_variants_encode().unwrap_err(),
        "cannot have variant with '*' repeat, use '+' instead"
    );

    let irp = Irp::parse("{}<1,-1|1,-3>(100,-10,([][22][33])2,([11][22][33])2,-100)*").unwrap();

    assert_eq!(
        irp.split_variants_encode().unwrap_err(),
        "cannot have variant with '*' repeat, use '+' instead"
    );

    let irp = Irp::parse("{}<1,-1|1,-3>(100,-10,[][22][33],[11][22][33],-100)*").unwrap();

    assert_eq!(
        irp.split_variants_encode().unwrap_err(),
        "cannot have variant with '*' repeat, use '+' instead"
    );

    let irp = Irp::parse("{100}<1|-1>((10:2)+,-100)2").unwrap();

    assert_eq!(
        irp.split_variants_encode().unwrap_err(),
        "multiple repeat markers in IRP"
    );

    let irp = Irp::parse("{100}<1|-1>((10:2)+,-100)1").unwrap();

    let m = irp.encode(Vartable::new(), 1).unwrap();

    assert_eq!(m.raw, vec![100, 100, 100, 10100]);

    let irp = Irp::parse("{100}<1|-1>((10:2)+,-100)0").unwrap();

    let m = irp.encode(Vartable::new(), 1).unwrap();

    assert!(m.raw.is_empty());
}
