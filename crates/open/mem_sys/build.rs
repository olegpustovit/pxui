fn main() {
    let mut native = cc::Build::new();
    native.file("src/mem_sys.c");
    if std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|target| target == "macos" || target == "ios")
    {
        native.file("src/profile.c");
    }
    native.warnings(true).compile("mem_sys");
    println!("cargo:rerun-if-changed=src/profile.c");
    println!("cargo:rerun-if-changed=src/mem_sys.c");
    if std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|target| target == "windows") {
        println!("cargo:rustc-link-lib=psapi");
    }
}
