use assert_cmd::Command;
use pretty_assertions::assert_eq;

#[test]
fn encode_test() {
    let mut cmd = Command::cargo_bin("cir").unwrap();

    let assert = cmd
        .args([
            "transmit",
            "--dry-run",
            "-aF=12",
            "--irp",
            "{40k,600}<1,-1|2,-1>(4,-1,F:8,^45m)+[F:0..255]",
            "--repeats",
            "1",
            "--irp-protocols",
            "../IrpTransmogrifier/src/main/resources/IrpProtocols.xml",
        ])
        .assert();

    let output = assert.get_output();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(stdout, "");

    assert_eq!(
        stderr,
        r#"info: carrier: 40000Hz
info: rawir: +2400 -600 +600 -600 +600 -600 +1200 -600 +1200 -600 +600 -600 +600 -600 +600 -600 +600 -31800 +2400 -600 +600 -600 +600 -600 +1200 -600 +1200 -600 +600 -600 +600 -600 +600 -600 +600 -31800
"#
    );
}

#[test]
fn encode_lircd_raw_test() {
    let mut cmd = Command::cargo_bin("cir").unwrap();

    let assert = cmd
        .args([
            "transmit",
            "--dry-run",
            "--keymap",
            "../testdata/lircd_conf/pace/DC420N.lircd.conf",
            "--keycode",
            "1",
        ])
        .assert();

    let output = assert.get_output();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(stdout, "");

    assert_eq!(
        stderr,
        r#"info: carrier: 36000Hz
info: duty cycle: 50%
info: rawir: +2664 -888 +444 -444 +444 -444 +444 -888 +444 -888 +888 -444 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +888 -123496
"#
    );
}

#[test]
fn encode_irp_test() {
    let mut cmd = Command::cargo_bin("cir").unwrap();

    let assert = cmd
        .args([
            "transmit",
            "--dry-run",
            "--irp=Blaupunkt",
            "--irp-protocols",
            "../IrpTransmogrifier/src/main/resources/IrpProtocols.xml",
            "-aF=0,D=1",
        ])
        .assert();

    let output = assert.get_output();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(stdout, "");

    assert_eq!(
        stderr,
        r#"info: carrier: 30300Hz
info: rawir: +512 -2560 +512 -512 +512 -512 +512 -512 +512 -512 +512 -512 +512 -512 +512 -512 +512 -512 +512 -512 +512 -23040 +512 -2560 +512 -1024 +512 -512 +512 -512 +512 -512 +512 -512 +512 -512 +1024 -1024 +512 -512 +512 -120832 +512 -2560 +512 -512 +512 -512 +512 -512 +512 -512 +512 -512 +512 -512 +512 -512 +512 -512 +512 -512 +512 -23040
"#
    );
}

#[test]
fn encode_lircd_aiwa_test() {
    let mut cmd = Command::cargo_bin("cir").unwrap();

    let assert = cmd
        .args([
            "transmit",
            "--dry-run",
            "--keymap",
            "../testdata/lircd_conf/aiwa/RC-5VP05.lircd.conf",
            "-K",
            "AUTO",
        ])
        .assert();

    let output = assert.get_output();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(stdout, "");

    assert_eq!(
        stderr,
        r#"info: carrier: 38000Hz
info: rawir: +9137 -4360 +688 -1558 +688 -1558 +688 -1558 +688 -1558 +688 -1558 +688 -1558 +688 -1558 +688 -441 +688 -441 +688 -441 +688 -441 +688 -441 +688 -441 +688 -441 +688 -441 +688 -441 +688 -441 +688 -441 +688 -441 +688 -441 +688 -1558 +688 -1558 +688 -1558 +688 -1558 +688 -1558 +688 -1558 +688 -441 +688 -441 +688 -1558 +688 -1558 +688 -1558 +688 -441 +688 -1558 +688 -441 +688 -1558 +688 -1558 +688 -441 +688 -441 +688 -441 +688 -1558 +688 -441 +688 -1558 +669 -22856
"#
    );
}

#[test]
fn encode_rawir_test() {
    let mut cmd = Command::cargo_bin("cir").unwrap();

    let assert = cmd
        .args([
            "transmit",
            "--dry-run",
            "--raw",
            r#"1000
            200
            1000"#,
        ])
        .assert();

    let output = assert.get_output();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(stdout, "");

    assert_eq!(
        stderr,
        r#"info: rawir: +1000 -200 +1000 -125000
"#
    );

    let mut cmd = Command::cargo_bin("cir").unwrap();

    let assert = cmd
        .args([
            "transmit",
            "--dry-run",
            "-f",
            "../testdata/rawir/mode2",
            "-r",
            "345",
            "-g",
            "30000",
            "-r",
            "+123 40 124",
            "-g",
            "40000",
            "-f",
            "../testdata/rawir/rawir",
        ])
        .assert();

    let output = assert.get_output();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(stdout, "");

    assert_eq!(
        stderr,
        r#"info: rawir: +1000 -700 +1200 -125000 +345 -30000 +123 -40 +124 -40000 +2000 -500 +2000 -40000
"#
    );
}

