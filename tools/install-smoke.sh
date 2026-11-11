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
