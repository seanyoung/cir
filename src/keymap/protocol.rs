use super::LinuxProtocol;

impl LinuxProtocol {
    pub fn find(name: &str) -> Option<&'static LinuxProtocol> {
        LINUX_PROTOCOLS.iter().find(|e| e.name == name)
    }

    /// Match protocol name with regard for spaces or dashes or underscores.
    /// Behaviour should match protocol_match() in ir-ctl
    pub fn find_like(name: &str) -> Option<&'static LinuxProtocol> {
        let str_like = |name: &str| -> String {
            name.chars()
                .filter_map(|ch| {
                    if matches!(ch, ' ' | '-' | '_') || !ch.is_ascii() {
                        None
                    } else {
                        Some(ch.to_ascii_lowercase())
                    }
                })
                .collect::<String>()
        };

        let name = str_like(name);

        LINUX_PROTOCOLS.iter().find(|e| str_like(e.name) == name)
    }
}

const LINUX_PROTOCOLS: &[LinuxProtocol] = &[
    LinuxProtocol {
        name: "rc5",
        decoder: "rc5",
        irp: Some(
            "{36k,msb,889}<1,-1|-1,1>(1,~CODE:1:6,T:1,CODE:5:8,CODE:6,^114m) [CODE:0..0x1FFF,T:0..1=0]",
        ),
        scancode_mask: 0x1f7f,
        protocol_no: 2,
    },
    LinuxProtocol {
        name: "rc5x_20",
        decoder: "rc5",
        irp: Some("{36k,msb,889}<1,-1|-1,1>(1,~CODE:1:14,T:1,CODE:5:16,-4,CODE:6:8,CODE:6,^114m) [CODE:0..0x1fffff,T:0..1=0]"),
        scancode_mask: 0x1f7f3f,
        protocol_no: 3,
    },
    LinuxProtocol {
        name: "rc5_sz",
        decoder: "rc5",
        irp: Some("{36k,msb,889}<1,-1|-1,1>(1,CODE:1:13,T:1,CODE:12,^114m) [CODE:0..0x2fff,T:0..1=0]"),
        scancode_mask: 0x2fff,
        protocol_no: 4,
    },
    LinuxProtocol {
        name: "jvc",
        decoder: "jvc",
        irp: Some("{37.9k,527,33%}<1,-1|1,-3>(16,-8,CODE:8:8,CODE:8,1,^59.08m,(CODE:8:8,CODE:8,1,^46.42m)*) [CODE:0..0xffff]"),
        scancode_mask: 0xffff,
        protocol_no: 5,
    },
    LinuxProtocol {
        name: "sony12",
        decoder: "sony",
        irp: Some("{40k,600}<1,-1|2,-1>(4,-1,CODE:7,CODE:5:16,^45m) [CODE:0..0x1fffff]"),
        scancode_mask: 0x1f007f,
        protocol_no: 6,
    },
    LinuxProtocol {
        name: "sony15",
        decoder: "sony",
        irp: Some("{40k,600}<1,-1|2,-1>(4,-1,CODE:7,CODE:8:16,^45m) [CODE:0..0xffffff]"),
        scancode_mask: 0xff007f,
        protocol_no: 7,
    },
    LinuxProtocol {
        name: "sony20",
        decoder: "sony",
        irp: Some("{40k,600}<1,-1|2,-1>(4,-1,CODE:7,CODE:5:16,CODE:8:8,^45m) [CODE:0..0x1fffff]"),
        scancode_mask: 0x1fff7f,
        protocol_no: 8,
    },
    LinuxProtocol {
        name: "nec",
        decoder: "nec",
        irp: Some("{38.4k,564}<1,-1|1,-3>(16,-8,CODE:8:8,~CODE:8:8,CODE:8,~CODE:8,1,^108m,(16,-4,1,^108m)*) [CODE:0..0xffff]"),
        scancode_mask: 0xffff,
        protocol_no: 9,
    },
    LinuxProtocol {
        name: "necx",
        decoder: "nec",
        irp: Some("{38.4k,564}<1,-1|1,-3>(16,-8,CODE:8:16,CODE:8:8,CODE:8,~CODE:8,1,^108m,(16,-4,1,^108m)*) [CODE:0..0xffffff]"),
        scancode_mask: 0xffffff,
        protocol_no: 10,
    },
    LinuxProtocol {
        name: "nec32",
        decoder: "nec",
        irp: Some("{38.4k,564}<1,-1|1,-3>(16,-8,CODE:8:16,CODE:8:24,CODE:8,CODE:8:8,1,^108m,(16,-4,1,^108m)*) [CODE:0..0xffffffff]"),
        scancode_mask: 0xffff_ffff,
        protocol_no: 11,
    },
    LinuxProtocol {
        name: "sanyo",
        decoder: "sanyo",
        irp: Some("{38k,562.5}<1,-1|1,-3>(16,-8,CODE:13:8,~CODE:13:8,CODE:8,~CODE:8,1,-42,(16,-8,1,-150)*) [CODE:0..0x1fffff]"),
        scancode_mask: 0x1fffff,
        protocol_no: 12,
    },
    LinuxProtocol {
        name: "mcir2-kbd",
        decoder: "mce_kbd",
        irp: None,
        scancode_mask: u32::MAX,
        protocol_no: 13,
    },
    LinuxProtocol {
        name: "mcir2-mse",
        decoder: "mce_kbd",
        irp: None,
        scancode_mask: u32::MAX,
        protocol_no: 14,
    },
    LinuxProtocol {
        name: "rc6_0",
        decoder: "rc6",
        irp: Some("{36k,444,msb}<-1,1|1,-1>(6,-2,1:1,0:3,<-2,2|2,-2>(T:1),CODE:16,^107m) [CODE:0..0xffff,T@:0..1=0]"),
        scancode_mask: 0xffff,
        protocol_no: 15,
    },
    LinuxProtocol {
        name: "rc6_6a_20",
        decoder: "rc6",
        irp: Some("{36k,444,msb}<-1,1|1,-1>(6,-2,1:1,6:3,<-2,2|2,-2>(T:1),CODE:20,-100m) [CODE:0..0xfffff,T@:0..1=0]"),
        scancode_mask: 0xf_ffff,
        protocol_no: 16,
    },
    LinuxProtocol {
        name: "rc6_6a_24",
        decoder: "rc6",
        irp: Some("{36k,444,msb}<-1,1|1,-1>(6,-2,1:1,6:3,<-2,2|2,-2>(T:1),CODE:24,^105m) [CODE:0..0xffffff,T@:0..1=0]"),
        scancode_mask: 0xff_ff_ff,
        protocol_no: 17,
    },
    LinuxProtocol {
        name: "rc6_6a_32",
        decoder: "rc6",
        irp: Some("{36k,444,msb}<-1,1|1,-1>(6,-2,1:1,6:3,<-2,2|2,-2>(T:1),CODE:32,MCE=(CODE>>16)==0x800f||(CODE>>16)==0x8034||(CODE>>16)==0x8046,^105m){MCE=0}[CODE:0..0xffffffff,T@:0..1=0]"),
        scancode_mask: 0xffff_ffff,
        protocol_no: 18,
    },
    LinuxProtocol {
        name: "rc6_mce",
        decoder: "rc6",
        irp: Some("{36k,444,msb}<-1,1|1,-1>(6,-2,1:1,6:3,-2,2,CODE:16:16,T:1,CODE:15,MCE=(CODE>>16)==0x800f||(CODE>>16)==0x8034||(CODE>>16)==0x8046,^105m){MCE=1}[CODE:0..0xffffffff,T@:0..1=0]"),
        scancode_mask: 0xffff_7fff,
        protocol_no: 19,
    },
    LinuxProtocol {
        name: "sharp",
        decoder: "sharp",
        irp: Some("{38k,264}<1,-3|1,-7>(CODE:5:8,CODE:8,1:2,1,-165,CODE:5:8,~CODE:8,2:2,1,-165) [CODE:0..0x1fff]"),
        scancode_mask: 0x1fff,
        protocol_no: 20,
    },
    LinuxProtocol {
        name: "xmp",
        decoder: "xmp",
        // TODO: irp
        irp: None,
        scancode_mask: u32::MAX,
        protocol_no: 21,
    },
    LinuxProtocol {
        name: "cec",
        decoder: "cec",
        irp: None,
        scancode_mask: u32::MAX,
        protocol_no: 22,
    },
    LinuxProtocol {
        name: "imon",
        decoder: "imon",
        // TODO: {416,38k,msb}<-1|1>(1,<P:1,1:1,(CHK=CHK>>1,P=CHK&1)|0:2,(CHK=CHK>>1,P=1)>(CODE:31),^106m){P=1,CHK=0x7efec2} [CODE:0..0x7fffffff],
        irp: None,
        scancode_mask: u32::MAX,
        protocol_no: 23,
    },
    LinuxProtocol {
        name: "rc-mm-12",
        decoder: "rc-mm",
        irp: Some("{36k,msb}<166.7,-277.8|166.7,-444.4|166.7,-611.1|166.7,-777.8>(416.7,-277.8,CODE:12,166.7,^27.778m) [CODE:0..0xfff]"),
        scancode_mask: 0xfff,
        protocol_no: 24,
    },
    LinuxProtocol {
        name: "rc-mm-24",
        decoder: "rc-mm",
        irp: Some("{36k,msb}<166.7,-277.8|166.7,-444.4|166.7,-611.1|166.7,-777.8>(416.7,-277.8,CODE:24,166.7,^27.778m) [CODE:0..0xffffff]"),
        scancode_mask:  0xfff_fff,
        protocol_no: 25,
    },
    LinuxProtocol {
        name: "rc-mm-32",
        decoder: "rc-mm",
        // toggle?
        irp: Some("{36k,msb}<166.7,-277.8|166.7,-444.4|166.7,-611.1|166.7,-777.8>(416.7,-277.8,CODE:32,166.7,^27.778m) [CODE:0..0xffffffff]"),
        scancode_mask: 0xffff_ffff,
        protocol_no: 26,
    },
    LinuxProtocol {
        name: "xbox-dvd",
        decoder: "xbox-dvd",
        // trailing space is a guess, should be verified on real hardware
        irp: Some("{38k,msb}<550,-900|550,-1900>(4000,-3900,~CODE:12,CODE:12,550,^100m) [CODE:0..0xfff]"),
        scancode_mask: 0xfff,
        protocol_no: 27,
    }
];

#[test]
fn find_like() {
    let p = LinuxProtocol::find_like("rc6-mce").unwrap();
    assert_eq!(p.name, "rc6_mce");

    let p = LinuxProtocol::find_like("rcmm12").unwrap();
    assert_eq!(p.name, "rc-mm-12");

    let p = LinuxProtocol::find_like("sony-12").unwrap();
    assert_eq!(p.name, "sony12");
}
