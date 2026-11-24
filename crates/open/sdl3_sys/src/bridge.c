#include "bridge.h"
#include "sdl3_abi.h"
#ifdef PXUI_SDL3_IOS
#include "ios.h"
#include "text_ios.h"
#endif

#include <limits.h>
#include <stdlib.h>
#include <string.h>
#include <stdatomic.h>

enum {
  INIT_VIDEO = 1, INIT_EVENTS = 2,
  WINDOW_HIDDEN = 1, WINDOW_HIGH_PIXEL_DENSITY = 2, WINDOW_METAL = 4
};

enum {
  EVENT_QUIT, EVENT_WINDOW, EVENT_KEY, EVENT_TEXT_INPUT, EVENT_TEXT_EDITING,
  EVENT_MOUSE_MOTION, EVENT_MOUSE_BUTTON, EVENT_MOUSE_WHEEL, EVENT_PINCH,
  EVENT_DROP, EVENT_DIALOG, EVENT_SCROLL, EVENT_TOUCH
};

enum {
  WINDOW_SHOWN, WINDOW_HIDDEN_CHANGE, WINDOW_MINIMIZED, WINDOW_RESTORED,
  WINDOW_OCCLUDED, WINDOW_FOCUS_GAINED, WINDOW_FOCUS_LOST, WINDOW_CLOSE_REQUESTED,
  WINDOW_RESIZED, WINDOW_PIXEL_SIZE_CHANGED
};

enum {
  KEY_CHAR, KEY_UNKNOWN, KEY_ARROW_UP, KEY_ARROW_DOWN, KEY_ARROW_LEFT, KEY_ARROW_RIGHT,
  KEY_SPACE, KEY_ENTER, KEY_ESCAPE, KEY_BACKSPACE, KEY_TAB, KEY_SHIFT, KEY_CONTROL,
  KEY_ALT, KEY_META, KEY_F1, KEY_F2, KEY_F3, KEY_F4, KEY_F5, KEY_F6, KEY_F7,
  KEY_F8, KEY_F9, KEY_F10, KEY_F11, KEY_F12, KEY_HOME, KEY_END, KEY_PAGE_UP,
  KEY_PAGE_DOWN, KEY_INSERT, KEY_DELETE
};

enum { BUTTON_LEFT, BUTTON_MIDDLE, BUTTON_RIGHT, BUTTON_X1, BUTTON_X2 };
enum { MOD_SHIFT = 1, MOD_CONTROL = 2, MOD_ALT = 4, MOD_META = 8,
       MOD_NUM_LOCK = 16, MOD_CAPS_LOCK = 32, MOD_SCROLL_LOCK = 64 };

static char *copy_text(const char *source, size_t *length)
{
  size_t size;
  char *copy;
  if (source == NULL) source = "";
  size = strlen(source);
  copy = (char *)malloc(size + 1);
  if (copy == NULL) return NULL;
  memcpy(copy, source, size + 1);
  *length = size;
  return copy;
}

int32_t pxui_sdl3_linked_version(void) { return SDL_GetVersion(); }
int32_t pxui_sdl3_compiled_version(void) { return SDL_VERSION; }
const char *pxui_sdl3_get_error(void) { return SDL_GetError(); }
void pxui_sdl3_clear_error(void) { SDL_ClearError(); }
const char *pxui_sdl3_platform_name(void) { return SDL_GetPlatform(); }
char *pxui_sdl3_pref_path(const char *org, const char *app) { return SDL_GetPrefPath(org, app); }
void pxui_sdl3_free(void *value) { SDL_free(value); }

bool pxui_sdl3_init_subsystem(uint32_t bits)
{
  SDL_InitFlags flags = 0;
  if ((bits & INIT_VIDEO) != 0) flags |= SDL_INIT_VIDEO;
  if ((bits & INIT_EVENTS) != 0) flags |= SDL_INIT_EVENTS;
  return SDL_InitSubSystem(flags);
}

void pxui_sdl3_quit_subsystem(uint32_t bits)
{
  SDL_InitFlags flags = 0;
  if ((bits & INIT_VIDEO) != 0) flags |= SDL_INIT_VIDEO;
  if ((bits & INIT_EVENTS) != 0) flags |= SDL_INIT_EVENTS;
  SDL_QuitSubSystem(flags);
}

