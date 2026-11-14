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

## Architecture: UI, platform and host integration

A crate's tier is its folder: `crates/public`, `crates/private` or `crates/open`. Put each new crate
in its tier folder. The gate enforces this table; ABI design lives in `specification/abi.md`.

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
- That ownership contract describes the hostless renderer API. Apps may instead use `pxui_host`, which owns device/queue setup, frame acquisition/presentation and image texture retirement. Keep generic host machinery there, native Metal calls in `metal_sys`, and AppKit chrome in `sdl3_sys`. The convenience host supports macOS and experimental iOS SDL/Metal. iOS uses a display link and drains frames on background notifications; touch, safe areas, keyboard and physical-device behavior remain unqualified. Other hosts keep the existing renderer API.
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

ABI 1.23 adds `key_pressed_with`: a raw, non-consuming key query with Shift/Ctrl/Alt/Meta held at
the press, including a modifier released later in the same batch. Capture the initial modifier state
before routing and replay event order, clearing on focus loss. Callers still arbitrate modal ownership.
Keep C invalid-input output atomicity and focused/modal static/dynamic warm-allocation checks.

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


ABI 1.15 adds `Ui::key_down` / `pxui_ui_key_down`, a scalar raw held-key query that respects
the existing owner/callback guards but does not filter widget or modal ownership or consume keys.
Event-only frames retain held keys until release/focus loss; full Input frames replace them with
the host snapshot. Preserve cross-frame modifiers, focused/modal queries, release/focus-loss and
static/dynamic zero-allocation checks. Popup filtering must recycle rejected accumulators and their
modifier buffers into the existing pools rather than dropping warmed storage. Hosts accepting a
custom popup result should dismiss after the popup call returns, before processing another input.

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

For iOS SDL bring-up, use `packaging/ios-spike.sh` with the shared build root. Target-specific
SDL3_DIR/pkg-config metadata must never fall back to host libraries. Keep arm64/device versus
simulator platform and iOS 18 floor checks, default stack size, UIKit entry through SDL and retained
Metal-layer invalidation. The probe draws the existing kit gallery through pxui_host for 182 frames,
with zero Rust allocations in the final 100 after 82 warm-up frames. Keep the display-link idle
deadline and same-process background/resume checks (`PXUI_IOS_LIFECYCLE_CHECK=1`); inactive
draws must skip before GPU work. Native UIKit callbacks are scoped to the main loop and may
not reenter Rust from an SDL event watch. Text/IME/dialog encoding reuses framework-owned
storage, and native IME area updates skip unchanged geometry. The first four-event resume
batch after a one-event startup must keep zero warm Rust allocations across both ABI paths.
The probe does not qualify finger identities/gestures, keyboard, safe areas, native allocations
or physical devices.
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

ABI 1.16 exposes the engine's host-supplied frame clock as Ui::now / pxui_ui_now.
Offset/opacity/scale on an indexed retained view compose in its table entry, including nested hit
and accessibility rectangles. They must leave cached draw instances unchanged and stop drawing
when settled. Same-time keyframe channels form one pose before interpolation; the last curve at
that timestamp shapes the shared segment. Preserve core and static/dynamic wrapper regressions.

Reopened view kinds reuse their retained node/hit/name buffers from the bounded free-view pool.
Preserve the nested-view reopen regression; LIFO reassignment can otherwise keep growing buffers
on warm resize/show/hide cycles. Profiling labels may remain on free records until reassignment.

Accessibility action injection validates the complete span before applying any action, then
translates in fixed stack chunks into the engine's retained input queue. Preserve the 4,096-action
bound, input order, invalid-tail atomicity, scalar payloads and static/dynamic warm allocation
regressions. A per-call translation Vec breaks accessible scrolling's warm-frame allocation gate.


ABI 1.17 adds Ui::text_line_box / pxui_ui_text_line_box. Positive explicit line heights use
rounded primary-face ascent/descent and floored half-leading, with ULP snapping at integer
boundaries. Preserve legacy text/text_spans positioning. Only explicit line-box nodes derive
Fit dimensions from shaped text; empty labels retain one line and resolved Grow widths settle
wrapped height in the same frame. Preserve invalid-input atomicity, static/dynamic resize/empty
checks and warm allocations. Font metrics are normalized once when a font is loaded.


