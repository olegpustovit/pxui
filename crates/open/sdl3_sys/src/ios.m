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
    BOOL busy, watching, keyboardVisible;
    __weak UIWindow *hostWindow;
    UIScreen *keyboardScreen;
    CGRect keyboardFrom, keyboardFrame;
    CFTimeInterval keyboardStarted, keyboardDuration;
    NSUInteger keyboardCurve;
#ifdef RAYS_SDL3_TEST_UTIL
    unsigned suppressedWakes;
#endif
}
- (instancetype)initWithCallback:(rays_ios_callback)function user:(void *)context;
- (void)begin;
- (void)wake;
- (int32_t)requestWake;
#ifdef RAYS_SDL3_TEST_UTIL
- (int32_t)testNestedWake;
#endif
- (void)observe:(SDL_Event *)event;
- (int)shutdown:(int)status;
- (void)environment:(rays_ios_environment *)out window:(UIWindow *)window;
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
- (void)keyboard:(NSNotification *)notification {
    if (atomic_load(&finished)) return;
    id screen = notification.object;
    NSDictionary *info = notification.userInfo;
    NSValue *from = info[UIKeyboardFrameBeginUserInfoKey], *to = info[UIKeyboardFrameEndUserInfoKey];
    if (![screen isKindOfClass:UIScreen.class] || (hostWindow && ![screen isEqual:hostWindow.screen]) || !from || !to) return;
    keyboardScreen = screen;
    keyboardFrom = from.CGRectValue; keyboardFrame = to.CGRectValue;
    keyboardStarted = CACurrentMediaTime();
    keyboardDuration = [info[UIKeyboardAnimationDurationUserInfoKey] doubleValue];
    keyboardCurve = [info[UIKeyboardAnimationCurveUserInfoKey] unsignedIntegerValue];
    atomic_store(&dirty, true);
    [self post];
}
- (void)keyboardVisibility:(NSNotification *)notification {
    if (atomic_load(&finished) || ![notification.object isEqual:hostWindow.screen]) return;
    keyboardVisible = [notification.name isEqualToString:UIKeyboardDidShowNotification];
    atomic_store(&dirty, true);
    [self post];
}
- (void)environment:(rays_ios_environment *)out window:(UIWindow *)window {
    if (![keyboardScreen isEqual:window.screen]) return;
    UIView *view = window.rootViewController.view;
    CGRect from = [keyboardScreen.coordinateSpace convertRect:keyboardFrom toCoordinateSpace:view];
    CGRect to = [keyboardScreen.coordinateSpace convertRect:keyboardFrame toCoordinateSpace:view];
    float a[4] = {(float)from.origin.x,(float)from.origin.y,(float)from.size.width,(float)from.size.height};
    float b[4] = {(float)to.origin.x,(float)to.origin.y,(float)to.size.width,(float)to.size.height};
    memcpy(out->keyboard_from,a,sizeof(a)); memcpy(out->keyboard_frame,b,sizeof(b));
    out->keyboard_started = keyboardStarted; out->keyboard_duration = keyboardDuration;
    out->keyboard_curve = (uint32_t)keyboardCurve;
    out->keyboard_visible = keyboardVisible ? 1 : 0;
}
- (void)begin {
    [NSNotificationCenter.defaultCenter addObserver:self selector:@selector(keyboard:) name:UIKeyboardWillChangeFrameNotification object:nil];
    [NSNotificationCenter.defaultCenter addObserver:self selector:@selector(keyboardVisibility:) name:UIKeyboardDidShowNotification object:nil];
    [NSNotificationCenter.defaultCenter addObserver:self selector:@selector(keyboardVisibility:) name:UIKeyboardDidHideNotification object:nil];
    rays_ios_step initial = [self invoke:RAYS_IOS_INIT];
    if (initial.action == RAYS_IOS_FAIL || initial.action == RAYS_IOS_EXIT) {
        [self finish:initial.action == RAYS_IOS_FAIL]; return;
    }
    SDL_Window *window = SDL_GetWindowFromID(initial.window);
    if (!window) { [self failure:"initialization did not supply a live SDL window"]; return; }
    UIWindow *native = (__bridge UIWindow *)SDL_GetPointerProperty(SDL_GetWindowProperties(window), SDL_PROP_WINDOW_UIKIT_WINDOW_POINTER, NULL);
    if (!native) { [self failure:"SDL window has no UIKit window"]; return; }
    hostWindow = native;
    CFRunLoopSourceContext context = {0}; context.info = (__bridge void *)self; context.perform = ios_source;
    CFRunLoopSourceRef createdSource = CFRunLoopSourceCreate(kCFAllocatorDefault, 0, &context);
    pthread_mutex_lock(&loop_lock); source = createdSource; pthread_mutex_unlock(&loop_lock);
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
    pthread_mutex_lock(&loop_lock);
    CFRunLoopSourceRef retained = source;
    if (retained) CFRetain(retained);
    pthread_mutex_unlock(&loop_lock);
    if (retained) { CFRunLoopSourceSignal(retained); CFRunLoopWakeUp(CFRunLoopGetMain()); CFRelease(retained); }
}
- (int32_t)requestWake {
    if (atomic_load(&finished)) return 0;
    atomic_store(&dirty, true);
    [self post];
    return 1;
}
#ifdef RAYS_SDL3_TEST_UTIL
- (int32_t)testNestedWake {
    if (!busy || atomic_load(&foreground)) return 0;
    unsigned before = suppressedWakes;
    [self requestWake];
    for (unsigned i = 0; i < 16 && suppressedWakes == before; ++i)
        CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0, true);
    return suppressedWakes != before;
}
#endif
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
    if (atomic_load(&finished)) return;
    if (busy) {
#ifdef RAYS_SDL3_TEST_UTIL
        ++suppressedWakes;
#endif
        return;
    }
    [self drain];
    dispatch_source_set_timer(timer, DISPATCH_TIME_FOREVER, DISPATCH_TIME_FOREVER, 0);
    if (!atomic_load(&foreground)) {
        link.paused = YES;
        atomic_store(&dirty, false);
        rays_ios_step step = [self invoke:RAYS_IOS_CONTROL];
        if (step.action == RAYS_IOS_FAIL || step.action == RAYS_IOS_EXIT) {
            [self finish:step.action == RAYS_IOS_FAIL]; return;
        }
        if (step.action == RAYS_IOS_WAIT) [self wait:step.delay];
        else if (step.action != RAYS_IOS_NEXT) [self failure:"unknown iOS control schedule"];
        // A requested frame remains deferred until foreground; control deadlines use only the timer.
        if (atomic_load(&foreground)) link.paused = NO;
        // A nested native run loop can consume a source while the Rust callback is busy.
        if (atomic_load(&dirty)) [self post];
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
    [self wait:step.delay];
}
- (void)wait:(double)seconds {
    if (isnan(seconds) || seconds < 0) { [self failure:"invalid iOS control deadline"]; return; }
    uint64_t delay = isfinite(seconds) ? (uint64_t)MIN(seconds * NSEC_PER_SEC, (double)(INT64_MAX / 2)) : 0;
    dispatch_source_set_timer(timer, isfinite(seconds) ? dispatch_time(DISPATCH_TIME_NOW, (int64_t)delay) : DISPATCH_TIME_FOREVER,
        DISPATCH_TIME_FOREVER, 0);
}
- (int)shutdown:(int)status {
    if (atomic_exchange(&finished, true)) return status;
    [NSNotificationCenter.defaultCenter removeObserver:self];
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

int32_t rays_sdl3_ios_wake(void) {
    @autoreleasepool {
        pthread_mutex_lock(&loop_lock); PXUIIOSLoop *loop = active_loop; pthread_mutex_unlock(&loop_lock);
        return loop ? [loop requestWake] : 0;
    }
}
#ifdef RAYS_SDL3_TEST_UTIL
int32_t rays_sdl3_ios_test_nested_wake(void) {
    if (!pthread_main_np()) return 0;
    return [active_loop testNestedWake];
}
#endif

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

int32_t rays_sdl3_ios_environment(uint32_t id, rays_ios_environment *out) {
    if (!pthread_main_np() || !out) { SDL_SetError("iOS environment requires main and output"); return 0; }
    SDL_Window *window = SDL_GetWindowFromID(id);
    if (!window) return 0;
    UIWindow *native = (__bridge UIWindow *)SDL_GetPointerProperty(SDL_GetWindowProperties(window), SDL_PROP_WINDOW_UIKIT_WINDOW_POINTER, NULL);
    if (!native) { SDL_SetError("SDL window has no UIKit window"); return 0; }
    SDL_Rect safe; int width, height;
    if (!SDL_GetWindowSafeArea(window,&safe) || !SDL_GetWindowSize(window,&width,&height)) return 0;
    rays_ios_environment value = {0};
    value.safe_area[0] = (float)MAX(0,safe.y);
    value.safe_area[1] = (float)MAX(0,width-safe.x-safe.w);
    value.safe_area[2] = (float)MAX(0,height-safe.y-safe.h);
    value.safe_area[3] = (float)MAX(0,safe.x);
    value.timestamp = CACurrentMediaTime();
    [active_loop environment:&value window:native];
    *out = value;
    return 1;
}
