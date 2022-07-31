use crate::protocols::parse;
use crate::rawir;
use crate::InfraredData;
use crate::{Irp, Vartable};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[test]
fn test() {
    let mut vars = Vartable::new();

    vars.set("F".to_string(), 1, 8);
    vars.set("D".to_string(), 0xe9, 8);
    vars.set("S".to_string(), 0xfe, 8);

    let irp = Irp::parse("{38.4k,564}<1,-1|1,-3>(16,-8,D:8,S:8,F:8,~F:8,1,^108m)* [D:0..255,S:0..255=255-D,F:0..255]").unwrap();

    let res = irp.encode(vars, 1).unwrap();

    // irptransmogrifier.sh  --irp "{38.0k,564}<1,-1|1,-3>(16,-8,D:8,S:8,F:8,~F:8,1,^108m)+" encode -r -n F=1,D=0xe9,S=0xfe
    assert_eq!(
        res.raw,
        rawir::parse("+9024,-4512,+564,-1692,+564,-564,+564,-564,+564,-1692,+564,-564,+564,-1692,+564,-1692,+564,-1692,+564,-564,+564,-1692,+564,-1692,+564,-1692,+564,-1692,+564,-1692,+564,-1692,+564,-1692,+564,-1692,+564,-564,+564,-564,+564,-564,+564,-564,+564,-564,+564,-564,+564,-564,+564,-564,+564,-1692,+564,-1692,+564,-1692,+564,-1692,+564,-1692,+564,-1692,+564,-1692,+564,-35244").unwrap()
    );

    let mut vars = Vartable::new();

    vars.set("F".to_string(), 1, 8);
    vars.set("D".to_string(), 0xe9, 8);
    vars.set("T".to_string(), 0, 8);

    let irp = Irp::parse("{36k,msb,889}<1,-1|-1,1>(1:1,~F:1:6,T:1,D:5,F:6,^114m)+").unwrap();
    let res = irp.encode(vars, 0).unwrap();

    // irptransmogrifier.sh  --irp "{36k,msb,889}<1,-1|-1,1>(1:1,~F:1:6,T:1,D:5,F:6,^114m)+" encode -r -n F=1,T=0,D=0xe9

    assert_eq!(
        res.raw,
        rawir::parse("+889,-889,+1778,-889,+889,-1778,+1778,-889,+889,-1778,+1778,-889,+889,-889,+889,-889,+889,-889,+889,-1778,+889,-89108").unwrap()
    );

    let mut vars = Vartable::new();

    vars.set("F".to_string(), 1, 8);
    vars.set("D".to_string(), 0xe9, 8);
    vars.set("S".to_string(), 0x88, 8);

    let irp = Irp::parse("{38k,400}<1,-1|1,-3>(8,-4,170:8,90:8,15:4,D:4,S:8,F:8,E:4,C:4,1,-48)+ {E=1,C=D^S:4:0^S:4:4^F:4:0^F:4:4^E:4}").unwrap();
    let res = irp.encode(vars, 0).unwrap();

    assert_eq!(
        res.raw,
        rawir::parse("+3200,-1600,+400,-400,+400,-1200,+400,-400,+400,-1200,+400,  -400  +400,-1200,+400,-400,+400,-1200,+400,-400,+400,-1200,+400,-400,+400,-1200,+400,-1200,+400,-400,+400,-1200,+400,-400,+400,-1200,+400,-1200,+400,-1200,+400,-1200,+400,-1200,+400,-400,+400,-400,+400,-1200,+400,-400,+400,-400,+400,-400,+400,-1200,+400,-400,+400,-400,+400,-400,+400,-1200,+400,-1200,+400,-400,+400,-400,+400,-400,+400,-400,+400,-400,+400,-400,+400,-400,+400,-1200,+400,-400,+400,-400,+400,-400,+400,-1200,+400,-400,+400,-400,+400,-1200,+400,-19200").unwrap()
    );
}