SDL_Window *pxui_sdl3_create_window(const char *title, int width, int height, uint32_t bits)
{
  SDL_WindowFlags flags = 0;
  if ((bits & WINDOW_HIDDEN) != 0) flags |= SDL_WINDOW_HIDDEN;
  if ((bits & WINDOW_HIGH_PIXEL_DENSITY) != 0) flags |= SDL_WINDOW_HIGH_PIXEL_DENSITY;
#ifndef _WIN32
  if ((bits & WINDOW_METAL) != 0) flags |= SDL_WINDOW_METAL;
#endif
  SDL_Window *window = SDL_CreateWindow(title, width, height, flags);
  pxui_sdl3_window_plain_titlebar(window);
  pxui_sdl3_scroll_monitor_install();
  return window;
}

void pxui_sdl3_destroy_window(SDL_Window *window) {
#if defined(PXUI_SDL3_IOS)
  pxui_dialog_close(SDL_GetWindowID(window));
  pxui_sdl3_ios_text_close(window);
#endif
  SDL_DestroyWindow(window);
}

bool pxui_sdl3_window_size(SDL_Window *window, int *width, int *height)
{
  return SDL_GetWindowSize(window, width, height);
}

bool pxui_sdl3_window_size_in_pixels(SDL_Window *window, int *width, int *height)
{
  return SDL_GetWindowSizeInPixels(window, width, height);
}

uint32_t pxui_sdl3_window_state(SDL_Window *window)
{
  SDL_WindowFlags flags = SDL_GetWindowFlags(window);
  return ((flags & SDL_WINDOW_HIDDEN) != 0 ? 1u : 0u)
      | ((flags & SDL_WINDOW_MINIMIZED) != 0 ? 2u : 0u)
      | ((flags & SDL_WINDOW_OCCLUDED) != 0 ? 4u : 0u);
}

uint32_t pxui_sdl3_window_id(SDL_Window *window) { return SDL_GetWindowID(window); }
float pxui_sdl3_window_pixel_density(SDL_Window *window) { return SDL_GetWindowPixelDensity(window); }
float pxui_sdl3_window_display_scale(SDL_Window *window) { return SDL_GetWindowDisplayScale(window); }
bool pxui_sdl3_window_position(SDL_Window *window, int *x, int *y)
{
  return SDL_GetWindowPosition(window, x, y);
}
const char *pxui_sdl3_window_title(SDL_Window *window) { return SDL_GetWindowTitle(window); }
bool pxui_sdl3_center_window(SDL_Window *window)
{
  return SDL_SetWindowPosition(window, SDL_WINDOWPOS_CENTERED, SDL_WINDOWPOS_CENTERED);
}
bool pxui_sdl3_set_window_size(SDL_Window *window, int width, int height)
{
  return SDL_SetWindowSize(window, width, height);
}
bool pxui_sdl3_set_window_resizable(SDL_Window *window, bool enabled)
{
  return SDL_SetWindowResizable(window, enabled);
}
bool pxui_sdl3_set_window_relative_mouse(SDL_Window *window, bool enabled)
{
  return SDL_SetWindowRelativeMouseMode(window, enabled);
}
bool pxui_sdl3_show_window(SDL_Window *window) { return SDL_ShowWindow(window); }
bool pxui_sdl3_raise_window(SDL_Window *window)
{
  pxui_sdl3_app_activate();
  return SDL_RaiseWindow(window);
}
bool pxui_sdl3_hide_window(SDL_Window *window) { return SDL_HideWindow(window); }
bool pxui_sdl3_restore_window(SDL_Window *window) { return SDL_RestoreWindow(window); }
bool pxui_sdl3_sync_window(SDL_Window *window) { return SDL_SyncWindow(window); }
void pxui_sdl3_set_window_background(SDL_Window *window, double red, double green, double blue)
{
  pxui_sdl3_platform_set_window_background(window, red, green, blue);
}

