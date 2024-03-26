use assert_cmd::Command;

#[test]
fn toggle_bit_mask() {
    let mut cmd = Command::cargo_bin("cir").unwrap();

    let assert = cmd
        .args([
            "decode", "lircd", "testdata/lircd_conf/d-link/DSM-10.lircd.conf", "-q", "-r",
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
decoded: remote:DLink_DSM-10 code:KEY_1
"#
    );

    // FIXME: toggle_bit_mask in post data
}

#[test]
fn ignore_mask() {
    let mut cmd = Command::cargo_bin("cir").unwrap();

    let assert = cmd
        .args([
            "decode", "lircd", "testdata/lircd_conf/apple/A1156.lircd.conf", "-q", "-r",
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

    // FIXME: post data is not ignored
    //     let mut cmd = Command::cargo_bin("cir").unwrap();

    //     let assert = cmd
    //     .args([
    //         "decode", "lircd", "testdata/lircd_conf/apple/A1156.lircd.conf", "-q", "-r",
    //         "+9065 -4484 +574 -547 +574 -1668 +574 -1668 +574 -1668 +574 -547 +574 -1668 +574 -1668 +574 -1668 +574 -1668 +574 -1668 +574 -1668 +574 -547 +574 -547 +574 -547 +574 -547 +574 -1668 +574 -547 +574 -1668 +574 -1668 +574 -547 +574 -547 +574 -547 +574 -547 +574 -547 +574 -547 +574 -547 +574 -1668 +574 -1668 +574 -1668 +574 -547 +574 -1668 +574 -547 +567 -37600 +9031 -2242 +567 -37600"
    //     ])
    //     .assert();

    //     let output = assert.get_output();

    //     let stdout = String::from_utf8_lossy(&output.stdout);
    //     let stderr = String::from_utf8_lossy(&output.stderr);

    //     assert_eq!(stderr, "");

    //     assert_eq!(
    //         stdout,
    //         r#"decoded: remote:Apple_A1156 code:KEY_PLAY
    // decoded: remote:Apple_A1156 code:KEY_PLAY
    // "#
    //     );
}
