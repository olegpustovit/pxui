#include <CoreGraphics/CoreGraphics.h>
#include <CoreText/CoreText.h>
#include <TargetConditionals.h>
#include <math.h>
#include <stdint.h>
#include <string.h>

typedef struct {
    const uint8_t *pixels;
    size_t stride;
    uint32_t width, height;
    int32_t left, top;
} GlyphMask;

// The bytes belong to the source kept by the allocator's release callback.
static void keep_bytes(void *block, void *info) {
    (void)block;
    (void)info;
}

CTFontDescriptorRef pxui_font_mask_face(const uint8_t *bytes, CFIndex length, uint32_t index,
                                        const uint8_t *name, CFIndex name_len, void *owner,
                                        CFAllocatorReleaseCallBack release) {
    CFAllocatorContext callbacks = { .info = owner, .release = release, .deallocate = keep_bytes };
    CFAllocatorRef allocator = CFAllocatorCreate(kCFAllocatorDefault, &callbacks);
    if (!allocator) {
        release(owner);
        return NULL;
    }
    CFDataRef data = CFDataCreateWithBytesNoCopy(kCFAllocatorDefault, bytes, length, allocator);
    CFRelease(allocator);
    if (!data) return NULL;

    CFArrayRef faces = CTFontManagerCreateFontDescriptorsFromData(data);
    CFRelease(data);
    if (!faces) return NULL;
    CTFontDescriptorRef face = index < CFArrayGetCount(faces) ? CFArrayGetValueAtIndex(faces, index) : NULL;
    if (face && CFArrayGetCount(faces) > 1) {
        // Glyph ids are face-local. Do not let a collection's descriptor order select the wrong face.
        CFTypeRef actual = CTFontDescriptorCopyAttribute(face, kCTFontNameAttribute);
        CFStringRef expected = CFStringCreateWithBytes(NULL, name, name_len, kCFStringEncodingUTF8, false);
        if (!actual || !expected || !CFEqual(actual, expected)) face = NULL;
        if (actual) CFRelease(actual);
        if (expected) CFRelease(expected);
    }
    if (face) CFRetain(face);
    CFRelease(faces);
    return face;
}

CTFontRef pxui_font_mask_font(CTFontDescriptorRef face, double ppem, double weight) {
    CTFontDescriptorRef variant = NULL;
    if (weight > 0) {
        int32_t tag = 0x77676874; // wght
        CFNumberRef axis = CFNumberCreate(NULL, kCFNumberSInt32Type, &tag);
        if (!axis) return NULL;
        variant = CTFontDescriptorCreateCopyWithVariation(face, axis, weight);
        CFRelease(axis);
        if (!variant) return NULL;
    }
    CTFontRef font = CTFontCreateWithFontDescriptor(variant ? variant : face, ppem, NULL);
    if (variant) CFRelease(variant);
    return font;
}

static size_t scratch_size(size_t old, size_t needed) {
    size_t result = old ? old : 64;
    while (result < needed) result *= 2;
    return result;
}

