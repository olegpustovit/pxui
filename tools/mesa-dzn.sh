#!/usr/bin/env bash
# Build Mesa's D3D12 Vulkan ICD (dzn, "microsoft-experimental") on Ubuntu 24.04 under WSL2 into
# ~/.local/mesa-dzn, then list the adapter and the device features ogpu_vulkan requires (docs/vulkan-linux.md).
# tools/mesa-dzn.sh [--install-deps]; MESA_DZN_PREFIX, MESA_DZN_JOBS override the prefix and job count.
# --install-deps uses sudo; with SUDO_PWD set (an .envrc) the password goes to sudo -S on stdin, never to the log.
set -euo pipefail
cd "$(dirname "$0")/.."
work="$PWD/target/mesa-dzn"
prefix="${MESA_DZN_PREFIX:-$HOME/.local/mesa-dzn}"
version=25.0.7
if [[ ${1:-} == --install-deps ]]; then
    if [[ -n ${SUDO_PWD:-} ]]; then sudo() { printf '%s\n' "$SUDO_PWD" | command sudo -S -p '' "$@"; }; fi
    sudo apt-get install -y build-essential curl meson ninja-build flex bison \
        python3-mako python3-yaml python3-packaging python3-ply directx-headers-dev \
        libdrm-dev libwayland-dev wayland-protocols libx11-xcb-dev libxcb-dri3-dev \
        libxcb-present-dev libxcb-shm0-dev libxcb-xfixes0-dev libxshmfence-dev \
        libxrandr-dev libexpat1-dev zlib1g-dev libzstd-dev vulkan-tools
fi
export PATH="/usr/bin:/bin:$PATH"
mkdir -p "$work"
archive="$work/mesa-$version.tar.xz"
if [[ ! -f "$archive" ]]; then
    curl -fL --retry 3 "https://archive.mesa3d.org/mesa-$version.tar.xz" -o "$archive.download"
    mv "$archive.download" "$archive"
fi
sha256sum -c <<< "592272df3cf01e85e7db300c449df5061092574d099da275d19e97ef0510f8a6  $archive"
if [[ ! -d "$work/mesa-$version" ]]; then
    tar -xf "$archive" -C "$work"
fi
setup=()
if [[ -f "$work/build/build.ninja" ]]; then setup+=(--reconfigure); fi
meson setup "${setup[@]}" "$work/build" "$work/mesa-$version" \
    --prefix "$prefix" --libdir lib -Dvulkan-drivers=microsoft-experimental \
    -Dgallium-drivers= -Dplatforms=x11,wayland -Dbuildtype=release \
    -Dglx=disabled -Degl=disabled -Dgbm=disabled -Dllvm=disabled
ninja -C "$work/build" -j "${MESA_DZN_JOBS:-4}"
ninja -C "$work/build" install
icd="$prefix/share/vulkan/icd.d/dzn_icd.x86_64.json"
printf 'ICD: %s\n' "$icd"
export LD_LIBRARY_PATH="/usr/lib/wsl/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export VK_DRIVER_FILES="$icd" VK_ICD_FILENAMES="$icd"
vulkaninfo --summary
# ogpu_vulkan needs a Vulkan 1.3 device with these; dzn 25.0 is 1.2 and lacks most of them, so it cannot run ogpu.
vulkaninfo 2>/dev/null | grep -E '^\s*apiVersion|VK_KHR_push_descriptor |VK_EXT_extended_dynamic_state3 |VK_EXT_robustness2 |VK_KHR_incremental_present ' \
    || true
