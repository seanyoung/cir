fn main() {
    cc::Build::new()
        .file("src/config_file.c")
        .file("src/driver.c")
        .file("src/ir_remote.c")
        .file("src/lirc_log.c")
        .file("src/receive.c")
        .file("src/transmit.c")
        .warnings(false)
        .compile("liblirc.a");
}
