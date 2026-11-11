//! Dynamic mode links `libpxui` by name. It is found next to the build products (`target/<profile>`, where the library is
//! built and uplifted), or in `PXUI_LIB_DIR` for an installed copy; the static feature needs neither.
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=PXUI_LIB_DIR");
    if let Some(dir) = std::env::var_os("PXUI_LIB_DIR") {
        println!("cargo:rustc-link-search=native={}", PathBuf::from(dir).display());
    }
    // OUT_DIR is target/<profile>/build/<package>-<hash>/out
    if let Some(profile_dir) = std::env::var_os("OUT_DIR").map(PathBuf::from).and_then(|out| out.ancestors().nth(3).map(PathBuf::from)) {
        println!("cargo:rustc-link-search=native={}", profile_dir.display());
    }
}
