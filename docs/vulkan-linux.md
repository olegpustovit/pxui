# Vulkan on Linux (lavapipe, headless, WSL2)

`ogpu_vulkan` builds on Linux with no window system; any Vulkan 1.3 device works, including the CPU
driver. Needs `libvulkan-dev` and `mesa-vulkan-drivers`; `libwayland-dev` adds the Wayland surface smoke
test. The tests also need `glslangValidator`, `spirv-link` and the validation layer. Without root, unpack the
`.deb`s of `glslang-tools`, `vulkan-validationlayers` and `vulkan-tools` (`apt-get download`, `dpkg -x <deb> ~/vk/root`) and:

```sh
export PATH=~/vk/root/usr/bin:$PATH LD_LIBRARY_PATH=~/vk/root/usr/lib/x86_64-linux-gnu
export VK_ADD_LAYER_PATH=~/vk/root/usr/share/vulkan/explicit_layer.d
export XDG_RUNTIME_DIR=/mnt/wslg/runtime-dir WAYLAND_DISPLAY=wayland-0   # WSLg, for the Wayland smoke test
cargo t -p ogpu_vulkan && cargo t -p pxui_render && cargo t -p pxui_wayland
```

A changed `PATH` reruns the shader build step. Build in the Linux filesystem, not under `/mnt/c`.

WSL2 exposes the host GPU through `/dev/dxg`; Ubuntu's Mesa ships no Vulkan driver for it, so `vulkaninfo` shows only
llvmpipe. `tools/mesa-dzn.sh --install-deps` builds Mesa 25.0.7's D3D12-backed Vulkan driver (dzn,
`-Dvulkan-drivers=microsoft-experimental`) into `~/.local/mesa-dzn`; select it with
`LD_LIBRARY_PATH=/usr/lib/wsl/lib VK_DRIVER_FILES=~/.local/mesa-dzn/share/vulkan/icd.d/dzn_icd.x86_64.json`. On an RTX 2080 Ti
it reports `Microsoft Direct3D12 (NVIDIA GeForce RTX 2080 Ti)`, but as a Vulkan 1.2 device without `VK_KHR_push_descriptor`,
`VK_EXT_extended_dynamic_state3`, `VK_EXT_robustness2` or `VK_KHR_incremental_present`: `ogpu_vulkan` requires the first four
and refuses the device (`device_create` fails with `NoAdapter`), so on WSL every ogpu test and host still runs on lavapipe. The script prints those
lines so a newer Mesa that closes the gap shows up.

Compositor hooks (`ogpu_core::dmabuf`, `backend::{external, dmabuf_formats, import_dmabuf, export_sync_file,
import_sync_file}`, Linux only) degrade through `backend::external(device)`. Lavapipe in WSL imports dmabufs
(linear only: no `VK_EXT_image_drm_format_modifier`) but exports none, and has no SYNC_FD semaphores; WSL has
no `/dev/dri` or `/dev/udmabuf`. `gpu_dmabuf_render_target_fence_and_texture` therefore imports a shared-memory
file, which the CPU driver maps like a dmabuf, and `gpu_sync_file_fences` prints `RAYS_TEST_SKIP` for its GPU
half: run the suite with `RAYS_ALLOW_SKIP=1` there.

`pxui_wayland` (the Wayland window host) links `libwayland-client` and `libxkbcommon` (`libwayland-dev`,
`libxkbcommon-dev`). Its protocol glue is checked in under `crates/open/pxui_wayland/protocol`; regenerating it needs
`wayland-scanner` and the `wayland-protocols` XML (`apt-get download wayland-protocols`, `dpkg -x` into `~/vk/root`).
Its window test skips without `WAYLAND_DISPLAY`. File dialogs go through the XDG desktop portal
(`org.freedesktop.portal.FileChooser` on the session bus, through libdbus-1: `libdbus-1-dev`), parented to the window's
xdg-foreign-unstable-v2 handle when the compositor exports one; without a portal (WSL runs none) `show_dialog` returns
`NO_FILE_DIALOG`. Its tests answer the requests from a fake portal on a private `dbus-daemon` (skipped without one). WSLg's compositor offers `wl_data_device` and `wp_viewporter` but no
fractional scale, cursor shape, xdg-decoration or text-input-v3, so those paths need another compositor to exercise;
without cursor-shape-v1 the cursor comes from the XCursor theme (`libwayland-cursor`, `XCURSOR_THEME`/`XCURSOR_SIZE`).
Files dragged in as `text/uri-list` become `FileDropped` events; `set_surrounding_text` gives an input method the text
around the caret, and its deletions arrive as Backspace/Delete presses. Without `wp_viewporter` the integer buffer scale
changes with the `PixelSizeChanged` the host announces, so the resized swapchain's present carries it.
`cargo run -p pxui_wayland --example window` runs the host loop.

