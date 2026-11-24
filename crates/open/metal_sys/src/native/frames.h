#pragma once
#include <stddef.h>
#include <stdint.h>
#include <stdbool.h>
#ifdef __cplusplus
extern "C" {
#endif
void *rays_metal_frames_create(void *layer, void *device, void *queue);
int rays_metal_frames_begin(void *host, uint32_t width, uint32_t height);
int rays_metal_frames_can_draw(void *host);
uint64_t rays_metal_frames_serial(void *host);
uint64_t rays_metal_frames_completed(void *host);
void *rays_metal_frames_commands(void *host);
void *rays_metal_frames_pass(void *host, double red, double green, double blue, const uint32_t (*rects)[4], uint32_t count, uint32_t *partial);
void rays_metal_frames_scissor(void *host, const uint32_t rect[4]);
void rays_metal_frames_cancel(void *host);
void *rays_metal_frames_end_pass(void *host);
void *rays_metal_frames_resume_pass(void *host);
int rays_metal_frames_acquire(void *host, uint64_t *wait_ns);
int rays_metal_frames_submit(void *host);
void *rays_metal_frames_present(void *host);
int rays_metal_frames_capture(void *host, int on);
int rays_metal_frames_read(void *host, uint8_t *out, size_t length, uint32_t *width, uint32_t *height);
int rays_metal_frames_wait(void *host);
void rays_metal_frames_destroy(void *host);
#ifdef __cplusplus
}
#endif
