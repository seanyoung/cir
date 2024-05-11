use assert_cmd::Command;

#[test]
fn toggle_bit_mask() {
    let mut cmd = Command::cargo_bin("cir").unwrap();

    let assert = cmd
        .args([
            "decode", "keymap", "../testdata/lircd_conf/d-link/DSM-10.lircd.conf", "-q", "-r",
            "+9132 -4396 +664 -460 +664 -460 +664 -460 +664 -1592 +664 -460 +664 -460 +664 -460 +664 -460 +664 -460 +664 -1592 +664 -1592 +664 -460 +664 -460 +664 -1592 +664 -1592 +664 -1592 +664 -460 +664 -460 +664 -1592 +664 -460 +664 -1592 +664 -460 +664 -460 +664 -460 +664 -1592 +664 -1592 +664 -460 +664 -1592 +664 -460 +664 -1592 +664 -1592 +664 -1592 +671 -42232 +9128 -2143 +671 -96305"
        ])
        .assert();

    let output = assert.get_output();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(stderr, "");

    assert_eq!(
        stdout,
        r#"decoded: remote:DLink_DSM-10 code:KEY_1
"#
    );

    // FIXME: toggle_bit_mask in post data
}

#[test]
fn ignore_mask() {
    let mut cmd = Command::cargo_bin("cir").unwrap();

    let assert = cmd
        .args([
            "decode", "keymap", "../testdata/lircd_conf/apple/A1156.lircd.conf", "-q", "-r",
            "+9065 -4484 +574 -547 +574 -1668 +574 -1668 +574 -1668 +574 -547 +574 -1668 +574 -1668 +574 -1668 +574 -1668 +574 -1668 +574 -1668 +574 -547 +574 -547 +574 -547 +574 -547 +574 -1668 +574 -547 +574 -1668 +574 -1668 +574 -547 +574 -547 +574 -547 +574 -547 +574 -547 +574 -1668 +574 -1668 +574 -547 +574 -547 +574 -547 +574 -1668 +574 -547 +574 -1668 +567 -37600 +9031 -2242 +567 -37600"
        ])
        .assert();

    let output = assert.get_output();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(stderr, "");

    assert_eq!(
        stdout,
        r#"decoded: remote:Apple_A1156 code:KEY_PLAY
decoded: remote:Apple_A1156 code:KEY_PLAY
"#
    );

    let mut cmd = Command::cargo_bin("cir").unwrap();

    let assert = cmd
        .args([
            "decode", "keymap", "../testdata/lircd_conf/apple/A1156.lircd.conf", "-q", "-r",
            "+9065 -4484 +574 -547 +574 -1668 +574 -1668 +574 -1668 +574 -547 +574 -1668 +574 -1668 +574 -1668 +574 -1668 +574 -1668 +574 -1668 +574 -547 +574 -547 +574 -547 +574 -547 +574 -1668 +574 -547 +574 -1668 +574 -1668 +574 -547 +574 -547 +574 -547 +574 -547 +574 -547 +574 -547 +574 -547 +574 -1668 +574 -1668 +574 -1668 +574 -547 +574 -1668 +574 -547 +567 -37600 +9031 -2242 +567 -37600"
        ])
        .assert();

    let output = assert.get_output();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(stderr, "");

    assert_eq!(
        stdout,
        r#"decoded: remote:Apple_A1156 code:KEY_PLAY
decoded: remote:Apple_A1156 code:KEY_PLAY
"#
    );
}

