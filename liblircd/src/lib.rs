use std::{
    ffi::{c_char, CStr},
    marker::PhantomData,
    slice,
};

#[allow(unused)]
extern "C" {
    fn parse_config(data: *const u8, length: usize) -> *mut ir_remote;
    fn free_config(remote: *mut ir_remote);
    fn send_buffer_init();
    fn remote_is_raw(remote: *const ir_remote) -> i32;
    fn send_buffer_put(remote: *const ir_remote, code: *const ir_ncode) -> i32;
    fn init_sim(remote: *const ir_remote, code: *const ir_ncode, repeat_preset: i32) -> i32;
    fn send_buffer_length() -> i32;
    fn send_buffer_data() -> *const u32;
    fn send_buffer_sum() -> i32;
    pub fn lirc_log_set_stdout();
}

#[repr(C)]
struct ir_remote {
    name: *const c_char,
    driver: *const c_char,
    codes: *const ir_ncode,
    bits: i32,
    flags: i32,
    eps: i32,
    aeps: u32,
    dyncodes_name: *const c_char,
    dyncode: i32,
    dyncodes: [ir_ncode; 2],

    phead: i32,
    shead: i32,
    pthree: i32,
    sthree: i32,
    ptwo: i32,
    stwo: i32,
    pone: i32,
    sone: i32,
    pzero: i32,
    szero: i32,
    plead: i32,
    ptrail: i32,
    pfoot: i32,
    sfoot: i32,
    prepeat: i32,
    srepeat: i32,

    pre_data_bits: i32,
    pre_data: u64,
    post_data_bits: i32,
    post_data: u64,
    pre_p: i32,
    pre_s: i32,
    post_p: i32,
    post_s: i32,

    gap: u32,
    gap2: u32,
    repeat_gap: u32,
    toggle_bit: i32,
    toggle_bit_mask: u64,
    suppress_repeat: i32,
    min_repeat: i32,
    min_code_repeat: u32,
    freq: u32,
    duty_cycle: u32,
    toggle_mask: u64,
    rc6_mask: u64,

    baud: u32,
    bits_in_byte: u32,
    parity: u32,
    stop_bits: u32,

    ignore_mask: u64,
    repeat_mask: u64,

    toggle_bit_mask_state: u64,
    toggle_mask_state: i32,
    repeat_count: i32,
    last_code: *const ir_ncode,
    toggle_code: *const ir_ncode,
    reps: i32,
    last_send: libc::timeval,
    min_remaining_gap: i32,
    max_remaining_gap: i32,

    min_total_signal_length: i32,
    max_total_signal_length: i32,
    min_gap_length: i32,
    max_gap_length: i32,
    min_pulse_length: i32,
    max_pulse_length: i32,
    min_space_length: i32,
    max_space_length: i32,
    release_detected: i32,
    manual_sort: i32,
    next: *mut ir_remote,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct ir_ncode {
    name: *const c_char,
    code: u64,
    length: i32,
    signals: *const i32,
    next: *const ir_code_node,
    current: *const ir_code_node,
    transmit_state: *const ir_code_node,
    next_ncode: *const ir_ncode,
}

#[repr(C)]
struct ir_code_node {
    code: u64,
    next: *const ir_code_node,
}

#[derive(Debug)]
pub struct LircdConf(*mut ir_remote);

impl LircdConf {
    pub fn parse(source: &str) -> Option<Self> {
        let remote = unsafe { parse_config(source.as_ptr(), source.len()) };

        if remote.is_null() {
            None
        } else {
            Some(LircdConf(remote))
        }
    }

    pub fn iter(&self) -> RemoteIterator {
        RemoteIterator(self.0, true, PhantomData)
    }
}

pub struct RemoteIterator<'a>(*mut ir_remote, bool, PhantomData<&'a ()>);

impl<'a> Iterator for RemoteIterator<'a> {
    type Item = Remote<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.1 {
            self.1 = false;
            Some(Remote(self.0, PhantomData))
        } else {
            let remote = unsafe { (*self.0).next };

            if remote.is_null() {
                None
            } else {
                self.0 = remote;

                Some(Remote(remote, PhantomData))
            }
        }
    }
}

impl Drop for LircdConf {
    fn drop(&mut self) {
        unsafe { free_config(self.0) };
    }
}

#[derive(Debug)]
pub struct Remote<'a>(*mut ir_remote, PhantomData<&'a ()>);

impl<'a> Remote<'a> {
    pub fn name(&self) -> String {
        let name = unsafe { CStr::from_ptr((*self.0).name) };

        String::from_utf8_lossy(name.to_bytes()).to_string()
    }

    pub fn codes_iter(&self) -> CodeIterator {
        CodeIterator(unsafe { (*self.0).codes }, true, self, PhantomData)
    }

    pub fn is_raw(&self) -> bool {
        unsafe { remote_is_raw(self.0) != 0 }
    }

    pub fn is_serial(&self) -> bool {
        unsafe { ((*self.0).flags & 0x0200) != 0 }
    }

    pub fn toggle_bit_mask(&self) -> u64 {
        unsafe { (*self.0).toggle_bit_mask }
    }

    pub fn toggle_bit(&self) -> i32 {
        unsafe { (*self.0).toggle_bit }
    }

    pub fn bit(&self, bit: usize) -> (i32, i32) {
        unsafe {
            match bit {
                0 => ((*self.0).pzero, (*self.0).szero),
                1 => ((*self.0).pone, (*self.0).sone),
                2 => ((*self.0).ptwo, (*self.0).stwo),
                3 => ((*self.0).pthree, (*self.0).sthree),
                _ => unreachable!(),
            }
        }
    }
}

pub struct CodeIterator<'a>(*const ir_ncode, bool, &'a Remote<'a>, PhantomData<&'a ()>);

impl<'a> Iterator for CodeIterator<'a> {
    type Item = Code<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let code = if self.1 {
            self.1 = false;
            self.0
        } else {
            unsafe { self.0.offset(1) }
        };

        self.0 = code;

        if unsafe { (*code).name.is_null() } {
            None
        } else {
            Some(Code(code, self.2))
        }
    }
}

pub struct Code<'a>(*const ir_ncode, &'a Remote<'a>);

impl<'a> Code<'a> {
    pub fn name(&self) -> String {
        let name = unsafe { CStr::from_ptr((*self.0).name) };

        String::from_utf8_lossy(name.to_bytes()).to_string()
    }

    pub fn code(&self) -> u64 {
        unsafe { (*self.0).code }
    }

    pub fn encode(&self) -> Option<Vec<u32>> {
        unsafe { send_buffer_init() };

        if unsafe { (*self.1 .0).toggle_mask } != 0 {
            unsafe {
                (*self.1 .0).toggle_mask_state = 0;
            }
        }

        let res = unsafe { send_buffer_put(self.1 .0, self.0) };
        if res != 1 {
            return None;
        }

        let raw =
            unsafe { slice::from_raw_parts(send_buffer_data(), send_buffer_length() as usize) };

        Some(raw.to_vec())
    }

    pub fn next(&self) -> Vec<u64> {
        let mut res = Vec::new();

        let mut next = unsafe { (*self.0).next };

        while next.is_null() {
            res.push(unsafe { (*next).code });

            next = unsafe { (*next).next };
        }

        res
    }
}
