//! PXUI dependency policy: the rules about this repository's own crates. The engine in main.rs is the
//! one Rays uses (tools/gate there); only this file differs.
use std::collections::{HashMap, HashSet};

pub const TIERS: &[&str] = &["public", "private", "open"];

pub fn tier_allows(from_tier: &str, from_crate: &str, to_tier: &str, to_crate: &str, optional: bool, static_feature: bool) -> bool {
    match from_tier {
        "public" => to_tier == "public" || (from_crate == "pxui" && to_tier == "private" && to_crate == "pxui_abi" && optional && static_feature),
        "private" => to_tier == "private" || to_tier == "open" || (to_tier == "public" && to_crate == "pxui_types"),
        "open" => to_tier == "open",
        _ => false,
    }
}

/// Helper leaves any product crate may name.
pub const HELPER_LEAVES: [&str; 1] = ["cfmt"];
/// Leaves a crate may name directly when its allowed dependencies reach them.
pub const OCAML_LEAVES: [&str; 1] = ["input"];

/// Direct production edges. Sys crates own their native ABI bridges.
pub fn direct_allowlist() -> HashMap<&'static str, &'static [&'static str]> {
    HashMap::from([
        ("cfmt", &[][..]),
        ("native_layer_token_sys", &[][..]),
        ("mem_sys", &[][..]),
        ("input", &[][..]),
        ("sdl3_sys", &["native_layer_token_sys"][..]),
        ("sdl3", &["sdl3_sys", "native_layer_token_sys"][..]),
        ("metal_sys", &["ogpu_core"][..]),
        ("vulkan_sys", &["ogpu_core"][..]),
        ("ogpu_core", &["native_layer_token_sys"][..]),
        ("ogpu_mock", &["ogpu_core"][..]),
        ("ogpu_metal", &["ogpu_core", "metal_sys", "native_layer_token_sys"][..]),
        ("ogpu_vulkan", &["ogpu_core", "vulkan_sys", "native_layer_token_sys"][..]),
        // Plain data in (input), a draw list out.
        ("pxui_core", &["input", "cfmt", "pxui_text", "pxui_types"][..]),
        ("pxui_text", &[][..]),
        // The UI painter: the portable GPU API and PXUI's draw list.
        ("pxui_render", &["ogpu_core", "pxui_core", "pxui_types"][..]),
        // The window hosts are leaves: SDL, the input types and the native layer token.
        ("pxui_sdl", &["sdl3", "input", "native_layer_token_sys"][..]),
        // pxui_wayland links libwayland-client, libxkbcommon and (file dialogs through the desktop portal) libdbus-1
        // through its own C shims, like pxui_atspi.
        ("pxui_wayland", &["input", "native_layer_token_sys"][..]),
        // The Linux AT-SPI2 bridge: libdbus-1 through its own C shim, over pxui's accessibility tree.
        ("pxui_atspi", &["pxui_core", "pxui_types"][..]),
        ("pxui_abi", &[][..]),
        ("pxui_dylib", &[][..]),
        ("pxui_types", &[][..]),
        // The public wrapper names only the public types; the optional static link is a package-level edge below.
        ("pxui", &["pxui_types"][..]),
        // The kit gallery app: the engine, the painter and the platform's window host and GPU backend.
        // The cross-ABI checks: a tool, so it names the private crates the public ones must not.
        // The direct-engine build of the gallery pages for the A7 bench: a tool, so it names the private engine.
        ("abi_bench", &["pxui_core", "pxui_types"][..]),
        ("abi_equal", &["input", "mem_sys", "pxui", "pxui_core", "pxui_types"][..]),
        ("pxui_gallery", &["pxui", "input", "mem_sys", "ogpu_core", "pxui_types", "ogpu_metal", "ogpu_vulkan", "pxui_sdl", "pxui_wayland"][..]),
    ])
}

