#pragma once
#include "bridge.h"

bool pxui_sdl3_ios_text_prepare(SDL_Window *window, uint32_t action);
bool pxui_sdl3_ios_text_status(SDL_Window *window);
void pxui_sdl3_ios_text_close(SDL_Window *window);
/* 0: unrelated, 1: owned payload transferred, -1: expired event. */
int pxui_sdl3_ios_text_translate(const SDL_Event *source, pxui_sdl3_event *out);
