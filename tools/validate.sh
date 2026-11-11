#!/usr/bin/env bash
# Local validation; there is no hosted CI. Runs on macOS (Metal) by default and never executes remotely.
#   tools/validate.sh              macOS: workspace check, gates, cargo t on each PXUI crate with its wall time, the wrapper and the
#                                  gallery in both link modes (static, and against the built libpxui), the installed-prefix check and,
#                                  last, the A7 performance gate in both modes
#   tools/validate.sh --windows    prints the commands to run on the Windows box (Vulkan, RTX)
#   tools/validate.sh --linux      prints the commands to run under WSL2 lavapipe
set -euo pipefail
cd "$(dirname "$0")/.."

case "${1:-}" in
--windows) cat <<'TXT'
On the Windows machine (Rust 1.99.1 windows-gnu per rust-toolchain.toml, MSYS2 UCRT64 with SDL3 in C:/msys64/ucrt64):
  cargo check --workspace --tests --locked
  cargo run -q -p gate -- deps
  cargo run -q -p gate -- shape
  cargo run -q -p gate -- tiers
  cargo t -p pxui_abi -p pxui_core -p pxui_render -p pxui_text -p ogpu_core -p ogpu_mock -p ogpu_vulkan -p pxui_sdl -p pxui_gallery -p input -p mem_sys -p native_layer_token_sys -p sdl3 -p sdl3_sys -p vulkan_sys -p cfmt
Needs the Vulkan SDK validation layer for ogpu_vulkan; a changed PATH reruns the shader build step.
TXT
  exit 0;;
--linux) cat <<'TXT'
In WSL2 (build in the Linux filesystem, not /mnt/c). Install libvulkan-dev mesa-vulkan-drivers libwayland-dev
libxkbcommon-dev libdbus-1-dev at-spi2-core glslang-tools vulkan-validationlayers; without root unpack the
.debs into ~/vk/root as in docs/vulkan-linux.md, then:
  export PATH=~/vk/root/usr/bin:$PATH LD_LIBRARY_PATH=~/vk/root/usr/lib/x86_64-linux-gnu
  export VK_ADD_LAYER_PATH=~/vk/root/usr/share/vulkan/explicit_layer.d
  export XDG_RUNTIME_DIR=/mnt/wslg/runtime-dir WAYLAND_DISPLAY=wayland-0   # WSLg, for the Wayland smoke test
  cargo run -q -p gate -- deps
  cargo run -q -p gate -- shape
  cargo run -q -p gate -- tiers
  RAYS_ALLOW_SKIP=1 cargo t -p pxui_abi -p pxui_atspi -p pxui_core -p pxui_render -p pxui_text -p ogpu_core -p ogpu_mock -p ogpu_vulkan -p pxui_sdl -p pxui_gallery -p pxui_wayland -p input -p mem_sys -p native_layer_token_sys -p sdl3 -p sdl3_sys -p vulkan_sys -p cfmt
TXT
  exit 0;;
"") ;;
*) echo "usage: $0 [--windows|--linux]" >&2; exit 2;;
esac

cargo check --workspace --tests
cargo run -q -p gate -- deps
cargo run -q -p gate -- shape
cargo run -q -p gate -- tiers
cargo run -q -p gate -- abi-header
cargo run -q -p gate -- abi-snapshot
cargo t -p gate
cargo t -p rays-t pxui_packages_run
for crate in pxui_abi pxui_atspi pxui_core pxui_render pxui_text ogpu_core ogpu_mock ogpu_metal ogpu_vulkan pxui_sdl pxui_wayland input mem_sys metal_sys native_layer_token_sys sdl3 sdl3_sys vulkan_sys cfmt pxui_gallery; do
    start=$SECONDS
    cargo t -p "$crate"
    echo "validate: $crate passed in $((SECONDS - start)) s"
done
# the dynamic mode: the same wrapper and gallery tests against the built libpxui, then the installed-prefix check
cargo t -p pxui
cargo t -p pxui --features static
cargo t -p pxui_gallery --no-default-features
cargo run -q -p gate -- abi-exports
cargo run -q -p gate -- install-smoke
# the A7 performance gate, last: static (builds the number the dynamic run compares first-frame time and memory with), then dynamic
RAYS_SLOW=1 cargo t --release -p pxui_gallery abi_bench
RAYS_SLOW=1 cargo t --release -p pxui_gallery --no-default-features abi_bench
echo "validate: ok"
