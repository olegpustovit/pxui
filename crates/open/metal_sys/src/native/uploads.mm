#include "metal_handle.h"
#include "metal_kinds.h"
#include <Metal/Metal.h>
#include <algorithm>
#include <limits>
#include <new>
#include <memory>

namespace {
struct PXUIMetalUploads {
    id<MTLCommandQueue> queue;
    id<MTLBuffer> buffer;
};
constexpr NSUInteger upload_limit = 4 * 1024 * 1024;
}

extern "C" int32_t rays_metal_uploads_create(MetalHandle *device_handle, void **out, const char **message) {
    if (message) *message = nullptr;
    if (out) *out = nullptr;
    @try { @autoreleasepool {
        id<MTLDevice> device = (__bridge id<MTLDevice>)rays_metal_handle_object(device_handle, KIND_DEVICE);
        if (!NSThread.isMainThread || device == nil || out == nullptr)
            return rays_metal_set_invalid_argument(message, "invalid main-thread texture upload device");
        std::unique_ptr<PXUIMetalUploads> uploads(new (std::nothrow) PXUIMetalUploads{});
        if (!uploads) return rays_metal_set_error(message, "could not allocate texture upload context");
        uploads->queue = [device newCommandQueue];
        if (!uploads->queue) return rays_metal_set_error(message, "could not create texture upload queue");
        *out = uploads.release();
        return 0;
    } } @catch (NSException *exception) { return rays_metal_set_error(message, exception.reason.UTF8String); }
}
extern "C" void rays_metal_uploads_destroy(void *raw) {
    @autoreleasepool { delete static_cast<PXUIMetalUploads *>(raw); }
}

// The shared texture-transfer entry validates the entire CPU span and texture region first.
int32_t rays_metal_uploads_copy(void *raw, void *texture_pointer, uint8_t *bytes, bool write,
    NSUInteger source_row, NSUInteger source_image, NSUInteger texel,
    MTLRegion region, NSUInteger mip, NSUInteger slice, const char **message) {
    @autoreleasepool {
        auto *uploads = static_cast<PXUIMetalUploads *>(raw);
        id<MTLTexture> texture = (__bridge id<MTLTexture>)texture_pointer;
        if (!NSThread.isMainThread || uploads == nullptr || texture.device.registryID != uploads->queue.device.registryID)
            return rays_metal_set_invalid_argument(message, "texture upload device mismatch");
        const NSUInteger row_bytes = region.size.width * texel;
        if (row_bytes > upload_limit)
            return rays_metal_set_unsupported(message, "texture row exceeds the 4 MiB transfer limit");
        const NSUInteger pitch = (row_bytes + 255) & ~NSUInteger(255);
        const NSUInteger rows = std::min(region.size.height, upload_limit / pitch);
        const NSUInteger length = rows * pitch;
        if (uploads->buffer.length < length) {
            id<MTLBuffer> buffer = [texture.device newBufferWithLength:length options:MTLResourceStorageModeShared];
            if (!buffer) return rays_metal_set_error(message, "could not allocate texture upload scratch");
            uploads->buffer = buffer;
        }
        auto *scratch = static_cast<uint8_t *>(uploads->buffer.contents);
        for (NSUInteger plane = 0; plane < region.size.depth; ++plane) {
            for (NSUInteger row = 0; row < region.size.height; row += rows) {
                const NSUInteger count = std::min(rows, region.size.height - row);
                if (write) for (NSUInteger i = 0; i < count; ++i)
                    memcpy(scratch + i * pitch, bytes + plane * source_image + (row + i) * source_row, row_bytes);
                id<MTLCommandBuffer> commands = [uploads->queue commandBuffer];
                id<MTLBlitCommandEncoder> encoder = [commands blitCommandEncoder];
                if (!commands || !encoder) return rays_metal_set_error(message, "could not encode texture transfer");
                const MTLSize size = MTLSizeMake(region.size.width, count, 1);
                const MTLOrigin origin = MTLOriginMake(region.origin.x, region.origin.y + row, region.origin.z + plane);
                if (write) [encoder copyFromBuffer:uploads->buffer sourceOffset:0 sourceBytesPerRow:pitch sourceBytesPerImage:pitch * count sourceSize:size
                    toTexture:texture destinationSlice:slice destinationLevel:mip destinationOrigin:origin];
                else [encoder copyFromTexture:texture sourceSlice:slice sourceLevel:mip sourceOrigin:origin sourceSize:size
                    toBuffer:uploads->buffer destinationOffset:0 destinationBytesPerRow:pitch destinationBytesPerImage:pitch * count];
                [encoder endEncoding];
                [commands commit];
                [commands waitUntilCompleted];
                if (commands.status == MTLCommandBufferStatusError)
                    return rays_metal_set_error(message, commands.error.localizedDescription.UTF8String ?: "texture transfer failed");
                if (!write) for (NSUInteger i = 0; i < count; ++i)
                    memcpy(bytes + plane * source_image + (row + i) * source_row, scratch + i * pitch, row_bytes);
            }
        }
        return 0;
    }
}

