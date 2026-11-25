// Process-wide malloc/calloc/realloc stack aggregation (Apple malloc_logger). Active only while capturing.
// Aggregation is allocation-free (mmap table + unfair lock); symbolisation happens in Rust after stop.
#include <stdint.h>
#include <string.h>
#include <pthread.h>
#include <sys/mman.h>
#include <os/lock.h>
#include <unwind.h>

typedef void (malloc_logger_t)(uint32_t, uintptr_t, uintptr_t, uintptr_t, uintptr_t, uint32_t);
extern malloc_logger_t *malloc_logger;

#define TRACE_DEPTH 64
#define TRACE_SLOTS (1 << 16)

typedef struct {
    uint64_t h, count, bytes;
    uint32_t n;
    uint8_t main, kind, used;
    uintptr_t pc[TRACE_DEPTH];
} trace_ent;

typedef struct {
    void **buf;
    int n;
    int max;
} unwind_buf;

static trace_ent *tab;
static os_unfair_lock lock = OS_UNFAIR_LOCK_INIT;
static volatile int capturing;
static uint64_t dropped;
static malloc_logger_t *previous_logger;

static _Unwind_Reason_Code unwind_fill(struct _Unwind_Context *ctx, void *arg) {
    unwind_buf *u = (unwind_buf *)arg;
    if (u->n >= u->max) return _URC_END_OF_STACK;
    uintptr_t ip = _Unwind_GetIP(ctx);
    if (ip) {
        // Return addresses point one past the call; leave as-is for atos/dladdr.
        u->buf[u->n++] = (void *)ip;
    }
    return _URC_NO_REASON;
}

static int capture_stack(void **buf, int max) {
    unwind_buf u = { buf, 0, max };
    _Unwind_Backtrace(unwind_fill, &u);
    return u.n;
}

static void trace_log(uint32_t t, uintptr_t a1, uintptr_t a2, uintptr_t a3, uintptr_t r, uint32_t skip) {
    (void)a1; (void)r; (void)skip;
    if (!capturing || !(t & 2)) return;
    uint64_t size = (t & 4) ? a3 : a2;
    uint8_t kind = (t & 4) ? 2 : (t & 64) ? 1 : 0;
    void *buf[TRACE_DEPTH];
    int n = capture_stack(buf, TRACE_DEPTH);
    uint8_t main = pthread_main_np() != 0;
    uint64_t h = 1469598103934665603ull ^ main;
    for (int i = 0; i < n; i++) { h ^= (uintptr_t)buf[i]; h *= 1099511628211ull; }
    os_unfair_lock_lock(&lock);
    uint32_t i = (uint32_t)(h ^ (h >> 29)) & (TRACE_SLOTS - 1);
    for (uint32_t probe = 0; probe < TRACE_SLOTS; probe++, i = (i + 1) & (TRACE_SLOTS - 1)) {
        trace_ent *e = &tab[i];
        if (!e->used) {
            e->used = 1; e->h = h; e->n = (uint32_t)n; e->main = main; e->kind = kind;
            memcpy(e->pc, buf, sizeof(void *) * (size_t)n);
            e->count = 1; e->bytes = size;
            os_unfair_lock_unlock(&lock);
            return;
        }
        if (e->h == h && e->n == (uint32_t)n && e->main == main
            && memcmp(e->pc, buf, sizeof(void *) * (size_t)n) == 0) {
            e->count++; e->bytes += size;
            os_unfair_lock_unlock(&lock);
            return;
        }
    }
    dropped++;
    os_unfair_lock_unlock(&lock);
}

void mem_sys_trace_start(void) {
    if (!tab) {
        tab = mmap(0, sizeof(trace_ent) * TRACE_SLOTS, PROT_READ | PROT_WRITE, MAP_ANON | MAP_PRIVATE, -1, 0);
    }
    capturing = 0;
    memset(tab, 0, sizeof(trace_ent) * TRACE_SLOTS);
    dropped = 0;
    if (malloc_logger != trace_log) {
        previous_logger = malloc_logger;
        malloc_logger = trace_log;
    }
    capturing = 1;
}

void mem_sys_trace_stop(void) {
    capturing = 0;
}

void mem_sys_trace_resume(void) {
    if (tab) capturing = 1;
}

void mem_sys_trace_clear(void) {
    int was = capturing;
    capturing = 0;
    if (tab) memset(tab, 0, sizeof(trace_ent) * TRACE_SLOTS);
    dropped = 0;
    capturing = was;
}

int mem_sys_trace_capturing(void) { return capturing; }

uint64_t mem_sys_trace_dropped(void) { return dropped; }

typedef struct {
    uint64_t count, bytes;
    uint32_t n;
    uint8_t main, kind;
    uintptr_t pc[TRACE_DEPTH];
} mem_sys_trace_raw;

int mem_sys_trace_snapshot(mem_sys_trace_raw *out, int max) {
    capturing = 0;
    if (!tab || !out || max <= 0) return 0;
    int written = 0;
    for (uint32_t i = 0; i < TRACE_SLOTS && written < max; i++) {
        trace_ent *e = &tab[i];
        if (!e->used) continue;
        mem_sys_trace_raw *d = &out[written++];
        d->count = e->count;
        d->bytes = e->bytes;
        d->n = e->n;
        d->main = e->main;
        d->kind = e->kind;
        memcpy(d->pc, e->pc, sizeof(uintptr_t) * TRACE_DEPTH);
    }
    return written;
}

int mem_sys_trace_depth(void) { return TRACE_DEPTH; }
int mem_sys_trace_slot_count(void) { return TRACE_SLOTS; }
