#![cfg(feature = "bpf")]

use aya_obj::{
    generated::{bpf_insn, bpf_map_type::BPF_MAP_TYPE_ARRAY},
    Map, Object,
};
use irp::{Irp, Options, Vartable};
use itertools::Itertools;
use num::Integer;
use std::{
    collections::{HashMap, HashSet},
    ops::DerefMut,
    sync::{atomic::AtomicI32, Mutex},
};

#[test]
fn rc5() {
    let irp = "{36k,msb,889}<1,-1|-1,1>(1,1:1,T:1,CODE:11,-20m) [CODE:0..2047,T@:0..1=0]";

    let irp = Irp::parse(irp).unwrap();

    let mut vars = Vartable::new();
    vars.set("CODE".into(), 102);
    let message = irp.encode_raw(vars, 0).unwrap();
    let options = Options {
        name: "rc5",
        source: file!(),
        aeps: 100,
        eps: 3,
        max_gap: 20000,
        ..Default::default()
    };

    let dfa = irp.compile(&options).unwrap();

    let (object, vars) = dfa.compile_bpf(&options).unwrap();

    let mut obj = Object::parse(&object).unwrap();
    let text_sections = HashSet::new();
    obj.relocate_calls(&text_sections).unwrap();

    let maps: HashMap<String, Map> = obj.maps.drain().collect();

    let mut value_size = None;

    let mut rel_maps = Vec::new();

    let map_id = unsafe { TESTS_NEXT_MAP_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst) };

    for (name, map) in maps.iter() {
        let Map::Legacy(def) = map else {
            panic!();
        };

        assert_eq!(def.def.map_type, BPF_MAP_TYPE_ARRAY as u32);
        assert_eq!(def.def.key_size, core::mem::size_of::<u32>() as u32);
        assert_eq!(def.def.map_flags, 0);
        assert_eq!(def.def.max_entries, 1);

        assert!(value_size.is_none());
        value_size = Some(def.def.value_size as usize);

        rel_maps.push((name.as_str(), map_id, map));
    }

    obj.relocate_maps(rel_maps.into_iter(), &text_sections)
        .unwrap();

    let function = obj
        .functions
        .get(&obj.programs["rc5"].function_key())
        .unwrap();

    let data = unsafe {
        core::slice::from_raw_parts(
            function.instructions.as_ptr() as *const u8,
            function.instructions.len() * core::mem::size_of::<bpf_insn>(),
        )
    };

    let mut vm = rbpf::EbpfVmMbuff::new(Some(data)).unwrap();

    let mut context = TestContext {
        sample: [0u8; 4],
        map: [0u64; 1024],
        codes: Vec::new(),
    };

    unsafe {
        let mut map = TEST_CONTEXTS.lock().unwrap();

        if map.is_none() {
            *map.deref_mut() = Some(HashMap::default());
        }

        map.as_mut().unwrap().insert(map_id, &mut context);
    }

    vm.register_helper(1, bpf_map_lookup_elem).unwrap();
    vm.register_helper(77, bpf_rc_repeat).unwrap();
    vm.register_helper(78, bpf_rc_keydown).unwrap();

    for (i, raw) in message.raw.iter().enumerate() {
        context.sample = raw.to_le_bytes();
        if i.is_even() {
            context.sample[3] |= 1;
        }

        let vars = vars
            .iter()
            .enumerate()
            .map(|(i, name)| format!("{name}={}", context.map[i]))
            .join(",");

        let mbuff = unsafe {
            std::slice::from_raw_parts(
                context.map.as_ptr().cast(),
                vars.len() * std::mem::size_of::<u64>(),
            )
        };

        println!("executing {raw} {vars}");

        let ret = vm.execute_program(mbuff, &context.sample).unwrap();
        assert_eq!(ret, 0);
    }

    assert_eq!(context.codes, vec![102]);

    unsafe {
        TEST_CONTEXTS
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .remove(&map_id);
    }
}

#[repr(C)]
struct TestContext {
    sample: [u8; 4],
    map: [u64; 1024],
    codes: Vec<u64>,
}

static mut TEST_CONTEXTS: Mutex<Option<HashMap<i32, *mut TestContext>>> = Mutex::new(None);
static mut TESTS_NEXT_MAP_ID: AtomicI32 = AtomicI32::new(1);

fn bpf_map_lookup_elem(def: u64, key: u64, _arg3: u64, _arg4: u64, _arg5: u64) -> u64 {
    unsafe {
        let def = def as i32;
        let e = *TEST_CONTEXTS
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .get(&def)
            .unwrap();

        let ptr = key as *const u32;
        assert_eq!(*ptr, 0);

        (*e).map.as_ptr() as u64
    }
}

fn bpf_rc_keydown(ctx: u64, protocol: u64, code: u64, flags: u64, _arg4: u64) -> u64 {
    let testmem = ctx as *mut TestContext;
    println!("rc_keydown protocol:{protocol} code:{code} flags:{flags}");

    unsafe {
        (*testmem).codes.push(code);
    }

    0
}

fn bpf_rc_repeat(ctx: u64, _arg2: u64, _arg3: u64, _arg4: u64, _arg5: u64) -> u64 {
    let testmem: *mut TestContext = ctx as *mut TestContext;

    println!("rc_repeat");

    unsafe {
        if let Some(last) = (*testmem).codes.last() {
            (*testmem).codes.push(*last);
        }
    }

    0
}