#[test]
fn encode_lircd_grundig_test() {
    let mut cmd = Command::cargo_bin("cir").unwrap();

    let assert = cmd
        .args([
            "transmit",
            "--dry-run",
            "--keymap",
            "../testdata/lircd_conf/grundig/RP75_LCD.lircd.conf",
            "-m",
            "grundig_rp75",
            "--keycode",
            "0",
        ])
        .assert();

    let output = assert.get_output();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(stdout, "");

    assert_eq!(
        stderr,
        r#"info: carrier: 38000Hz
info: rawir: +871 -2894 +1363 -2188 +1229 -2192 +1230 -1102 +637 -1098 +638 -1094 +637 -1098 +638 -1723 +638 -435 +638 -443 +638 -1735 +638 -444 +637 -1736 +638 -443 +638 -1735 +638 -57449
"#
    );
}

#[test]
fn empty_lircd_conf() {
    let mut cmd = Command::cargo_bin("cir").unwrap();

    let assert = cmd
        .args([
            "transmit",
            "--dry-run",
            "--keymap",
            "../testdata/lircd_conf/empty",
            "--list-codes",
        ])
        .assert();

    let output = assert.get_output();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(stdout, "");

    assert_eq!(
        stderr,
        r#"error: ../testdata/lircd_conf/empty: parse error at error at 1:3: expected "table"
"#
    );
}

#[test]
fn keymaps() {
    let mut cmd = Command::cargo_bin("cir").unwrap();

    let assert = cmd
        .args([
            "transmit",
            "--dry-run",
            "-v",
            "--keymap",
            "../testdata/rc_keymaps/RM-687C.toml",
            "--keycode",
            "KEY_0",
        ])
        .assert();

    let output = assert.get_output();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(stdout, "");

    assert_eq!(
        stderr,
        r#"debug: using irp for encoding: {msb}<565,-637|1166,-637>(2369,-637,CODE:12,-40m) [CODE:0..4095]
info: carrier: 38000Hz
info: rawir: +2369 -637 +1166 -637 +565 -637 +565 -637 +1166 -637 +565 -637 +565 -637 +565 -637 +1166 -637 +565 -637 +565 -637 +565 -637 +565 -40637
"#
    );

    let mut cmd = Command::cargo_bin("cir").unwrap();

    let assert = cmd
        .args([
            "transmit",
            "--dry-run",
            "-k",
            "../testdata/rc_keymaps/RM-786.toml",
            "-K",
            "KEY_CABLEFWD",
        ])
        .assert();

    let output = assert.get_output();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(stdout, "");

    assert_eq!(
        stderr,
        r#"info: rawir: +2437 -553 +618 -569 +619 -576 +1239 -573 +618 -572 +1239 -578 +1238 -580 +616 -597 +619 -570 +618 -564 +618 -577 +618 -573 +1242 -20000
"#
    );

    let mut cmd = Command::cargo_bin("cir").unwrap();

    let assert = cmd
        .args([
            "transmit",
            "--dry-run",
            "--keymap",
            "foo.toml",
            "--keycode",
            "KEY_CABLEFWD",
        ])
        .assert();

    let output = assert.get_output();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(stdout, "");

    assert_eq!(
        stderr,
        r#"error: foo.toml: No such file or directory (os error 2)
"#
    );

    let mut cmd = Command::cargo_bin("cir").unwrap();

    let assert = cmd
        .args([
            "transmit",
            "--dry-run",
            "--keymap",
            "Cargo.toml",
            "--keycode",
            "KEY_CABLEFWD",
        ])
        .assert();

    let output = assert.get_output();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(stdout, "");

    assert_eq!(
        stderr,
        r#"error: Cargo.toml: missing top level protocols array
"#
    );

    let mut cmd = Command::cargo_bin("cir").unwrap();

    let assert = cmd
        .args([
            "transmit",
            "--dry-run",
            "--keymap",
            "../testdata/rc_keymaps/rc6_mce.toml",
            "--keycode",
            "KEY_ENTER",
            "-v",
        ])
        .assert();

    let output = assert.get_output();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(stdout, "");

    assert_eq!(
        stderr,
        r#"debug: using irp for encoding: {36k,444,msb}<-1,1|1,-1>(6,-2,1:1,6:3,-2,2,CODE:16:16,T:1,CODE:15,MCE=(CODE>>16)==0x800f||(CODE>>16)==0x8034||(CODE>>16)==0x8046,^105m)+{MCE=1}[CODE:0..0xffffffff,T@:0..1=0]
info: carrier: 36000Hz
info: rawir: +2664 -888 +444 -444 +444 -444 +444 -888 +444 -888 +1332 -888 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +888 -444 +444 -444 +444 -444 +444 -888 +444 -444 +444 -444 +444 -444 +444 -444 +888 -888 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +888 -888 +888 -444 +444 -68148
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
