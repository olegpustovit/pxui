#import <Foundation/Foundation.h>
#import <Metal/Metal.h>
#import <QuartzCore/CAMetalLayer.h>
#include "frames.h"
#include <atomic>
#include <new>
#include <string.h>
#include <time.h>
#include <TargetConditionals.h>
#if TARGET_OS_IOS
#import <UIKit/UIKit.h>
#elif TARGET_OS_OSX
#import <AppKit/AppKit.h>
#endif

namespace {
std::atomic<bool> presentVerificationFailed{false};

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
    // Weak identity distinguishes textures even if a replacement reuses an object's address.
    __weak id<MTLTexture> textures[4];
    size_t replaceTexture;
    id colorSpace;
    bool resetPresent;
    bool fullPresent;
    uint64_t verifyEvery;
    uint64_t presents;
    std::atomic<uint64_t> verified{0};
    id<MTLBuffer> verifyBuffers[3];
#if TARGET_OS_IOS
    id pauseObserver;
    id resumeObserver;
    UIScreen *screen;
    bool suspended;
    bool failed;
#elif TARGET_OS_OSX
    id presentObservers[5];
    bool windowVisible;
#endif
};

void readPresentTexture(id<MTLBlitCommandEncoder> blit, id<MTLTexture> texture,
                        id<MTLBuffer> buffer, NSUInteger offset) {
    const NSUInteger rowBytes = texture.width * 4;
    [blit copyFromTexture:texture sourceSlice:0 sourceLevel:0 sourceOrigin:MTLOriginMake(0, 0, 0)
               sourceSize:MTLSizeMake(texture.width, texture.height, 1) toBuffer:buffer
        destinationOffset:offset destinationBytesPerRow:rowBytes destinationBytesPerImage:0];
}
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

void *rays_metal_frames_create(void *layerPointer, void *devicePointer, void *queuePointer,
                               int fullPresent, uint64_t verifyEvery) {
    @autoreleasepool {
        if (![NSThread isMainThread] || !layerPointer || !devicePointer || !queuePointer) return NULL;
        PXUIMetalFrames *frames = new (std::nothrow) PXUIMetalFrames{};
        if (!frames) return NULL;
        frames->layer = (__bridge CAMetalLayer *)layerPointer;
        frames->device = (__bridge id<MTLDevice>)devicePointer;
        frames->queue = (__bridge id<MTLCommandQueue>)queuePointer;
        frames->layer.device = frames->device;
        frames->layer.pixelFormat = MTLPixelFormatBGRA8Unorm;
        frames->fullPresent = fullPresent != 0;
        frames->verifyEvery = verifyEvery;
        frames->layer.framebufferOnly = frames->fullPresent && !verifyEvery;
        frames->resetPresent = true;
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
            frames->resetPresent = true;
        }];
#elif TARGET_OS_OSX
        NSNotificationCenter *center = NSNotificationCenter.defaultCenter;
        frames->presentObservers[0] = [center addObserverForName:NSWindowDidChangeOcclusionStateNotification object:nil queue:nil usingBlock:^(NSNotification *note) {
            NSView *view = (NSView *)frames->layer.delegate;
            NSWindow *window = note.object;
            if ((window.occlusionState & NSWindowOcclusionStateVisible) &&
                (![view isKindOfClass:NSView.class] || view.window == window))
                frames->resetPresent = true;
        }];
        void (^screenChanged)(NSNotification *) = ^(NSNotification *note) {
            NSView *view = (NSView *)frames->layer.delegate;
            if (![view isKindOfClass:NSView.class] || view.window == note.object)
                frames->resetPresent = true;
        };
        frames->presentObservers[1] = [center addObserverForName:NSWindowDidChangeScreenNotification object:nil queue:nil usingBlock:screenChanged];
        frames->presentObservers[4] = [center addObserverForName:NSWindowDidChangeScreenProfileNotification object:nil queue:nil usingBlock:screenChanged];
        frames->presentObservers[2] = [center addObserverForName:NSApplicationDidChangeScreenParametersNotification object:nil queue:nil usingBlock:^(NSNotification *) {
            frames->resetPresent = true;
        }];
        frames->presentObservers[3] = [center addObserverForName:NSApplicationDidBecomeActiveNotification object:nil queue:nil usingBlock:^(NSNotification *) {
            frames->resetPresent = true;
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
                frames->resetPresent = true;
                return -1;
            }
        }
        const CGSize size = CGSizeMake(width, height);
        if (!CGSizeEqualToSize(frames->layer.drawableSize, size) ||
            frames->width != width || frames->height != height) {
            frames->layer.drawableSize = size;
            frames->resetPresent = true;
        }
        // This host's renderer and copy pipelines both target BGRA8Unorm.
        if (frames->layer.pixelFormat != MTLPixelFormatBGRA8Unorm) {
            frames->layer.pixelFormat = MTLPixelFormatBGRA8Unorm;
            frames->resetPresent = true;
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
    frames->resetPresent = true;
}

