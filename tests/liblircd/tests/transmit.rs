use liblircd::LircdConf;
use std::fs::read_to_string;

#[test]
fn encode() {
    let conf = read_to_string("../../testdata/lircd_conf/thomson/ROC740.lircd.conf").unwrap();

    //unsafe { lirc_log_set_stdout() };

    let conf = LircdConf::parse(&conf).unwrap();

    let lircd_conf: Vec<_> = conf.iter().collect();

    assert_eq!(lircd_conf.len(), 1);

    let remote = &lircd_conf[0];

    assert_eq!(remote.name(), "Thomson_ROC740");

    for code in remote.codes_iter() {
        let name = code.name();

        println!("code:{} {:x}", name, code.code());

        if name == "KEY_OK" {
            let raw = code.encode().unwrap();

            assert_eq!(
                raw,
                [
                    560, 1930, 560, 1930, 560, 4450, 560, 4450, 560, 1930, 560, 1930, 560, 1930,
                    560, 1930, 560, 4450, 560, 4450, 560, 1930, 560, 1930, 559, 2067, 560, 1930,
                    560, 1930, 560, 4450, 560, 4450, 560, 1930, 560, 1930, 560, 1930, 560, 1930,
                    560, 4450, 560, 4450, 560, 1930, 560, 1930, 559
                ]
            );
        }
    }

    let conf = read_to_string("../../testdata/lircd_conf/motorola/DCH3200.lircd.conf").unwrap();

    // unsafe { lirc_log_set_stdout() };

    let conf = LircdConf::parse(&conf).unwrap();

    let lircd_conf: Vec<_> = conf.iter().collect();

    assert_eq!(lircd_conf.len(), 2);

    let remote = &lircd_conf[0];

    assert_eq!(remote.name(), "Motorola_DCH3200");

    for code in remote.codes_iter() {
        let name = code.name();

        println!("code:{} {:x}", name, code.code());

        if name == "KEY_OK" {
            let raw = code.encode().unwrap();

            assert_eq!(
                raw,
                [
                    8990, 4411, 540, 4413, 540, 2179, 540, 2179, 540, 2179, 540, 4413, 540, 2179,
                    540, 2179, 540, 2179, 540, 2179, 540, 2179, 540, 2179, 540, 2179, 540, 2179,
                    540, 4413, 540, 4413, 540, 4413, 538
                ]
            );
        }
    }

    let conf = read_to_string("../../testdata/lircd_conf/motorola/QIP2500.lircd.conf").unwrap();

    let conf = LircdConf::parse(&conf).unwrap();

    let lircd_conf: Vec<_> = conf.iter().collect();

    assert_eq!(lircd_conf.len(), 1);

    let remote = &lircd_conf[0];

    assert_eq!(remote.name(), "Motorola_QIP2500");

    for code in remote.codes_iter() {
        let name = code.name();

        println!("code:{} {:x}", name, code.code());

        if name == "KEY_POWER" {
            let raw = code.encode().unwrap();

            assert_eq!(
                raw,
                [
                    8979, 4468, 520, 2218, 520, 4460, 520, 2218, 520, 4460, 520, 2218, 520, 2218,
                    520, 2218, 520, 2218, 520, 2218, 520, 2218, 520, 2218, 520, 2218, 520, 2218,
                    520, 4460, 520, 4460, 520, 2218, 519, 32712, 8980, 2247, 519, 87708, 8980,
                    2247, 519, 87708, 8980, 2247, 519, 87708, 8980, 2247, 519, 87708, 8980, 2247,
                    519, 87708, 8980, 2247, 519, 87708, 8980, 2247, 519
                ]
            );

            assert_eq!(
                remote.decode(&raw),
                vec!(0x5006, 0x5006, 0x5006, 0x5006, 0x5006, 0x5006, 0x5006, 0x5006)
            );
        }
    }

    // now test decode of a remote with toggle_bit_mask set (more than one bit)

    let conf = read_to_string("../../testdata/lircd_conf/d-link/DSM-10.lircd.conf").unwrap();

    //unsafe { lirc_log_set_stdout() };

    let conf = LircdConf::parse(&conf).unwrap();

    let lircd_conf: Vec<_> = conf.iter().collect();

    assert_eq!(lircd_conf.len(), 1);

    let remote = &lircd_conf[0];

    assert_eq!(remote.name(), "DLink_DSM-10");

    // encode
    let code = remote
        .codes_iter()
        .find(|code| code.name() == "KEY_1")
        .unwrap();

    let data = code.encode().unwrap();

    let result = remote.decode(&data);

    assert_eq!(result, vec![0x42BD]);

    // cargo run transmit irp '{msb}<664,-460|664,-1592>(9132,-4396,0x1067:16,(CODE^0x6a6a):16,671,^108247,(9128,-2143,671,^108247)*) [CODE:0..65535]' -fCODE=0x42BD
    let data = [
        9132, 4396, 664, 460, 664, 460, 664, 460, 664, 1592, 664, 460, 664, 460, 664, 460, 664,
        460, 664, 460, 664, 1592, 664, 1592, 664, 460, 664, 460, 664, 1592, 664, 1592, 664, 1592,
        664, 460, 664, 460, 664, 1592, 664, 460, 664, 1592, 664, 460, 664, 460, 664, 460, 664,
        1592, 664, 1592, 664, 460, 664, 1592, 664, 460, 664, 1592, 664, 1592, 664, 1592, 671,
        42232, 9128, 2143, 671, 96305,
    ];

    let result = remote.decode(&data);

    assert_eq!(result, vec![0x42BD, 0x42BD]);

    // cargo run transmit lircd testdata/lircd_conf/d-link/DSM-10.lircd.conf KEY_1
    let data = [
        9132, 4396, 664, 460, 664, 460, 664, 460, 664, 1592, 664, 460, 664, 460, 664, 460, 664,
        460, 664, 460, 664, 1592, 664, 1592, 664, 460, 664, 460, 664, 1592, 664, 1592, 664, 1592,
        664, 460, 664, 1592, 664, 460, 664, 460, 664, 460, 664, 460, 664, 1592, 664, 460, 664,
        1592, 664, 460, 664, 1592, 664, 1592, 664, 1592, 664, 1592, 664, 460, 664, 1592, 671,
        42232,
    ];
    let result = remote.decode(&data);

    assert_eq!(result, vec![0x42BD]);

    // now test decoder of a remote with an ignore_mask

    let conf = read_to_string("../../testdata/lircd_conf/apple/A1156.lircd.conf").unwrap();

    //unsafe { lirc_log_set_stdout() };

    let conf = LircdConf::parse(&conf).unwrap();

    let lircd_conf: Vec<_> = conf.iter().collect();

    assert_eq!(lircd_conf.len(), 1);

    let remote = &lircd_conf[0];

    assert_eq!(remote.name(), "Apple_A1156");

    // encode
    let code = remote
        .codes_iter()
        .find(|code| code.name() == "KEY_FASTFORWARD")
        .unwrap();

    let data = code.encode().unwrap();

    let result = remote.decode(&data);

    assert_eq!(result, vec![0xe0]);

    // cargo run transmit irp '{msb}<574,-547|574,-1668>(9065,-4484,0x77e1:16,(CODE^0x80):8,0xc5:8,567,-37.6m,(9031,-2242,567,-37.6m)*) [CODE:0..255]' -fCODE=0xe0
    let data = [
        9065, 4484, 574, 547, 574, 1668, 574, 1668, 574, 1668, 574, 547, 574, 1668, 574, 1668, 574,
        1668, 574, 1668, 574, 1668, 574, 1668, 574, 547, 574, 547, 574, 547, 574, 547, 574, 1668,
        574, 547, 574, 1668, 574, 1668, 574, 547, 574, 547, 574, 547, 574, 547, 574, 547, 574,
        1668, 574, 1668, 574, 547, 574, 547, 574, 547, 574, 1668, 574, 547, 574, 1668, 567, 37600,
        9031, 2242, 567, 37600,
    ];

    let result = remote.decode(&data);

    assert_eq!(result, vec![0xe0, 0xe0]);

    // cargo run transmit irp '{msb}<574,-547|574,-1668>(9065,-4484,0x77e1:16,(CODE^0x80):8,(0xc5^0xff):8,567,-37.6m,(9031,-2242,567,-37.6m)*) [CODE:0..255]' -fCODE=0xe0
    let data = [
        9065, 4484, 574, 547, 574, 1668, 574, 1668, 574, 1668, 574, 547, 574, 1668, 574, 1668, 574,
        1668, 574, 1668, 574, 1668, 574, 1668, 574, 547, 574, 547, 574, 547, 574, 547, 574, 1668,
        574, 547, 574, 1668, 574, 1668, 574, 547, 574, 547, 574, 547, 574, 547, 574, 547, 574, 547,
        574, 547, 574, 1668, 574, 1668, 574, 1668, 574, 547, 574, 1668, 574, 547, 567, 37600, 9031,
        2242, 567, 37600,
    ];

    let result = remote.decode(&data);

    assert_eq!(result, vec![0xe0, 0xe0]);
}
