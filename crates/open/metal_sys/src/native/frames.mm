#import <Foundation/Foundation.h>
#import <Metal/Metal.h>
#import <QuartzCore/CAMetalLayer.h>
#import <objc/runtime.h>
#import <QuartzCore/CAAnimation.h>
#import <QuartzCore/CAMediaTimingFunction.h>
#import <QuartzCore/CATransaction.h>
#include <math.h>
#include <TargetConditionals.h>
#if defined(PXUI_METAL_TEST_UTIL) && TARGET_OS_OSX
#import <QuartzCore/CARenderer.h>
#endif
#include "frames.h"
#include <new>
#include <string.h>
#include <time.h>
#if TARGET_OS_IOS
#import <UIKit/UIKit.h>
#endif

namespace {
constexpr size_t LayerLimit = 16;
struct LayerKey {
    float time, rect[4], opacity, curve[4];
    bool operator==(const LayerKey &other) const {
        return time == other.time && opacity == other.opacity &&
            memcmp(rect, other.rect, sizeof rect) == 0 && memcmp(curve, other.curve, sizeof curve) == 0;
    }
};
struct Layer {
    uint16_t view, hz;
    uint8_t count, autoreverse, color[4];
    uint64_t revision;
    double start;
    float offset, clip[4];
    LayerKey keys[9];
    bool operator==(const Layer &other) const {
        if (view != other.view || hz != other.hz || count != other.count || autoreverse != other.autoreverse ||
            revision != other.revision || start != other.start || offset != other.offset ||
            memcmp(color, other.color, sizeof color) != 0 || memcmp(clip, other.clip, sizeof clip) != 0) return false;
        for (size_t i = 0; i < count; ++i) {
            if (!(keys[i] == other.keys[i])) return false;
        }
        return true;
    }
};
static_assert(sizeof(LayerKey) == 40 && sizeof(Layer) == 416);

struct PXUIMetalFrames {
    CAMetalLayer *layer;
    id<MTLDevice> device;
    id<MTLCommandQueue> queue;
    MTLRenderPassDescriptor *pass;
    id<MTLCommandBuffer> pending[3];
    uint64_t serials[3];
    uint64_t serial;
    uint64_t completed;
    id<CAMetalDrawable> drawable;
    id<MTLCommandBuffer> commands;
    id<MTLRenderCommandEncoder> encoder;
    // One-shot readback (`rays_metal_frames_capture`): the next submit also copies its drawable here.
    bool capture;
    id<MTLBuffer> captureBuffer;
    id<MTLCommandBuffer> captured;
    uint32_t captureWidth;
    uint32_t captureHeight;
    // Damage-limited frames draw into `canvas`, which keeps the last frame; `copy` puts it on the fresh drawable.
    id<MTLTexture> canvas;
    bool canvasValid;
    uint32_t width;
    uint32_t height;
    id<MTLRenderPipelineState> fill;
    id<MTLRenderPipelineState> copy;
    MTLRenderPassDescriptor *present;
    struct {
        CALayer *clip;
        CALayer *fill;
        Layer shown;
    } overlay[LayerLimit];
    Layer staged[LayerLimit];
    size_t overlayCount, stageCount;
    double layerNow;
    double clockBase;
    bool clockSet;
    bool transactionalDrawable;
#if TARGET_OS_IOS
    id pauseObserver;
    id resumeObserver;
    bool suspended;
    bool failed;
#endif
};
}

