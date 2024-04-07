use super::{
    expression::clone_filter, Event, Expression, Irp, ParameterSpec, RepeatMarker, Vartable,
};
use log::trace;
use std::{
    collections::HashMap,
    fmt,
    ops::{Add, BitAnd, BitOr, BitXor, Neg, Not, Rem, Shl, Shr, Sub},
    rc::Rc,
};

/**
 * Here we build the decoder nfa (non-deterministic finite automation)
 */

#[derive(PartialEq, Debug, Clone)]
pub(crate) struct Edge {
    pub dest: usize,
    pub actions: Vec<Action>,
}

#[derive(PartialEq, Debug, Hash, Eq, Clone)]
pub(crate) enum Length {
    Expression(Rc<Expression>),
    Range(u32, Option<u32>),
}

#[derive(PartialEq, Debug, Hash, Eq, Clone)]
pub(crate) enum Action {
    Flash {
        length: Length,
        complete: bool,
    },
    Gap {
        length: Length,
        complete: bool,
    },
    Set {
        var: String,
        expr: Rc<Expression>,
    },
    AssertEq {
        left: Rc<Expression>,
        right: Rc<Expression>,
    },
    Done(Event, Vec<String>),
}

#[derive(PartialEq, Default, Clone, Debug)]
pub(crate) struct Vertex {
    pub entry: Vec<Action>,
    pub edges: Vec<Edge>,
}

/// Non-deterministic finite automation for decoding IR. We create the DFA
/// (deterministic finite automation) from this, but it can also be used for
/// decoding IR.
#[derive(Debug, Default)]
pub struct NFA {
    pub(crate) verts: Vec<Vertex>,
}

impl Irp {
    /// Generate an NFA decoder for this IRP. This may fail if it is impossible
    /// for this IRP.
    pub fn build_nfa(&self) -> Result<NFA, String> {
        let mut builder = Builder::new(self);

        builder.add_constants();

        if let Some(down) = &self.variants[0] {
            builder.build(Event::Down, down)?;
        }

        if let Some(repeat) = &self.variants[1] {
            builder.build(Event::Repeat, repeat)?;
        }

        if let Some(up) = &self.variants[2] {
            builder.build(Event::Up, up)?;
        }

        Ok(NFA {
            verts: builder.complete(),
        })
    }
}

impl NFA {
    /// Generate a GraphViz dot file and write to the given path
    pub fn dotgraphviz(&self, path: &str) {
        crate::graphviz::graphviz(&self.verts, "NFA", &[], path);
    }

    /// Add nfa states for parsing raw IR
    pub fn add_raw(&mut self, raw: &[u32], event: Event, code: i64) {
        assert_ne!(raw.len(), 0);
        assert_eq!(raw.len() % 2, 0);

        if self.verts.is_empty() {
            self.verts.push(Vertex::default());
        }

        let mut pos = 0;
        let mut flash = true;

        for raw in raw {
            let length = Rc::new(Expression::Number((*raw).into()));
            let actions = vec![if flash {
                Action::Flash {
                    length: Length::Expression(length),
                    complete: true,
                }
            } else {
                Action::Gap {
                    length: Length::Expression(length),
                    complete: true,
                }
            }];

            if let Some(next) = self.verts[pos].edges.iter().find_map(|edge| {
                if edge.actions == actions && self.verts[edge.dest].entry.is_empty() {
                    Some(edge.dest)
                } else {
                    None
                }
            }) {
                pos = next;
            } else {
                let next = self.verts.len();

                self.verts.push(Vertex::default());

                self.verts[pos].edges.push(Edge {
                    actions,
                    dest: next,
                });

                pos = next;
            }

            flash = !flash;
        }

        self.verts[pos].entry.push(Action::Set {
            var: "CODE".into(),
            expr: Rc::new(Expression::Number(code)),
        });

        self.verts[pos]
            .entry
            .push(Action::Done(event, vec!["CODE".into()]));
    }
}

pub(crate) fn gen_mask(v: i64) -> i64 {
    (1i64 << v) - 1
}

/// track which bits of which variables may be set, builder head, etc.
#[derive(Clone, Debug)]
pub(crate) struct Builder<'a> {
    cur: BuilderLocation,
    saved: Vec<BuilderLocation>,
    verts: Vec<Vertex>,
    extents: Vec<i64>,
    constants: Vartable<'a>,
    definitions: HashMap<String, Rc<Expression>>,
    pub irp: &'a Irp,
}

#[derive(Clone, Debug, Default)]
struct BuilderLocation {
    head: usize,
    seen_edges: bool,
    vars: HashMap<String, i64>,
}

impl<'a> Builder<'a> {
    pub fn new(irp: &'a Irp) -> Self {
        let verts = vec![Vertex::default()];

        Builder {
            cur: BuilderLocation::default(),
            saved: Vec::new(),
            extents: Vec::new(),
            constants: Vartable::new(),
            definitions: HashMap::new(),
            verts,
            irp,
        }
    }

    fn set(&mut self, name: &str, fields: i64) {
        assert!(fields != 0);

        if let Some(e) = self.cur.vars.get_mut(name) {
            *e |= fields;
        } else {
            self.cur.vars.insert(name.to_owned(), fields);
        }
    }

