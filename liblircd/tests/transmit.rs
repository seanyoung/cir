use liblircd::LircdConf;
use std::fs::read_to_string;

#[test]
fn encode() {
    let conf = read_to_string("../testdata/lircd_conf/thomson/ROC740.lircd.conf").unwrap();

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

    let conf = read_to_string("../testdata/lircd_conf/motorola/DCH3200.lircd.conf").unwrap();

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

    let conf = read_to_string("../testdata/lircd_conf/motorola/QIP2500.lircd.conf").unwrap();

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
}
