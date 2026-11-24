#import <AppKit/AppKit.h>
#include "metal_link.h"
#include "bridge.h"

@interface PXUISDLWindowPacing : NSObject
@property(nonatomic, readonly) uint64_t presentation;
- (instancetype)initWithLayer:(CAMetalLayer *)layer;
- (BOOL)tick:(unsigned)hz lead:(double *)lead;
- (void)releaseDrawable;
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
    BOOL waiting;
    double frameLead;
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

- (BOOL)frame:(double)lead {
    if (!waiting || !self.presentation) return NO;
    frameLead = lead;
    waiting = NO;
    return YES;
}

- (void)releaseDrawable {
    if (@available(macOS 14.0, *)) [(PXUISDLMetalLink *)link releaseDrawable];
}

- (BOOL)tick:(unsigned)hz lead:(double *)lead {
    if (@available(macOS 14.0, *)) {
        if (!self.presentation || generation == linkFailedAt || waiting || !layer.device) return NO;
        if (!link) {
            __weak PXUISDLWindowPacing *weak = self;
            link = [[PXUISDLMetalLink alloc] initWithLayer:layer frame:^BOOL(double lead) { return [weak frame:lead]; }];
            if (!link) { linkFailedAt = generation; return NO; }
        }
        waiting = YES;
        frameLead = -1.0;
        [(PXUISDLMetalLink *)link setRate:hz maximum:window.screen.maximumFramesPerSecond];
        [(PXUISDLMetalLink *)link setPaused:NO];
        // A display can disappear or stop delivering updates during a screen transition.
        CFTimeInterval end = CACurrentMediaTime() + 0.1;
        while (waiting && self.presentation) {
            CFTimeInterval remaining = end - CACurrentMediaTime();
            if (remaining <= 0 || CFRunLoopRunInMode(kCFRunLoopDefaultMode, remaining, true) == kCFRunLoopRunFinished) break;
        }
        BOOL served = frameLead >= 0.0;
        if (!served) {
            [self pause];
            if (self.presentation) {
                [(PXUISDLMetalLink *)link invalidate];
                link = nil;
                linkFailedAt = generation;
            }
        }
        waiting = NO;
        *lead = frameLead;
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

bool pxui_sdl3_window_tick(void *pacing, unsigned hz, double *lead) {
    @autoreleasepool {
        return [(__bridge PXUISDLWindowPacing *)pacing tick:hz lead:lead];
    }
}

void pxui_sdl3_window_release_drawable(void *pacing) {
    [(__bridge PXUISDLWindowPacing *)pacing releaseDrawable];
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
