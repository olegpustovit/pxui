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
- Owner sign-offs (2026-11-17, pxui Unification Plan in LocalWave's AGENTS.md): (4) build-time measurement scripts
  (LocalWave `tools/dx/buildtime.sh`) may use their own target directories and a temporary worktree, removed when they
  finish; the shared-`target/` rule above holds for everything else. (5) Breaking API and ABI changes are welcome until
  the 2.0 freeze, with no compat layers, twins or shims; only performance, build speed and zero warm allocations
  constrain a change (ABI minors still bump and manifests are regenerated, never hand-edited).

## Contracts

- Dependencies are std and `cc`. The one exception is cosmic-text, in `pxui_text` only.
- A warm frame allocates nothing; `mem_sys`'s counting allocator is what the tests assert against. One named exception: shaping a string the text cache has never seen during a fast fling may
  allocate inside the shaping library; it has its own measured workload whose ceiling may only go down (pxui_core
  `window_pxui_fling_shaping_stays_under_its_ceiling`, `FLING_CEILING`). The prepare queue (a host-owned `TextWorker` attached to
  any number of `Ui`s; virtual lists' rows past the viewport, `Ui::prewarm_text`) keeps other first-time shaping off the UI thread.
  A UI draws with a small fixed number of draw calls (one instanced pipeline); a change that adds
  draw calls or per-frame allocations needs a measurement and a test that pins the new number.
- A leaf's bounds come from its parent; skip its child-placement pass only without children and
  without SCROLL/SCROLL_X. Empty scrolling grids still clamp both offsets. Layout child loops copy
  only the fields they use, not the full 600-byte Node with its visual/motion payload. Preserve
  leaf/parent transitions, native pixels, warm allocations and the unchanged layout-scaling ratio.
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

## Public API documentation

The code is the documentation. There is no separate guide, tutorial or API
reference to keep in step; `specification/` describes behaviour, not signatures.

A type that crosses the C ABI as a record, and a type the engine shares (a style value both sides take, such as `Overlay` or `Choice`), lives in `pxui_types`; an authoring-only style value, callback set or response struct lives in `pxui` (`author/`).
Style-value builders name sizes one way: a length in points ends in `_px` (`w_px`, `h_px`), a `Size` takes the bare
name (`w`, `h`), and alignment is one builder taking its enum (`Label::align(TextAlign::Center)`).
A style value holds a table of per-state looks (`States`) by reference (`look(&'a States)`, borrowed for the value's `'a`) and every
other setting by value; a flag with no value is a no-arg builder (`danger()`), a state that varies takes a `bool` (`enabled(bool)`).

Every public item in `pxui` and `pxui_types` has a `///` comment. For a function
or method the comment has, in this order:

1. One sentence saying what it does, in the caller's terms.
2. What the caller must know to use it correctly: units, coordinate space,
   when the result is valid, what happens on a dead handle.
3. `/// Allocates:` — "never", or exactly when (first call, cache miss, growth).
4. `/// Identity:` — for any `label` or `key` parameter, how the key is derived
   and what collides.
5. `/// Panics:` — "never", or the condition.
6. A fenced example that compiles and runs under `cargo t -p pxui --doc`.
   Examples use `pxui::doc::frames`. `ignore`, `no_run`, `text` and
   `compile_fail` fences are not accepted on functions. `cargo run -p gate -- docs` runs each
   module's doctests as its own `cargo t -p <crate> --doc <path of the module>` chunk and fails when a
   doctest is outside every chunk, ignored or skipped.

Fields, variants and constants need one line each; they need no example.

Change the comment in the same commit as the signature or behaviour. A commit
that changes public behaviour without touching its comment is incomplete.
`cargo run -p gate -- docs` enforces the shape; `#![deny(missing_docs)]`
enforces presence.

## Public API errors

Use `Result<T, Error>` for operations that can fail on input or host state, and `Option<T>` when
the requested item may be absent. Native host errors carry a static message naming the failed
operation. A call tied to a Ui can also report the library's detail through `Ui::last_error`.
An accessibility update reports whether it applied to a live handle. Numeric and text-data
errors from fallible setters return `Error::InvalidInput`.

Panics cover documented programmer errors. A Rust callback panic resumes after the ABI call
returns. C entry points validate their callers' records and return status codes.

## Architecture: UI, platform and host integration

A crate's tier is its folder: `crates/public`, `crates/private` or `crates/open`. Put each new crate
in its tier folder. The gate enforces this table; ABI design lives in `docs/abi.md`.

| Tier | Crates | May depend on | Rule |
| --- | --- | --- | --- |
| public | `pxui` (wrapper), `pxui_types` | std, `pxui_types`; `pxui_abi` only behind `static` | Stable surface; additive changes only, bump ABI minor. |
| private | `pxui_abi`, `pxui_dylib`, `pxui_core`, `pxui_text`, `pxui_render`, `pxui_atspi` | private, open, `pxui_types` | Reachable only through `libpxui`; change freely. |
| open | `ogpu_*`, `*_sys`, `input`, `sdl3`, `pxui_sdl`, `pxui_wayland`, `mem_sys`, `cfmt` | open only | Statically linked by apps; no ABI promise, apps pin the commit. |
| host | `pxui_host` | public, open | Framework-owned window/GPU/image lifecycle using public UI APIs; no private engine imports. |

- Apps talk to UI through `pxui`. If an app needs private functionality, add a `pxui_*` export, wrapper method and `pxui.h` declaration together.
- `libpxui` is the `pxui_dylib` cdylib over `pxui_abi` (its own package, so it is built with LTO like the static build) and exports only `pxui_*`; `pxui` uses the dynamic ABI or those same functions through the `static` feature.
- FFI uses `repr(C)`, pointer/length pairs and callback/user pairs. Borrows last through the next frame; Rust heap types, cross-side freeing and unwinding do not cross the ABI.
- The app owns the GPU. PXUI adopts native handles and records into the app's command buffer/pass; it does not submit, wait, present or destroy shared resources. Calls stay on one thread, and the library creates no windows or displays.
- That ownership contract describes the hostless renderer API. Apps may instead use `pxui_host`, which owns device/queue setup, frame acquisition/presentation and image texture retirement. Keep generic host machinery there, native Metal calls in `metal_sys`, and AppKit chrome in `sdl3_sys`. The convenience host supports macOS and experimental iOS SDL/Metal. iOS uses a display link and drains frames on background notifications; safe-area geometry and keyboard show/hide are qualified on the iOS simulator. Basic touch arbitration and docked keyboard avoidance have simulator checks. Native UITextInput fixtures cover marked text, traits and Done; manual editing, real IME/dictation, floating-keyboard avoidance and physical-device behavior remain unqualified. Other hosts keep the existing renderer API.
- `pxui::images` owns retained descriptors, bounded UTF-8 image paths, coalesced visibility-aware requests and LRU metadata. `pxui_host::Graphics` owns their textures and GPU fences. Applications deliver decoded RGBA data or a miss with the requested epoch and an increasing delivery revision; source access and application-specific decode scheduling stay outside the framework. Never expose a caller-supplied completed serial for image destruction. Preserve exact-pixel, stale-epoch, invalid-packet, in-flight retirement, constructor cleanup and zero-warm-allocation checks.
- The native frame context is a private Objective-C++ struct with ARC-owned members. Do not register an Objective-C frame class: the native shim can be linked into both the static framework host and dynamic renderer, and duplicate runtime classes collide.
- ABI snapshots are append-only and gated. Paint shapes stay in wrapper spans; only text calls the library. `abi_bench` gates performance against the recorded baseline.

Mark deliberate shortcuts with known ceilings using `ponytail:` and an upgrade trigger.
Never shortcut correctness, input validation, lifetime safety, security or accessibility.

## Focused text fields

Keep `text_field` focus rebuilding for caret/IME output. Its edit buffer is reused, and text/composition
paint captures borrow byte ranges in the frame arena; keep ranges valid through arena growth and reset.
Selection and inline decorations stream the same shaped caret ranges instead of cosmic-text's allocating
highlight iterator. Preserve the core/static/dynamic focused selection/IME allocation tests and native
1x/2x pixel regression. Changed text/input events and other editor widgets are separate allocation paths.
Custom editor parts supply the font size for focused glyphs, caret/selection and IME. Pointer hits use
the previous displayed part's size; paint uses the newly built part's size. Preserve zero/default and
relative sizing, same-frame size changes, UTF-8 selection and static/dynamic warm-allocation checks.
Text fields read scalar signals and temporarily borrow their retained key/modifier buffers while
editing, restoring them before subsequent queries. Warm modified keyboard navigation must not clone
those events or captured press modifiers; owned text/IME changes remain separate allocation paths.

## Carry/drop queries

Modal signal filtering reuses the signal map rather than collecting its keys each frame. Fitted
plain labels reuse the painter's existing string buffer; preserve the legacy character-width cut,
UTF-8 boundaries, tiny-width ellipsis and ASCII caps. Keep the modal wheel allocation regression
and fitted/plain draw-instance parity case. Owned `ellipsis` helper results remain a separate path.

The ABI output dirty-span buffer must reserve from the engine layers' retained dirty capacities,
not only the previous frame's actual range count. New fragmentation of an already warm draw must
not allocate while reading output. Preserve the regression that acknowledges a draw, reads an
unchanged frame, then requests sixteen disjoint updates without new draw storage. `layout_scaling`
is a timing case: the test runner puts it in the existing serial group, retaining its 1.5x bound.

Dismissed popup roots and attached roots are ignored while routing the next frame's old hit tree,
then cleared before rebuilding. Preserve the overlapping next-press and warm-allocation regression;
clearing modal ownership alone leaves the previous popup's clickable nodes in that tree.

SlotTable reuses tombstones and grows only when the live count requires it; tombstone churn must
rehash through retained spare arrays without increasing capacity. Recycled empty text/name/value
buffers choose the smallest suitable donor, preserving larger buffers for longer labels. Existing
node identities keep the fast capacity check, even though set_box clears their strings. Preserve the registered 100-frame identity/text
churn case and LocalWave's alternating cache-limit/menu regression; do not extend their warm-up.
Empty-string donors use retained sorted capacity/slot indices, built lazily once per field per
frame. Node clears, retirement, empty writes and buffer transfers keep active indices current;
nonempty writes remove their entry. Reserve metadata with slot capacity so the first warm query
does not allocate. Keep the differential full-search check, live-content preservation, bounded
metadata and dynamic/static scrolling/allocation tests. Stable nodes keep their capacity fast path.

SDL dialog callbacks release-publish their copied outcome before calling the existing host wake.
Chosen, cancelled and failed dialogs must wake an idle event loop; do not add periodic polling.
Keep the typed native bridge regression's zero-time wait checks for all three outcomes.

iOS document pickers retain the original selected NSURL in file_access_sys::FileSelection.
Window::take_dialog_selection consumes it once before the next pump; ignored outcomes and window
destruction release capabilities. Move them to the file worker before creating bookmarks. Never
reconstruct provider access from the portable event's path. Preserve original-object identity,
main-thread refusal, cancellation/failure/expiry and eight-slot teardown checks. The optional
PXUI_IOS_PICKER_CHECK probe drives actual UIKit presentation/delegates before the original
182/82/100 allocation gate; local URL fixtures do not qualify real provider grants or hardware.

For iOS SDL bring-up, use `packaging/ios-spike.sh` with the shared build root. Target-specific
SDL3_DIR/pkg-config metadata must never fall back to host libraries. Keep arm64/device versus
simulator platform and iOS 18 floor checks, default stack size, UIKit entry through SDL and retained
Metal-layer invalidation. The probe draws the existing kit gallery through pxui_host for 182 frames,
with zero Rust allocations in the final 100 after 82 warm-up frames. Keep the display-link idle
deadline and same-process background/resume checks (`PXUI_IOS_LIFECYCLE_CHECK=1`); inactive
draws must skip before GPU work. Native UIKit callbacks are scoped to the main loop and may
not reenter Rust from an SDL event watch. Text/IME/dialog encoding reuses framework-owned
storage, and native IME area updates skip unchanged geometry. The first five-event resume
batch after a one-event startup must keep zero warm Rust allocations across both ABI paths;
retain the eight-slot startup reserve without allocating in input::at_time.
Keep actual native did-show/did-hide checks and exact safe-area bounds, including a keyboard
already dismissed during background/resume. Do not wait for a second notification after state
already matches. The probe does not qualify finger gestures, IME editing, synchronized avoidance
motion, native allocations or physical devices.
The Metal backend uses the standard render-pipeline API when Metal 4 compiler creation reports
Unsupported; retain function/device/profile validation, binding reflection and native pixel checks.
Private textures use a device-owned, synchronous GPU staging queue and at most 4 MiB retained
scratch per native Device wrapper. Preserve large chunked/padded transfers and warm Rust allocation
checks; never infer unified memory to bypass private transfers. Normal copy usages are required.
Native executables may select the backend through pxui_host's required normal platform dependency;
the gate must reject duplicate app/host ownership and optional/dev/missing/wrong-platform backends.

SDL3 drop strings are borrowed, like text/IME event strings. Copy them into the bridge-owned
payload and never free the source pointer; SDL reclaims its temporary event memory on a later
pump. The typed trace uses a static borrowed drop string so an accidental source free aborts
deterministically, and verifies that later source mutation cannot change the owned Rust event.

Reopened view kinds reuse their retained node/hit/name buffers from the bounded free-view pool.
Preserve the nested-view reopen regression; LIFO reassignment can otherwise keep growing buffers
on warm resize/show/hide cycles. Profiling labels may remain on free records until reassignment.

Accessibility action injection validates the complete span before applying any action, then
translates in fixed stack chunks into the engine's retained input queue. Preserve the 4,096-action
bound, input order, invalid-tail atomicity, scalar payloads and static/dynamic warm allocation
regressions. A per-call translation Vec breaks accessible scrolling's warm-frame allocation gate.

Paint::curve / Paint::cubic use the renderer's existing analytic quadratic instances through the
shared shape writer. Cubic uses four midpoint-matched segments for control-sized paths; preserve
the documented limit and LocalWave's subpixel SVG comparison. No new draw pipeline or C layout.
Reject nonfinite geometry and nonpositive/nonfinite widths; warm shape building allocates nothing.

Analytic stroke widths must also fit the existing f16 instance field (maximum 65,504); reject
finite overflow before writing an instance. Preserve the huge-finite-width regression.

Indexed section headings must return to the viewport with one frame build. Discarded paint is
not a clean build cache: skipping that rebuild forces visit_view to settle the entire frame.
Preserve the callback-count assertion alongside clipping, pixels and warm allocations.

Virtual lists clamp invalid offsets before selecting rows after their extent shrinks. Preserve the
one-build shrink-to-three/empty regression and warm allocations; clamping only during arrange
can leave a resting virtual list blank until the next input.

Changing a nested view's transform or patched accessible content must invalidate its ancestors'
accessibility spans. A cached parent copies its entire subtree's span; invalidating only the child
leaves stale bounds under an unchanged parent. Preserve flat/nested scale-retarget/keyframe checks,
unchanged glyph instances, no additional content builds and zero warm allocations.

Widget touch arbitration lives in pxui_core::ui_core::touch, never in applications. Retain the
fixed 16-contact bound, 8-point axis lock, 10-point slop, per-scroll release velocity, normal finger
decay and independent trackpad physics. Captures move to scroll ancestors before virtual rows
retire. Catching a coast must not click; modal/focus/stale-owner/multitouch/mouse cancellation
must not release or commit a control. Internal pointer actions feed existing widgets and popup
dismissal, while routed viewports receive typed touches only. Suppress hover while using touch.
pxui_sdl disables both SDL mouse/touch emulation hints. Preserve core physics/cancellation checks,
static/dynamic 10,000-row allocation checks and the optional PXUI_IOS_TOUCH_CHECK SDL/Metal probe.
Synthetic simulator input does not qualify hardware feel, long press, edge navigation or refresh.

The scoped iOS loop delivers Signal::Control for inactive worker wakes and explicit deadlines.
Keep the display link paused and all Metal drawing disabled in these callbacks. NextFrame defers
drawing until foreground; WaitUntil uses the existing dispatch timer without idle polling. Retain
the same-process lifecycle probe: a background worker wake and deadline at frame 90, no frame or
warm Rust allocation, then resume and finish the original 182/82/100 checks. This is not a grant
of iOS background execution time or qualification of actual background audio on a physical device.

pxui_sdl::app::run shares the callback contract across desktop and iOS. Finish, captured resources
and the window must clean up in that order on success, initialization errors, frame errors and
invalid deadlines. Window::preferences owns cached Apple appearance/reduced motion: AppKit KVO
and workspace notifications, UIKit trait registration and accessibility notifications. Native
subscriptions belong to sdl3_sys; apps only read copied snapshots. Preserve main-thread refusal,
native notifications, duplicate suppression, teardown and zero-warm-read checks.
iOS worker wakes signal the retained CFRunLoop source directly, never queue SDL user events.
Retain the source under the loop lock before signalling; do not invoke Rust from a worker or event
watch. Re-post dirty inactive work after a control callback: its nested native run loop can consume
a source while the Rust busy guard is set. Keep the deterministic native regression, original
failure log, light/dark changes and 182/82/100 allocation/lifecycle checks.

A fresh cached-view record reserves at least 32 bytes for its diagnostic name; recycling adaptive
subtrees must not grow these short-name buffers after warm-up. Preserve full names and prefix-based
rebuild diagnostics; labels longer than 32 bytes remain supported and may grow storage when cold.
Keep the recycled-record zero-allocation regression and existing node/hit-capacity reuse checks.

Generic Apple profiling belongs to mem_sys::profile: thread CPU time, initial-thread-only QoS and
RAII Instruments intervals. Keep its native C symbols/log namespace generic and keep QoS changes
after worker creation in consumers. Preserve error handling, monotonic clock and zero-warm-Rust
allocation checks. Apps must not depend on GPU crates even for tests. Texture inspection is
crate-private/test-only in pxui_host; its internal main-thread harness retains full-batch native
pixel/replay, warm draw and GPU-completion assertions. Prefix native cases with window_ so cargo t
serializes them even in a library test target. Public `hash_of(&value)` (fixed-seed FNV-1a 64, the one public hash) owns transient revision hashing while
callers supply facts; it borrows the input, allocates nothing and gives the same value on every run and platform.
