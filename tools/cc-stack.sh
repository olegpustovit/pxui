#!/bin/sh
# Executables keep the legacy 256 MB stack and local library rpaths. Consumers may opt into
# plain cc with PXUI_CONSUMER_DEFAULT_STACK=1 while sharing the same Cargo linker fingerprint.
for arg in "$@"; do
    [ "$arg" != -dynamiclib ] || exec cc "$@"
done
if [ "${PXUI_CONSUMER_DEFAULT_STACK:-}" = 1 ]; then
    pxui_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
    case "${CARGO_MANIFEST_DIR:-}" in
    "$pxui_root"/crates/*|"$pxui_root"/tools/*) ;;
    *) exec cc "$@" ;;
    esac
fi
# Cargo cannot order the libpxui cdylib before a dependent's link: when it is missing, say what builds it.
out=$(cc "$@" -Wl,-stack_size,0x10000000 -Wl,-rpath,@executable_path -Wl,-rpath,@executable_path/.. 2>&1) && status=0 || status=$?
[ -z "$out" ] || echo "$out" >&2
[ "$status" = 0 ] || case $out in
*"library 'pxui' not found"*) echo "libpxui is not built for this profile: run 'make lib' (Rays) or 'cargo build -p pxui_dylib' (PXUI; --release for release), or use 'cargo t'" >&2 ;;
esac
exit $status
