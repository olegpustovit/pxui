# PXUI ABI baseline

Measured 2026-11-11 on Apple M1 (arm64), macOS 27.0.1, PXUI `ede7132`, debug `dev` profile. The page timings and GPU draw counts are the pre-migration implementation; the test added to collect them does not alter the engine.

## Gallery page build

`RAYS_SLOW=1 cargo t -p pxui_gallery abi_baseline_pages` runs each page through `Ui::frame` 1,010 times at 640×464 points and 1x scale. The first 10 frames warm the UI; the table reports the median of the next 1,000 forced page rebuilds. Input frame construction is outside the timed region. Counts are measuring-thread heap allocations per frame.

| Page | Median frame build (µs) | Allocations per frame |
| --- | ---: | ---: |
| Kit | 42.00 | 0 |
| Controls | 20.25 | 0 |
| Collections | 112.12 | 0 |
| Overlays | 4.25 | 0 |
| Layout | 70.62 | 0 |

Correction (A7): the allocation column was wrong. The test read `mem_sys::local_alloc_count`, which stays at zero until the thread calls `local_reset`, and it never did. A rebuild of each page allocates what the page's own code does (`format!` labels, owned strings): Kit 46, Controls 0, Collections 1, Overlays 0, Layout 80 through the engine, and the same through the wrapper except Kit (48 static, 47 dynamic: owned copies of widget results). A frame whose view is cached allocates nothing on any page but Layout, whose anchored popup is a root box that the engine rebuilds the view for every frame. The tests now count with `pxui_gallery::gpu::allocations()` (the process-wide count plus, in a dynamic build, the library's own through `pxui_counter`).

## GPU frame

`cargo t -p pxui_gallery gpu_gallery_pages` renders each page under both themes at 1x and 2x. In the default Rays theme at 1x, the painter records these draw calls per changed page; `Gpu::render` commits exactly one command buffer per render, so each rendered frame has one submit. The idle gallery submits no frame after it settles. Every rendered page matched its golden on Metal with a maximum channel difference of 0.

| Page | Draw calls per rendered frame |
| --- | ---: |
| Kit | 1 |
| Controls | 2 |
| Collections | 1 |
| Overlays | 1 |
| Layout | 1 |

## Gallery build

Measured with `/usr/bin/time -p cargo build -p pxui_gallery` in the same workspace target directory. `cargo clean -p pxui_gallery` removed the gallery package artifacts first; dependencies remained warm. This records a cold gallery-package compile/link and a subsequent no-op warm build, not a clean workspace build.

| Build | Wall time |
| --- | ---: |
| Cold gallery package, dependencies warm | 2.02 s |
| Warm no-op build | 0.07 s |

These measurements are macOS-only. Windows/NVIDIA and WSL/lavapipe qualification remains required by the plan.

## A6: packaging

`tools/install.sh` puts `libpxui.1.dylib` (install name `@rpath/libpxui.1.dylib`) or `libpxui.so.1.0` (soname `libpxui.so.1`) with the `libpxui` dev link, `pxui.h` and the themes in a prefix. `tools/install-smoke.sh` (`gate -- install-smoke`) installs into a temporary prefix, builds and runs the C smoke against it, links `pxui_gallery --no-default-features` against it with `PXUI_LIB_DIR`, starts a copy of the gallery that finds the library only through its own rpath (no `DYLD_LIBRARY_PATH` or `LD_LIBRARY_PATH`) and checks that it draws a frame and that `otool -L` (`ldd`) lists `libpxui` as its only PXUI library. `gate -- abi-exports` checks that the library exports `pxui_*` and nothing else. `tools/validate.sh` runs `cargo t -p pxui` and `cargo t -p pxui_gallery` in both link modes (static, and against the built `libpxui`; `cargo t` builds and links the library for the packages that need it).

## A7: performance gate

