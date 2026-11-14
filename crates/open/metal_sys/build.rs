use std::{env, fs, path::PathBuf};
mod build_config;
fn build_failure(message: impl std::fmt::Display) -> ! { panic!("{message}") }
fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let abi = env::var("CARGO_CFG_TARGET_ABI").unwrap_or_default();
    if let Some((sdk_name, runtime_name)) = build_config::apple_platform(&os, &abi) {
        let sdk = std::process::Command::new("/usr/bin/xcrun")
            .args(["--sdk", sdk_name, "--show-sdk-version"])
            .output().expect("xcrun could not report the Apple SDK version");
        assert!(sdk.status.success(), "xcrun could not report the {sdk_name} SDK version: {}", String::from_utf8_lossy(&sdk.stderr));
        build_config::check_sdk_version(std::str::from_utf8(&sdk.stdout).expect("non-UTF-8 Apple SDK version")).unwrap_or_else(|message| build_failure(message));
        println!("cargo:rerun-if-env-changed=RAYS_METAL_SANITIZERS");
        let sanitizers = build_config::parse_sanitizers(&env::var("RAYS_METAL_SANITIZERS").unwrap_or_default()).unwrap_or_else(|message| build_failure(message));
        let native_dir = manifest.join("src/native");
        println!("cargo:rerun-if-changed={}", native_dir.display());
        let mut build = cc::Build::new();
        build.cpp(true).include(&native_dir)
            .flag("-std=c++17").flag("-fobjc-arc").flag("-fblocks").warnings(true);
        if os == "ios" { build.flag("-Werror=unguarded-availability-new"); }
        build.flags(build_config::compile_flags(&sanitizers));
        build.flags(build_config::profile_compile_flags(&env::var("PROFILE").unwrap_or_default()));
        // One translation unit including every .mm: the Foundation and Metal headers dominate each
        // file's compile, so parsing them once is about 5x faster than six separate compiles.
        let mut sources: Vec<PathBuf> = fs::read_dir(&native_dir).into_iter().flatten().flatten().map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "mm")).collect();
        sources.sort();
        let mut unity = String::new();
        for path in &sources {
            println!("cargo:rerun-if-changed={}", path.display());
            unity.push_str(&format!("#include \"{}\"\n", path.display()));
        }
        let unity_path = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("metal_unity.mm");
        fs::write(&unity_path, unity).unwrap_or_else(|error| build_failure(format!("{}: {error}", unity_path.display())));
        build.file(unity_path);
        // Rust uses -nodefaultlibs; availability guards still need Clang's platform-version runtime.
        let runtime = build.get_compiler().to_command().arg(format!("-print-file-name=libclang_rt.{runtime_name}.a"))
            .output().expect("Clang could not locate its Apple runtime");
        assert!(runtime.status.success(), "Clang could not locate its {runtime_name} runtime");
        let runtime = PathBuf::from(std::str::from_utf8(&runtime.stdout).expect("non-UTF-8 Clang runtime path").trim());
        assert!(runtime.is_absolute() && runtime.is_file(), "Clang {runtime_name} runtime is missing: {}", runtime.display());
        let target = env::var("TARGET").unwrap();
        let arch = if target.starts_with("aarch64-") { "arm64" } else { target.split('-').next().unwrap() };
        let architectures = std::process::Command::new("/usr/bin/xcrun")
            .args(["lipo", "-archs"]).arg(&runtime).output().expect("lipo could not inspect Clang runtime");
        assert!(architectures.status.success(), "lipo could not inspect Clang runtime");
        let runtime_directory = if architectures.stdout.split(|byte| byte.is_ascii_whitespace()).filter(|word| !word.is_empty()).count() > 1 {
            // Rust bundles native static archives into rlibs; it cannot bundle Apple's fat iOS runtime.
            let directory = PathBuf::from(env::var_os("OUT_DIR").unwrap());
            let status = std::process::Command::new("/usr/bin/xcrun")
                .args(["lipo", "-thin", arch]).arg(&runtime)
                .arg("-output").arg(directory.join(format!("libclang_rt.{runtime_name}.a")))
                .status().expect("lipo could not thin Clang runtime");
            assert!(status.success(), "Clang runtime has no {arch} slice");
            directory
        } else { runtime.parent().expect("Clang runtime directory").to_path_buf() };
        println!("cargo:rustc-link-search=native={}", runtime_directory.display());
        println!("cargo:rustc-link-lib=static=clang_rt.{runtime_name}");
        build.compile("metal_generated");
        for framework in build_config::FRAMEWORKS {
            println!("cargo:rustc-link-lib=framework={framework}");
        }
        for flag in build_config::link_flags(&sanitizers) {
            println!("cargo:rustc-link-arg={flag}");
        }
    }
}
