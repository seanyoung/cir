use cir::keymap::LinuxProtocol;
use irp::{Irp, Vartable};
use libirctl::rc_proto;
use rand::RngCore;
use std::ffi::CStr;

#[test]
fn kernel_scancode_encode() {
    for proto in [
        rc_proto::RC_PROTO_RC5,
        rc_proto::RC_PROTO_RC5X_20,
        rc_proto::RC_PROTO_RC5_SZ,
        rc_proto::RC_PROTO_JVC,
        rc_proto::RC_PROTO_SONY12,
        rc_proto::RC_PROTO_SONY15,
        rc_proto::RC_PROTO_SONY20,
        rc_proto::RC_PROTO_NEC,
        rc_proto::RC_PROTO_NECX,
        rc_proto::RC_PROTO_NEC32,
        rc_proto::RC_PROTO_SANYO,
        rc_proto::RC_PROTO_RC6_0,
        rc_proto::RC_PROTO_RC6_6A_20,
        rc_proto::RC_PROTO_RC6_6A_24,
        rc_proto::RC_PROTO_RC6_6A_32,
        rc_proto::RC_PROTO_RC6_MCE,
        rc_proto::RC_PROTO_SHARP,
        rc_proto::RC_PROTO_RCMM12,
        rc_proto::RC_PROTO_RCMM24,
        rc_proto::RC_PROTO_RCMM32,
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

        let irp = Irp::parse(linux.irp.unwrap()).unwrap();

        for _ in 0..1000 {
            let scancode = rng.next_u32() & linux.scancode_mask;

            let mut kencoded = libkcodec::encode(proto, scancode);
            // TODO: we're not comparing the trailing gap
            kencoded.pop();

            let mut vars = Vartable::new();

            vars.set("CODE".into(), scancode as i64);

            let mut our = irp.encode_raw(vars, 0).unwrap();
            our.remove_trailing_gap();

            if [
                rc_proto::RC_PROTO_JVC,
                rc_proto::RC_PROTO_NEC,
                rc_proto::RC_PROTO_NECX,
                rc_proto::RC_PROTO_NEC32,
                rc_proto::RC_PROTO_SANYO,
                rc_proto::RC_PROTO_SHARP,
                rc_proto::RC_PROTO_RC6_0,
                rc_proto::RC_PROTO_RC6_6A_20,
                rc_proto::RC_PROTO_RC6_6A_24,
                rc_proto::RC_PROTO_RC6_6A_32,
                rc_proto::RC_PROTO_RC6_MCE,
                rc_proto::RC_PROTO_RCMM12,
                rc_proto::RC_PROTO_RCMM24,
                rc_proto::RC_PROTO_RCMM32,
            ]
            .contains(&proto)
            {
                assert!(compare_with_rounding(&kencoded, &our.raw));
            } else {
                assert_eq!(kencoded, our.raw);
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