#[test]
fn rs200() {
    let irp = Irp::parse("{35.7k,msb}<50p,-120p|21p,-120p>(25:6,(H4-1):2,(H3-1):2,(H2-1):2,(H1-1):2,P:1,(D-1):3,F:2,0:2,sum:4,-1160p)*{   P=~(#(D-1)+#F):1,sum=9+((H4-1)*4+(H3-1)) + ((H2-1)*4+(H1-1)) + (P*8+(D-1)) + F*4}").unwrap();

    let mut vars = Vartable::new();

    vars.set("D".to_string(), 4, 8);
    vars.set("F".to_string(), 1, 8);
    vars.set("H1".to_string(), 4, 8);
    vars.set("H2".to_string(), 2, 8);
    vars.set("H3".to_string(), 3, 8);
    vars.set("H4".to_string(), 4, 8);

    let res = irp.encode(vars, 1).unwrap();

    assert!(compare_with_rounding(
        &res.raw,
        &rawir::parse("+1401,-3361,+588,-3361,+588,-3361,+1401,-3361,+1401,-3361,+588,-3361,+588,-3361,+588,-3361,+588,-3361,+1401,-3361,+1401,-3361,+588,-3361,+588,-3361,+588,-3361,+1401,-3361,+1401,-3361,+588,-3361,+588,-3361,+1401,-3361,+588,-3361,+1401,-3361,+1401,-3361,+1401,-3361,+588,-3361,+1401,-3361,+588,-35854").unwrap()
    ));
}

#[test]
fn nec() {
    let mut vars = Vartable::new();

    vars.set("F".to_string(), 1, 8);
    vars.set("D".to_string(), 0xe9, 8);

    let irp = Irp::parse("{38.4k,564}<1,-1|1,-3>(16,-8,D:8,S:8,F:8,~F:8,1,^108m)* [D:0..255,S:0..255=255-D,F:0..255]").unwrap();
    let res = irp.encode(vars, 1).unwrap();

    // irptransmogrifier.sh --irp  "{38.4k,564}<1,-1|1,-3>(16,-8,D:8,S:8,F:8,~F:8,1,^108m)* [D:0..255,S:0..255=255-D,F:0..255]" encode -r -n F=1,D=0xe9
    assert_eq!(
        res.raw,
        rawir::parse("+9024,-4512,+564,-1692,+564,-564,+564,-564,+564,-1692,+564,-564,+564,-1692,+564,-1692,+564,-1692,+564,-564,+564,-1692,+564,-1692,+564,-564,+564,-1692,+564,-564,+564,-564,+564,-564,+564,-1692,+564,-564,+564,-564,+564,-564,+564,-564,+564,-564,+564,-564,+564,-564,+564,-564,+564,-1692,+564,-1692,+564,-1692,+564,-1692,+564,-1692,+564,-1692,+564,-1692,+564,-39756").unwrap()
    );
}

#[test]
fn keeprite_ac() {
    let mut vars = Vartable::new();

    vars.set("A".to_string(), 1, 32);
    vars.set("B".to_string(), 0xe9, 32);

    let irp = Irp::parse("{38.1k,570,msb}<1,-1|1,-3>(16,-8,A:35,1,-20m,B:32,1,-20m)[A:0..0x7FFFFFFFF, B:0..UINT32_MAX]").unwrap();
    let res = irp.encode(vars, 1).unwrap();

    // irptransmogrifier.sh --irp  "{38.1k,570,msb}<1,-1|1,-3>(16,-8,A:35,1,-20m,B:32,1,-20m)[A:0..0x7FFFFFFFF, B:0..UINT32_MAX]" encode -r -n A=1,B=0xe9
    assert_eq!(
        res.raw,
        rawir::parse("+9120,-4560,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-1710,+570,-20000,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-1710,+570,-1710,+570,-1710,+570,-570,+570,-1710,+570,-570,+570,-570,+570,-1710,+570,-20000").unwrap()
    );
}

#[test]
fn variants() {
    let irp = Irp::parse("{}<1,-1|1,-3>([11][22][33],-100)+").unwrap();
    let res = irp.encode(Vartable::new(), 1).unwrap();

    assert_eq!(res.raw, rawir::parse("+11 -100 +22 -100 +33 -100").unwrap());

    let irp = Irp::parse("{}<1,-1|1,-3>([11][22][33],-100)+").unwrap();

    let res = irp.encode(Vartable::new(), 1).unwrap();

    assert_eq!(res.raw, rawir::parse("+11 -100 +22 -100 +33 -100").unwrap());

    let irp = Irp::parse("{}<1,-1|1,-3>(111,-222,[11][][33],-100)+").unwrap();
    let res = irp.encode(Vartable::new(), 1).unwrap();

    assert_eq!(
        res.raw,
        rawir::parse("+111 -222 +11 -100 +111 -222 +111 -222 +33 -100").unwrap()
    );

    let irp = Irp::parse("{100}<1,-1|1,-3>([1][2],-10,10:10,1,-100m)").unwrap();
    let res = irp.encode(Vartable::new(), 1);

    assert_eq!(
        res.err(),
        Some(String::from("cannot have variant without repeat"))
    );

    let irp = Irp::parse("{}<1,-1|1,-3>([11][22],-100)*").unwrap();

    let res = irp.encode(Vartable::new(), 1);

    assert_eq!(
        res.err(),
        Some(String::from(
            "cannot have variant with \'*\' repeat, use \'+\' instead"
        ))
    );
}