SDL_Cursor *pxui_sdl3_create_cursor(int shape)
{
  static const SDL_SystemCursor shapes[] = {
    SDL_SYSTEM_CURSOR_DEFAULT, SDL_SYSTEM_CURSOR_TEXT,
    SDL_SYSTEM_CURSOR_EW_RESIZE, SDL_SYSTEM_CURSOR_NS_RESIZE
  };
  if (shape < 0 || (size_t)shape >= sizeof(shapes) / sizeof(shapes[0])) {
    SDL_SetError("invalid system cursor shape");
    return NULL;
  }
  return SDL_CreateSystemCursor(shapes[shape]);
}
bool pxui_sdl3_set_cursor(SDL_Cursor *cursor) { return SDL_SetCursor(cursor); }
void pxui_sdl3_destroy_cursor(SDL_Cursor *cursor) { SDL_DestroyCursor(cursor); }
bool pxui_sdl3_set_control_click_right_click(bool enabled)
{
  return SDL_SetHint(SDL_HINT_MAC_CTRL_CLICK_EMULATE_RIGHT_CLICK, enabled ? "1" : "0");
}
bool pxui_sdl3_set_mouse_relative_system_scale(bool enabled)
{
  return SDL_SetHint(SDL_HINT_MOUSE_RELATIVE_SYSTEM_SCALE, enabled ? "1" : "0");
}
bool pxui_sdl3_set_hint(const char *name, const char *value) { return SDL_SetHint(name, value); }
bool pxui_sdl3_clipboard_set_text(const char *text) { return SDL_SetClipboardText(text); }
char *pxui_sdl3_clipboard_get_text(void) { return SDL_GetClipboardText(); }
bool pxui_sdl3_start_text_input(SDL_Window *window) {
#ifdef PXUI_SDL3_IOS
  return pxui_sdl3_start_text_input_with_options(window, 1, 1, 0);
#else
  return SDL_StartTextInput(window);
#endif
}
bool pxui_sdl3_start_text_input_with_options(SDL_Window *window, uint32_t autocorrect, uint32_t capitalization, uint32_t action) {
  if (autocorrect > 1 || capitalization > 3 || action > 2) return SDL_SetError("invalid text input options");
#ifdef PXUI_SDL3_IOS
  if (!pxui_sdl3_ios_text_prepare(window, action)) return false;
#endif
  SDL_PropertiesID props = SDL_CreateProperties();
  if (!props) return false;
  bool ready = SDL_SetBooleanProperty(props, SDL_PROP_TEXTINPUT_AUTOCORRECT_BOOLEAN, autocorrect != 0)
      && SDL_SetNumberProperty(props, SDL_PROP_TEXTINPUT_CAPITALIZATION_NUMBER, capitalization)
      && SDL_SetBooleanProperty(props, SDL_PROP_TEXTINPUT_MULTILINE_BOOLEAN, false);
  bool started = ready && SDL_StartTextInputWithProperties(window, props);
  SDL_DestroyProperties(props);
#ifdef PXUI_SDL3_IOS
  if (started) started = pxui_sdl3_ios_text_prepare(window, action);
#endif
  return started;
}
bool pxui_sdl3_stop_text_input(SDL_Window *window) { return SDL_StopTextInput(window); }
bool pxui_sdl3_clear_text_input_area(SDL_Window *window) { return SDL_SetTextInputArea(window, NULL, 0); }
bool pxui_sdl3_set_text_input_area(SDL_Window *window,
    int x, int y, int width, int height, int cursor)
{
  SDL_Rect area = { x, y, width, height };
  return SDL_SetTextInputArea(window, &area, cursor);
}

int pxui_sdl3_show_dialog(SDL_Window *window, int kind,
    const char *const *names, const char *const *patterns, int count,
    const char *default_location)
{
  return pxui_dialog_show((pxui_dialog_kind)kind, window, names, patterns, count, default_location);
}
bool pxui_sdl3_take_dialog(pxui_dialog_result *result) { return pxui_dialog_take(result); }

