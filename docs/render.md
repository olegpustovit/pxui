# Rendering and presentation

pxui builds a retained draw list in logical window points. The renderer uses damage to decide which parts of a
retained canvas to repaint. The presenter then ensures that the window receives a current image. Drawing,
copying to a swapchain image and reporting surface damage are separate operations.

## Canvas damage

[`DamageList`](../crates/public/pxui_types/src/damage.rs) holds up to eight rectangles. It clips them to the
viewport, drops contained rectangles and joins touching rectangles when the bounding box adds at most 25% to
their combined area. At capacity it merges the pair that adds the least area, including the incoming rectangle
in the search. A sum above 60% of the viewport becomes full damage. An empty list means there are no changed
pixels. `Ui::damage_list()` exposes the list to hosts. Single-rectangle ABI fields report its bounding box for
existing clients.

Movement, patching and removal damage the old coverage as well as the new coverage when present. Theme,
density and viewport changes can invalidate the entire canvas. Damage from an unacknowledged frame carries
into the next build or step. The host also holds damage until it can render the canvas. On Metal, a submitted
canvas update can still owe a present if no drawable was available. A later frame can pay that debt by
presenting the current canvas.

A backdrop blur samples beyond its destination. Its source footprint expands the destination by `3 × sigma + 8
/ scale` points on each side, covering the filter taps, pixel rounding and downsample blocks. Damage
intersecting that footprint adds the blur destination to the list. Repeat until stable because one blur can
feed another. Reusing a cached blur requires a host-owned retained target, ordered submission and complete
backdrop damage. The renderer rechecks the footprint at target scale. A cache miss during a partial Metal pass
causes a full canvas redraw so the blur can sample a complete backdrop.

## Culling contract

Bounds must cover every pixel the instance can affect, including strokes, shadows and antialiasing. Extra
coverage costs work. Missing coverage changes pixels. At the host boundary, round damage outward to drawable
pixels, pad it for antialiasing and clamp to the target. [The
culler](../crates/private/pxui_render/src/cull.rs) tests view clips and maps scissors into conservative
view-local bounds. It uses the inverse transform and adds a margin for device-pixel coverage. A singular or
unusable inverse keeps candidates conservatively.

The culler walks view runs and, where needed, instance bounds in paint order. Each selected range carries a
bit for each damage rectangle it may touch. The renderer respects the original paint order and texture breaks
for each scissor. It can pack selected ranges into its existing upload ring to avoid submitting intervening
instances. It divides overlapping rectangles into disjoint scissor pieces so a translucent instance blends at
most once at each damaged pixel. Geometry covering several pieces may be submitted under several scissors. The
single-scissor draw is still used and can replay a dense layer under that scissor. Selection and scissoring
leave retained instance bytes intact.

## Presentation

**Metal.** Draw into the retained canvas and copy the whole canvas to each presented drawable. Partial canvas
repaint still saves scene work. Acquire the drawable late or use the drawable loan from the display link. A
new canvas, changed presentation generation or invalidated GPU state requires a full canvas repaint. The
whole-canvas present needs no drawable-content history.

R4b measured two alternate Metal presenters on the M1 Mac mini at 1440 × 872, with 160 presents per 14-second
trace. Strategy A copied accumulated damage from the canvas into known drawables. Strategy B rendered directly
into known drawables. After culling, the full canvas copy had the lowest total GPU time in both scenarios, so
the experimental paths and Metal present verifier were removed. The R4b.2 measurements in GPU ms per present
were:

| Presenter | Playing list | Pointer |
| --- | ---: | ---: |
| Retained canvas, full copy | 0.25347 | 0.84984 |
| Damage copy | 0.67483 | 0.93949 |
| Direct damage rendering | 0.35071 | 1.42840 |
| Direct full rendering | 1.96048 | 1.96091 |

