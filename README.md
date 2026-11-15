# PXUI

A small, fast, native UI engine in Rust. Plain events go in, a draw list comes out; one instanced
pipeline paints it on Metal (Apple Silicon) or Vulkan (Windows, Linux). It is the UI of
Rays, which embeds this repository as a submodule at `pxui/`, and it
builds and tests on its own.

Dependencies are std and `cc`; `pxui_text` alone adds cosmic-text for shaping and glyph rasterisation.

## Crates

| Crate | Role |
| --- | --- |
| `pxui` | ids, layout, widgets, theme, draw list. Knows nothing about the GPU or the window system. |
| `pxui_text` | cosmic-text, shaping cache, glyph atlases, the bundled Pragmasevka font |
| `pxui_render` | the painter: one instanced pipeline, atlas textures, draws into a pass the caller opened |
| `pxui_sdl` | SDL3 window host: lifecycle, event pump, surface token |
| `pxui_host` | macOS and experimental iOS framework host: device/frame presentation and bounded image texture ownership, using the public UI wrapper |
| `pxui_wayland` | Wayland client window host (Linux) |
| `pxui_atspi` | AT-SPI2 accessibility bridge (Linux): serves `pxui_core::a11y`'s tree over libdbus-1 |
| `pxui_gallery` | the kit gallery app (`cargo run -p pxui_gallery`): every widget, both themes, goldens |
| `ogpu_core`, `ogpu_mock` | portable GPU API and its mock |
| `ogpu_metal`, `metal_sys` | Metal backend and its Objective-C++ shim |
| `ogpu_vulkan`, `vulkan_sys` | Vulkan backend and its C++ shim (vendored volk and VMA) |
| `native_layer_token_sys` | the SDL-free handle between a window and the GPU |
| `input` | key, event and frame-input types |
| `sdl3`, `sdl3_sys` | SDL3 binding |
| `mem_sys`, `cfmt` | counting allocator (what the zero-allocation tests assert against), C-style formatting |
| `tools/t` | `cargo t`, the deadline-enforced test runner |
| `tools/gate` | the dependency and shape gate (`cargo run -p gate -- deps`, `-- shape`) |

Hosts expose the same few functions and there is no `Host` trait: an app links the one it wants.

## Host loop

On macOS and experimental iOS, `pxui_host` owns GPU setup, frame presentation and image texture lifetimes:

```rust
let mut window = pxui_host::Window::open("demo", 900, 600)?;
let mut ui = pxui::Ui::new()?;
let mut graphics = pxui_host::Graphics::open(&mut window, &ui)?;
// Build UI with public pxui APIs, then present it:
graphics.draw(&mut window, &ui, None)?;
```

`graphics.images()` lends descriptors and bounded decode requests. Deliver decoded RGBA with
`upload_image`; the framework validates packets, caches textures and retires them after GPU
completion. Source access and decoding run in the application's worker. `configure_images` takes
density and the maximum requested source pixels; application layout policy chooses that bound.
The host adds no private engine dependencies or third-party packages. Native/process allocations
are separate from the warm Rust allocator checks.

`pxui_host::Events` retains the ABI event span and its UTF-8 bytes. Clear and reuse the app's
input batch, call the fallible `Platform::pump`, feed `Events::encode` and
`Platform::environment()` and `Platform::keyboard()` into `Ui::frame`, then call
`Platform::apply` once per completed UI frame for cursor, keyboard/IME, background and clipboard
requests. Numeric input and unchanged focused IME geometry allocate no Rust objects once warm;
new text and clipboard contents are separate paths.

`pxui_sdl::app::run` shares the callback contract across desktop and iOS, including Finish and
capture cleanup before window destruction on errors. `Window::preferences` caches Apple system
appearance and reduced motion through native notifications; read its copied snapshot and drop
the monitor before the window.