namespace {
double cubic(double a, double b, double t) {
    const double u = 1 - t;
    return 3 * u * u * t * a + 3 * u * t * t * b + t * t * t;
}
double derivative(double a, double b, double t) {
    const double u = 1 - t;
    return 3 * u * u * a + 6 * u * t * (b-a) + 3 * t * t * (1-b);
}
CGRect rectangle(const float rect[4]) {
    return CGRectMake(rect[0], rect[1], rect[2], rect[3]);
}
void animate_layer(CALayer *fill, const Layer &run, double clockBase) {
    if (run.count < 2) return;
    const double duration = run.keys[run.count - 1].time;
    NSMutableArray *times = [NSMutableArray array];
    NSMutableArray *curves = [NSMutableArray array];
    NSMutableArray *bounds = [NSMutableArray array];
    NSMutableArray *positions = [NSMutableArray array];
    NSMutableArray *opacities = [NSMutableArray array];
    auto append = [&](double seconds, double t, const LayerKey &a, const LayerKey &b) {
        auto lerp = [&](size_t i) { return a.rect[i] + (b.rect[i] - a.rect[i]) * t; };
        CGRect size = CGRectMake(0, 0, lerp(2), lerp(3));
        CGPoint at = CGPointMake(lerp(0) - run.clip[0], lerp(1) - run.clip[1]);
        [times addObject:@(seconds / duration)];
        [bounds addObject:[NSValue value:&size withObjCType:@encode(CGRect)]];
        [positions addObject:[NSValue value:&at withObjCType:@encode(CGPoint)]];
        [opacities addObject:@(a.opacity + (b.opacity - a.opacity) * t)];
    };
    append(0, 0, run.keys[0], run.keys[0]);
    for (size_t i = 1; i < run.count; ++i) {
        const LayerKey &a = run.keys[i - 1], &b = run.keys[i];
        const float *p = b.curve;
        // Shorter, mathematically identical cubics keep CA's timing solver below one channel at 2x.
        for (int j = 0; j < 8; ++j) {
            const double lo = j / 8.0, hi = (j + 1) / 8.0;
            const double x0 = cubic(p[0], p[2], lo), x1 = cubic(p[0], p[2], hi);
            const double y0 = cubic(p[1], p[3], lo), y1 = cubic(p[1], p[3], hi);
            const double dx = (hi - lo) / (3 * (x1 - x0));
            const double dy = (hi - lo) / (3 * (y1 - y0));
            [curves addObject:[CAMediaTimingFunction functionWithControlPoints:
                derivative(p[0], p[2], lo) * dx :derivative(p[1], p[3], lo) * dy
                :1 - derivative(p[0], p[2], hi) * dx :1 - derivative(p[1], p[3], hi) * dy]];
            append(a.time + (b.time - a.time) * x1, y1, a, b);
        }
    }
    auto animation = [&](NSString *path, NSArray *values) {
        CAKeyframeAnimation *keyframes = [CAKeyframeAnimation animationWithKeyPath:path];
        keyframes.keyTimes = times;
        keyframes.timingFunctions = curves;
        keyframes.values = values;
        keyframes.duration = duration;
        keyframes.autoreverses = run.autoreverse != 0;
        keyframes.repeatCount = HUGE_VALF;
        keyframes.beginTime = [fill convertTime:clockBase + run.start fromLayer:nil];
        keyframes.timeOffset = run.offset;
        if (@available(macOS 12, iOS 15, *)) {
            if (run.hz) keyframes.preferredFrameRateRange = CAFrameRateRangeMake(run.hz, MAX(30.0f, (float)run.hz), run.hz);
        }
        [fill addAnimation:keyframes forKey:[@"pxui-keys." stringByAppendingString:path]];
    };
    animation(@"bounds", bounds);
    animation(@"position", positions);
    animation(@"opacity", opacities);
}

bool layers_changed(const PXUIMetalFrames *frames) {
    if (frames->stageCount != frames->overlayCount) return true;
    for (size_t i = 0; i < frames->stageCount; ++i) {
        if (!(frames->staged[i] == frames->overlay[i].shown)) return true;
    }
    return false;
}
void sync_layers(PXUIMetalFrames *frames) {
    if (!frames->clockSet && frames->stageCount) {
        frames->clockBase = CACurrentMediaTime() - frames->layerNow;
        frames->clockSet = true;
    }
    for (size_t i = 0; i < frames->stageCount; ++i) {
        auto &out = frames->overlay[i];
        const Layer &run = frames->staged[i];
        if (out.clip && run == out.shown) continue;
        if (!out.clip) {
            out.clip = [CALayer layer];
            out.clip.masksToBounds = YES;
            out.fill = [CALayer layer];
            out.fill.anchorPoint = CGPointZero;
            out.fill.allowsEdgeAntialiasing = YES;
            [out.clip addSublayer:out.fill];
            [frames->layer addSublayer:out.clip];
        }
        CGRect clip = rectangle(run.clip);
        out.clip.frame = clip;
        out.clip.contentsScale = frames->layer.contentsScale;
        out.fill.contentsScale = frames->layer.contentsScale;
        const LayerKey &a = run.keys[0];
        out.fill.bounds = CGRectMake(0, 0, a.rect[2], a.rect[3]);
        out.fill.position = CGPointMake(a.rect[0] - clip.origin.x, a.rect[1] - clip.origin.y);
        out.fill.opacity = a.opacity;
        if (run.revision != out.shown.revision || run.view != out.shown.view ||
            memcmp(run.color, out.shown.color, sizeof run.color) != 0) {
            CGColorSpaceRef space = frames->layer.colorspace;
            if (space) CGColorSpaceRetain(space); else space = CGColorSpaceCreateDeviceRGB();
            CGFloat rgba[4];
            for (size_t channel = 0; channel < 4; ++channel) rgba[channel] = run.color[channel] / 255.0;
            CGColorRef color = CGColorCreate(space, rgba);
            out.fill.backgroundColor = color;
            CGColorRelease(color);
            CGColorSpaceRelease(space);
        }
        [out.fill removeAllAnimations];
        animate_layer(out.fill, run, frames->clockBase);
        out.shown = run;
    }
    for (size_t i = frames->stageCount; i < frames->overlayCount; ++i) {
        auto &out = frames->overlay[i];
        [out.clip removeFromSuperlayer];
        out.clip = nil;
        out.fill = nil;
        out.shown = {};
    }
    frames->overlayCount = frames->stageCount;
}
}

