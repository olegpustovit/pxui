fn main() {
    if cfg!(target_os = "linux") {
        println!("cargo:rustc-cdylib-link-arg=-Wl,-soname,libpxui.so.2");
    } else if cfg!(target_os = "macos") {
        println!("cargo:rustc-cdylib-link-arg=-Wl,-install_name,@rpath/libpxui.2.dylib");
        // the major version, 2.0.0: an app linked against another major does not load this library (DX7.10)
        println!("cargo:rustc-cdylib-link-arg=-Wl,-compatibility_version,2.0.0");
        println!("cargo:rustc-cdylib-link-arg=-Wl,-current_version,2.0.0");
    }
}