/// Package-specific dependency policy for packages whose library crate names collide.
pub fn package_allowlist() -> HashMap<&'static str, &'static [&'static str]> {
    HashMap::from([
        // The public wrapper's optional static link points at the cdylib whose crate name is `pxui`.
        ("pxui", &["pxui", "pxui_types"][..]),
        // libpxui: the exports of pxui_abi (crate name `pxui`) as one C library
        ("pxui_dylib", &["pxui"][..]),
        ("pxui_abi", &["input", "mem_sys", "pxui_core", "pxui_render", "pxui_types", "pxui_text", "pxui_atspi", "ogpu_core", "ogpu_metal", "ogpu_vulkan"][..]),
    ])
}

pub fn local_workspace_only() -> HashSet<&'static str> {
    HashSet::new()
}

pub fn forbidden_reaches() -> HashMap<String, HashSet<String>> {
    let mut rules: HashMap<String, HashSet<String>> = HashMap::new();
    let mut forbid = |library: &str, names: &[&str]| {
        rules.entry(library.to_owned()).or_default().extend(names.iter().map(|name| (*name).to_owned()));
    };
    // GPU backends know nothing about windows, the UI or each other.
    for library in ["ogpu_core", "ogpu_mock", "metal_sys", "vulkan_sys", "ogpu_metal", "ogpu_vulkan"] {
        forbid(library, &["sdl3", "sdl3_sys", "pxui_core", "pxui_text", "pxui_render", "pxui_sdl", "pxui_wayland", "pxui_atspi"]);
    }
    forbid("pxui_sdl", &["ogpu_core"]);
    forbid("pxui_wayland", &["sdl3", "pxui_sdl", "ogpu_core", "pxui_core", "pxui_render"]);
    forbid("pxui_atspi", &["sdl3", "pxui_sdl", "pxui_wayland", "ogpu_core", "pxui_render"]);
    forbid("pxui_core", &["sdl3", "ogpu_core"]);
    forbid("pxui_render", &["sdl3", "metal_sys", "vulkan_sys", "ogpu_metal", "ogpu_vulkan", "ogpu_mock", "mem_sys"]);
    rules
}

/// External crates a package may name (package, dependency); `cc` is allowed everywhere as a build dependency.
pub const EXTERNAL_CRATES: &[(&str, &str)] = &[("pxui_text", "cosmic-text")];

/// Direct edges that are forbidden outright (from, to, reason).
pub const NO_DIRECT: &[(&str, &str, &str)] = &[];

/// Crates whose tests (dev dependencies) may not reach the window host or SDL either.
pub const TESTS_INCLUDED: &[&str] = &["pxui_core", "pxui_text", "pxui_render"];
pub const TESTS_INCLUDED_FORBIDDEN: &[&str] = &["sdl3", "sdl3_sys", "pxui_sdl"];

/// No registry checks here.
pub const DOCS_CHECKS: &[(&str, &str, Option<&str>)] = &[];

/// Crates besides `*_sys` that may use `unsafe`.
pub const UNSAFE_CRATES: &[&str] = &["pxui_wayland", "pxui_atspi"]; // own their C shims (Wayland, libdbus-1) and the extern blocks over them, like *_sys crates

