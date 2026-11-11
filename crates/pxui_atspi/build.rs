//! Compiles the libdbus-1 shim and links libdbus-1 on Linux. Nothing on other targets.
//! Headers and libraries through pkg-config, else the Debian/Ubuntu multiarch layout.
fn main() {
    println!("cargo:rerun-if-changed=src/shim.c");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("linux") {
        return;
    }
    cc::Build::new().file("src/shim.c").includes(dbus_includes()).warnings(true).compile("pxui_atspi_shim");
    link_search(&["dbus-1"]);
    println!("cargo:rustc-link-lib=dbus-1");
}

/// `pkg-config`'s flags with `prefix` (`-I`, `-L`) for `libs`; `None` without pkg-config or the packages.
fn pkg_config(libs: &[&str], flag: &str, prefix: &str) -> Option<Vec<String>> {
    let out = std::process::Command::new("pkg-config").arg(flag).args(libs).output().ok().filter(|o| o.status.success())?;
    Some(String::from_utf8_lossy(&out.stdout).split_whitespace().filter_map(|f| f.strip_prefix(prefix)).map(str::to_owned).collect())
}

/// libdbus-1's include directories: pkg-config's, else the Debian/Ubuntu multiarch layout.
fn dbus_includes() -> Vec<String> {
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    pkg_config(&["dbus-1"], "--cflags-only-I", "-I").filter(|d| !d.is_empty())
        .unwrap_or_else(|| vec!["/usr/include/dbus-1.0".to_owned(), format!("/usr/lib/{arch}-linux-gnu/dbus-1.0/include")])
}

/// Library directories pkg-config names for `libs` (none: the linker's default paths).
fn link_search(libs: &[&str]) {
    for dir in pkg_config(libs, "--libs-only-L", "-L").unwrap_or_default() {
        println!("cargo:rustc-link-search=native={dir}");
    }
}
