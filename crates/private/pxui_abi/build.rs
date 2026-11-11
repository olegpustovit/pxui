fn main() {
    if cfg!(target_os = "linux") {
        println!("cargo:rustc-cdylib-link-arg=-Wl,-soname,libpxui.so.1");
    } else if cfg!(target_os = "macos") {
        println!("cargo:rustc-cdylib-link-arg=-Wl,-install_name,@rpath/libpxui.1.dylib");
    }
}
