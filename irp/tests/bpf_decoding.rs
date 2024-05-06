#![cfg(feature = "bpf")]

use aya_obj::{generated::bpf_map_type::BPF_MAP_TYPE_ARRAY, Map, Object};
use irp::{Irp, Message, Options, Protocol, Vartable, DFA};
use itertools::Itertools;
use num::Integer;
use rand::Rng;
use std::{
    collections::{HashMap, HashSet},
    ops::DerefMut,
    path::PathBuf,
    sync::{atomic::AtomicI32, Mutex},
};

/// This is the BPF version of tests::decode_all()
#[test]
fn decode_all() {
    let mut protocols = Protocol::parse(&PathBuf::from(
        "tests/IrpTransmogrifier/src/main/resources/IrpProtocols.xml",
    ))
    .unwrap();

    let mut total_tests = 0;
    let mut fails = 0;
    let mut failing_protocols: HashSet<&str> = HashSet::new();
    let mut rng = rand::thread_rng();

    for protocol in &mut protocols {
        // TODO: See https://github.com/qmonnet/rbpf/pull/108
        if protocol.name.starts_with("XMP") {
            continue;
        }

        println!("trying {}: {}", protocol.name, protocol.irp);

        if protocol.name == "NEC-Shirriff" {
            protocol.irp = "{38.4k,msb,564}<1,-1|1,-3>(16,-8,data:length,1,^108m) [data:0..UINT32_MAX,length:1..63]".into();
        }

        let irp = Irp::parse(&protocol.irp).unwrap();

        let nfa = match irp.build_nfa() {
            Ok(nfa) => nfa,
            Err(s) => {
                println!("compile {} failed {}", protocol.irp, s);
                fails += 1;
                continue;
            }
        };

        let max_gap = if protocol.name == "Epson" {
            100000
        } else if protocol.name == "NRC17" {
            110500
        } else {
            20000
        };

        let options = Options {
            name: &protocol.name,
            aeps: 10,
            eps: 3,
            max_gap,
            ..Default::default()
        };

        let dfa = nfa.build_dfa(&options);

        let first = if irp.has_ending() { 1 } else { 0 };

        for n in first..10 {
            let repeats = if n < 3 { n } else { rng.gen_range(n..n + 20) };

            let mut vars = Vartable::new();
            let mut params = HashMap::new();

            for param in &irp.parameters {
                let value = rng.gen_range(param.min..=param.max);

                params.insert(param.name.to_owned(), value);
                vars.set(param.name.to_owned(), value);
            }

            let msg = irp.encode_raw(vars, repeats).unwrap();

            if msg.raw.len() < 3 {
                println!("protocol:{} repeats:{} too short", protocol.name, repeats);
                continue;
            }

            total_tests += 1;

            let mut decodes = bpf_decode(&dfa, &options, &protocol.name, &msg);

            // if nothing decoded, we fail to decode
            let mut ok = !decodes.is_empty();

            while let Some(code) = decodes.pop() {
                let mut received = code;
                println!("received:{received}");
                for param in irp.parameters.iter().rev() {
                    if param.name == "T" {
                        continue;
                    }

                    let proto_mask = match (protocol.name.as_str(), param.name.as_str()) {
                        ("Zenith5", "F") => 31,
                        ("Zenith6", "F") => 63,
                        ("Zenith7", "F") => 127,
                        ("Zenith", "F") => (1 << (code & 15)) - 1,
                        ("NEC-Shirriff", "data") => (1u64 << (code & 63)) - 1u64,
                        ("Fujitsu_Aircon_old", "tOn") => !0xf0,
                        _ => !0,
                    };

                    let expected = params[&param.name] as u64;
                    let bits = param.max.ilog2() + 1;
                    let mask = gen_mask(bits);

                    if ((expected & proto_mask) & mask) != (received & mask) {
                        println!(
                            "{} does not match, expected {expected} got {} protomask {proto_mask}",
                            param.name,
                            received & mask
                        );
                        ok = false;
                    } else {
                        println!("{}:{bits} matches: {}", param.name, expected);
                    }

                    received >>= bits;
                }
            }

            if !ok {
                println!(
                    "{} failed to decode, irp: {} ir: {}",
                    protocol.name,
                    protocol.irp,
                    msg.print_rawir()
                );

                println!(
                    "expected: {}",
                    irp.parameters
                        .iter()
                        .map(|param| format!("{}={}", param.name, params[&param.name]))
                        .join(",")
                );

                fails += 1;

                failing_protocols.insert(protocol.name.as_str());
            }
            println!("FAILS:{fails}/{total_tests}");
        }
    }

    println!("tests: {total_tests} fails: {fails}");

    println!(
        "failing protocol: {} {failing_protocols:?}",
        failing_protocols.len()
    );

    assert_eq!(failing_protocols.len(), 38);
}

fn bpf_decode(dfa: &DFA, options: &Options, name: &str, message: &Message) -> Vec<u64> {
    let (object, vars) = dfa.compile_bpf(options).unwrap();

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
        .get(&obj.programs[name].function_key())
        .unwrap();

    let data = unsafe {
        core::slice::from_raw_parts(
            function.instructions.as_ptr().cast(),
            std::mem::size_of_val(&*function.instructions),
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

        println!(
            "executing {}{raw} {vars}",
            if i.is_even() { '-' } else { '+' }
        );

        let ret = vm.execute_program(mbuff, &context.sample).unwrap();
        assert_eq!(ret, 0);
    }

    unsafe {
        TEST_CONTEXTS
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .remove(&map_id);
    }

    context.codes
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

fn gen_mask(v: u32) -> u64 {
    if v < 64 {
        (1u64 << v) - 1
    } else {
        u64::MAX
    }
}