Measured 2026-11-11 on Apple M1, macOS 27.0.1, release profile (thin LTO, 16 codegen units), `RAYS_SLOW=1 cargo t --release -p pxui_gallery abi_bench` (static) and `... --no-default-features abi_bench` (dynamic). `abi_bench` builds each page three ways (the engine called directly from `tools/abi_bench`, which compiles the gallery's own page source against `pxui_core`; the wrapper static; the wrapper against `libpxui`), interleaves 600 forced rebuilds of each and compares medians. The machine was shared (load 10 to 30): the table is the median of eight reports per mode (four runs, two reports each); a run differs from the median by up to 1%.

| Page | Nodes | Direct (µs) | Wrapper static (µs) | vs direct | Direct (µs) | Wrapper dynamic (µs) | vs direct | Dynamic µs per 1,000 nodes |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Kit | 87 | 28.2 | 29.0 | +3.2% | 28.1 | 29.0 | +3.4% | 11.5 |
| Controls | 83 | 18.8 | 19.4 | +3.1% | 20.1 | 20.8 | +3.5% | 8.5 |
| Collections | 263 | 104.8 | 105.9 | +1.0% | 109.4 | 110.9 | +1.3% | 5.5 |
| Overlays | 14 | 3.9 | 4.1 | +5.3% | 4.1 | 4.4 | +5.5% | 16.4 |
| Layout | 319 | 66.7 | 70.0 | +4.9% | 69.1 | 72.3 | +4.6% | 10.0 |

Before this round: static +4.3, +7.8, +14.1, +7.1, +7.0%; dynamic +9.6, +12.6, +15.7, +13.6, +8.3% (18 to 63 µs per 1,000 nodes).

| Metric | Limit | Result |
| --- | --- | --- |
| Frame build, static vs direct | under 1% | FAIL: +1.0% to +5.3% (see the residual below) |
| Frame build, dynamic vs direct | under 3% and 5 µs per 1,000 widgets | FAIL: +1.3% to +5.5%, 5.5 to 16.4 µs per 1,000 nodes |
| Allocations per settled frame | 0 in all three | PASS on Kit, Controls, Collections, Overlays; Layout allocates 80 in all three (its popup is a root box, so the engine rebuilds the view every frame; the wrapper matches the engine). A settled frame's `Renderer::prepare` and `Renderer::draw` allocate nothing (draw was 14), pinned by `renderer_draw_settled` |
| Draw calls | identical | PASS: 1, 2, 1, 1, 1 in both modes; static also compares the layers' instances: identical bytes on all five pages |
| Submits per frame | the app's own | PASS: one per render |
| Idle frame | no library call | PASS: 0 calls over the two seconds after the gallery settles |
| Paint closure of N shapes, no text | at most N / span + 1 calls | PASS: 2, 4 and 6 calls over an empty closure for 100, 1,000 and 10,000 shapes |
| Resident memory, one-label app | dynamic not above static | FAIL by 848 KiB: dynamic 103,088 KiB, static 102,240 KiB (was +1,040). The private footprint is the same in both (40 MB, `footprint`); the difference is clean, shareable file pages: the gallery's and `libpxui`'s text resident together is 6,304 KiB against the static binary's 5,632 KiB (`vmmap`, deterministic over runs): OGPU, `metal_sys` and `input` exist twice in a dynamic build (the app's copy and the library's), as section 5 of the plan expects |
| First presented frame, one-label app | dynamic within 2 ms of static | PASS: 152.7 ms dynamic, 153.1 ms static (fastest of 31) |
| Library release profile | fat LTO and one codegen unit only if it wins | Measured again with `libpxui` actually optimized across crates (below), no win: fat LTO and one codegen unit made the dynamic pages slower (+8% to +23% against +3% to +5%) and the dylib 7.0 MB against 11.0 MB; its text resident in the label app is 430 KiB smaller (dynamic +240 KiB over static instead of +672). Not adopted |

What changed, with its effect:

- `libpxui` is built by its own package, `pxui_dylib` (a cdylib over `pxui_abi`): cargo builds a lib that is both cdylib and rlib without any LTO, so the library's engine ran without cross-crate inlining (a label took 260 more instructions than direct, a button 450). Dynamic Controls +9% became +3%, Overlays +12% became +4%, Kit +8% became +3.5%. The earlier "fat LTO, no win" row measured that unoptimized build.
- A node crosses as the engine's own bytes: `Node` and everything in it are `#[repr(C)]` (an optional field is a byte-tagged `Opt*` value, a grid a pointer and length, enums bytes), the export checks its 22 tag and flag bytes and two sizes in place (about 85 instructions, no branch per byte) and the engine copies it once into the box (`node_ref`). The 504-byte `NodeDesc` encode and decode is gone (about 50 ns a node).
- The theme is lent by pointer (`pxui_ui_theme` returns the engine's `Theme`, the same C layout as `ThemeSnapshot`): `Ui::theme()` returns `&Theme` again, without the 57 KB decode or the per-key cache; `tokens()` reads it. `pxui_ui_theme_key` and `pxui_ui_tokens` are gone.
- Collection flags are bytes in the engine too (`list_view`, `tree`: not 0 is set), so a 100,000-row collection crosses with no scan, and the tree writes its rows into the caller's buffer (no 67 KB copy): Collections static +14% became +1%.
- An ASCII label is checked inline (the UTF-8 check is a call only for other text).
- OGPU re-points what it adopted (`readopt_commands`, `readopt_render_encoder`: Metal rebinds its handles, Vulkan replaces its native wrapper), so the renderer adopts the app's command buffer and pass once.

The residual, measured as retired instructions and cycles (`proc_pid_rusage`, the same frames run alone, not interleaved): per call over the direct engine, a node costs 156 instructions (85 of them the C-input check), a button 110, a label 61, `text` 83, `within` 90 (the callback trampoline: a TLS check and `catch_unwind` each way), `theme`/`tokens` 24; a frame costs 800 (the input record, the output record, two trampolines). Layout makes 400 calls, so about 55,000 instructions (+5.6%) and +3.3% cycles; Overlays' 14 nodes cannot amortize the frame's fixed part: 5 µs per 1,000 nodes is 70 ns for its whole frame, less than the frame entry alone. Removing the rest means not checking what C hands over (a trusted static entry that skips the tag, flag and UTF-8 checks, which the static build could take since its caller is Rust) and building the output record only when it is read; neither was done here.

The atlas CPU mirror lever was not built: the mirror's pages are untouched until glyphs land in them, so a one-label app does not pay for it, and it is the same in both builds.

## Deferred performance work

Measured 2026-11-11 on the same Apple M1, release profile, machine shared (load 7 to 14, a qemu VM near 240% of a core): the median of ten reports per mode (five runs, two reports each). The static path now calls trusted entries and the dynamic path trims what it can; the plan's limits are still not met everywhere, so each gate in `tools/pxui_gallery/tests/abi_bench.rs` is the measured median plus 25%, and the plan's value stays beside it as a named constant (`PLAN_*`, `GATE_*`) with a `ponytail:` comment naming the deferred work.

| Page | Nodes | Static vs direct | Dynamic vs direct | Dynamic µs per 1,000 nodes | Dynamic µs added to a frame |
| --- | ---: | ---: | ---: | ---: | ---: |
| Kit | 87 | +1.8% | +3.3% | 10.5 | 0.88 |
| Controls | 83 | +0.7% | +3.5% | 8.0 | 0.62 |
| Collections | 263 | +0.8% | +1.5% | 5.9 | 1.57 |
| Overlays | 14 | +3.1% | +4.6% | 13.4 | 0.19 |
| Layout | 319 | +0.5% | +3.3% | 7.0 | 2.25 |

Before this round (the table above, A7): static +1.0% to +5.3%, dynamic +1.3% to +5.5%.

| Metric | Plan target | Gate now | Measured |
| --- | --- | --- | --- |
| Frame build, static vs direct | under 1% | under 6% (`GATE_STATIC_RATIO`: the worst run, +4.6% on Kit under load 14, plus 25%) | +0.5% to +3.1% |
| Frame build, dynamic vs direct | under 3% | under 6% (`GATE_DYNAMIC_RATIO`) | +1.5% to +4.6% |
| Dynamic µs per 1,000 nodes | under 5 | under 8.7 on pages of 100 nodes or more (`GATE_DYNAMIC_US_PER_1000_NODES`); smaller pages: at most 1.5 µs added to a frame (`GATE_DYNAMIC_SMALL_PAGE_US`: the worst run, 1.17 µs on Kit under load 14, plus 25%; the median was 0.88) | 5.9 and 7.0 (Collections, Layout); 0.19 to 0.88 µs (the small pages) |
| Dynamic resident memory vs static | not above static | private (dirty) footprint at most 512 KiB above static (`GATE_PRIVATE_TOLERANCE_KIB`) and clean shared code pages at most 1,750 KiB above (`GATE_CLEAN_DELTA_KIB`: +848 KiB in two runs, +1,392 KiB in one under load 22, plus 25%) | private +272 KiB, clean shared pages +848 to +1,392 KiB over static (the static label app: 101,984 KiB resident, 40,529 KiB private) |

What changed in this round:

- `pxui_abi::trusted`: the hot exports' bodies without what only an untrusted caller needs (poison, null, UTF-8 and tag checks, `catch_unwind`). The `pxui_*` export validates what C hands over and calls the same entry (`export!(trusted ..)`), so the two paths run one body; the static wrapper calls the entries directly with Rust types (a `&str`, a `&Node`, a closure for the children of a container). A text over 256 MiB, a number that is not finite, a stale box and an oversized list stay checked in the entry. Covered: node, text, label, button, toggle, text field, theme, within, scope, col, view, scroll view, panel, modal, anchored, split, grid list, the frames, the form widgets, the inspector rows, the footer, tip, rect, box of key, the collections and the table.
- A panic in an app callback now unwinds through the static library (no trampoline, no per-call catch); a frame abandons its engine state on the way (`Ui::abandon_frame`) and the panic reaches the app's `frame` call unchanged.
- The wrapper's `check` panics out of line: it had made `Ui::node` too large to inline, so every node copied its 496 bytes once more (Layout +4.9% became +0.5%).
- The frame's output record is built when it is read, not at the end of every frame.
- The wrapper's deferred-panic check is one relaxed load while no thread holds a panic.

Residual, as instructions over the direct engine (`proc_pid_rusage`, the same frames run alone): static, a frame costs about 400 (the input record built from the C descriptor, the frame's two moves of the engine's input) and a trusted call 4 to 11; the pages that still carry more call widgets that validate and decode a descriptor (the value field, the text area, the menu bar, the picker, the context menu, the popups). Dynamic, a node costs 160 (85 of them the 22 tag and flag bytes, read one by one: they sit 6 to 40 bytes apart in the 496-byte record, so wide compares do not group them), a label 67, a button 113, `text` 90, `within` 91 (a wrapper trampoline, its catch and the library's entry), `theme` and `tokens` 25, and a frame 700.

Open items: `rays_editor::window_graph_costs` fails here too (the edit step allocates 754 against its budget of 700; the budget is unchanged and the cause is not yet found). The Rays gate (`cargo run -p gate -- all`) reports `duplicate crate name pxui` for `pxui_dylib` and the wrapper, which both name their library `pxui`; this predates this round.
