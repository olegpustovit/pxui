#include <TargetConditionals.h>
#include <SDL3/SDL.h>
#include "preferences.h"
_Static_assert(sizeof(PXUIPreferencesState) == 8, "system preference layout");
#if TARGET_OS_IOS
#import <UIKit/UIKit.h>

@interface PXUIPreferences : NSObject {
@public
    PXUIPreferencesState state;
    void (*wake)(void);
    __weak UIWindow *window;
    id<UITraitChangeRegistration> traits;
    id motion;
}
- (void)refresh;
@end
@implementation PXUIPreferences
- (void)refresh {
    if (!NSThread.isMainThread) {
        __weak PXUIPreferences *owner = self;
        dispatch_async(dispatch_get_main_queue(), ^{ [owner refresh]; });
        return;
    }
    UIUserInterfaceStyle style = window.traitCollection.userInterfaceStyle;
    PXUIPreferencesState next = {style == UIUserInterfaceStyleDark ? 2u : style == UIUserInterfaceStyleLight ? 1u : 0u, UIAccessibilityIsReduceMotionEnabled() ? 1u : 0u};
    if (next.appearance != state.appearance || next.reduced_motion != state.reduced_motion) {
        state = next;
        if (wake) wake();
    }
}
- (void)dealloc {
    if (traits) [window unregisterForTraitChanges:traits];
    if (motion) [NSNotificationCenter.defaultCenter removeObserver:motion];
}
@end
void *pxui_sdl3_preferences_create(uint32_t windowID, void (*wake)(void)) {
    if (!NSThread.isMainThread) return NULL;
    SDL_Window *handle = SDL_GetWindowFromID(windowID);
    if (!handle) return NULL;
    @autoreleasepool {
        UIWindow *window = (__bridge UIWindow *)SDL_GetPointerProperty(SDL_GetWindowProperties(handle), SDL_PROP_WINDOW_UIKIT_WINDOW_POINTER, NULL);
        if (!window) return NULL;
        PXUIPreferences *owner = [PXUIPreferences new];
        if (!owner) return NULL;
        owner->window = window;
        [owner refresh];
        owner->wake = wake;
        __weak PXUIPreferences *weak = owner;
        owner->traits = [window registerForTraitChanges:@[UITraitUserInterfaceStyle.class] withHandler:^(id<UITraitEnvironment> environment, UITraitCollection *previous) {
            (void)environment; (void)previous; [weak refresh];
        }];
        owner->motion = [NSNotificationCenter.defaultCenter addObserverForName:UIAccessibilityReduceMotionStatusDidChangeNotification object:nil queue:nil usingBlock:^(NSNotification *note) { (void)note; [weak refresh]; }];
        if (!owner->traits || !owner->motion) return NULL;
        return (__bridge_retained void *)owner;
    }
}
void pxui_sdl3_preferences_read(void *raw, PXUIPreferencesState *state) { *state = ((__bridge PXUIPreferences *)raw)->state; }
void pxui_sdl3_preferences_release(void *raw) { @autoreleasepool { (void)CFBridgingRelease(raw); } }
#else
#import <AppKit/AppKit.h>

static char PXUIAppearanceContext;
@interface PXUIPreferences : NSObject {
@public
    PXUIPreferencesState state;
    void (*wake)(void);
    NSApplication *application;
    NSWorkspace *workspace;
    id displayObserver;
}
- (instancetype)initWithApplication:(NSApplication *)app workspace:(NSWorkspace *)space wake:(void (*)(void))callback;
- (void)refresh;
@end