static int key_code(SDL_Keycode keycode, SDL_Scancode scancode, int *value)
{
  if (keycode >= 33 && keycode <= 126) {
    *value = keycode >= 'A' && keycode <= 'Z' ? keycode + ('a' - 'A') : keycode;
    return KEY_CHAR;
  }
  switch (keycode) {
  case SDLK_UP: return KEY_ARROW_UP;
  case SDLK_DOWN: return KEY_ARROW_DOWN;
  case SDLK_LEFT: return KEY_ARROW_LEFT;
  case SDLK_RIGHT: return KEY_ARROW_RIGHT;
  case SDLK_SPACE: return KEY_SPACE;
  case SDLK_RETURN: case SDLK_KP_ENTER: return KEY_ENTER;
  case SDLK_ESCAPE: return KEY_ESCAPE;
  case SDLK_BACKSPACE: return KEY_BACKSPACE;
  case SDLK_TAB: return KEY_TAB;
  case SDLK_LSHIFT: case SDLK_RSHIFT: return KEY_SHIFT;
  case SDLK_LCTRL: case SDLK_RCTRL: return KEY_CONTROL;
  case SDLK_LALT: case SDLK_RALT: return KEY_ALT;
  case SDLK_LGUI: case SDLK_RGUI: return KEY_META;
  case SDLK_F1: return KEY_F1; case SDLK_F2: return KEY_F2; case SDLK_F3: return KEY_F3;
  case SDLK_F4: return KEY_F4; case SDLK_F5: return KEY_F5; case SDLK_F6: return KEY_F6;
  case SDLK_F7: return KEY_F7; case SDLK_F8: return KEY_F8; case SDLK_F9: return KEY_F9;
  case SDLK_F10: return KEY_F10; case SDLK_F11: return KEY_F11; case SDLK_F12: return KEY_F12;
  case SDLK_HOME: return KEY_HOME; case SDLK_END: return KEY_END;
  case SDLK_PAGEUP: return KEY_PAGE_UP; case SDLK_PAGEDOWN: return KEY_PAGE_DOWN;
  case SDLK_INSERT: return KEY_INSERT; case SDLK_DELETE: return KEY_DELETE;
  default: break;
  }
  if (scancode >= SDL_SCANCODE_A && scancode <= SDL_SCANCODE_Z) {
    *value = 'a' + (scancode - SDL_SCANCODE_A);
    return KEY_CHAR;
  }
  if (scancode >= SDL_SCANCODE_1 && scancode <= SDL_SCANCODE_9) {
    *value = '1' + (scancode - SDL_SCANCODE_1);
    return KEY_CHAR;
  }
  if (scancode == SDL_SCANCODE_0) {
    *value = '0';
    return KEY_CHAR;
  }
  *value = keycode;
  return KEY_UNKNOWN;
}

static void window_event(const SDL_Event *source, pxui_sdl3_event *event)
{
  switch (source->type) {
  case SDL_EVENT_WINDOW_SHOWN: event->a = WINDOW_SHOWN; break;
  case SDL_EVENT_WINDOW_HIDDEN: event->a = WINDOW_HIDDEN_CHANGE; break;
  case SDL_EVENT_WINDOW_MINIMIZED: event->a = WINDOW_MINIMIZED; break;
  case SDL_EVENT_WINDOW_RESTORED: event->a = WINDOW_RESTORED; break;
  case SDL_EVENT_WINDOW_OCCLUDED: event->a = WINDOW_OCCLUDED; break;
  case SDL_EVENT_WINDOW_FOCUS_GAINED: event->a = WINDOW_FOCUS_GAINED; break;
  case SDL_EVENT_WINDOW_FOCUS_LOST: event->a = WINDOW_FOCUS_LOST; break;
  case SDL_EVENT_WINDOW_CLOSE_REQUESTED: event->a = WINDOW_CLOSE_REQUESTED; break;
  case SDL_EVENT_WINDOW_RESIZED:
    event->a = WINDOW_RESIZED;
    event->b = source->window.data1;
    event->c = source->window.data2;
    break;
  case SDL_EVENT_WINDOW_PIXEL_SIZE_CHANGED:
    event->a = WINDOW_PIXEL_SIZE_CHANGED;
    event->b = source->window.data1;
    event->c = source->window.data2;
    break;
  case SDL_EVENT_WINDOW_DISPLAY_SCALE_CHANGED: {
    int width = 0, height = 0;
    SDL_GetWindowSizeInPixels(SDL_GetWindowFromEvent(source), &width, &height);
    event->a = WINDOW_PIXEL_SIZE_CHANGED;
    event->b = width;
    event->c = height;
    break;
  }
  default: break;
  }
}

/* The window an SDL event names, read from the event itself so an event for a window closed since
   still names it (SDL_GetWindowFromEvent answers NULL then); 0 for events that name none. */
static bool translate_known(const SDL_Event *source, pxui_sdl3_event *event);