**Damage-only present and frame pacing (R8).** The canvas texture is the source of truth. `be::commit_present_damage`
copies into the acquired swapchain image only what that image lacks: `ogpu_core::damage::History` keeps, per image index,
the union of the damage presented since that image was last shown. Unknown images, a rebuilt (out-of-date) or resized
swapchain, and a frame without a damage list get a whole copy. When the device has `VK_KHR_incremental_present`
(lavapipe does, dzn does not), the frame's damage rides along as `VkPresentRegionsKHR`, and Mesa's Wayland WSI sends
one `wl_surface.damage_buffer` per rect. A whole frame damages `(0, 0, INT32_MAX, INT32_MAX)`. Kill switch:
`PXUI_VK_FULL_PRESENT=1` (read when the surface is created) or `be::set_full_present`. `pxui_wayland` paces by
`wl_surface.frame`: `Window::request_frame` (called right before the present, so the callback and a `wp_presentation`
feedback ride the WSI's commit), then `frame_ready` stays false until the callback is done, and `wait` wakes for it. A
host draws only when something changed (input, or a repaint due by its rate class: every callback for Display,
`repaint_at` spacing for Ambient(hz)) *and* `frame_ready`. An idle window therefore commits nothing, and a hidden or
minimised one, which gets no callbacks, stops. `frame_stats` reports callbacks, requests and presentation feedback
(presented/discarded, time, refresh, flags). `window_wayland_damage_pacing` starts its own
`weston --backend=headless --idle-time=0` and runs a child under `WAYLAND_DEBUG=client` to check those requests. Run any
long-lived weston with `--idle-time=0`: its desktop shell locks the screen after 300 s, which hides every surface, so
FIFO presents (and frame-paced hosts) stop. Weston 13 offers no `wp_fractional_scale_v1`, so the fractional path (with
`wp_viewporter`) still needs another compositor.

Design hooks, not built: (1) Promoted views (R7's `FrameOut.promoted`) map to desynchronized `wl_subsurface`s, one per
promoted view, each with its own swapchain (or a dmabuf `wl_buffer`) and its own damage history; their position goes
through `wl_subsurface.set_position` on the parent's commit, and content commits independently. (2) A DRM/KMS host
presents the same canvas through an atomic commit with `FB_DAMAGE_CLIPS` (a blob of `drm_mode_rect`, x1/y1/x2/y2 in
framebuffer pixels) filled by `ogpu_core::damage::drm_clips` from the same per-buffer `damage::History`: each scanout buffer owes its debt the same way a
swapchain image does. KMS and real-display energy are pending hardware; WSL has no `/dev/dri`.

`pxui_atspi` (the AT-SPI2 accessibility bridge) links `libdbus-1` (`libdbus-1-dev`). Its bus test starts a private
`dbus-daemon` and, when `at-spi2-core` is installed, `at-spi-bus-launcher` with the registry; it skips without
`dbus-daemon`. A libatspi client walk runs too when `/usr/bin/python3` imports `gi` with the Atspi typelib: unpack
`gir1.2-atspi-2.0` and `gir1.2-freedesktop` into `~/vk/root` and set
`GI_TYPELIB_PATH=~/vk/root/usr/lib/x86_64-linux-gnu/girepository-1.0`.

## The PXUI gallery on each platform

`tools/pxui_gallery` shows every PXUI widget on six pages (Kit, Controls, Collections, Overlays, Layout, Lists) under the Rays theme
and `assets/themes/dusk.theme` (reloaded when the file changes). It builds a frame only for input, a due repaint,
a toast's expiry or the once-a-second theme poll, and sleeps in the host's wait otherwise.

- macOS (`pxui_sdl` + Metal): `cargo run -p pxui_gallery`
- Windows (`pxui_sdl` + Vulkan): the same command, from the MSYS2 UCRT64 shell with `TMP` set to a Windows path.
- Linux (`pxui_wayland` + Vulkan, WSLg or any Wayland compositor): the environment above (`VK_ADD_LAYER_PATH` only
  for validation), then the same command. The `pxui_atspi` bridge connects only when an accessibility bus answers
  (`DBUS_SESSION_BUS_ADDRESS` or `AT_SPI_BUS_ADDRESS`); without one the gallery runs with the tree off. To walk it, start
  a private `dbus-daemon --session --print-address`, `at-spi-bus-launcher --launch-immediately` on it, run the gallery
  with `DBUS_SESSION_BUS_ADDRESS` set, and walk the application with `/usr/bin/python3` and
  `GI_TYPELIB_PATH=~/vk/root/usr/lib/x86_64-linux-gnu/girepository-1.0` (`Atspi.get_desktop(0)`; `do_action(0)` presses).
  Run python from a directory without stray `enum.py`/`re.py` files and not from the nix profile's python.

Environment switches (all optional): `PXUI_GALLERY_PAGE=0..3`, `PXUI_GALLERY_DUSK=1`, `PXUI_GALLERY_RTL=1`,
`PXUI_GALLERY_COMPACT=1`, `PXUI_GALLERY_THEME=path`, `PXUI_GALLERY_HIDDEN=1` (offscreen only, no window),
`PXUI_GALLERY_SHOT=out.png` (save the third drawn frame, then exit 0), `PXUI_GALLERY_EXIT_AFTER=secs` and
`PXUI_GALLERY_COUNT=1` (print `frames`, `draws` and `settled` on exit; `settled` counts frames built after the first half
second and must stay 0 while idle). Example: `PXUI_GALLERY_PAGE=1 PXUI_GALLERY_DUSK=1 PXUI_GALLERY_SHOT=/tmp/p.png
cargo run -p pxui_gallery`.

`cargo t -p pxui_gallery` renders every page at 1x and 2x under both themes against `tools/pxui_gallery/fixtures` (tolerance
2 per channel on every backend; lavapipe and NVIDIA stay within it) and checks the idle budget. The gallery's test image
is 100x60 because the nearest-sampled texel boundaries of a 96x64 image under a 40-point box land exactly on pixel
centres, which Metal and NVIDIA break differently (one texel row, 127 levels).
