fn main() {
    println!("cargo:rerun-if-changed=src/selection.m");
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if os == "macos" || os == "ios" {
        let mut native = cc::Build::new();
        native
            .file("src/selection.m")
            .flag("-fobjc-arc")
            .warnings(true)
            .flag("-Werror");
        if std::env::var_os("CARGO_FEATURE_TEST_UTIL").is_some() {
            native.define("PXUI_FILE_TEST_UTIL", None);
        }
        native.compile("pxui_file_selection");
        println!("cargo:rustc-link-lib=framework=Foundation");
    }
}
