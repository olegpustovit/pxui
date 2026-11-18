# ABI history

The current wire contract is [pxui.v1.txt](../abi/pxui.v1.txt); numeric layouts are in
[layout.v1.txt](../abi/layout.v1.txt). Historical rows stay frozen. Change the exports,
header, wrapper, snapshots and C layout assertions together; rebuild app and dylib together.
Static/dynamic validation, pixel and warm-allocation checks remain required.

## Policy

**Before 2.0: the 1.x ABI is not frozen (D28, owner, 18 Nov 2026).** Until the 2.0 cut, C exports and records change in place and an
export a step supersedes is deleted in that step: no parallel exports, legacy bodies or old-caller tests. `abi-snapshot` is a change
detector only: any change to exports, records or layouts takes the next minor and regenerated manifests (`cargo run -p gate --
abi-snapshot --write`, never by hand), and existing rows may change, be removed or be reshaped. The 1.x frozen fixtures and the 1.57
old-consumer check are retired; `abi-frozen` and `abi-compat` report "no 2.x baseline yet" and pass. The promise, append-only and growth
rules below are the 2.x stability rules: the gate applies the growth rule only to a manifest of major 2 or later, and U5 re-baselines
the manifests and `fixtures/abi/2.N/` at 2.0, when `abi-frozen` and `abi-compat` check again.

**Version.** `pxui_abi_version()` returns `major << 16 | minor` (`ABI_VERSION` in `pxui_abi`, mirrored by the `minor N` line of
`abi/pxui.v1.txt`). The wrapper refuses a library whose major is not 1. Minor grows by one for each additive change.

**Promise.** An app built against 1.N runs unchanged against any later 1.M library (M >= N). "Unchanged" covers: every export
of 1.N with its signature; every record layout and field offset; every flag, enum and code value; the behaviour of existing
calls on inputs they accepted; and identity hashes (text content/style keys, widget and node identities) once they have been
frozen by a released minor. An app built against 1.N does not run against an older library (1.M, M < N): create refuses it (Minimum library, below).

**Append-only rules.**

- Exports are never removed, renamed or re-signatured. A different shape is a new export.
- Records are `repr(C)` with `size: u32` first. **Growth (the rule the `abi-snapshot` and `abi-frozen` gates enforce).** A recorded layout row
  changes only by growing at its end, under a minor bump: the same name and alignment, a larger size, every recorded field offset in place
  and every recorded field's type unchanged (the gates read the field types from the header the row was recorded with: `abi-snapshot` compares
  the committed `abi/pxui.v1.txt` and `include/pxui.h` with the working tree, `abi-frozen` each `fixtures/abi/2.N/` pair), and the new fields appended at or past the
  recorded size (or, with none, the last field a record that itself grew only at its end, as `PxuiBodies` at the end of `PxuiThemeSnapshot`).
  Only two kinds of record may grow: (a) a record the library writes and the app reads, the config a `body_kinds!` line names (the record a registered body receives; a name ending in `Config` proves nothing) and a borrowed snapshot an
  export returns as `const T *` (`PxuiThemeSnapshot`); a reader built against the smaller layout reads its prefix, and a reader built against
  the larger one reads a grown field only when the record's `size` (a config's) covers it, or after checking the library's minor (a
  snapshot's); (b) a record the caller writes whose first field is `uint32_t size` and that the library reads at the recorded size
  (`read_desc!(ptr, T, [.., recorded size, ..])`, or a hand-written reader's `const <RECORD>_V<minor>: usize = <size>` that its size check names: `matches!(size, <RECORD>_V<minor> | ..)`; only library code counts, not comments or `#[cfg(test)]` code). Every other change of
  a recorded row or of a recorded record's field types is refused, an append to a caller-written record without the size prefix included.
  Fields put into a record's padding before this rule (`PxuiFontDesc.size_points` 1.79, `PxuiPanel.padding_points` 1.78) and
  `PxuiUiDesc.abi_built` (1.60, recorded as its own row `PxuiUiDescV1_60`) are pinned in the gate as reviewed exceptions. The library reads
  the caller's `size` and copies no more than it knows, so a caller built against a larger or smaller layout is never over-read. **Recorded sizes:** a record that grew is read only at a size it has had (`read_desc!(ptr, T, [old sizes])`: each earlier
  layout, e.g. `PxuiButtonDesc` 96, 104 and the whole record) or at the whole record and more; a size below the first or ending inside a field
  is refused. The bytes the size covers are read, every grown field is zero, and a grown field or pointer (the icon button's `press`, the toggle's
  `style`, the segmented control's `names`, the button's label) is never read, validated or dereferenced unless the size covers it. A record that has never grown has
  its whole size as its only size. An output record is written only up to the caller's `size`; none has grown yet, so each is refused below its whole size
  (`sized_mut!`), and the first output to grow gets a copy-out of `min(size, whole)` bytes. Existing fields keep offset, size and meaning.
- Enum and flag values are never reused or renumbered. A retired value stays reserved.
- `abi/pxui.v1.txt` and `abi/layout.v1.txt` change only by appending lines or by a row grown under the growth rule above; each such
  change has a row in the table below, and the `abi-snapshot`, `abi-frozen` and `abi-header` gates check them. Historical rows stay frozen
  otherwise. The manifests are written by the gates (`abi-snapshot --write`, `PXUI_LAYOUT_WRITE=1`), never by hand; `abi-snapshot` fails
  while `abi/pxui.v1.txt` differs from what it would write.
- One minor bump per additive change, made in the same commit as the export, header, snapshots and docs row.

**Reserved flag bits.** Node flags (`Flags`, `u64`) use bits 0-21 today (highest is `FIT_TEXT_PIXEL`, 1 << 21). Bits 22-62 are
reserved for future minors and must be passed as zero. Bit 63 is unassigned.

**Limits.** An untrusted caller (a compositor hosts many apps) can make no export panic, abort, poison its `Ui` or allocate without
bound; every limit refuses with an error, never cuts. A test calls every export the manifests list, outside a frame and inside one,
with zero, null, garbage and oversized arguments, and fails on any panic, `PXUI_ERR_PANIC` or poisoned `Ui` (`pxui_abi` `fuzz.rs`; a
name missing from it fails too). The export macro's `catch_unwind` is the backstop for a library bug, not a way to refuse an argument.
Since 1.66:

| Input | Cap | Past it |
| --- | --- | --- |
| An identifier: a key (`PxuiKeyedDesc.key`), a node or container label used as identity, a body, theme-file or bridge name, a carry kind | 256 bytes (`MAX_KEY_BYTES`) | refused; the Rust wrapper panics (a programmer error: keys are `&'static str` or ids) |
| Display text: labels, titles, accessible names (`PxuiKeyedDesc.name`) and values, tips, menu and list rows, hints, options; a node's text with spans or as a line box (`ui_text_spans`, `ui_text_line_box`, `ui_label_text_spans`, `ui_label_text_line_box`) | 64 KiB (`MAX_LABEL_BYTES`) | refused from C; the Rust wrapper clips it at a character boundary first (user data never panics) |
| Editor text: a text field's or area's value, a splice, a paste, an edit, a theme source, text the text service measures or shapes (`text_width`/`_spans`, `text_quads`, `text_rasterize_rgba`, `ui_label_measure`, `ui_label_quads`, `ui_label_rasterize_rgba`, the shaper's); one event's text | 16 MiB (`MAX_TEXT_BYTES`) | refused |
| A text prepared off the UI thread (`shaper_prepare`) | 1 MiB (`pxui_text` `MAX_PREPARED_BYTES`) | refused |
| `text_prewarm` / `ui_label_prewarm` text | 16 KiB | refused |
| A font; fonts per text service; font bytes per text service (1.63) | 32 MiB; 64; 256 MiB | refused |
| A call's string span (options, rows, titles, hints, a text area's messages and splices); one frame's event text (or one `ui_wants_frame` call's) | 16 MiB together (`MAX_TOTAL_TEXT_BYTES`) | refused |
| Strings the library hands back between two frames (edited text, lent payloads, a picker query) | 4096 and 64 MiB (`MAX_RETURNED`, `MAX_RETURNED_BYTES`) | `PXUI_ERR_STATE` until the next frame |
| `FrameIn.width`/`height`, `PxuiInput.logical_size`, a resize or pixel-size event | `0..=65536` (`MAX_SURFACE`) | refused |

