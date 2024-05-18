use cir::keymap::{Keymap, LinuxProtocol};
use irp::{Irp, Vartable};
use libirctl::{
    encode_bpf_protocol, free_keymap, keymap, parse_keymap, protocol_encode,
    protocol_encoder_available, protocol_match,
};
use rand::RngCore;
use std::{ffi::CStr, ffi::CString, fs::read_dir, path::Path};

fn irctl_compare_encode(path: &Path, scancode: u32, our_keymap: &Keymap) {
    // first encode using old ir-ctl
    let mut keymap: *mut keymap = std::ptr::null_mut();
    let mut buf = vec![0u32; 1024];

    let cs = CString::new(path.to_str().unwrap()).unwrap();

    unsafe {
        assert_eq!(parse_keymap(cs.as_ptr(), &mut keymap, false), 0);
    };

    let mut length = 0;

    if ["pulse_distance", "pulse_length", "manchester"].contains(&our_keymap.protocol.as_str()) {
        unsafe { encode_bpf_protocol(keymap, scancode, buf.as_mut_ptr(), &mut length) };
    } else {
        let protocol = unsafe {
            if (*keymap).variant.is_null() {
                (*keymap).variant
            } else {
                (*keymap).protocol
            }
        };

        let mut proto = libirctl::rc_proto::RC_PROTO_CEC;

        assert!(unsafe { protocol_match(protocol, &mut proto) });

        if !unsafe { protocol_encoder_available(proto) } {
            return;
        }

        length = unsafe { protocol_encode(proto, scancode, buf.as_mut_ptr()) };
    }

    buf.truncate(length as usize);

    unsafe { free_keymap(keymap) };

    let mut message = our_keymap.encode_scancode(scancode.into(), 0).unwrap();

    message.remove_trailing_gap();

    assert_eq!(message.raw, buf);
}

#[test]
fn keymap_encode() {
    recurse(Path::new("../testdata/rc_keymaps"));
}

fn recurse(path: &Path) {
    for entry in read_dir(path).unwrap() {
        let e = entry.unwrap();
        let path = e.path();
        if e.metadata().unwrap().file_type().is_dir() {
            recurse(&path);
        } else if path.to_string_lossy().ends_with(".toml") {
            for keymap in Keymap::parse_file(&path).unwrap() {
                if ["pulse_distance", "pulse_length", "manchester"]
                    .contains(&keymap.protocol.as_str())
                {
                    for scancode in keymap.scancodes.keys() {
                        println!("{} {:#x}", path.display(), *scancode);
                        irctl_compare_encode(&path, *scancode as u32, &keymap);
                    }
                }
            }
        }
    }
}