bool rays_metal_frames_layers(void *raw, const void *records, size_t count, double now) {
    PXUIMetalFrames *frames = static_cast<PXUIMetalFrames *>(raw);
    if (!NSThread.isMainThread || !frames || !isfinite(now) || count > LayerLimit || (count && !records)) return false;
    const Layer *layers = static_cast<const Layer *>(records);
    for (size_t i = 0; i < count; ++i) {
        const Layer &layer = layers[i];
        if (!layer.count || layer.count > 9 || !isfinite(layer.start) || !isfinite(layer.offset) || layer.offset < 0 ||
            layer.clip[2] <= 0 || layer.clip[3] <= 0) return false;
        for (float v : layer.clip) if (!isfinite(v)) return false;
        for (size_t k = 0; k < layer.count; ++k) {
            const LayerKey &key = layer.keys[k];
            if (!isfinite(key.time) || (k ? key.time <= layer.keys[k - 1].time : key.time != 0) ||
                !isfinite(key.opacity) || key.opacity < 0 || key.opacity > 1 ||
                key.rect[2] <= 0 || key.rect[3] <= 0) return false;
            for (float v : key.rect) if (!isfinite(v)) return false;
            for (float v : key.curve) if (!isfinite(v) || v < 0 || v > 1) return false;
        }
    }
    if (count) memcpy(frames->staged, layers, count * sizeof(Layer));
    frames->stageCount = count;
    frames->layerNow = now;
    return true;
}

int rays_metal_frames_can_draw(void *raw) {
    if (!NSThread.isMainThread || !raw) return 0;
#if TARGET_OS_IOS
    PXUIMetalFrames *frames = static_cast<PXUIMetalFrames *>(raw);
    return !frames->suspended && UIApplication.sharedApplication.applicationState == UIApplicationStateActive;
#else
    return 1;
#endif
}