static SDL_WindowID event_window(const SDL_Event *source)
{
  if (source->type >= SDL_EVENT_WINDOW_FIRST && source->type <= SDL_EVENT_WINDOW_LAST) return source->window.windowID;
  switch (source->type) {
  case SDL_EVENT_KEY_DOWN: case SDL_EVENT_KEY_UP: return source->key.windowID;
  case SDL_EVENT_TEXT_INPUT: return source->text.windowID;
  case SDL_EVENT_TEXT_EDITING: return source->edit.windowID;
  case SDL_EVENT_MOUSE_MOTION: return source->motion.windowID;
  case SDL_EVENT_MOUSE_BUTTON_DOWN: case SDL_EVENT_MOUSE_BUTTON_UP: return source->button.windowID;
  case SDL_EVENT_MOUSE_WHEEL: return source->wheel.windowID;
  case SDL_EVENT_FINGER_DOWN: case SDL_EVENT_FINGER_MOTION:
  case SDL_EVENT_FINGER_UP: case SDL_EVENT_FINGER_CANCELED: return source->tfinger.windowID;
  case SDL_EVENT_PINCH_BEGIN: case SDL_EVENT_PINCH_UPDATE: case SDL_EVENT_PINCH_END: return source->pinch.windowID;
  case SDL_EVENT_DROP_BEGIN: case SDL_EVENT_DROP_POSITION:
  case SDL_EVENT_DROP_COMPLETE: case SDL_EVENT_DROP_FILE: return source->drop.windowID;
  default: return 0;
  }
}

static bool translate_event(const SDL_Event *source, pxui_sdl3_event *event)
{
  bool translated = translate_known(source, event);
  if (translated) event->window = event_window(source);
  return translated;
}

