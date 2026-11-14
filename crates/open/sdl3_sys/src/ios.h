#pragma once
#include <stdint.h>

enum { RAYS_IOS_INIT, RAYS_IOS_FRAME, RAYS_IOS_PAUSE, RAYS_IOS_RESUME, RAYS_IOS_MEMORY, RAYS_IOS_FINISH, RAYS_IOS_CONTROL };
enum { RAYS_IOS_NEXT, RAYS_IOS_WAIT, RAYS_IOS_EXIT, RAYS_IOS_FAIL };
typedef struct { uint32_t window; int32_t action; double delay; } rays_ios_step;
typedef rays_ios_step (*rays_ios_callback)(void *, int32_t);
int32_t rays_sdl3_run_ios(rays_ios_callback callback, void *user);
int32_t rays_sdl3_ios_wake(void);

typedef struct {
    float safe_area[4], keyboard_frame[4], keyboard_from[4];
    double keyboard_started, keyboard_duration, timestamp;
    uint32_t keyboard_curve, keyboard_visible;
    float keyboard_current[4];
    uint32_t keyboard_flags;
} rays_ios_environment;
int32_t rays_sdl3_ios_environment(uint32_t window, rays_ios_environment *out);
