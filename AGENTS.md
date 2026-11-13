# PXUI development guide

PXUI is a Rust native UI engine: `pxui_core` (layout, widgets, draw list), `pxui_text`, `pxui_render`,
window hosts (`pxui_sdl`, `pxui_wayland`), the AT-SPI2 bridge (`pxui_atspi`) and the portable GPU layer under them (`ogpu_*`, `*_sys`).
The Cargo workspace is the repository root; `crates/public`, `crates/private` and `crates/open` hold
the library tiers, `tools/` the test runner, gate and gallery, `fixtures/` the goldens,
`specification/` the design contracts. Rays embeds this repository
as its `pxui/` submodule through path dependencies: every crate must build and test here alone, and
nothing may reach outside this repository (no `../../..` paths past the root).

## Fast iteration

- Read the actual flow and callers first. Reuse existing code, std and the installed dependencies.
  Write the smallest correct change, delete obsolete code, add no abstractions.
- Fast compilation AND fast debug execution are requirements. Measure both before changing profiles,
  dependencies, crate boundaries or hot paths. Budgets: clean debug build of the workspace under 30 s
  (cosmic-text is about 13 s of it); warm `cargo check -p pxui_core` after an edit under 1 s.
- Keep incremental compilation and the warm shared `target/`. No routine clean, private target
  directories or RUSTFLAGS/profile overrides.
- Embedded Apple Silicon macOS workspaces sharing `target/` should resolve the same `tools/cc-stack.sh`
  linker path. Consumers can set `PXUI_CONSUMER_DEFAULT_STACK=1` to keep the platform stack; PXUI-owned
  tools keep their legacy stack. Without the opt-out existing consumers retain their stack setting.
  The public wrapper tracks this environment value so changing it relinks consumers.
- Inner loop: `cargo check -p <crate>`. Focused regression: `cargo t -p <crate> [filter]`.
  Never run raw `cargo test` or test binaries: `cargo t` kills cases at 5 seconds and runs at 60 seconds,
  serializes GPU/window cases, reports the slowest tests and records `target/t-times.tsv`.
- A focused warm run should finish under 30 seconds. Never raise deadlines to hide regressions or
  weaken assertions. Leave one small runnable regression for nontrivial logic.
- Run `git diff --check` and `cargo run -p gate -- deps` before committing; commit completed stages.

## Contracts

- Dependencies are std and `cc`. The one exception is cosmic-text, in `pxui_text` only.
- A warm frame allocates nothing; `mem_sys`'s counting allocator is what the tests assert against.
  A UI draws with a small fixed number of draw calls (one instanced pipeline); a change that adds
  draw calls or per-frame allocations needs a measurement and a test that pins the new number.
- `pxui`, `pxui_text` and `pxui_render` contain no SDL and no window code, and `pxui` knows nothing
  about the GPU. Input is data in (`input::Event`, a logical size, a scale factor, a time); output is
  data out (a draw list plus cursor, IME, clipboard and repaint request). A target is a render pass
  the caller opened, not a swapchain.
- Metal (Apple Silicon) and Vulkan (Windows, Linux) are both first class. No Metal-only or
  unified-memory assumption in a GPU data path. No CPU raster, OpenGL or browser fallback; missing GPU
  support is a typed startup error.
- Hosts expose the same handful of functions; there is no `Host` trait and nothing generic to
  monomorphise. Public functions take concrete types or `&mut dyn FnMut`.
- SDL/GPU/window/resource operations stay on the initial thread. No per-frame threads.
- UI and input use logical points; drawable-pixel conversion belongs at the backend boundary.
  UI returns intents; it does not mutate application state during frame construction.
- Preserve the theme tokens, Pragmasevka, 24-point rows/bars and 20-point controls unless a spec
  change says otherwise. Goldens are owned here (`fixtures/`): refresh them alongside the intentional
  change, at the display density the golden was made at.
- Required C/C++/Objective-C++ ABI code lives under its owning `*_sys` crate; the Metal bindings are
  checked in under `crates/open/metal_sys/src/`.
- `cargo run -p gate -- deps` enforces crate boundaries and `-- shape` code shape (no generic items, no `unsafe` outside `*_sys`, no `impl Trait`, no panics for control flow); policy is `tools/gate/src/policy.rs`.
- `cargo run -p gate -- tiers` enforces the public/private/open dependency table below.

## Architecture: three tiers

A crate's tier is its folder: `crates/public`, `crates/private` or `crates/open`. Put each new crate
in its tier folder. The gate enforces this table; ABI design lives in `specification/abi.md`.

| Tier | Crates | May depend on | Rule |
| --- | --- | --- | --- |
| public | `pxui` (wrapper), `pxui_types` | std, `pxui_types`; `pxui_abi` only behind `static` | Stable surface; additive changes only, bump ABI minor. |
| private | `pxui_abi`, `pxui_dylib`, `pxui_core`, `pxui_text`, `pxui_render`, `pxui_atspi` | private, open, `pxui_types` | Reachable only through `libpxui`; change freely. |
| open | `ogpu_*`, `*_sys`, `input`, `sdl3`, `pxui_sdl`, `pxui_wayland`, `mem_sys`, `cfmt` | open only | Statically linked by apps; no ABI promise, apps pin the commit. |

