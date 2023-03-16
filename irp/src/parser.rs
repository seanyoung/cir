use super::{
    variants::variants, Expression, GeneralSpec, Irp, ParameterSpec, RepeatMarker, Stream, Unit,
    Vartable,
};
use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
    str::FromStr,
};

#[derive(PartialEq)]
enum GeneralItem<'a> {
    Msb,
    Lsb,
    Value(f64, Option<&'a str>),
}

peg::parser! {
    grammar irp_parser() for str {
        pub(super) rule irp() -> (Vec<GeneralItem<'input>>, Expression, Vec<Expression>, Vec<ParameterSpec>)
         = gs:general_spec() stream:bitspec_irstream() def:definitions()* specs:parameter_specs()?
        {
            let defs: Vec<Expression> = def.into_iter().flatten().collect();
            let specs = specs.unwrap_or_default();

            (gs, stream, defs, specs)
        }

        rule general_spec() -> Vec<GeneralItem<'input>>
         = _ "{" _ items:(general_item() ** ",") "}" _ { items }

        rule general_item() -> GeneralItem<'input>
         = _ "msb" _ { GeneralItem::Msb }
         / _ "lsb" _ { GeneralItem::Lsb }
         / _ v:number_decimals() _ u:$("u" / "p" / "k" / "%")? _ { GeneralItem::Value(v, u) }

        rule number_decimals() -> f64
         = n:$(['0'..='9']* "." ['0'..='9']+)
         {? match f64::from_str(n) { Ok(n) => Ok(n), Err(_) => Err("f64") } }
         / n:$(['0'..='9']+)
         {? match f64::from_str(n) { Ok(n) => Ok(n), Err(_) => Err("f64") } }

        rule definitions() -> Vec<Expression>
         = "{" _ def:(definition() ** ("," _)) "}" _ { def }

        rule definition() -> Expression
         = i:identifier() _ "=" _ e:expression() _ { Expression::Assignment(i.to_owned(), Rc::new(e)) }

        #[cache_left_rec]
        rule expression() -> Expression
         = cond:expression() "?" _ left:expression2() ":" _ right:expression2()
           { Expression::Conditional(Rc::new(cond), Rc::new(left), Rc::new(right)) }
         / expression2()

        #[cache_left_rec]
        rule expression2() -> Expression
         = left:expression2() "||" _ right:expression3()
           { Expression::Or(Rc::new(left), Rc::new(right)) }
         / expression3()

        #[cache_left_rec]
        rule expression3() -> Expression
         = left:expression3() "&&" _ right:expression4()
           { Expression::And(Rc::new(left), Rc::new(right)) }
         / expression4()

        #[cache_left_rec]
        rule expression4() -> Expression
         = left:expression4() "|" _ right:expression5()
           { Expression::BitwiseOr(Rc::new(left), Rc::new(right)) }
         / expression5()

        #[cache_left_rec]
        rule expression5() -> Expression
         = left:expression5() "&" _ right:expression6()
         { Expression::BitwiseAnd(Rc::new(left), Rc::new(right)) }
         / expression6()

        #[cache_left_rec]
        rule expression6() -> Expression
        = left:expression6() "^" _ right:expression7()
        { Expression::BitwiseXor(Rc::new(left), Rc::new(right)) }
        / expression7()

        #[cache_left_rec]
        rule expression7() -> Expression
         = left:expression7() "!=" _ right:expression8()
         { Expression::NotEqual(Rc::new(left), Rc::new(right)) }
         / left:expression7() "==" _ right:expression8()
         { Expression::Equal(Rc::new(left), Rc::new(right)) }
         / expression8()

        #[cache_left_rec]
        rule expression8() -> Expression
         = left:expression8() "<=" _ right:expression9()
         { Expression::LessEqual(Rc::new(left), Rc::new(right)) }
         / left:expression8() ">=" _ right:expression9()
         { Expression::GreaterEqual(Rc::new(left), Rc::new(right)) }
         / left:expression8() "<" _ right:expression9()
         { Expression::Less(Rc::new(left), Rc::new(right)) }
         / left:expression8() ">" _ right:expression9()
         { Expression::Greater(Rc::new(left), Rc::new(right)) }
         / expression9()

        #[cache_left_rec]
        rule expression9() -> Expression
         = left:expression9() "<<" _ right:expression10()
         { Expression::ShiftLeft(Rc::new(left), Rc::new(right)) }
         / left:expression9() ">>" _ right:expression10()
         { Expression::ShiftRight(Rc::new(left), Rc::new(right)) }
         / expression10()

        #[cache_left_rec]
        rule expression10() -> Expression
         = left:expression10() "+" _ right:expression11()
         { Expression::Add(Rc::new(left), Rc::new(right)) }
         / left:expression10() "-" _ right:expression11()
         { Expression::Subtract(Rc::new(left), Rc::new(right)) }
         / expression11()

        #[cache_left_rec]
        rule expression11() -> Expression
        = left:expression11() "*" _ right:expression12()
         { Expression::Multiply(Rc::new(left), Rc::new(right)) }
         / left:expression11() "/" _ right:expression12()
         { Expression::Divide(Rc::new(left), Rc::new(right)) }
         / left:expression11() "%" _ right:expression12()
         { Expression::Modulo(Rc::new(left), Rc::new(right)) }
         / expression12()

        #[cache_left_rec]
        rule expression12() -> Expression
         = left:expression13() "**" _ right:expression12()
         { Expression::Power(Rc::new(left), Rc::new(right)) }
         / expression13()

        #[cache_left_rec]
        rule expression13() -> Expression
         = "#" _ expr:expression14()
         { Expression::BitCount(Rc::new(expr)) }
         / expression14()

        #[cache_left_rec]
        rule expression14() -> Expression
         = "!" _ expr:expression15()
         { Expression::Not(Rc::new(expr)) }
         / expression15()

        #[cache_left_rec]
        rule expression15() -> Expression
         = "-" _ expr:expression16()
         { Expression::Negative(Rc::new(expr)) }
         / expression16()

        #[cache_left_rec]
        rule expression16() -> Expression
         = complement_bit_field()
         / "~" _ expr:expression17()
         { Expression::Complement(Rc::new(expr)) }
         / expression17()

        #[cache_left_rec]
        rule expression17() -> Expression
         = bit_field() / primary_item()

        rule bit_field() -> Expression
         = complement:"~"? _ value:primary_item() ":" _ reverse:"-"? length:primary_item() offset:offset()?
         {
            let value = if complement.is_some() {
                Rc::new(Expression::Complement(Rc::new(value)))
            } else {
                Rc::new(value)
            };

            Expression::BitField {
                value,
                reverse: reverse.is_some(),
                length: Rc::new(length),
                offset: offset.map(Rc::new),
            }
         }
         / complement:"~"? _ value:primary_item() "::" _ offset:primary_item()
         {
            let value = if complement.is_some() {
                Rc::new(Expression::Complement(Rc::new(value)))
            } else {
                Rc::new(value)
            };

            Expression::InfiniteBitField {
                value,
                offset: Rc::new(offset),
            }
         }

        rule complement_bit_field() -> Expression
         = "~" _ value:primary_item() ":" _ reverse:"-"? length:primary_item() offset:offset()?
         {
            let value = Rc::new(Expression::Complement(Rc::new(value)));

            Expression::BitField {
                value,
                reverse: reverse.is_some(),
                length: Rc::new(length),
                offset: offset.map(Rc::new),
            }
         }
         / "~" _ value:primary_item() "::" _ offset:primary_item()
         {
            let value = Rc::new(Expression::Complement(Rc::new(value)));

            Expression::InfiniteBitField {
                value,
                offset: Rc::new(offset),
            }
         }

        rule offset() -> Expression
         = ":" _ offset:primary_item() { offset }

        rule primary_item() -> Expression
         = number()
         / i:identifier() _ { Expression::Identifier(i.to_owned()) }
         / "(" _ e:expression() ")" _ { e }

        rule identifier() -> &'input str
         = quiet!{$(['_' | 'a'..='z' | 'A'..='Z']['_' | 'a'..='z' | 'A'..='Z' | '0'..='9']*)}
         / expected!("identifier")

        rule bare_number() -> i64
         = "0x" n:$(['0'..='9' | 'a'..='f' | 'A'..='F']+) _
         {? i64::from_str_radix(n, 16).map_err(|_| "i64") }
         / "0b" n:$(['0'..='1']+) _
         {? i64::from_str_radix(n, 2).map_err(|_| "i64") }
         / n:$("0" ['0'..='7']*) _
         {? i64::from_str_radix(n, 8).map_err(|_| "i64") }
         / n:$(['1'..='9'] ['0'..='9']*) _
         {? n.parse().map_err(|_| "i64") }
         / "UINT8_MAX" _ { u8::MAX as i64 }
         / "UINT16_MAX" _ { u16::MAX as i64 }
         / "UINT32_MAX" _ { u32::MAX as i64 }
         / "UINT64_MAX" _ { u64::MAX as i64 }

        rule number() -> Expression
         = n:bare_number() !(_ ['u'|'m'|'p']) { Expression::Number(n) }

        rule duration() -> Expression
         = id:identifier() _ unit:unit() { Expression::FlashIdentifier(id.to_owned(), unit) }
         / "-" id:identifier() _ unit:unit() { Expression::GapIdentifier(id.to_owned(), unit) }
         / "^" id:identifier() _ unit:unit() { Expression::ExtentIdentifier(id.to_owned(), unit) }
         / number:number_decimals() _ unit:unit() { Expression::FlashConstant(number, unit) }
         / "-" number:number_decimals() _ unit:unit() { Expression::GapConstant(number, unit) }
         / "^" number:number_decimals() _ unit:unit() { Expression::ExtentConstant(number, unit)}

        rule unit() -> Unit
         = "m" _ { Unit::Milliseconds }
         / "u" _ { Unit::Microseconds }
         / "p" _ { Unit::Pulses }
         / _ { Unit::Units }

        rule bare_irstream() -> Vec<Rc<Expression>>
         = items:(irstream_item() ** ("," _)) { items }

        rule irstream() -> Expression
         = "(" _ stream:bare_irstream() ")" _ repeat:repeat_marker()?
         {
            Expression::Stream(Stream {
                bit_spec: Vec::new(),
                stream,
                repeat,
            })
         }

        rule repeat_marker() -> RepeatMarker
         = "*" _ { RepeatMarker::Any }
         / "+" _ { RepeatMarker::OneOrMore }
         / n:$(['0'..='9']+) _ more:"+"? _
         {?
            match n.parse() {
                Ok(n) if more.is_some() => Ok(RepeatMarker::CountOrMore(n)),
                Ok(n) => Ok(RepeatMarker::Count(n)),
                Err(_) => Err("i64")
            }
         }

        rule irstream_item() -> Rc<Expression>
         = item:(variation()
         / bit_field()
         / definition()
         / duration()
         / irstream()
         / bitspec_irstream()) { Rc::new(item) }

        rule bitspec_item() -> Rc<Expression>
         = item:(variation()
         / bit_field()
         / bitspec_definition()
         / duration()
         / irstream()
         / bitspec_irstream()) { Rc::new(item) }

        rule bitspec_definition() -> Expression
         = i:identifier() _ "=" _ e:expression() _ { Expression::Assignment(i.to_owned(), Rc::new(e)) }

        rule bare_bitspec() -> Rc<Expression>
         = bitspec:(bitspec_item() ** ("," _))
         { Rc::new(Expression::List(bitspec)) }

        rule bitspec() -> Vec<Rc<Expression>>
         // !"!!" is for IrpTransmogrifier compatibility, no other reason
         = "<" _ bare:(bare_bitspec() ++ (!"||" "|" _)) ">" _ { bare }

        rule bitspec_irstream() -> Expression
         = bit_spec:bitspec() irstream:irstream() {
            if let Expression::Stream(mut stream) = irstream {
                stream.bit_spec = bit_spec;

                Expression::Stream(stream)
            } else {
                unreachable!()
            }
         }

        rule variation() -> Expression
         = a1:alternative() a2:alternative() a3:alternative()?
         {
            let mut list = vec![a1, a2];

            if let Some(e) = a3 {
                list.push(e);
            }

            Expression::Variation(list)
         }

        rule alternative() -> Vec<Rc<Expression>>
         = "[" _ bare:bare_irstream() "]" _ { bare }

        rule parameter_specs() -> Vec<ParameterSpec>
         = "[" _ specs:(parameter_spec() ** ("," _)) "]" _ { specs }

        rule parameter_spec() -> ParameterSpec
         = id:identifier() _ memory:"@"? _ ":" _ min:bare_number() _ ".." _ max:bare_number() _ default:initializer()?
         {
            ParameterSpec {
                name: id.to_owned(),
                memory: memory.is_some(),
                min,
                max,
                default,
            }
        }

        rule initializer() -> Expression
         = "=" _ expr:expression() { expr }

        rule _ = quiet!{(commentline() / commentblock() / [' ' | '\n' | '\r' | '\t'])*}

        rule commentline() = "//" [^'\n']*
        rule commentblock() = "/*" ([_] !"*/")* [_] "*/"
    }
}

impl Irp {
    /// Parse an irp and validate. The result can be used for encoding or decoding.
    pub fn parse(input: &str) -> Result<Irp, String> {
        match irp_parser::irp(input) {
            Ok((general, stream, definitions, parameters)) => {
                let general_spec = general_spec(&general)?;

                check_parameters(&parameters)?;
                check_definitions(&definitions, &parameters)?;
                check_stream(&stream)?;

                let stream = Rc::new(stream);
                let variants = variants(&stream)?;

                Ok(Irp {
                    general_spec,
                    stream,
                    definitions,
                    parameters,
                    variants,
                })
            }
            Err(pos) => Err(format!("parse error at {pos}")),
        }
    }

    /// The carrier frequency in Hertz. None means unknown, Some(0) means
    /// unmodulated.
    pub fn carrier(&self) -> i64 {
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

    /// Does this IRP have an ending part
    pub fn has_ending(&self) -> bool {
        self.variants[2].is_some()
    }
}

fn general_spec(items: &[GeneralItem]) -> Result<GeneralSpec, String> {
    let mut res = GeneralSpec {
        duty_cycle: None,
        carrier: 38000,
        lsb: true,
        unit: 1.0,
    };

    let mut unit = None;
    let mut lsb = None;
    let mut carrier = None;

    for item in items {
        match item {
            GeneralItem::Lsb | GeneralItem::Msb => {
                if lsb.is_some() {
                    return Err("bit order (lsb,msb) specified twice".into());
                }

                lsb = Some(*item == GeneralItem::Lsb);
            }
            GeneralItem::Value(v, u) => {
                let v = *v;

                let u = match u {
                    Some("%") => {
                        if v < 1.0 {
                            return Err("duty cycle less than 1% not valid".into());
                        }
                        if v > 99.0 {
                            return Err("duty cycle larger than 99% not valid".into());
                        }
                        if res.duty_cycle.is_some() {
                            return Err("duty cycle specified twice".into());
                        }

                        res.duty_cycle = Some(v as u8);

                        continue;
                    }
                    Some("k") => {
                        if carrier.is_some() {
                            return Err("carrier frequency specified twice".into());
                        }

                        carrier = Some((v * 1000.0) as i64);

                        continue;
                    }
                    Some("p") => Unit::Pulses,
                    Some("u") => Unit::Units,
                    None => Unit::Units,
                    _ => unreachable!(),
                };

                unit = Some((v, u));
            }
        }
    }

    if let Some(carrier) = carrier {
        res.carrier = carrier;
    }

    if let Some((p, u)) = unit {
        res.unit = match u {
            Unit::Pulses => p * 1_000_000.0 / res.carrier as f64,
            Unit::Milliseconds => p * 1000.0,
            Unit::Units | Unit::Microseconds => p,
        }
    }

    if Some(false) == lsb {
        res.lsb = false;
    }

    Ok(res)
}

fn check_parameters(parameters: &[ParameterSpec]) -> Result<(), String> {
    let mut seen_names: Vec<&str> = Vec::new();
    let mut vars = Vartable::new();

    for parameter in parameters {
        if seen_names.contains(&parameter.name.as_str()) {
            return Err(format!("duplicate parameter called {}", parameter.name));
        }
        seen_names.push(&parameter.name);

        let min = parameter.min;
        let max = parameter.max;

        if min < 0 || max < 0 || min > max {
            return Err(format!("invalid minimum {min} and maximum {max}"));
        }

        if parameter.memory && parameter.default.is_none() {
            return Err(format!(
                "memory parameter {} requires default value",
                parameter.name,
            ));
        }

        vars.set(parameter.name.to_owned(), min);
    }

    for parameter in parameters {
        if let Some(default) = &parameter.default {
            default.eval(&vars)?;
        }
    }

    Ok(())
}

fn check_definitions(
    definitions: &[Expression],
    parameters: &[ParameterSpec],
) -> Result<(), String> {
    let mut seen_names: Vec<&str> = Vec::new();
    let mut deps: HashMap<&str, HashSet<String>> = HashMap::new();

    for definition in definitions {
        if let Expression::Assignment(name, expr) = definition {
            if seen_names.contains(&name.as_str()) {
                return Err(format!("duplicate definition called {name}"));
            }
            seen_names.push(name);

            // definition cannot define itself
            let mut dependents = HashSet::new();
            expr.visit(&mut dependents, false, &|expr, dependents| {
                if let Expression::Identifier(var) = &expr {
                    dependents.insert(var.to_owned());
                }
            });

            if dependents.contains(name) {
                return Err(format!("definition {definition} depends on its own value"));
            }

            if parameters.iter().any(|parameter| &parameter.name == name) {
                return Err(format!(
                    "definition {name} overrides with parameter with same name"
                ));
            }

            deps.insert(name, dependents);
        } else {
            return Err(format!("invalid definition {definition}"));
        }
    }

    for name in deps.keys() {
        let mut visited: HashSet<&str> = HashSet::new();
        visited.insert(name);

        fn check_dep<'a>(
            def_name: &str,
            dep_name: &'a str,
            deps: &'a HashMap<&str, HashSet<String>>,
            visited: &mut HashSet<&'a str>,
        ) -> Result<(), String> {
            if let Some(dep) = deps.get(dep_name) {
                for name in dep {
                    if visited.contains(name.as_str()) {
                        return Err(format!(
                            "definition for {def_name} is circular via {dep_name}"
                        ));
                    } else {
                        let mut visited = visited.clone();
                        visited.insert(dep_name);
                        check_dep(def_name, name, deps, &mut visited)?;
                    }
                }
            }
            Ok(())
        }

        check_dep(name, name, &deps, &mut visited)?;
    }

    Ok(())
}

fn check_stream(stream: &Expression) -> Result<(), String> {
    match stream {
        Expression::Stream(stream) => {
            match &stream.repeat {
                Some(RepeatMarker::Count(count)) | Some(RepeatMarker::CountOrMore(count))
                    if *count > 64 =>
                {
                    return Err(format!("repeat count of {count} too large"))
                }
                _ => (),
            }

            for expr in &stream.stream {
                match expr.as_ref() {
                    Expression::FlashConstant(..)
                    | Expression::FlashIdentifier(..)
                    | Expression::GapConstant(..)
                    | Expression::GapIdentifier(..)
                    | Expression::ExtentConstant(..)
                    | Expression::ExtentIdentifier(..)
                    | Expression::Assignment(..)
                    | Expression::BitField { .. } => check_stream(expr)?,
                    Expression::Variation(list) => {
                        for list in list {
                            for expr in list {
                                match expr.as_ref() {
                                    Expression::FlashConstant(..)
                                    | Expression::FlashIdentifier(..)
                                    | Expression::GapConstant(..)
                                    | Expression::GapIdentifier(..)
                                    | Expression::ExtentConstant(..)
                                    | Expression::ExtentIdentifier(..)
                                    | Expression::Assignment(..)
                                    | Expression::BitField { .. } => check_stream(expr)?,
                                    _ => {
                                        return Err(format!(
                                            "expression {expr} not expected in variation"
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    Expression::Stream(..) => {
                        check_stream(expr)?;
                    }
                    _ => {
                        return Err(format!("expression {expr} not expected in stream"));
                    }
                }
            }

            if stream.bit_spec.len() > 16 {
                return Err(format!(
                    "bitspec contains {} values, no more than 16 supported",
                    stream.bit_spec.len()
                ));
            }

            for expr in &stream.bit_spec {
                if let Expression::List(list) = expr.as_ref() {
                    for expr in list {
                        match expr.as_ref() {
                            Expression::FlashConstant(..)
                            | Expression::FlashIdentifier(..)
                            | Expression::GapConstant(..)
                            | Expression::GapIdentifier(..)
                            | Expression::Assignment(..)
                            | Expression::BitField { .. } => check_stream(expr)?,
                            _ => {
                                return Err(format!("expression {expr} not expected in bit spec"));
                            }
                        }
                    }
                } else {
                    return Err("bit should be list of expressions".into());
                }
            }
        }
        Expression::List(list) => {
            for expr in list {
                check_stream(expr)?;
            }
        }
        Expression::Variation(list) => {
            for list in list {
                for expr in list {
                    check_stream(expr)?;
                }
            }
        }
        Expression::BitField { length, .. } => {
            if let Ok(length) = length.eval(&Vartable::new()) {
                if !(0..64).contains(&length) {
                    return Err(format!("bitfield of length {length} not supported"));
                }
            }
        }
        _ => (),
    }
    Ok(())
}

#[test]
fn precedence() {
    let irp = Irp::parse("{}<1|-1>(){A=B<<C+D*E}").unwrap();

    assert_eq!(format!("{}", irp.definitions[0]), "A=(B << (C + (D * E)))");

    let irp = Irp::parse("{}<1|-1>(){A=F**G**H+128*~T>=8}").unwrap();

    assert_eq!(
        format!("{}", irp.definitions[0]),
        "A=(((F ** (G ** H)) + (128 * ~T)) >= 8)"
    );

    let irp = Irp::parse("{}<1|-1>(){A=F||G&&H|I&J^K}").unwrap();

    assert_eq!(
        format!("{}", irp.definitions[0]),
        "A=(F || (G && (H | (I & (J ^ K)))))"
    );

    let irp = Irp::parse("{}<1|-1>(){A=F>G*10&&H*20<J}").unwrap();

    assert_eq!(
        format!("{}", irp.definitions[0]),
        "A=((F > (G * 10)) && ((H * 20) < J))"
    );

    let irp = Irp::parse("{}<1|-1>(){A=E*F+G<<2}").unwrap();

    assert_eq!(format!("{}", irp.definitions[0]), "A=(((E * F) + G) << 2)");

    let irp = Irp::parse("{}<1|-1>(){A=E<<F+G*2}").unwrap();

    assert_eq!(format!("{}", irp.definitions[0]), "A=(E << (F + (G * 2)))");
}