    fn unset(&mut self, name: &str) {
        self.cur.vars.remove(name);
    }

    pub fn all_field_set(&self, name: &str, fields: i64) -> bool {
        if let Some(e) = self.cur.vars.get(name) {
            (e & fields) == fields
        } else {
            false
        }
    }

    pub fn any_field_set(&self, name: &str, fields: i64) -> bool {
        if let Some(e) = self.cur.vars.get(name) {
            (e & fields) != 0
        } else {
            false
        }
    }

    fn is_any_set(&self, name: &str) -> bool {
        self.cur.vars.contains_key(name)
    }

    fn add_done(&mut self, event: Event) -> Result<bool, String> {
        if self.cur.seen_edges {
            let res = self
                .irp
                .parameters
                .iter()
                .filter_map(|param| {
                    if self.cur.vars.contains_key(&param.name) {
                        Some(param.name.to_owned())
                    } else {
                        None
                    }
                })
                .collect();

            self.add_entry_action(Action::Done(event, res));
            self.mask_results()?;
            self.cur.seen_edges = false;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn add_vertex(&mut self) -> usize {
        let node = self.verts.len();

        self.verts.push(Vertex::default());

        node
    }

    fn set_head(&mut self, head: usize) {
        self.cur.head = head;
    }

    fn add_entry_action(&mut self, action: Action) {
        self.verts[self.cur.head].entry.push(action);
    }

    fn add_edge(&mut self, edge: Edge) {
        self.verts[self.cur.head].edges.push(edge);
    }

    fn add_action_at_node(&mut self, node: usize, action: Action) {
        self.verts[node].entry.push(action);
    }

    fn add_edge_at_node(&mut self, node: usize, edge: Edge) {
        self.verts[node].edges.push(edge);
    }

    /// Once done building, return the completed vertices
    fn complete(self) -> Vec<Vertex> {
        self.verts
    }

    fn push_location(&mut self) {
        self.saved.push(self.cur.clone());
    }

    fn pop_location(&mut self) {
        self.cur = self.saved.pop().unwrap();
    }

    /// The list of definitions may contain variables which are constants and
    /// can be evaluated now. Some of those will be modified at a later point;
    /// and will end up as variables with initializers, else if the variables
    /// are truly constant then they end up in the constants map.
    fn add_constants(&mut self) {
        // first add the true constants
        while {
            let mut changes = false;

            for def in &self.irp.definitions {
                if let Expression::Assignment(name, expr) = def {
                    if self.is_any_set(name) {
                        continue;
                    }

                    let mut modified_anywhere = false;

                    // is this variable modified anywhere
                    self.irp.stream.visit(
                        &mut modified_anywhere,
                        false,
                        &|expr: &Expression, modified: &mut bool| {
                            if let Expression::Assignment(var_name, _) = expr {
                                if var_name == name {
                                    *modified = true;
                                }
                            }
                        },
                    );

                    if modified_anywhere {
                        continue;
                    }

                    if self.expression_available(expr, false).is_ok() {
                        let val = expr.eval(&self.constants).unwrap();

                        self.constants.set(name.to_owned(), val);

                        changes = true;

                        self.set(name, !0);
                    }
                }
            }

            changes
        } {}

        // any remaining definitions which are available are either modified or
        // depend on a variable which will be modified at some point
        while {
            let mut changes = false;

            for def in &self.irp.definitions {
                if let Expression::Assignment(name, expr) = def {
                    if self.is_any_set(name) {
                        continue;
                    }

                    if self.expression_available(expr, false).is_ok() {
                        // just set an initial value
                        self.add_entry_action(Action::Set {
                            var: name.to_owned(),
                            expr: self.const_folding(expr),
                        });

                        changes = true;

                        self.set(name, !0);
                    }

                    self.definitions.insert(name.to_owned(), expr.clone());
                }
            }

            changes
        } {}
    }

    fn expression_list(
        &mut self,
        list: &[Rc<Expression>],
        bit_spec: &[&[Rc<Expression>]],
        last: bool,
    ) -> Result<(), String> {
        let mut pos = 0;

        while pos < list.len() {
            let mut bit_count = 0;
            let mut expr_count = 0;

            while let Some(expr) = list.get(pos + expr_count) {
                if let Expression::BitField { length, .. } = expr.as_ref() {
                    let (min_len, max_len, _) = self.bit_field_length(length)?;

                    // if variable length bit field is preceded by a constant length fields, process
                    // those before this one
                    if expr_count != 0 && max_len != min_len {
                        break;
                    }

                    if max_len > 64 {
                        return Err(format!(
                            "bitfield of length {max_len} longer than the 64 maximum"
                        ));
                    }

                    if bit_count + max_len > 64 {
                        break;
                    }

                    bit_count += min_len;
                    expr_count += 1;

                    // variable length bitfields should be processed by one
                    if max_len != min_len {
                        break;
                    }
                } else {
                    break;
                }
            }

            let last = last && pos == list.len() - 1;

            if expr_count == 0 {
                // not a bit field
                self.expression(&list[pos], bit_spec, last)?;
                pos += 1;
                continue;
            }

            // if it is a single constant bitfield, just expand it - no loops needed
            if expr_count == 1 && bit_count <= 8 {
                if let Expression::BitField {
                    value,
                    offset: None,
                    reverse,
                    ..
                } = list[pos].as_ref()
                {
                    if let Expression::Number(value) = self.const_folding(value).as_ref() {
                        if self.irp.general_spec.lsb ^ reverse {
                            for bit in 0..bit_count {
                                let last = last && bit == bit_count - 1;

                                let e = &bit_spec[0][((value >> bit) & 1) as usize];

                                self.expression(e, &bit_spec[1..], last)?;
                            }
                        } else {
                            for bit in (0..bit_count).rev() {
                                let last = last && bit == 0;

                                let e = &bit_spec[0][((value >> bit) & 1) as usize];

                                self.expression(e, &bit_spec[1..], last)?;
                            }
                        }

                        pos += 1;
                        continue;
                    }
                }
            }

            let do_reverse = if expr_count == 1 {
                if let Expression::BitField {
                    value,
                    offset,
                    reverse,
                    length,
                } = list[pos].as_ref()
                {
                    let (min_len, max_len, store_length) = self.bit_field_length(length)?;

                    if min_len != max_len {
                        self.decode_bits(
                            Some(min_len),
                            max_len,
                            *reverse,
                            store_length,
                            bit_spec,
                            last,
                        )?;

                        let offset = if let Some(offset) = offset {
                            self.const_folding(offset).eval(&Vartable::new())?
                        } else {
                            0
                        };

                        let bits = Expression::Identifier(String::from("$bits"));

                        let bits = if offset > 0 {
                            Expression::ShiftLeft(
                                Rc::new(bits),
                                Rc::new(Expression::Number(offset)),
                            )
                        } else {
                            bits
                        };

                        if let Expression::Identifier(name) = value.as_ref() {
                            self.add_entry_action(Action::Set {
                                var: name.to_owned(),
                                expr: self.const_folding(&Rc::new(bits)),
                            });

                            self.set(name, !0);
                        } else {
                            return Err(format!(
                                "expression {value} not supported for variable length bitfield"
                            ));
                        }

                        pos += 1;
                        continue;
                    } else {
                        *reverse
                    }
                } else {
                    unreachable!();
                }
            } else {
                false
            };

            self.decode_bits(None, bit_count, do_reverse, None, bit_spec, last)?;

            let mut delayed = Vec::new();

            // now do stuff with bitfields
            let mut offset = if self.irp.general_spec.lsb {
                0
            } else {
                bit_count
            };

            for i in 0..expr_count {
                let expr = list[i + pos].as_ref();

                if let Expression::BitField {
                    value,
                    length,
                    offset: bitfield_offset,
                    reverse,
                } = expr
                {
                    let length = self.const_folding(length).eval(&Vartable::new())?;

                    if !self.irp.general_spec.lsb {
                        offset -= length;
                    }

                    let bitfield_offset = if let Some(bitfield_offset) = bitfield_offset {
                        self.const_folding(bitfield_offset).eval(&Vartable::new())?
                    } else {
                        0
                    };

                    let mut value = self.const_folding(value);

                    let bits = Rc::new(Expression::Identifier(String::from("$bits")));

                    let mut bits = if offset > bitfield_offset {
                        Rc::new(Expression::ShiftRight(
                            bits,
                            Rc::new(Expression::Number(offset - bitfield_offset)),
                        ))
                    } else if offset < bitfield_offset {
                        Rc::new(Expression::ShiftLeft(
                            bits,
                            Rc::new(Expression::Number(bitfield_offset - offset)),
                        ))
                    } else {
                        bits
                    };

                    if *reverse && !do_reverse {
                        bits = Rc::new(Expression::BitReverse(bits, length, bitfield_offset));
                    }

                    let mask = gen_mask(length) << bitfield_offset;

                    // F:4 => ($bits & 15) = (F & 15)
                    // ~F:4 => (~$bits & 15) = (F & 15)
                    // (F-1):4 => ($bits & 15) + 1 = F
                    // ~(F-1):4 => ~($bits & 15) + 1 = F
                    match value.as_ref() {
                        Expression::Complement(comp) => {
                            if matches!(comp.as_ref(), Expression::Identifier(..)) {
                                value = comp.clone();
                                bits = Rc::new(Expression::BitwiseAnd(
                                    Rc::new(Expression::Complement(bits)),
                                    Rc::new(Expression::Number(mask)),
                                ));
                            }
                        }
                        _ => {
                            bits = Rc::new(Expression::BitwiseAnd(
                                bits,
                                Rc::new(Expression::Number(mask)),
                            ));
                        }
                    };

                    match self.expression_available(expr, true) {
                        Ok(_) => {
                            // We know all the variables in here or its constant
                            self.check_bits_in_var(value, bits, mask)?
                        }
                        Err(name) => match self.inverse(bits, value.clone(), &name) {
                            Some((bits, actions, _)) => {
                                actions
                                    .into_iter()
                                    .for_each(|act| self.add_entry_action(act));

                                self.use_decode_bits(&name, bits, mask, &mut delayed)?;
                            }
                            None => {
                                return Err(format!("expression {value} not supported",));
                            }
                        },
                    }

                    if self.irp.general_spec.lsb {
                        offset += length;
                    }
                }
            }

            for action in delayed {
                match &action {
                    Action::AssertEq { left, right } => {
                        self.have_definitions(left)?;
                        self.have_definitions(right)?;
                    }
                    Action::Set { expr, .. } => {
                        self.have_definitions(expr)?;
                    }
                    _ => (),
                }
                self.add_entry_action(action);
            }

            pos += expr_count;
        }

        Ok(())
    }

    /// Fetch the length for a bitfield. If the length is not a constant, it has
    /// to be a parameter with constant min and max.
    fn bit_field_length(
        &self,
        length: &Rc<Expression>,
    ) -> Result<(i64, i64, Option<String>), String> {
        let length = self.const_folding(length);

        match length.as_ref() {
            Expression::Number(v) => Ok((*v, *v, None)),
            Expression::Identifier(name) => {
                if let Some(param) = self.irp.parameters.iter().find(|def| def.name == *name) {
                    let min = param.min;
                    let max = param.max;

                    if min > max {
                        Err(format!("parameter {name} has min > max ({min} > {max})",))
                    } else {
                        Ok((min, max, Some(name.to_owned())))
                    }
                } else {
                    Err(format!("bit field length {name} is not a parameter"))
                }
            }
            expr => Err(format!("bit field length {expr} not known")),
        }
    }

    fn use_decode_bits(
        &mut self,
        name: &str,
        mut bits: Rc<Expression>,
        mask: i64,
        delayed: &mut Vec<Action>,
    ) -> Result<(), String> {
        if let Some(def) = self.definitions.get(name) {
            let def = def.clone();

            let expr = if self.any_field_set(name, !mask) {
                Rc::new(Expression::BitwiseOr(
                    Rc::new(Expression::Identifier(name.to_owned())),
                    bits.clone(),
                ))
            } else {
                // clippy 1.70.0 incorrectly warns about this
                #[allow(clippy::redundant_clone)]
                bits.clone()
            };

            self.add_entry_action(Action::Set {
                var: name.to_owned(),
                expr: self.const_folding(&expr),
            });

            if !self.is_any_set(name) {
                bits = Rc::new(Expression::Identifier(name.to_owned()));
            }

            self.set(name, mask);

            let def = self.const_folding(&def);

            let left = self.const_folding(&Rc::new(Expression::BitwiseAnd(
                def.clone(),
                Rc::new(Expression::Number(mask)),
            )));

            let action = Action::AssertEq {
                left,
                right: self.const_folding(&bits),
            };

            if self.have_definitions(&def).is_ok() {
                self.add_entry_action(action);
            } else {
                delayed.push(action);
            }
        } else if self.all_field_set(name, mask) {
            let left = self.const_folding(&Rc::new(Expression::BitwiseAnd(
                Rc::new(Expression::Identifier(name.to_owned())),
                Rc::new(Expression::Number(mask)),
            )));

            self.add_entry_action(Action::AssertEq {
                left,
                right: self.const_folding(&bits),
            });
        } else {
            let expr = if self.any_field_set(name, !mask) {
                Rc::new(Expression::BitwiseOr(
                    Rc::new(Expression::Identifier(name.to_owned())),
                    bits,
                ))
            } else {
                bits
            };

            self.set(name, mask);

            self.add_entry_action(Action::Set {
                var: name.to_owned(),
                expr: self.const_folding(&expr),
            });
        }

        Ok(())
    }

    fn check_bits_in_var(
        &mut self,
        value: Rc<Expression>,
        bits: Rc<Expression>,
        mask: i64,
    ) -> Result<(), String> {
        let left = bits;
        let right = self.const_folding(&Rc::new(Expression::BitwiseAnd(
            value,
            Rc::new(Expression::Number(mask)),
        )));
        self.add_entry_action(Action::AssertEq { left, right });

        Ok(())
    }

    /// Look for a definition of name in the list of definitions. If found,
    /// add it to the actions. This function works recursively, so if a definition
    /// requires a further definition, then that definition will be included too
    fn add_definition(&mut self, name: &str) -> bool {
        if let Some(def) = self.definitions.get(name) {
            let def = def.clone();
            for _ in 0..2 {
                match self.expression_available(&def, false) {
                    Ok(_) => {
                        trace!("found definition {} = {}", name, def);

                        self.add_entry_action(Action::Set {
                            var: name.to_owned(),
                            expr: self.const_folding(&def),
                        });

                        self.set(name, !0);

                        return true;
                    }
                    Err(name) => {
                        if !self.add_definition(&name) {
                            return false;
                        }
                        // try once more
                    }
                }
            }
        }

        let mut found = false;

        for def in &self.irp.definitions {
            if let Expression::Assignment(var, expr) = def {
                let expr = self.const_folding(expr);

                if let Some((expr, actions, mask)) =
                    self.inverse(Rc::new(Expression::Identifier(var.to_string())), expr, name)
                {
                    if self.expression_available(&expr, false).is_err() {
                        continue;
                    }

                    if let Some(mask) = mask {
                        if self.all_field_set(name, mask) {
                            continue;
                        }
                    }

                    let mut expr = self.const_folding(&expr);

                    if self.is_any_set(name) {
                        expr = Rc::new(Expression::BitwiseOr(
                            Rc::new(Expression::Identifier(name.to_owned())),
                            expr,
                        ));
                    }

                    trace!("found definition {} = {}", name, expr);

                    self.add_entry_action(Action::Set {
                        var: name.to_owned(),
                        expr,
                    });

                    self.set(name, mask.unwrap_or(!0));

                    actions
                        .into_iter()
                        .for_each(|act| self.add_entry_action(act));

                    found = true;

                    // do not break here, there might be more bits to be found in another inverse definition
                }
            }
        }

        found
    }

    /// For given expression, add any required definitions or return an error if
    /// no definitions can be found
    fn have_definitions(&mut self, expr: &Expression) -> Result<(), String> {
        loop {
            match self.expression_available(expr, false) {
                Ok(_) => {
                    return Ok(());
                }
                Err(name) => {
                    if !self.add_definition(&name) {
                        return Err(format!("variable ‘{name}’ not known"));
                    }
                }
            }
        }
    }

    /// Add the bit specs for bitfields. max specifies the maximum number of
    /// bits, min specifies a minimum if the number of bits is not fixed at max.
    /// The length can be stored in the variable name store_length if set.
    fn decode_bits(
        &mut self,
        min: Option<i64>,
        max: i64,
        reverse: bool,
        store_length: Option<String>,
        bit_spec: &[&[Rc<Expression>]],
        last: bool,
    ) -> Result<(), String> {
        self.cur.seen_edges = true;

        let width = match bit_spec[0].len() {
            2 => 1,
            4 => 2,
            8 => 3,
            16 => 4,
            w => {
                return Err(format!("bit spec with {w} fields not supported"));
            }
        };

        if max == 1 && min.is_none() {
            let next = self.add_vertex();

            for (bit, e) in bit_spec[0].iter().enumerate() {
                self.push_location();

                self.expression(e, &bit_spec[1..], last)?;

                self.add_entry_action(Action::Set {
                    var: String::from("$bits"),
                    expr: Rc::new(Expression::Number(bit as i64)),
                });

                self.add_edge(Edge {
                    dest: next,
                    actions: vec![],
                });

                self.pop_location();
            }

            self.set_head(next);

            return Ok(());
        }

        let before = self.cur.head;

        let entry = self.add_vertex();

        let next = self.add_vertex();

        let done = self.add_vertex();

        self.add_edge(Edge {
            dest: entry,
            actions: vec![],
        });

        self.set_head(entry);

        if let Some(length) = &store_length {
            self.set(length, !0);
        }

        let length = store_length.unwrap_or_else(|| String::from("$b"));

        for (bit, e) in bit_spec[0].iter().enumerate() {
            self.push_location();

            self.expression(e, &bit_spec[1..], last)?;

            self.add_entry_action(Action::Set {
                var: String::from("$v"),
                expr: Rc::new(Expression::Number(bit as i64)),
            });

            self.add_edge(Edge {
                dest: next,
                actions: vec![],
            });

            self.pop_location();
        }

        self.add_action_at_node(
            before,
            Action::Set {
                var: length.to_owned(),
                expr: Rc::new(Expression::Number(0)),
            },
        );

        self.add_action_at_node(
            before,
            Action::Set {
                var: String::from("$bits"),
                expr: Rc::new(Expression::Number(0)),
            },
        );

        if !(self.irp.general_spec.lsb ^ reverse) {
            self.add_action_at_node(
                next,
                Action::Set {
                    var: String::from("$bits"),
                    expr: Rc::new(Expression::BitwiseOr(
                        Rc::new(Expression::ShiftLeft(
                            Rc::new(Expression::Identifier(String::from("$bits"))),
                            Rc::new(Expression::Number(width)),
                        )),
                        Rc::new(Expression::Identifier(String::from("$v"))),
                    )),
                },
            );
        } else {
            self.add_action_at_node(
                next,
                Action::Set {
                    var: String::from("$bits"),
                    expr: Rc::new(Expression::BitwiseOr(
                        Rc::new(Expression::Identifier(String::from("$bits"))),
                        Rc::new(Expression::ShiftLeft(
                            Rc::new(Expression::Identifier(String::from("$v"))),
                            Rc::new(Expression::Identifier(length.to_owned())),
                        )),
                    )),
                },
            );
        }

        self.add_action_at_node(
            next,
            Action::Set {
                var: length.to_owned(),
                expr: Rc::new(Expression::Add(
                    Rc::new(Expression::Identifier(length.to_owned())),
                    Rc::new(Expression::Number(width)),
                )),
            },
        );

        self.add_edge_at_node(
            next,
            Edge {
                dest: entry,
                actions: vec![Action::AssertEq {
                    left: Rc::new(Expression::Less(
                        Rc::new(Expression::Identifier(length.to_owned())),
                        Rc::new(Expression::Number(max)),
                    )),
                    right: Rc::new(Expression::Number(1)),
                }],
            },
        );

        self.add_edge_at_node(
            next,
            Edge {
                dest: done,
                actions: vec![Action::AssertEq {
                    left: Rc::new(Expression::Less(
                        Rc::new(Expression::Identifier(length.to_owned())),
                        Rc::new(Expression::Number(max)),
                    )),
                    right: Rc::new(Expression::Number(0)),
                }],
            },
        );

        if let Some(min) = min {
            self.add_edge_at_node(
                next,
                Edge {
                    dest: done,
                    actions: vec![Action::AssertEq {
                        left: Rc::new(Expression::GreaterEqual(
                            Rc::new(Expression::Identifier(length)),
                            Rc::new(Expression::Number(min)),
                        )),
                        right: Rc::new(Expression::Number(1)),
                    }],
                },
            );
        }

        self.set_head(done);

        Ok(())
    }

    fn build(&mut self, event: Event, expr: &Expression) -> Result<(), String> {
        // find all extents
        self.extents = Vec::new();

        expr.visit(self, true, &|expr, builder: &mut Builder| {
            if let Expression::ExtentConstant(v, u) = expr {
                builder
                    .extents
                    .insert(0, u.eval_rational(v, &builder.irp.general_spec).unwrap());
            }
        });

        let node = self.add_vertex();

        self.add_edge(Edge {
            dest: node,
            actions: vec![],
        });

        self.set_head(node);

        // start with the first extent
        self.next_extent();

        self.expression(expr, &[], true)?;

        if self.cur.seen_edges {
            self.add_done(event)?;

            self.add_edge(Edge {
                dest: 0,
                actions: vec![],
            });

            self.set_head(0);
            self.cur.seen_edges = false;
        }

        Ok(())
    }

    fn next_extent(&mut self) {
        if let Some(v) = self.extents.pop() {
            self.add_entry_action(Action::Set {
                var: "$extent".to_owned(),
                expr: Rc::new(Expression::Number(v)),
            });

            self.set("$extent", !0);
        } else {
            self.unset("$extent");
        }
    }

    fn expression(
        &mut self,
        expr: &Expression,
        bit_spec: &[&[Rc<Expression>]],
        last: bool,
    ) -> Result<(), String> {
        match expr {
            Expression::Stream(stream) => {
                let repeats = match stream.repeat {
                    Some(RepeatMarker::Count(n)) => n,
                    None => 1,
                    _ => unreachable!(),
                };

                let mut bit_spec = bit_spec.to_vec();

                if !stream.bit_spec.is_empty() {
                    bit_spec.insert(0, &stream.bit_spec);
                }

                for n in 0..repeats {
                    self.expression_list(&stream.stream, &bit_spec, last && (n == repeats - 1))?;
                }
            }
            Expression::List(list) => {
                self.expression_list(list, bit_spec, last)?;
            }
            Expression::FlashConstant(v, u) => {
                self.cur.seen_edges = true;

                let len = u.eval_rational(v, &self.irp.general_spec)?;

                let node = self.add_vertex();

                self.add_edge(Edge {
                    dest: node,
                    actions: vec![Action::Flash {
                        length: Length::Expression(Rc::new(Expression::Number(len))),
                        complete: last,
                    }],
                });

                self.set_head(node);

                self.subtract_extent(Rc::new(Expression::Number(len)));
            }
            Expression::GapConstant(v, u) => {
                self.cur.seen_edges = true;

                let len = u.eval_rational(v, &self.irp.general_spec)?;

                let node = self.add_vertex();

                self.add_edge(Edge {
                    dest: node,
                    actions: vec![Action::Gap {
                        length: Length::Expression(Rc::new(Expression::Number(len))),
                        complete: last,
                    }],
                });

                self.set_head(node);

                self.subtract_extent(Rc::new(Expression::Number(len)));
            }
            Expression::FlashIdentifier(var, unit) => {
                if !self.is_any_set(var) {
                    return Err(format!("variable ‘{var}’ is not set"));
                }

                let unit = unit.eval(1, &self.irp.general_spec)?;

                let mut expr = Rc::new(Expression::Identifier(var.to_owned()));

                if unit != 1 {
                    expr = Rc::new(Expression::Multiply(
                        expr,
                        Rc::new(Expression::Number(unit)),
                    ));
                }

                let node = self.add_vertex();

                self.add_edge(Edge {
                    dest: node,
                    actions: vec![Action::Flash {
                        length: Length::Expression(expr),
                        complete: last,
                    }],
                });

                self.set_head(node);

                let mut expr = Rc::new(Expression::Identifier(var.to_owned()));

                if unit != 1 {
                    expr = Rc::new(Expression::Multiply(
                        expr,
                        Rc::new(Expression::Number(unit)),
                    ));
                }

                self.subtract_extent(expr);
            }
            Expression::GapIdentifier(var, unit) => {
                if !self.is_any_set(var) {
                    return Err(format!("variable ‘{var}’ is not set"));
                }

                let unit = unit.eval(1, &self.irp.general_spec)?;

                let node = self.add_vertex();

                let mut expr = Rc::new(Expression::Identifier(var.to_owned()));

                if unit != 1 {
                    expr = Rc::new(Expression::Multiply(
                        expr,
                        Rc::new(Expression::Number(unit)),
                    ));
                }

                self.add_edge(Edge {
                    dest: node,
                    actions: vec![Action::Gap {
                        length: Length::Expression(expr),
                        complete: last,
                    }],
                });

                self.set_head(node);

                let mut expr = Rc::new(Expression::Identifier(var.to_owned()));

                if unit != 1 {
                    expr = Rc::new(Expression::Multiply(
                        expr,
                        Rc::new(Expression::Number(unit)),
                    ));
                }

                self.subtract_extent(expr);
            }
            Expression::BitField { length, .. } => {
                let length = length.eval(&Vartable::new())?;

                self.decode_bits(None, length, false, None, bit_spec, last)?;
            }
            Expression::ExtentConstant(_, _) => {
                self.cur.seen_edges = true;

                let node = self.add_vertex();

                self.add_edge(Edge {
                    dest: node,
                    actions: vec![Action::Gap {
                        length: Length::Expression(Rc::new(Expression::Identifier(
                            "$extent".into(),
                        ))),
                        complete: last,
                    }],
                });

                self.next_extent();

                self.set_head(node);
            }
            Expression::Assignment(var, expr) => {
                if self.is_any_set(var) && self.irp.parameters.iter().any(|p| &p.name == var) {
                    return Ok(());
                }

                self.have_definitions(expr)?;

                self.add_entry_action(Action::Set {
                    var: var.to_owned(),
                    expr: self.const_folding(expr),
                });

                self.set(var, !0);
            }
            _ => return Err(format!("expression {expr} not supported")),
        }

        Ok(())
    }

    /// Do we have all the vars to evaluate an expression, i.e. can this
    /// expression be evaluated now.
    fn expression_available(
        &self,
        expr: &Expression,
        ignore_definitions: bool,
    ) -> Result<(), String> {
        match expr {
            Expression::FlashConstant(..)
            | Expression::GapConstant(..)
            | Expression::ExtentConstant(..)
            | Expression::Number(..) => Ok(()),
            Expression::FlashIdentifier(name, ..)
            | Expression::GapIdentifier(name, ..)
            | Expression::ExtentIdentifier(name, ..)
            | Expression::Identifier(name) => {
                if (name.starts_with('$') || self.cur.vars.contains_key(name))
                    && (!ignore_definitions || !self.definitions.contains_key(name))
                {
                    Ok(())
                } else {
                    Err(name.to_owned())
                }
            }
            Expression::BitField {
                value,
                length,
                offset,
                ..
            } => {
                if let Some(res) = self.bitfield_known(value, length, offset, ignore_definitions) {
                    res
                } else {
                    if let Some(offset) = &offset {
                        self.expression_available(offset, ignore_definitions)?;
                    }
                    self.expression_available(value, ignore_definitions)?;
                    self.expression_available(length, ignore_definitions)
                }
            }
            Expression::InfiniteBitField { value, offset } => {
                self.expression_available(value, ignore_definitions)?;
                self.expression_available(offset, ignore_definitions)
            }
            Expression::Assignment(_, expr)
            | Expression::Complement(expr)
            | Expression::Not(expr)
            | Expression::Negative(expr)
            | Expression::BitCount(expr)
            | Expression::Log2(expr)
            | Expression::BitReverse(expr, ..) => {
                self.expression_available(expr, ignore_definitions)
            }
            Expression::Add(left, right)
            | Expression::Subtract(left, right)
            | Expression::Multiply(left, right)
            | Expression::Power(left, right)
            | Expression::Divide(left, right)
            | Expression::Modulo(left, right)
            | Expression::ShiftLeft(left, right)
            | Expression::ShiftRight(left, right)
            | Expression::LessEqual(left, right)
            | Expression::Less(left, right)
            | Expression::Greater(left, right)
            | Expression::GreaterEqual(left, right)
            | Expression::NotEqual(left, right)
            | Expression::Equal(left, right)
            | Expression::BitwiseAnd(left, right)
            | Expression::BitwiseOr(left, right)
            | Expression::BitwiseXor(left, right)
            | Expression::Or(left, right)
            | Expression::And(left, right) => {
                self.expression_available(left, ignore_definitions)?;
                self.expression_available(right, ignore_definitions)
            }
            Expression::Conditional(cond, left, right) => {
                self.expression_available(cond, ignore_definitions)?;
                self.expression_available(left, ignore_definitions)?;
                self.expression_available(right, ignore_definitions)
            }
            Expression::List(list) => {
                for expr in list {
                    self.expression_available(expr, ignore_definitions)?;
                }
                Ok(())
            }
            Expression::Variation(list) => {
                for expr in list.iter().flatten() {
                    self.expression_available(expr, ignore_definitions)?;
                }
                Ok(())
            }
            Expression::Stream(stream) => {
                for expr in &stream.bit_spec {
                    self.expression_available(expr, ignore_definitions)?;
                }
                for expr in &stream.stream {
                    self.expression_available(expr, ignore_definitions)?;
                }
                Ok(())
            }
        }
    }

    fn bitfield_known(
        &self,
        value: &Rc<Expression>,
        length: &Rc<Expression>,
        offset: &Option<Rc<Expression>>,
        ignore_definitions: bool,
    ) -> Option<Result<(), String>> {
        let name = match value.as_ref() {
            Expression::Identifier(name) => name,
            Expression::Complement(expr) => {
                if let Expression::Identifier(name) = expr.as_ref() {
                    name
                } else {
                    return None;
                }
            }
            _ => {
                return None;
            }
        };

        let length = if let Expression::Number(v) = self.const_folding(length).as_ref() {
            *v
        } else {
            return None;
        };

        let offset = if let Some(offset) = offset {
            if let Expression::Number(v) = self.const_folding(offset).as_ref() {
                *v
            } else {
                return None;
            }
        } else {
            0
        };

        let mask = gen_mask(length) << offset;

        if self.all_field_set(name, mask)
            && (!ignore_definitions || !self.definitions.contains_key(name))
        {
            Some(Ok(()))
        } else {
            Some(Err(name.to_string()))
        }
    }

    /// For the given parameter, get the mask
    pub fn param_to_mask(&self, param: &ParameterSpec) -> Result<i64, String> {
        Ok((param.max as u64 + 1).next_power_of_two() as i64 - 1)
    }

    /// Mask results
    fn mask_results(&mut self) -> Result<(), String> {
        for param in &self.irp.parameters {
            let mask = self.param_to_mask(param)?;

            if let Some(fields) = self.cur.vars.get(&param.name) {
                if (fields & !mask) != 0 {
                    self.add_entry_action(Action::Set {
                        var: param.name.to_owned(),
                        expr: Rc::new(Expression::BitwiseAnd(
                            Rc::new(Expression::Identifier(param.name.to_owned())),
                            Rc::new(Expression::Number(mask)),
                        )),
                    });
                }
            }
        }

        Ok(())
    }

    /// Remove some value of the extend
    fn subtract_extent(&mut self, expr: Rc<Expression>) {
        if self.is_any_set("$extent") {
            let expr = Rc::new(Expression::Subtract(
                Rc::new(Expression::Identifier("$extent".to_owned())),
                expr,
            ));

            self.add_entry_action(Action::Set {
                var: "$extent".to_owned(),
                expr,
            });
        }
    }

    /// Constant folding of expressions. This simply folds constants values and a few
    /// expression. More to be done.
    pub fn const_folding(&self, expr: &Rc<Expression>) -> Rc<Expression> {
        macro_rules! unary {
            ($expr:expr, $op:ident) => {{
                if let Expression::Number(expr) = $expr.as_ref() {
                    Some(Rc::new(Expression::Number(expr.$op().into())))
                } else {
                    None
                }
            }};
        }

        macro_rules! binary {
            ($left:expr, $right:expr, $op:ident) => {{
                if let (Expression::Number(left), Expression::Number(right)) =
                    ($left.as_ref(), $right.as_ref())
                {
                    Some(Rc::new(Expression::Number(left.$op(right))))
                } else {
                    None
                }
            }};
        }

        let new = clone_filter(expr, &|expr| match expr.as_ref() {
            Expression::Identifier(name) => {
                let val = self.constants.get(name).ok()?;
                Some(Rc::new(Expression::Number(val)))
            }
            Expression::Complement(expr) => unary!(expr, not),
            Expression::BitCount(expr) => unary!(expr, count_ones),
            Expression::Negative(expr) => unary!(expr, neg),

            Expression::Add(left, right) => binary!(left, right, add),
            Expression::Subtract(left, right) => binary!(left, right, sub),
            Expression::Modulo(left, right) => binary!(left, right, rem),
            Expression::Multiply(left, right) => {
                // fold multiply by power-of-two into shift
                match (left.as_ref(), right.as_ref()) {
                    (Expression::Number(left), Expression::Number(right)) => {
                        Some(Rc::new(Expression::Number(left * right)))
                    }
                    (Expression::Number(no), _) if (*no as u64).is_power_of_two() => {
                        Some(Rc::new(Expression::ShiftLeft(
                            right.clone(),
                            Rc::new(Expression::Number((*no).trailing_zeros() as i64)),
                        )))
                    }
                    (_, Expression::Number(no)) if (*no as u64).is_power_of_two() => {
                        Some(Rc::new(Expression::ShiftLeft(
                            left.clone(),
                            Rc::new(Expression::Number((*no).trailing_zeros() as i64)),
                        )))
                    }
                    _ => None,
                }
            }
            Expression::Divide(left, right) => {
                // fold divide by power-of-two into shift
                match (left.as_ref(), right.as_ref()) {
                    (Expression::Number(left), Expression::Number(right)) => {
                        Some(Rc::new(Expression::Number(left / right)))
                    }
                    (_, Expression::Number(no)) if (*no as u64).is_power_of_two() => {
                        Some(Rc::new(Expression::ShiftRight(
                            left.clone(),
                            Rc::new(Expression::Number((*no).trailing_zeros() as i64)),
                        )))
                    }
                    _ => None,
                }
            }
            Expression::BitwiseAnd(left, right) => binary!(left, right, bitand),
            Expression::BitwiseOr(left, right) => binary!(left, right, bitor),
            Expression::BitwiseXor(left, right) => binary!(left, right, bitxor),
            Expression::ShiftLeft(left, right) => binary!(left, right, shl),
            Expression::ShiftRight(left, right) => binary!(left, right, shr),
            // TODO: logical (Not, And, Or, Equal, More, Ternary)
            _ => None,
        });

        new.unwrap_or_else(|| expr.clone())
    }
}

impl fmt::Display for Length {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Length::Expression(e) => write!(f, "{e}"),
            Length::Range(min, None) => write!(f, "{min}.."),
            Length::Range(min, Some(max)) => write!(f, "{min}..{max}"),
        }
    }
}
