fn main() {
    println!("cargo:rerun-if-env-changed=PXUI_LIB_DIR");
    if let Some(dir) = std::env::var_os("PXUI_LIB_DIR") {
        println!("cargo:rustc-link-search=native={}", std::path::PathBuf::from(dir).display());
    }
}