namespace {
struct PXUIMetalUploadBatch {
    id<MTLCommandBuffer> commands;
    id<MTLBlitCommandEncoder> encoder;
    id<MTLBuffer> buffer;
    NSMutableArray<id<MTLResource>> *pinned;
    NSUInteger used;
};
}

extern "C" int32_t rays_metal_upload_batch_create(MetalHandle *command_handle, void **out, const char **message) {
    if (message) *message = nullptr;
    if (out) *out = nullptr;
    @try { @autoreleasepool {
        id<MTLCommandBuffer> commands = (__bridge id<MTLCommandBuffer>)rays_metal_handle_object(command_handle, KIND_COMMAND_BUFFER);
        if (!NSThread.isMainThread || !commands || !out || commands.status > MTLCommandBufferStatusEnqueued)
            return rays_metal_set_invalid_argument(message, "invalid recording command buffer for texture uploads");
        std::unique_ptr<PXUIMetalUploadBatch> batch(new (std::nothrow) PXUIMetalUploadBatch{});
        if (!batch) return rays_metal_set_error(message, "could not allocate a texture upload batch");
        batch->commands = commands;
        if (!commands.retainedReferences) {
            NSMutableArray<id<MTLResource>> *pinned = [NSMutableArray new];
            [commands addCompletedHandler:^(id<MTLCommandBuffer>) { [pinned removeAllObjects]; }];
            batch->pinned = pinned;
        }
        *out = batch.release();
        return 0;
    } } @catch (NSException *exception) { return rays_metal_set_error(message, exception.reason.UTF8String); }
}

extern "C" void rays_metal_upload_batch_destroy(void *raw) {
    @try { @autoreleasepool {
        std::unique_ptr<PXUIMetalUploadBatch> batch(static_cast<PXUIMetalUploadBatch *>(raw));
        [batch->encoder endEncoding];
    } } @catch (NSException *) { }
}

// The transfer bridge has checked the complete CPU span and the native texture region.
int32_t rays_metal_upload_batch_copy(void *raw, void *texture_pointer, const uint8_t *bytes,
    NSUInteger source_row, NSUInteger texel, MTLRegion region, NSUInteger mip, const char **message) {
    auto *batch = static_cast<PXUIMetalUploadBatch *>(raw);
    id<MTLTexture> texture = (__bridge id<MTLTexture>)texture_pointer;
    if (!NSThread.isMainThread || texture.device.registryID != batch->commands.device.registryID)
        return rays_metal_set_invalid_argument(message, "texture upload command buffer device mismatch");
    const NSUInteger row_bytes = region.size.width * texel;
    if (row_bytes > upload_limit)
        return rays_metal_set_unsupported(message, "texture row exceeds the 4 MiB transfer limit");
    const NSUInteger pitch = (row_bytes + 255) & ~NSUInteger(255);
    if (!batch->encoder) {
        batch->encoder = [batch->commands blitCommandEncoder];
        if (!batch->encoder) return rays_metal_set_error(message, "could not encode texture uploads");
        batch->encoder.label = @"pxui texture uploads";
    }
    if (batch->pinned && ![batch->pinned containsObject:texture]) [batch->pinned addObject:texture];
    for (NSUInteger row = 0; row < region.size.height;) {
        if (batch->buffer.length - batch->used < pitch) {
            const NSUInteger needed = std::min(region.size.height - row, upload_limit / pitch) * pitch;
            id<MTLBuffer> buffer = [texture.device newBufferWithLength:std::max(needed, NSUInteger(256 * 1024)) options:MTLResourceStorageModeShared];
            if (!buffer) return rays_metal_set_error(message, "could not allocate texture upload staging");
            batch->buffer = buffer;
            [batch->pinned addObject:buffer];
            batch->used = 0;
        }
        const NSUInteger count = std::min(region.size.height - row, (batch->buffer.length - batch->used) / pitch);
        auto *scratch = static_cast<uint8_t *>(batch->buffer.contents) + batch->used;
        for (NSUInteger i = 0; i < count; ++i)
            memcpy(scratch + i * pitch, bytes + (row + i) * source_row, row_bytes);
        [batch->encoder copyFromBuffer:batch->buffer sourceOffset:batch->used sourceBytesPerRow:pitch
            sourceBytesPerImage:pitch * count sourceSize:MTLSizeMake(region.size.width, count, 1)
            toTexture:texture destinationSlice:0 destinationLevel:mip
            destinationOrigin:MTLOriginMake(region.origin.x, region.origin.y + row, 0)];
        batch->used += pitch * count;
        row += count;
    }
    return 0;
}
