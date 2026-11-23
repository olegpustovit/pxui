#import <QuartzCore/CAMetalDisplayLink.h>
#import <QuartzCore/CAAnimation.h>
#import <objc/runtime.h>
#include "metal_link.h"

@interface PXUISDLMetalLink () <CAMetalDisplayLinkDelegate>
@end

@implementation PXUISDLMetalLink {
    CAMetalLayer *layer;
    CAMetalDisplayLink *link;
    void (^frame)(double);
    unsigned lastRate;
    float lastMaximum;
    BOOL inFrame;
}

- (instancetype)initWithLayer:(CAMetalLayer *)target frame:(void (^)(double))callback {
    if (!(self = [super init]) || !target.device) return nil;
    link = [[CAMetalDisplayLink alloc] initWithMetalLayer:target];
    if (!link) return nil;
    layer = target;
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

- (void)invalidate {
    [link invalidate];
    link = nil;
}

- (void)metalDisplayLink:(CAMetalDisplayLink *)sender needsUpdate:(CAMetalDisplayLinkUpdate *)update {
    if (sender.paused || inFrame) return;
    inFrame = YES;
    const void *key = pxui_metal_drawable_key();
    objc_setAssociatedObject(layer, key, update.drawable, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
    frame(fmax(0, fmin(0.1, update.targetPresentationTimestamp - CACurrentMediaTime())));
    objc_setAssociatedObject(layer, key, nil, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
    inFrame = NO;
}
@end
