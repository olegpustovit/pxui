#!/bin/sh
# Executables get a 256 MB main-thread stack (ld rejects -stack_size for dylibs, proc macros) and an rpath to the directory
# above them (target/<profile>), where the dynamic `libpxui.1.dylib` stands for the tests that link it.
case " $* " in
*" -dynamiclib "*) exec cc "$@" ;;
*) exec cc "$@" -Wl,-stack_size,0x10000000 -Wl,-rpath,@executable_path/.. ;;
esac