#[test]
fn keymap() {
    let mut cmd = Command::cargo_bin("cir").unwrap();

    let assert = cmd
        .args([
            "decode", "keymap", "../testdata/rc_keymaps/sony.toml", "-v", "-r",
            "+2400 -600 +600 -600 +600 -600 +600 -600 +600 -600 +600 -600 +1200 -600 +1200 -600 +600 -600 +600 -600 +600 -600 +600 -600 +1200 -26400"
        ])
        .assert();

    let output = assert.get_output();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        stderr,
        r#"debug: decoding irp {40k,600}<1,-1|2,-1>(4,-1,CODE:7,CODE:5:16,^45m) [CODE:0..0x1fffff] for keymap Sony-RM-U305C
debug: decoding irp {40k,600}<1,-1|2,-1>(4,-1,CODE:7,CODE:8:16,^45m) [CODE:0..0xffffff] for keymap Sony-RM-U305C
debug: decoding irp {40k,600}<1,-1|2,-1>(4,-1,CODE:7,CODE:5:16,CODE:8:8,^45m) [CODE:0..0x1fffff] for keymap Sony-RM-U305C
debug: generated NFA for Sony-RM-U305C
debug: generated DFA for Sony-RM-U305C
debug: generated NFA for Sony-RM-U305C
debug: generated DFA for Sony-RM-U305C
debug: generated NFA for Sony-RM-U305C
debug: generated DFA for Sony-RM-U305C
info: decoding: +2400 -600 +600 -600 +600 -600 +600 -600 +600 -600 +600 -600 +1200 -600 +1200 -600 +600 -600 +600 -600 +600 -600 +600 -600 +1200 -26400
debug: variable CODE=1048672
debug: scancode 0x100060
"#
    );

    assert_eq!(
        stdout,
        r#"decoded: keymap:Sony-RM-U305C code:KEY_SONY-AV-SLEEP
"#
    );

    let mut cmd = Command::cargo_bin("cir").unwrap();

    let assert = cmd
        .args([
            "decode", "keymap", "../testdata/rc_keymaps/sony-12.toml", "-r",
            "+2400 -600 +1200 -600 +600 -600 +1200 -600 +600 -600 +1200 -600 +600 -600 +600 -600 +600 -600 +600 -600 +600 -600 +600 -600 +600 -26400"
        ])
        .assert();

    let output = assert.get_output();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        stderr,
        r#"info: decoding: +2400 -600 +1200 -600 +600 -600 +1200 -600 +600 -600 +1200 -600 +600 -600 +600 -600 +600 -600 +600 -600 +600 -600 +600 -600 +600 -26400
"#
    );

    assert_eq!(
        stdout,
        r#"decoded: keymap:Sony-RM-U305C code:KEY_SONY-AV-AV-I/O
"#
    );
    let mut cmd = Command::cargo_bin("cir").unwrap();

    let assert = cmd
        .args([
            "decode", "keymap", "../testdata/rc_keymaps/dish_network.toml", "-q", "-r",
            "+525 -6045 +440 -2780 +440 -2780 +440 -2780 +440 -2780 +440 -1645 +440 -2780 +440 -2780 +440 -2780 +440 -2780 +440 -2780 +440 -2780 +440 -2780 +440 -2780 +440 -2780 +440 -2780 +440 -2780 +450 -40000"
        ])
        .assert();

    let output = assert.get_output();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(stderr, "");

    assert_eq!(
        stdout,
        r#"decoded: keymap:Dish Network code:KEY_POWER
"#
    );

    let mut cmd = Command::cargo_bin("cir").unwrap();

    let assert = cmd
        .args([
            "decode", "keymap", "../testdata/rc_keymaps/rc6_mce.toml", "-q", "-r",
            "+2664 -888 +444 -444 +444 -444 +444 -888 +444 -888 +1332 -888 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +444 -444 +888 -444 +444 -444 +444 -444 +444 -888 +444 -444 +444 -444 +444 -444 +444 -444 +888 -888 +444 -444 +444 -444 +888 -888 +888 -444 +444 -444 +444 -888 +444 -444 +444 -67704"
        ])
        .assert();

    let output = assert.get_output();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(stderr, "");

    assert_eq!(
        stdout,
        r#"decoded: keymap:rc6_mce code:KEY_GREEN
"#
    );

    let mut cmd = Command::cargo_bin("cir").unwrap();

    let assert = cmd
        .args([
            "decode", "keymap", "../testdata/rc_keymaps/RM-786.toml", "-r",
            "+2465 -569 +620 -582 +618 -584 +1242 -581 +618 -585 +620 -583 +620 -585 +1242 -607 +622 -575 +1243 -584 +1243 -578 +621 -579 +619 -20000"
        ])
        .assert();

    let output = assert.get_output();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(stderr, "info: decoding: +2465 -569 +620 -582 +618 -584 +1242 -581 +618 -585 +620 -583 +620 -585 +1242 -607 +622 -575 +1243 -584 +1243 -578 +621 -579 +619 -20000\n");

    assert_eq!(
        stdout,
        r#"decoded: keymap:HSVP code:KEY_AUXMUTE
"#
    );
}
