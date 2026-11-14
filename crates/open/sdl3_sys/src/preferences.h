#pragma once
#include <stdint.h>
typedef struct { uint32_t appearance, reduced_motion; } PXUIPreferencesState;
void *rays_sdl3_preferences_create(uint32_t window, void (*wake)(void));
void rays_sdl3_preferences_read(void *owner, PXUIPreferencesState *state);
void rays_sdl3_preferences_release(void *owner);
