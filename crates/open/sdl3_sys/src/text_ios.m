#import <UIKit/UIKit.h>
#include <stdlib.h>
#include <string.h>
#include "text_ios.h"
#include "ios.h"

#define TEXT_OBSERVER "pxui.ios.text.observer"
enum { TEXT_SLOTS = 64, TEXT_BYTES = 1024 * 1024 };
typedef struct { char *text; size_t bytes; uint64_t token; uint32_t window; int start, length; } TextSlot;
static TextSlot slots[TEXT_SLOTS];
static size_t held_bytes;
static uint64_t next_token;
static Uint32 text_event;
static unsigned observers;

static void release_slot(TextSlot *slot) {
    if (slot->text) { held_bytes -= slot->bytes; free(slot->text); slot->text = NULL; }
}

static UITextField *focused_field(UIWindow *window) {
    UIViewController *controller = window.rootViewController;
    for (UIView *child in controller.view.subviews) {
        if ([child isKindOfClass:UITextField.class] && child.isFirstResponder) {
            UITextField *field = (UITextField *)child;
            if (field.delegate == (id)controller) return field;
        }
    }
    return nil;
}

static NSUInteger scalar_count(NSString *text, NSUInteger end) {
    NSUInteger count = 0;
    for (NSUInteger i = 0; i < end; ++i, ++count) {
        unichar c = [text characterAtIndex:i];
        if (CFStringIsSurrogateHighCharacter(c) && i + 1 < end && CFStringIsSurrogateLowCharacter([text characterAtIndex:i + 1])) ++i;
    }
    return count;
}

@interface PXUITextInputObserver : NSObject {
@public
    uint32_t windowID, action;
    const char *failure;
    __weak UIWindow *window;
    __weak UITextField *markedField;
    NSString *marked;
    int markedStart, markedLength;
    BOOL counted;
}
- (instancetype)initWithWindow:(SDL_Window *)native;
- (void)changed:(NSNotification *)notification;
- (void)ended:(NSNotification *)notification;
- (void)began:(NSNotification *)notification;
- (void)applyAction:(UITextField *)field;
- (void)publish:(NSString *)text start:(int)start length:(int)length;
@end

@implementation PXUITextInputObserver
- (instancetype)initWithWindow:(SDL_Window *)native {
    if ((self = [super init])) {
        windowID = SDL_GetWindowID(native);
        window = (__bridge UIWindow *)SDL_GetPointerProperty(SDL_GetWindowProperties(native), SDL_PROP_WINDOW_UIKIT_WINDOW_POINTER, NULL);
        if (!window) return nil;
        if (!text_event) {
            text_event = SDL_RegisterEvents(1);
            if (!text_event) return nil;
        }
        ++observers;
        counted = YES;
        NSNotificationCenter *center = NSNotificationCenter.defaultCenter;
        [center addObserver:self selector:@selector(changed:) name:UITextFieldTextDidChangeNotification object:nil];
        [center addObserver:self selector:@selector(ended:) name:UITextFieldTextDidEndEditingNotification object:nil];
        [center addObserver:self selector:@selector(began:) name:UITextFieldTextDidBeginEditingNotification object:nil];
    }
    return self;
}
- (void)dealloc {
    [NSNotificationCenter.defaultCenter removeObserver:self];
    for (unsigned i = 0; i < TEXT_SLOTS; ++i) if (slots[i].window == windowID) release_slot(&slots[i]);
    if (counted && --observers == 0 && text_event) {
        SDL_FlushEvent(text_event);
        text_event = 0;
    }
}
- (void)publish:(NSString *)text start:(int)start length:(int)length {
    if (failure) return;
    size_t bytes = [text lengthOfBytesUsingEncoding:NSUTF8StringEncoding];
    unsigned index = 0;
    while (index < TEXT_SLOTS && slots[index].text) ++index;
    if (index == TEXT_SLOTS || bytes > TEXT_BYTES - held_bytes) {
        failure = "iOS marked-text queue exceeded 64 events or 1 MiB";
        pxui_sdl3_ios_wake();
        return;
    }
    const char *utf8 = text.UTF8String;
    char *copy = utf8 ? malloc(bytes + 1) : NULL;
    if (!copy) { failure = "could not copy iOS marked text"; pxui_sdl3_ios_wake(); return; }
    memcpy(copy, utf8, bytes + 1);
    TextSlot *slot = &slots[index];
    *slot = (TextSlot){copy, bytes, ++next_token, windowID, start, length};
    held_bytes += bytes;
    SDL_Event event = {0};
    event.user.type = text_event;
    event.user.windowID = windowID;
    event.user.code = (Sint32)index;
    event.user.data1 = (void *)(uintptr_t)slot->token;
    if (!SDL_PushEvent(&event)) {
        release_slot(slot);
        failure = "could not queue iOS marked text";
        pxui_sdl3_ios_wake();
    }
}
- (void)changed:(NSNotification *)notification {
    UITextField *field = notification.object;
    SDL_Window *native = SDL_GetWindowFromID(windowID);
    if (![field isKindOfClass:UITextField.class] || field != focused_field(window)
        || !native || !SDL_TextInputActive(native)) return;
    UITextRange *range = field.markedTextRange;
    if (range) {
        NSString *value = [field textInRange:range];
        UITextRange *selection = field.selectedTextRange;
        NSInteger a = selection ? [field offsetFromPosition:range.start toPosition:selection.start] : 0;
        NSInteger b = selection ? [field offsetFromPosition:range.start toPosition:selection.end] : a;
        NSUInteger start = MIN(value.length, (NSUInteger)MAX(0, a));
        NSUInteger end = MIN(value.length, (NSUInteger)MAX((NSInteger)start, b));
        int from = (int)scalar_count(value, start);
        int length = (int)(scalar_count(value, end) - (NSUInteger)from);
        if (![value isEqualToString:marked] || from != markedStart || length != markedLength) {
            [self publish:value start:from length:length];
            marked = [value copy]; markedStart = from; markedLength = length; markedField = field;
        }
    } else if (markedField == field) {
        [self publish:@"" start:0 length:0];
        marked = nil; markedField = nil;
    }
}
- (void)ended:(NSNotification *)notification {
    if (notification.object == markedField) {
        [self publish:@"" start:0 length:0];
        marked = nil; markedField = nil;
    }
}
- (void)applyAction:(UITextField *)field {
    UIReturnKeyType type = action == 1 ? UIReturnKeyDone : action == 2 ? UIReturnKeySearch : UIReturnKeyDefault;
    if (field && field.returnKeyType != type) { field.returnKeyType = type; [field reloadInputViews]; }
}
- (void)began:(NSNotification *)notification {
    UITextField *field = notification.object;
    if ([field isKindOfClass:UITextField.class] && field == focused_field(window)) [self applyAction:field];
}
@end

