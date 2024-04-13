#![cfg(feature = "bpf")]
#![cfg(target_os = "linux")]

use aya_obj::{
    generated::{bpf_insn, bpf_map_type::BPF_MAP_TYPE_ARRAY},
    Map, Object,
};
use irp::{Irp, Options, Vartable};
use itertools::Itertools;
use num::Integer;
use std::collections::{HashMap, HashSet};

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

        rel_maps.push((name.as_str(), 7, map));
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

    vm.register_helper(1, bpf_map_lookup_elem).unwrap();
    vm.register_helper(77, bpf_rc_repeat).unwrap();
    vm.register_helper(78, bpf_rc_keydown).unwrap();

    for (i, raw) in message.raw.iter().enumerate() {
        let mut e = raw.to_le_bytes();
        if i.is_even() {
            e[3] = 1;
        }

        let map =
            unsafe { std::slice::from_raw_parts(MAP_MEMORY.as_ptr() as *const u64, vars.len()) };

        let vars = vars
            .iter()
            .enumerate()
            .map(|(i, name)| format!("{name}={}", map[i]))
            .join(",");

        println!("executing {e:?} {raw} {vars}");

        let mbuff = unsafe { &MAP_MEMORY[0..value_size.unwrap()] };

        let _return = vm.execute_program(mbuff, &e).unwrap();
        assert_eq!(_return, 0);
    }

    unsafe {
        assert_eq!(CODES, vec![102]);
    }
}

static mut MAP_MEMORY: [u8; 16384] = [0u8; 16384];
static mut CODES: Vec<u64> = Vec::new();

fn bpf_map_lookup_elem(def: u64, key: u64, _arg3: u64, _arg4: u64, _arg5: u64) -> u64 {
    assert_eq!(def, 7);

    unsafe {
        let ptr = key as *const u32;
        assert_eq!(*ptr, 0);
    }

    let p = unsafe { MAP_MEMORY.as_ptr() };

    p as u64
}

fn bpf_rc_keydown(_ctx: u64, protocol: u64, code: u64, flags: u64, _arg4: u64) -> u64 {
    println!("rc_keydown protocol:{protocol} code:{code} flags:{flags}");

    unsafe {
        CODES.push(code);
    }

    0
}

fn bpf_rc_repeat(_ctx: u64, _arg2: u64, _arg3: u64, _arg4: u64, _arg5: u64) -> u64 {
    println!("rc_repeat");

    0
}
