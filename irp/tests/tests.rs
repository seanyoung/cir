use irp::{Decoder, Event, InfraredData, Irp, Message, Options, Protocol, Vartable};
use irptransmogrifier::{create_jvm, IrpTransmogrifierRender};
use itertools::Itertools;
use rand::Rng;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

#[test]
fn test() {
    let mut vars = Vartable::new();

    vars.set("F".to_string(), 1);
    vars.set("D".to_string(), 0xe9);
    vars.set("S".to_string(), 0xfe);

    let irp = Irp::parse("{38.4k,564}<1,-1|1,-3>(16,-8,D:8,S:8,F:8,~F:8,1,^108m)* [D:0..255,S:0..255=255-D,F:0..255]").unwrap();

    let res = irp.encode_raw(vars, 1).unwrap();

    // irptransmogrifier.sh  --irp "{38.0k,564}<1,-1|1,-3>(16,-8,D:8,S:8,F:8,~F:8,1,^108m)+" encode -r -n F=1,D=0xe9,S=0xfe
    assert_eq!(
        res.raw,
        Message::parse("+9024,-4512,+564,-1692,+564,-564,+564,-564,+564,-1692,+564,-564,+564,-1692,+564,-1692,+564,-1692,+564,-564,+564,-1692,+564,-1692,+564,-1692,+564,-1692,+564,-1692,+564,-1692,+564,-1692,+564,-1692,+564,-564,+564,-564,+564,-564,+564,-564,+564,-564,+564,-564,+564,-564,+564,-564,+564,-1692,+564,-1692,+564,-1692,+564,-1692,+564,-1692,+564,-1692,+564,-1692,+564,-35244").unwrap().raw
    );

    let mut vars = Vartable::new();

    vars.set("F".to_string(), 1);
    vars.set("D".to_string(), 0xe9);
    vars.set("T".to_string(), 0);

    let irp = Irp::parse("{36k,msb,889}<1,-1|-1,1>(1:1,~F:1:6,T:1,D:5,F:6,^114m)+").unwrap();
    let res = irp.encode_raw(vars, 0).unwrap();

    // irptransmogrifier.sh  --irp "{36k,msb,889}<1,-1|-1,1>(1:1,~F:1:6,T:1,D:5,F:6,^114m)+" encode -r -n F=1,T=0,D=0xe9

    assert_eq!(
        res.raw,
        Message::parse("+889,-889,+1778,-889,+889,-1778,+1778,-889,+889,-1778,+1778,-889,+889,-889,+889,-889,+889,-889,+889,-1778,+889,-89108").unwrap().raw
    );

    let mut vars = Vartable::new();

    vars.set("F".to_string(), 1);
    vars.set("D".to_string(), 0xe9);
    vars.set("S".to_string(), 0x88);

    let irp = Irp::parse("{38k,400}<1,-1|1,-3>(8,-4,170:8,90:8,15:4,D:4,S:8,F:8,E:4,C:4,1,-48)+ {E=1,C=D^S:4:0^S:4:4^F:4:0^F:4:4^E:4}").unwrap();
    let res = irp.encode_raw(vars, 0).unwrap();

    assert_eq!(
        res.raw,
        Message::parse("+3200,-1600,+400,-400,+400,-1200,+400,-400,+400,-1200,+400,  -400  +400,-1200,+400,-400,+400,-1200,+400,-400,+400,-1200,+400,-400,+400,-1200,+400,-1200,+400,-400,+400,-1200,+400,-400,+400,-1200,+400,-1200,+400,-1200,+400,-1200,+400,-1200,+400,-400,+400,-400,+400,-1200,+400,-400,+400,-400,+400,-400,+400,-1200,+400,-400,+400,-400,+400,-400,+400,-1200,+400,-1200,+400,-400,+400,-400,+400,-400,+400,-400,+400,-400,+400,-400,+400,-400,+400,-1200,+400,-400,+400,-400,+400,-400,+400,-1200,+400,-400,+400,-400,+400,-1200,+400,-19200").unwrap().raw
    );
}

