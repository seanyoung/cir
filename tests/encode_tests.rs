use assert_cmd::Command;

#[test]
fn encode_test() {
    let mut cmd = Command::cargo_bin("cir").unwrap();

    let assert = cmd
        .args(&[
            "encode",
            "irp",
            "-fF=12",
            "{40k,600}<1,-1|2,-1>(4,-1,F:8,^45m)+[F:0..255]",
        ])
        .assert();

    let output = assert.get_output();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(stderr, "");

    assert_eq!(
        stdout,
        r#"carrier: 40000Hz
rawir: +2400 -600 +600 -600 +600 -600 +1200 -600 +1200 -600 +600 -600 +600 -600 +600 -600 +600 -31800 +2400 -600 +600 -600 +600 -600 +1200 -600 +1200 -600 +600 -600 +600 -600 +600 -600 +600 -31800
"#
    );
}

#[test]
fn encode_lircd_raw_test() {
    let mut cmd = Command::cargo_bin("cir").unwrap();

    let assert = cmd
        .args(&[
            "encode",
            "lircd",
            "testdata/lircd_conf/pace/DC420N.lircd.conf",
            "1",
        ])
        .assert();

    let output = assert.get_output();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(stderr, "");

    assert_eq!(
        stdout,
        r#"carrier: 36000Hz
duty cycle: 50%
rawir: +2664 -888 +444 -444 +444 -444 +444 -888 +444 -888 +888 -444 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +888 -123496
"#
    );
}

#[test]
fn encode_lircd_aiwa_test() {
    let mut cmd = Command::cargo_bin("cir").unwrap();

    let assert = cmd
        .args(&[
            "encode",
            "lircd",
            "testdata/lircd_conf/aiwa/RC-5VP05.lircd.conf",
            "AUTO",
        ])
        .assert();

    let output = assert.get_output();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(stderr, "");

    assert_eq!(
        stdout,
        r#"carrier: 38000Hz
rawir: +9137 -4360 +688 -1558 +688 -1558 +688 -1558 +688 -1558 +688 -1558 +688 -1558 +688 -1558 +688 -441 +688 -441 +688 -441 +688 -441 +688 -441 +688 -441 +688 -441 +688 -441 +688 -441 +688 -441 +688 -441 +688 -441 +688 -441 +688 -1558 +688 -1558 +688 -1558 +688 -1558 +688 -1558 +688 -1558 +688 -441 +688 -441 +688 -1558 +688 -1558 +688 -1558 +688 -441 +688 -1558 +688 -441 +688 -1558 +688 -1558 +688 -441 +688 -441 +688 -441 +688 -1558 +688 -441 +688 -1558 +669 -22856
"#
    );
}

#[test]
fn encode_rawir_test() {
    let mut cmd = Command::cargo_bin("cir").unwrap();

    let assert = cmd
        .args(&[
            "encode",
            "rawir",
            r#"1000
            200
            1000"#,
        ])
        .assert();

    let output = assert.get_output();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(stderr, "");

    assert_eq!(
        stdout,
        r#"rawir: +1000 -200 +1000 -125000
"#
    );

    let mut cmd = Command::cargo_bin("cir").unwrap();

    let assert = cmd
        .args(&[
            "encode",
            "rawir",
            "-f",
            "testdata/rawir/mode2",
            "345",
            "-g",
            "30000",
            "+123 40 124",
            "-g",
            "40000",
            "-f",
            "testdata/rawir/rawir",
        ])
        .assert();

    let output = assert.get_output();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(stderr, "");

    assert_eq!(
        stdout,
        r#"rawir: +1000 -700 +1200 -125000 +345 -30000 +123 -40 +124 -40000 +2000 -500 +2000 -40000
"#
    );
}

#[test]
fn encode_lircd_grundig_test() {
    let mut cmd = Command::cargo_bin("cir").unwrap();

    let assert = cmd
        .args(&[
            "encode",
            "lircd",
            "testdata/lircd_conf/grundig/RP75_LCD.lircd.conf",
            "-m",
            "grundig_rp75",
            "0",
        ])
        .assert();

    let output = assert.get_output();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(stderr, "");

    assert_eq!(
        stdout,
        r#"carrier: 38000Hz
rawir: +871 -2894 +1363 -2188 +1229 -2192 +1230 -1102 +637 -1098 +638 -1094 +637 -1098 +638 -1723 +638 -435 +638 -443 +638 -1735 +638 -444 +637 -1736 +638 -443 +638 -1735 +638 -57449
"#
    );
}
///
/// This tests needs a /dev/lirc0 rc-loopback device
#[test]
#[cfg(feature = "loopback-tests")]
fn transmit_test() {
    let mut cmd = Command::cargo_bin("cir").unwrap();

    let recv = cmd.args(&["receive", "--one-shot"]).assert();

    let mut cmd = Command::cargo_bin("cir").unwrap();

    let assert = cmd
        .args(&[
            "transmit",
            "irp",
            "-e=1",
            "-fF=12",
            "{40k,600}<1,-1|2,-1>(4,-1,F:8,^45m)+[F:0..255]",
        ])
        .assert();

    let output = assert.get_output();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(stdout, "");
    assert_eq!(stderr, "");

    let output = recv.get_output();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(stderr, "");
    assert_eq!(
        stdout,
        r#"carrier: 40000Hz
rawir: +2400 -600 +600 -600 +600 -600 +1200 -600 +1200 -600 +600 -600 +600 -600 +600 -600 +600 -31800 +2400 -600 +600 -600 +600 -600 +1200 -600 +1200 -600 +600 -600 +600 -600 +600 -600 +600 -31800
"#
    );
}
