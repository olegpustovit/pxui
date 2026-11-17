#pragma once
#include <stdint.h>
typedef struct {
    double top_inset;
    double content_width, content_height;
    uint32_t resizable, zoom_enabled, full_size_content, full_screen_primary;
} PxuiSDL3ChromeState;
int pxui_sdl3_chrome_configure(void *layer, double minimum_width, double minimum_height);
int pxui_sdl3_chrome_read(void *layer, PxuiSDL3ChromeState *state);
int pxui_sdl3_chrome_update_background(void *window, void *color);
