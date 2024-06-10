fn main() {
    cc::Build::new()
        .file("src/ir-imon-decoder.c")
        .file("src/ir-jvc-decoder.c")
        .file("src/ir-nec-decoder.c")
        .file("src/ir-rc5-decoder.c")
        .file("src/ir-rc6-decoder.c")
        .file("src/ir-rcmm-decoder.c")
        .file("src/ir-sanyo-decoder.c")
        .file("src/ir-sharp-decoder.c")
        .file("src/ir-sony-decoder.c")
        .file("src/ir-xmp-decoder.c")
        .file("src/rc-ir-raw.c")
        .warnings(false)
        .compile("libcodec.a");
}