Paint::curve / Paint::cubic use the renderer's existing analytic quadratic instances through the
shared shape writer. Cubic uses four midpoint-matched segments for control-sized paths; preserve
the documented limit and LocalWave's subpixel SVG comparison. No new draw pipeline or C layout.
Reject nonfinite geometry and nonpositive/nonfinite widths; warm shape building allocates nothing.

ABI 1.18 adds TextStyle::WRAP_WORD_SPACES (3), retaining logical blank advances omitted between
visible soft-wrapped runs in fitting metrics. Preserve normal word/glyph/no-wrap metrics and all
visible glyph positions. Reuse the text engine's row scratch and cached metrics; inspect blank
shape glyphs against monotonically ordered logical run bounds, never re-shape substrings or scan
every paragraph once per line. Hidden/truncated paragraphs do not contribute space advances.
Preserve static/dynamic whitespace, paragraph, BiDi, glyph equality and warm allocation checks.


ABI 1.19 exposes Ui::density / pxui_ui_density: the existing integer rendering density shared
with Paint, rounded from host scale and clamped to at least one. Do not change the engine's
fractional-scale policy here. Preserve initial/owner/callback queries, cache invalidation when
density changes, equal-density retention and static/dynamic warm allocation checks.

Analytic stroke widths must also fit the existing f16 instance field (maximum 65,504); reject
finite overflow before writing an instance. Preserve the huge-finite-width regression.


ABI 1.20 adds Ui::section_list / pxui_ui_section_list: one scrolling lead-in, optional pinned
heading and fixed-height virtual rows, with an optional overlay scrollbar. Reuse the existing
sticky, blocking and scroll layout; keep row selection O(viewport rows), never allocate per count.
C callbacks receive SIZE_MAX for the lead-in, SIZE_MAX-1 for the heading, otherwise a row index.
Validate the whole descriptor before building anything; preserve callback guards and recovery.
Culled cached views still update their composed geometry and accessibility visibility, without
keeping offscreen draw runs. Preserve indexed/flat/clipped heading scrolling, the 570-row storage
bound, native 1x/2x opaque heading/right-edge pixels, click blocking and warm allocation checks.

Indexed section headings must return to the viewport with one frame build. Discarded paint is
not a clean build cache: skipping that rebuild forces visit_view to settle the entire frame.
Preserve the callback-count assertion alongside clipping, pixels and warm allocations.

Virtual lists clamp invalid offsets before selecting rows after their extent shrinks. Preserve the
one-build shrink-to-three/empty regression and warm allocations; clamping only during arrange
can leave a resting virtual list blank until the next input.

ABI 1.21 adds Ui::section_list_measured / pxui_ui_section_list_measured. Reuse the existing
measured-list prefix cache and binary search, including same-frame measurement settling; the row
height is an estimate for unseen rows. The callback receives the logical content Y of each part.
Warm height changes/scroll/shrink allocate nothing; storage is two floats per row plus one prefix
entry. Preserve static/dynamic descriptor, callback recovery, native 1x/2x and one-draw checks.

Changing a nested view's transform or patched accessible content must invalidate its ancestors'
accessibility spans. A cached parent copies its entire subtree's span; invalidating only the child
leaves stale bounds under an unchanged parent. Preserve flat/nested scale-retarget/keyframe checks,
unchanged glyph instances, no additional content builds and zero warm allocations.

ABI 1.22 adds filled triangles through the existing shape span: kind 9, A/B in rect and C as f32
bits in extra, one 64-byte instance and the existing instanced draw. Reject degenerate/nonfinite
geometry and overflowing or subnormal edge lengths before writing. Keep Metal, GLSL and SPIR-V
together; preserve both windings, native 1x/2x coverage, one-draw and warm-allocation checks.

Ui::a11y_value exposes the existing text-value storage with UTF-8 byte anchor/caret positions.
Validate the whole input before mutation; reversed selections are valid, numeric values take
precedence, and stale handles remain quiet in the Rust wrapper. a11y_value_of borrows the current
tree's text. Preserve C invalid-input atomicity and static/dynamic buffer-reuse checks. No C layout
changes in this minor.

The accessibility-value ABI accepts UTF-8 byte endpoints but must convert them to Unicode scalar
offsets for the engine's tree, matching its existing text fields and AT-SPI contract. Keep reversed,
combining-mark and non-BMP selections, validation before mutation and zero warm allocations.
