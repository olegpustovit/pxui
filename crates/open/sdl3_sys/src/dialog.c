#include <stdatomic.h>
#include <stdlib.h>
#include <string.h>

#include "bridge.h"

/* The dialog table is per process by design: PXUI_DIALOG_SLOTS (8) open dialogs at most, each outcome
   capped at DIALOG_MAX_PATHS paths and DIALOG_MAX_BYTES bytes. Dialogs are reserved, shown and taken
   on the initial thread; the platform's completion callback may run on any thread and only fills its
   own slot, published through the slot's atomic state. Each slot records the window that opened it,
   so the outcome returns to that window whichever window has focus. */
enum { DIALOG_MAX_PATHS = 4096, DIALOG_MAX_BYTES = 1 << 20 };
enum { DIALOG_FREE = 0, DIALOG_OPEN, DIALOG_DONE };

typedef struct {
  atomic_int state;
  int id;
  SDL_WindowID window;
  SDL_DialogFileFilter *filters;
  int filter_count;
  char *strings;
  char *default_location;
  pxui_dialog_outcome_kind outcome;
  char *payload;
  size_t payload_length;
  void *urls;
} dialog_slot;

static dialog_slot dialog_slots[PXUI_DIALOG_SLOTS];
static int next_dialog_id = 1;

static void release_buffers(dialog_slot *slot)
{
  #if defined(PXUI_SDL3_IOS)
  if (slot->urls != NULL) pxui_dialog_release_urls(slot->urls);
  #endif
  slot->urls = NULL;
  free(slot->filters);
  free(slot->strings);
  free(slot->default_location);
  free(slot->payload);
  slot->filters = NULL;
  slot->strings = NULL;
  slot->default_location = NULL;
  slot->payload = NULL;
  slot->payload_length = 0;
  slot->filter_count = 0;
}

static void fail(dialog_slot *slot, const char *message)
{
  size_t length = strlen(message);
  slot->outcome = PXUI_OUTCOME_FAILED;
  slot->payload = (char *)malloc(length + 1);
  if (slot->payload != NULL) {
    memcpy(slot->payload, message, length + 1);
    slot->payload_length = length;
  }
}

void *pxui_dialog_reserve(const char *const *names, const char *const *patterns,
    int count, const char *default_location, int *id)
{
  dialog_slot *slot = NULL;
  size_t strings_length = 0;
  int index;
  for (index = 0; index < PXUI_DIALOG_SLOTS && slot == NULL; index++) {
    int expected = DIALOG_FREE;
    if (atomic_compare_exchange_strong(&dialog_slots[index].state, &expected, DIALOG_OPEN)) {
      slot = &dialog_slots[index];
    }
  }
  if (slot == NULL) {
    SDL_SetError("too many file dialogs are open");
    return NULL;
  }
  slot->urls = NULL;
  slot->filters = NULL;
  slot->filter_count = 0;
  slot->strings = NULL;
  slot->default_location = NULL;
  slot->payload = NULL;
  slot->payload_length = 0;
  slot->outcome = PXUI_OUTCOME_CANCELLED;
  slot->window = 0;
  for (index = 0; index < count; index++) {
    strings_length += strlen(names[index]) + 1 + strlen(patterns[index]) + 1;
  }
  if (count > 0) {
    char *write;
    slot->filters = (SDL_DialogFileFilter *)calloc((size_t)count, sizeof(SDL_DialogFileFilter));
    slot->strings = (char *)malloc(strings_length);
    if (slot->filters == NULL || slot->strings == NULL) goto out_of_memory;
    write = slot->strings;
    for (index = 0; index < count; index++) {
      size_t name_length = strlen(names[index]) + 1;
      size_t pattern_length = strlen(patterns[index]) + 1;
      memcpy(write, names[index], name_length);
      slot->filters[index].name = write;
      write += name_length;
      memcpy(write, patterns[index], pattern_length);
      slot->filters[index].pattern = write;
      write += pattern_length;
    }
    slot->filter_count = count;
  }
  if (default_location != NULL) {
    size_t length = strlen(default_location) + 1;
    slot->default_location = (char *)malloc(length);
    if (slot->default_location == NULL) goto out_of_memory;
    memcpy(slot->default_location, default_location, length);
  }
  slot->id = next_dialog_id;
  next_dialog_id = next_dialog_id == SDL_MAX_SINT32 ? 1 : next_dialog_id + 1;
  *id = slot->id;
  return slot;

out_of_memory:
  release_buffers(slot);
  atomic_store(&slot->state, DIALOG_FREE);
  SDL_SetError("out of memory copying the file dialog arguments");
  return NULL;
}