#[test]
fn rs200() {
    let irp = Irp::parse("{35.7k,msb}<50p,-120p|21p,-120p>(25:6,(H4-1):2,(H3-1):2,(H2-1):2,(H1-1):2,P:1,(D-1):3,F:2,0:2,sum:4,-1160p)*{   P=~(#(D-1)+#F):1,sum=9+((H4-1)*4+(H3-1)) + ((H2-1)*4+(H1-1)) + (P*8+(D-1)) + F*4}").unwrap();

    let mut vars = Vartable::new();

    vars.set("D".to_string(), 4);
    vars.set("F".to_string(), 1);
    vars.set("H1".to_string(), 4);
    vars.set("H2".to_string(), 2);
    vars.set("H3".to_string(), 3);
    vars.set("H4".to_string(), 4);

    let res = irp.encode_raw(vars, 1).unwrap();

    assert!(compare_with_rounding(
        &res.raw,
        &Message::parse("+1401,-3361,+588,-3361,+588,-3361,+1401,-3361,+1401,-3361,+588,-3361,+588,-3361,+588,-3361,+588,-3361,+1401,-3361,+1401,-3361,+588,-3361,+588,-3361,+588,-3361,+1401,-3361,+1401,-3361,+588,-3361,+588,-3361,+1401,-3361,+588,-3361,+1401,-3361,+1401,-3361,+1401,-3361,+588,-3361,+1401,-3361,+588,-35854").unwrap().raw
    ));
}

#[test]
fn nec() {
    let mut vars = Vartable::new();

    vars.set("F".to_string(), 1);
    vars.set("D".to_string(), 0xe9);

    let irp = Irp::parse("{38.4k,564}<1,-1|1,-3>(16,-8,D:8,S:8,F:8,~F:8,1,^108m)* [D:0..255,S:0..255=255-D,F:0..255]").unwrap();
    let res = irp.encode_raw(vars, 1).unwrap();

    // irptransmogrifier.sh --irp  "{38.4k,564}<1,-1|1,-3>(16,-8,D:8,S:8,F:8,~F:8,1,^108m)* [D:0..255,S:0..255=255-D,F:0..255]" encode -r -n F=1,D=0xe9
    assert_eq!(
        res.raw,
        Message::parse("+9024,-4512,+564,-1692,+564,-564,+564,-564,+564,-1692,+564,-564,+564,-1692,+564,-1692,+564,-1692,+564,-564,+564,-1692,+564,-1692,+564,-564,+564,-1692,+564,-564,+564,-564,+564,-564,+564,-1692,+564,-564,+564,-564,+564,-564,+564,-564,+564,-564,+564,-564,+564,-564,+564,-564,+564,-1692,+564,-1692,+564,-1692,+564,-1692,+564,-1692,+564,-1692,+564,-1692,+564,-39756").unwrap().raw
    );
}

#[test]
fn keeprite_ac() {
    let mut vars = Vartable::new();

    vars.set("A".to_string(), 1);
    vars.set("B".to_string(), 0xe9);

    let irp = Irp::parse("{38.1k,570,msb}<1,-1|1,-3>(16,-8,A:35,1,-20m,B:32,1,-20m)[A:0..0x7FFFFFFFF, B:0..UINT32_MAX]").unwrap();
    let res = irp.encode_raw(vars, 1).unwrap();

    // irptransmogrifier.sh --irp  "{38.1k,570,msb}<1,-1|1,-3>(16,-8,A:35,1,-20m,B:32,1,-20m)[A:0..0x7FFFFFFFF, B:0..UINT32_MAX]" encode -r -n A=1,B=0xe9
    assert_eq!(
        res.raw,
        Message::parse("+9120,-4560,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-1710,+570,-20000,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-570,+570,-1710,+570,-1710,+570,-1710,+570,-570,+570,-1710,+570,-570,+570,-570,+570,-1710,+570,-20000").unwrap().raw
    );
}

