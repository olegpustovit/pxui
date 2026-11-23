#pragma once

#import <QuartzCore/CAMetalLayer.h>
#import <QuartzCore/CAFrameRateRange.h>
#import <objc/runtime.h>
#include <math.h>

// The delegate lends this layer's drawable during its callback. metal_sys consumes it at most once.
static inline const void *pxui_metal_drawable_key(void) {
    return (const void *)sel_registerName("pxuiDisplayLinkDrawable");
}

API_AVAILABLE(macos(14.0), ios(17.0))
@interface PXUISDLMetalLink : NSObject
- (instancetype)initWithLayer:(CAMetalLayer *)layer frame:(void (^)(double))frame;
- (void)setPaused:(BOOL)paused;
- (void)setRate:(unsigned)hz maximum:(float)maximum;
- (void)invalidate;
@end

static inline CAFrameRateRange pxui_frame_rate(unsigned hz, float maximum) API_AVAILABLE(macos(12.0), ios(15.0)) {
    maximum = fmaxf(1, maximum);
    if (!hz) return CAFrameRateRangeMake(maximum, maximum, maximum);
    float upper = fminf(30, maximum);
    float lower = fminf(hz, upper);
    // Sparse repaint schedules still need a prompt update when they arm the link.
    return CAFrameRateRangeMake(lower, upper, fmaxf(lower, fminf(20, upper)));
}