/// Files besides those of `UNSAFE_CRATES` that may use `unsafe`, each with its reason.
pub const UNSAFE_FILES: &[(&str, &str)] = &[
    ("crates/private/pxui_abi/src/lib.rs", "the stable ABI requires an unmangled C symbol export"),
    ("crates/private/pxui_abi/src/macros.rs", "the export macro declares the unmangled C symbols and wraps each body against unwinding"),
    ("crates/private/pxui_abi/src/state.rs", "the Ui handle's shared state and the helpers that decode caller-owned C pointers"),
    ("crates/private/pxui_abi/src/ui.rs", "creates and frees the Ui and decodes caller-owned frame, theme and event buffers at the C ABI boundary"),
    ("crates/private/pxui_abi/src/safety.rs", "tests the exports through raw C handles"),
    ("crates/private/pxui_core/src/arena.rs", "frame bump arena: canvas closures are written into raw bytes and called through type-erased records"),
    ("crates/private/pxui_render/src/lib.rs", "writes instance bytes into the host-mapped upload ring and views instance structs as bytes"),
    ("crates/private/pxui_abi/src/a11y.rs", "validates and translates caller-owned C ABI pointers at the PXUI FFI boundary"),
    ("crates/private/pxui_abi/src/a11y_state.rs", "validates and translates caller-owned C ABI pointers at the PXUI FFI boundary"),
    ("crates/private/pxui_abi/src/collections.rs", "validates borrowed collection spans and invokes C ABI callbacks"),
    ("crates/private/pxui_abi/src/composite.rs", "validates callback contexts and marshals composite UI data at the C ABI boundary"),
    ("crates/private/pxui_abi/src/image.rs", "validates borrowed GPU image descriptors at the C ABI boundary"),
    ("crates/private/pxui_abi/src/interaction.rs", "validates caller-owned event and output buffers at the C ABI boundary"),
    ("crates/private/pxui_abi/src/kit.rs", "validates callback contexts and caller-owned buffers at the C ABI boundary"),
    ("crates/private/pxui_abi/src/node.rs", "validates C node descriptors and caller-owned strings at the C ABI boundary"),
    ("crates/private/pxui_abi/src/overlays.rs", "validates borrowed overlay strings and callback contexts at the C ABI boundary"),
    ("crates/private/pxui_abi/src/paint.rs", "validates opaque C paint handles and callback contexts at the C ABI boundary"),
    ("crates/private/pxui_abi/src/style.rs", "validates borrowed style descriptors at the C ABI boundary"),
    ("crates/private/pxui_abi/src/text.rs", "validates C text handles and callback contexts at the C ABI boundary"),
    ("crates/private/pxui_abi/src/text_inputs.rs", "validates caller-owned text and callback pointers at the C ABI boundary"),
    ("crates/private/pxui_abi/src/text_width.rs", "validates borrowed text descriptors at the C ABI boundary"),
    ("crates/private/pxui_abi/src/theme.rs", "validates borrowed theme descriptors at the C ABI boundary"),
    ("crates/private/pxui_abi/src/widgets.rs", "validates borrowed widget descriptors and callback contexts at the C ABI boundary"),
    ("crates/public/pxui/src/a11y.rs", "calls the private C ABI through validated safe wrapper methods"),
    ("crates/public/pxui/src/a11y_state.rs", "calls the private C ABI through validated safe wrapper methods"),
    ("crates/public/pxui/src/collections.rs", "calls the private C ABI and passes stack callback contexts with bounded lifetimes"),
    ("crates/public/pxui/src/composite.rs", "calls the private C ABI and contains callback panics before returning to C"),
    ("crates/public/pxui/src/image.rs", "calls the private C ABI with caller-owned GPU image descriptors"),
    ("crates/public/pxui/src/interaction.rs", "calls the private C ABI through safe wrapper methods"),
    ("crates/public/pxui/src/kit.rs", "calls the private C ABI and contains callback panics before returning to C"),
    ("crates/public/pxui/src/lib.rs", "calls the private C ABI through safe wrapper methods and owns opaque handles"),
    ("crates/public/pxui/src/node.rs", "calls the private C ABI through safe wrapper methods"),
    ("crates/public/pxui/src/overlays.rs", "calls the private C ABI and contains callback panics before returning to C"),
    ("crates/public/pxui/src/paint.rs", "calls the private C ABI and contains callback panics before returning to C"),
    ("crates/public/pxui/src/style.rs", "calls the private C ABI through safe wrapper methods"),
    ("crates/public/pxui/src/text.rs", "calls the private text C ABI through safe wrapper methods"),
    ("crates/public/pxui/src/text_inputs.rs", "calls the private C ABI and validates callback inputs"),
    ("crates/public/pxui/src/text_width.rs", "calls the private C ABI through safe wrapper methods"),
    ("crates/public/pxui/src/widgets.rs", "calls the private C ABI through safe wrapper methods"),
    ("crates/private/pxui_abi/src/ui_state.rs", "validates borrowed strings and caller-owned output pointers at the C ABI boundary"),
    ("crates/private/pxui_abi/src/box_ops.rs", "validates box descriptors, borrowed strings, callback contexts and caller-owned outputs at the C ABI boundary"),
    ("crates/public/pxui/src/macros.rs", "the library! macro declares the extern block that links libpxui"),
    ("crates/private/pxui_abi/src/kit_ext.rs", "validates widget descriptors, borrowed strings, callback contexts and caller-owned outputs at the C ABI boundary"),
    ("crates/public/pxui/src/kit_ext.rs", "calls the private C ABI, passes stack callback contexts and reads borrowed library strings"),
    ("crates/public/pxui/src/theme_scope.rs", "calls the private C ABI and owns the opaque theme handle"),
    ("crates/private/pxui_abi/src/renderer.rs", "adopts the app's native GPU handles, validates caller-owned descriptors and records into the app's command buffer at the C ABI boundary"),
    ("crates/public/pxui_types/src/shape.rs", "stores instances into the library's span of layer storage"),
    ("crates/public/pxui_types/src/layout.rs", "a grid holds its tracks as pointer and length so a node is the same bytes in C; only Grid::new makes one, from 'static slices"),
    ("crates/public/pxui/src/ui_state.rs", "calls the private C ABI through safe wrapper methods"),
    ("crates/public/pxui/src/box_ops.rs", "calls the private C ABI, boxes the hit closure behind its callback and reads borrowed library strings"),
    ("crates/open/ogpu_core/src/backend.rs", "validates and adopts raw native GPU handles at the explicit backend bridge"),
    ("crates/open/ogpu_metal/src/native/pipeline.rs", "calls Metal pipeline functions through the native backend wrapper"),
    ("crates/open/ogpu_metal/src/native/resource.rs", "calls Metal resource functions through the native backend wrapper"),
];

