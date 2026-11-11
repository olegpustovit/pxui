#include <pxui.h>
#include <stdio.h>

int main(void) {
    uint32_t version = pxui_abi_version();
    if (version == 0) return 1;
    printf("PXUI ABI %u.%u\n", version >> 16, version & 0xffff);
    return 0;
}
