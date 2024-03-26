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
}
