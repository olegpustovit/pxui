#pragma once
#include <stdint.h>

enum { RAYS_IOS_INIT, RAYS_IOS_FRAME, RAYS_IOS_PAUSE, RAYS_IOS_RESUME, RAYS_IOS_MEMORY, RAYS_IOS_FINISH };
enum { RAYS_IOS_NEXT, RAYS_IOS_WAIT, RAYS_IOS_EXIT, RAYS_IOS_FAIL };
typedef struct { uint32_t window; int32_t action; double delay; } rays_ios_step;
typedef rays_ios_step (*rays_ios_callback)(void *, int32_t);
int32_t rays_sdl3_run_ios(rays_ios_callback callback, void *user);