static void release_observer(void *unused, void *value) { (void)unused; CFRelease(value); }

bool pxui_sdl3_ios_text_prepare(SDL_Window *native, uint32_t action) {
    @autoreleasepool {
        SDL_PropertiesID props = SDL_GetWindowProperties(native);
        PXUITextInputObserver *observer = (__bridge PXUITextInputObserver *)SDL_GetPointerProperty(props, TEXT_OBSERVER, NULL);
        if (!observer) {
            observer = [[PXUITextInputObserver alloc] initWithWindow:native];
            if (!observer) return SDL_SetError("could not observe the UIKit text field");
            void *retained = (void *)CFBridgingRetain(observer);
            if (!SDL_SetPointerPropertyWithCleanup(props, TEXT_OBSERVER, retained, release_observer, NULL)) return false;
        }
        observer->action = action;
        [observer applyAction:focused_field(observer->window)];
        return true;
    }
}

bool pxui_sdl3_ios_text_status(SDL_Window *window) {
    PXUITextInputObserver *observer = (__bridge PXUITextInputObserver *)SDL_GetPointerProperty(SDL_GetWindowProperties(window), TEXT_OBSERVER, NULL);
    if (observer && observer->failure) return SDL_SetError("%s", observer->failure);
    return true;
}

void pxui_sdl3_ios_text_close(SDL_Window *window) {
    SDL_ClearProperty(SDL_GetWindowProperties(window), TEXT_OBSERVER);
}

int pxui_sdl3_ios_text_translate(const SDL_Event *source, pxui_sdl3_event *out) {
    if (!text_event || source->type != text_event) return 0;
    if (source->user.code < 0 || source->user.code >= TEXT_SLOTS) return -1;
    TextSlot *slot = &slots[source->user.code];
    if (!slot->text || slot->token != (uint64_t)(uintptr_t)source->user.data1 || slot->window != source->user.windowID) return -1;
    out->text = slot->text; out->text_len = slot->bytes; out->a = slot->start; out->b = slot->length;
    held_bytes -= slot->bytes;
    slot->text = NULL;
    return 1;
}