**Linux/Vulkan.** The retained canvas is authoritative. `commit_present_damage` records current canvas damage
for each swapchain image. [Image history](../crates/open/ogpu_core/src/damage.rs) tracks up to 16 image
indices per generation, with up to eight rectangles per image. The acquired image gets the damage it missed
since its last present plus this frame's damage. Overflow collapses to a bounding box. An unknown image, a new
generation or extent, missing damage information, or debt above 60% of the image means a full copy. A failed
present invalidates the history. When `VK_KHR_incremental_present` is available, the present also reports
damage from the current frame to the compositor. Mesa's Wayland WSI translates those regions into
`wl_surface.damage_buffer` requests. This hint and the image's accumulated copy debt serve different
consumers. See [Vulkan on Linux](vulkan-linux.md) for the Wayland probe and the subsurface and KMS design
hooks.

## Scheduling and visibility

Input and due deadlines can wake UI work. An animation-only step can update retained output when no build is
needed. A scheduled step, a canvas draw and a window present are separate events. A stable visible window can
wait for input or its next deadline without drawing.

- `Rate::Display` lets visible transitions, display-rate keyframes and interactive motion follow display
  updates. Finite keyframes default to this class.
- `Rate::Ambient(hz)` limits ambient work once interaction settles. Indefinitely repeating keyframes default
  to ambient 30 Hz. The fastest visible ambient rate among canvas animations establishes a shared fixed
  phase. A due build or step samples the latest scheduled tick and requests the next tick on that phase.
  Early builds reuse the current ambient sample. Late callbacks skip missed ticks without shifting the phase
  or issuing catch-up frames. Ambient motion samples at display rate for one second after input.
- Apple hosts use `CAMetalDisplayLink` when available. Its rate hint guides callback delivery. The host checks
  the actual repaint deadline before treating a callback as due work and pauses the link while waiting without
  imminent work. macOS occlusion and minimization suspend drawing while allowing control callbacks for worker
  wakes and deadlines. Exposure refreshes the presentation. iOS pauses drawing and the link in the background
  while allowing control callbacks.
- A Wayland host requests `wl_surface.frame` just before presenting and waits for `frame_ready()` before the
  next changed frame. When the compositor withholds callbacks for a hidden or minimized surface, drawing stops
  after any already invited frame. `frame_stats()` reports callbacks and presentation feedback.

## Apple compositor promotion

Promotion is restricted to content with verified coverage and timing parity. The [eligibility
check](../crates/private/pxui_core/src/ui_core/promotion.rs) accepts a single plain rectangular fill from a
static hinted view or an eligible repeating view-only keyframe animation. The current vector `CALayer` path is
qualified at 1× and 2×, with pixel-aligned fixed left and right edges, a pixel-aligned clip and motion of at
most one of the top and bottom edges. It maps the supported Bézier segments to Core Animation on a shared
clock. Unsupported geometry or motion and animated ancestors keep the content on the canvas. Backdrop blur
disables promotion. The native parity test compares 200 samples per density with a maximum difference of one
level per channel.

Compare the entire clipped swept region against conservatively padded later canvas geometry. Any intersection
can change z-order because the native layers sit above the canvas. Overlapping promoted candidates also reject
the earlier candidate. When a candidate loses eligibility, damage its swept coverage and restore its retained
instances to the canvas in the same update that removes its native layer. A newly promoted layer likewise
damages its canvas coverage. Ordinary canvas draws skip promoted instances, and a capture can replay those
retained instances through the in-app renderer. Layer changes and canvas presentation use the same
transaction. With display-link pacing, both wait for a drawable loan. Eligible loops run in Core Animation
without requesting app frames for their own motion.

## Kill switches

Set environment flags before creating the applicable renderer, host or surface.