static bool translate_known(const SDL_Event *source, pxui_sdl3_event *event)
{
  int value = 0;
  memset(event, 0, sizeof(*event));
#ifdef PXUI_SDL3_IOS
  int marked = pxui_sdl3_ios_text_translate(source, event);
  if (marked) { event->tag = EVENT_TEXT_EDITING; return marked > 0; }
#endif
  switch (source->type) {
  case SDL_EVENT_QUIT:
    event->tag = EVENT_QUIT;
    return true;
  case SDL_EVENT_WINDOW_SHOWN: case SDL_EVENT_WINDOW_HIDDEN:
  case SDL_EVENT_WINDOW_MINIMIZED: case SDL_EVENT_WINDOW_RESTORED:
  case SDL_EVENT_WINDOW_OCCLUDED: case SDL_EVENT_WINDOW_FOCUS_GAINED:
  case SDL_EVENT_WINDOW_FOCUS_LOST: case SDL_EVENT_WINDOW_CLOSE_REQUESTED:
  case SDL_EVENT_WINDOW_DISPLAY_SCALE_CHANGED: case SDL_EVENT_WINDOW_RESIZED:
  case SDL_EVENT_WINDOW_PIXEL_SIZE_CHANGED:
    event->tag = EVENT_WINDOW;
    window_event(source, event);
    return true;
  case SDL_EVENT_FINGER_DOWN: case SDL_EVENT_FINGER_MOTION:
  case SDL_EVENT_FINGER_UP: case SDL_EVENT_FINGER_CANCELED: {
    SDL_Window *window=SDL_GetWindowFromID(source->tfinger.windowID);
    int width=0,height=0;
    if (!window || !SDL_GetWindowSize(window,&width,&height) || width<=0 || height<=0) return false;
    event->tag=EVENT_TOUCH;
    event->a=source->type==SDL_EVENT_FINGER_DOWN?0:source->type==SDL_EVENT_FINGER_MOTION?1:source->type==SDL_EVENT_FINGER_UP?2:3;
    event->touch_id=source->tfinger.touchID; event->finger_id=source->tfinger.fingerID;
    event->x=(double)source->tfinger.x*width; event->y=(double)source->tfinger.y*height;
    event->z=(double)source->tfinger.timestamp/1000000000.0; event->w=source->tfinger.pressure;
    return true;
  }
  case SDL_EVENT_KEY_DOWN: case SDL_EVENT_KEY_UP:
    event->tag = EVENT_KEY;
    event->a = key_code(source->key.key, source->key.scancode, &value);
    event->b = value;
    if ((source->key.mod & SDL_KMOD_SHIFT) != 0) event->c |= MOD_SHIFT;
    if ((source->key.mod & SDL_KMOD_CTRL) != 0) event->c |= MOD_CONTROL;
    if ((source->key.mod & SDL_KMOD_ALT) != 0) event->c |= MOD_ALT;
    if ((source->key.mod & SDL_KMOD_GUI) != 0) event->c |= MOD_META;
    if ((source->key.mod & SDL_KMOD_NUM) != 0) event->c |= MOD_NUM_LOCK;
    if ((source->key.mod & SDL_KMOD_CAPS) != 0) event->c |= MOD_CAPS_LOCK;
    if ((source->key.mod & SDL_KMOD_SCROLL) != 0) event->c |= MOD_SCROLL_LOCK;
    event->d = source->key.down;
    event->e = source->key.repeat;
    return true;
  case SDL_EVENT_TEXT_INPUT:
    event->tag = EVENT_TEXT_INPUT;
    event->text = copy_text(source->text.text, &event->text_len);
    return event->text != NULL;
  case SDL_EVENT_TEXT_EDITING:
    event->tag = EVENT_TEXT_EDITING;
    event->a = source->edit.start;
    event->b = source->edit.length;
    event->text = copy_text(source->edit.text, &event->text_len);
    return event->text != NULL;
  case SDL_EVENT_MOUSE_MOTION:
    event->tag = EVENT_MOUSE_MOTION;
    event->x = source->motion.x;
    event->y = source->motion.y;
    event->z = source->motion.xrel;
    event->w = source->motion.yrel;
    return true;
  case SDL_EVENT_MOUSE_BUTTON_DOWN: case SDL_EVENT_MOUSE_BUTTON_UP:
    switch (source->button.button) {
    case SDL_BUTTON_LEFT: event->a = BUTTON_LEFT; break;
    case SDL_BUTTON_MIDDLE: event->a = BUTTON_MIDDLE; break;
    case SDL_BUTTON_RIGHT: event->a = BUTTON_RIGHT; break;
    case SDL_BUTTON_X1: event->a = BUTTON_X1; break;
    case SDL_BUTTON_X2: event->a = BUTTON_X2; break;
    default: return false;
    }
    event->tag = EVENT_MOUSE_BUTTON;
    event->b = source->button.down;
    event->x = source->button.x;
    event->y = source->button.y;
    return true;
  case SDL_EVENT_MOUSE_WHEEL:
    event->tag = EVENT_MOUSE_WHEEL;
    event->a = source->wheel.direction == SDL_MOUSEWHEEL_FLIPPED ? 1 : 0;
    event->x = source->wheel.x;
    event->y = source->wheel.y;
    event->z = source->wheel.mouse_x;
    event->w = source->wheel.mouse_y;
    event->integer_x = source->wheel.integer_x;
    event->integer_y = source->wheel.integer_y;
    return true;
  case SDL_EVENT_PINCH_BEGIN: case SDL_EVENT_PINCH_UPDATE: case SDL_EVENT_PINCH_END:
    event->tag = EVENT_PINCH;
    event->a = source->type == SDL_EVENT_PINCH_BEGIN ? 0
        : source->type == SDL_EVENT_PINCH_UPDATE ? 1 : 2;
    event->x = source->pinch.scale;
    return true;
  case SDL_EVENT_DROP_BEGIN: case SDL_EVENT_DROP_POSITION:
  case SDL_EVENT_DROP_COMPLETE: case SDL_EVENT_DROP_FILE:
    event->tag = EVENT_DROP;
    event->a = source->type == SDL_EVENT_DROP_BEGIN ? 0
        : source->type == SDL_EVENT_DROP_POSITION ? 1
        : source->type == SDL_EVENT_DROP_COMPLETE ? 2 : 3;
    event->x = source->drop.x;
    event->y = source->drop.y;
    if (source->type == SDL_EVENT_DROP_FILE) {
      if (source->drop.data == NULL) return false;
      // SDL3 retains ownership of the event string; only our copied payload is ours to free.
      event->text = copy_text(source->drop.data, &event->text_len);
      return event->text != NULL;
    }
    return true;
  default:
    return false;
  }
}

void pxui_sdl3_event_free(pxui_sdl3_event *event)
{
#if defined(PXUI_SDL3_IOS)
  if (event->dialog_urls != NULL) pxui_dialog_release_urls(event->dialog_urls);
#endif
  event->dialog_urls = NULL;
  free(event->text);
  event->text = NULL;
  event->text_len = 0;
}