On iOS, `pxui_sdl::ios::run` owns the window and callback. UIKit drives its display link;
worker wakes signal the run-loop source without SDL user events.
`Schedule::WaitUntil` pauses it until input, a worker wake or the repaint deadline. Metal stops
new submissions while inactive and drains pending work on resign-active notifications. The
callback's `Finish` and captured resource destruction happen before window destruction. Return
errors through the callback result; a panic aborts at the C boundary.
`Signal::Control` services worker wakes and explicit `WaitUntil` deadlines while inactive;
the display link stays paused and `NextFrame` defers drawing until foreground. This does not
grant background execution time to a suspended iOS app. See
[`ios_spike`](tools/ios_spike/src/main.rs) and build it with
`packaging/ios-spike.sh iphonesimulator /absolute/shared/target`. Set
`SIMCTL_CHILD_PXUI_IOS_LIFECYCLE_CHECK=1` on `simctl launch`, wait for the frame-90 marker,
background it with another app, wait for both `PXUI_IOS_BACKGROUND_*_OK` markers at frame 90,
then relaunch the same bundle without terminating it. The 182-frame / 82-warm-up / 100-measured
allocation gate, idle deadline, same-process resume and inactive worker/deadline callbacks are
qualified on the iOS 18.6 simulator. Basic finger arbitration and docked keyboard avoidance also
have simulator checks; IME editing, physical gesture feel, devices and native allocations remain to qualify.

`Window::show_dialog` uses UIKit document pickers on iOS for files, multiple files and folders.
For a chosen `input::Event::FileDialog`, call `Window::take_dialog_selection(id)` once before the
next pump, then move the returned `FileSelection`s to your file worker. `FileSelection::bookmark`
uses the exact original selected NSURL, rejects the UI thread and balances any security scope it
starts. A plain path returns `None`; it does not claim a provider grant. The portable input event
still carries paths only. Unconsumed capabilities expire at the next pump or window destruction.
The framework retains at most eight native dialog outcomes, 4,096 paths / 1 MiB per selection.
iOS path-only SaveFile is unsupported; export needs a source-URL API.

`SIMCTL_CHILD_PXUI_IOS_PICKER_CHECK=1` runs native picker presentation/delegate checks before the
gallery's unchanged 182/82/100 allocation gate. It checks original-URL worker bookmarking,
main-thread refusal, cancellation, invalid URLs, duplicate callbacks, ignored-result expiry and
window-close slot reclamation. This passes on iOS 18.6; its local URL fixtures do not qualify
real file-provider permissions, cloud downloads or physical-device behavior.

ABI 1.25 adds typed touch transport: full 64-bit device/finger IDs, began/moved/ended/cancelled,
logical-point positions, pressure and the source monotonic timestamp. `Event::TOUCH` (25) borrows
a 48-byte `Touch` through its `text` pointer/length; it is numeric data, not UTF-8. `Events` owns
that storage, and `Routed::touch(index)` copies a validated routed payload. Invalid phases,
nonfinite values and pressure outside 0..=1 fail before input mutation; outside-window positions
remain valid. Public `Event` and `FrameIn` layouts are unchanged. SDL coalesces only adjacent
motion for the same device/finger, preserving phase and interleaved-finger boundaries. Native SDL
trace and static/dynamic ABI checks assert zero warm Rust allocations. This is transport coverage;
these transport checks alone do not qualify widget gesture arbitration or physical finger behavior.

Widget touch routing now shares the engine's pointer/capture and scroll paths. A tap inside a
scrolling container waits for release; 8-point axis locking and 10-point slop distinguish dragging
from tapping. The selected scroll ancestor retains the gesture when virtual rows retire. Touch-down
catches momentum without activating a row; source timestamps drive velocity, with the plan's
0.998-per-millisecond finger decay and existing 0.55 rubber-band curve. Trackpad coast settings are
unchanged. A held stretch idles; cancellation releases it without a fling. Buttons and scrubbers
outside scroll containers use normal pointer signals. Touch suppresses mouse hover until mouse
input resumes. Popup/focus loss, stale owners, extra fingers and mouse takeover cancel safely.

