use cir::keymap::Keymap;
use libirctl::{encode_bpf_protocol, free_keymap, keymap, parse_keymap};
use std::{ffi::CString, fs::File, io::Read, path::PathBuf};

fn irctl_compare_encode(path: &str, scancode: u32) {
    // first encode using old ir-ctl
    let mut keymap: *mut keymap = std::ptr::null_mut();
    let mut buf = vec![0u32; 1024];

    let cs = CString::new(path).unwrap();

    unsafe {
        assert_eq!(parse_keymap(cs.as_ptr(), &mut keymap, false), 0);
    };

    let mut length = 0;

    unsafe { encode_bpf_protocol(keymap, scancode, buf.as_mut_ptr(), &mut length) };

    buf.truncate(length as usize);

    unsafe { free_keymap(keymap) };

    let path = PathBuf::from(path);

    let mut f = File::open(&path).unwrap();

    let mut contents = String::new();

    f.read_to_string(&mut contents).unwrap();

    let keymap = Keymap::parse_text(&contents, &path).unwrap();

    let mut message = keymap[0].encode_scancode(scancode.into(), 0).unwrap();

    message.remove_trailing_gap();

    assert_eq!(message.raw, buf);
}

#[test]
fn encode() {
    irctl_compare_encode("testdata/rc_keymaps/dish_network.toml", 0x8c00);
    irctl_compare_encode("testdata/rc_keymaps/TelePilot_100C.toml", 0x7e);
    irctl_compare_encode("testdata/rc_keymaps/RM-687C.toml", 0x3f0);
}
