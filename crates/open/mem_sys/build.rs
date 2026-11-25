fn main() {
    let mut native = cc::Build::new();
    native.file("src/mem_sys.c");
    let apple = std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|target| target == "macos" || target == "ios");
    if apple {
        native.file("src/profile.c");
    }
    // `trace` is a no-op off Apple: the malloc_logger hook is Darwin-only.
    if apple && std::env::var("CARGO_FEATURE_TRACE").is_ok() {
        native.file("src/trace.c");
        println!("cargo:rerun-if-changed=src/trace.c");
    }
    native.warnings(true).compile("mem_sys");
    println!("cargo:rerun-if-changed=src/profile.c");
    println!("cargo:rerun-if-changed=src/mem_sys.c");
    if std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|target| target == "windows") {
        println!("cargo:rustc-link-lib=psapi");
    }
}
