use super::{
    build_dfa::DFA,
    build_nfa::{Action, Length},
    Event, Expression, Options,
};
use inkwell::{
    builder,
    context::Context,
    module::Module,
    targets::{CodeModel, FileType, RelocMode, Target, TargetTriple},
    types::{BasicType, StructType},
    values::{BasicValue, GlobalValue, IntValue, PointerValue},
    AddressSpace, IntPredicate, OptimizationLevel,
};
use once_cell::sync::OnceCell;
use std::{collections::HashMap, fs::File, io::Write, rc::Rc};

static LLVM_INIT: OnceCell<()> = OnceCell::new();
static LLVM_TARGET_TRIPLE: &str = "bpf-unknown-unknown";

impl DFA {
    /// Compile the DFA to a BPF program for Linux kernel IR decoding
    pub fn compile_bpf(&self, options: &Options) -> Result<(Vec<u8>, Vec<String>), String> {
        LLVM_INIT.get_or_init(|| {
            Target::initialize_bpf(&Default::default());
        });

        let context = Context::create();
        let module = context.create_module(options.name);
        module.set_source_file_name(options.source);
        let vars = find_all_vars(self);

        let target_triple = TargetTriple::create(LLVM_TARGET_TRIPLE);
        module.set_triple(&target_triple);

        let (map, decoder_state_ty) = define_map_def(&module, &vars, &context);
        define_license(&module, &context);

        let builder = context.create_builder();

        let mut builder = Builder {
            dfa: self,
            options,
            module,
            builder,
            vars,
            decoder_state_ty,
            decoder_state: context
                .bool_type()
                .ptr_type(AddressSpace::default())
                .const_null(),
        };

        builder.define_function(map, &context);

        if let Some(path) = options.llvm_ir {
            builder.module.print_to_file(path).unwrap();
        }

        let target = Target::from_name("bpf").unwrap();

        let target_machine = target
            .create_target_machine(
                &target_triple,
                "v3",
                "+alu32",
                OptimizationLevel::Default,
                RelocMode::Default,
                CodeModel::Default,
            )
            .unwrap();

        if let Some(path) = options.assembly {
            let code = target_machine.write_to_memory_buffer(&builder.module, FileType::Assembly);

            match code {
                Ok(mem_buf) => {
                    let slice = mem_buf.as_slice();

                    let mut file = match File::create(path) {
                        Ok(file) => file,
                        Err(e) => return Err(e.to_string()),
                    };

                    file.write_all(slice).unwrap();
                }
                Err(e) => return Err(e.to_string()),
            }
        }

        let code = target_machine.write_to_memory_buffer(&builder.module, FileType::Object);

        match code {
            Ok(mem_buf) => {
                let slice = mem_buf.as_slice();

                if let Some(path) = options.object {
                    let mut file = match File::create(path) {
                        Ok(file) => file,
                        Err(e) => return Err(e.to_string()),
                    };

                    file.write_all(slice).unwrap();
                }

                let mut vars = vec![String::new(); builder.vars.len()];

                builder.vars.iter().for_each(|(k, v)| {
                    vars[v.offset] = k.to_string();
                });

                Ok((slice.to_vec(), vars))
            }
            Err(e) => Err(e.to_string()),
        }
    }
}

struct Builder<'a> {
    options: &'a Options<'a>,
    dfa: &'a DFA,
    module: Module<'a>,
    builder: builder::Builder<'a>,
    decoder_state_ty: StructType<'a>,
    decoder_state: PointerValue<'a>,
    vars: HashMap<&'a str, VarValue<'a>>,
}

#[derive(Default, Debug)]
struct VarValue<'a> {
    offset: usize,
    value: Option<IntValue<'a>>,
    dirty: bool,
}

impl<'a> Builder<'a> {
    fn define_function(&mut self, map_def: GlobalValue<'a>, context: &'a Context) {
        let i32 = context.i32_type();
        let i64 = context.i64_type();
        let i32_ptr = context.i32_type().ptr_type(AddressSpace::default());
        let i64_ptr = context.i64_type().ptr_type(AddressSpace::default());

        let fn_type = i32.fn_type(&[i32_ptr.into()], false);

        let function = self.module.add_function("bpf_decoder", fn_type, None);

        function.set_section(Some(&format!("lirc_mode2/{}", self.options.name)));

        let entry = context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);

