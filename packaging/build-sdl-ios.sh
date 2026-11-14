#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
sdk=${1:-iphonesimulator}
case "$sdk" in
    iphoneos) platform=2 ;;
    iphonesimulator) platform=7 ;;
    *) echo "usage: $0 [iphoneos|iphonesimulator] [build-root]" >&2; exit 2 ;;
esac
build_root=${2:-"$PWD/target"}
mkdir -p "$build_root/downloads"
build_root=$(cd "$build_root" && pwd)
version=$(sed -n 's/.*((sdl3 .*[(]floor \([^)]*\)[)].*/\1/p' packaging/sdl3.lock)
sha=$(sed -n 's/.*((sdl3 .*[(]sha256 \([^)]*\)[)].*/\1/p' packaging/sdl3.lock)
test -n "$version"
test -n "$sha"
archive="$build_root/downloads/SDL3-$version.tar.gz"
if [ ! -f "$archive" ]; then
    curl --fail --location --retry 2 "https://www.libsdl.org/release/SDL3-$version.tar.gz" -o "$archive.part"
    mv "$archive.part" "$archive"
fi
echo "$sha  $archive" | shasum -a 256 --check
if [ ! -d "$build_root/SDL3-$version" ]; then tar -xzf "$archive" -C "$build_root"; fi
prefix="$build_root/sdl-$sdk"
build="$build_root/sdl-build-$sdk"
cmake -S "$build_root/SDL3-$version" -B "$build" \
    -DCMAKE_SYSTEM_NAME=iOS -DCMAKE_OSX_SYSROOT="$sdk" \
    -DCMAKE_OSX_ARCHITECTURES=arm64 -DCMAKE_OSX_DEPLOYMENT_TARGET=18.0 \
    -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX="$prefix" -DCMAKE_INSTALL_LIBDIR=lib \
    -DSDL_SHARED=OFF -DSDL_STATIC=ON -DSDL_FRAMEWORK=OFF \
    -DSDL_OPENGL=OFF -DSDL_OPENGLES=OFF -DSDL_VULKAN=OFF \
    -DSDL_TESTS=OFF -DSDL_TEST_LIBRARY=OFF -DSDL_EXAMPLES=OFF
cmake --build "$build" --parallel "$(sysctl -n hw.ncpu)"
cmake --install "$build"
xcrun lipo -verify_arch arm64 "$prefix/lib/libSDL3.a"
xcrun otool -l "$prefix/lib/libSDL3.a" | awk -v expected="$platform" '
    $1 == "platform" { count++; if ($2 != expected) bad = 1 }
    $1 == "minos" { if ($2 != "18.0") bad = 1 }
    END { if (!count || bad) { print "SDL archive has the wrong Apple platform or deployment floor" > "/dev/stderr"; exit 1 } }
'
echo "SDL3 for $sdk installed at $prefix"