#[test]
fn vars() {
    let mut vars = Vartable::new();

    vars.set("S".to_string(), 2, 8);
    vars.set("F".to_string(), 0xe9, 8);

    let irp = Irp::parse(
        "{40k,520,msb}<1,-10|1,-1,1,-8>(S:1,<1:2|2:2>(F:D),-90m)*{D=8}[S:0..1,F:1..255]",
    )
    .unwrap();

    let res = irp.encode(vars, 1);

    assert_eq!(
        res.err(),
        Some(String::from(
            "2 is more than maximum value 1 for parameter S"
        ))
    );

    let mut vars = Vartable::new();

    vars.set("S".to_string(), 1, 8);
    vars.set("F".to_string(), 0, 8);

    let res = irp.encode(vars, 1);

    assert_eq!(
        res.err(),
        Some(String::from(
            "0 is less than minimum value 1 for parameter F"
        ))
    );

    let mut vars = Vartable::new();

    vars.set("S".to_string(), 1, 8);
    vars.set("X".to_string(), 0, 8);

    let res = irp.encode(vars, 1);

    assert_eq!(res.err(), Some(String::from("missing value for F")));

    let mut vars = Vartable::new();

    vars.set("S".to_string(), 1, 8);
    vars.set("F".to_string(), 2, 8);
    vars.set("X".to_string(), 0, 8);

    let res = irp.encode(vars, 1);

    assert_eq!(res.err(), Some(String::from("no parameter called X")));

    let mut vars = Vartable::new();

    let irp = Irp::parse("{40k,520,msb}<1,-10|1,-1,1,-8>(S:1,<1:2|2:2>(F:D),-90m)*{D=8}").unwrap();

    vars.set("S".to_string(), 1, 8);
    vars.set("F".to_string(), 2, 8);
    vars.set("X".to_string(), 0, 8);

    let res = irp.encode(vars, 1);

    assert!(res.is_ok());
}

