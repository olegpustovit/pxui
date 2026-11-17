#import <AppKit/AppKit.h>
#import <QuartzCore/CALayer.h>
#import <QuartzCore/CAMetalLayer.h>
#import <objc/runtime.h>
#include <math.h>
#include "chrome.h"

static char backgroundKey;

static void installBackground(NSWindow *window, CALayer *layer) {
    if (!layer.opaque || ![layer isKindOfClass:CAMetalLayer.class] || objc_getAssociatedObject(window, &backgroundKey)) return;
    NSLock *lock = [NSLock new];
    __block NSColor *current = window.backgroundColor ?: NSColor.windowBackgroundColor;
    void (^update)(NSColor *) = ^(NSColor *color) { [lock lock]; current = color; [lock unlock]; };
    NSColor *dynamic = [NSColor colorWithName:nil dynamicProvider:^NSColor *(NSAppearance *appearance) {
        (void)appearance;
        [lock lock]; NSColor *color = current; [lock unlock];
        return color;
    }];
    objc_setAssociatedObject(window, &backgroundKey, update, OBJC_ASSOCIATION_COPY_NONATOMIC);
    // Opaque Metal paints the full content; keep fallback colour current without invalidating every native view.
    window.backgroundColor = dynamic;
}

int pxui_sdl3_chrome_update_background(void *rawWindow, void *rawColor) {
    if (!NSThread.isMainThread || !rawWindow || !rawColor) return 0;
    NSWindow *window = (__bridge NSWindow *)rawWindow;
    void (^update)(NSColor *) = objc_getAssociatedObject(window, &backgroundKey);
    if (!update) return 0;
    update((__bridge NSColor *)rawColor);
    return 1;
}

static NSView *windowView(void *raw) {
    if (!NSThread.isMainThread || !raw) return nil;
    id delegate = ((__bridge CALayer *)raw).delegate;
    return [delegate isKindOfClass:NSView.class] ? delegate : nil;
}

int pxui_sdl3_chrome_configure(void *raw, double minimum_width, double minimum_height) {
    @autoreleasepool {
        NSView *view = windowView(raw);
        NSWindow *window = view.window;
        if (!window || !isfinite(minimum_width) || !isfinite(minimum_height) || minimum_width <= 0 || minimum_height <= 0) return 0;
        window.styleMask |= NSWindowStyleMaskFullSizeContentView;
        window.titlebarAppearsTransparent = YES;
        window.titleVisibility = NSWindowTitleHidden;
        window.titlebarSeparatorStyle = NSTitlebarSeparatorStyleNone;
        window.collectionBehavior = (window.collectionBehavior & ~NSWindowCollectionBehaviorFullScreenAuxiliary) | NSWindowCollectionBehaviorFullScreenPrimary;
        window.contentMinSize = NSMakeSize(minimum_width, minimum_height);
        if (view.layer == (__bridge CALayer *)raw
            && (view == window.contentView || view.superview == window.contentView)
            && NSContainsRect([view convertRect:view.bounds toView:window.contentView], window.contentView.bounds)) {
            installBackground(window, (__bridge CALayer *)raw);
        }
        return 1;
    }
}

int pxui_sdl3_chrome_read(void *raw, PxuiSDL3ChromeState *state) {
    @autoreleasepool {
        NSView *view = windowView(raw);
        NSWindow *window = view.window;
        if (!window || !state) return 0;
        NSRect layout = [view convertRect:window.contentLayoutRect fromView:nil];
        double top = view.isFlipped ? NSMinY(layout) - NSMinY(view.bounds) : NSMaxY(view.bounds) - NSMaxY(layout);
        *state = (PxuiSDL3ChromeState){ MAX(0, top), NSWidth(view.bounds), NSHeight(view.bounds), (window.styleMask & NSWindowStyleMaskResizable) != 0, [window standardWindowButton:NSWindowZoomButton].enabled, (window.styleMask & NSWindowStyleMaskFullSizeContentView) != 0, (window.collectionBehavior & NSWindowCollectionBehaviorFullScreenPrimary) != 0 };
        return 1;
    }
}

