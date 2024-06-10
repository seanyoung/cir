// We want to use the same names as the lirc source code
#![allow(non_camel_case_types)]

use libirctl::rc_proto;

#[allow(unused)]
extern "C" {
    fn ir_rc5_encode(proto: rc_proto, scancode: u32, event: *mut ir_raw_event, max: u32) -> u32;
    fn ir_rc6_encode(proto: rc_proto, scancode: u32, event: *mut ir_raw_event, max: u32) -> u32;
    fn ir_jvc_encode(proto: rc_proto, scancode: u32, event: *mut ir_raw_event, max: u32) -> u32;
    fn ir_sony_encode(proto: rc_proto, scancode: u32, event: *mut ir_raw_event, max: u32) -> u32;
    fn ir_nec_encode(proto: rc_proto, scancode: u32, event: *mut ir_raw_event, max: u32) -> u32;
    fn ir_sanyo_encode(proto: rc_proto, scancode: u32, event: *mut ir_raw_event, max: u32) -> u32;
    fn ir_sharp_encode(proto: rc_proto, scancode: u32, event: *mut ir_raw_event, max: u32) -> u32;
    fn ir_rcmm_encode(proto: rc_proto, scancode: u32, event: *mut ir_raw_event, max: u32) -> u32;
}

#[no_mangle]
extern "C" fn rc_repeat(_dev: *const u8) {
    // TODO
}

#[no_mangle]
extern "C" fn rc_keydown(_dev: *const u8, _protocol: u32, _scancode: u64, _toggle: u32) {
    // TODO
}

#[derive(Clone, Default)]
#[repr(C)]
struct ir_raw_event {
    duration: u32,
    duty_cycle: u8,

    pulse: bool,
    overflow: bool,
    timeout: bool,
    carrier_report: bool,
}

pub fn encode(proto: rc_proto, scancode: u32) -> Vec<u32> {
    let mut raw = vec![ir_raw_event::default(); 1024];

    match proto {
        rc_proto::RC_PROTO_RC5 | rc_proto::RC_PROTO_RC5X_20 | rc_proto::RC_PROTO_RC5_SZ => {
            let len = unsafe { ir_rc5_encode(proto, scancode, raw.as_mut_ptr(), raw.len() as u32) };

            raw.truncate(len as usize);
        }
        rc_proto::RC_PROTO_JVC => {
            let len = unsafe { ir_jvc_encode(proto, scancode, raw.as_mut_ptr(), raw.len() as u32) };

            raw.truncate(len as usize);
        }
        rc_proto::RC_PROTO_SONY12 | rc_proto::RC_PROTO_SONY15 | rc_proto::RC_PROTO_SONY20 => {
            let len =
                unsafe { ir_sony_encode(proto, scancode, raw.as_mut_ptr(), raw.len() as u32) };

            raw.truncate(len as usize);
        }
        rc_proto::RC_PROTO_NEC | rc_proto::RC_PROTO_NECX | rc_proto::RC_PROTO_NEC32 => {
            let len = unsafe { ir_nec_encode(proto, scancode, raw.as_mut_ptr(), raw.len() as u32) };

            raw.truncate(len as usize);
        }
        rc_proto::RC_PROTO_SHARP => {
            let len =
                unsafe { ir_sharp_encode(proto, scancode, raw.as_mut_ptr(), raw.len() as u32) };

            raw.truncate(len as usize);
        }
        rc_proto::RC_PROTO_SANYO => {
            let len =
                unsafe { ir_sanyo_encode(proto, scancode, raw.as_mut_ptr(), raw.len() as u32) };

            raw.truncate(len as usize);
        }
        rc_proto::RC_PROTO_RC6_0
        | rc_proto::RC_PROTO_RC6_6A_20
        | rc_proto::RC_PROTO_RC6_6A_24
        | rc_proto::RC_PROTO_RC6_6A_32
        | rc_proto::RC_PROTO_RC6_MCE => {
            let len = unsafe { ir_rc6_encode(proto, scancode, raw.as_mut_ptr(), raw.len() as u32) };

            raw.truncate(len as usize);
        }
        rc_proto::RC_PROTO_RCMM12 | rc_proto::RC_PROTO_RCMM24 | rc_proto::RC_PROTO_RCMM32 => {
            let len =
                unsafe { ir_rcmm_encode(proto, scancode, raw.as_mut_ptr(), raw.len() as u32) };

            raw.truncate(len as usize);
        }
        _ => panic!("proto {}", proto as u32),
    }

    raw.iter().map(|raw| raw.duration).collect()
}
