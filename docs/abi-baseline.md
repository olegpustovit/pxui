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

Measured 2026-11-11 on Apple M1, macOS 27.0.1, release profile (thin LTO, 16 codegen units), `RAYS_SLOW=1 cargo t --release -p pxui_gallery abi_bench` (static) and `... --no-default-features abi_bench` (dynamic). One machine with other work running: runs repeated, medians below, differences under 1% between runs. `abi_bench` builds each page three ways (the engine called directly from `tools/abi_bench`, which compiles the gallery's own page source against `pxui_core`; the wrapper static; the wrapper against `libpxui`), interleaves 600 forced rebuilds of each and compares medians.

| Page | Nodes | Direct (µs) | Wrapper static (µs) | vs direct | Wrapper dynamic (µs) | vs direct | Dynamic µs per 1,000 nodes |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Kit | 87 | 28.0 | 29.3 | +4.3% | 31.0 | +9.6% | 31 |
| Controls | 83 | 19.8 | 21.3 | +7.8% | 24.2 | +12.6% | 33 |
| Collections | 263 | 109.5 | 124.9 | +14.1% | 121.8 | +15.7% | 63 |
| Overlays | 14 | 4.1 | 4.4 | +7.1% | 4.5 | +13.6% | 39 |
| Layout | 319 | 69.3 | 74.1 | +7.0% | 75.7 | +8.3% | 18 |

| Metric | Limit | Result |
| --- | --- | --- |
| Frame build, static vs direct | under 1% | FAIL: +4% to +14% |
| Frame build, dynamic vs direct | under 3% and 5 µs per 1,000 widgets | FAIL: +8% to +16%, 18 to 63 µs per 1,000 nodes |
| Allocations per settled frame | 0 in all three | PASS on Kit, Controls, Collections, Overlays. Layout allocates 80 in all three (its popup is a root box, so the engine rebuilds the view every frame; the wrapper matches the engine) |
| Draw calls | identical | PASS: 1, 2, 1, 1, 1 in both modes, as the engine's before the migration (static also compares the layers' instances: identical bytes on all five pages) |
| Submits per frame | the app's own | PASS: one per render, the epoch of the app's queue advances by one |
| Idle frame | no library call | PASS: 0 calls over the two seconds after the gallery settles (the frame gate is decided in the app from the last frame's repaint time) |
| Paint closure of N shapes, no text | at most N / span + 1 calls | PASS counting the span a partly filled one still is: 2, 4 and 6 calls over an empty closure for 100, 1,000 and 10,000 shapes |
| Resident memory, one-label app | dynamic not above static | FAIL: dynamic 103,072 KiB, static 102,032 KiB (+1,040 KiB, repeatable) |
| First presented frame, one-label app | dynamic within 2 ms of static | PASS: 148.1 ms dynamic, 153.5 ms static (fastest of 31 runs; medians 172.0 and 170.4) |
| Library release profile | fat LTO and one codegen unit only if it wins | Measured, no win: dynamic page ratios unchanged to 0.5% over four alternating runs, resident memory 103,184 against 103,200 KiB, first frame inside the noise; the dylib is 7.2 MB against 10.6 MB. Not adopted (no `dist` profile) |

Where the time goes (sample of the Layout and Collections pages): the wrapper turns each `Node` into a `NodeDesc` and the library back into an engine `Node` (about 45 ns a node, mostly struct copies), and a 100,000-row collection is validated on every call because C owns the flags (`list_view`, `tree`: byte flags 0 or 1 and the tree's `children`, 1.6 MB of nodes, about 15 µs of Collections' 16). Changes made on the way, with their effect on these pages: `Ui::theme()` decoded 50 KB on every call (Controls 11x and Layout 4.8x the engine; now cached per theme under `pxui_ui_theme_key`, a new export), the library built a new event `Vec` every frame (one allocation per frame, now reused), an app image got a new engine handle per call (four uses of one texture drew five times instead of twice, now one handle per texture), the flag scan of collections stopped short at the first bad byte and so could not vectorize (Collections +59% became +14%), and a plain node cleared 1 KiB of grid track storage it did not use. Not done: encode and decode the node once (a `Node` that the library can use as is), engine entry points that take u8 flags so a collection needs no scan, and the OGPU adopt allocations (a settled `Renderer::draw` makes 14: 8 adopting the commands, 5 the encoder, 1 at the end; pinned by `renderer_draw_settled`).

The atlas CPU mirror lever was not built: the mirror's pages are untouched until glyphs land in them, so a one-label app does not pay for it, and it is the same in both builds, so it does not explain the dynamic build's extra megabyte.