static bool take_dialog_event(pxui_sdl3_event *event)
{
  pxui_dialog_result result;
  if (!pxui_dialog_take(&result)) return false;
  memset(event, 0, sizeof(*event));
  event->dialog_urls = result.urls;
  event->tag = EVENT_DIALOG;
  event->id = result.id;
  event->window = result.window;
  event->a = (int32_t)result.outcome;
  event->text = result.payload;
  event->text_len = result.payload_length;
  if (result.outcome == PXUI_OUTCOME_FAILED && event->text == NULL) {
    event->text = copy_text("the file dialog failed", &event->text_len);
  }
  return event->text != NULL || result.outcome == PXUI_OUTCOME_CANCELLED;
}

static bool take_scroll_event(pxui_sdl3_event *event)
{
  memset(event, 0, sizeof(*event));
  event->tag = EVENT_SCROLL;
  if (!pxui_sdl3_scroll_take(&event->x, &event->y, &event->a, &event->z)) return false;
  /* the trackpad monitor sees AppKit events, not SDL windows: the window under the pointer owns them */
  SDL_Window *under = SDL_GetMouseFocus();
  event->window = under != NULL ? SDL_GetWindowID(under) : 0;
  return true;
}

static _Thread_local unsigned int pump_depth;
static _Thread_local bool in_live_redraw;
static bool live_watch_installed;
static _Atomic(pxui_sdl3_live_resize_callback) live_resize_callback;
static bool test_resize_next_poll;

static bool SDLCALL live_resize_watch(void *userdata, SDL_Event *event)
{
  pxui_sdl3_live_resize_callback callback;
  (void)userdata;
  callback = atomic_load_explicit(&live_resize_callback, memory_order_acquire);
  /* Watches can run on the thread that pushed an event. Redraw only inside the
     host's window event pump, which the Rust layer restricts to the main thread. */
  if (pump_depth == 0 || in_live_redraw
      || (event->type != SDL_EVENT_WINDOW_RESIZED && event->type != SDL_EVENT_WINDOW_PIXEL_SIZE_CHANGED)
      || callback == NULL) return true;
  in_live_redraw = true;
  callback(event->window.windowID);
  in_live_redraw = false;
  return true;
}

static void live_resize_install(void)
{
  if (!live_watch_installed) live_watch_installed = SDL_AddEventWatch(live_resize_watch, NULL);
}

void pxui_sdl3_set_live_resize_callback(pxui_sdl3_live_resize_callback callback)
{
  atomic_store_explicit(&live_resize_callback, callback, memory_order_release);
}

bool pxui_sdl3_test_polling_event_queue(void) { return pump_depth != 0; }
void pxui_sdl3_test_queue_resize_during_poll(void) { test_resize_next_poll = true; }

static void begin_pump(void)
{
  live_resize_install();
  pump_depth++;
  if (test_resize_next_poll) {
    SDL_Event resize_event;
    test_resize_next_poll = false;
    SDL_zero(resize_event);
    resize_event.window.type = SDL_EVENT_WINDOW_PIXEL_SIZE_CHANGED;
    resize_event.window.data1 = 640;
    resize_event.window.data2 = 480;
    SDL_SetEventEnabled(resize_event.type, true);
    SDL_PushEvent(&resize_event);
  }
}

bool pxui_sdl3_poll_event(pxui_sdl3_event *event)
{
  SDL_Event native_event;
  if (take_dialog_event(event)) return true;
  if (take_scroll_event(event)) return true;
  begin_pump();
  while (SDL_PollEvent(&native_event)) {
    if (translate_event(&native_event, event)) {
      pump_depth--;
      return true;
    }
  }
  pump_depth--;
  return take_scroll_event(event);
}

bool pxui_sdl3_wait_event_timeout(int timeout_ms)
{
  bool ready;
  begin_pump();
  ready = SDL_WaitEventTimeout(NULL, timeout_ms);
  pump_depth--;
  return ready;
}

bool pxui_sdl3_wake(void)
{
#ifdef PXUI_SDL3_IOS
  return pxui_sdl3_ios_wake() != 0;
#else
  SDL_Event event;
  SDL_zero(event);
  event.type = SDL_EVENT_USER;
  return SDL_PushEvent(&event);
#endif
}
