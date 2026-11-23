# ABI

The wire contract is [pxui.v1.txt](../abi/pxui.v1.txt) (file names keep `v1`: the manifest format); numeric layouts are in
[layout.v1.txt](../abi/layout.v1.txt); the extension set is [pxui.ext.txt](../abi/pxui.ext.txt). Change the exports, header, wrapper,
snapshots and C layout assertions together; rebuild app and dylib together. Static/dynamic validation, pixel and warm-allocation checks
remain required.

## Policy

**Baseline.** This is ABI 2.0, a clean cut (owner D28; the 1.x interface, its history and its fixtures are gone). From here the C
interface is stable: an export is never removed, renamed or re-signatured, a record's layout is never altered except by growing at its
end, and an enum or flag value is never reused. `abi-snapshot` records the manifests, `abi-frozen` compares them with the copy kept in
`fixtures/abi/2.N/` and `abi-compat` runs the old-app binary kept there against the library built from the tree.

**Version.** `pxui_abi_version()` returns `major << 16 | minor` (`ABI_VERSION` in `pxui_abi`, the `minor N` line of `abi/pxui.v1.txt`,
`pxui_types::ABI_BUILT` and `PXUI_ABI_BUILT` in `pxui.h`, kept equal by tests). The shared library is `libpxui.2` (`libpxui.2.dylib`,
install name `@rpath/libpxui.2.dylib`, compatibility version 2.0.0 and current version 2.3.0; ELF soname `libpxui.so.2`). Minor grows by one for each
additive change.

**Promise.** An app built against 2.N runs unchanged against any later 2.M library (M >= N): every export with its signature, every
record layout and field offset, every flag, enum and code value, the behaviour of existing calls on inputs they accepted, and identity
hashes once a released minor has frozen them. **Minimum library:** an app states the version it was built for
(`PxuiUiDesc.abi_built`, `PXUI_ABI_BUILT`) and `pxui_ui_create` refuses another major (a 1.x app), an unstated version (0) and a newer
minor than the library's own with `PXUI_ERR_VERSION`, creating nothing (the Rust `Ui::new` returns `Error::Version`). The library therefore
never reads a caller record grown past what it knows, and an app never reads a library-written record (a body config, a snapshot) the
library did not write at that size.

**Append-only rules.**

- Exports are never removed, renamed or re-signatured. A different shape is a new export.
- Records are `repr(C)` with `size: u32` first and one size each at 2.0 (no earlier layout is read). **Growth (enforced by `abi-snapshot`
  and `abi-frozen`):** a recorded layout row changes only by growing at its end, under a minor bump: the same name and alignment, a larger
  size, every recorded field offset in place and every recorded field's type unchanged, the new fields appended at or past the recorded
  size. Only two kinds of record may grow: (a) a record the library writes and the app reads (the config a `body_kinds!` line names, a
  borrowed snapshot an export returns as `const T *`); a reader built against the smaller layout reads its prefix, and a reader built against
  the larger one reads a grown field only when the record's `size` covers it or after checking the library's minor; (b) a record the caller
  writes whose first field is `size` and that the library reads at the minimum size it has had: a caller record is read from `size` bytes
  when `size` covers the 2.0 layout (`read_desc!`), copies no more than it knows, and treats every grown field as zero (the DX7.4
  minimum-size rule, tested by `sized_records`). Any other change of a recorded row is refused. An output record is written only up to the
  caller's `size`.
- Spans cross as `{ptr, len, stride}` with the stride at least the record's size; elements may grow.
- Enum and flag values are never reused or renumbered. A retired value stays reserved.
- `abi/pxui.v1.txt` and `abi/layout.v1.txt` change only by appending lines or by a row grown under the growth rule; each change takes a
  minor bump and a row in the history table below. The manifests are written by the gates (`abi-snapshot --write`,
  `PXUI_LAYOUT_WRITE=1`), never by hand.
- One minor bump per additive change, made in the same commit as the export, header, snapshots and docs row.

**Reserved flag bits.** Node flags (`Flags`, `u64`) use bits 0-21 today (highest is `FIT_TEXT_PIXEL`, 1 << 21). Bits 22-62 are
reserved for future minors and must be passed as zero. Bit 63 is unassigned.