#ifdef PXUI_SDL3_TEST_UTIL
int pxui_sdl3_chrome_background_state(void *raw, double rgb[3]) {
    @autoreleasepool {
        NSWindow *window = windowView(raw).window;
        NSColor *color = [window.backgroundColor colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
        if (!window || !color || !rgb) return -1;
        rgb[0] = color.redComponent; rgb[1] = color.greenComponent; rgb[2] = color.blueComponent;
        return objc_getAssociatedObject(window, &backgroundKey) ? 1 : 0;
    }
}
static BOOL fallbackPixel(NSColor *color, double red, double green, double blue) {
    unsigned char pixel[4] = {0};
    CGColorSpaceRef space = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
    CGContextRef context = CGBitmapContextCreate(pixel, 1, 1, 8, 4, space, kCGImageAlphaPremultipliedLast | kCGBitmapByteOrder32Big);
    if (!context) { CGColorSpaceRelease(space); return NO; }
    CGContextSetFillColorWithColor(context, color.CGColor);
    CGContextFillRect(context, CGRectMake(0, 0, 1, 1));
    CGContextRelease(context);
    CGColorSpaceRelease(space);
    return abs((int)pixel[0] - (int)lround(red * 255)) <= 1
        && abs((int)pixel[1] - (int)lround(green * 255)) <= 1
        && abs((int)pixel[2] - (int)lround(blue * 255)) <= 1 && pixel[3] == 255;
}

static NSColor *fallbackViewPixel(NSWindow *window) {
    NSView *frame = window.contentView.superview;
    NSRect region = NSMakeRect(40, 40, 1, 1);
    NSBitmapImageRep *image = [frame bitmapImageRepForCachingDisplayInRect:region];
    if (!image) return nil;
    [frame cacheDisplayInRect:region toBitmapImageRep:image];
    return [[image colorAtX:0 y:0] colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
}

int pxui_sdl3_chrome_background_check(uint64_t (*clock)(void), uint64_t times[2]) {
    if (!NSThread.isMainThread) return 0;
    __weak id weakColor = nil, weakUpdate = nil;
    @autoreleasepool {
        [NSApplication sharedApplication];
        NSWindow *window = [[NSWindow alloc] initWithContentRect:NSMakeRect(0, 0, 100, 100)
            styleMask:NSWindowStyleMaskTitled | NSWindowStyleMaskFullSizeContentView
            backing:NSBackingStoreBuffered defer:YES];
        window.releasedWhenClosed = NO;
        window.titlebarAppearsTransparent = YES;
        NSColor *red = [NSColor colorWithSRGBRed:1 green:0 blue:0 alpha:1];
        if (pxui_sdl3_chrome_update_background((__bridge void *)window, (__bridge void *)red)) return 0;
        NSColor *blue = [NSColor colorWithSRGBRed:0 green:0 blue:1 alpha:1];
        for (unsigned i=0; i<10; i++) window.backgroundColor = i%2 ? red : blue;
        uint64_t begin = clock();
        for (unsigned i=0; i<100; i++) window.backgroundColor = i%2 ? red : blue;
        uint64_t end = clock();
        if (!begin || begin == UINT64_MAX || end == UINT64_MAX || end < begin) return 0;
        times[0] = end - begin;
        CAMetalLayer *layer = [CAMetalLayer layer];
        layer.opaque = NO;
        installBackground(window, layer);
        if (objc_getAssociatedObject(window, &backgroundKey)) return 0;
        CALayer *ordinary = [CALayer layer];
        ordinary.opaque = YES;
        installBackground(window, ordinary);
        if (objc_getAssociatedObject(window, &backgroundKey)) return 0;
        layer.opaque = YES;
        NSView *partial = [[NSView alloc] initWithFrame:NSMakeRect(0, 0, 10, 10)];
        partial.wantsLayer = YES;
        partial.layer = layer;
        [window.contentView addSubview:partial];
        pxui_sdl3_chrome_configure((__bridge void *)layer, 10, 10);
        if (objc_getAssociatedObject(window, &backgroundKey)) return 0;
        [partial removeFromSuperview];
        partial.layer = nil;
        installBackground(window, layer);
        id installed = window.backgroundColor;
        id update = objc_getAssociatedObject(window, &backgroundKey);
        if (!installed || !update) return 0;
        weakColor = installed; weakUpdate = update;
        installBackground(window, layer);
        if (window.backgroundColor != installed || objc_getAssociatedObject(window, &backgroundKey) != update) return 0;
        const double colors[][3] = {{0.9,0.9,0.9},{0.08,0.08,0.08},{1,0,0},{0,1,0},{0,0,1},{0.1,0.2,0.3},{0.9,0.1,0.7},{0.4,0.6,0.2}};
        __block BOOL passed = YES;
        for (unsigned i=0; i<8; i++) {
            const double *c = colors[i];
            window.viewsNeedDisplay = NO;
            NSColor *next = [NSColor colorWithSRGBRed:c[0] green:c[1] blue:c[2] alpha:1];
            passed &= pxui_sdl3_chrome_update_background((__bridge void *)window, (__bridge void *)next) != 0;
            passed &= !window.viewsNeedDisplay;
            for (NSAppearanceName name in @[NSAppearanceNameAqua, NSAppearanceNameDarkAqua]) {
                [[NSAppearance appearanceNamed:name] performAsCurrentDrawingAppearance:^{
                    if (!fallbackPixel(window.backgroundColor,c[0],c[1],c[2])) passed = NO;
                }];
            }
        }
        begin = clock();
        for (unsigned i=0; i<100; i++) {
            NSColor *next = i%2 ? red : blue;
            passed &= pxui_sdl3_chrome_update_background((__bridge void *)window, (__bridge void *)next) != 0;
        }
        end = clock();
        if (!begin || begin == UINT64_MAX || end == UINT64_MAX || end < begin) return 0;
        times[1] = end - begin;
        NSColor *green = [NSColor colorWithSRGBRed:0.1 green:0.7 blue:0.2 alpha:1];
        passed &= pxui_sdl3_chrome_update_background((__bridge void *)window, (__bridge void *)green) != 0;
        [window setContentSize:NSMakeSize(120, 110)];
        NSColor *optimized = fallbackViewPixel(window);
        window.backgroundColor = green;
        NSColor *baseline = fallbackViewPixel(window);
        passed &= optimized && baseline
            && baseline.greenComponent > baseline.redComponent && baseline.greenComponent > baseline.blueComponent
            && fabs(optimized.redComponent-baseline.redComponent) < 0.01
            && fabs(optimized.greenComponent-baseline.greenComponent) < 0.01
            && fabs(optimized.blueComponent-baseline.blueComponent) < 0.01
            && fabs(optimized.alphaComponent-baseline.alphaComponent) < 0.01;
        [window close];
        if (!passed) return 0;
    }
    return !weakColor && !weakUpdate;
}
#endif
