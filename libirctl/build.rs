fn main() {
    cc::Build::new()
        .file("src/bpf_encoder.c")
        .file("src/keymap.c")
        .file("src/ir-encode.c")
        .file("src/toml.c")
        .warnings(false)
        .compile("libirctl.a");
}
