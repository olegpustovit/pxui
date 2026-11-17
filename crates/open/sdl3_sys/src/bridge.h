#ifndef PXUI_SDL3_SYS_BRIDGE_H
#define PXUI_SDL3_SYS_BRIDGE_H

#include <SDL3/SDL.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include <SDL3/SDL_metal.h>
#include "dialog.h"

typedef struct {
  int32_t tag;
  int32_t a, b, c, d, e;
  /* the SDL window the event belongs to; 0: none (quit, synthetic, a dialog without a parent) */
  uint32_t window;
  int32_t g, h;
  int32_t integer_x, integer_y;
  int32_t id;
  double x, y, z, w;
  char *text;
  size_t text_len;
  uint64_t touch_id, finger_id;
  void *dialog_urls;
} pxui_sdl3_event;

int32_t pxui_sdl3_linked_version(void);
int32_t pxui_sdl3_compiled_version(void);
const char *pxui_sdl3_get_error(void);
void pxui_sdl3_clear_error(void);
const char *pxui_sdl3_platform_name(void);
char *pxui_sdl3_pref_path(const char *org, const char *app);
void pxui_sdl3_free(void *value);
bool pxui_sdl3_is_main_thread(void);
void pxui_sdl3_configure_fatal_errors(void);
#if defined(PXUI_SDL3_IOS)
int pxui_sdl3_run_app(int (*main_function)(void));
#endif
bool pxui_sdl3_init_subsystem(uint32_t bits);
void pxui_sdl3_quit_subsystem(uint32_t bits);

SDL_Window *pxui_sdl3_create_window(const char *title, int width, int height, uint32_t flags);
void pxui_sdl3_destroy_window(SDL_Window *window);
bool pxui_sdl3_window_size(SDL_Window *window, int *width, int *height);
bool pxui_sdl3_window_size_in_pixels(SDL_Window *window, int *width, int *height);
uint32_t pxui_sdl3_window_state(SDL_Window *window);
uint32_t pxui_sdl3_window_id(SDL_Window *window);
float pxui_sdl3_window_pixel_density(SDL_Window *window);
float pxui_sdl3_window_display_scale(SDL_Window *window);
bool pxui_sdl3_window_position(SDL_Window *window, int *x, int *y);
const char *pxui_sdl3_window_title(SDL_Window *window);
bool pxui_sdl3_center_window(SDL_Window *window);
bool pxui_sdl3_set_window_size(SDL_Window *window, int width, int height);
bool pxui_sdl3_set_window_resizable(SDL_Window *window, bool enabled);
bool pxui_sdl3_set_window_relative_mouse(SDL_Window *window, bool enabled);
bool pxui_sdl3_show_window(SDL_Window *window);
bool pxui_sdl3_raise_window(SDL_Window *window);
bool pxui_sdl3_hide_window(SDL_Window *window);
bool pxui_sdl3_restore_window(SDL_Window *window);
bool pxui_sdl3_sync_window(SDL_Window *window);
void pxui_sdl3_set_window_background(SDL_Window *window, double red, double green, double blue);

SDL_Cursor *pxui_sdl3_create_cursor(int shape);
bool pxui_sdl3_set_cursor(SDL_Cursor *cursor);
void pxui_sdl3_destroy_cursor(SDL_Cursor *cursor);
bool pxui_sdl3_set_control_click_right_click(bool enabled);
bool pxui_sdl3_set_mouse_relative_system_scale(bool enabled);
bool pxui_sdl3_set_hint(const char *name, const char *value);
bool pxui_sdl3_clipboard_set_text(const char *text);
char *pxui_sdl3_clipboard_get_text(void);
bool pxui_sdl3_start_text_input(SDL_Window *window);
bool pxui_sdl3_start_text_input_with_options(SDL_Window *window, uint32_t autocorrect, uint32_t capitalization, uint32_t action);
bool pxui_sdl3_set_text_input_area(SDL_Window *window, int x, int y, int width, int height, int cursor);
bool pxui_sdl3_stop_text_input(SDL_Window *window);
bool pxui_sdl3_clear_text_input_area(SDL_Window *window);
void pxui_sdl3_scroll_monitor_install(void);
void pxui_sdl3_window_plain_titlebar(SDL_Window *window);
void pxui_sdl3_app_activate(void);
void pxui_sdl3_platform_set_window_background(SDL_Window *window, double red, double green, double blue);

SDL_MetalView pxui_sdl3_create_metal_view(SDL_Window *window);
void pxui_sdl3_destroy_metal_view(SDL_MetalView view);
void *pxui_sdl3_metal_layer(SDL_MetalView view);
void pxui_sdl3_retain_layer(void *layer);
void pxui_sdl3_release_layer(void *layer);

int pxui_sdl3_show_dialog(SDL_Window *window, int kind,
    const char *const *names, const char *const *patterns, int count,
    const char *default_location);
bool pxui_sdl3_take_dialog(pxui_dialog_result *result);
/* One process callback (SDL's queue is per process), run on the initial thread inside the event pump
   with the SDL window id of the window being resized; the caller dispatches by that id. */
typedef void (*pxui_sdl3_live_resize_callback)(uint32_t window);
void pxui_sdl3_set_live_resize_callback(pxui_sdl3_live_resize_callback callback);

bool pxui_sdl3_test_polling_event_queue(void);
void pxui_sdl3_test_queue_resize_during_poll(void);

bool pxui_sdl3_poll_event(pxui_sdl3_event *event);
bool pxui_sdl3_wait_event_timeout(int timeout_ms);
bool pxui_sdl3_wake(void);
void pxui_sdl3_event_free(pxui_sdl3_event *event);
bool pxui_sdl3_scroll_take(double *x, double *y, int *phase, double *seconds);

#endif