**Error code of a cap.** An argument past its cap is `PXUI_ERR_ARGUMENT` on every path (C exports, trusted entries, the text service; a
call that returns a value answers its failure value: null, 0, `SIZE_MAX` or `UINT32_MAX` as its row says), with the reason in
`pxui_last_error`. The one budget that is not an argument's, the strings handed back since the frame began, answers `PXUI_ERR_STATE`
(the call is not allowed now; the next frame resets it). Nothing is cut on the C side; only the Rust wrapper clips display text.

- Events (`frame`, `ui_frame`, `ui_wants_frame`: one validator): `x`, `y` and `value` finite for every kind; text editing start and
  length not negative; a resize or pixel-size change within `0..=65536`; a known kind and code. `FrameIn.width`/`height` and
  `PxuiInput.logical_size` within `0..=65536` points (`MAX_SURFACE`); a held key code that names no key is refused as the event path
  refuses it.
- Builders (every call that declares nodes: widgets, containers, `ui_node`, `ui_box`, `ui_draw`, `part`, ...) answer `PXUI_ERR_STATE`
  outside a frame callback, before the engine is touched (`enter!` and the `build` export form); the `Ui` is not poisoned.
- A shaper, a theme and an accessibility bridge whose call panicked are poisoned like a `Ui`: every later call answers its failure value
  at once; destroy still frees them.
- Calls are counted per `Ui` (`ui_calls`, extension set): no counter is shared between `Ui`s or threads; `counter(0)` reads 0.
- A shaper needs a font: `shaper_create` with none (or only refused ones) is null, never a panic at its first text.

**Version and features.** `PxuiUiDesc.abi_built` (the record's 1.60 tail) is the `major << 16 | minor` the app was built against (`PXUI_ABI_BUILT`;
`pxui_types::ABI_BUILT`, kept equal to `ABI_VERSION` by a test). **Minimum library (from 1.84).** Create checks it against the library's own version (nothing
is fixed to major 1): another major, or the same major with a newer minor than the library's, returns null (the Rust wrapper's `Ui::new` returns
`Error::Version` first); 0, a 12-byte record, is unstated. An app therefore runs only on a library of its own minor or newer, so the library never reads a
caller record grown past what it knows and an app never reads a library-written record (a body config, a snapshot) the library did not write at that size.
A newer library serves an older app through the size rule (records read at their recorded sizes) and `pxui_supports` answers for what an app may use.
`pxui_supports(id)` returns 1 when the library, as built, has the feature with that id and 0 otherwise or for an id it does not know. Ids (`PXUI_FEATURE_*` in
`pxui.h`, `pxui_types::feature`, `pxui::Feature`) are append-only, never reused or renumbered, and 0 is never an id; a feature whose presence depends on the build
(the accessibility bridge on Linux, the Metal and Vulkan backends) answers for the library that was built; a feature of a minor is present in every build of it.

| Id | Name | Since / meaning |
| --- | --- | --- |
| 1 | `A11Y_BRIDGE` | `pxui_a11y_*`: the AT-SPI2 bridge (Linux builds) |
| 2 | `METAL` | the Metal backend (Apple targets; answers from the target today, switch to `cfg!(feature)` if a backend becomes optional) |
| 3 | `VULKAN` | the Vulkan backend (Windows and Linux targets; same note) |
| 4 | `EXTENSION_SET` | `abi/pxui.ext.txt` exported (no stability promise) |
| 5 | `BUTTON_CONTENT` | 1.56 |
| 6 | `ICON_BUTTON_CURVES` | 1.54 |
| 7 | `TOGGLE_RESPONSE` | 1.51 |
| 8 | `SEGMENTED_RESPONSE` | 1.51 |
| 9 | `DIALOG_RESPONSE` | 1.53 |
| 10 | `WIDGET_VARIANTS` | 1.52 |
| 11 | `LABEL_NODE` | 1.54 |
| 12 | `MENU_ITEM_IDS` | 1.58 |
| 13 | `FRAME_SURFACE` | 1.57 |
| 14 | `ITEM_TEXT` | 1.49 |
| 15 | `COLOR_TOKENS` | 1.43 |
| 16 | `SWIPE_ACTIONS` | 1.40 |
| 17 | `DISMISS` | 1.41 |
| 18 | `LOOK_TOKENS` | 1.61: look colours that name colour tokens, resolved where a look is applied |
| 19 | `ICON_WORD` | 1.61: `ui_icon_word` / `paint_icon_word` |
| 20 | `TEXT_WORKER` | 1.62: `text_worker_create` / `text_worker_destroy`, `ui_set_text_worker`, `text_prewarm` (the prepare queue) |
| 21 | `TEXT_FIELD_KEYED` | 1.64 |
| 22 | `LABEL_TEXT` | 1.65: `ui_label_text_spans`, `ui_label_text_line_box`, `ui_label_measure`, `ui_label_quads`, `ui_label_rasterize_rgba`, `ui_label_prewarm` |
| 23 | `MENU_MAX_H` | 1.67: `PxuiMenuExtra.max_h` (`Menu::max_h`) |
| 24 | `SEGMENTED_RESULT` | 1.67: `ui_segmented_result`, `PxuiSegmentedResult`, `PxuiSegmentedDesc`/`PxuiSegmentConfig.has_selected`; 1.67 also changes the keyboard model of every segmented call (one Tab stop, the chosen option; every option focusable to assistive technology) |
| 25 | `WIDGETS_KEYED` | 1.68: `PxuiKeyedDesc` and the `ui_*_keyed` widget and container calls |
| 26 | `SEGMENTED_STORED_FOCUS` | 1.69: `PxuiSegmentedDesc.focus_on_click` (`Segmented::focus_on_click`), `PxuiSegmentConfig.stored`/`has_stored` |
| 27 | `IMAGE_CHECKED` | 1.70: `ui_image_node`, `paint_image_checked` |
| 28 | `BUTTON_PARTS` | 1.71: `PxuiButtonDesc`/`PxuiButtonConfig` leading icon, detail and accessory (`Button::icon`/`detail`/`accessory`), `PxuiIconButtonDesc.checkable`/`checked` (`IconButton::selected`), `PxuiDialogDesc`/`PxuiDialogConfig.details` (`DialogButton::detail`) |
| 29 | `VIEW_KEYED` | 1.72: `pxui_ui_view_keyed` (`Ui::view_of`) |

