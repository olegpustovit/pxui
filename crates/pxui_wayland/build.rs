//! Compiles the Wayland shim, the portal's D-Bus shim and the checked-in wayland-scanner protocol code (no scanner at
//! build time) and links libwayland-client, libxkbcommon and libdbus-1. Nothing on other targets.
//! The protocol files come from wayland-protocols 1.45 (`wayland-scanner client-header|private-code <xml>`; only
//! tablet-v2's code, which cursor-shape-v1 references), with trailing blank lines stripped.
//! Headers and libraries through pkg-config, else the default search paths (Debian/Ubuntu layout).
fn main() {
    println!("cargo:rerun-if-changed=src/shim.c");
    println!("cargo:rerun-if-changed=src/portal.c");
    println!("cargo:rerun-if-changed=protocol");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("linux") {
        return;
    }
    let protocols = ["xdg-shell", "xdg-decoration-unstable-v1", "viewporter", "fractional-scale-v1", "cursor-shape-v1", "tablet-v2", "text-input-unstable-v3", "relative-pointer-unstable-v1", "pointer-constraints-unstable-v1", "xdg-foreign-unstable-v2"];
    let mut build = cc::Build::new();
    build.file("src/shim.c").includes(pkg_config(&["wayland-client", "xkbcommon"], "--cflags-only-I", "-I").unwrap_or_default()).warnings(true);
    for protocol in protocols {
        build.file(format!("protocol/{protocol}.c"));
    }
    build.compile("pxui_wayland_shim");
    // The file dialog's D-Bus (the XDG desktop portal), its own shim beside pxui_atspi's.
    cc::Build::new().file("src/portal.c").includes(dbus_includes()).warnings(true).compile("pxui_wayland_portal");
    link_search(&["wayland-client", "xkbcommon", "dbus-1"]);
    println!("cargo:rustc-link-lib=wayland-client");
    println!("cargo:rustc-link-lib=xkbcommon");
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
