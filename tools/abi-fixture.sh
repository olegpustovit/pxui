#!/usr/bin/env bash
# usage: tools/abi-fixture.sh <minor>   (run on macOS and on Linux at the release commit)
# Builds the old-app fixture for release 2.<minor> (the 1.x fixtures were retired, D28; U5 makes the first at 2.0): copies this commit's header and frozen manifest into fixtures/abi/2.<minor>/ and
# compiles tools/abi-old-app.c against that header and the installed library as fixtures/abi/2.<minor>/old_app-<os>-<arch>. Commit all of it.
set -euo pipefail
minor=${1:?usage: abi-fixture.sh <minor>}
repo=$(cd "$(dirname "$0")/.." && pwd)
target_dir=${CARGO_TARGET_DIR:-"$repo/target"}
dir="$repo/fixtures/abi/2.$minor"
tmp=$(mktemp -d "${TMPDIR:-/tmp}/pxui-abi-fixture.XXXXXX")
trap 'rm -rf "$tmp"' EXIT
grep -qx "minor $minor" "$repo/abi/pxui.v1.txt" || { echo "abi-fixture: abi/pxui.v1.txt is not at minor $minor" >&2; exit 1; }
mkdir -p "$dir"
cp "$repo/include/pxui.h" "$repo/abi/pxui.v1.txt" "$dir/"
cp "$repo/tools/abi-old-app.c" "$dir/old_app.c"
CARGO_TARGET_DIR="$target_dir" "$repo/tools/install.sh" "$tmp/prefix" >/dev/null
"${CC:-cc}" -std=c11 -DPXUI_FIXTURE_MINOR="$minor" -I"$dir" "$dir/old_app.c" -L"$tmp/prefix/lib" -lpxui \
    -o "$dir/old_app-$(uname -s)-$(uname -m)"
echo "abi-fixture: $dir/old_app-$(uname -s)-$(uname -m)"