/// The public wrapper (`crates/public/pxui`) is safe Rust over a C ABI, and three shape rules that hold elsewhere cannot hold
/// there. It is one policy, not a list of lines:
/// - `catch_unwind`: an app closure that panics inside a library callback must not unwind through C; the wrapper catches it
///   in every trampoline and resumes it at the call that ran the callback.
/// - lifetime parameters on structs: a value that borrows library memory (a frame, a painter, a text service) is tied to its `Ui`
///   by a lifetime, which is how Rust says the library's pointer is valid only that long.
/// - generics: only the closure trampolines of a callback crossing the ABI. A type parameter is `F` (bare on the struct that holds
///   the closure, `F: FnMut(..)`/`Fn`/`FnOnce` on the function, or `F: ?Sized` over a `dyn` closure) or `R`, the closure's result.
///   Any other generic item there still fails.
pub fn wrapper_allows(path: &str, rule: &str, generics: &[&str]) -> bool {
    if !path.starts_with("crates/public/pxui/src/") { return false; }
    match rule {
        "catch_unwind" | "struct lifetime" => true,
        "generic" => !generics.is_empty() && closure_generics(generics),
        _ => false,
    }
}

// The tokens between `<` and `>`: lifetimes, `F`, or `F: Fn/FnMut/FnOnce(..)` separated by top-level commas.
fn closure_generics(tokens: &[&str]) -> bool {
    let mut depth = 0i32;
    let mut parameters: Vec<Vec<&str>> = vec![Vec::new()];
    for (at, token) in tokens.iter().enumerate() {
        match *token {
            "(" | "[" | "<" => depth += 1,
            ")" | "]" => depth -= 1,
            ">" if at == 0 || tokens[at - 1] != "-" => depth -= 1,
            "," if depth == 0 => { parameters.push(Vec::new()); continue; }
            _ => {}
        }
        if let Some(parameter) = parameters.last_mut() { parameter.push(token); }
    }
    parameters.iter().filter(|parameter| !parameter.is_empty()).all(|parameter| match parameter.as_slice() {
        ["'", ..] => true,
        ["F"] | ["R"] | ["F", ":", "?", "Sized"] => true,
        ["F", ":", bound, ..] => matches!(*bound, "Fn" | "FnMut" | "FnOnce"),
        _ => false,
    })
}
