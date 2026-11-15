#include <stdint.h>
#include <time.h>
#include <errno.h>
#include <pthread.h>
#include <pthread/qos.h>
#include <os/signpost.h>

uint64_t mem_sys_thread_cpu_ns(void) {
    struct timespec time;
    if (clock_gettime(CLOCK_THREAD_CPUTIME_ID, &time) != 0) return UINT64_MAX;
    return (uint64_t)time.tv_sec * 1000000000 + (uint64_t)time.tv_nsec;
}

int mem_sys_ui_thread_priority(void) {
    if (!pthread_main_np()) return EINVAL;
    return pthread_set_qos_class_self_np(QOS_CLASS_USER_INTERACTIVE, 0);
}

static os_log_t frame_log;
static pthread_once_t frame_log_once = PTHREAD_ONCE_INIT;
static void create_frame_log(void) {
    frame_log = os_log_create("org.pxui.profile", OS_LOG_CATEGORY_POINTS_OF_INTEREST);
}
uint64_t mem_sys_frame_interval_begin(uint64_t frame) {
    pthread_once(&frame_log_once, create_frame_log);
    if (!os_signpost_enabled(frame_log)) return OS_SIGNPOST_ID_NULL;
    os_signpost_id_t interval = os_signpost_id_generate(frame_log);
    os_signpost_interval_begin(frame_log, interval, "UI build", "frame=%llu", (unsigned long long)frame);
    return interval;
}
void mem_sys_frame_interval_end(uint64_t interval, uint64_t wall, uint64_t cpu) {
    if (interval != OS_SIGNPOST_ID_NULL) os_signpost_interval_end(frame_log, interval, "UI build", "wall_us=%llu cpu_us=%llu qos=%u", (unsigned long long)wall, (unsigned long long)cpu, (unsigned int)qos_class_self());
}