**Extension set.** The Rays vocabulary and diagnostics (`inspector_*`, `line_field`, `value_field`, `picker`, `diagnostics`, with the
records only they use) are not part of the promise. They are listed in `abi/pxui.ext.txt`, which carries no stability guarantee: they may
change or disappear between minors, and an app pins the commit if it uses them. The calls stay exported and declared in `include/pxui.h`.
The set is the names in that file; the `abi-snapshot` gate keeps its rows current (`cargo run -p gate -- abi-snapshot --write`) and
`abi/pxui.v1.txt` and `abi/layout.v1.txt` carry none of them. Moving a call into the set is an edit of that file, reviewed as one; a row
left in the frozen manifest is frozen.

**Old-app fixture.** At each release `tools/abi-fixture.sh <minor>` (on macOS and on Linux) keeps the release's header, its manifest and a C
app built against it in `fixtures/abi/2.<minor>/` (from 2.0; D28 retired the 1.x fixtures). `cargo run -p gate -- abi-compat` runs every kept binary against the library built from
the tree, and `abi-frozen` fails when a row the fixture's manifest froze is missing or changed in `abi/pxui.v1.txt` (a layout row may only
grow at its end behind its size prefix: same name and alignment, every frozen offset in place, new fields at or past the frozen size, no
shrink), or when the manifest gained a row without a later minor: editing `abi/pxui.v1.txt` to match changed code does not touch the fixture's copy.

**Adding a call.** In one commit: the `pxui_abi` export; the wrapper method in `pxui`; the declaration in `include/pxui.h`;
the appended row in `abi/pxui.v1.txt` (and `abi/layout.v1.txt` for a new record); the row in the table below; the
`ABI_VERSION` bump with the `minor` line. Then run the `abi-header` and `abi-snapshot` gates.

**Cut at 2.0.** Kept through 1.x under the promise, removed when the major becomes 2 (each has a replacement):

- `pxui_ui_image` (keyed and named by one label) and `pxui_paint_image` (drops bad input silently): replaced by `pxui_ui_image_node` and
  `pxui_paint_image_checked` (1.70).