        // get map entry 0 (which we use as the decoder state)
        let key = self.builder.build_alloca(i32, "key").unwrap();
        self.builder.build_store(key, i32.const_zero()).unwrap();

        let fn_type = i64_ptr.fn_type(&[i32_ptr.into(), i32_ptr.into()], false);

        let bpf_map_lookup_elem = i64.const_int(1, false).const_to_pointer(i32_ptr);

        let decoder_state = self
            .builder
            .build_indirect_call(
                fn_type,
                bpf_map_lookup_elem,
                &[map_def.as_pointer_value().into(), key.into()],
                "decoder_state",
            )
            .unwrap()
            .try_as_basic_value()
            .left()
            .unwrap()
            .into_pointer_value();

        let non_zero_key = context.append_basic_block(function, "non_zero_key");
        let zero_key = context.append_basic_block(function, "zero_key");

        let res = self
            .builder
            .build_int_compare(
                IntPredicate::NE,
                decoder_state,
                i32_ptr.const_null(),
                "not_null",
            )
            .unwrap();

        self.builder
            .build_conditional_branch(res, non_zero_key, zero_key)
            .unwrap();

        self.builder.position_at_end(zero_key);
        self.builder
            .build_return(Some(&i32.const_zero().as_basic_value_enum()))
            .unwrap();

        self.builder.position_at_end(non_zero_key);

        // we now have a valid decoder map
        self.decoder_state = decoder_state;

        // load the lirc mode2 value and check its type
        let lirc = self
            .builder
            .build_load(
                i32,
                function.get_first_param().unwrap().into_pointer_value(),
                "lirc",
            )
            .unwrap()
            .into_int_value();

        let lirc_ty = self
            .builder
            .build_right_shift(lirc, i32.const_int(24, false), false, "lirc_ty")
            .unwrap();

        let lirc_ok = context.append_basic_block(function, "lirc_ok");
        let error = context.append_basic_block(function, "error");

        self.builder
            .build_switch(
                lirc_ty,
                zero_key,
                &[
                    (i32.const_zero(), lirc_ok),
                    (i32.const_int(1, false), lirc_ok),
                    (i32.const_int(3, false), lirc_ok),
                    (i32.const_int(4, false), error),
                ],
            )
            .unwrap();

        self.builder.position_at_end(lirc_ok);

        // we known the lirc mode2 value is ok,

        let length = self
            .builder
            .build_and(lirc, i32.const_int(0x00ff_ffff, false), "length")
            .unwrap();

        let is_pulse = self
            .builder
            .build_int_compare(
                IntPredicate::EQ,
                lirc_ty,
                i32.const_int(1, false),
                "is_pulse",
            )
            .unwrap();

        // load the current state and switch to it
        let load = self
            .builder
            .build_load(
                i64,
                self.builder
                    .build_struct_gep(self.decoder_state_ty, decoder_state, 0, "state")
                    .unwrap(),
                "state",
            )
            .unwrap();

        load.as_instruction_value()
            .unwrap()
            .set_alignment(8)
            .unwrap();

        let state = load.into_int_value();

        let mut cases = Vec::new();

