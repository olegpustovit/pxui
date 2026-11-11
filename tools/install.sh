#!/usr/bin/env bash
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
prefix=${1:-/usr/local}
target_dir=${CARGO_TARGET_DIR:-"$repo/target"}
cd "$repo"

cargo build --release -p pxui_abi --target-dir "$target_dir"

mkdir -p "$prefix/lib" "$prefix/include" "$prefix/share/pxui/themes"
case "$(uname -s)" in
    Darwin)
        install -m 755 "$target_dir/release/libpxui.dylib" "$prefix/lib/libpxui.1.dylib"
        ln -sfn libpxui.1.dylib "$prefix/lib/libpxui.dylib"
        ;;
    Linux)
        install -m 755 "$target_dir/release/libpxui.so" "$prefix/lib/libpxui.so.1.0"
        ln -sfn libpxui.so.1.0 "$prefix/lib/libpxui.so.1"
        ln -sfn libpxui.so.1 "$prefix/lib/libpxui.so"
        ;;
    *)
        echo "install.sh supports macOS and Linux" >&2
        exit 2
        ;;
esac

install -m 644 "$repo/include/pxui.h" "$prefix/include/pxui.h"
install -m 644 "$repo/assets/themes/"*.theme "$prefix/share/pxui/themes/"
