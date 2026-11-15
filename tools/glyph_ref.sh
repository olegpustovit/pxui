#!/bin/sh
# Renders the CoreText reference rasters into $1 (default target/glyph_ref), then prints the MAE table:
#   tools/glyph_ref.sh [dir]     (macOS; build the probe, render, run the pxui_text test)
set -e
cd "$(dirname "$0")/.."
dir=${1:-target/glyph_ref}; mkdir -p "$dir"
clang -O1 -fobjc-arc -framework CoreText -framework CoreGraphics -framework Foundation tools/glyph_ref.m -o "$dir/coretext"
L=${LOCALWAVE_FONTS:-$HOME/WORK/GIT/localwave/localwave/Fonts}
for spec in "hg:$L/HostGrotesk.ttf" "gm:$L/GeistMono.ttf" "ps:assets/fonts/Pragmasevka-Regular.ttf"; do
  for sz in 11 13 15 17 20 28; do for sc in 1 2; do for pol in black white; do
    "$dir/coretext" "${spec#*:}" $sz $sc $pol "Hamburgefonstiv Quick 0123456789" "$dir/${spec%%:*}-$sz-${sc}x-$pol.pgm"
  done; done; done
done
echo "$L" > "$dir/fonts_dir"
PXUI_GLYPH_REF=$(cd "$dir" && pwd) CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-target} cargo t -p pxui_text glyph_ref
