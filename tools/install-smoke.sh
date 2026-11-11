#!/usr/bin/env bash
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
target_dir=${CARGO_TARGET_DIR:-"$repo/target"}
tmp=$(mktemp -d "${TMPDIR:-/tmp}/pxui-install-smoke.XXXXXX")
trap 'rm -rf "$tmp"' EXIT
prefix="$tmp/prefix"

CARGO_TARGET_DIR="$target_dir" "$repo/tools/install.sh" "$prefix"
test -s "$prefix/include/pxui.h"
cmp "$repo/include/pxui.h" "$prefix/include/pxui.h"

themes=0
for theme in "$repo"/assets/themes/*.theme; do
    test -f "$theme" || continue
    cmp "$theme" "$prefix/share/pxui/themes/${theme##*/}"
    themes=$((themes + 1))
done
test "$themes" -gt 0

case "$(uname -s)" in
    Darwin)
        test -L "$prefix/lib/libpxui.dylib"
        test "$(readlink "$prefix/lib/libpxui.dylib")" = libpxui.1.dylib
        test -e "$prefix/lib/libpxui.dylib"
        test -e "$prefix/lib/libpxui.1.dylib"
        runtime_var=DYLD_LIBRARY_PATH
        ;;
    Linux)
        test -L "$prefix/lib/libpxui.so"
        test -L "$prefix/lib/libpxui.so.1"
        test "$(readlink "$prefix/lib/libpxui.so")" = libpxui.so.1
        test "$(readlink "$prefix/lib/libpxui.so.1")" = libpxui.so.1.0
        test -e "$prefix/lib/libpxui.so"
        test -e "$prefix/lib/libpxui.so.1.0"
        runtime_var=LD_LIBRARY_PATH
        ;;
    *)
        echo "install smoke supports macOS and Linux" >&2
        exit 2
        ;;
esac

"${CC:-cc}" -std=c11 -I"$prefix/include" "$repo/tools/install-smoke.c" \
    -L"$prefix/lib" -Wl,-rpath,"$prefix/lib" -lpxui -o "$tmp/install-smoke"
env "$runtime_var=$prefix/lib" "$tmp/install-smoke"

# The gallery, linked dynamically against the installed prefix (PXUI_LIB_DIR) and started from a copy that finds the library only
# through its own rpath (`<prefix>/lib/gallery/..` is `<prefix>/lib`): no path variable, and libpxui is the one PXUI library it loads.
export PXUI_LIB_DIR="$prefix/lib"
CARGO_TARGET_DIR="$target_dir" cargo build -q -p pxui_gallery --no-default-features
unset PXUI_LIB_DIR
mkdir -p "$prefix/lib/gallery"
cp "$target_dir/debug/pxui_gallery" "$prefix/lib/gallery/pxui_gallery"
case "$(uname -s)" in
    Darwin) libs=$(otool -L "$prefix/lib/gallery/pxui_gallery" | tail -n +2);;
    *) libs=$(ldd "$prefix/lib/gallery/pxui_gallery");;
esac
echo "$libs" | grep -q libpxui
# nothing else of PXUI's (an ogpu, sdl3 or pxui_* library of its own) is a library the app loads
test "$(echo "$libs" | grep -ci 'pxui\|ogpu')" -eq 1 || { echo "install-smoke: the gallery loads PXUI libraries besides libpxui:"; echo "$libs"; exit 1; }
out=$(env -u DYLD_LIBRARY_PATH -u LD_LIBRARY_PATH PXUI_GALLERY_HIDDEN=1 PXUI_GALLERY_EXIT_AFTER=1 PXUI_GALLERY_COUNT=1 "$prefix/lib/gallery/pxui_gallery")
echo "$out" | grep -q 'draws [1-9]' || { echo "install-smoke: the installed-prefix gallery drew no frame: $out"; exit 1; }
echo "install-smoke: gallery from the installed prefix drew a frame; libpxui is its only PXUI library"
