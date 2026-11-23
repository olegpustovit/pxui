#pragma once
#include <stdint.h>
typedef struct { uint32_t appearance, reduced_motion, overlay_scrollers; } PXUIPreferencesState;
void *pxui_sdl3_preferences_create(uint32_t window, void (*wake)(void));
void pxui_sdl3_preferences_read(void *owner, PXUIPreferencesState *state);
void pxui_sdl3_preferences_release(void *owner);