`pxui_sdl::Window::open` disables SDL mouse/touch emulation to avoid duplicate delivery. Update
the host and libpxui together from the pinned checkout. Routed viewports receive the original
typed touches, without the engine's synthetic pointer actions. Contact storage is fixed at 16;
overflow cancels widget recognition until focus resets. This is single-finger widget arbitration;
pinch, long-press menus, interactive edge-back, pull-to-refresh and device feel remain open.
`SIMCTL_CHILD_PXUI_IOS_TOUCH_CHECK=1` runs 18 taps/scrolls/coast catches/cancellations through actual
SDL, virtual widgets and Metal, preserving the original 182/82/100 allocation and idle-wake checks.
The simulator trace uses synthetic SDL fingers, not physical touch or UIScrollView parity evidence.

ABI 1.24 appends `FrameIn::environment`: safe-area insets (top/right/bottom/left), keyboard
start/target rectangles, host-clock start time, duration, raw UIKit curve and did-show/did-hide
visibility. `Ui::environment()` reads it inside cached views; a changed environment invalidates
those views. The frozen 48-byte and 56-byte descriptors keep their layout and use a default
environment. Forward-sized old descriptors ignore an incomplete environment tail. Invalid
geometry is rejected before changing the frame. `EnvironmentChanged` is a numeric input event,
so native geometry changes wake an otherwise idle UI. Retained event buffers start with eight
slots for combined focus/visibility/keyboard batches and still grow for larger peaks.

