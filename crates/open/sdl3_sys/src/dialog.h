#ifndef PXUI_SDL3_SYS_DIALOG_H
#define PXUI_SDL3_SYS_DIALOG_H

#include <SDL3/SDL.h>
#include <stdbool.h>
#include <stddef.h>

enum { PXUI_DIALOG_SLOTS = 8 };

typedef enum {
  PXUI_DIALOG_OPEN_FILE,
  PXUI_DIALOG_OPEN_FILES,
  PXUI_DIALOG_SAVE_FILE,
  PXUI_DIALOG_OPEN_FOLDER
} pxui_dialog_kind;

typedef enum {
  PXUI_OUTCOME_CHOSEN,
  PXUI_OUTCOME_CANCELLED,
  PXUI_OUTCOME_FAILED
} pxui_dialog_outcome_kind;

typedef struct {
  int id;
  SDL_WindowID window;
  pxui_dialog_outcome_kind outcome;
  char *payload;
  size_t payload_length;
  void *urls;
} pxui_dialog_result;

int pxui_dialog_show(pxui_dialog_kind kind, SDL_Window *window,
    const char *const *names, const char *const *patterns, int count,
    const char *default_location);
bool pxui_dialog_take(pxui_dialog_result *result);

void *pxui_dialog_reserve(const char *const *names, const char *const *patterns,
    int count, const char *default_location, int *id);
void pxui_dialog_abandon(void *slot);
/* The window whose pump receives the slot's outcome (0: none, routed like any window-less event). */
void pxui_dialog_set_owner(void *slot, SDL_WindowID window);
#if defined(PXUI_SDL3_IOS)
void pxui_dialog_set_urls(void *slot, void *urls);
void pxui_dialog_release_urls(void *urls);
bool pxui_dialog_ios_show(void *slot, SDL_Window *window, pxui_dialog_kind kind,
    const SDL_DialogFileFilter *filters, int count, const char *location);
void pxui_dialog_ios_close(SDL_WindowID window);
void pxui_dialog_close(SDL_WindowID window);
#endif
void SDLCALL pxui_dialog_callback(void *slot, const char *const *filelist, int filter);
#endif
