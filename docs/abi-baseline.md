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