| Switch | Restores |
| --- | --- |
| `PXUI_FULL_DAMAGE=1` or `Graphics::set_full_damage` | Full canvas redraw for damaged frames. Idle frames can still be skipped. |
| `PXUI_NO_CULL=1` or `Graphics::set_no_cull` | Replay all canvas geometry under each damage scissor. |
| `PXUI_VK_FULL_PRESENT=1` or `ogpu_core::backend::set_full_present` on Vulkan | Whole canvas-to-image copies without incremental present regions. Metal always copies the full canvas. |
| `PXUI_NO_LAYERS=1` or `Graphics::set_no_layers` | Render otherwise promoted views and their animations in the canvas. |
| `PXUI_TIMER_PACING=1` on Apple | Timer and event pacing in place of Metal display-link updates. |
| `PXUI_NO_BLUR_REUSE=1` | Recompute the backdrop blur even when the host opted in to reuse using `PXUI_TARGET_BACKDROP_UI_DAMAGE`. |
| `PXUI_LEGACY_BLUR=1` | Use the original blur taps without the large-radius box prefilter. |
| `PXUI_LEGACY_CLIP=1` | Evaluate instance and view clips even when a clip is absent. |
| `PXUI_NO_OPAQUE_FAST=1` | Use analytic shading for the interior of eligible opaque rectangles. |

## Measuring

Run the signed release scenarios from the LocalWave checkout on a quiet machine. Keep the display, density,
drawable dimensions and power conditions fixed across runs.

```sh
make bundle
for scenario in playing-list idle pointer hidden; do
    tools/energy/run.sh "$scenario" 5
done
```

The runner uses an isolated signed bundle and writes `target/energy/<scenario>.json` plus the raw trace and
app log under `target/energy/`. It records presents, GPU ms/s by encoder and channel, process CPU, idle
wakeups and app counter deltas. Use the duration and drawable size in `docs/energy-baseline.json` when
comparing to the checked-in baseline. Use `LOCALWAVE_ENERGY_RECORD=1` to collect a new sample without baseline
comparison. Use `LOCALWAVE_ENERGY_POWER=1` when noninteractive `sudo` access to `powermetrics` is available.

For a matched comparison, the fixed replay submits 160 frames at 20 Hz within a 14-second trace. The runner
requires both the app draw count and the traced present count to equal 160.

```sh
for scenario in playing-list pointer; do
    LOCALWAVE_ENERGY_FIXED=1 LOCALWAVE_ENERGY_RECORD=1 tools/energy/run.sh "$scenario" 14
done
```

To isolate encoder work in an existing run, attach Metal System Trace to the target process:

```sh
xcrun xctrace record --template 'Metal System Trace' --attach "$pid" \
    --no-prompt --time-limit 5s --output render.trace
for schema in ca-client-present-request metal-gpu-intervals; do
    xcrun xctrace export --input render.trace \
        --xpath "/trace-toc/run/data/table[@schema='$schema']" > "$schema.xml"
done
```

Filter `ca-client-present-request` and `metal-gpu-intervals` to the app PID. Compare the canvas and
full-present encoders separately. Divide GPU time by trace duration for GPU ms/s and by the present count for
GPU ms/present. Use the same workload with one kill switch changed at a time. Keep CPU time, GPU time and wall
time as separate measurements.

Set `PXUI_FRAME_REASONS=1` before creating the UI. Read deltas from `Ui::frame_reasons()` or
`pxui_ui_frame_reasons` to distinguish explicit requests, interaction, transitions, finite keyframes and
active or idle loops. The step counters show successful animation steps and refusals due to state, rebuilds,
interaction or deadlines. The sampled node key and curve help identify unexpected display-rate animations.
LocalWave's energy runner sets the flag itself and records draw, step, transition and finite-keyframe deltas.
Use the trace for present counts because scheduling counters also advance on frames that never present.

The partial/full differential tests compare the same sequence of UI states rendered both ways. Metal canvas
pixels must match byte for byte. Vulkan/lavapipe permits two levels per channel. Include culling disabled and
full-damage runs when diagnosing differences, and preserve the warm-allocation checks and retained instance
fingerprints.
