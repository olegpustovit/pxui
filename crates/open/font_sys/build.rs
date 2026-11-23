fn main() {
    println!("cargo:rerun-if-changed=src/mask.c");
    if std::env::var("CARGO_CFG_TARGET_VENDOR").as_deref() == Ok("apple") {
        cc::Build::new().file("src/mask.c").flag("-fvisibility=hidden").compile("pxui_font_mask");
    }
}