void pxui_dialog_abandon(void *raw)
{
  dialog_slot *slot = (dialog_slot *)raw;
  release_buffers(slot);
  atomic_store_explicit(&slot->state, DIALOG_FREE, memory_order_release);
}

void SDLCALL pxui_dialog_callback(void *raw, const char *const *filelist, int filter)
{
  dialog_slot *slot = (dialog_slot *)raw;
  size_t total = 0;
  int count = 0;
  (void)filter;
  if (filelist == NULL) {
    const char *message = SDL_GetError();
    fail(slot, message != NULL && message[0] != '\0' ? message : "the file dialog failed");
  } else if (filelist[0] == NULL) {
    slot->outcome = PXUI_OUTCOME_CANCELLED;
  } else {
    for (; filelist[count] != NULL; count++) {
      total += strlen(filelist[count]) + 1;
      if (count >= DIALOG_MAX_PATHS || total > DIALOG_MAX_BYTES) break;
    }
    if (filelist[count] != NULL) {
      fail(slot, "the selection has too many files");
    } else {
      char *cursor = (char *)malloc(total);
      slot->payload = cursor;
      if (cursor == NULL) {
        fail(slot, "out of memory copying the chosen paths");
      } else {
        int index;
        for (index = 0; index < count; index++) {
          size_t length = strlen(filelist[index]) + 1;
          memcpy(cursor, filelist[index], length);
          cursor += length;
        }
        slot->outcome = PXUI_OUTCOME_CHOSEN;
        slot->payload_length = total;
      }
    }
  }
  atomic_store_explicit(&slot->state, DIALOG_DONE, memory_order_release);
  (void)pxui_sdl3_wake();
}

void pxui_dialog_set_owner(void *slot, SDL_WindowID window) { ((dialog_slot *)slot)->window = window; }

int pxui_dialog_show(pxui_dialog_kind kind, SDL_Window *window,
    const char *const *names, const char *const *patterns, int count,
    const char *default_location)
{
  int id = 0;
  dialog_slot *slot = (dialog_slot *)pxui_dialog_reserve(names, patterns, count, default_location, &id);
  if (slot == NULL) return 0;
  pxui_dialog_set_owner(slot, SDL_GetWindowID(window));
#if defined(PXUI_SDL3_IOS)
  if (!pxui_dialog_ios_show(slot, window, kind, slot->filters, slot->filter_count, slot->default_location)) {
    pxui_dialog_abandon(slot);
    return 0;
  }
#else
  switch (kind) {
  case PXUI_DIALOG_OPEN_FILE:
  case PXUI_DIALOG_OPEN_FILES:
    SDL_ShowOpenFileDialog(pxui_dialog_callback, slot, window, slot->filters,
        slot->filter_count, slot->default_location, kind == PXUI_DIALOG_OPEN_FILES);
    break;
  case PXUI_DIALOG_SAVE_FILE:
    SDL_ShowSaveFileDialog(pxui_dialog_callback, slot, window, slot->filters,
        slot->filter_count, slot->default_location);
    break;
  case PXUI_DIALOG_OPEN_FOLDER:
    SDL_ShowOpenFolderDialog(pxui_dialog_callback, slot, window, slot->default_location, false);
    break;
  default:
    pxui_dialog_abandon(slot);
    SDL_SetError("invalid file dialog kind");
    return 0;
  }
#endif
  return id;
}

bool pxui_dialog_take(pxui_dialog_result *result)
{
  dialog_slot *found = NULL;
  int index;
  for (index = 0; index < PXUI_DIALOG_SLOTS; index++) {
    dialog_slot *slot = &dialog_slots[index];
    if (atomic_load_explicit(&slot->state, memory_order_acquire) == DIALOG_DONE
        && (found == NULL || slot->id < found->id)) found = slot;
  }
  if (found == NULL) return false;
  result->id = found->id;
  result->window = found->window;
  result->outcome = found->outcome;
  result->payload = found->payload;
  result->payload_length = found->payload_length;
  result->urls = found->urls;
  found->urls = NULL;
  found->payload = NULL;
  found->payload_length = 0;
  release_buffers(found);
  atomic_store_explicit(&found->state, DIALOG_FREE, memory_order_release);
  return true;
}

#if defined(PXUI_SDL3_IOS)
void pxui_dialog_set_urls(void *raw, void *urls) { ((dialog_slot *)raw)->urls = urls; }
void pxui_dialog_close(SDL_WindowID window) {
  pxui_dialog_ios_close(window);
  for (int index = 0; index < PXUI_DIALOG_SLOTS; index++) {
    dialog_slot *slot = &dialog_slots[index];
    if (atomic_load_explicit(&slot->state, memory_order_acquire) == DIALOG_DONE && slot->window == window)
      pxui_dialog_abandon(slot);
  }
}
#endif