        // we will add a switch statement at the end of lirc ok once we have built all the cases
        for (state_no, vert) in self.dfa.verts.iter().enumerate() {
            let block = context.append_basic_block(function, &format!("state_{state_no}_entry"));
            self.builder.position_at_end(block);

            cases.push((i64.const_int(state_no as u64, false), block));

            for edge in &vert.edges {
                let next = context.append_basic_block(function, "next");

                for action in &edge.actions {
                    match action {
                        Action::Flash {
                            length: Length::Range(min, max),
                            ..
                        } => {
                            let ok = context.append_basic_block(function, "ok");

                            self.builder
                                .build_conditional_branch(is_pulse, ok, next)
                                .unwrap();

                            self.builder.position_at_end(ok);

                            let ok = context.append_basic_block(function, "ok");

                            let res = self
                                .builder
                                .build_int_compare(
                                    IntPredicate::UGE,
                                    length,
                                    i32.const_int(*min as u64, false),
                                    "min",
                                )
                                .unwrap();

                            self.builder
                                .build_conditional_branch(res, ok, next)
                                .unwrap();

                            self.builder.position_at_end(ok);

                            if let Some(max) = max {
                                let ok = context.append_basic_block(function, "ok");

                                let res = self
                                    .builder
                                    .build_int_compare(
                                        IntPredicate::ULE,
                                        length,
                                        i32.const_int(*max as u64, false),
                                        "max",
                                    )
                                    .unwrap();

                                self.builder
                                    .build_conditional_branch(res, ok, next)
                                    .unwrap();

                                self.builder.position_at_end(ok);
                            }
                        }
                        Action::Gap {
                            length: Length::Range(min, max),
                            ..
                        } => {
                            let ok = context.append_basic_block(function, "ok");

                            self.builder
                                .build_conditional_branch(is_pulse, next, ok)
                                .unwrap();

                            self.builder.position_at_end(ok);

                            let ok = context.append_basic_block(function, "ok");

                            let res = self
                                .builder
                                .build_int_compare(
                                    IntPredicate::UGE,
                                    length,
                                    i32.const_int(*min as u64, false),
                                    "min",
                                )
                                .unwrap();

                            self.builder
                                .build_conditional_branch(res, ok, next)
                                .unwrap();

                            self.builder.position_at_end(ok);

                            if let Some(max) = max {
                                let ok = context.append_basic_block(function, "ok");

                                let res = self
                                    .builder
                                    .build_int_compare(
                                        IntPredicate::ULE,
                                        length,
                                        i32.const_int(*max as u64, false),
                                        "max",
                                    )
                                    .unwrap();

                                self.builder
                                    .build_conditional_branch(res, ok, next)
                                    .unwrap();

                                self.builder.position_at_end(ok);
                            }
                        }
                        Action::Set { var, expr } => {
                            let value = self.emit(expr, context);
                            self.update_var(var, value);
                        }
                        Action::AssertEq { left, right } => {
                            let left = self.emit(left, context);
                            let right = self.emit(right, context);

                            let ok = context.append_basic_block(function, "ok");

                            let res = self
                                .builder
                                .build_int_compare(IntPredicate::EQ, left, right, "eq")
                                .unwrap();

                            self.builder
                                .build_conditional_branch(res, ok, next)
                                .unwrap();

                            self.builder.position_at_end(ok);
                        }
                        Action::Done(Event::Repeat, vars) if vars.is_empty() => {
                            let fn_type = i32.fn_type(&[], false);

                            let bpf_rc_repeat = i64.const_int(77, false).const_to_pointer(i32_ptr);

                            self.builder
                                .build_indirect_call(fn_type, bpf_rc_repeat, &[], "")
                                .unwrap();
                        }
                        Action::Done(ev, _) => {
                            let flags = if self.vars.contains_key("T") {
                                // T should be 0 or 1; this corresponds with LIRC_SCANCODE_FA
                                self.load_var("T", context)
                            } else {
                                context.i64_type().const_zero()
                            };

                            let flags = self
                                .builder
                                .build_or(
                                    flags,
                                    match ev {
                                        Event::Down => context.i64_type().const_zero(),
                                        Event::Repeat => context.i64_type().const_int(2, false),
                                        Event::Up => context.i64_type().const_int(4, false),
                                    },
                                    "",
                                )
                                .unwrap();

                            let code = self.load_var("CODE", context);

                            let protocol = context
                                .i32_type()
                                .const_int(self.options.protocol as u64, false);

                            let fn_type = i32.fn_type(&[i32.into(), i64.into(), i64.into()], false);

                            let bpf_rc_keydown = i64.const_int(78, false).const_to_pointer(i32_ptr);

                            self.builder
                                .build_indirect_call(
                                    fn_type,
                                    bpf_rc_keydown,
                                    &[protocol.into(), code.into(), flags.into()],
                                    "",
                                )
                                .unwrap();
                        }
                        _ => unimplemented!(),
                    }
                }

                assert_eq!(vert.entry.len(), 0);

                self.write_dirty();
                self.clear_vars();

                self.builder
                    .build_store(
                        self.builder
                            .build_struct_gep(self.decoder_state_ty, decoder_state, 0, "state")
                            .unwrap(),
                        i64.const_int(edge.dest as u64, false),
                    )
                    .unwrap()
                    .set_alignment(8)
                    .unwrap();

                self.builder
                    .build_return(Some(&i32.const_zero().as_basic_value_enum()))
                    .unwrap();

                self.builder.position_at_end(next);
            }

            self.builder.build_unconditional_branch(error).unwrap();
        }

