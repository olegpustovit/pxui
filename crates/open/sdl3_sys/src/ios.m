#import <UIKit/UIKit.h>
#import <QuartzCore/CADisplayLink.h>
#include <stdatomic.h>
#include <pthread.h>
#include <math.h>
#include "bridge.h"
#include "ios.h"

enum { PENDING_PAUSE = 1, PENDING_RESUME = 2, PENDING_MEMORY = 4, PENDING_EXIT = 8 };
@class PXUIIOSLoop;
static PXUIIOSLoop *active_loop;
static pthread_mutex_t loop_lock = PTHREAD_MUTEX_INITIALIZER;
static bool ios_event_watch(void *unused, SDL_Event *event);
static void ios_source(void *context);

@interface PXUIIOSLoop : NSObject {
    rays_ios_callback callback;
    void *user;
    CADisplayLink *link;
    dispatch_source_t timer;
    CFRunLoopSourceRef source;
    atomic_bool finished, foreground, dirty;
    atomic_uint pending;
    BOOL busy, watching;
}
- (instancetype)initWithCallback:(rays_ios_callback)function user:(void *)context;
- (void)begin;
- (void)wake;
- (void)observe:(SDL_Event *)event;
- (int)shutdown:(int)status;
@end

@implementation PXUIIOSLoop
- (instancetype)initWithCallback:(rays_ios_callback)function user:(void *)context {
    if ((self = [super init])) {
        callback = function; user = context;
        atomic_init(&finished, false); atomic_init(&foreground, false);
        atomic_init(&dirty, true); atomic_init(&pending, 0);
    }
    return self;
}
- (void)dealloc { if (source) CFRelease(source); }
- (rays_ios_step)invoke:(int)signal {
    busy = YES;
    rays_ios_step step = callback(user, signal);
    busy = NO;
    return step;
}
- (void)finish:(int)status {
    exit([self shutdown:status]);
}
- (void)failure:(const char *)message {
    SDL_SetError("%s", message);
    fprintf(stderr, "PXUI iOS loop: %s\n", message);
    [self finish:1];
}
- (void)begin {
    rays_ios_step initial = [self invoke:RAYS_IOS_INIT];
    if (initial.action == RAYS_IOS_FAIL || initial.action == RAYS_IOS_EXIT) {
        [self finish:initial.action == RAYS_IOS_FAIL]; return;
    }
    SDL_Window *window = SDL_GetWindowFromID(initial.window);
    if (!window) { [self failure:"initialization did not supply a live SDL window"]; return; }
    UIWindow *native = (__bridge UIWindow *)SDL_GetPointerProperty(SDL_GetWindowProperties(window), SDL_PROP_WINDOW_UIKIT_WINDOW_POINTER, NULL);
    if (!native) { [self failure:"SDL window has no UIKit window"]; return; }
    CFRunLoopSourceContext context = {0}; context.info = (__bridge void *)self; context.perform = ios_source;
    source = CFRunLoopSourceCreate(kCFAllocatorDefault, 0, &context);
    if (!source) { [self failure:"could not create the iOS wake source"]; return; }
    CFRunLoopAddSource(CFRunLoopGetMain(), source, kCFRunLoopCommonModes);
    timer = dispatch_source_create(DISPATCH_SOURCE_TYPE_TIMER, 0, 0, dispatch_get_main_queue());
    if (!timer) { [self failure:"could not create the iOS repaint timer"]; return; }
    __weak PXUIIOSLoop *weak = self;
    dispatch_source_set_event_handler(timer, ^{
        PXUIIOSLoop *loop = weak;
        [loop wake];
    });
    dispatch_source_set_timer(timer, DISPATCH_TIME_FOREVER, DISPATCH_TIME_FOREVER, 0);
    dispatch_activate(timer);
    link = [CADisplayLink displayLinkWithTarget:self selector:@selector(frame:)];
    if (!link) { [self failure:"could not create the iOS display link"]; return; }
    float maximum = (float)native.screen.maximumFramesPerSecond;
    maximum = MAX(1, maximum);
    link.preferredFrameRateRange = CAFrameRateRangeMake(MIN(60, maximum), maximum, maximum);
    link.paused = YES;
    [link addToRunLoop:NSRunLoop.mainRunLoop forMode:NSRunLoopCommonModes];
    if (!SDL_AddEventWatch(ios_event_watch, NULL)) { [self failure:"could not register the iOS event watch"]; return; }
    watching = YES;
    SDL_SetiOSEventPump(false);
    atomic_store(&foreground, UIApplication.sharedApplication.applicationState == UIApplicationStateActive);
    [self wake];
}
- (void)post {
    if (source) { CFRunLoopSourceSignal(source); CFRunLoopWakeUp(CFRunLoopGetMain()); }
}
- (void)observe:(SDL_Event *)event {
    if (atomic_load(&finished) || event->type == SDL_EVENT_POLL_SENTINEL) return;
    unsigned signals = 0;
    switch (event->type) {
    case SDL_EVENT_WILL_ENTER_BACKGROUND:
        atomic_store(&foreground, false); signals = PENDING_PAUSE; break;
    case SDL_EVENT_DID_ENTER_FOREGROUND:
        atomic_store(&foreground, true); signals = PENDING_RESUME; break;
    case SDL_EVENT_DID_ENTER_BACKGROUND: case SDL_EVENT_WILL_ENTER_FOREGROUND:
        break;
    case SDL_EVENT_LOW_MEMORY: signals = PENDING_MEMORY; break;
    case SDL_EVENT_TERMINATING: case SDL_EVENT_QUIT: signals = PENDING_EXIT; break;
    default: break;
    }
    if (signals) atomic_fetch_or(&pending, signals);
    atomic_store(&dirty, true);
    // Leave SDL/UIKit notification dispatch before invoking Rust or touching the display link.
    [self post];
}
- (void)drain {
    unsigned signals = atomic_exchange(&pending, 0);
    for (unsigned bit = PENDING_PAUSE; bit <= PENDING_MEMORY; bit <<= 1) {
        if (!(signals & bit)) continue;
        int signal = bit == PENDING_PAUSE ? RAYS_IOS_PAUSE : bit == PENDING_RESUME ? RAYS_IOS_RESUME : RAYS_IOS_MEMORY;
        rays_ios_step step = [self invoke:signal];
        if (step.action == RAYS_IOS_FAIL || step.action == RAYS_IOS_EXIT)
            [self finish:step.action == RAYS_IOS_FAIL];
    }
    if (signals & PENDING_EXIT) [self finish:0];
}
- (void)wake {
    if (atomic_load(&finished) || busy) return;
    [self drain];
    dispatch_source_set_timer(timer, DISPATCH_TIME_FOREVER, DISPATCH_TIME_FOREVER, 0);
    if (!atomic_load(&foreground)) {
        link.paused = YES;
        return;
    }
    link.paused = NO;
}
- (void)frame:(CADisplayLink *)sender {
    (void)sender;
    if (atomic_load(&finished) || busy) return;
    [self drain];
    if (!atomic_load(&foreground)) { link.paused = YES; return; }
    atomic_store(&dirty, false);
    rays_ios_step step = [self invoke:RAYS_IOS_FRAME];
    [self drain];
    if (step.action == RAYS_IOS_FAIL || step.action == RAYS_IOS_EXIT) {
        [self finish:step.action == RAYS_IOS_FAIL]; return;
    }
    if (!atomic_load(&foreground)) { link.paused = YES; return; }
    if (step.action != RAYS_IOS_NEXT && step.action != RAYS_IOS_WAIT) {
        [self failure:"unknown iOS frame schedule"]; return;
    }
    if (step.action == RAYS_IOS_NEXT || atomic_load(&dirty)) return;
    if (isnan(step.delay) || step.delay < 0) { [self failure:"invalid iOS repaint deadline"]; return; }
    if (step.delay <= link.duration) return;
    link.paused = YES;
    uint64_t delay = isfinite(step.delay) ? (uint64_t)MIN(step.delay * NSEC_PER_SEC, (double)(INT64_MAX / 2)) : 0;
    dispatch_source_set_timer(timer, isfinite(step.delay) ? dispatch_time(DISPATCH_TIME_NOW, (int64_t)delay) : DISPATCH_TIME_FOREVER,
        DISPATCH_TIME_FOREVER, 0);
}
- (int)shutdown:(int)status {
    if (atomic_exchange(&finished, true)) return status;
    [link invalidate]; link = nil;
    if (timer) { dispatch_source_cancel(timer); timer = nil; }
    if (source) CFRunLoopSourceInvalidate(source);
    if (watching) { SDL_RemoveEventWatch(ios_event_watch, NULL); watching = NO; }
    rays_ios_step step = [self invoke:RAYS_IOS_FINISH];
    if (step.action == RAYS_IOS_FAIL) status = 1;
    pthread_mutex_lock(&loop_lock); active_loop = nil; pthread_mutex_unlock(&loop_lock);
    return status;
}
@end