#[test]
fn scancode_encode() {
    for proto in [
        libirctl::rc_proto::RC_PROTO_RC5,
        libirctl::rc_proto::RC_PROTO_RC5X_20,
        libirctl::rc_proto::RC_PROTO_RC5_SZ,
        libirctl::rc_proto::RC_PROTO_JVC,
        libirctl::rc_proto::RC_PROTO_SONY12,
        libirctl::rc_proto::RC_PROTO_SONY15,
        libirctl::rc_proto::RC_PROTO_SONY20,
        libirctl::rc_proto::RC_PROTO_NEC,
        libirctl::rc_proto::RC_PROTO_NECX,
        libirctl::rc_proto::RC_PROTO_NEC32,
        libirctl::rc_proto::RC_PROTO_SANYO,
        libirctl::rc_proto::RC_PROTO_RC6_0,
        libirctl::rc_proto::RC_PROTO_RC6_6A_20,
        libirctl::rc_proto::RC_PROTO_RC6_6A_24,
        libirctl::rc_proto::RC_PROTO_RC6_6A_32,
        libirctl::rc_proto::RC_PROTO_RC6_MCE,
        libirctl::rc_proto::RC_PROTO_SHARP,
        libirctl::rc_proto::RC_PROTO_RCMM12,
        libirctl::rc_proto::RC_PROTO_RCMM24,
        libirctl::rc_proto::RC_PROTO_RCMM32,
        libirctl::rc_proto::RC_PROTO_XBOX_DVD,
    ] {
        let name = unsafe { CStr::from_ptr(libirctl::protocol_name(proto)) }
            .to_str()
            .unwrap();
        let linux = LinuxProtocol::find(name).unwrap();

        assert_eq!(linux.scancode_mask, unsafe {
            libirctl::protocol_scancode_mask(proto)
        });
        assert_eq!(linux.protocol_no, proto as u32);
        let mut rng = rand::thread_rng();

        if unsafe { libirctl::protocol_encoder_available(proto) } {
            let irp = Irp::parse(linux.irp.unwrap()).unwrap();

            if proto == libirctl::rc_proto::RC_PROTO_NEC
                || proto == libirctl::rc_proto::RC_PROTO_NECX
                || proto == libirctl::rc_proto::RC_PROTO_NEC32
            {
                assert_eq!(irp.carrier(), 38400);
            } else if proto == libirctl::rc_proto::RC_PROTO_JVC {
                assert_eq!(irp.carrier(), 37900);
            } else {
                assert_eq!(irp.carrier(), unsafe {
                    libirctl::protocol_carrier(proto) as i64
                });
            }

            let max_size = unsafe { libirctl::protocol_max_size(proto) } as usize;

            let mut irctl = vec![0u32; max_size];

            for _ in 0..1000 {
                let scancode = rng.next_u32() & linux.scancode_mask;

                irctl.resize(max_size as usize, 0);

                let len = unsafe { libirctl::protocol_encode(proto, scancode, irctl.as_mut_ptr()) };

                assert!(
                    len as usize <= max_size,
                    "{len} {max_size} proto:{proto:?} scancode:{scancode:#x}"
                );

                irctl.resize(len as usize, 0);

                let mut vars = Vartable::new();

                vars.set("CODE".into(), scancode as i64);

                let mut our = irp.encode_raw(vars, 0).unwrap();
                our.remove_trailing_gap();

                if [
                    libirctl::rc_proto::RC_PROTO_JVC,
                    libirctl::rc_proto::RC_PROTO_NEC,
                    libirctl::rc_proto::RC_PROTO_NECX,
                    libirctl::rc_proto::RC_PROTO_NEC32,
                    libirctl::rc_proto::RC_PROTO_SANYO,
                    libirctl::rc_proto::RC_PROTO_SHARP,
                    libirctl::rc_proto::RC_PROTO_RC6_0,
                    libirctl::rc_proto::RC_PROTO_RC6_6A_20,
                    libirctl::rc_proto::RC_PROTO_RC6_6A_24,
                    libirctl::rc_proto::RC_PROTO_RC6_6A_32,
                    libirctl::rc_proto::RC_PROTO_RC6_MCE,
                ]
                .contains(&proto)
                {
                    assert!(compare_with_rounding(&irctl, &our.raw));
                } else {
                    assert_eq!(irctl, our.raw);
                }
            }
        } else if let Some(irp) = linux.irp {
            let irp = Irp::parse(irp).unwrap();

            for _ in 0..1000 {
                let scancode = rng.next_u32() & linux.scancode_mask;

                let mut vars = Vartable::new();

                vars.set("CODE".into(), scancode as i64);

                let our = irp.encode_raw(vars, 0).unwrap();

                assert!(!our.raw.is_empty());
            }
        }
    }
}

fn compare_with_rounding(l: &[u32], r: &[u32]) -> bool {
    if l == r {
        return true;
    }

    if l.len() != r.len() {
        println!(
            "comparing:\n{:?} with\n{:?}\n have different lengths {} and {}",
            l,
            r,
            l.len(),
            r.len()
        );

        return false;
    }

    for i in 0..l.len() {
        // sharp gap in the middle
        if l[i] == 40000 && r[i] == 43560 {
            continue;
        }

        let diff = l[i].abs_diff(r[i]);

        // is the difference more than 168
        if diff > 168 {
            println!(
                "comparing:\nleft:{:?} with\nright:{:?}\nfailed at position {} out of {} diff {diff}",
                l,
                r,
                i,
                l.len()
            );

            return false;
        }
    }

    true
}