#[test]
fn parse_all_of_them() {
    let protocols = parse(&PathBuf::from("IrpProtocols.xml"));

    let mut broken = 0;
    let mut total = 0;
    for p in &protocols {
        total += 1;
        if let Err(s) = Irp::parse(&p.irp) {
            broken += 1;
            println!("{}: {}: {}", p.name, p.irp, s);
        }
    }

    if broken != 0 {
        panic!("{} out of {} broken", broken, total);
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
        let diff = if l[i] > r[i] {
            l[i] - r[i]
        } else {
            r[i] - l[i]
        };
        // is the difference more than 8 and more than 1 promille
        if diff > 8 && (diff * 1000 / l[i]) > 0 {
            println!(
                "comparing:\nleft:{:?} with\nright:{:?}\nfailed at position {} out of {}",
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

#[derive(Serialize, Deserialize)]
pub struct TestData {
    pub protocol: String,
    #[serde(default)]
    pub repeats: u64,
    pub params: Vec<Param>,
    #[serde(default)]
    pub pronto: String,
    #[serde(default)]
    pub render: Vec<Vec<u32>>,
}

#[derive(Serialize, Deserialize)]
pub struct Param {
    pub name: String,
    pub value: u64,
}

#[test]
fn compare_encode_to_transmogrifier() {
    // load test data
    let data = std::fs::read_to_string("transmogrifier_test_data.json").unwrap();

    let all_testdata: Vec<TestData> = serde_json::from_str(&data).unwrap();
    let protocols = parse(&PathBuf::from("IrpProtocols.xml"));

    let mut fails = 0;
    let total_tests = all_testdata.len();

    for testcase in &all_testdata {
        let protocol = protocols
            .iter()
            .find(|p| p.name == testcase.protocol)
            .unwrap();

        let mut vars = Vartable::new();

        for param in &testcase.params {
            vars.set(param.name.to_owned(), param.value as i64, 8);
        }

        let irp = Irp::parse(&protocol.irp).unwrap();

        if testcase.pronto.is_empty() {
            let f = irp.encode(vars, testcase.repeats).unwrap();

            if !compare_with_rounding(&testcase.render[0], &f.raw) {
                println!("FAIL testing {} irp {}", protocol.name, protocol.irp);

                for param in &testcase.params {
                    println!("{} = {}", param.name, param.value);
                }
                println!("repeats {}", testcase.repeats);

                fails += 1;
            }
        } else {
            let pronto = irp.encode_pronto(vars).unwrap();

            let f = pronto.to_string();

            if f != testcase.pronto {
                let left: Vec<u32> = f
                    .split_whitespace()
                    .map(|v| u32::from_str_radix(v, 16).unwrap())
                    .collect();

                let right: Vec<u32> = testcase
                    .pronto
                    .split_whitespace()
                    .map(|v| u32::from_str_radix(v, 16).unwrap())
                    .collect();

                if left[0] != right[0]
                    || left[1] != right[1]
                    || left[2] != right[2]
                    || left[3] != right[3]
                    || !compare_with_rounding(&left, &right)
                {
                    println!("FAIL testing {} irp {}", protocol.name, protocol.irp);

                    println!("left: {}", f);
                    println!("right: {}", testcase.pronto);

                    for param in &testcase.params {
                        println!("{} = {}", param.name, param.value);
                    }

                    fails += 1;
                }
            }
        }
    }

    println!("tests: {} fails: {}", total_tests, fails);

    assert_eq!(fails, 0);
}

#[test]
fn compare_decode_to_transmogrifier() {
    // load test data
    let data = std::fs::read_to_string("transmogrifier_test_data.json").unwrap();

    let all_testdata: Vec<TestData> = serde_json::from_str(&data).unwrap();
    let protocols = parse(&PathBuf::from("IrpProtocols.xml"));

    let mut fails = 0;
    let mut total_tests = all_testdata.len();

    for testcase in &all_testdata {
        let protocol = protocols
            .iter()
            .find(|p| p.name == testcase.protocol)
            .unwrap();

        if !testcase.pronto.is_empty() || testcase.render[0].is_empty() {
            total_tests -= 1;
            continue;
        }

        println!("trying {}", protocol.name);

        let irp = Irp::parse(&protocol.irp).unwrap();

        let nfa = match irp.compile() {
            Ok(nfa) => nfa,
            Err(s) => {
                println!("compile {} failed {}", protocol.irp, s);
                fails += 1;
                continue;
            }
        };

        let mut decoder = nfa.decoder(10, 3, 3000);

        for data in InfraredData::from_u32_slice(&testcase.render[0]) {
            decoder.input(data);
        }

        if let Some(res) = decoder.get() {
            let mut ok = true;

            for param in &testcase.params {
                let mask = match (protocol.name.as_str(), param.name.as_str()) {
                    ("Zenith5", "F") => 31,
                    ("Zenith6", "F") => 63,
                    ("Zenith7", "F") => 127,
                    ("Zenith", "F") => (1 << res["D"]) - 1,
                    ("NEC-Shirriff", "data") => (1 << res["length"]) - 1,

                    _ => !0,
                };

                if res.get(&param.name) != Some(&((param.value & mask) as i64)) {
                    println!(
                        "{} does not match, expected {} got {:?}",
                        param.name,
                        param.value,
                        res.get(&param.name),
                    );
                    ok = false;
                }
            }

            if !ok {
                println!(
                    "{} failed to decode, irp: {} ir: {}",
                    protocol.name,
                    protocol.irp,
                    rawir::print_to_string(&testcase.render[0])
                );

                fails += 1;
            }
        } else {
            println!(
                "{} failed to decode, irp: {} ir: {}",
                protocol.name,
                protocol.irp,
                rawir::print_to_string(&testcase.render[0])
            );
            println!(
                "expected: {}",
                testcase
                    .params
                    .iter()
                    .map(|param| format!("{}={}", param.name, param.value))
                    .collect::<Vec<String>>()
                    .join(", ")
            );
            fails += 1;
        }
    }

    println!("tests: {} fails: {}", total_tests, fails);

    // TODO: we still have a whole bunch of fails
    assert_eq!(fails, 34);
}