static bool ios_event_watch(void *unused, SDL_Event *event) {
    (void)unused;
    @autoreleasepool {
        pthread_mutex_lock(&loop_lock); PXUIIOSLoop *loop = active_loop; pthread_mutex_unlock(&loop_lock);
        [loop observe:event];
    }
    return true;
}
static void ios_source(void *context) { [(__bridge PXUIIOSLoop *)context wake]; }
static int ios_main_loop(void) { [active_loop begin]; return 0; }

int32_t rays_sdl3_run_ios(rays_ios_callback callback, void *user) {
    if (!pthread_main_np() || !callback) return -1;
    PXUIIOSLoop *loop = [[PXUIIOSLoop alloc] initWithCallback:callback user:user];
    if (!loop) { SDL_SetError("could not allocate iOS loop"); return -1; }
    pthread_mutex_lock(&loop_lock);
    if (active_loop) { pthread_mutex_unlock(&loop_lock); return -1; }
    active_loop = loop;
    pthread_mutex_unlock(&loop_lock);
    @try {
        int status = rays_sdl3_run_app(ios_main_loop);
        return [loop shutdown:status];
    } @catch (NSException *exception) {
        fprintf(stderr, "PXUI iOS loop: %s\n", exception.reason.UTF8String);
        return [loop shutdown:1];
    }
}
