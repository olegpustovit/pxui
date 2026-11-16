# ABI history

The current wire contract is [pxui.v1.txt](../abi/pxui.v1.txt); numeric layouts are in
[layout.v1.txt](../abi/layout.v1.txt). Historical rows stay frozen. Change the exports,
header, wrapper, snapshots and C layout assertions together; rebuild app and dylib together.
Static/dynamic validation, pixel and warm-allocation checks remain required.

## Policy

**Version.** `pxui_abi_version()` returns `major << 16 | minor` (`ABI_VERSION` in `pxui_abi`, mirrored by the `minor N` line of
`abi/pxui.v1.txt`). The wrapper refuses a library whose major is not 1. Minor grows by one for each additive change.

**Promise.** An app built against 1.N runs unchanged against any later 1.M library (M >= N). "Unchanged" covers: every export
of 1.N with its signature; every record layout and field offset; every flag, enum and code value; the behaviour of existing
calls on inputs they accepted; and identity hashes (text content/style keys, widget and node identities) once they have been
frozen by a released minor. A call added after N is simply absent from an older library; the app checks the minor before using it.

**Append-only rules.**

- Exports are never removed, renamed or re-signatured. A different shape is a new export.
- Records are `repr(C)` with `size: u32` first. A record grows only at its end (a record that grows at its end updates its own size row and appends its new field offsets; existing offsets never change); the library reads the caller's `size`, refuses a
  shorter record (`read_desc!`, `sized!`) and copies no more than it knows, so a caller built against a larger or smaller
  layout is never over-read. Existing fields keep offset, size and meaning.
- Enum and flag values are never reused or renumbered. A retired value stays reserved.
- `abi/pxui.v1.txt` and `abi/layout.v1.txt` change only by appending lines; each appended line has a row in the table below, and
  the `abi-snapshot` and `abi-header` gates check them. Historical rows stay frozen.
- One minor bump per additive change, made in the same commit as the export, header, snapshots and docs row.

**Reserved flag bits.** Node flags (`Flags`, `u64`) use bits 0-21 today (highest is `FIT_TEXT_PIXEL`, 1 << 21). Bits 22-62 are
reserved for future minors and must be passed as zero. Bit 63 is unassigned.

**Extension set.** The Rays vocabulary and diagnostics (`inspector_*`, `line_field`, `value_field`, `picker`, `diagnostics`)
are not part of the promise. They are to live in a separate manifest, `abi/pxui.ext.txt` (moved there by DX7.8), which carries
no stability guarantee: they may change or disappear between minors, and an app pins the commit if it uses them.
Until DX7.8 lands they are still listed in `abi/pxui.v1.txt` and are frozen like the rest.

**Adding a call.** In one commit: the `pxui_abi` export; the wrapper method in `pxui`; the declaration in `include/pxui.h`;
the appended row in `abi/pxui.v1.txt` (and `abi/layout.v1.txt` for a new record); the row in the table below; the
`ABI_VERSION` bump with the `minor` line. Then run the `abi-header` and `abi-snapshot` gates.

**Deprecation.** A deprecated call is kept forever and keeps its behaviour. Mark it in its docs row (and `#[deprecated]` on the
wrapper) and name the replacement; nothing is removed from the manifest.