        self.builder.position_at_end(lirc_ok);

        self.builder.build_switch(state, error, &cases).unwrap();

        // error path for decoder reset
        self.builder.position_at_end(error);

        self.builder
            .build_store(
                self.builder
                    .build_struct_gep(self.decoder_state_ty, decoder_state, 0, "$state")
                    .unwrap(),
                i64.const_zero(),
            )
            .unwrap()
            .set_alignment(8)
            .unwrap();

        self.builder
            .build_return(Some(&i32.const_zero().as_basic_value_enum()))
            .unwrap();
    }

    fn emit(&mut self, expr: &'a Rc<Expression>, context: &'a Context) -> IntValue<'a> {
        macro_rules! unary {
            ($expr:expr,  $op:ident) => {{
                let expr = self.emit($expr, context);

                self.builder.$op(expr, "").unwrap()
            }};
        }

        macro_rules! binary {
            ($left:expr, $right:expr, $op:ident) => {{
                let left = self.emit($left, context);
                let right = self.emit($right, context);

                self.builder.$op(left, right, "").unwrap()
            }};
        }

        macro_rules! compare {
            ($left:expr, $right:expr, $pred:path) => {{
                let left = self.emit($left, context);
                let right = self.emit($right, context);

                self.builder
                    .build_int_z_extend(
                        self.builder
                            .build_int_compare($pred, left, right, "")
                            .unwrap(),
                        context.i64_type(),
                        "",
                    )
                    .unwrap()
            }};
        }

        match expr.as_ref() {
            Expression::Number(n) => context.i64_type().const_int(*n as u64, false),

            Expression::Complement(expr) => unary!(expr, build_not),
            Expression::Negative(expr) => unary!(expr, build_int_neg),
            Expression::Not(expr) => {
                let expr = self.emit(expr, context);

                self.builder
                    .build_int_z_extend(
                        self.builder
                            .build_int_compare(
                                IntPredicate::EQ,
                                expr,
                                context.i64_type().const_zero(),
                                "",
                            )
                            .unwrap(),
                        context.i64_type(),
                        "",
                    )
                    .unwrap()
            }
            Expression::BitCount(expr) => {
                let expr = self.emit(expr, context);

                let i64 = context.i64_type();

                let fn_type = i64.fn_type(&[i64.into()], false);

                // use llvm popcount builtin - has good code generation
                let function = self.module.add_function("llvm.ctpop.i64", fn_type, None);

                self.builder
                    .build_call(function, &[expr.into()], "")
                    .unwrap()
                    .try_as_basic_value()
                    .left()
                    .unwrap()
                    .into_int_value()
            }

            Expression::Add(left, right) => binary!(left, right, build_int_add),
            Expression::Subtract(left, right) => binary!(left, right, build_int_sub),
            Expression::Multiply(left, right) => binary!(left, right, build_int_mul),
            Expression::Divide(left, right) => binary!(left, right, build_int_signed_div),
            Expression::Modulo(left, right) => binary!(left, right, build_int_signed_rem),

            Expression::BitwiseAnd(left, right) => binary!(left, right, build_and),
            Expression::BitwiseOr(left, right) => binary!(left, right, build_or),
            Expression::BitwiseXor(left, right) => binary!(left, right, build_xor),

            Expression::ShiftLeft(left, right) => binary!(left, right, build_left_shift),
            Expression::ShiftRight(left, right) => {
                let left = self.emit(left, context);
                let right = self.emit(right, context);

                self.builder
                    .build_right_shift(left, right, false, "")
                    .unwrap()
            }

            Expression::Less(left, right) => compare!(left, right, IntPredicate::SLT),
            Expression::LessEqual(left, right) => compare!(left, right, IntPredicate::SLE),
            Expression::Greater(left, right) => compare!(left, right, IntPredicate::SGT),
            Expression::GreaterEqual(left, right) => compare!(left, right, IntPredicate::SGE),
            Expression::Equal(left, right) => compare!(left, right, IntPredicate::EQ),
            Expression::NotEqual(left, right) => compare!(left, right, IntPredicate::NE),
            Expression::Identifier(name) => self.load_var(name, context),

            _ => unimplemented!("{expr}"),
        }
    }

    fn load_var(&mut self, name: &'a str, context: &'a Context) -> IntValue<'a> {
        match self.vars.get_mut(name) {
            Some(e) => {
                if let Some(value) = e.value {
                    return value;
                }

                let load = self
                    .builder
                    .build_load(
                        context.i64_type(),
                        self.builder
                            .build_struct_gep(
                                self.decoder_state_ty,
                                self.decoder_state,
                                e.offset as u32,
                                name,
                            )
                            .unwrap(),
                        name,
                    )
                    .unwrap();

                load.as_instruction_value()
                    .unwrap()
                    .set_alignment(8)
                    .unwrap();

                let value = load.into_int_value();

                e.value = Some(value);

                value
            }
            None => unreachable!(),
        }
    }

    fn update_var(&mut self, name: &'a str, value: IntValue<'a>) {
        match self.vars.get_mut(name) {
            Some(e) => {
                e.value = Some(value);
                e.dirty = true;
            }
            None => unreachable!(),
        }
    }

    fn write_dirty(&self) {
        for (name, e) in &self.vars {
            if e.dirty {
                self.builder
                    .build_store(
                        self.builder
                            .build_struct_gep(
                                self.decoder_state_ty,
                                self.decoder_state,
                                e.offset as u32,
                                name,
                            )
                            .unwrap(),
                        e.value.unwrap(),
                    )
                    .unwrap()
                    .set_alignment(8)
                    .unwrap();
            }
        }
    }

    fn clear_vars(&mut self) {
        for (_, e) in self.vars.iter_mut() {
            e.dirty = false;
            e.value = None;
        }
    }
}