#[test]
fn variants() {
    let irp = Irp::parse("{}<1,-1|1,-3>([11][22][33],-100)+").unwrap();
    let res = irp.encode_raw(Vartable::new(), 1).unwrap();

    assert_eq!(
        res.raw,
        Message::parse("+11 -100 +22 -100 +33 -100").unwrap().raw
    );

    let irp = Irp::parse("{}<1,-1|1,-3>([11][22][33],-100)+").unwrap();

    let res = irp.encode_raw(Vartable::new(), 1).unwrap();

    assert_eq!(
        res.raw,
        Message::parse("+11 -100 +22 -100 +33 -100").unwrap().raw
    );

    let irp = Irp::parse("{}<1,-1|1,-3>(111,-222,[11][][33],-100)+").unwrap();
    let res = irp.encode_raw(Vartable::new(), 1).unwrap();

    assert_eq!(
        res.raw,
        Message::parse("+111 -222 +11 -100 +111 -222 +111 -222 +33 -100")
            .unwrap()
            .raw
    );

    let irp = Irp::parse("{100}<1,-1|1,-3>([1][2],-10,10:10,1,-100m)");

    assert_eq!(
        irp.err(),
        Some(String::from("variant [1][2] found without repeat marker"))
    );

    let irp = Irp::parse("{}<1,-1|1,-3>([11][22],-100)*");

    assert_eq!(
        irp.err(),
        Some(String::from(
            "cannot have variant with \'*\' repeat, use \'+\' instead"
        ))
    );

    let irp = Irp::parse("{58k,10}<1,-2|1,-4>([40][30][20],-100,[1][2][3],-16m)+").unwrap();
    let res = irp.encode_raw(Vartable::new(), 1).unwrap();

    assert_eq!(
        res.raw,
        Message::parse("+400 -1000 +10 -16000 +300 -1000 +20 -16000 +200 -1000 +30 -16000")
            .unwrap()
            .raw
    );

    let irp = Irp::parse("{58k,10}<1,-2|1,-4>([40][30][],-100,[1][2][3],-16m)+").unwrap();
    let res = irp.encode_raw(Vartable::new(), 1).unwrap();

    assert_eq!(
        res.raw,
        Message::parse("+400 -1000 +10 -16000 +300 -1000 +20 -16000")
            .unwrap()
            .raw
    );

    let irp = Irp::parse("{58k,10}<1,-2|1,-4>([40][30],-100,[1][2][3],-16m)+").unwrap();
    let res = irp.encode_raw(Vartable::new(), 1).unwrap();

    assert_eq!(
        res.raw,
        Message::parse("+400 -1000 +10 -16000 +300 -1000 +20 -16000")
            .unwrap()
            .raw
    );

    let irp = Irp::parse("{58k,10}<1,-2|1,-4>([40][30][20],-100,[1][2][],-16m)+").unwrap();
    let res = irp.encode_raw(Vartable::new(), 1).unwrap();

    assert_eq!(
        res.raw,
        Message::parse("+400 -1000 +10 -16000 +300 -1000 +20 -16000 +200 -1000")
            .unwrap()
            .raw
    );
}

#[test]
fn vars() {
    let mut vars = Vartable::new();

    vars.set("S".to_string(), 2);
    vars.set("F".to_string(), 0xe9);

    let irp = Irp::parse(
        "{40k,520,msb}<1,-10|1,-1,1,-8>(S:1,<1:2|2:2>(F:D),-90m)*{D=8}[S:0..1,F:1..255]",
    )
    .unwrap();

    let res = irp.encode_raw(vars, 1);

    assert_eq!(
        res.err(),
        Some(String::from(
            "2 is more than maximum value 1 for parameter S"
        ))
    );

    let mut vars = Vartable::new();

    vars.set("S".to_string(), 1);
    vars.set("F".to_string(), 0);

    let res = irp.encode_raw(vars, 1);

    assert_eq!(
        res.err(),
        Some(String::from(
            "0 is less than minimum value 1 for parameter F"
        ))
    );

    let mut vars = Vartable::new();

    vars.set("S".to_string(), 1);
    vars.set("X".to_string(), 0);

    let res = irp.encode_raw(vars, 1);

    assert_eq!(res.err(), Some(String::from("missing value for F")));

    let mut vars = Vartable::new();

    vars.set("S".to_string(), 1);
    vars.set("F".to_string(), 2);
    vars.set("X".to_string(), 0);

    let res = irp.encode_raw(vars, 1);

    assert_eq!(res.err(), Some(String::from("no parameter called X")));

    let mut vars = Vartable::new();

    let irp = Irp::parse("{40k,520,msb}<1,-10|1,-1,1,-8>(S:1,<1:2|2:2>(F:D),-90m)*{D=8}").unwrap();

    vars.set("S".to_string(), 1);
    vars.set("F".to_string(), 2);
    vars.set("X".to_string(), 0);

    let res = irp.encode_raw(vars, 1);

    assert!(res.is_ok());
}