- `pxui_ui_segmented` (the C choice call's segmented kind, `trusted::choice` kind 2), `pxui_ui_segmented_plain`, `pxui_ui_segmented_response`
  and the library-internal `trusted::segmented_plain`: replaced by `pxui_ui_segmented_result` (1.67), which alone reports no choice and the
  secondary option.

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
| 1.47 | — | Not released: the number was reserved for DX1.11 while DX-PERF landed first; 1.47 has no exports and no record changes. |
| 1.48 | `ui_button_plain`, `ui_button_response`, `ui_icon_button_plain`, `ui_icon_button_response`, 40-byte `PxuiResponse` | The kit button and icon button calls reporting one compact record (`PXUI_RESPONSE_*` bits: clicked, hovered, held, focused, activated; the box) instead of the box, the whole `PxuiSignal` and the focus; the `_plain` forms take no descriptor (the default button keyed by `key`, the default icon button with a glyph). The dynamic wrapper's `button`, `button_with`, `button_content` and `icon_button` use them; `_with` stays for callers that want the whole signal. (Provisional minor: 47 is DX1.11's.) |
| 1.49 | `PxuiSectionListDesc.key`, `PxuiMeasuredSectionDesc.key` (64 -> 72 bytes), `PxuiRowKey` | Virtual rows keyed by the item: `key(user, index)` returns row `index`'s item id and its row box is keyed by it (an id node under the list's rows, as `Key::Id`), so a row's state and its children's identities follow the item when items reorder (React's row key). A 64-byte record (no `key`) or a null `key` keys rows by index as before; any other short size is refused. |
| 1.49 | `ui_item_text` | From inside a `list_view`/`tree` row or `table_view` cell callback of the `_with` calls, the kit's text of that item through its row body: a list row's label, a tree row's (naming the row, as `ui_tree` does), a table cell's (naming the cell, as `ui_table_view` does). With it the Rust wrapper has one shape per collection (the cell closure); the text-row exports are unchanged. Refused outside a collection item. |
| 1.49 | `ui_resolve_label` | The `PxuiTextStyle` a `PxuiLabelStyle` shapes with, resolved against the Ui's theme label as `ui_label` resolves it (unset fields the theme label's, tracking in ems becomes points), so text measured or prepared off the tree matches what the Ui draws. Two roles: the label style authors (labels, painter labels, the Rust `text_line_box`/`text_spans`), the text style is what the shaper sees (shapers, the text service, prepared text, painter metrics and spans). No record change. |
| 1.50 | `PxuiMenuExtra.row_h`, `.max_rows`, `.checked` (80 -> 104 bytes), `PxuiMenuRowConfig.row_h` (104 -> 112 bytes) | `Ui::menu(key, at, &[MenuItem], &Menu)` crosses through `ui_context_menu` (the export stays): `row_h` is the row height in points (0: the theme's), `max_rows` the rows shown before the row column scrolls (0: all), `checked` one mark byte per row (0 no slot, 1 slot, 2 slot with the accent square; replaces `selected`/`lead_from` when not empty). Submenus must be given in increasing row order (their rows are numbered after the menu's in that order); a `size` below 80, a submenu row out of range or out of order, or a non-finite or negative `row_h` is refused. The context-menu row body is given `row_h`. |
| 1.51 | `ui_toggle_plain`, `ui_toggle_response`, `ui_segmented_plain`, `ui_segmented_response`, `ui_disabled`, 88-byte `PxuiToggleDesc`, 64-byte `PxuiSegmentedDesc`; `PxuiToggleConfig` and `PxuiSegmentConfig` grow at their end | The kit toggle and segmented control with options, each built through its body and reporting one `PxuiResponse`. Toggle: row height, switch size, switch and knob looks, side inset, a detail line under the text, a press anywhere on the row, enabled. Segmented: strip height, segments sharing the strip's width, segment looks, enabled, accessible name; also reports the choice. `PxuiToggleConfig` grows by the row height, switch size, the two looks (`PxuiStates` pointers valid for the call), the inset, `press_row` and the detail; `PxuiSegmentConfig` by the looks and `grow`. `ui_disabled(disabled, build)` builds a subtree in which every node declared with CLICKABLE or FOCUSABLE loses both and reads as disabled to accessibility; scopes nest by OR, a view cached inside rebuilds when the flag flips. `ui_toggle` and `ui_segmented` are unchanged. |
| 1.52 | `ui_variant`, `variant: u16` at the end of every `Pxui*Config` (34 records, each +8 bytes) | A per-call variant for widget bodies: `Button`, `IconButton`, `Label`, `Menu`, `Toggle`, `Segmented` and `TextInputOptions` carry `.variant(n)`; the wrapper sets it with `ui_variant(kind, variant)` before the widget call and clears it after (a frame starts with it cleared), every registered body of that call and kind (a menu's rows, a segmented control's segments) reads it as `config.variant` (0 when the call set none), and a body's trampoline keeps it 0 while the body runs so widgets built inside are given none; a body of another kind built inside the call (a menu's scroll view) is given none. Records grow at the end behind `size`; a body reads `variant` only when `size` covers it. `TextInputOptions` gains the Rust-side `variant`; its C record `PxuiTextInputOptions` is unchanged (12 bytes). |
| 1.53 | `ui_dialog_response`, 88-byte `PxuiDialogDesc`, 16-byte `PxuiDialogResult`, `PxuiDialogConfig` grows to 80 bytes | The modal dialog with flagged buttons (default, cancel, danger, disabled; at most 64) keyed by `key`, answering `chosen` and `dismissed`; the body config carries the flags (`default_at`, `cancel_at` indices or -1, `danger` and `disabled` bitmasks). `ui_dialog` stays: the last button is the default, none cancels. |
| 1.54 | `ui_label_node`, 16-byte `PxuiLabelBox` (`PXUI_LABEL_BOX_SET_HEIGHT`/`_ROLE`); after `variant`: `PxuiLabelConfig.label_box` (128 -> 144), `PxuiButtonConfig.centered` (136 -> 144), `PxuiIconButtonConfig.press` (128 -> 136); at the end: `PxuiButtonDesc.centered` (96 -> 104), `PxuiIconButtonDesc.press` (96 -> 104); `PxuiCurve` (the C view of `Curve`) | What LocalWave's text and button helpers need as style values. A label's box: its height (its lines centred in it), a whole-pixel fitted width and its accessible role, crossing as a `PxuiLabelBox` beside the unchanged `PxuiLabelStyle`; `ui_label_node` is `ui_label` with the box (null for none) returning the label's node (a null style is the theme's label), which the wrapper's `label`/`label_fmt`/`label_digits` return; `ui_label` and `label` are legacy: superseded by `ui_label_node`, kept for the append-only ABI. `Button::centered()`: the box exactly its size with nothing around it in its line, its text centred (`Button::sized(w, h)` is `DEFAULT.size(Px, Px).centered()`). `IconButton::press_curves(&'static [Curve; 2])`: the press scale's curves, crossing as a pointer to two `PxuiCurve`s (null: the kit's springs, 60 in, 20 out); a pointer that is not 4-aligned or a kind past 3 is refused before the curves are read. The 96-byte button and icon button records are still read (the new fields as unset), other short sizes are refused; bodies get the new fields after `variant`. Line boxes now keep their floored half-leading: the first baseline is the line's top plus its rounded ascent and floored half-leading (before, the glyphs sat as if the layout's own half-leading were not there, so a larger line height in a fixed box moved the text up). |
| 1.55 | At the end: `PxuiMenuExtra.padding`/`has_name`/`name`/`selected` (104 -> 144), `PxuiToggleDesc.has_style`/`style` (88 -> 160), `PxuiSegmentedDesc.w`/`padding`/`min_segment`/`names` (64 -> 96); after `variant`: `PxuiToggleConfig.has_style`/`style` (128 -> 192), `PxuiSegmentConfig.padding`/`min_segment` (104 -> 112) | What LocalWave's settings bodies need from the standard calls. `Menu::padding` (the space above the first row and below the last, 6 by default), `Menu::name` (the sheet's accessible name; none by default: the key is identity only, never the name) and `MenuItem::selected` (the current choice of a settings menu: CHECKED and SELECTED, the `checked` menu look, `state.checked` in a row body's record). `Toggle::label(Label)`: the title's text style (unset fields the toggle's look); the detail line wraps to the room the switch leaves, is its own Label node and grows the row; a `press_row` toggle takes the focus on a press (Enter/Space then flip it). `Segmented::size`/`padding`/`min_segment`; options are `&[Segment]` with `Segment::name` the option's accessible name (`names` beside the texts in C). A dialog's behaviour gives its body's title and buttons the Heading/Button role and the name only when the body left them unset, never overwriting a role, states or a name the body set. The shorter records are still read (the new fields as their defaults); other short sizes are refused. |
| 1.56 | `ui_button_content`, `PxuiButtonBuild`; at the end: `PxuiButtonDesc.has_label`/`axis`/`content_align`/`focus_on_click`/`gap`/`label`/`label_box` (104 -> 192); after `centered`: `PxuiButtonConfig.has_label`/`axis`/`content_align`/`gap`/`text_style`/`text_box` (144 -> 232) | What LocalWave's buttons need without an inner container. `Button::label(Label)`: the button's text drawn through the label line box (as `ui.label`; unset width: the text's, unset pad: none, unset colour: the look's text colour for the state); `Button::axis`/`gap`/`content_align`: the contents laid out by the box (`content_align` places them across it, start/centre/end, centred down it); `Button::focus_on_click(true)`: a press focuses the button (the kit's: Tab only). `ui_button_content` is `ui_button_response` whose content callback is also given the button's `PxuiStateDesc` this frame (the wrapper's `button_content(key, &Button, \|ui, state\| ..)`); `ui_button_response` is legacy: superseded by `ui_button_content`, kept for the append-only ABI (its callback gets no state). The 96- and 104-byte button records are still read (the new fields unset); a label that does not resolve, a gap that is not finite and non-negative or a kind past its last is refused. A disabled widget never reports a click: a press begun while it was enabled, Enter/Space and an assistive technology's Press on a node shown disabled are dropped. |
| 1.57 | `FrameIn.surface` (152 -> 160 bytes), `PxuiFrameIn` | The id of the surface a frame is for, reserved before the freeze: the library has one surface per `Ui`, ignores the value and a host passes 0 (a 152-byte record, from before 1.57, is read without it). Events carry no surface: they belong to the frame's. `Event` is an unsized 48-byte array element, so it cannot grow; a later minor that needs per-event surfaces adds a new event record. |
| 1.58 | At the end: `PxuiMenuExtra.focus_on_click`/`ids`/`has_id` (144 -> 184); after `variant`: `PxuiToggleConfig.stored`/`clicked` (192 -> 200), `PxuiSegmentConfig.line`/`strip` (112 -> 176) | What LocalWave's menus, settings and sheets need (DX3.13). Menu rows are keyed by `MenuItem::id` when given, else by their position, never by their text (two rows of one text are distinct); `MenuResponse::chosen_id` (wrapper) reports the chosen row's id; `Menu::focus_on_click` lets a press focus its row. The menu sheet's role is set before the popup body runs, so a registered body's role stays (a dialog's sheet likewise); past `max_rows` the sheet itself scrolls (no scroll view node inside it); the menu is placed at the sheet's least width (180), so it stays inside the view. A toggle body gets the stored value and whether it flipped beside `on`; a segment body gets the line and the strip it sits in and may restyle them. Registered Rust bodies read every pointer field of their config without `unsafe` through one accessor family in `pxui::bodies`: `text_of` (a `Str`), `text_at` (a `Slice<Str>`: dialog buttons, tooltip lines), `hint_at` (footer hints), `states_of` (`states`, `track`, `knob`) and `curves_of` (an icon button's `press`). The 144-byte menu extra is still read; other short sizes are refused. |
| 1.59 | `PXUI_RESPONSE_SECONDARY` (32) in `PxuiResponse.flags` | A secondary (right-button, or the two-finger or control click the platform reports as one) press began on the box this frame: the wrapper's `Response::secondary`, a context-menu request, for every widget reporting a `PxuiResponse`. No record or call changes. The wrapper's `pressable` (no export: node, signal, focus, gesture state and look) takes a `Pressable` style (`look(&States)`, `enabled`, `tab_stop`, `selected`) and gives its contents the state; `Signal::BUTTON_PRIMARY`/`BUTTON_SECONDARY` name `Signal.button`. |
| 1.60 | `PxuiUiDesc.abi_built` (12 -> 16 bytes), `pxui_supports`, `PXUI_FEATURE_*` | The major the app was built against, checked once at create (minor not compared); the feature registry. Grown input records (`PxuiButtonDesc`, `PxuiIconButtonDesc`, `PxuiToggleDesc`, `PxuiSegmentedDesc`, `PxuiMenuExtra`, `PxuiUiDesc`, and `FrameIn` at 48, 56, 128, 152) are read only at a size they have had or at the whole record and more; grown fields and pointers are gated on the size. |
| 1.61 | `ui_icon_word`, `paint_icon_word`, `PXUI_FEATURE_LOOK_TOKENS` (18), `PXUI_FEATURE_ICON_WORD` (19); colour encoding (final): a colour with alpha 0 and bytes 0x54, 0x4b, b is colour token b (no layout change) | `Color::token(ColorToken)` (`Color` is `Rgba`): a colour with alpha 0 and bytes 0x54, 0x4b, b is a token; recorded as final. A token at or past `ColorToken::LIMIT` (64), or past the installed table, is transparent; table entries that are themselves tokens are stored clear. The library resolves a token to its light or dark entry for the theme in effect (a `themed` scope's own) only where a look or colour is applied: a node's look (`node`, `set_look`/`style`/`pressable`, `patch_look`, `set_node_fields`), an animated look's target, the states of buttons, icon buttons, toggles and segments and the menu and picker row swatches before their bodies run, and every look of a theme (`set_theme`, `themed`; the root theme again on a colour table change). Interpolated looks (transitions, animations) are never resolved again. A theme, appearance or table change rebuilds views, so token looks follow it with no app rebuild. Before this minor such a colour painted nothing. `Paint` calls with a raw token still paint nothing (resolve with `ui_color`/`paint_color`). `ui_icon_word(word)` gives the next `ui_icon_button_with`/`_response` call's drawing a word, read in the `PxuiIconDraw` callback with `paint_icon_word` (0 elsewhere), so one callback draws many icons by id; the call takes it. A registered body that calls `draw` from its own paint callback passes no word. The wrapper's `Icon::draw(f, word)`: `f` is `draw_user`, `word` the icon word. |
| 1.62 | `text_worker_create`, `text_worker_destroy`, `ui_set_text_worker`, `text_prewarm`, `PxuiTextWorker`, `PXUI_FEATURE_TEXT_WORKER` (20) | The prepare queue. A worker is one thread the host owns and attaches to any number of `Ui`s (no hidden or per-`Ui` thread); each attached `Ui` has a lane: its own fork of its fonts on the worker (a font change reforks that lane once; font bytes are shared) and fixed buffers (128 requests in flight, 16 KiB of text per flush, 32 installs per frame). Attached, a `Ui`'s virtual lists (`list`, `section_list`, grids; measured lists for rows below the viewport) queue the unseen text of the rows they build past the viewport, which lays out empty until it arrives (those rows are out of view); it is installed before a frame builds, so a scroll does not shape on the UI thread. `text_prewarm` is the app's hint (0: unbounded width; refused: negative or non-finite width, text over 16 KiB); without a worker it does nothing. A preparation that fails or panics is delivered as a failure and shapes inline; a worker destroyed (it waits for at most one shaping call) or whose thread ended leaves its `Ui`s shaping inline. Text a fling brings into view before its delivery still shapes on the UI thread: the one named exception, its ceiling pinned by pxui_core `window_pxui_fling_shaping_stays_under_its_ceiling`. A worker serves at most 64 `Ui`s (`ui_set_text_worker` answers `PXUI_ERR_STATE` past that, the `Ui` left detached); after a panicking preparation the lane's font fork is renewed. Threads: `text_worker_create`/`text_worker_destroy` on any thread; `ui_set_text_worker` and `text_prewarm` on the `Ui`'s thread; several threads may attach to one worker, but the host serializes `text_worker_destroy` against `ui_set_text_worker` on that worker. The worker pointer is not validated (as a shaper's): it must be one `text_worker_create` returned and not destroyed; using it after destroy is undefined. |
| 1.63 | none (limits and refusals) | **Text, image and renderer hardening for untrusted callers.** *Fonts* (`ui_create_with_fonts`, `shaper_create`, `shaper_add_font`, `text_add_font`): a font is 1 byte to 32 MiB (`MAX_FONT_BYTES`; was 16 MiB for `ui_create_with_fonts` only, unbounded for the adds), a text service holds at most 64 fonts (`MAX_FONTS`; `Style.font` is a `u16`; was 128) and 256 MiB of font bytes in all (`MAX_FONT_TOTAL_BYTES`); bytes that are not a parseable font are refused (`UINT32_MAX` for the adds, `NULL` for the creates) with nothing kept; fonts are never removed. The Rust wrapper's `add_font` now answers `None` for such bytes. *Layouts*: retained layouts have a byte budget (`LAYOUT_BYTES`, 64 MiB) over their estimated shaped size (160 bytes per text byte plus 1 KiB per layout, since cosmic-text reports no buffer sizes), besides the 4,096-layout count. Past it the least valuable layouts go, never one laid out in the current frame (so a frame showing many large texts keeps them all, and the excess goes the next frame; a layout larger than the whole budget stays only while used); a text service that never ends a frame evicts regardless past twice the budget. *Text length*: every shaper, text-service and spans entry refuses text over 256 MiB (`MAX_TEXT_BYTES`) with `PXUI_ERR_ARGUMENT` (`SIZE_MAX` for the sized-result calls). *System fonts* (`text_system_fonts`, `shaper_system_fonts`): memory-mapped by the OS, demand-paged and outside the font budget above; bounded instead to 128 files, 256 MiB per file (the macOS colour-emoji collection is about 190 MB) and 1 GiB of mapped address space in all, a file past either skipped. *Renderer*: `renderer_create` reads `gpu` and `target` only after their `size` covers the record (a shorter record is refused, `NULL`; the lease of a gpu record too short to hold `release_owner` is not released); `renderer_resize` answers `PXUI_ERR_ARGUMENT` for an extent above the device's maximum 2D texture dimension, allocating nothing. *Images*: native-image identities are per `Ui`, not per thread; bounds as in 1.13 (the eviction scan is bounded by 1,024 entries; it is not O(1)). |
| 1.64 | `ui_text_field_keyed`, `ui_input_box`, 56-byte `PxuiTextFieldDesc` | A text field keyed by `desc.key` and named `desc.name` (empty: unnamed), size-prefixed, and the box of the text input the last text field, line field, search field, text area or value field call built (the wrapper reads its response from it and names it). Legacy, cut at 2.0: `text_field`, `text_field_with_options`, `ui_search_field` and `ui_text_area_submit` (they keep working; the wrapper reads the box they built through `ui_input_box` and names it itself). `ui_line_field` and `ui_value_field` are in the extension set. |
| 1.65 | `ui_label_text_spans`, `ui_label_text_line_box`, `ui_label_measure`, `ui_label_quads`, `ui_label_rasterize_rgba`, `ui_label_prewarm`, `PXUI_FEATURE_LABEL_TEXT` (22) | Text calls on a label style (`PxuiLabelStyle`, `digits` ignored), resolved against the `Ui`'s theme in the same call as `ui_resolve_label` resolves it, then run as their resolved-style twins (`ui_text_spans`, `ui_text_line_box`, `text_width`/`text_width_spans`, `text_quads`, `text_rasterize_rgba`, `text_prewarm`) with the content's layout key: no resolve round trip per call. The Rust wrapper's `Ui::text_spans`/`text_line_box`/`prewarm_text` and `UiText` take a `Label` through them (static builds call the trusted entries with the label's fields borrowed). A null, short, invalid or unresolvable label record is refused as the twin refuses an invalid style; `ui_label_measure`'s spans may be null. |
| 1.66 | Limits (see **Limits**): per-role string caps, one event validator, surface and held-key checks, builders refused outside a frame, bounded returned strings, poisoned shapers/themes/bridges; `ui_calls` (extension set); `counter(0)` reads 0; `shaper_create` with no font is null | OS-grade untrusted callers. The Rust wrapper clips display text past 64 KiB (identifiers past 256 bytes still panic). Behaviour change on inputs earlier minors accepted: an identifier over 256 bytes, display text over 64 KiB, editor/event text over 16 MiB (was 256 MiB), a non-finite event coordinate or value in a frame (`ui_wants_frame` already refused it), a negative text editing range, a resize, frame size or logical size outside `0..=65536`, an unknown held key code (was `Key::Unknown`), a builder outside a frame (was a caught panic: `PXUI_ERR_PANIC` and a poisoned `Ui`), a 4097th string handed back between frames. Grid track lists are found by hash, not by a scan (O(grids) a frame). |
| 1.67 | At the end: `PxuiMenuExtra.max_h`/`reserved4` (184 -> 192), `PxuiSegmentedDesc.has_selected`/`reserved3` (96 -> 104), `PxuiSegmentConfig.has_selected`/`reserved3` (176 -> 184); `ui_segmented_result`, `PxuiSegmentedResult` (24 bytes); `PXUI_FEATURE_MENU_MAX_H` (23), `PXUI_FEATURE_SEGMENTED_RESULT` (24); every segmented control is a radio group | `max_h` bounds a menu sheet's whole height in points, its 1-point edges and padding included (finite, not negative; 0 no bound; the 184-byte 1.58 record is still read); with `max_rows` the smaller wins and the sheet scrolls past it (`Menu::max_h`). `has_selected` (final): 1 when `selected` is the choice (clamped to the options), 0 for no choice (`selected` not read); a 64- or 96-byte descriptor is read as 1. `ui_segmented_result` takes it and reports into `PxuiSegmentedResult` the choice (`has_chosen` 0: none) and the option a secondary press began on (`has_secondary`; never a choice); a null or short result is refused; `ui_segmented_response` refuses `has_selected` 0. A segment body is given `has_selected` (0 with `selected` 0 for no choice). Behaviour of every segmented call from this minor (`ui_segmented`, `_plain`, `_response`, `_result`): the strip is no longer focusable; every option is a Radio focusable to assistive technology (FOCUSABLE and the Focus action, which any node with the FOCUSABLE accessibility state now offers) and only the chosen option (the first with none) is a Tab stop; Left and Right on the focused option move the focus and the choice (clamped), Space or Enter chooses it, a press chooses without focusing; when the choice changes while an option holds the focus the focus moves to it, and when the focused option goes the focus moves to the Tab stop (the strip with no option left); `PXUI_RESPONSE_SECONDARY`, `HOVERED`, `HELD` and `FOCUSED` cover the options, `CLICKED`/`ACTIVATED` a press or key on one. Wrapper: `Ui::segmented_with(key, options, selected: Option<usize>, &Segmented) -> SegmentedResponse { response, chosen, secondary }` (the short `segmented(label, options, usize)` is removed), `Pressable::focus_on_click` (no export: `FOCUSABLE` without `TAB_ONLY`). |
| 1.68 | 40-byte `PxuiKeyedDesc` (`size`, `reserved`, `key`, `name`); `ui_checkbox_keyed`, `ui_choice_keyed` (`PXUI_CHOICE_RADIO` 0, `_TABS` 1, `_DROPDOWN` 3), `ui_slider_keyed`, `ui_progress_keyed`, `ui_spinner_keyed`, `ui_badge_keyed`, `ui_panel_keyed`, `ui_popup_keyed`, `ui_modal_keyed`, `ui_anchored_keyed`, `ui_scroll_view_keyed`, `ui_toasts_keyed`, `ui_table_view_keyed`; ext: `ui_picker_keyed` | Each widget's identity and its shown/accessible text cross apart in one size-prefixed record: `key` is identity only (a static string or `Key::label` of an item id) and `name` is never parsed for a key, so no in-band `##`/`###` encoding is needed. A checkbox click is the flip of `on`; a choice reports the choice, a slider the value. Toasts are keyed by an id per item (`ids` as long as `items`), never their text. `ui_table_view_keyed` is the one table: the virtual table, or with `text` the fixed text table at that rectangle (each cell's text given to `ui_item_text`), reporting the clicked header or text row. A keyed popup or table refuses a non-empty `desc->label`. Every keyed call refuses a short record or a non-zero `reserved`. The label-keyed `ui_checkbox`, `ui_radio`, `ui_tabs`, `ui_dropdown`, `ui_slider`, `ui_progress`, `ui_spinner`, `ui_badge`, `panel`, `ui_popup`, `ui_modal`, `ui_anchored`, `ui_scroll_view`, `ui_toasts`, `ui_table`, `ui_table_view_with` and `ui_picker` are legacy, cut at 2.0 (they keep their behaviour; `ui_anchored_keyed` takes sizes in points or fit). The key-only `ui_section_list(_measured)`, `ui_grid_list`, `ui_list_view_with` and `ui_tree_with` carry no name: their `label` is the key verbatim. (Provisional minor.) |
| 1.69 | At the end: `PxuiSegmentedDesc.focus_on_click`/`reserved4` (104 -> 112), `PxuiSegmentConfig.stored`/`has_stored`/`reserved4` (184 -> 200); `PXUI_FEATURE_SEGMENTED_STORED_FOCUS` (26) | `focus_on_click` (0 or 1; a 64-, 96- or 104-byte descriptor reads 0): a press on an option focuses it, and the pressed option being the choice is the strip's one Tab stop; by default a press chooses without focusing (`Segmented::focus_on_click`, as `Button`, `Menu` and `Pressable`). `stored`/`has_stored`: a segment body is given the choice the caller passed this frame, clamped to the options (`has_stored` 0, `stored` 0: none), before a press or key moved `selected`, as `PxuiToggleConfig.stored` is the toggle's. A feature probe is added (not only the size rule) because an older library reads a larger descriptor up to the size it knows and would silently ignore `focus_on_click`. |
| 1.70 | `ui_image_node`, `paint_image_checked`; `PXUI_FEATURE_IMAGE_CHECKED` (27) | An image widget has no key (the n-th image among its siblings, as a label is) and its `name` is its accessible name only, never parsed for `##`/`###`. A painted image reports a refusal (`PXUI_ERR_ARGUMENT`, nothing drawn) for a null painter or descriptor, an invalid descriptor (zero size, no native handle, a depth format, a short `size`), an unknown fit or a non-finite rectangle, where `paint_image` drops it silently. `ui_image` (label-keyed) and `paint_image` are legacy, cut at 2.0 (behaviour kept). (Provisional minor.) |
| 1.71 | At the end: `PxuiButtonDesc.lead`/`lead_draw`/`lead_user`/`detail`/`accessory`/`has_detail_label`/`has_accessory_label`/`reserved3`/`detail_label`/`detail_box`/`accessory_label`/`accessory_box` (192 -> 424), `PxuiButtonConfig.lead`/`lead_draw`/`lead_user`/`detail`/`accessory`/`has_detail_style`/`has_accessory_style`/`reserved4`/`detail_style`/`detail_box`/`accessory_style`/`accessory_box` (232 -> 464), `PxuiIconButtonDesc.checkable`/`checked`/`reserved2` (104 -> 112), `PxuiDialogDesc.details` (88 -> 104), `PxuiDialogConfig.details` (88 -> 104); `PXUI_FEATURE_BUTTON_PARTS` (28) | A 96-, 104- or 192-byte button record reads no leading icon, detail or accessory (empty strings, null drawing); `has_*_label` > 1, an invalid style or nonzero `reserved3` is refused; the leading drawing takes the word `ui_icon_word` set, as an icon button does. With a build callback (`ui_button_with`/`_response`/`_content`) the contents replace the text and the parts. `checkable`/`checked` 0 or 1 with `checked` <= `checkable` (a 96- or 104-byte record is a plain button): an on/off key, accessible CHECKABLE and CHECKED, its checked look. `details` has as many strings as `buttons` or none (an 88-byte record: none); empty is no detail. A registered body reads the config fields only when `size` covers them; a dialog body's `details` always has as many entries as `buttons`. |
| 1.72 | `ui_view_keyed`; `PXUI_FEATURE_VIEW_KEYED` (29) | The authoring view (`Ui::view_of(key, &inputs, f)`), keyed by `PxuiKeyedDesc.key` (a non-empty `name` is refused: a view has no accessible name). It fills its parent, so its size comes from the enclosing node; on the main axis of a row or column whose size there is fit it takes its content's size (a content-sized view), since filling would leave nothing. It has its own painter table entry. `revision` is the hash of what the build reads (`hash_of`). `ui_view`/`ui_view_flat` keep their explicit sizes. (Provisional minor.) |
| 1.73 | `theme_parse`, `theme_set_color`, `theme_density`, `theme_reduced_motion` | Parse a detached handle and resolve its body names in the destination Ui. Root installation requires every name and keeps the previous theme on error. Scopes fall back to built-in bodies. A handle may carry its own color table, density and motion preference. The legacy Ui-aware source and light/dark color table retain their behavior. |
| 1.74 | `renderer_create_checked`, `body_register_checked`, `ui_a11y_name_checked`, `ui_a11y_number_checked` | Constructors return the refusal status and write outputs on success. A complete GPU descriptor transfers its lease to the checked constructor once the output pointer is supplied. Checked accessibility setters return `PXUI_ERR_STALE` when the node generation or key has retired. The legacy entry points retain their return conventions. |
| 1.75 | `ui_key_pressed_mods` | Match the requested modifiers together on one raw key press. Shift, Ctrl, Alt and Meta use bits 0–3. Zero requires no held modifiers and 16 accepts any state. Later releases leave the press decision intact. The earlier one-modifier and wildcard routes retain their behavior. |
| 1.76 | `ui_pick`, `ui_slider_result`, `ui_scrub_result`, `ui_panel_root`, `ui_popup_root`, `ui_modal_root`, `ui_split_result`, `ui_menu_bar_result`, `ui_segmented_items`; `PxuiPickItem` | One result per call for the Rust U1b.1 results: pick-one items with an id, name and detail (an id keys the item, a place keys one without, in separate namespaces), slider and scrub phases with the full response (a commit carries an equal preview), overlay roots, the split grip's response and held ratio, and the menu bar's dismissal. The earlier exports keep their results. |
| 1.77 | `ui_key_seed`, `ui_remember_key`, `ui_image_keyed`; a `ui_hover_delay` key in the resolved form | Keys for the Rust U1b.2 lookups. A label of a 0x01 byte and 16 lowercase hex digits is a resolved key (`Key::Resolved`): every keyed call takes it as the key itself, never hashed again (a 0x01 label carrying 0 or 1, or not in that form, is refused with `PXUI_ERR_ARGUMENT` by every export whose label is a key, the legacy label-keyed ones included, whose identity-only labels are identifiers capped at 256 bytes and whose shown `##` labels keep the display cap); authored labels beginning with a byte below 0x20 are reserved. `ui_key_seed` reports the seed a lookup resolves an authored key under (the active scope during a build, the root seed outside one); `ui_expanded` resolves under the same seed. `ui_remember_key` snapshots into the history of the field keyed `key` only (the active editor's or a shelved one), never changing the active editor. `ui_image_keyed` keys an image by a label or id and names it separately. `ui_hover_delay` waits on a resolved-form key without copying text; other text keeps the 1.x wait. `ui_root_at` keeps the innermost matching candidate whatever their order. |
| 1.78 | `ui_font_points`, `ui_row_points`, `ui_set_font_points`, `ui_theme_exact`, `paint_label_measure`, `paint_label_spans`; `PxuiPanel.padding_points` (alignment hole, offsets unchanged) | Fractional units for the Rust U1b.3 measurement step. The body size and kit row keep fractions (`ui_font_size`/`ui_row_height`/`ui_set_font_size` stay whole points, rounded half away from zero). `ui_theme_exact` lends the engine's snapshot, whose token text sizes are `float` in the low half of each 1.x `int64_t` slot; `ui_theme` lends the integer projection, copied for the C caller on the call. `PxuiFont` carries a fractional size at offset 28 (its tail padding) and `PxuiPanel` one at offset 20 (its alignment hole): each counts only while it agrees with the integer field within half a point, so a 1.x record keeps its integer meaning. A painter's label measure and spans resolve the style against the painter's theme, so a painter measures what it paints. |
| 1.79 | `PxuiFontDesc.size_points` (alignment hole at offset 4, offsets unchanged) | A look's font keeps fractional points through `ui_style` and the other look descriptors, read only while it agrees with `size` within half a point (a 1.x record keeps its integer meaning). The Rust `set_look` is folded into `style` (one `States` value; all six looks are checked before any applies, as before); `ui_set_look` stays for C. |
| 1.80 | `ui_scroll_view_node`, `ui_text_area_node` | The Rust U1b.5 `scroll_view(key, Node, f)` and `text_area(key, value, Node, ..)`: the scroll view and the text area are laid out by a complete checked `Node` (no grid) instead of two sizes or an absolute rectangle; the 1.x calls keep their meaning (the same engine body with a node of their sizes). |
| 1.81 | `ui_layout_node`; labels and images keyed positionally | The Rust U1b.6 keyless `row`/`column`/`stack` (D25): a layout container is the n-th of its scope, keyed by the library in a positional namespace (a seed-and-kind mix no label, id or resolved label goes through); labels and images move from the reserved `"\0label"`/`"\0image"` labels to the same namespace, each kind counted apart. Behaviour of every 1.x call is unchanged except those two internal key values. `pxui_scope` is the Rust `ui.id(key, f)`. |
| 1.82 | `ui_text_field_response`, `ui_search_field_response`, `ui_line_field_response`, `ui_value_field_response`, `ui_text_area_response` | Each text input reports what happened to its box (`PxuiResponse`: box, hover, press, focus, click) in the one call, and the search, line and value fields and the text area name the box to a screen reader; the Rust wrapper made `ui_input_box`, `ui_a11y_name_checked`, `ui_signal` and `ui_focused` follow-up calls for that (U1c.5, D26). The legacy exports are unchanged. |
| 1.83 | `ui_list_view_rows`, `ui_table_view_rows` | The Rust U2a.1 row keys: `selection_list` and the virtual `table_view` key row `i`'s box by the item id `key(desc->user, i)` (`PxuiRowKey`, as `PxuiSectionListDesc.key`), so a row's state follows its item when the items reorder or sort; a NULL `key` keys by index. The records are unchanged: the key crosses as an argument (new exports chosen at the U2a shape stop). The text table has no row boxes and ignores it. `ui_list_view_with` and `ui_table_view_keyed` keep their index keys. |
| 1.84 | `ui_tabs_result`; `PxuiTabConfig.detail` (80 to 96 bytes) | The Rust U2a.2 `tabs(key, &[Segment], Option<usize>, &Tabs) -> PickResponse`: the tabs are `PxuiPickItem`s (checked as `ui_pick` checks them, in one pass): an id keys a tab (a tab without one is keyed by its place in a separate namespace), `detail` is drawn after the text by the built-in body, `has_selected` 0 is no tab in use (Right then takes the first, Left the last). `focus_on_click` 1 focuses the bar on a press (not a secondary one), whether or not the caller applies the choice. The result is a `PxuiSegmentedResult`: the tab in use and the tab a secondary press began on (never a choice). A registered tab body is given the detail in `PxuiTabConfig.detail`, appended at 80 (the record grows under the growth rule; a body reads it only when `size` covers it). `ui_tabs` and `ui_choice_keyed` kind 1 keep their behaviour (legacy, cut at 2.0). Behaviour of every segmented call from this minor: with `focus_on_click` the pressed option takes the focus on the press, and keeps it when the caller does not apply the choice. Create refuses an app built for a newer minor than the library (`PxuiUiDesc.abi_built`, the minimum-library rule; an older or unstated one is served as before). |
| 1.85 | `ui_checkbox_response`, `PxuiCheckboxDesc` (56 bytes); `ui_section_index_result`; `ui_disclosure`; `PxuiCheckConfig.states`/`box_size`/`passive` (72 to 88 bytes); body kind `PXUI_BODY_SECTION_INDEX` (40), `PxuiSectionIndexConfig` (80 bytes), `PxuiBodies.section_index` (`PxuiThemeSnapshot` 56888 to 56896 bytes) | The Rust U2a.3 checkbox style, section index and disclosure. `PxuiCheckboxDesc`: `disabled`, `on`, `passive` 0 or 1, `reserved` 0, `box_size` finite and not negative (0: 12 points), `look` NULL or six looks; the key is identity only; refused before any build otherwise. A passive checkbox has no hit area, focus or hover (the row it sits in owns them), still reports its checked state read-only, and with empty text is the box alone. The section index is a focusable slider over its letters (Up/Down, Home/End, an assistive SetValue, a press or drag), reporting the letter chosen this frame into a `PxuiSegmentedResult`; an empty `letters` is a disabled strip; it is built by the `section_index` body (`PxuiSectionIndexConfig`: the strip entered as `node`, its letters, the active letter), which a theme may replace; the body kind's id is the new last field of `PxuiBodies`, so `PxuiThemeSnapshot` grows at its end (a reader built for 1.84 reads its prefix). `ui_disclosure` builds a collapsible section through the `inspector_section` body with the caller's `open`; its response's `CLICKED` is a toggle. A registered checkbox or radio body is given the box's looks, side and passive mark in `PxuiCheckConfig`'s tail, appended at 72 (read only when `size` covers it). `ui_checkbox_keyed` is legacy, cut at 2.0. |
| 1.86 | `ui_modal_sheet`, `PxuiSheetDesc` (20 bytes), `PxuiPickRow` (64 bytes), `PxuiPanelConfig.present` (72 to 80 bytes); extension set: `PxuiPickerRows` fills `PxuiPickRow`s, `PxuiPickerRowConfig.art_kind`/`art` | The Rust U2a.4 sheets: `Overlay::present(Presentation)` and `.dismiss(DismissOpts)` reach C through the one modal route `ui_modal_sheet` (a new export: `ui_modal_root` passes its width as arguments and has no record to grow). A bottom sheet sits on the content area's bottom edge, a full one fills it; both take a downward drag-to-dismiss with the given thresholds and are moved by the drag. Every sheet field is checked before the build. Picker fills (extension set, no stability promise) hand back `PxuiPickRow`s: a glyph or `Images` id art and the heading of the group a row starts; each row is checked once. A registered panel, window, popup or modal body is told the presentation in `PxuiPanelConfig.present` (0 centred, 1 bottom, 2 full; a `uint64_t` so it starts past the record's recorded 72 bytes), read only when `size` covers it. `PxuiPickerRows` changed in place (extension set, no stability promise): a caller built before 1.86 that fills `PxuiStr`s into the span is refused by the row size check, never misread. `ui_modal`, `ui_modal_keyed` and `ui_modal_root` keep their centred behaviour. |
| 1.87 | Changed in place (D28): `ui_section_list(keyed, PxuiSectionListDesc, key, build, user, PxuiListResult*)` (32-byte desc; builds its trailer as index `SIZE_MAX - 2`), `ui_table(keyed, PxuiTableDesc, key, cell, user, PxuiTableResult*)` (the column table, 96-byte desc); new `ui_selection_list`, `ui_table_head`, `PxuiCellState` (48), `PxuiListResult` (104), `PxuiSelectionListDesc` (64), `PxuiColumn` (40), `PxuiColumnWidth` (16), `PxuiTableHeadDesc` (56), `PxuiHeadResult` (56), `PxuiTableResult` (168); deleted `ui_list_view`/`_with`/`_rows`, `ui_table_view`/`_with`/`_keyed`/`_rows`, `ui_section_list_measured`, the text table and its records, body kind `table_cell` and `PxuiTableCellConfig` (`PXUI_BODY_PICKER` and later renumbered; `PxuiThemeSnapshot` 56896 to 56888) | The Rust U2b.1 `selection_list(key, count, row_key, &mut ListState, &SelectionList, cell)`, `table(key, &[Column], rows, row_key, &mut TableState, &Table, cell)` and `table_head(key, &[Column], &mut TableState, &TableHead)`: selection, sort and widths in app-owned state, columns checked in one pass before any effect, one result record each. |

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
image. Native identity interning is per `Ui` (1.63; before, one table per thread) and bounded to 1,024 entries and a 128 MiB estimated texel
working set, evicting the least recently used by a scan of at most 1,024 entries on a miss that overflows (one oversized image may stand alone). Wrappers retain weak image identities and retire
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