**Limits.** An untrusted caller (a compositor hosts many apps) can make no export panic, abort, poison its `Ui` or allocate without
bound; every limit refuses with an error, never cuts. A test calls every export the manifests list, outside a frame and inside one,
with zero, null, garbage and oversized arguments, and fails on any panic, `PXUI_ERR_PANIC` or poisoned `Ui` (`pxui_abi` `fuzz.rs`; a
name missing from it fails too). The export macro's `catch_unwind` is the backstop for a library bug, not a way to refuse an argument.
| Input | Cap | Past it |
| --- | --- | --- |
| An identifier: a key (`PxuiKeyedDesc.key`), a node or container label used as identity, a body, theme-file or bridge name, a carry kind | 256 bytes (`MAX_KEY_BYTES`) | refused; the Rust wrapper panics (a programmer error: keys are `&'static str` or ids) |
| Display text: labels, titles, accessible names (`PxuiKeyedDesc.name`) and values, tips, menu and list rows, hints, options; a node's text with spans or as a line box (`pxui_ui_text` with spans or a line box) | 64 KiB (`MAX_LABEL_BYTES`) | refused from C; the Rust wrapper clips it at a character boundary first (user data never panics) |
| Editor text: a text field's or area's value, a splice, a paste, an edit, a theme source, text the text service measures or shapes (`ui_label_measure`, `ui_label_quads`, `ui_label_rasterize_rgba`, the shaper's); one event's text | 16 MiB (`MAX_TEXT_BYTES`) | refused |
| A text prepared off the UI thread (`shaper_prepare`) | 1 MiB (`pxui_text` `MAX_PREPARED_BYTES`) | refused |
| `ui_label_prewarm` text | 16 KiB | refused |
| A font; fonts per text service; font bytes per text service | 32 MiB; 64; 256 MiB | refused |
| A call's string span (options, rows, titles, hints, a text area's messages and splices); one frame's event text (or one `ui_wants_frame` call's) | 16 MiB together (`MAX_TOTAL_TEXT_BYTES`) | refused |
| Strings the library hands back between two frames (edited text, lent payloads, a picker query) | 4096 and 64 MiB (`MAX_RETURNED`, `MAX_RETURNED_BYTES`) | `PXUI_ERR_STATE` until the next frame |
| Pending accessibility actions per Ui, across calls | 4096 | whole batch refused with `PXUI_ERR_STATE` until the next frame or surface removal |
| `FrameIn.width`/`height`, `PxuiInput.logical_size`, a resize or pixel-size event | `0..=65536` (`MAX_SURFACE`) | refused |

**Error code of a cap.** An argument past its cap is `PXUI_ERR_ARGUMENT` on every path (C exports, trusted entries, the text service; a
call that returns a value answers its failure value: null, 0, `SIZE_MAX` or `UINT32_MAX` as its row says), with the reason in
`pxui_last_error`. The budgets across calls (strings handed back since the frame began and pending accessibility actions) answer
`PXUI_ERR_STATE` (the call is not allowed now; the next frame resets them). Nothing is cut on the C side; only the Rust wrapper clips display text.

- Events (`pxui_frame`, `pxui_ui_wants_frame`: one validator): `x`, `y` and `value` finite for every kind; text editing start and
  length not negative; a resize or pixel-size change within `0..=65536`; a known kind and code. `FrameIn.width`/`height` and
  `PxuiInput.logical_size` within `0..=65536` points (`MAX_SURFACE`); a held key code that names no key is refused as the event path
  refuses it.
- Builders (every call that declares nodes: widgets, containers, `ui_node`, `ui_box`, `ui_draw`, `part`, ...) answer `PXUI_ERR_STATE`
  outside a frame callback, before the engine is touched (`enter!` and the `build` export form); the `Ui` is not poisoned.
- A shaper, a theme and an accessibility bridge whose call panicked are poisoned like a `Ui`: every later call answers its failure value
  at once; destroy still frees them.
- Calls are counted per `Ui` (`pxui_ui_diagnostics`, extension set): no counter is shared between `Ui`s or threads.
- A shaper needs a font: `shaper_create` with none (or only refused ones) is null, never a panic at its first text.


**Version and features.** `pxui_supports(id)` returns 1 when the library, as built, has the feature with that id and 0 otherwise or for
an id it does not know. Ids (`PXUI_FEATURE_*` in `pxui.h`, `pxui_types::feature`, `pxui::Feature`) are append-only, never reused or
renumbered, and 0 is never an id; none is named for a version. A feature whose presence depends on the build answers for the library that
was built; nothing else in the header is asked about: it is in every build of the major, and a feature of a later minor answers 0 on an
older library.

| Id | Name | Meaning |
| --- | --- | --- |
| 1 | `A11Y_BRIDGE` | `pxui_a11y_*`: the AT-SPI2 bridge (Linux builds) |
| 2 | `METAL` | the Metal backend (Apple targets; answers from the target today, switch to `cfg!(feature)` if a backend becomes optional) |
| 3 | `VULKAN` | the Vulkan backend (Windows and Linux targets; same note) |
| 4 | `EXTENSION_SET` | `abi/pxui.ext.txt` exported (no stability promise) |

**Extension set.** The Rays vocabulary and the diagnostics are not part of the promise: `inspector_*`, `footer`, `grip`, `field_drag`,
`line_field`, `patch_field`, `value_field`, `picker`, `pxui_ui_diagnostics` and `pxui_counter`, with the records only they use. They are
listed in `abi/pxui.ext.txt`, which carries no stability guarantee: they may change or disappear between minors, and an app pins the commit
if it uses them (`PXUI_FEATURE_EXTENSION_SET`). The calls stay exported and declared in `include/pxui.h`. The set is the names in that file;
the `abi-snapshot` gate keeps its rows current (`cargo run -p gate -- abi-snapshot --write`) and `abi/pxui.v1.txt` and `abi/layout.v1.txt`
carry none of them. Moving a call into the set is an edit of that file, reviewed as one; a row left in the frozen manifest is frozen.
*Extension body kinds:* the body kinds `inspector_row`, `footer`, `grip`, `line_field`, `value_field`, `picker` and
`picker_field` build the Rays vocabulary; their numbers in `PxuiBodyKind` and their slots in `PxuiBodies` are fixed like every kind's, but
their `Pxui*Config` records (in the extension layout rows) carry no stability promise. `inspector_section` is also the body of the generic
`pxui_ui_disclosure`, so its record stays frozen.


