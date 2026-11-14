#import <AppKit/AppKit.h>
#import <QuartzCore/CALayer.h>
#include <math.h>
#include "chrome.h"

static NSView *windowView(void *raw) {
    if (!NSThread.isMainThread || !raw) return nil;
    id delegate = ((__bridge CALayer *)raw).delegate;
    return [delegate isKindOfClass:NSView.class] ? delegate : nil;
}

int rays_sdl3_chrome_configure(void *raw, double minimum_width, double minimum_height) {
    @autoreleasepool {
        NSWindow *window = windowView(raw).window;
        if (!window || !isfinite(minimum_width) || !isfinite(minimum_height) || minimum_width <= 0 || minimum_height <= 0) return 0;
        window.styleMask |= NSWindowStyleMaskFullSizeContentView;
        window.titlebarAppearsTransparent = YES;
        window.titleVisibility = NSWindowTitleHidden;
        window.titlebarSeparatorStyle = NSTitlebarSeparatorStyleNone;
        window.collectionBehavior = (window.collectionBehavior & ~NSWindowCollectionBehaviorFullScreenAuxiliary) | NSWindowCollectionBehaviorFullScreenPrimary;
        window.contentMinSize = NSMakeSize(minimum_width, minimum_height);
        return 1;
    }
}

int rays_sdl3_chrome_read(void *raw, RaysSDL3ChromeState *state) {
    @autoreleasepool {
        NSView *view = windowView(raw);
        NSWindow *window = view.window;
        if (!window || !state) return 0;
        NSRect layout = [view convertRect:window.contentLayoutRect fromView:nil];
        double top = view.isFlipped ? NSMinY(layout) - NSMinY(view.bounds) : NSMaxY(view.bounds) - NSMaxY(layout);
        *state = (RaysSDL3ChromeState){ MAX(0, top), NSWidth(view.bounds), NSHeight(view.bounds), (window.styleMask & NSWindowStyleMaskResizable) != 0, [window standardWindowButton:NSWindowZoomButton].enabled, (window.styleMask & NSWindowStyleMaskFullSizeContentView) != 0, (window.collectionBehavior & NSWindowCollectionBehaviorFullScreenPrimary) != 0 };
        return 1;
    }
}