The iOS host reads [SDL's safe-area query](https://wiki.libsdl.org/SDL3/SDL_GetWindowSafeArea)
and converts keyboard frames from their UIScreen into the SDL view's logical coordinates,
following [Apple's coordinate-space guidance](https://developer.apple.com/videos/play/wwdc2023/10281/).
It preserves UIKit curve values such as 7 and a stable mapping to the frame clock. The framework
clears SDL's automatic keyboard avoidance area, keeping the Metal view fixed while layout uses
the environment; the authored caret remains available in the UI's IME output. The gallery probe
pads its content by the safe-area insets.

ABI 1.26 appends `FrameIn::keyboard`, a 20-byte current presentation rectangle and validity/motion
flags. The old 48-, 56- and 128-byte frame prefixes and 72-byte `Environment` remain unchanged;
incomplete keyboard tails are ignored. Invalid geometry is rejected before mutation.
`Ui::keyboard()` exposes the snapshot; `Ui::content_rect()` combines safe areas and a full-width
bottom-occluding keyboard. Cached views invalidate when it changes, and motion requests another
frame until the native guide settles. Generic modals center within these usable bounds.

The iOS host samples the presentation layer of an inert view constrained to UIKit's keyboard
layout guide, preserving native motion, including curve 7. There is no Rust animation approximation.
`SIMCTL_CHILD_PXUI_IOS_KEYBOARD_CHECK=1` puts a field at the content bottom, shows/hides twice
(including measured frames), checks its IME bounds and real intermediate native geometry, then
finishes the unchanged 182/82/100 zero warm Rust allocation and idle-wake checks. Refocusing reuses
editor storage and undo history; routed keys choose a spare modifier buffer with enough capacity.
Manual touch editing, interactive IME candidates, microphone dictation, floating-keyboard field
avoidance and external-screen/device behavior remain unqualified. A floating keyboard is exposed
by `keyboard()` without shrinking global content.

ABI 1.27 adds `Ui::text_field_with_options` and `Ui::text_input_options`. `TextInputOptions` selects
autocorrection (0/1), capitalization (none/sentences/words/letters, 0..=3) and Return/Done/Search
(0..=2). `SEARCH_FIELD` disables autocorrection and capitalization. Done clears focus and IME
output in the same frame; Search leaves submission to the app. Raw key queries remain
non-consuming, so applications must arbitrate shortcuts against focus at the start of the frame.
The original `text_field` keeps its default traits. No existing descriptor layout changes.

The iOS host observes SDL's hidden UIKit field for marked-text updates; pxui still paints the
visible editor. It preserves Unicode scalar selection offsets, replaces the selected base text
with preedit, and positions the caret within it. The native queue owns copied payloads, bounded to
64 events / 1 MiB, rejects stale generations and reports overflow as a host error. Other UIKit
fields are ignored. `SIMCTL_CHILD_PXUI_IOS_TEXT_CHECK=1` exercises native UITextInput methods,
traits, commits, cancellation, deletion, Done and queue lifetime/limits before the original
182/82/100 warm Rust allocation and idle-wake checks. These simulator fixtures post change
notifications explicitly; they do not qualify real candidate interaction or microphone dictation.
Changed owned text and Apple's native text system remain separate allocation paths.

The shape of an app (names settle with the engine rewrite; `crates/private/pxui_render/tests/gpu_pxui_render.rs`
and `crates/private/pxui_core/tests/gpu_pxui_parity.rs` hold the compiling version):

```rust
let mut win = pxui_sdl::Window::open("demo", 900, 600)?;          // or pxui_wayland
let gpu = ogpu_metal::device()?;                                  // or ogpu_vulkan
let mut ui = pxui_core::Ui::new(/* theme */);
let mut painter = pxui_render::Painter::new(&gpu, format, depth, 1, mask_size, color_size)?;
let mut events = Vec::with_capacity(256);
while win.pump(&mut events) {
    let frame = /* events, win.size(), win.scale(), win.time() */;
    let out = ui.frame(&frame, &mut |ui| app(ui));                // out.draw: layers, out.platform: cursor, IME, clipboard, repaint time
    painter.begin_frame();
    /* open a render pass on the window's target */
    painter.draw(&pass, size_px, scale, instances, breaks)?;
    /* present, then apply out.platform */
}
```

Input is data in, output is data out: there are no callbacks into the platform.

## Working here

```sh
cargo check -p pxui_core                 # inner loop
cargo t -p pxui_core [filter]            # never raw `cargo test`: cases die at 5 s, runs at 60 s
cargo t -p pxui_host                    # macOS: frame/image lifetime and native chrome
cargo run -p gate -- deps           # dependency boundaries (`shape` checks code shape)
tools/validate.sh                   # all of it on macOS; --windows / --linux print the commands for those machines
```

Validation is local; there is no hosted CI.

macOS needs `brew install pkgconf sdl3`. Linux Vulkan setup is in [docs/vulkan-linux.md](docs/vulkan-linux.md).
Contributor rules are in [AGENTS.md](AGENTS.md). [MIT license](LICENSE).


ABI 1.28 makes `Text` and `UiText` measurement, width, rasterization and quad methods keyless.
The library derives layout identity from text, complete style, width and spans, and retains at
most 4,096 layouts per shaper, including standalone use without frame calls. Existing C calls
select this behavior with key=0; nonzero legacy keys still work. Warm measurement reuses storage;
new layouts and owned raster/quad results allocate.

ABI 1.29 adds `ui::FIT_TEXT_PIXEL` (`PXUI_FIT_TEXT_PIXEL` in C). A Fit text node rounds its
intrinsic width, including text insets, to the frame's rendering density; explicit min/max bounds
still apply. It retains a validated text fingerprint across builds and shares layouts with equal
labels, so unchanged nodes do not hash their text again. It supports plain, line-box and inline
span text, caps and RTL. `geom::hash::FastHasher` exposes the existing internal hash primitive for
UI revisions and identities.

ABI 1.30 adds `Ui::animate_keys_repeat(&view, &keys, Repeat::Forever, autoreverse, start_offset)`
(`pxui_ui_animate_keys_repeat` in C). `Repeat::Count(n)` counts complete cycles, including the
reverse leg when enabled; zero is refused by the Rust wrapper and means forever in C. Keyframes
need positive total duration; the finite nonnegative offset is seconds into the first cycle.
Hidden/clipped views request no frames; their timeline continues when another event reveals them.
Retarget with `animate`/`animate_keys` to stop. Reduced motion settles at the terminal pose, the
first stop for autoreverse. Scale-only loops change transform entries without rebuilding content.

`Ui::box_of_a11y(id)` resolves a published accessibility identity to its live node without exposing
slot encoding to hosts. Unknown or retired identities return `None`, including after slot reuse.
Use ordinary `Ui::rect` on that handle to inspect its displayed, transformed geometry.

ABI 1.31 adds `Ui::scrub(label, value, range, step) -> Scrub` for caller-painted horizontal
controls. The result carries `node`, `preview`, `commit` and `held`; use `set_node_layout` and
`draw` on the node. Pointer capture previews across the full node and commits on left release;
ordered Arrow/Home/End/Page presses and accessibility adjustments take over capture and commit directly. Page steps
are ten ordinary steps; command-modified keys stay available to the app. RTL reverses horizontal
mapping and arrows. Collapsed ranges are read-only and cancel active capture. Values/range/step
must be finite, ranges ordered with finite span, and step positive. Focus/modal/retirement and
pointer/touch cancellation never manufacture a commit. Warm input/build/draw reuses routed storage.
The C entry validates the size-prefixed 48-byte `PxuiScrubOut` before writing its node/flags/values.


ABI 1.32 adds `Ui::reveal(&list, index)` for fixed/measured lists, sections and grids. It scrolls
only enough to expose that item, accounting for padding, sticky headers and the current viewport;
an oversized item aligns its leading edge. Measured rows use cached heights and estimates for
unvisited rows. Requests made during construction settle after layout; requests between frames
invalidate the containing retained view. Unknown indices and stale handles do nothing. The request
and warm build/draw allocate nothing; cold list creation and new measurements keep their existing
storage/measurement costs.


ABI 1.33 adds `Event::FILES_DROPPED` (26): x/y are logical points, code 0 carries NUL-separated
UTF-8 paths and code 1 carries an error. No C record changes. `input::Event::FilesDropped` holds
`position` and `paths: Result<Vec<String>, String>`. SDL retains an ordered set across pumps and
publishes it once on completion. Empty/cancel delivery remains `FileDragEnded`; focus loss only
hides hover. A first file without preceding hover announces its position so callers can capture
application context immediately. A new native Begin cancels an abandoned set. The host rejects
a whole set above 1,000,000 files or 64 MiB of path text, or with empty/NUL-containing paths.
Apps retain their own path and import-policy validation. Actual file payloads allocate; warm
empty/hover/cancel pumps reuse storage. Legacy single-file wire events remain available.


`hash!(value)` derives transient UI identities/revisions using the framework's existing fast hash,
evaluating the expression once and borrowing it. Applications supply the facts, rather than
constructing hashers. Values remain identical to the previous FastHasher calls; this is not a
DoS-resistant map hash. It adds no C ABI surface.

Apple profiling helpers live in `mem_sys::profile`: `thread_cpu_time_ns`, initial-thread-only
`ui_thread_priority` and the RAII `FrameInterval` for optional Instruments intervals. The log
subsystem is `org.pxui.profile`, with the existing `UI build` frame/wall/CPU/QoS fields. Warm Rust
counters cover these calls; native allocations and scheduling still require separate evidence.
Native texture inspection is test-only and crate-private. The host's main-thread test executable
builds inside the library, retaining pixel, full-batch replay and GPU-completion lifetime checks.


ABI 1.34 adds `Ui::text_layout_count()`: the cumulative number of text layouts performed after
cache misses, including changed content, style, width or spans. Read it between frames and compare
two values to audit cold shaping. Cache hits leave it unchanged; the query allocates nothing.
It measures layout work, not glyph rasterization, native allocation or all process memory. No C
record changes; C rejects null outputs before writing and retains the normal owner/callback guards.