fn find_all_vars<'a>(dfa: &'a DFA) -> HashMap<&'a str, VarValue<'a>> {
    let mut vars: HashMap<&'a str, VarValue<'a>> = HashMap::new();
    vars.insert("$state", VarValue::default());

    let mut next = 1;

    for vert in &dfa.verts {
        for action in vert
            .edges
            .iter()
            .flat_map(|edge| edge.actions.iter())
            .chain(&vert.entry)
        {
            if let Action::Set { var, .. } = action {
                vars.entry(var).or_insert_with(|| {
                    let offset = next;
                    next += 1;
                    VarValue {
                        offset,
                        ..Default::default()
                    }
                });
            }
        }
    }

    vars
}

fn define_map_def<'ctx>(
    module: &Module<'ctx>,
    vars: &HashMap<&str, VarValue>,
    context: &'ctx Context,
) -> (GlobalValue<'ctx>, StructType<'ctx>) {
    let i32 = context.i32_type().as_basic_type_enum();
    let i64 = context.i64_type().as_basic_type_enum();

    let field_types = vec![i32; 7];

    let bpf_map_def = context.struct_type(&field_types, false);

    let gv = module.add_global(
        bpf_map_def,
        Some(AddressSpace::default()),
        "decoder_state_map",
    );

    let i32 = context.i32_type();

    let def = bpf_map_def.const_named_struct(&[
        // BPF_MAP_TYPE_ARRAY
        i32.const_int(2, false).into(),
        // key_size
        i32.const_int(4, false).into(),
        // value_size
        i32.const_int(vars.len() as u64 * 8, false).into(),
        // max_entries
        i32.const_int(1, false).into(),
        // map_flags
        i32.const_zero().into(),
        // padding
        i32.const_zero().into(),
        i32.const_zero().into(),
    ]);

    gv.set_initializer(&def);
    gv.set_section(Some("maps"));
    gv.set_alignment(4);

    let field_types = vec![i64; vars.len()];

    let decoder_state_ty = context.struct_type(&field_types, false);

    (gv, decoder_state_ty)
}

fn define_license<'ctx>(module: &Module<'ctx>, context: &'ctx Context) {
    let ty = context.i8_type().array_type(4);

    let gv = module.add_global(ty, Some(AddressSpace::default()), "_license");

    gv.set_initializer(&context.const_string(b"GPL", true));
    gv.set_section(Some("license"));
}
