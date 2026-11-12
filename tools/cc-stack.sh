#!/bin/sh
# Executables get a 256 MB main-thread stack (ld rejects -stack_size for dylibs, proc macros) and rpaths to their own directory and
# the one above (target/<profile>), where the dynamic `libpxui.1.dylib` stands for the binaries (target/<profile>/<bin>) and the tests
# (target/<profile>/deps/<test>) that link it.
case " $* " in
*" -dynamiclib "*) exec cc "$@" ;;
esac
# Cargo cannot order the libpxui cdylib before a dependent's link: when it is missing, say what builds it.
out=$(cc "$@" -Wl,-stack_size,0x10000000 -Wl,-rpath,@executable_path -Wl,-rpath,@executable_path/.. 2>&1) && status=0 || status=$?
[ -z "$out" ] || echo "$out" >&2
[ "$status" = 0 ] || case $out in
*"library 'pxui' not found"*) echo "libpxui is not built for this profile: run 'make lib' (Rays) or 'cargo build -p pxui_dylib' (PXUI; --release for release), or use 'cargo t'" >&2 ;;
esac
exit $status
