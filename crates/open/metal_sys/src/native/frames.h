#pragma once
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
void *rays_metal_frames_create(void *layer, void *device, void *queue);
int rays_metal_frames_begin(void *host, uint32_t width, uint32_t height);
uint64_t rays_metal_frames_serial(void *host);
uint64_t rays_metal_frames_completed(void *host);
void *rays_metal_frames_commands(void *host);
void *rays_metal_frames_pass(void *host, double red, double green, double blue);
int rays_metal_frames_submit(void *host);
int rays_metal_frames_wait(void *host);
void rays_metal_frames_destroy(void *host);
#ifdef __cplusplus
}
#endif
