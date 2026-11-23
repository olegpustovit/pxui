#import <Foundation/Foundation.h>
#import <Metal/Metal.h>
#import <QuartzCore/CAMetalLayer.h>
#include "frames.h"
#include <new>
#include <string.h>
#include <TargetConditionals.h>
#if TARGET_OS_IOS
#import <UIKit/UIKit.h>
#endif

namespace {
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
#if TARGET_OS_IOS
    id pauseObserver;
    id resumeObserver;
    bool suspended;
    bool failed;
#endif
};
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
#if TARGET_OS_IOS
        frames->suspended = UIApplication.sharedApplication.applicationState != UIApplicationStateActive;
        frames->pauseObserver = [NSNotificationCenter.defaultCenter addObserverForName:UIApplicationWillResignActiveNotification object:nil queue:nil usingBlock:^(NSNotification *) {
            frames->suspended = true;
            if (frames->encoder) [frames->encoder endEncoding];
            frames->encoder = nil;
            frames->commands = nil;
            frames->drawable = nil;
            frames->pass.colorAttachments[0].texture = nil;
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
        id<MTLCommandBuffer> previous = frames->pending[slot];
        if (previous) {
            [previous waitUntilCompleted];
            if (previous.status == MTLCommandBufferStatusError) return -1;
            frames->completed = MAX(frames->completed, frames->serials[slot]);
            frames->pending[slot] = nil;
        }
        const CGSize size = CGSizeMake(width, height);
        if (!CGSizeEqualToSize(frames->layer.drawableSize, size)) frames->layer.drawableSize = size;
        frames->drawable = [frames->layer nextDrawable];
        if (!frames->drawable) return 0;
        frames->commands = [frames->queue commandBuffer];
        if (!frames->commands) { frames->drawable = nil; return -1; }
        frames->serial++;
        return 1;
    }
}

uint64_t rays_metal_frames_serial(void *raw) { return (static_cast<PXUIMetalFrames *>(raw))->serial; }
uint64_t rays_metal_frames_completed(void *raw) { return (static_cast<PXUIMetalFrames *>(raw))->completed; }
void *rays_metal_frames_commands(void *raw) { return (__bridge void *)(static_cast<PXUIMetalFrames *>(raw))->commands; }

void *rays_metal_frames_pass(void *raw, double red, double green, double blue) {
    @autoreleasepool {
        PXUIMetalFrames *frames = static_cast<PXUIMetalFrames *>(raw);
        if (![NSThread isMainThread] || !frames->commands || frames->encoder) return NULL;
        MTLRenderPassColorAttachmentDescriptor *color = frames->pass.colorAttachments[0];
        color.texture = frames->drawable.texture;
        color.loadAction = MTLLoadActionClear;
        color.storeAction = MTLStoreActionStore;
        color.clearColor = MTLClearColorMake(red, green, blue, 1);
        frames->encoder = [frames->commands renderCommandEncoderWithDescriptor:frames->pass];
        return (__bridge void *)frames->encoder;
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
        if (!frames->commands || !frames->encoder) return -1;
        [frames->encoder endEncoding];
        frames->encoder = nil;
        if (frames->capture) {
            id<MTLTexture> texture = frames->drawable.texture;
            const NSUInteger width = texture.width, height = texture.height, bytes = width * height * 4;
            if (!frames->captureBuffer || frames->captureBuffer.length < bytes)
                frames->captureBuffer = [frames->device newBufferWithLength:bytes options:MTLResourceStorageModeShared];
            id<MTLBlitCommandEncoder> blit = [frames->commands blitCommandEncoder];
            if (!frames->captureBuffer || !blit) return -1;
            [blit copyFromTexture:texture sourceSlice:0 sourceLevel:0 sourceOrigin:MTLOriginMake(0, 0, 0)
                       sourceSize:MTLSizeMake(width, height, 1) toBuffer:frames->captureBuffer destinationOffset:0
            destinationBytesPerRow:width * 4 destinationBytesPerImage:bytes];
            [blit endEncoding];
            frames->captured = frames->commands;
            frames->captureWidth = (uint32_t)width;
            frames->captureHeight = (uint32_t)height;
        }
        [frames->commands presentDrawable:frames->drawable];
        [frames->commands commit];
        const NSUInteger slot = (frames->serial - 1) % 3;
        frames->pending[slot] = frames->commands;
        frames->serials[slot] = frames->serial;
        frames->commands = nil;
        frames->drawable = nil;
        frames->pass.colorAttachments[0].texture = nil;
        return 1;
    }
}

int rays_metal_frames_capture(void *raw, int on) {
    PXUIMetalFrames *frames = static_cast<PXUIMetalFrames *>(raw);
    if (![NSThread isMainThread] || frames->commands) return -1;
    // Drawables acquired after this point are blit-readable only while capturing.
    frames->layer.framebufferOnly = on ? NO : YES;
    frames->capture = on != 0;
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
            if (frames->pending[i].status == MTLCommandBufferStatusError) success = NO;
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