void *rays_metal_frames_create(void *layerPointer, void *devicePointer, void *queuePointer) {
    @autoreleasepool {
        if (![NSThread isMainThread] || !layerPointer || !devicePointer || !queuePointer) return NULL;
        PXUIMetalFrames *frames = new (std::nothrow) PXUIMetalFrames{};
        if (!frames) return NULL;
        frames->layer = (__bridge CAMetalLayer *)layerPointer;
        frames->device = (__bridge id<MTLDevice>)devicePointer;
        frames->queue = (__bridge id<MTLCommandQueue>)queuePointer;
        frames->layer.device = frames->device;
        frames->layer.pixelFormat = MTLPixelFormatBGRA8Unorm;
        frames->layer.framebufferOnly = YES;
        frames->layer.maximumDrawableCount = 3;
#if TARGET_OS_OSX
        frames->layer.displaySyncEnabled = YES;
#endif
        frames->pass = [MTLRenderPassDescriptor new];
        frames->present = [MTLRenderPassDescriptor new];
        frames->present.colorAttachments[0].storeAction = MTLStoreActionStore;
        // A fullscreen triangle: `fill` paints the scissored damage the panel colour (Metal has no scissored clear),
        // `copy` reads the whole canvas texel for texel onto a drawable that needs a full copy.
        static NSString *const source = @"#include <metal_stdlib>\nusing namespace metal;\n"
            "struct V { float4 p [[position]]; };\n"
            "vertex V pxui_full(uint id [[vertex_id]]) { float2 q = float2((id << 1) & 2, id & 2); V v; v.p = float4(q * 2.0 - 1.0, 0.0, 1.0); return v; }\n"
            "fragment float4 pxui_fill(constant float4 &c [[buffer(0)]]) { return c; }\n"
            "fragment float4 pxui_copy(V v [[stage_in]], texture2d<float> t [[texture(0)]]) { return t.read(uint2(v.p.xy)); }\n";
        NSError *error = nil;
        id<MTLLibrary> library = [frames->device newLibraryWithSource:source options:nil error:&error];
        MTLRenderPipelineDescriptor *pipeline = [MTLRenderPipelineDescriptor new];
        pipeline.vertexFunction = [library newFunctionWithName:@"pxui_full"];
        pipeline.colorAttachments[0].pixelFormat = MTLPixelFormatBGRA8Unorm;
        pipeline.fragmentFunction = [library newFunctionWithName:@"pxui_fill"];
        if (library) frames->fill = [frames->device newRenderPipelineStateWithDescriptor:pipeline error:&error];
        pipeline.fragmentFunction = [library newFunctionWithName:@"pxui_copy"];
        if (library) frames->copy = [frames->device newRenderPipelineStateWithDescriptor:pipeline error:&error];
        if (!frames->fill || !frames->copy) { delete frames; return NULL; }
#if TARGET_OS_IOS
        frames->suspended = UIApplication.sharedApplication.applicationState != UIApplicationStateActive;
        frames->pauseObserver = [NSNotificationCenter.defaultCenter addObserverForName:UIApplicationWillResignActiveNotification object:nil queue:nil usingBlock:^(NSNotification *) {
            frames->suspended = true;
            rays_metal_frames_cancel(frames);
            if (!rays_metal_frames_wait(frames)) frames->failed = true;
            // An acquired but unsubmitted serial has no remaining GPU work after this drain.
            frames->completed = frames->serial;
        }];
        frames->resumeObserver = [NSNotificationCenter.defaultCenter addObserverForName:UIApplicationDidBecomeActiveNotification object:nil queue:nil usingBlock:^(NSNotification *) {
            frames->suspended = false;
        }];
#endif
        return frames;
    }
}

int rays_metal_frames_begin(void *raw, uint32_t width, uint32_t height) {
    @autoreleasepool {
        PXUIMetalFrames *frames = static_cast<PXUIMetalFrames *>(raw);
        if (![NSThread isMainThread] || frames->commands || !width || !height) return -1;
#if TARGET_OS_IOS
        if (frames->failed) return -1;
        if (!rays_metal_frames_can_draw(frames)) return 0;
#endif
        const NSUInteger slot = frames->serial % 3;
        [frames->pending[slot] waitUntilCompleted];
        for (NSUInteger i = 0; i < 3; ++i) {
            id<MTLCommandBuffer> previous = frames->pending[i];
            if (previous.status < MTLCommandBufferStatusCompleted) continue;
            if (i != slot && previous.status != MTLCommandBufferStatusError) continue;
            if (i != slot) [previous waitUntilCompleted];
            frames->completed = MAX(frames->completed, frames->serials[i]);
            frames->pending[i] = nil;
            if (previous.status == MTLCommandBufferStatusError) {
                frames->canvasValid = false;
                return -1;
            }
        }
        const CGSize size = CGSizeMake(width, height);
        if (!CGSizeEqualToSize(frames->layer.drawableSize, size) ||
            frames->width != width || frames->height != height) {
            frames->layer.drawableSize = size;
        }
        // This host's renderer and copy pipelines both target BGRA8Unorm.
        if (frames->layer.pixelFormat != MTLPixelFormatBGRA8Unorm) {
            frames->layer.pixelFormat = MTLPixelFormatBGRA8Unorm;
        }
        frames->commands = [frames->queue commandBuffer];
        if (!frames->commands) return -1;
        frames->width = width;
        frames->height = height;
        frames->serial++;
        return 1;
    }
}