- Apps talk to UI through `pxui`. If an app needs private functionality, add a `pxui_*` export, wrapper method and `pxui.h` declaration together.
- `libpxui` is the `pxui_dylib` cdylib over `pxui_abi` (its own package, so it is built with LTO like the static build) and exports only `pxui_*`; `pxui` uses the dynamic ABI or those same functions through the `static` feature.
- FFI uses `repr(C)`, pointer/length pairs and callback/user pairs. Borrows last through the next frame; Rust heap types, cross-side freeing and unwinding do not cross the ABI.
- The app owns the GPU. PXUI adopts native handles and records into the app's command buffer/pass; it does not submit, wait, present or destroy shared resources. Calls stay on one thread, and the library creates no windows or displays.
- ABI snapshots are append-only and gated. Paint shapes stay in wrapper spans; only text calls the library. `abi_bench` gates performance against the recorded baseline.

Mark deliberate shortcuts with known ceilings using `ponytail:` and an upgrade trigger.
Never shortcut correctness, input validation, lifetime safety, security or accessibility.

## Focused text fields

Keep `text_field` focus rebuilding for caret/IME output. Its edit buffer is reused, and text/composition
paint captures borrow byte ranges in the frame arena; keep ranges valid through arena growth and reset.
Selection and inline decorations stream the same shaped caret ranges instead of cosmic-text's allocating
highlight iterator. Preserve the core/static/dynamic focused selection/IME allocation tests and native
1x/2x pixel regression. Changed text/input events and other editor widgets are separate allocation paths.

## Numeric keyboard signals

ABI 1.10 reuses routed signal/modifier buffers. The Rust `signal_state` convenience uses the existing
`pxui_ui_signal` function and returns the same scalars with empty owned keys; `signal` still copies
all key/text/IME events. Do not discard consumed activation keys to remove allocations. Preserve
per-event modifiers, stale-handle quietness and static/dynamic warm keyboard tests. The ABI encodes
borrowed core events into its retained chunks, reserving all text bytes before publishing pointers.
Signal/modifier pools grow only to the peak events/signals seen; owned text and legacy key-vector
reads remain separate allocation paths. No C function signature or layout changed in this minor.

## Carry/drop queries

ABI 1.11 adds borrowed Rust `carrying_ref`/`drop_target_ref` conveniences over the existing C exports.
The ABI copies query strings into retained slots: C borrows survive payload replacement/cancellation
until the next frame. Owned getters still copy. Core reuses gesture strings and moves released
payloads; the ghost temporarily owns its payload while copying text into frame storage. Preserve
whole-gesture static/dynamic zero-allocation checks and the C borrow-lifetime regression. Cached
views must propagate size reflow through their descendants and invalidate enclosing caches when a
virtual grid's build-time size estimate changes; keep the nested-grid same-frame resize check.

ABI 1.12 adds `transfer_carry` to move a pointer carry's capture to a live hit ancestor. Virtual
children may then disappear; the ancestor disappearing or losing capture still cancels it. Standard
scroll containers follow pointer edges in either axis, including RTL. Preserve time-based bounded
scrolling and the two UI-owned subpixel remainders (no extra per-slot storage), plus static/dynamic
modified-input/cancellation tests and the 100,000-cell edge/precision/allocation regression. Release
and explicit cancellation must not click the capture owner. Validate owner presence before painting
the ghost so an omitted container leaves no ghost in the same frame. Retired slots clear text/name/
value contents while retaining their buffers for reuse before the next slot identity is assigned.

Modal signal filtering reuses the signal map rather than collecting its keys each frame. Fitted
plain labels reuse the painter's existing string buffer; preserve the legacy character-width cut,
UTF-8 boundaries, tiny-width ellipsis and ASCII caps. Keep the modal wheel allocation regression
and fitted/plain draw-instance parity case. Owned `ellipsis` helper results remain a separate path.


ABI 1.13 adds Renderer::forget_image / pxui_renderer_forget_image. Hosts stop referencing an evicted
image and forget it on every sampling renderer before destroying/reusing the native handle, passing
the host's actual completed serial on the prepare timeline; completion must not regress. Refuse incomplete draws and never destroy the caller's
image. Native identity interning is an O(1) LRU bounded to 1,024 entries and a 128 MiB estimated texel
working set (one oversized image may stand alone). Wrappers retain weak image identities and retire
abandoned entries by actual completed serials. Ignore inherited/trailing image breaks whose segment
has no IMAGE instances; solid/text runs must not refresh resource use or bind stale textures. Preserve
the 128-distinct-image native pixel/warm allocation test, explicit release guards, metadata bounds,
and both backend paths. This is no process/native-memory qualification.


ABI 1.14 adds `Paint::fill_fractional` in the shared shape writer. `fill` keeps its pixel snapping;
fractional fills preserve finite, increasing bounds and use RECT flag AREA (bit 2) for pixel-area
coverage on axis-aligned fills. Keep Metal/GLSL and checked-in SPIR-V together. Preserve the 1x/2x
native thin-bar area/pixel, one-draw and warm-allocation checks, plus static/dynamic descriptor tests.
Derivative inputs must be evaluated before fragment clipping/discard. No C layout changed.

The ABI output dirty-span buffer must reserve from the engine layers' retained dirty capacities,
not only the previous frame's actual range count. New fragmentation of an already warm draw must
not allocate while reading output. Preserve the regression that acknowledges a draw, reads an
unchanged frame, then requests sixteen disjoint updates without new draw storage. `layout_scaling`
is a timing case: the test runner puts it in the existing serial group, retaining its 1.5x bound.
