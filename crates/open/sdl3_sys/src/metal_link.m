#import <QuartzCore/CAMetalDisplayLink.h>
#import <QuartzCore/CAAnimation.h>
#import <objc/runtime.h>
#include "metal_link.h"

@interface PXUISDLMetalLink () <CAMetalDisplayLinkDelegate>
@end

@implementation PXUISDLMetalLink {
    CAMetalLayer *layer;
    CAMetalDisplayLink *link;
    BOOL (^frame)(double);
    unsigned lastRate;
    float lastMaximum;
    BOOL inFrame;
}

- (instancetype)initWithLayer:(CAMetalLayer *)target frame:(BOOL (^)(double))callback {
    if (!(self = [super init]) || !target.device) return nil;
    link = [[CAMetalDisplayLink alloc] initWithMetalLayer:target];
    if (!link) return nil;
    layer = target;
    // Ownership lasts through pauses and callbacks without a drawable loan.
    objc_setAssociatedObject(layer, sel_registerName("pxuiDisplayLinkOwner"), @YES, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
    frame = [callback copy];
    link.delegate = self;
    link.paused = YES;
    [link addToRunLoop:NSRunLoop.mainRunLoop forMode:NSRunLoopCommonModes];
    return self;
}

- (void)setPaused:(BOOL)paused {
    if (link.paused != paused) link.paused = paused;
}

- (void)setRate:(unsigned)hz maximum:(float)maximum {
    if (lastRate == hz && lastMaximum == maximum) return;
    lastRate = hz;
    lastMaximum = maximum;
    link.preferredFrameRateRange = pxui_frame_rate(hz, maximum);
}

- (void)releaseDrawable {
    objc_setAssociatedObject(layer, pxui_metal_drawable_key(), nil, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
    inFrame = NO;
}

- (void)invalidate {
    [self releaseDrawable];
    if (!link) return;
    [link invalidate];
    objc_setAssociatedObject(layer, sel_registerName("pxuiDisplayLinkOwner"), nil, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
    link = nil;
}

- (void)metalDisplayLink:(CAMetalDisplayLink *)sender needsUpdate:(CAMetalDisplayLinkUpdate *)update {
    if (sender.paused || inFrame) return;
    inFrame = YES;
    const void *key = pxui_metal_drawable_key();
    objc_setAssociatedObject(layer, key, update.drawable, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
    double lead = fmax(0, fmin(0.1, update.targetPresentationTimestamp - CACurrentMediaTime()));
    if (!frame(lead)) [self releaseDrawable];
}
@end
