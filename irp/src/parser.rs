use super::irp;
use super::{Expression, GeneralSpec, IrStream, Irp, ParameterSpec, RepeatMarker, Unit};
use num::Num;
use std::str::FromStr;

/// Parse an irp into an AST type representation
impl Irp {
    pub fn parse(input: &str) -> Result<Irp, String> {
        let mut parser = irp::PEG::new();

        match parser.parse(input) {
            Ok(node) => {
                let general_spec = general_spec(&node.children[0], input)?;
                let stream = bitspec_irstream(&node.children[1], input)?;
                let definitions = definitions(&node.children[2], input)?;
                let parameters = parameters(&node.children[3], input)?;

                Ok(Irp {
                    general_spec,
                    stream,
                    definitions,
                    parameters,
                })
            }
            Err(pos) => Err(format!("parse error at {}:{}", pos.0, pos.1)),
        }
    }

    /// The carrier frequency in Hertz. None means unknown, Some(0) means
    /// unmodulated.
    pub fn carrier(&self) -> Option<i64> {
        self.general_spec.carrier
    }
    /// Duty cycle of the carrier pulse wave. Between 1% and 99%.
    pub fn duty_cycle(&self) -> Option<u8> {
        self.general_spec.duty_cycle
    }

    /// Bit-ordering rule to use when converting variables from binary form to
    /// bit sequences. When true, variables are encoded for transmission with
    /// their least significant bit first, otherwise the order is reversed.
    pub fn lsb(&self) -> bool {
        self.general_spec.lsb
    }

    /// Unit of time that may be used in durations and extents. The default unit
    /// is 1.0 microseconds. If a carrier frequency is defined, the unit may
    /// also be defined in terms of a number of carrier frequency pulses.
    pub fn unit(&self) -> f64 {
        self.general_spec.unit
    }
}

fn collect_rules(node: &irp::Node, rule: irp::Rule) -> Vec<&irp::Node> {
    let mut list = Vec::new();

    fn recurse<'t>(node: &'t irp::Node, rule: irp::Rule, list: &mut Vec<&'t irp::Node>) {
        if node.rule == rule {
            list.push(node);
        } else {
            for node in &node.children {
                recurse(node, rule, list);
            }
        }
    }

    recurse(node, rule, &mut list);

    list
}

fn general_spec(node: &irp::Node, input: &str) -> Result<GeneralSpec, String> {
    let mut res = GeneralSpec {
        duty_cycle: None,
        carrier: None,
        lsb: true,
        unit: 1.0,
    };

    let mut unit = None;
    let mut lsb = None;

    for node in collect_rules(node, irp::Rule::general_item) {
        let s = node.as_str(input);

        if matches!(node.alternative, Some(2) | Some(3)) {
            let v = f64::from_str(node.children[0].as_str(input)).unwrap();

            let u = if node.alternative == Some(2) {
                match node.children[2].as_str(input) {
                    "%" => {
                        if v < 1.0 {
                            return Err("duty cycle less than 1% not valid".to_string());
                        }
                        if v > 99.0 {
                            return Err("duty cycle larger than 99% not valid".to_string());
                        }
                        if res.duty_cycle.is_some() {
                            return Err("duty cycle specified twice".to_string());
                        }

                        res.duty_cycle = Some(v as u8);

                        continue;
                    }
                    "k" => {
                        if res.carrier.is_some() {
                            return Err("carrier frequency specified twice".to_string());
                        }

                        res.carrier = Some((v * 1000.0) as i64);

                        continue;
                    }
                    "p" => Unit::Pulses,
                    "u" => Unit::Units,
                    _ => unreachable!(),
                }
            } else {
                Unit::Units
            };

            if unit.is_some() {
                return Err("unit specified twice".to_string());
            }

            unit = Some((v, u));
        } else {
            match s {
                "msb" | "lsb" => {
                    if lsb.is_some() {
                        return Err("bit order (lsb,msb) specified twice".to_string());
                    }

                    lsb = Some(s == "lsb");
                }
                _ => unreachable!(),
            }
        }
    }

    if let Some((p, u)) = unit {
        res.unit = match u {
            Unit::Pulses => {
                if let Some(f) = res.carrier {
                    p * 1_000_000.0 / f as f64
                } else {
                    return Err("pulse unit specified without carrier frequency".to_string());
                }
            }
            Unit::Milliseconds => p * 1000.0,
            Unit::Units | Unit::Microseconds => p,
        }
    }

    if Some(false) == lsb {
        res.lsb = false;
    }

    Ok(res)
}

fn definitions(node: &irp::Node, input: &str) -> Result<Vec<Expression>, String> {
    let mut list = Vec::new();

    for node in collect_rules(node, irp::Rule::definition) {
        list.push(expression(node, input)?);
    }

    Ok(list)
}