#[test]
fn parse_all_of_them() {
    let protocols = Protocol::parse(&PathBuf::from(
        "../IrpTransmogrifier/src/main/resources/IrpProtocols.xml",
    ))
    .unwrap();

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
        panic!("{broken} out of {total} broken");
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

#[test]
fn compare_encode_to_transmogrifier() {
    let mut protocols = Protocol::parse(&PathBuf::from(
        "../IrpTransmogrifier/src/main/resources/IrpProtocols.xml",
    ))
    .unwrap();

    let mut total_tests = 0;
    let mut fails = 0;
    let jvm = create_jvm("../IrpTransmogrifier");
    let mut rng = rand::thread_rng();

    protocols.push(Protocol {
        irp: "{37k,432}<1,-1|1,-3>(8,-4,84:8,50:8,0:4,D:4,S:4,F:12,((D)^S^(F*16)):8,1,-173)* [D:0..15,S:0..15,F:0..4095]".into(),
        ..Default::default()
    });

    for protocol in &protocols {
        let irp = Irp::parse(&protocol.irp).unwrap();

        let trans_irp = IrpTransmogrifierRender::new(&jvm, &protocol.irp).unwrap();

        let mut vars = Vartable::new();

        let mut params = HashMap::new();

        if irp.parameters.is_empty() {
            println!("irp {} has not parameters, skipping", protocol.irp);
            continue;
        }

        for param in &irp.parameters {
            let value = rng.gen_range(param.min..=param.max);

            params.insert(param.name.to_owned(), value);
            vars.set(param.name.to_owned(), value);
        }

        let msg = irp.encode(vars.clone()).unwrap();
        let trans_msg = trans_irp.render_raw(params.clone()).unwrap();

        for i in 0..3 {
            if !compare_with_rounding(&msg[i], &trans_msg[i]) {
                println!("FAIL testing {} irp {}", protocol.name, protocol.irp);

                for (name, value) in &params {
                    println!("{name} = {value}");
                }

                fails += 1;
            }
        }

        total_tests += 1;

        // Test pronto
        let trans_pronto = trans_irp.render_pronto(params.clone()).unwrap();

        let pronto = irp.encode_pronto(vars).unwrap().to_string();

        if pronto != trans_pronto {
            let left: Vec<u32> = pronto
                .split_whitespace()
                .map(|v| u32::from_str_radix(v, 16).unwrap())
                .collect();

            let right: Vec<u32> = trans_pronto
                .split_whitespace()
                .map(|v| u32::from_str_radix(v, 16).unwrap())
                .collect();

            if left[0] != right[0]
                || left[1] != right[1]
                || left[2] != right[2]
                || left[3] != right[3]
                || !compare_with_rounding(&left, &right)
            {
                println!("FAIL testing pronto {} irp {}", protocol.name, protocol.irp);

                println!("left: {pronto}");
                println!("right: {trans_pronto}");

                for (name, value) in &params {
                    println!("{name} = {value}");
                }

                fails += 1;
            }
        }

        total_tests += 1;
    }

    println!("tests: {total_tests} fails: {fails}");

    assert_eq!(fails, 0);
}

#[test]
fn decode_all() {
    let mut protocols = Protocol::parse(&PathBuf::from(
        "../IrpTransmogrifier/src/main/resources/IrpProtocols.xml",
    ))
    .unwrap();

    let mut total_tests = 0;
    let mut fails = 0;
    let mut failed_protocols: HashSet<&str> = HashSet::new();
    let mut rng = rand::thread_rng();

    for protocol in &mut protocols {
        println!("trying {}", protocol.name);

        if protocol.name == "NEC-Shirriff" {
            protocol.irp = "{38.4k,msb,564}<1,-1|1,-3>(16,-8,data:length,1,^128m) [data:0..UINT32_MAX,length:1..63]".into();
        }

        let irp = Irp::parse(&protocol.irp).unwrap();

        let nfa = match irp.build_nfa() {
            Ok(nfa) => nfa,
            Err(s) => {
                println!("compile {} failed {}", protocol.irp, s);
                fails += 1;
                continue;
            }
        };

        let max_gap = if protocol.name == "Epson" {
            100000
        } else if protocol.name == "NRC17" {
            110500
        } else {
            20000
        };

        let options = Options {
            aeps: 10,
            eps: 3,
            max_gap,
            ..Default::default()
        };

        let dfa = nfa.build_dfa(&options);

        let mut decoder = Decoder::new(options);

        let first = if irp.has_ending() { 1 } else { 0 };

        for n in first..10 {
            let repeats = if n < 3 { n } else { rng.gen_range(n..n + 20) };

            decoder.dfa_input(InfraredData::Reset, &dfa, |_, _| {});

            let mut vars = Vartable::new();
            let mut params = HashMap::new();

            for param in &irp.parameters {
                let value = rng.gen_range(param.min..=param.max);

                params.insert(param.name.to_owned(), value);
                vars.set(param.name.to_owned(), value);
            }

            let msg = irp.encode_raw(vars, repeats).unwrap();

            if msg.raw.len() < 3 {
                println!("protocol:{} repeats:{} too short", protocol.name, repeats);
                continue;
            }

            total_tests += 1;

            let mut decodes = Vec::new();

            for data in InfraredData::from_u32_slice(&msg.raw) {
                decoder.dfa_input(data, &dfa, |ev, res| decodes.push((ev, res)));
            }

            // if nothing decoded, we fail to decode
            let mut ok = !decodes.is_empty();

            while let Some((ev, mut res)) = decodes.pop() {
                if ev == Event::Down && res.is_empty() {
                    continue;
                }

                for param in &irp.parameters {
                    let mask = match (protocol.name.as_str(), param.name.as_str()) {
                        ("Zenith5", "F") => 31,
                        ("Zenith6", "F") => 63,
                        ("Zenith7", "F") => 127,
                        ("Zenith", "F") => (1 << res["D"]) - 1,
                        ("NEC-Shirriff", "data") if res["length"] < 64 => {
                            let mask = (1u64 << res["length"]) - 1u64;
                            mask as i64
                        }
                        ("Fujitsu_Aircon_old", "tOn") => !0xf0,
                        _ => !0,
                    };

                    let value = params[&param.name];

                    if let Some(v) = res.get(&param.name) {
                        if (v & mask) != (value & mask) {
                            println!(
                                "{ev} {} does not match, expected {value} got {v}",
                                param.name
                            );
                        } else {
                            res.insert(param.name.to_owned(), value);
                        }
                    } else {
                        println!("{ev} {} not decoded", param.name);
                    }
                }

                if params != res {
                    ok = false;
                    failed_protocols.insert(&protocol.name);
                }
            }

            if !ok {
                println!(
                    "{} failed to decode, irp: {} ir: {}",
                    protocol.name,
                    protocol.irp,
                    msg.print_rawir()
                );

                println!(
                    "expected: {}",
                    irp.parameters
                        .iter()
                        .map(|param| format!("{}={}", param.name, params[&param.name]))
                        .join(",")
                );

                fails += 1;
            }
        }
    }

    println!("tests: {total_tests} fails: {fails}");
    println!("Failed protocols: {failed_protocols:?}");

    let expected_failures = HashSet::from(["Pioneer-2Part", "Epson", "F12x", "Roku-8bit"]);
    assert_eq!(failed_protocols, expected_failures);
}

#[test]
fn max_bitspec() {
    let irp = Irp::parse(
        "{38.6k,480}<1,-1|-1,1,-1|-1,1>([][P=1][P=2],4,-2,F:6,C:4,-48m)*{C=3+#D+#P+#F}[D:0..31,F:0..63]",
    )
    .unwrap();

    let mut vars = Vartable::new();

    vars.set("D".to_string(), 3);
    vars.set("F".to_string(), 3);

    let res = irp.encode_raw(vars, 1);

    assert_eq!(
        res.err(),
        Some(String::from("Cannot encode 3 with current bit_spec"))
    );

    let mut vars = Vartable::new();

    vars.set("D".to_string(), 2);
    vars.set("F".to_string(), 1);

    let res = irp.encode_raw(vars, 1);

    assert_eq!(
        res.unwrap().raw,
        Message::parse("+1920 -1440 +480 -480 +480 -480 +480 -960 +480 -480 +480 -48480 +1920 -1440 +480 -480 +480 -480 +480 -960 +480 -480 +480 -48480").unwrap().raw,
    );

    let irp = Irp::parse("{33k,1}<16p,-p>(F:1)2[F:0..1]").unwrap();

    let mut vars = Vartable::new();

    vars.set("F".to_string(), 1);

    let res = irp.encode_raw(vars, 1);

    assert_eq!(
        res.err(),
        Some(String::from("Cannot encode 1 with current bit_spec"))
    );

    let irp = Irp::parse("{33k,1}<16p,-p|8p,-p|4p,-p>(F:1)2[F:0..1]").unwrap();

    let mut vars = Vartable::new();

    vars.set("F".to_string(), 1);

    let res = irp.encode_raw(vars, 1);

    assert_eq!(
        res.err(),
        Some(String::from("Cannot encode 3 with current bit_spec"))
    );
}

#[test]
fn unit0() {
    let irp = Irp::parse("{0}<1|-1>(1m,-100m)").unwrap();

    let vars = Vartable::new();

    let res = irp.encode_raw(vars, 1);

    assert!(res.is_ok());

    let irp = Irp::parse("{0}<1|-1>(1m,1:1,-100m)").unwrap();

    let vars = Vartable::new();

    let res = irp.encode_raw(vars, 1);

    assert!(res.is_err());
}

#[test]
fn arithmetic_in_bitspec() {
    let irp = Irp::parse("{.0k}<A=2-3|-1>(1m,-100m)").unwrap();

    let vars = Vartable::new();

    let res = irp.encode_raw(vars, 1);

    assert!(res.is_ok());

    let irp = Irp::parse("{.0k}<2,-3||-1>(1m,-100m)");

    assert_eq!(irp.err(), Some("parse error at error at 1:11: expected one of \",\", \".\", \">\", \"m\", \"p\", \"u\", ['0'..='9']".into()));

    let irp = Irp::parse("{100}<>(1,-2,2u,-100m)").unwrap();

    let vars = Vartable::new();

    let res = irp.encode_raw(vars, 1);

    assert_eq!(res.unwrap().raw, vec![100, 200, 2, 100000]);

    let irp = Irp::parse("{100}<| |>(1,-2,2u,-100m)").unwrap();

    let vars = Vartable::new();

    let res = irp.encode_raw(vars, 1);

    assert_eq!(res.unwrap().raw, vec![100, 200, 2, 100000]);
}

#[test]
fn leading_gap() {
    let irp = Irp::parse("{100}<1>(^40,2,-20)3+").unwrap();

    let vars = Vartable::new();

    let res = irp.encode_raw(vars, 1);

    assert_eq!(
        res.unwrap().raw,
        vec![200, 3800, 200, 3800, 200, 2000, 200, 2000]
    );
}

#[test]
fn complement_definition() {
    let irp = Irp::parse(
        "{38.1k,100}<1,-1|1,-3>(C:8,-25m)* { C=~(D+S+F+255):6} [D:0..255,S:0..255,F:0..255]",
    )
    .unwrap();

    let mut vars = Vartable::new();

    vars.set("F".to_string(), 1);
    vars.set("D".to_string(), 13);
    vars.set("S".to_string(), 251);

    let res = irp.encode_raw(vars, 1);

    assert_eq!(
        res.unwrap().raw,
        vec![100, 300, 100, 300, 100, 300, 100, 100, 100, 300, 100, 300, 100, 100, 100, 25100]
    );

    let irp = Irp::parse(
        "{38.1k,100}<1,-1|1,-3>(C:8,-25m)* { C=~(D+S+F+255):6+256 } [D:0..255,S:0..255,F:0..255]",
    )
    .unwrap();

    let mut vars = Vartable::new();

    vars.set("F".to_string(), 1);
    vars.set("D".to_string(), 13);
    vars.set("S".to_string(), 251);

    let res = irp.encode_raw(vars, 1);

    assert_eq!(
        res.unwrap().raw,
        vec![100, 300, 100, 300, 100, 300, 100, 100, 100, 300, 100, 300, 100, 100, 100, 25100]
    );
}

#[test]
fn negative_vars() {
    let irp = Irp::parse("{100}<1,-1|1,-3>(10,-P,10,P,-1000)").unwrap();

    let mut vars = Vartable::new();

    vars.set("P".to_string(), 3);

    let res = irp.encode_raw(vars, 1);

    assert_eq!(res.unwrap().raw, vec![1000, 300, 1300, 100000]);

    let mut vars = Vartable::new();

    vars.set("P".to_string(), -3);

    let res = irp.encode_raw(vars, 1);

    assert_eq!(res.unwrap().raw, vec![2300, 100300]);
}

#[test]
fn parse_or_in_bitspec() {
    // should be parsed as a single bit spec, last=-1|-2 is an expression
    let irp = Irp::parse(
        "{416,38k,msb}<last,1,last=-1|-2,last=1>(1,CODE:31,^106m){last=1} [CODE:0..0x7fffffff]",
    )
    .unwrap();

    let mut vars = Vartable::new();

    vars.set("CODE".to_string(), 0x1fffffff);

    let res = irp.encode_raw(vars, 1);

    assert_eq!(
        res.err(),
        Some("Cannot encode 1 with current bit_spec".into())
    );

    // with u to force parser to parse it as
    // bit 0: last,1,last=-1
    // bit 1: -2u,last=1
    let irp = Irp::parse(
        "{416,38k,msb}<last,1,last=-1|-2u,last=1>(1,CODE:31,^106m){last=1} [CODE:0..0x7fffffff]",
    )
    .unwrap();

    let mut vars = Vartable::new();

    vars.set("CODE".to_string(), 0x1fffffff);

    let res = irp.encode_raw(vars, 1);

    assert_eq!(res.unwrap().raw, vec![1248, 416, 416, 103920]);
}

#[test]
fn zero_bitspec() {
    let irp = Irp::parse("{10}<1,-1|1,-3>(C:4){C=255:0}").unwrap();

    let mut vars = Vartable::new();

    vars.set("CODE".to_string(), 0x1fffffff);

    let res = irp.encode_raw(vars, 1);

    assert_eq!(res.unwrap().raw, vec![10, 10, 10, 10, 10, 10, 10, 10]);
}

#[test]
fn logical_and_or() {
    let irp = Irp::parse("{10}<-1,1|-3,1>(10,C:4,D:4,-10){C=A&&B,D=A||B}").unwrap();

    let mut vars = Vartable::new();

    vars.set("A".to_string(), 0x00);
    vars.set("B".to_string(), 0xa);

    let res = irp.encode_raw(vars, 1);

    assert_eq!(
        res.unwrap().raw,
        vec![100, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 30, 10, 10, 10, 30, 10, 100]
    );

    let mut vars = Vartable::new();

    vars.set("A".to_string(), 0x5);
    vars.set("B".to_string(), 0x1);

    let res = irp.encode_raw(vars, 1);

    assert_eq!(
        res.unwrap().raw,
        vec![100, 30, 10, 10, 10, 10, 10, 10, 10, 30, 10, 10, 10, 30, 10, 10, 10, 100]
    );
}