int rays_metal_frames_acquire(void *raw, uint64_t *wait_ns, int *historySlot) {
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
        const uint64_t started = clock_gettime_nsec_np(CLOCK_UPTIME_RAW);
        id<CAMetalDrawable> drawable = [frames->layer nextDrawable];
        *wait_ns = clock_gettime_nsec_np(CLOCK_UPTIME_RAW) - started;
        id<MTLTexture> texture = drawable.texture;
        if (!frames->commands || texture.width != frames->width || texture.height != frames->height ||
            texture.pixelFormat != frames->canvas.pixelFormat || (frames->capture && texture.framebufferOnly)) {
            frames->resetPresent = true;
            return 0;
        }
        frames->drawable = drawable;
#if TARGET_OS_IOS
        UIView *view = (UIView *)frames->layer.delegate;
        UIScreen *screen = [view isKindOfClass:UIView.class] ? view.window.windowScene.screen : nil;
        if (frames->screen != screen) {
            frames->screen = screen;
            frames->resetPresent = true;
        }
#elif TARGET_OS_OSX
        NSView *view = (NSView *)frames->layer.delegate;
        const bool visible = [view isKindOfClass:NSView.class] && view.window.isVisible;
        // An explicit show can reach the host before AppKit sends its occlusion notification.
        if (visible && !frames->windowVisible) frames->resetPresent = true;
        frames->windowVisible = visible;
#endif
        id space = (__bridge id)frames->layer.colorspace;
        if (frames->colorSpace != space && (!frames->colorSpace || !space ||
            !CFEqual((__bridge CFTypeRef)frames->colorSpace, (__bridge CFTypeRef)space))) {
            frames->colorSpace = space;
            frames->resetPresent = true;
        }
        if (!frames->fullPresent && !frames->capture && !presentVerificationFailed.load(std::memory_order_relaxed) &&
            !texture.framebufferOnly) {
            size_t index = 0;
            while (index < 4 && frames->textures[index] != texture) ++index;
            if (index == 4) {
                index = frames->replaceTexture++ % 4;
                frames->textures[index] = texture;
                frames->resetPresent = true;
            }
            *historySlot = (int)index;
            const bool reset = frames->resetPresent;
            frames->resetPresent = false;
            return reset ? 2 : 1;
        }
        return 1;
    }
}