**Handle head.** `PxuiUi` is opaque except its first field, `const PxuiThemeSnapshot *theme` at offset 0, frozen at 2.0 (owner ruling, 21 Nov 2026): read-only for the app, NULL until a theme read on the handle (`pxui_ui_theme`) fills it, then valid for as long as the handle is and not rewritten while the handle is usable (a theme swap is refused inside a frame; a themed scope builds through a handle of its own). The snapshot record behind it follows the growth rule (its parts are last, `size` and `part_count` say how much was written). Nothing else of `PxuiUi` is part of the interface.

**Contracts.** The header states the error model, the thread and lifetime contract, the raw-handle exception (D16: worker, shaper,
prepared-text, accessibility and renderer pointers are not validated), per-Ui body names and the GPU-device decision (OS-6, D24: a renderer
owns its device; sharing one across windows is a host concern deferred past 2.0). The `_plain` calls (`pxui_ui_button_plain`,
`pxui_ui_icon_button_plain`, `pxui_ui_toggle_plain`) are the string-only fast path of the descriptor forms, not duplicates.

**Accessibility surfaces.** One bridge publishes a tree per nonzero surface id. Each `PxuiA11yAction.surface` identifies its destination;
`pxui_ui_inject_a11y` refuses a whole batch containing another surface before applying any action. `pxui_a11y_remove_surface` releases
a closed surface's tree (unknown nonzero ids are ignored). Owner approved this correction and regeneration of the unreleased 2.0 baseline
on 21 Nov 2026, before the U6 release freeze.

**Old-app fixture.** At each release `tools/abi-fixture.sh <minor>` (on macOS and on Linux) keeps the release's header, its manifest and a C
app built against it (`tools/abi-old-app.c`) in `fixtures/abi/2.<minor>/`. `cargo run -p gate -- abi-compat` runs every kept binary against
the library built from the tree, and `abi-frozen` fails when a row the fixture's manifest froze is missing or changed in `abi/pxui.v1.txt`
(a layout row may only grow at its end behind its size prefix), or when the manifest gained a row without a later minor: editing
`abi/pxui.v1.txt` to match changed code does not touch the fixture's copy. `pxui_abi`'s `the_library_checks_the_version_the_app_was_built_against_once_at_create`
tests that a 1.x app, an unstated one and a newer minor are refused cleanly.

**Adding a call.** In one commit: the `pxui_abi` export; the wrapper method in `pxui`; the declaration in `include/pxui.h`; the appended
row in `abi/pxui.v1.txt` (and `abi/layout.v1.txt` for a new record); the history row below; the `ABI_VERSION` and `ABI_BUILT` bump with
the `minor` line. Then run the `abi-header`, `abi-snapshot`, `abi-frozen` and `abi-compat` gates.

**Deprecation.** A deprecated call is kept forever and keeps its behaviour. Mark it in its docs row (and `#[deprecated]` on the wrapper)
and name the replacement; nothing is removed from the manifest.

## History

| Version | Change |
| --- | --- |
| 2.0 | The clean cut. Dropped the 1.x twins and legacy forms (the text-callback list, tree and table forms, the label-keyed widget exports, the integer-size paint text calls, `pxui_ui_variant`, `pxui_theme_load`, `pxui_verify_new_views`, `pxui_ui_frame`); one plain name per job (`pxui_ui_create` with the font list and a status, `pxui_ui_label`, `pxui_ui_button`, `pxui_ui_animate_keys`, `pxui_ui_text`, `pxui_shaper_measure`, the `pxui_ui_*` prefix for `within`, `part`, `part_box`, `slot`, `scope`); size-prefixed records with one size each and strided `{ptr, len, stride}` spans; sized per-surface events and frames; the variant a field of each widget record; one record per menu row (`PxuiMenuRow`, `PxuiMenuOptions`); the DX1.12G body names; features reset to four ids; `pxui_ui_diagnostics` and `pxui_counter` in the extension set with the Rays vocabulary; `PxuiUi.theme` frozen as the one read-only head field (owner, 21 Nov 2026); `PXUI_ERR_VERSION`; `libpxui.2`. |
| 2.1 | Added `pxui_theme_animate_swap` for instant appearance changes and `pxui_ui_label_at` for keyed, positioned line-box labels. |
| 2.2 | Added `pxui_ui_retain_palette_labels` for instant palette changes in positioned-label views and `pxui_ui_retain_view_on_resize` for viewport-independent views. |
| 2.3 | Added `pxui_ui_retain_palette_paint` for palette-independent canvas and positioned-label views. `PxuiPaintInfo.reserved[0]` signals symbolic shape colors in that callback. |