fn parameters(node: &irp::Node, input: &str) -> Result<Vec<ParameterSpec>, String> {
    let mut list = Vec::new();

    for node in collect_rules(node, irp::Rule::parameter_spec) {
        let name = node.children[0].as_str(input).to_owned();
        let memory = !node.children[2].is_empty();
        let min = expression(&node.children[6], input)?;
        let max = expression(&node.children[10], input)?;
        let default_node = &node.children[12];

        let default = if default_node.is_empty() {
            None
        } else {
            Some(expression(&default_node.children[1], input)?)
        };

        list.push(ParameterSpec {
            name,
            memory,
            min,
            max,
            default,
        });
    }

    Ok(list)
}

fn expression(node: &irp::Node, input: &str) -> Result<Expression, String> {
    match node.rule {
        irp::Rule::expression
        | irp::Rule::expression2
        | irp::Rule::expression3
        | irp::Rule::expression4
        | irp::Rule::expression5
        | irp::Rule::expression6
        | irp::Rule::expression7
        | irp::Rule::expression8
        | irp::Rule::expression9
        | irp::Rule::expression10
        | irp::Rule::expression11
        | irp::Rule::expression12
        | irp::Rule::expression13
        | irp::Rule::expression14
        | irp::Rule::expression15
        | irp::Rule::expression16 => {
            // expression1
            if node.children.len() == 3 {
                let expr = Box::new(expression(&node.children[2], input)?);
                match node.children[0].as_str(input) {
                    "#" => Ok(Expression::BitCount(expr)),
                    "!" => Ok(Expression::Not(expr)),
                    "~" => Ok(Expression::Complement(expr)),
                    "-" => Ok(Expression::Negative(expr)),
                    op => panic!("{} not expected", op),
                }
            } else if node.children.len() == 4 {
                let left = Box::new(expression(&node.children[0], input)?);
                let right = Box::new(expression(&node.children[3], input)?);

                match node.children[1].as_str(input) {
                    "*" => Ok(Expression::Multiply(left, right)),
                    "/" => Ok(Expression::Divide(left, right)),
                    "%" => Ok(Expression::Modulo(left, right)),
                    "+" => Ok(Expression::Add(left, right)),
                    "-" => Ok(Expression::Subtract(left, right)),
                    "<<" => Ok(Expression::ShiftLeft(left, right)),
                    ">>" => Ok(Expression::ShiftRight(left, right)),
                    "<=" => Ok(Expression::LessEqual(left, right)),
                    ">=" => Ok(Expression::MoreEqual(left, right)),
                    ">" => Ok(Expression::More(left, right)),
                    "<" => Ok(Expression::Less(left, right)),
                    "!=" => Ok(Expression::NotEqual(left, right)),
                    "==" => Ok(Expression::Equal(left, right)),
                    "&" => Ok(Expression::BitwiseAnd(left, right)),
                    "|" => Ok(Expression::BitwiseOr(left, right)),
                    "^" => Ok(Expression::BitwiseXor(left, right)),
                    "&&" => Ok(Expression::And(left, right)),
                    "||" => Ok(Expression::Or(left, right)),
                    "**" => Ok(Expression::Power(left, right)),
                    _ => unimplemented!(),
                }
            } else if node.children.len() == 6 {
                let cond = Box::new(expression(&node.children[0], input)?);
                let left = Box::new(expression(&node.children[3], input)?);
                let right = Box::new(expression(&node.children[6], input)?);

                Ok(Expression::Ternary(cond, left, right))
            } else {
                // expression2
                expression(&node.children[0], input)
            }
        }
        irp::Rule::expression17 => expression(&node.children[0], input),
        irp::Rule::primary_item => match node.alternative {
            Some(0) => expression(&node.children[0], input),
            Some(1) => {
                let s = node.children[0].as_str(input);

                Ok(Expression::Identifier(s.to_owned()))
            }
            Some(2) => expression(&node.children[2], input),
            _ => unreachable!(),
        },
        irp::Rule::bit_field => {
            let mut value = Box::new(expression(&node.children[2], input)?);
            if !node.children[0].is_empty() {
                value = Box::new(Expression::Complement(value));
            }
            match node.alternative {
                Some(0) => {
                    let reverse = !node.children[5].is_empty();
                    let length = Box::new(expression(&node.children[7], input)?);

                    let skip_node = &node.children[8];

                    let skip = if skip_node.children.is_empty() {
                        None
                    } else {
                        Some(Box::new(expression(&skip_node.children[2], input)?))
                    };

                    Ok(Expression::BitField {
                        value,
                        length,
                        reverse,
                        skip,
                    })
                }
                Some(1) => {
                    let skip = Box::new(expression(&node.children[5], input)?);

                    Ok(Expression::InfiniteBitField { value, skip })
                }
                _ => unreachable!(),
            }
        }
        irp::Rule::number => {
            // number
            let s = node.as_str(input);

            if s == "UINT8_MAX" {
                Ok(Expression::Number(u8::MAX as i64))
            } else if s == "UINT16_MAX" {
                Ok(Expression::Number(u16::MAX as i64))
            } else if s == "UINT32_MAX" {
                Ok(Expression::Number(u32::MAX as i64))
            } else if s == "UINT64_MAX" {
                Ok(Expression::Number(u64::MAX as i64))
            } else if let Some(hex) = s.strip_prefix("0x") {
                Ok(Expression::Number(i64::from_str_radix(hex, 16).unwrap()))
            } else if let Some(bin) = s.strip_prefix("0b") {
                Ok(Expression::Number(i64::from_str_radix(bin, 2).unwrap()))
            } else {
                Ok(Expression::Number(s.parse().unwrap()))
            }
        }
        irp::Rule::bitspec_definition | irp::Rule::definition => {
            let name = node.children[0].as_str(input);

            let expr = expression(&node.children[4], input)?;

            Ok(Expression::Assignment(name.to_string(), Box::new(expr)))
        }
        irp::Rule::duration => {
            let unit = match node.children[2].as_str(input) {
                "" => Unit::Units,
                "p" => Unit::Pulses,
                "u" => Unit::Microseconds,
                "m" => Unit::Milliseconds,
                err => panic!("unit {} not expected", err),
            };

            let duration_node = &node.children[1];
            let op = node.children[0].as_str(input);
            let duration = duration_node.as_str(input);

            if duration_node.alternative == Some(0) {
                if op == "^" {
                    Ok(Expression::ExtentIdentifier(duration.to_owned(), unit))
                } else if op == "-" {
                    Ok(Expression::GapIdentifier(duration.to_owned(), unit))
                } else {
                    assert_eq!(op, "");

                    Ok(Expression::FlashIdentifier(duration.to_owned(), unit))
                }
            } else {
                let value = f64::from_str_radix(duration, 10).unwrap();

                if op == "^" {
                    Ok(Expression::ExtentConstant(value, unit))
                } else if op == "-" {
                    Ok(Expression::GapConstant(value, unit))
                } else {
                    assert_eq!(op, "");

                    Ok(Expression::FlashConstant(value, unit))
                }
            }
        }
        irp::Rule::irstream_item | irp::Rule::bitspec_item => expression(&node.children[0], input),
        irp::Rule::irstream => {
            let (stream, repeat) = irstream(node, input)?;

            Ok(Expression::Stream(IrStream {
                bit_spec: Vec::new(),
                stream,
                repeat,
            }))
        }
        irp::Rule::bitspec_irstream => bitspec_irstream(node, input),
        irp::Rule::variation => {
            let mut list = Vec::new();

            for node in collect_rules(node, irp::Rule::alternative) {
                list.push(bare_irstream(node, input)?);
            }

            Ok(Expression::Variation(list))
        }
        rule => {
            println!("node:{}", node.print_to_string(input));
            panic!("rule {:?} not expected", rule);
        }
    }
}