uint64_t rays_metal_frames_serial(void *raw) { return (static_cast<PXUIMetalFrames *>(raw))->serial; }
uint64_t rays_metal_frames_completed(void *raw) { return (static_cast<PXUIMetalFrames *>(raw))->completed; }
void *rays_metal_frames_commands(void *raw) { return (__bridge void *)(static_cast<PXUIMetalFrames *>(raw))->commands; }

void *rays_metal_frames_pass(void *raw, double red, double green, double blue, const uint32_t (*rects)[4], uint32_t count, uint32_t *partial) {
    @autoreleasepool {
        PXUIMetalFrames *frames = static_cast<PXUIMetalFrames *>(raw);
        if (![NSThread isMainThread] || !frames->commands || frames->encoder) return NULL;
        const NSUInteger w = frames->width, h = frames->height;
        if (!frames->canvas || frames->canvas.width != w || frames->canvas.height != h) {
            MTLTextureDescriptor *d = [MTLTextureDescriptor texture2DDescriptorWithPixelFormat:frames->layer.pixelFormat
                width:w height:h mipmapped:NO];
            d.usage = MTLTextureUsageRenderTarget | MTLTextureUsageShaderRead;
            d.storageMode = MTLStorageModePrivate;
            frames->canvas = [frames->device newTextureWithDescriptor:d];
            frames->canvasValid = false;
            if (!frames->canvas) return NULL;
        }
        *partial = frames->canvasValid && count <= 8 ? count : 0;
        for (uint32_t i = 0; i < *partial; ++i) {
            const auto &r = rects[i];
            if (!r[2] || !r[3] || (uint64_t)r[0] + r[2] > w || (uint64_t)r[1] + r[3] > h
                || (r[2] == w && r[3] == h)) {
                *partial = 0;
            }
        }
        MTLRenderPassColorAttachmentDescriptor *color = frames->pass.colorAttachments[0];
        color.texture = frames->canvas;
        color.loadAction = *partial ? MTLLoadActionLoad : MTLLoadActionClear;
        color.storeAction = MTLStoreActionStore;
        color.clearColor = MTLClearColorMake(red, green, blue, 1);
        frames->encoder = [frames->commands renderCommandEncoderWithDescriptor:frames->pass];
        if (frames->encoder && *partial) rays_metal_frames_scissor(raw, rects[0]);
        frames->canvasValid = frames->encoder != nil;
        return (__bridge void *)frames->encoder;
    }
}

void *rays_metal_frames_end_pass(void *raw) {
    PXUIMetalFrames *frames = static_cast<PXUIMetalFrames *>(raw);
    if (![NSThread isMainThread] || !frames->encoder) return NULL;
    [frames->encoder endEncoding];
    frames->encoder = nil;
    return (__bridge void *)frames->canvas;
}

void *rays_metal_frames_resume_pass(void *raw) {
    @autoreleasepool {
        PXUIMetalFrames *frames = static_cast<PXUIMetalFrames *>(raw);
        if (![NSThread isMainThread] || !frames->commands || !frames->canvas || frames->encoder) return NULL;
        frames->pass.colorAttachments[0].loadAction = MTLLoadActionLoad;
        frames->encoder = [frames->commands renderCommandEncoderWithDescriptor:frames->pass];
        return (__bridge void *)frames->encoder;
    }
}

void rays_metal_frames_scissor(void *raw, const uint32_t rect[4]) {
    PXUIMetalFrames *frames = static_cast<PXUIMetalFrames *>(raw);
    if (!frames->encoder || !rect[2] || !rect[3] ||
        (uint64_t)rect[0] + rect[2] > frames->canvas.width || (uint64_t)rect[1] + rect[3] > frames->canvas.height) return;
    const MTLClearColor color = frames->pass.colorAttachments[0].clearColor;
    const float panel[4] = {(float)color.red, (float)color.green, (float)color.blue, 1};
    [frames->encoder setScissorRect:(MTLScissorRect){rect[0], rect[1], rect[2], rect[3]}];
    [frames->encoder setRenderPipelineState:frames->fill];
    [frames->encoder setFragmentBytes:panel length:sizeof panel atIndex:0];
    [frames->encoder drawPrimitives:MTLPrimitiveTypeTriangle vertexStart:0 vertexCount:3];
}