| Version | Field/function | Purpose |
| --- | --- | --- |
| 1.0 | Initial public UI/text/renderer ABI | Opaque owners, sized descriptors and callback-scoped borrows. |
| 1.1 | `view_backdrop_saturation` | Control saturation after backdrop blur. |
| 1.2 | `renderer_resize` | Prepare reusable blur targets before command recording. |
| 1.3 | `TextStyle.features` | Enable OpenType tabular numbers without changing style size. |
| 1.4 | `TextSpan`, `TextSpans`, inline-text calls | Apply bounded UTF-8 text spans and decorations. |
| 1.5 | `TextStyle.ellipsis` modes | Add head and middle ellipsis, retaining full accessible labels. |
| 1.6 | `shaper_system_fonts`, `text_system_fonts` | Enable native missing-cluster font discovery explicitly. |
| 1.7 | Atlas codes 2/3, `GLYPH_PAGE_2` | Add second bounded mask/colour pages without draw breaks. |
| 1.8 | `FrameIn.appearance/reduced_motion`, theme calls | Cache host preferences and replace parsed themes between frames. |
| 1.9 | Retained view controls, `list_measured` | Expose opacity/scale/clip and variable-height virtual rows. |
| 1.10 | `Ui::signal_state` | Read scalar keyboard state while retaining routed event buffers. |
| 1.11 | `carrying_ref`, `drop_target_ref` | Borrow carry/drop payloads without owned copies. |
| 1.12 | `transfer_carry` | Keep capture on a stable ancestor as virtual rows retire. |
| 1.13 | `renderer_forget_image` | Release interned native wrappers after completed sampling serials. |
| 1.14 | `Paint::fill_fractional` | Preserve subpixel rectangle coverage in the shared shape writer. |
| 1.15 | `ui_key_down` | Query raw held keys without consuming or allocating events. |
| 1.16 | `ui_now`, retained view offset | Expose host time and compose retained motion transforms. |
| 1.17 | `ui_text_line_box` | Give text explicit line metrics and intrinsic block dimensions. |
| 1.18 | `TextStyle::WRAP_WORD_SPACES` | Retain omitted separator advances in fitting metrics. |
| 1.19 | `ui_density` | Expose the existing integer rendering density. |
| 1.20 | `ui_section_list` | Combine scrolling lead-in, pinned heading and virtual rows. |
| 1.21 | `ui_section_list_measured` | Support wrapped measured rows in section lists. |
| 1.22 | Accessible text values, analytic play shape | Expose text semantics and one-instance triangle painting. |
| 1.23 | `ui_key_pressed_with` | Query non-consuming press-time modifier combinations. |
| 1.24 | `FrameIn.environment` | Append safe-area/platform environment to frozen frame prefixes. |
| 1.25 | Numeric `Touch` event payload | Deliver typed bounded contacts without mouse emulation. |
| 1.26 | `FrameIn.keyboard`, usable-content queries | Expose keyboard geometry and docked usable bounds. |
| 1.27 | `TextInputOptions`, text-input option calls | Configure native text traits and return-key behavior. |
| 1.28 | Keyless Rust text service; C key=0 derives content identity | Share bounded content/style/width/span layouts without caller hashes. C signatures stay unchanged. |
| 1.29 | `FIT_TEXT_PIXEL` / `PXUI_FIT_TEXT_PIXEL` | Round intrinsic text width to render density; retained nodes reuse validated text fingerprints. No descriptor layout change. |
| 1.30 | `ui_animate_keys_repeat`, `kit::Repeat`, `box_of_a11y` | Repeat retained keyframes with counted/forever cycles, autoreverse and phase offset. Resolve live nodes from published accessibility identities; no descriptor layout change. |
| 1.31 | `ui_scrub`, `Scrub`, 48-byte `PxuiScrubOut` | Caller-painted horizontal adjustment with captured preview/release commit, routed keys and accessible values. |
| 1.32 | `ui_reveal` | Minimum-scroll reveal using retained fixed/measured list, section and grid geometry. |
| 1.33 | `FILES_DROPPED` event kind 26 | Completed host file sets with logical position, ordered paths or a whole-set transport error. |
| 1.34 | `ui_text_layout_count` | Observe cumulative text cache-miss layouts without allocating or exposing the engine. |
| 1.35 | `ui_phase_cpu_times` | Opt-in Apple thread CPU timings for seven engine phases, summed over settling passes; unavailable clocks refuse without output writes. |
| 1.36 | `text_fork`, `shaper_prepare`, `text_install`, `prepared_destroy` | Worker-owned shaping and bounded, allocation-free installation between frames; the same record returns retired storage to its worker. Font-generation mismatch refuses without mutation. |
| 1.37 | `flags::LIGHT` / `PXUI_LIGHT` (MASK instance bit 4), `Instance::light` (`light_extra` packs the words) | Apple-raster glyphs ask the renderer for CoreText light-on-dark stem growth. `extra[0]` packs two f16: ppem in device pixels (low half) and device scale (high half); `extra[1]` is 0. The shader derives round(4 * linear Rec.709 luminance of the instance colour), grows the quad one pixel and adds the extra coverage from neighbouring texels. Renderers that predate 1.37 ignore the bit and `extra`, drawing the plain mask; no layout, export or signature change. |
| 1.38 | `ui_gesture`, `Gesture*`, 16-byte `PxuiGestureIn`, 20-byte `PxuiGestureOut` | Touch-only UIKit gestures on a node: delayed highlight and long press, horizontal row swipe with 8 pt axis lock, interactive leading-edge back, pull to refresh with a held refresh state. Closed-form springs on the frame clock; one-frame pulses; no descriptor layout change. |
| 1.39 | `PxuiTransition`, `PxuiOptTransition` replace `PxuiCurve`, `PxuiOptCurve`; `PxuiNode` 488 bytes | A node's `transition` is a Bezier or a spring (24-byte value), not a `Curve`: the 128-byte sampled table made every node copy 112 bytes longer. `Ui::animate`, keyframes and `PxuiCurveDesc` still take tables and steps (unchanged, 160 and 344 bytes). Version 1 is unreleased, so the layout rows change in place. A record from before 1.39 (`size` 488 or 600) is still accepted (`size` is a minimum) and read at the same offsets; only a node whose `transition` was a table or steps, which existed since the unreleased inline-table change, is now refused as an invalid tag. Apps and the dylib must rebuild together. |
| 1.40 | `ui_swipe_actions`, 16-byte `PxuiSwipeIn`, `PxuiGestureOut` flags 512/1024/2048/4096 | Row swipe with actions on both sides and a full-swipe commit: leading/trailing widths (0 disables a side, mirrored right to left), a commit distance, a one-frame commit pulse per side and an open flag per side. `PxuiGestureIn` is unchanged; records using only `reveal` keep their trailing-only swipe. A side without actions no longer takes the touch (a swipe row with `reveal` 0 scrolls instead). |
| 1.41 | `ui_dismiss`, 12-byte `PxuiDismissIn`/`PxuiDismissOut` | Vertical drag-to-dismiss on a node for full-screen players and bottom sheets: a downward touch drag past the 8 pt axis lock (handed over by an inner vertical scroll only at its top) moves a `dismiss` offset; a release past a fraction of the node height (default 0.25) or at 600 pt/s commits for one frame and settles to the height, else springs back; upward drags rubber-band at 0.55. `PxuiGestureIn/Out` are unchanged; the flags are 8192/16384/32768. |
| 1.42 | `PxuiGestureIn.kinds` bit 5 (32), touch drag | A node taking the whole touch at once, like a mouse press: Moved/Pressed on touch-down, pointer moves while held (capture keeps the node), release on lift. No scroll arbitration, so a scroll view around an index strip or scrubber does not scroll. No layout change; older libraries refuse kind 32. |
| 1.43 | `ui_set_colors`, `ui_color`, `paint_color`; Rust `theme::ColorToken` | App-defined colour tokens: a fixed 64-entry table of light/dark `PxuiColor` pairs installed between frames (two parallel arrays, count 0 clears; a changed table rebuilds every view once). Reads pick the dark entry while the theme in effect is dark (`theme::dark`: panel channel sum below 384; `themed` scopes decide inside them, painters use the root theme), so a light/dark theme swap changes them with no app rebuild; past the table they read transparent. No record or layout change. |
| 1.44 | `ui_button_with`, 96-byte `PxuiButtonDesc` | The kit button with options: size (kit, points, fill), enabled, selected, danger ink, accessible name, placement (start, centre, end), extra padding, six looks (or the theme's) and an optional build callback that replaces its text inside the box. Reports the box, its signal and focus. `ui_button` and `ui_button_styled` are unchanged; `PxuiButtonConfig` (what a button body is given) grows at its end by the size, placement and padding the caller asked for. |
| 1.45 | `ui_label`, `paint_label`, `paint_label_chars`, 64-byte `PxuiLabelStyle`; `PxuiLabelConfig` 56 -> 120 bytes | Every label is built through the label body with a style of one meaning: a field whose `PXUI_LABEL_SET_*` bit is clear is the theme's label style (the label look's font, size, weight, tracking in ems, capitals and colour; full width; the row height per line, 0 for 1.2 times the size; 12 points in with 4 above a parent's first child). `PxuiLabelConfig` grows at its end by `style`, the style as the caller gave it, and a `digits` flag asks for a number drawn one cached character layout at a time with digits on the advance of `0`. `color` is an app colour token. `pxui_ui_label` keys every label by its place among its siblings, and `pxui_label` now does too (it is `pxui_ui_label` with nothing set). `paint_label`/`paint_label_chars` paint text in the same style, resolved against the painter's root theme; `paint_text_styled` is unchanged. The whole style is validated before anything is built or drawn. |
| 1.46 | `ui_icon_button_with`, 96-byte `PxuiIconButtonDesc`, `PxuiIconDraw`; `PxuiIconButtonConfig` grows to 128 bytes | The kit icon button with options: size (kit square, points, fill), enabled, six looks (or the theme's), a press scale and the icon (a glyph or an app drawing: a callback and user data called at paint time with the 20-point square and the look's ink); the body config carries the looks, size kinds, press scale, drawing and ink at its end. The default body springs its plate and glyph to the press scale while held. `ui_icon_button` is deprecated: use `ui_icon_button_with` (kept, unchanged). |

Frame extensions retain the frozen 48/56/128-byte prefixes. Validate leading sizes, active
records, flags and UTF-8 boundaries before mutation; partial tails never justify reading
past supplied storage. Heap ownership and unwinding never cross the ABI.

## Version notes

### 1.10

ABI 1.10 reuses routed signal/modifier buffers. The Rust `signal_state` convenience uses the existing
`pxui_ui_signal` function and returns the same scalars with empty owned keys; `signal` still copies
all key/text/IME events. Do not discard consumed activation keys to remove allocations. Preserve
per-event modifiers, stale-handle quietness and static/dynamic warm keyboard tests. The ABI encodes
borrowed core events into its retained chunks, reserving all text bytes before publishing pointers.
Signal/modifier pools grow only to the peak events/signals seen; owned text and legacy key-vector
reads remain separate allocation paths. No C function signature or layout changed in this minor.

### 1.11

ABI 1.11 adds borrowed Rust `carrying_ref`/`drop_target_ref` conveniences over the existing C exports.
The ABI copies query strings into retained slots: C borrows survive payload replacement/cancellation
until the next frame. Owned getters still copy. Core reuses gesture strings and moves released
payloads; the ghost temporarily owns its payload while copying text into frame storage. Preserve
whole-gesture static/dynamic zero-allocation checks and the C borrow-lifetime regression. Cached
views must propagate size reflow through their descendants and invalidate enclosing caches when a
virtual grid's build-time size estimate changes; keep the nested-grid same-frame resize check.

### 1.12

ABI 1.12 adds `transfer_carry` to move a pointer carry's capture to a live hit ancestor. Virtual
children may then disappear; the ancestor disappearing or losing capture still cancels it. Standard
scroll containers follow pointer edges in either axis, including RTL. Preserve time-based bounded
scrolling and the two UI-owned subpixel remainders (no extra per-slot storage), plus static/dynamic
modified-input/cancellation tests and the 100,000-cell edge/precision/allocation regression. Release
and explicit cancellation must not click the capture owner. Validate owner presence before painting
the ghost so an omitted container leaves no ghost in the same frame. Retired slots clear text/name/
value contents while retaining their buffers for reuse before the next slot identity is assigned.

### 1.13

ABI 1.13 adds Renderer::forget_image / pxui_renderer_forget_image. Hosts stop referencing an evicted
image and forget it on every sampling renderer before destroying/reusing the native handle, passing
the host's actual completed serial on the prepare timeline; completion must not regress. Refuse incomplete draws and never destroy the caller's
image. Native identity interning is an O(1) LRU bounded to 1,024 entries and a 128 MiB estimated texel
working set (one oversized image may stand alone). Wrappers retain weak image identities and retire
abandoned entries by actual completed serials. Ignore inherited/trailing image breaks whose segment
has no IMAGE instances; solid/text runs must not refresh resource use or bind stale textures. Preserve
the 128-distinct-image native pixel/warm allocation test, explicit release guards, metadata bounds,
and both backend paths. This is no process/native-memory qualification.

### 1.14

ABI 1.14 adds `Paint::fill_fractional` in the shared shape writer. `fill` keeps its pixel snapping;
fractional fills preserve finite, increasing bounds and use RECT flag AREA (bit 2) for pixel-area
coverage on axis-aligned fills. Keep Metal/GLSL and checked-in SPIR-V together. Preserve the 1x/2x
native thin-bar area/pixel, one-draw and warm-allocation checks, plus static/dynamic descriptor tests.
Derivative inputs must be evaluated before fragment clipping/discard. No C layout changed.

### 1.15

ABI 1.15 adds `Ui::key_down` / `pxui_ui_key_down`, a scalar raw held-key query that respects
the existing owner/callback guards but does not filter widget or modal ownership or consume keys.
Event-only frames retain held keys until release/focus loss; full Input frames replace them with
the host snapshot. Preserve cross-frame modifiers, focused/modal queries, release/focus-loss and
static/dynamic zero-allocation checks. Popup filtering must recycle rejected accumulators and their
modifier buffers into the existing pools rather than dropping warmed storage. Hosts accepting a
custom popup result should dismiss after the popup call returns, before processing another input.

### 1.16

ABI 1.16 exposes the engine's host-supplied frame clock as Ui::now / pxui_ui_now.
Offset/opacity/scale on an indexed retained view compose in its table entry, including nested hit
and accessibility rectangles. They must leave cached draw instances unchanged and stop drawing
when settled. Same-time keyframe channels form one pose before interpolation; the last curve at
that timestamp shapes the shared segment. Preserve core and static/dynamic wrapper regressions.

### 1.17

ABI 1.17 adds Ui::text_line_box / pxui_ui_text_line_box. Positive explicit line heights use
rounded primary-face ascent/descent and floored half-leading, with ULP snapping at integer
boundaries. Preserve legacy text/text_spans positioning. Only explicit line-box nodes derive
Fit dimensions from shaped text; empty labels retain one line and resolved Grow widths settle
wrapped height in the same frame. Preserve invalid-input atomicity, static/dynamic resize/empty
checks and warm allocations. Font metrics are normalized once when a font is loaded.

### 1.18

ABI 1.18 adds TextStyle::WRAP_WORD_SPACES (3), retaining logical blank advances omitted between
visible soft-wrapped runs in fitting metrics. Preserve normal word/glyph/no-wrap metrics and all
visible glyph positions. Reuse the text engine's row scratch and cached metrics; inspect blank
shape glyphs against monotonically ordered logical run bounds, never re-shape substrings or scan
every paragraph once per line. Hidden/truncated paragraphs do not contribute space advances.
Preserve static/dynamic whitespace, paragraph, BiDi, glyph equality and warm allocation checks.

### 1.19

ABI 1.19 exposes Ui::density / pxui_ui_density: the existing integer rendering density shared
with Paint, rounded from host scale and clamped to at least one. Do not change the engine's
fractional-scale policy here. Preserve initial/owner/callback queries, cache invalidation when
density changes, equal-density retention and static/dynamic warm allocation checks.

### 1.20

ABI 1.20 adds Ui::section_list / pxui_ui_section_list: one scrolling lead-in, optional pinned
heading and fixed-height virtual rows, with an optional overlay scrollbar. Reuse the existing
sticky, blocking and scroll layout; keep row selection O(viewport rows), never allocate per count.
C callbacks receive SIZE_MAX for the lead-in, SIZE_MAX-1 for the heading, otherwise a row index.
Validate the whole descriptor before building anything; preserve callback guards and recovery.
Culled cached views still update their composed geometry and accessibility visibility, without
keeping offscreen draw runs. Preserve indexed/flat/clipped heading scrolling, the 570-row storage
bound, native 1x/2x opaque heading/right-edge pixels, click blocking and warm allocation checks.

### 1.21

ABI 1.21 adds Ui::section_list_measured / pxui_ui_section_list_measured. Reuse the existing
measured-list prefix cache and binary search, including same-frame measurement settling; the row
height is an estimate for unseen rows. The callback receives the logical content Y of each part.
Warm height changes/scroll/shrink allocate nothing; storage is two floats per row plus one prefix
entry. Preserve static/dynamic descriptor, callback recovery, native 1x/2x and one-draw checks.

### 1.22

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

### 1.23

ABI 1.23 adds `key_pressed_with`: a raw, non-consuming key query with Shift/Ctrl/Alt/Meta held at
the press, including a modifier released later in the same batch. Capture the initial modifier state
before routing and replay event order, clearing on focus loss. Callers still arbitrate modal ownership.
Keep C invalid-input output atomicity and focused/modal static/dynamic warm-allocation checks.

### 1.24

ABI 1.24 appends the 72-byte Environment to FrameIn, retaining both frozen 48/56-byte prefixes.
Only a complete new tail is read; forward-sized legacy descriptors ignore partial new tails.
Validate finite nonnegative insets/extents/duration, finite clock arithmetic and visibility 0/1
before mutating input/environment/output. Input's snapshot entry resets the environment to its
default. Keep static/dynamic cached-view invalidation, unchanged-cache, warm allocation, invalid
output atomicity and layout checks. UIKit curve 7 is data, not a standard easing enum. Keyboard
notifications must match the window's screen, use view coordinates and map their clock once.

### 1.25

ABI 1.25 carries numeric Touch payloads (48 bytes) through Event.text for kind 25; this is never
UTF-8. Preserve full 64-bit identities, source timestamps, all four phases and outside-window
positions. Reserve typed arrays before publishing their pointers, and reuse host/routed buffers.
Reject invalid payloads before mutating frame state/output in both entry points. Preserve native
SDL trace, adjacent same-finger coalescing and static/dynamic 182/82/100 allocation regressions.
Touch transport coverage does not qualify widget gesture arbitration or real-device feel.

### 1.26

ABI 1.26 adds FrameIn::keyboard and Ui::keyboard/content_rect without changing Environment or the
frozen 48/56/128-byte frame prefixes. UIKit keyboard-layout-guide presentation geometry owns the
animated bounds; apps compose within content_rect. Preserve intermediate geometry, native curve 7,
cached-view invalidation, moving-only repaint, incomplete-tail/invalid-input checks and the native
182/82/100 zero Rust allocation gate, including measured refocus/show/hide and IME bounds. Floating
keyboards do not shrink global content. Editor identity survives blur; retain its buffers/history
and move shelved stacks. Numeric routing picks adequate retained modifier capacity before growing.

### 1.27

ABI 1.27 adds TextInputOptions and text_field_with_options/text_input_options without changing
existing descriptors. Keep default field semantics; Done dismisses focus/IME in the same build,
Search leaves submission to the app, and raw key queries remain non-consuming. Apps must guard
shortcuts against focus at frame entry. Marked text replaces the selected base range; scalar
selection offsets determine its caret and selection. Stream all shaped selection spans without
allocating scratch. Preserve static/dynamic Unicode, commit-plus-Done and warm-refocus checks.
The iOS observer belongs in sdl3_sys and accepts only SDL's owned hidden field. Preserve copied
native queue payloads, generation/window validation, 64-event/1-MiB bounds, cleanup and typed
failure; no borrowed native string may escape a callback. Native allocations and changed owned
text are not part of the zero warm Rust frame claim. The native text probe uses explicit
UITextInput calls/notifications; actual keyboard candidate and dictation interaction need hardware
qualification.

### 1.28

ABI 1.28 makes standalone/UI Rust text measurement and raster/quads calls keyless. C key=0
selects library-owned identity over all text/style/width/span fields; legacy nonzero keys remain
accepted with full equality checks. Drawing and measurement share the identity helper. Retain the
4,096-layout bound without a frame clock, incremental expiry, interleaved-style/span correctness,
short-prefix validation and static/dynamic zero-warm-allocation checks. Owned raster/quads outputs
and new layouts allocate; they are separate from warm measurement.

### 1.29

ABI 1.29 adds FIT_TEXT_PIXEL (bit 21), rounding Fit text width including insets to rendering
density before explicit size clamps. Retained node memos validate against cached text and reuse
its fingerprint across styles/widths; identical labels still share layouts. Preserve slot recycling,
caps, spans, empty text, RTL/asymmetric-inset native pixels, min/max bounds and static/dynamic
zero-warm-allocation checks. Keep legacy plain-text painting unchanged without the flag. Plain
pixel-fit labels preserve the legacy unbounded glyph positioning whenever text fits. FastHasher
is shared through public geom::hash for internal revisions/identities, not untrusted hash tables.

### 1.30

ABI 1.30 adds repeating keyframes with counted/forever cycles, autoreverse and a start offset.
A counted cycle includes both legs. Hidden/clipped runs keep their timeline without requesting
frames; retargeting stops them, and reduced motion settles at the terminal pose (the first stop
for autoreverse). Reuse bounded keyframe storage and the existing transform table: scale-only
loops must not rebuild content or rewrite instances. Preserve static/dynamic geometry, cycle
boundary, visibility, stop, reduced-motion, invalid-packet and zero-warm-allocation checks.

`Ui::box_of_a11y` resolves published identities with generation validation; hosts never decode
slot IDs. Unknown/retired IDs leave output unchanged and do not revive after slot reuse. Geometry
checks for retained animations must inspect transformed Ui::rect values, not static source quads.

### 1.31

ABI 1.31 adds caller-painted Ui::scrub with a size-prefixed 48-byte PxuiScrubOut. Own pointer
capture/mapping, release-only pointer commits, ordered routed Arrow/Home/End/Page keys and numeric
accessibility in the framework. Borrow key/modifier storage; never clone owned signals for warm
scrubbing. Read-only transitions cancel capture/focus. Keep command chords available to apps,
RTL/endpoints, independent new-capture previews/previous-release commits, digital takeover,
modal/focus/retirement/multitouch cancellation and no false release on touch cancel.
Validate value/range/span/positive step, output prefix and UTF-8 before mutation. Preserve actual
short-prefix, forward-tail, callback-ownership, static/dynamic and zero-warm-allocation checks.

### 1.32

ABI 1.32 owns minimum-scroll reveal in the framework. Retain fixed/measured list and grid geometry,
including section insets, sticky occlusion and caller padding. Resolve grid wrapping against actual
layout during resize, and settle requests made inside callbacks after layout without replaying input.
Keep live cached-list geometry beyond the age-based retirement window; stale generations and invalid
indices must stay quiet. Reuse measured prefix storage. Preserve minimum adjustment, resize, RTL,
fixed/measured/static/dynamic and zero-warm-allocation regressions.

### 1.33

ABI 1.33 adds completed FilesDropped events. SDL owns batching across pumps and the copied path
strings until one completion, preserving order and logical coordinates. Bound retained transport
paths to 1,000,000/64 MiB and reject the whole set visibly on overflow; never submit its accepted
prefix. Keep focus loss separate from cancellation and notify first-file position when no hover
preceded it, so application account capture cannot drift. A new Begin cancels an abandoned batch.
Keep application import policy outside the host. Preserve two-pump native delivery, one-shot
consumption, invalid UTF-8/code/position refusal, and zero warm empty/hover/cancel Rust allocations.
Owned successful/error payloads remain cold allocation paths; never drop events to meet the gate.

### 1.34

ABI 1.34 exposes a monotonic text layout-miss counter for qualification. Increment only after the
validated layout cache-hit path, count changed layouts as well as new keys, and never reset it at
frame boundaries. The UI getter borrows through the normal owner guard and rejects unavailable
text/invalid outputs before writing. Preserve static/dynamic cold-versus-cached and zero-warm-query
checks. This counter does not qualify glyph rasterization or native/process allocations.

### 1.35

ABI 1.35 exposes opt-in CPU phase timing through the public API. Supply the native thread clock
from the ABI platform layer; keep the core free of native/GPU dependencies. Sum every settling
pass, reset at the outer frame, reject failed/regressing clocks and preserve invalid-output atomicity.
Existing wall-phase semantics remain unchanged. Retain deterministic clock/settling and static/dynamic
zero-warm-allocation checks. Apple clock support does not qualify other platforms/native allocation.

### 1.38

ABI 1.38 `ui_gesture` owns the UIKit gestures above scroll, in pxui_core::ui_core::gesture/touch and touch
only: 0.1 s delayed highlight and 0.5 s long press inside the 10-point slop, horizontal row swipe after
the 8-point axis lock (toward the actions, or any direction on an open row), leading-edge back from a
20-point band, and pull to refresh on a scroll node (it may stretch short content). Timers run on the frame
clock through `Touches::wake`; swipe and back settle on closed-form critically damped springs
(`gsettle` keeps the frames coming); outputs are per-node retained state plus one-frame pulses, a refresh
pulse is stamped for the next frame because layout fires it. Gesture nodes join the hit list but never
become `topmost`. RTL mirrors the edge, swipe direction and `swipe` sign. Keep the synthetic-trace cases in
tests/cases/gesture.rs and the warm-allocation check; hardware feel stays a device comparison.

### 1.40

ABI 1.40 `ui_swipe_actions` (after `ui_gesture` each build) gives the row swipe actions on both sides: signed
internal `swipe` is positive toward the trailing side, `leading`/`reveal` are the side widths (0 disables a
side), and a release past `full` points of finger travel (unstretched, never on a cancel) pulses
SWIPE_COMMIT_LEADING/TRAILING for one frame and settles closed; open rows set SWIPE_OPEN_LEADING/TRAILING.
`PxuiGestureIn` is unchanged, so `reveal`-only records keep a trailing-only swipe.

### 1.41

ABI 1.41 `ui_dismiss` (after `ui_gesture` each build, which resets kinds) adds the vertical dismiss: mode 7 starts at
the axis lock on a downward drag when no inner scroll holds the contact (absent, or at its top and not gliding), keeps
the offset continuous with a catching spring, rubber-bands upward past 0 and, on release, commits (a fraction of the node
height, default 0.25, or 600 pt/s downward; never on a cancel) with a one-frame pulse, then settles to the height.

### 1.42

ABI 1.42 gesture kind 32 (`GestureOpts::touch_drag`) claims a touch at touch-down: mode 1 (the mouse press/drag path) with
no scroll, long press, swipe, back or dismiss arbitration, for index strips and scrubbers inside scroll views.
