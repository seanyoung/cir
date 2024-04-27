// We want to use the same names as the lirc source code
#![allow(non_camel_case_types)]
#![allow(unused)]

use std::{
    ffi::{c_char, CStr},
    marker::PhantomData,
    ptr, slice,
};

// keymap.h

#[repr(C)]
pub struct keymap {
    next: *const keymap,
    name: *const c_char,
    protocol: *const c_char,
    variant: *const c_char,
    param: *const protocol_param,
    scancode: *const scancode_entry,
    raw: *const raw_entry,
}

#[repr(C)]
pub struct protocol_param {
    next: *const protocol_param,
    name: *const c_char,
    value: u64,
}

#[repr(C)]
pub struct scancode_entry {
    next: *const scancode_entry,
    scancode: u64,
    keycode: *const c_char,
}

#[repr(C)]
pub struct raw_entry {
    next: *const raw_entry,
    scancode: u64,
    raw_length: u32,
    keycode: *const c_char,
    raw: [u32; 1],
}

extern "C" {
    pub fn parse_keymap(fname: *const c_char, keymap: *mut *mut keymap, verbose: bool) -> i32;
    pub fn free_keymap(keymap: *const keymap);
    pub fn keymap_param(keymap: *const keymap, name: *const c_char, fallback: i32) -> i32;
}

// ir-encode.h

#[derive(Clone, Copy, PartialEq, Debug)]
#[repr(u32)]
pub enum rc_proto {
    RC_PROTO_UNKNOWN = 0,
    RC_PROTO_OTHER = 1,
    RC_PROTO_RC5 = 2,
    RC_PROTO_RC5X_20 = 3,
    RC_PROTO_RC5_SZ = 4,
    RC_PROTO_JVC = 5,
    RC_PROTO_SONY12 = 6,
    RC_PROTO_SONY15 = 7,
    RC_PROTO_SONY20 = 8,
    RC_PROTO_NEC = 9,
    RC_PROTO_NECX = 10,
    RC_PROTO_NEC32 = 11,
    RC_PROTO_SANYO = 12,
    RC_PROTO_MCIR2_KBD = 13,
    RC_PROTO_MCIR2_MSE = 14,
    RC_PROTO_RC6_0 = 15,
    RC_PROTO_RC6_6A_20 = 16,
    RC_PROTO_RC6_6A_24 = 17,
    RC_PROTO_RC6_6A_32 = 18,
    RC_PROTO_RC6_MCE = 19,
    RC_PROTO_SHARP = 20,
    RC_PROTO_XMP = 21,
    RC_PROTO_CEC = 22,
    RC_PROTO_IMON = 23,
    RC_PROTO_RCMM12 = 24,
    RC_PROTO_RCMM24 = 25,
    RC_PROTO_RCMM32 = 26,
    RC_PROTO_XBOX_DVD = 27,
}

// These functions are defined in ir-encode.[ch], which comes from v4l-utils' ir-ctl
extern "C" {
    pub fn protocol_match(name: *const c_char, proto: rc_proto);
    pub fn protocol_carrier(proto: rc_proto) -> u32;
    pub fn protocol_max_size(proto: rc_proto) -> u32;
    pub fn protocol_scancode_valid(proto: rc_proto, scancode: *mut u32) -> bool;
    pub fn protocol_scancode_mask(proto: rc_proto) -> u32;
    pub fn protocol_encoder_available(proto: rc_proto) -> bool;
    pub fn protocol_encode(proto: rc_proto, scancode: u32, buf: *mut u32) -> u32;
    pub fn protocol_name(proto: rc_proto) -> *const c_char;
}

// bpf_encoder.h
extern "C" {
    pub fn encode_bpf_protocol(
        keymap: *const keymap,
        scancode: u32,
        buf: *mut u32,
        length: *mut u32,
    );
}