#ifdef PXUI_SDL3_TEST_UTIL
static bool text_queue_check(SDL_Window *native, PXUITextInputObserver *observer) {
    if (held_bytes) return false;
    SDL_Event event;
    pxui_sdl3_event out = {0}, duplicate = {0};
    @autoreleasepool { [observer publish:[NSString stringWithFormat:@"temporary %u", 19] start:2 length:3]; }
    if (SDL_PeepEvents(&event, 1, SDL_GETEVENT, text_event, text_event) != 1) return false;
    if (pxui_sdl3_ios_text_translate(&event, &out) != 1 || strcmp(out.text, "temporary 19") || out.a != 2 || out.b != 3) return false;
    free(out.text);
    if (pxui_sdl3_ios_text_translate(&event, &duplicate) != -1 || held_bytes) return false;
    [observer publish:@"identity" start:0 length:0];
    if (SDL_PeepEvents(&event, 1, SDL_GETEVENT, text_event, text_event) != 1) return false;
    void *token = event.user.data1;
    event.user.data1 = (void *)((uintptr_t)token + 1);
    if (pxui_sdl3_ios_text_translate(&event, &duplicate) != -1) return false;
    event.user.data1 = token;
    if (pxui_sdl3_ios_text_translate(&event, &out) != 1) return false;
    free(out.text);
    for (unsigned i = 0; i <= TEXT_SLOTS; ++i) [observer publish:@"x" start:0 length:0];
    if (!observer->failure || pxui_sdl3_ios_text_status(native) || held_bytes != TEXT_SLOTS) return false;
    observer->failure = NULL;
    SDL_ClearError();
    unsigned drained = 0;
    while (SDL_PeepEvents(&event, 1, SDL_GETEVENT, text_event, text_event) == 1) {
        if (pxui_sdl3_ios_text_translate(&event, &out) != 1) return false;
        free(out.text); ++drained;
    }
    if (drained != TEXT_SLOTS || held_bytes) return false;
    [observer publish:[@"x" stringByPaddingToLength:TEXT_BYTES + 1 withString:@"x" startingAtIndex:0] start:0 length:0];
    if (!observer->failure || held_bytes) return false;
    observer->failure = NULL;
    [observer publish:@"retired" start:0 length:0];
    return held_bytes == 7;
}

bool pxui_sdl3_ios_test_text_step(int step) {
    @autoreleasepool {
        SDL_Window *native = SDL_GetKeyboardFocus();
        if (!native) return false;
        PXUITextInputObserver *observer = (__bridge PXUITextInputObserver *)SDL_GetPointerProperty(SDL_GetWindowProperties(native), TEXT_OBSERVER, NULL);
        if (!observer) return false;
        if (step == 8) {
            if (!text_queue_check(native, observer)) return false;
            pxui_sdl3_ios_text_close(native);
            observer = nil;
            return held_bytes == 0 && observers == 0 && text_event == 0;
        }
        UITextField *field = focused_field(observer->window);
        UIReturnKeyType action = step == 7 ? UIReturnKeyDone : UIReturnKeySearch;
        if (!field || field.autocorrectionType != UITextAutocorrectionTypeNo
            || field.autocapitalizationType != UITextAutocapitalizationTypeNone || field.returnKeyType != action) return false;
        if (step == 9) {
            UITextField *foreign = [UITextField new];
            [observer->window.rootViewController.view addSubview:foreign];
            if (![foreign becomeFirstResponder]) return false;
            [foreign setMarkedText:@"foreign" selectedRange:NSMakeRange(7, 0)];
            [NSNotificationCenter.defaultCenter postNotificationName:UITextFieldTextDidChangeNotification object:foreign];
            bool untouched = foreign.returnKeyType == UIReturnKeyDefault && held_bytes == 0;
            [field becomeFirstResponder];
            [foreign removeFromSuperview];
            return untouched && field.isFirstResponder;
        }
        switch (step) {
        case 0: [field setMarkedText:@"ni" selectedRange:NSMakeRange(2, 0)]; break;
        case 1: [field setMarkedText:@"你😀" selectedRange:NSMakeRange(1, 2)]; break;
        case 2:
            [field setMarkedText:@"你好" selectedRange:NSMakeRange(2, 0)];
            [field unmarkText]; break;
        case 3: [field insertText:@" café 🎵"]; break;
        case 4: [field deleteBackward]; break;
        case 5: [field setMarkedText:@"cancel" selectedRange:NSMakeRange(6, 0)]; break;
        case 6:
            [field replaceRange:field.markedTextRange withText:@""];
            [field unmarkText]; break;
        case 7:
            return [field.delegate respondsToSelector:@selector(textFieldShouldReturn:)] && [field.delegate textFieldShouldReturn:field];
        default: return false;
        }
        [NSNotificationCenter.defaultCenter postNotificationName:UITextFieldTextDidChangeNotification object:field];
        return true;
    }
}
#endif
