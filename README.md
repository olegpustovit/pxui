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
input batch, call `Platform::pump`, feed `Events::encode` into `Ui::frame`, then call
`Platform::apply` once per completed UI frame for cursor, keyboard/IME, background and clipboard
requests. Numeric input and unchanged focused IME geometry allocate no Rust objects once warm;
new text and clipboard contents are separate paths.

On iOS, `pxui_sdl::ios::run` owns the window and callback. UIKit drives its display link;
`Schedule::WaitUntil` pauses it until input, a worker wake or the repaint deadline. Metal stops
new submissions while inactive and drains pending work on resign-active notifications. The
callback's `Finish` and captured resource destruction happen before window destruction. Return
errors through the callback result; a panic aborts at the C boundary. See
[`ios_spike`](tools/ios_spike/src/main.rs) and build it with
`packaging/ios-spike.sh iphonesimulator /absolute/shared/target`. Set
`SIMCTL_CHILD_PXUI_IOS_LIFECYCLE_CHECK=1` on `simctl launch`, wait for the frame-90 marker,
background it with another app, then relaunch the same bundle without terminating it. The
182-frame / 82-warm-up / 100-measured allocation gate, idle deadline and same-process resume
are qualified on the iOS 18.6 simulator. Finger gestures, safe areas, actual keyboard behavior,
physical devices and native allocations remain to qualify.

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
