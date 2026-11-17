#pragma once
#include <stdint.h>
// Mirrors pxui_types::ffi::A11yNode / A11yAction (checked by size in ax.rs); the native side never links pxui.
typedef struct {
    uint32_t tag, text_start, text_end, reserved;
    double now, min, max, step;
} PxuiAXValue;
typedef struct {
    uint64_t id;
    uint32_t parent, role, name_start, name_end, description_start, description_end;
    PxuiAXValue value;
    float rect[4];
    uint32_t states, actions, selection[2], child_count, first_child, next_sibling, last_child;
} PxuiAXNode;
typedef struct { uint64_t id; uint32_t action; uint32_t reserved; double payload; } PxuiAXAction;
typedef struct {
    char role[64], label[128], value[128];
    int has_number, enabled, selected, expanded, focused, actionable, children;
    double number, min, max, frame[4];
    uintptr_t identity;
} PxuiAXInfo;
void *pxui_sdl3_ax_open(void *layer, void (*wake)(void));
void pxui_sdl3_ax_close(void *host);
int pxui_sdl3_ax_voiceover(void);
int pxui_sdl3_ax_sync(void *host, const PxuiAXNode *nodes, uint32_t count, const char *strings, uint32_t length, uint32_t first_top);
void pxui_sdl3_ax_clear(void *host);
uint32_t pxui_sdl3_ax_take(void *host, PxuiAXAction *out, uint32_t capacity);
uint64_t pxui_sdl3_ax_notifications(void *host);
int pxui_sdl3_ax_test_find(void *host, const char *label, PxuiAXInfo *info);
int pxui_sdl3_ax_test_perform(void *host, const char *label, int kind, double number);