static int prepare_bitmap(CGContextRef *bitmap, size_t width, size_t height) {
    size_t w = *bitmap ? CGBitmapContextGetWidth(*bitmap) : 0;
    size_t h = *bitmap ? CGBitmapContextGetHeight(*bitmap) : 0;
    if (w >= width && h >= height) return 1;

#if TARGET_OS_OSX
    // macOS draws text with font smoothing (its stem darkening), which CoreGraphics applies only to glyphs on an opaque
    // background: black on white in gray, inverted to coverage after drawing (`invert` below)
    CGColorSpaceRef gray = CGColorSpaceCreateDeviceGray();
    CGContextRef fresh = gray ? CGBitmapContextCreate(NULL, scratch_size(w, width), scratch_size(h, height),
                                                      8, 0, gray, kCGImageAlphaNone) : NULL;
    if (gray) CGColorSpaceRelease(gray);
    if (!fresh) return 0;
    CGContextSetAllowsAntialiasing(fresh, true);
    CGContextSetShouldAntialias(fresh, true);
    CGContextSetAllowsFontSmoothing(fresh, true);
    CGContextSetShouldSmoothFonts(fresh, true);
#else
    CGContextRef fresh = CGBitmapContextCreate(NULL, scratch_size(w, width), scratch_size(h, height),
                                               8, 0, NULL, kCGImageAlphaOnly);
    if (!fresh) return 0;
    CGContextSetAllowsAntialiasing(fresh, true);
    CGContextSetShouldAntialias(fresh, true);
    CGContextSetAllowsFontSmoothing(fresh, false);
    CGContextSetShouldSmoothFonts(fresh, false);
#endif
    CGContextSetAllowsFontSubpixelPositioning(fresh, true);
    CGContextSetShouldSubpixelPositionFonts(fresh, true);
    CGContextSetAllowsFontSubpixelQuantization(fresh, true);
    CGContextSetShouldSubpixelQuantizeFonts(fresh, true);
#if TARGET_OS_OSX
    CGContextSetGrayFillColor(fresh, 0, 1);
#else
    CGContextSetGrayFillColor(fresh, 1, 1);
#endif

    if (*bitmap) CFRelease(*bitmap);
    *bitmap = fresh;
    return 1;
}

int pxui_font_mask_draw(CTFontRef font, CGContextRef *bitmap, CGGlyph glyph,
                        double phase, GlyphMask *result) {
    *result = (GlyphMask){0};
    if (glyph >= CTFontGetGlyphCount(font)) return 0;
    CGRect ink = CTFontGetBoundingRectsForGlyphs(font, kCTFontOrientationHorizontal, &glyph, NULL, 1);
    if (CGRectIsEmpty(ink)) return 1;

    // Rasterized coverage may extend beyond the outline bounds. Crop the guard pixels after drawing.
    double left = floor(CGRectGetMinX(ink) + phase) - 2;
    double top = ceil(CGRectGetMaxY(ink)) + 2;
    double width = ceil(CGRectGetMaxX(ink) + phase) + 2 - left;
    double height = top - floor(CGRectGetMinY(ink)) + 2;
    if (!(width >= 1 && width <= 2048 && height >= 1 && height <= 2048 &&
          left >= INT32_MIN && left <= INT32_MAX - 2048 && top >= INT32_MIN + 2048 && top <= INT32_MAX)) return 0;
    size_t w = (size_t)width, h = (size_t)height;
    if (!prepare_bitmap(bitmap, w, h)) return 0;
    uint8_t *pixels = CGBitmapContextGetData(*bitmap);
    size_t stride = CGBitmapContextGetBytesPerRow(*bitmap);
#if TARGET_OS_OSX
    uint8_t blank = 255;
#else
    uint8_t blank = 0;
#endif
    for (size_t y = 0; y < h; y++) memset(pixels + y * stride, blank, w);

    CGPoint pen = CGPointMake(phase - left, CGBitmapContextGetHeight(*bitmap) - top);
    CTFontDrawGlyphs(font, &glyph, &pen, 1, *bitmap);
    CGContextFlush(*bitmap);
#if TARGET_OS_OSX
    // ink is dark on white: coverage is its inverse
    for (size_t y = 0; y < h; y++) {
        for (size_t x = 0; x < w; x++) pixels[y * stride + x] = 255 - pixels[y * stride + x];
    }
#endif

    size_t x0 = w, x1 = 0, y0 = h, y1 = 0;
    for (size_t y = 0; y < h; y++) {
        for (size_t x = 0; x < w; x++) {
            if (pixels[y * stride + x] == 0) continue;
            if (x < x0) x0 = x;
            if (x + 1 > x1) x1 = x + 1;
            if (y < y0) y0 = y;
            y1 = y + 1;
        }
    }
    if (x0 < x1) {
        *result = (GlyphMask){ pixels + y0 * stride + x0, stride, (uint32_t)(x1 - x0), (uint32_t)(y1 - y0),
                              (int32_t)left + (int32_t)x0, (int32_t)top - (int32_t)y0 };
    }
    return 1;
}