### DX1.12 (F part 2): legacy marks, no new exports

The Rust wrapper dropped `inspector_section_view`, `inspector_toggle_value` and `tooltip` (twins of `inspector_section`, `inspector_toggle` and `tip`); their C exports `pxui_ui_inspector_section_view`, `pxui_ui_inspector_toggle_value` and `pxui_ui_tooltip` are legacy, kept for the append-only ABI. `carrying_ref`/`drop_target_ref` became `carrying`/`drop_target` (the owned copies are gone), `scroll_offset`/`scroll_position` became `Ui::scroll`, and `view_offset`/`view_opacity`/`view_scale`/`view_transform_about` became `Ui::set_view(&ViewTransform)`; the C exports are unchanged. `animate_keys_repeat` folded into `animate_keys(box, keys, &Keyframes)` (the engine's `Ui::animate_keys` has the same signature; `Keyframe`, `Keyframes` and `Repeat` live in `pxui_types::ui`, and the owned `Payload`/`Drop` moved into `pxui_core`, out of the public surface), which calls `pxui_ui_animate_keys` for the plain run and `pxui_ui_animate_keys_repeat` otherwise.

### DX1.12 (D): one container and one view call

The Rust wrapper declares containers one way (`row`/`column`/`stack`, `node` + `within`) and views one way (`view_of(key, &inputs, f)`,
`pxui_ui_view_keyed`, 1.72): it dropped `col`, `scope`, `box_`/`box_fmt` (`BoxOpts` and its hit closure), `view` and `view_flat`.
`pxui_col` and `pxui_scope` are legacy, cut at 2.0 (`pxui_ui_node` + `pxui_within` do their job); `pxui_ui_view` and `pxui_ui_view_flat`
are legacy, cut at 2.0 (`pxui_ui_view_keyed` in a node of the wanted size does their job; the flat variant had no caller, so the table
entry choice goes); `pxui_ui_box` stays the C call for a box with a hit callback.