int rays_metal_frames_submit(void *raw, const uint32_t (*rects)[4], uint32_t count) {
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
        if (displayed) {
            id<MTLTexture> texture = frames->drawable.texture;
            const bool partial = count && !frames->fullPresent && !frames->capture &&
                !presentVerificationFailed.load(std::memory_order_relaxed);
            if (partial) {
                if (!rects || count > 8 || texture.framebufferOnly) return -1;
                for (uint32_t i = 0; i < count; ++i) {
                    const auto &r = rects[i];
                    if (!r[2] || !r[3] || (uint64_t)r[0] + r[2] > frames->width ||
                        (uint64_t)r[1] + r[3] > frames->height) return -1;
                }
            }
            MTLRenderPassColorAttachmentDescriptor *color = frames->present.colorAttachments[0];
            color.texture = texture;
            color.loadAction = partial ? MTLLoadActionLoad : MTLLoadActionDontCare;
            id<MTLRenderCommandEncoder> present = [frames->commands renderCommandEncoderWithDescriptor:frames->present];
            color.texture = nil;
            if (!present) return -1;
            present.label = partial ? @"Render Command 1 pxui present damage" : @"Render Command 1 pxui present full";
            [present setRenderPipelineState:frames->copy];
            [present setFragmentTexture:frames->canvas atIndex:0];
            if (partial) {
                for (uint32_t i = 0; i < count; ++i) {
                    const auto &r = rects[i];
                    [present setScissorRect:(MTLScissorRect){r[0], r[1], r[2], r[3]}];
                    [present drawPrimitives:MTLPrimitiveTypeTriangle vertexStart:0 vertexCount:3];
                }
            } else {
                [present drawPrimitives:MTLPrimitiveTypeTriangle vertexStart:0 vertexCount:3];
            }
            [present endEncoding];

            const bool verify = frames->verifyEvery && !presentVerificationFailed.load(std::memory_order_relaxed) &&
                ++frames->presents % frames->verifyEvery == 0 && !texture.framebufferOnly;
            if (frames->capture || verify) {
                const NSUInteger width = texture.width, height = texture.height, bytes = width * height * 4;
                id<MTLBlitCommandEncoder> blit = [frames->commands blitCommandEncoder];
                if (!blit) return -1;
                if (frames->capture) {
                    if (frames->captureBuffer.length < bytes)
                        frames->captureBuffer = [frames->device newBufferWithLength:bytes options:MTLResourceStorageModeShared];
                    if (frames->captureBuffer) {
                        readPresentTexture(blit, texture, frames->captureBuffer, 0);
                        frames->captured = frames->commands;
                        frames->captureWidth = (uint32_t)width;
                        frames->captureHeight = (uint32_t)height;
                    }
                }
                if (verify) {
                    auto &buffer = frames->verifyBuffers[slot];
                    if (buffer.length < 2 * bytes)
                        buffer = [frames->device newBufferWithLength:2 * bytes options:MTLResourceStorageModeShared];
                    if (buffer) {
                        readPresentTexture(blit, texture, buffer, 0);
                        readPresentTexture(blit, frames->canvas, buffer, bytes);
                        id<MTLBuffer> snapshot = buffer;
                        // Frame retirement and destruction wait for this handler before reusing the slot.
                        [frames->commands addCompletedHandler:^(id<MTLCommandBuffer> command) {
                            if (command.status != MTLCommandBufferStatusCompleted) return;
                            const auto *pixels = static_cast<const uint8_t *>(snapshot.contents);
                            if (memcmp(pixels, pixels + bytes, bytes) &&
                                !presentVerificationFailed.exchange(true, std::memory_order_relaxed)) {
                                fprintf(stderr, "PXUI_PRESENT_VERIFY mismatch=1 partial_present=disabled\n");
                            }
                            frames->verified.fetch_add(1, std::memory_order_release);
                        }];
                    }
                }
                [blit endEncoding];
                if ((frames->capture && !frames->captureBuffer) || (verify && !frames->verifyBuffers[slot])) {
                    frames->captured = nil;
                    return -1;
                }
            }
            [frames->commands presentDrawable:frames->drawable];
        }
        [frames->commands commit];
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
    frames->layer.framebufferOnly = frames->fullPresent && !frames->capture && !frames->verifyEvery;
    frames->resetPresent = true;
    return 1;
}

int rays_metal_frames_full_present(void *raw, int on) {
    PXUIMetalFrames *frames = static_cast<PXUIMetalFrames *>(raw);
    if (![NSThread isMainThread] || frames->commands) return -1;
    frames->fullPresent = on != 0;
    frames->layer.framebufferOnly = frames->fullPresent && !frames->capture && !frames->verifyEvery;
    frames->resetPresent = true;
    return 1;
}

uint64_t rays_metal_frames_verified(void *raw, int *failed) {
    PXUIMetalFrames *frames = static_cast<PXUIMetalFrames *>(raw);
    const uint64_t verified = frames->verified.load(std::memory_order_acquire);
    *failed = presentVerificationFailed.load(std::memory_order_relaxed);
    return verified;
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
                frames->resetPresent = true;
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
#elif TARGET_OS_OSX
        for (id observer : frames->presentObservers)
            [NSNotificationCenter.defaultCenter removeObserver:observer];
#endif
        [frames->encoder endEncoding];
        frames->encoder = nil;
        rays_metal_frames_wait(frames);
        delete frames;
    }
}
