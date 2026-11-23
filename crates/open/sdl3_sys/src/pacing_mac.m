#import <AppKit/AppKit.h>
#import <QuartzCore/CAAnimation.h>
#include "metal_link.h"
#include "bridge.h"

typedef void (*pxui_tick)(void *, double);

@interface PXUISDLWindowPacing : NSObject
@property(nonatomic, readonly) uint64_t presentation;
- (instancetype)initWithLayer:(CAMetalLayer *)layer;
- (BOOL)tick:(pxui_tick)callback user:(void *)user rate:(unsigned)hz;
- (void)refreshVisibility;
- (void)pause;
@end

@implementation PXUISDLWindowPacing {
    __weak NSWindow *window;
    CAMetalLayer *layer;
    id link;
    uint64_t linkFailedAt;
    uint64_t generation;
    BOOL visible;
    pxui_tick tick;
    void *tickUser;
}

- (instancetype)initWithLayer:(CAMetalLayer *)target {
    if (!(self = [super init])) return nil;
    NSView *view = (NSView *)target.delegate;
    if (![view isKindOfClass:NSView.class] || !view.window) return nil;
    window = view.window;
    layer = target;
    generation = 1;
    visible = [self isVisible];
    NSNotificationCenter *center = NSNotificationCenter.defaultCenter;
    [center addObserver:self selector:@selector(visibility:) name:NSWindowDidChangeOcclusionStateNotification object:window];
    [center addObserver:self selector:@selector(visibility:) name:NSWindowDidMiniaturizeNotification object:window];
    [center addObserver:self selector:@selector(visibility:) name:NSWindowDidDeminiaturizeNotification object:window];
    [center addObserver:self selector:@selector(display:) name:NSWindowDidChangeScreenNotification object:window];
    [center addObserver:self selector:@selector(display:) name:NSWindowDidChangeBackingPropertiesNotification object:window];
    return self;
}

- (void)dealloc {
    [NSNotificationCenter.defaultCenter removeObserver:self];
    if (@available(macOS 14.0, *)) [(PXUISDLMetalLink *)link invalidate];
}

- (BOOL)isVisible {
    return window.visible && !window.miniaturized && (window.occlusionState & NSWindowOcclusionStateVisible);
}

- (void)visibility:(NSNotification *)notification {
    (void)notification;
    [self refreshVisibility];
}

- (void)refreshVisibility {
    BOOL next = [self isVisible];
    if (next == visible) return;
    visible = next;
    if (!visible) [self pause];
    ++generation;
    pxui_sdl3_wake();
}

- (void)display:(NSNotification *)notification {
    (void)notification;
    ++generation;
    pxui_sdl3_wake();
}

- (uint64_t)presentation {
    return visible ? generation : 0;
}

- (void)pause {
    if (@available(macOS 14.0, *)) [(PXUISDLMetalLink *)link setPaused:YES];
}

- (void)frame:(double)lead {
    pxui_tick callback = tick;
    tick = NULL;
    if (callback && self.presentation) callback(tickUser, lead);
}

- (BOOL)tick:(pxui_tick)callback user:(void *)user rate:(unsigned)hz {
    if (@available(macOS 14.0, *)) {
        if (!self.presentation || generation == linkFailedAt || tick || !layer.device) return NO;
        if (!link) {
            __weak PXUISDLWindowPacing *weak = self;
            link = [[PXUISDLMetalLink alloc] initWithLayer:layer frame:^(double lead) { [weak frame:lead]; }];
            if (!link) { linkFailedAt = generation; return NO; }
        }
        tick = callback;
        tickUser = user;
        [(PXUISDLMetalLink *)link setRate:hz maximum:window.screen.maximumFramesPerSecond];
        [(PXUISDLMetalLink *)link setPaused:NO];
        // A display can disappear or stop delivering updates during a screen transition.
        CFTimeInterval end = CACurrentMediaTime() + 0.1;
        while (tick && self.presentation) {
            CFTimeInterval remaining = end - CACurrentMediaTime();
            if (remaining <= 0 || CFRunLoopRunInMode(kCFRunLoopDefaultMode, remaining, true) == kCFRunLoopRunFinished) break;
        }
        BOOL served = tick == NULL;
        if (!served) {
            [self pause];
            if (self.presentation) {
                [(PXUISDLMetalLink *)link invalidate];
                link = nil;
                linkFailedAt = generation;
            }
        }
        tick = NULL;
        tickUser = NULL;
        return served;
    }
    return NO;
}
@end

void *pxui_sdl3_window_pacing_open(void *layer) {
    @autoreleasepool {
        if (!NSThread.isMainThread || !layer) return NULL;
        return (__bridge_retained void *)[[PXUISDLWindowPacing alloc] initWithLayer:(__bridge CAMetalLayer *)layer];
    }
}

void pxui_sdl3_window_pause_link(void *pacing) {
    [(__bridge PXUISDLWindowPacing *)pacing pause];
}

void pxui_sdl3_window_refresh_visibility(void *pacing) {
    [(__bridge PXUISDLWindowPacing *)pacing refreshVisibility];
}

uint64_t pxui_sdl3_window_presentation(void *pacing) {
    return ((__bridge PXUISDLWindowPacing *)pacing).presentation;
}

bool pxui_sdl3_window_tick(void *pacing, unsigned hz, pxui_tick callback, void *user) {
    @autoreleasepool {
        return [(__bridge PXUISDLWindowPacing *)pacing tick:callback user:user rate:hz];
    }
}

void pxui_sdl3_window_pacing_close(void *pacing) {
    CFRelease(pacing);
}

#ifdef PXUI_SDL3_TEST_UTIL
void pxui_sdl3_test_order_front(void *layer, bool cover) {
    @autoreleasepool {
        NSView *view = (NSView *)((__bridge CAMetalLayer *)layer).delegate;
        NSWindow *window = view.window;
        window.level = NSFloatingWindowLevel + (cover ? 1 : 0);
        [window orderFrontRegardless];
    }
}
#endif