void rays_metal_frames_cancel(void *raw) {
    PXUIMetalFrames *frames = static_cast<PXUIMetalFrames *>(raw);
    [frames->encoder endEncoding];
    frames->encoder = nil;
    frames->commands = nil;
    frames->drawable = nil;
    frames->pass.colorAttachments[0].texture = nil;
    frames->canvasValid = false;
    if (frames->transactionalDrawable) {
        frames->layer.presentsWithTransaction = NO;
        frames->transactionalDrawable = false;
    }
}

int rays_metal_frames_acquire(void *raw, uint64_t *wait_ns) {
    @autoreleasepool {
        PXUIMetalFrames *frames = static_cast<PXUIMetalFrames *>(raw);
        if (![NSThread isMainThread]) return -1;
#if TARGET_OS_IOS
        if (frames->failed) return -1;
        if (!rays_metal_frames_can_draw(frames)) return 0;
#endif
        if (!frames->commands || !frames->canvasValid) return -1;
        [frames->encoder endEncoding];
        frames->encoder = nil;
        frames->transactionalDrawable = layers_changed(frames);
        if (frames->transactionalDrawable) frames->layer.presentsWithTransaction = YES;
        const uint64_t started = clock_gettime_nsec_np(CLOCK_UPTIME_RAW);
        // The display-link drawable predates capture and transaction mode changes.
        // Acquire a fresh drawable under the new mode for either transition.
        const void *key = (const void *)sel_registerName("pxuiDisplayLinkDrawable");
        id<CAMetalDrawable> drawable = objc_getAssociatedObject(frames->layer, key);
        if (drawable) objc_setAssociatedObject(frames->layer, key, nil, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
        if (frames->capture || frames->transactionalDrawable) drawable = nil;
        if (!drawable) drawable = [frames->layer nextDrawable];
        *wait_ns = clock_gettime_nsec_np(CLOCK_UPTIME_RAW) - started;
        id<MTLTexture> texture = drawable.texture;
        if (!frames->commands || texture.width != frames->width || texture.height != frames->height ||
            texture.pixelFormat != frames->canvas.pixelFormat || texture.framebufferOnly != frames->layer.framebufferOnly) {
            return 0;
        }
        frames->drawable = drawable;
        return 1;
    }
}

int rays_metal_frames_submit(void *raw) {
    @autoreleasepool {
        PXUIMetalFrames *frames = static_cast<PXUIMetalFrames *>(raw);
        if (![NSThread isMainThread]) return -1;
#if TARGET_OS_IOS
        if (frames->failed) return -1;
        if (!rays_metal_frames_can_draw(frames)) return 0;
#endif
        if (!frames->commands || frames->encoder || !frames->canvasValid) return -1;
        const bool displayed = frames->drawable != nil;
        const NSUInteger slot = (frames->serial - 1) % 3;
        const bool updateLayers = displayed && frames->transactionalDrawable;
        if (displayed) {
            id<MTLTexture> texture = frames->drawable.texture;
            auto *color = frames->present.colorAttachments[0];
            color.texture = texture;
            color.loadAction = MTLLoadActionDontCare;
            id<MTLRenderCommandEncoder> present = [frames->commands renderCommandEncoderWithDescriptor:frames->present];
            color.texture = nil;
            if (!present) return -1;
            present.label = @"Render Command 1 pxui present full";
            [present setRenderPipelineState:frames->copy];
            [present setFragmentTexture:frames->canvas atIndex:0];
            [present drawPrimitives:MTLPrimitiveTypeTriangle vertexStart:0 vertexCount:3];
            [present endEncoding];

            if (frames->capture) {
                const NSUInteger width = texture.width, height = texture.height, bytes = width * height * 4;
                if (frames->captureBuffer.length < bytes)
                    frames->captureBuffer = [frames->device newBufferWithLength:bytes options:MTLResourceStorageModeShared];
                if (!frames->captureBuffer) return -1;
                id<MTLBlitCommandEncoder> blit = [frames->commands blitCommandEncoder];
                if (!blit) return -1;
                [blit copyFromTexture:texture sourceSlice:0 sourceLevel:0 sourceOrigin:MTLOriginMake(0, 0, 0)
                           sourceSize:MTLSizeMake(width, height, 1) toBuffer:frames->captureBuffer
                    destinationOffset:0 destinationBytesPerRow:width * 4 destinationBytesPerImage:0];
                [blit endEncoding];
                frames->captured = frames->commands;
                frames->captureWidth = (uint32_t)width;
                frames->captureHeight = (uint32_t)height;
            }
            if (!updateLayers) [frames->commands presentDrawable:frames->drawable];
        }
        if (updateLayers) {
            [CATransaction begin];
            [CATransaction setDisableActions:YES];
            sync_layers(frames);
            [frames->commands commit];
            [frames->commands waitUntilScheduled];
            [frames->drawable present];
            [CATransaction commit];
        } else {
            [frames->commands commit];
        }
        if (frames->transactionalDrawable) {
            frames->layer.presentsWithTransaction = NO;
            frames->transactionalDrawable = false;
        }
        frames->pending[slot] = frames->commands;
        frames->serials[slot] = frames->serial;
        frames->commands = nil;
        frames->drawable = nil;
        frames->pass.colorAttachments[0].texture = nil;
        return displayed ? 1 : 0;
    }
}

int rays_metal_frames_capture(void *raw, int on) {
    PXUIMetalFrames *frames = static_cast<PXUIMetalFrames *>(raw);
    if (![NSThread isMainThread] || frames->commands) return -1;
    frames->capture = on != 0;
    frames->layer.framebufferOnly = !frames->capture;
    return 1;
}

int rays_metal_frames_read(void *raw, uint8_t *out, size_t length, uint32_t *width, uint32_t *height) {
    @autoreleasepool {
        PXUIMetalFrames *frames = static_cast<PXUIMetalFrames *>(raw);
        if (![NSThread isMainThread]) return -1;
        if (!frames->captured) return 0;
        *width = frames->captureWidth;
        *height = frames->captureHeight;
        const size_t bytes = (size_t)frames->captureWidth * frames->captureHeight * 4;
        if (!out) return 1;
        if (length != bytes) return -1;
        [frames->captured waitUntilCompleted];
        const bool failed = frames->captured.status == MTLCommandBufferStatusError;
        frames->captured = nil;
        if (failed) return -1;
        memcpy(out, frames->captureBuffer.contents, bytes);
        return 1;
    }
}

int rays_metal_frames_wait(void *raw) {
    @autoreleasepool {
        PXUIMetalFrames *frames = static_cast<PXUIMetalFrames *>(raw);
        BOOL success = YES;
#if TARGET_OS_IOS
        if (frames->failed) success = NO;
#endif
        for (NSUInteger i = 0; i < 3; i++) {
            [frames->pending[i] waitUntilCompleted];
            if (frames->pending[i].status == MTLCommandBufferStatusError) {
                success = NO;
                frames->canvasValid = false;
            }
            frames->completed = MAX(frames->completed, frames->serials[i]);
            frames->pending[i] = nil;
        }
        return success ? 1 : 0;
    }
}

void rays_metal_frames_destroy(void *raw) {
    @autoreleasepool {
        PXUIMetalFrames *frames = static_cast<PXUIMetalFrames *>(raw);
#if TARGET_OS_IOS
        [NSNotificationCenter.defaultCenter removeObserver:frames->pauseObserver];
        [NSNotificationCenter.defaultCenter removeObserver:frames->resumeObserver];
#endif
        [frames->encoder endEncoding];
        frames->encoder = nil;
        rays_metal_frames_wait(frames);
        delete frames;
    }
}

#if defined(PXUI_METAL_TEST_UTIL) && TARGET_OS_OSX
extern "C" bool rays_metal_frames_test_read_canvas(void *raw, uint8_t *pixels, size_t len) {
    @autoreleasepool {
        auto *frame = static_cast<PXUIMetalFrames *>(raw);
        if (!frame->commands || !frame->encoder || len != size_t(frame->width) * frame->height * 4) return false;
        if (!frame->captureBuffer || frame->captureBuffer.length < len)
            frame->captureBuffer = [frame->device newBufferWithLength:len options:MTLResourceStorageModeShared];
        if (!frame->captureBuffer) return false;
        rays_metal_frames_end_pass(raw);
        id<MTLBlitCommandEncoder> blit = [frame->commands blitCommandEncoder];
        [blit copyFromTexture:frame->canvas sourceSlice:0 sourceLevel:0 sourceOrigin:MTLOriginMake(0, 0, 0)
                   sourceSize:MTLSizeMake(frame->width, frame->height, 1) toBuffer:frame->captureBuffer
            destinationOffset:0 destinationBytesPerRow:frame->width * 4 destinationBytesPerImage:len];
        [blit endEncoding];
        id<MTLCommandBuffer> commands = frame->commands;
        if (rays_metal_frames_submit(raw) != 0) return false;
        [commands waitUntilCompleted];
        frame->completed = frame->serial;
        if (commands.status != MTLCommandBufferStatusCompleted) return false;
        memcpy(pixels, frame->captureBuffer.contents, len);
        return true;
    }
}

extern "C" bool rays_metal_frames_test_layers(void *raw, uint32_t width, uint32_t height, uint32_t density,
                                              const uint8_t *background, const double *times,
                                              size_t count, uint8_t *pixels, size_t len) {
    @autoreleasepool {
        const size_t stride = size_t(width) * height * 4;
        if (len != stride * count) return false;
        auto *frame = static_cast<PXUIMetalFrames *>(raw);
        PXUIMetalFrames probe{};
        probe.layer = [CAMetalLayer layer];
        probe.layer.frame = CGRectMake(0, 0, width, height);
        probe.layer.colorspace = frame->layer.colorspace;
        CGColorSpaceRef space = CGColorSpaceCreateDeviceRGB();
        CGFloat rgba[] = { background[0] / 255.0, background[1] / 255.0, background[2] / 255.0, 1 };
        CGColorRef clear = CGColorCreate(space, rgba);
        probe.layer.backgroundColor = clear;
        CGColorRelease(clear);
        CGColorSpaceRelease(space);
        probe.stageCount = frame->stageCount;
        probe.clockSet = true;
        probe.clockBase = 1;
        probe.layer.speed = 0;
        probe.layer.timeOffset = probe.clockBase;
        for (size_t i = 0; i < probe.stageCount; ++i) {
            Layer &run = probe.staged[i];
            run = frame->staged[i];
            for (float &v : run.clip) v *= density;
            for (size_t k = 0; k < run.count; ++k)
                for (float &v : run.keys[k].rect) v *= density;
        }

        MTLTextureDescriptor *desc = [MTLTextureDescriptor texture2DDescriptorWithPixelFormat:MTLPixelFormatBGRA8Unorm width:width height:height mipmapped:NO];
        desc.usage = MTLTextureUsageRenderTarget;
        desc.storageMode = MTLStorageModeShared;
        id<MTLTexture> target = [frame->device newTextureWithDescriptor:desc];
        if (!target) return false;
        CARenderer *renderer = [CARenderer rendererWithMTLTexture:target options:@{kCARendererMetalCommandQueue:frame->queue}];
        renderer.layer = probe.layer;
        renderer.bounds = probe.layer.bounds;
        [CATransaction begin];
        [CATransaction setDisableActions:YES];
        sync_layers(&probe);
        [CATransaction commit];
        for (size_t i = 0; i < count; ++i) {
            @autoreleasepool {
                [CATransaction begin];
                [CATransaction setDisableActions:YES];
                probe.layer.timeOffset = probe.clockBase + times[i];
                [CATransaction commit];
                [CATransaction flush];
                [renderer beginFrameAtTime:CACurrentMediaTime() timeStamp:nullptr];
                [renderer addUpdateRect:renderer.bounds];
                [renderer render];
                [renderer endFrame];
                id<MTLCommandBuffer> fence = [frame->queue commandBuffer];
                [fence commit];
                [fence waitUntilCompleted];
                if (fence.status != MTLCommandBufferStatusCompleted) return false;
                [target getBytes:pixels + i * stride bytesPerRow:width * 4
                       fromRegion:MTLRegionMake2D(0, 0, width, height) mipmapLevel:0];
            }
        }
        return true;
    }
}
#endif
