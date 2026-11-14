#pragma once
#include <stdint.h>
typedef struct {
    double top_inset;
    double content_width, content_height;
    uint32_t resizable, zoom_enabled, full_size_content, full_screen_primary;
} RaysSDL3ChromeState;
int rays_sdl3_chrome_configure(void *layer, double minimum_width, double minimum_height);
int rays_sdl3_chrome_read(void *layer, RaysSDL3ChromeState *state);
