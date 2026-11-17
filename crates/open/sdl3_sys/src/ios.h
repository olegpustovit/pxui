#pragma once
#include <stdint.h>

enum { PXUI_IOS_INIT, PXUI_IOS_FRAME, PXUI_IOS_PAUSE, PXUI_IOS_RESUME, PXUI_IOS_MEMORY, PXUI_IOS_FINISH, PXUI_IOS_CONTROL };
enum { PXUI_IOS_NEXT, PXUI_IOS_WAIT, PXUI_IOS_EXIT, PXUI_IOS_FAIL };
typedef struct { uint32_t window; int32_t action; double delay; } pxui_ios_step;
typedef pxui_ios_step (*pxui_ios_callback)(void *, int32_t);
int32_t pxui_sdl3_run_ios(pxui_ios_callback callback, void *user);
int32_t pxui_sdl3_ios_wake(void);

typedef struct {
    float safe_area[4], keyboard_frame[4], keyboard_from[4];
    double keyboard_started, keyboard_duration, timestamp;
    uint32_t keyboard_curve, keyboard_visible;
    float keyboard_current[4];
    uint32_t keyboard_flags;
} pxui_ios_environment;
int32_t pxui_sdl3_ios_environment(uint32_t window, pxui_ios_environment *out);
