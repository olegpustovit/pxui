#include "metal_handle.h"
#include "metal_kinds.h"
#include <Metal/Metal.h>

extern "C" int32_t rays_metal_render_encoder_restart(MetalHandle *commands,
    MetalHandle *pass, MetalHandle *encoder, const char **message) {
  if (message) *message = nullptr;
  @try {
    @autoreleasepool {
      id<MTLCommandBuffer> buffer = (__bridge id<MTLCommandBuffer>)rays_metal_handle_object(commands, KIND_COMMAND_BUFFER);
      MTLRenderPassDescriptor *descriptor = (__bridge MTLRenderPassDescriptor *)rays_metal_handle_object(pass, KIND_RENDER_PASS_DESCRIPTOR);
      if (!buffer || !descriptor || !rays_metal_handle_object(encoder, KIND_RENDER_ENCODER))
        return rays_metal_set_invalid_argument(message, "invalid reusable render pass");
      id<MTLRenderCommandEncoder> next = [buffer renderCommandEncoderWithDescriptor:descriptor];
      if (!next) return rays_metal_set_error(message, "Metal returned no render encoder");
      if (rays_metal_handle_rebind(encoder, (__bridge void *)next, KIND_RENDER_ENCODER) != 0) {
        [next endEncoding];
        return rays_metal_set_invalid_argument(message, "render encoder could not be rebound");
      }
      return 0;
    }
  } @catch (NSException *exception) { return rays_metal_set_error(message, exception.reason.UTF8String); }
}