@implementation PXUIPreferences
- (instancetype)initWithApplication:(NSApplication *)app workspace:(NSWorkspace *)space wake:(void (*)(void))callback {
    self = [super init];
    if (self) {
        application = app;
        workspace = space;
        [self refresh];
        wake = callback;
        [application addObserver:self forKeyPath:@"effectiveAppearance" options:0 context:&PXUIAppearanceContext];
        __weak PXUIPreferences *owner = self;
        displayObserver = [workspace.notificationCenter addObserverForName:NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification object:workspace queue:nil usingBlock:^(NSNotification *note) {
            (void)note;
            [owner refresh];
        }];
    }
    return self;
}
- (void)refresh {
    if (!NSThread.isMainThread) {
        __weak PXUIPreferences *owner = self;
        dispatch_async(dispatch_get_main_queue(), ^{ [owner refresh]; });
        return;
    }
    NSString *match = [application.effectiveAppearance bestMatchFromAppearancesWithNames:@[NSAppearanceNameAqua, NSAppearanceNameDarkAqua]];
    PXUIPreferencesState next = { [match isEqual:NSAppearanceNameDarkAqua] ? 2u : [match isEqual:NSAppearanceNameAqua] ? 1u : 0u, workspace.accessibilityDisplayShouldReduceMotion ? 1u : 0u };
    if (next.appearance != state.appearance || next.reduced_motion != state.reduced_motion) {
        state = next;
        if (wake) wake();
    }
}
- (void)observeValueForKeyPath:(NSString *)keyPath ofObject:(id)object change:(NSDictionary *)change context:(void *)context {
    if (context == &PXUIAppearanceContext) [self refresh];
    else [super observeValueForKeyPath:keyPath ofObject:object change:change context:context];
}
- (void)dealloc {
    [application removeObserver:self forKeyPath:@"effectiveAppearance" context:&PXUIAppearanceContext];
    [workspace.notificationCenter removeObserver:displayObserver];
}
@end

void *pxui_sdl3_preferences_create(uint32_t window, void (*wake)(void)) {
    if (!NSThread.isMainThread || !NSApp || !SDL_GetWindowFromID(window)) return NULL;
    @autoreleasepool { return (__bridge_retained void *)[[PXUIPreferences alloc] initWithApplication:NSApp workspace:NSWorkspace.sharedWorkspace wake:wake]; }
}
void pxui_sdl3_preferences_read(void *raw, PXUIPreferencesState *state) {
    *state = ((__bridge PXUIPreferences *)raw)->state;
}
void pxui_sdl3_preferences_release(void *raw) {
    @autoreleasepool { (void)CFBridgingRelease(raw); }
}

#ifdef PXUI_SDL3_TEST_UTIL
// These objects affect only this test process, never global preferences or another application instance.
@interface LWTestWorkspace : NSObject
@property BOOL accessibilityDisplayShouldReduceMotion;
@property(strong) NSNotificationCenter *notificationCenter;
@end
@implementation LWTestWorkspace
@end
static unsigned testWakes;
static void testWake(void) { ++testWakes; }
int pxui_sdl3_preferences_test_notifications(void) {
    if (!NSThread.isMainThread || !NSApp) return 0;
    @autoreleasepool {
        NSAppearance *original = NSApp.appearance;
        LWTestWorkspace *space = [LWTestWorkspace new];
        space.notificationCenter = [NSNotificationCenter new];
        PXUIPreferences *monitor = [[PXUIPreferences alloc] initWithApplication:NSApp workspace:(NSWorkspace *)space wake:testWake];
        NSApp.appearance = [NSAppearance appearanceNamed:NSAppearanceNameAqua];
        unsigned baseline = testWakes;
        BOOL passed = monitor->state.appearance == 1 && monitor->state.reduced_motion == 0;
        space.accessibilityDisplayShouldReduceMotion = YES;
        [space.notificationCenter postNotificationName:NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification object:space];
        passed &= monitor->state.reduced_motion == 1 && testWakes == baseline + 1;
        [space.notificationCenter postNotificationName:NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification object:space];
        passed &= testWakes == baseline + 1;
        space.accessibilityDisplayShouldReduceMotion = NO;
        [space.notificationCenter postNotificationName:NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification object:space];
        passed &= monitor->state.reduced_motion == 0 && testWakes == baseline + 2;
        NSApp.appearance = [NSAppearance appearanceNamed:NSAppearanceNameDarkAqua];
        passed &= monitor->state.appearance == 2 && testWakes == baseline + 3;
        monitor = nil;
        NSApp.appearance = [NSAppearance appearanceNamed:NSAppearanceNameAqua];
        [space.notificationCenter postNotificationName:NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification object:space];
        passed &= testWakes == baseline + 3;
        NSApp.appearance = original;
        return passed;
    }
}
#endif

#endif
