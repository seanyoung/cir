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