fn bitspec_irstream(node: &irp::Node, input: &str) -> Result<Expression, String> {
    let bit_spec = bitspec(&node.children[0], input)?;
    let (stream, repeat) = irstream(&node.children[1], input)?;

    Ok(Expression::Stream(IrStream {
        bit_spec,
        stream,
        repeat,
    }))
}

fn bitspec(node: &irp::Node, input: &str) -> Result<Vec<Expression>, String> {
    let mut list = Vec::new();

    for node in collect_rules(node, irp::Rule::bare_bitspec) {
        list.push(Expression::List(bare_bitspec(node, input)?));
    }

    match list.len() {
        2 | 4 | 8 | 16 => Ok(list),
        len => Err(format!(
            "bitspec should have 2, 4, 8, or 16 entries, found {}",
            len
        )),
    }
}

fn bare_bitspec(node: &irp::Node, input: &str) -> Result<Vec<Expression>, String> {
    let mut list = Vec::new();

    for node in collect_rules(node, irp::Rule::bitspec_item) {
        list.push(expression(node, input)?);
    }

    Ok(list)
}

fn bare_irstream(node: &irp::Node, input: &str) -> Result<Vec<Expression>, String> {
    let mut list = Vec::new();

    for node in collect_rules(node, irp::Rule::irstream_item) {
        list.push(expression(node, input)?);
    }

    Ok(list)
}

fn irstream(
    node: &irp::Node,
    input: &str,
) -> Result<(Vec<Expression>, Option<RepeatMarker>), String> {
    assert_eq!(node.rule, irp::Rule::irstream);

    let bare_irstream = bare_irstream(&node.children[2], input)?;

    let repeat_node = &node.children[5];

    let repeat = if repeat_node.is_empty() {
        None
    } else {
        Some(match repeat_node.alternative {
            Some(0) => RepeatMarker::Any,
            Some(1) => RepeatMarker::OneOrMore,
            Some(2) => {
                let value = repeat_node.children[0].as_str(input).parse().unwrap();
                if repeat_node.children[1].is_empty() {
                    RepeatMarker::Count(value)
                } else {
                    RepeatMarker::CountOrMore(value)
                }
            }
            _ => unreachable!(),
        })
    };

    Ok((bare_irstream, repeat))
}
