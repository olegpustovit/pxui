#import <UIKit/UIKit.h>
#import <UniformTypeIdentifiers/UniformTypeIdentifiers.h>
#include "bridge.h"
#include <stdlib.h>
#include <string.h>

@interface PXUIDocumentPicker : NSObject <UIDocumentPickerDelegate, UIAdaptivePresentationControllerDelegate>
@property(nonatomic) void *slot;
@property(nonatomic) SDL_WindowID window;
@property(nonatomic, strong) UIDocumentPickerViewController *picker;
@property(nonatomic) BOOL finished;
- (void)finish:(NSArray<NSURL *> *)urls;
@end
static NSMutableArray<PXUIDocumentPicker *> *activePickers;
#ifdef PXUI_SDL3_TEST_UTIL
static __weak UIViewController *testPickerParent;
#endif

@implementation PXUIDocumentPicker
- (void)finish:(NSArray<NSURL *> *)urls {
    if (_finished) return;
    _finished = YES;
    _picker.delegate = nil;
    _picker.presentationController.delegate = nil;
    if (urls && urls.count) {
        BOOL valid = urls.count <= 4096;
        size_t bytes = 0;
        const char **paths = valid ? calloc(urls.count + 1, sizeof(char *)) : NULL;
        if (!paths) valid = NO;
        for (NSUInteger index = 0; valid && index < urls.count; index++) {
            NSURL *url = urls[index];
            if (![url isKindOfClass:NSURL.class]) { valid = NO; break; }
            NSString *path = url.path;
            valid = url.isFileURL && path.isAbsolutePath && path.UTF8String &&
                [path rangeOfString:[NSString stringWithCharacters:(unichar[]){0} length:1]].location == NSNotFound;
            if (valid) { paths[index] = path.UTF8String; bytes += strlen(paths[index]) + 1; valid = bytes <= 1024 * 1024; }
        }
        if (valid) {
            pxui_dialog_set_urls(_slot, (__bridge_retained void *)[urls copy]);
            pxui_dialog_callback(_slot, paths, -1);
        } else {
            SDL_SetError("Invalid or oversized document selection");
            pxui_dialog_callback(_slot, NULL, -1);
        }
        free(paths);
    } else {
        const char *empty[] = {NULL};
        pxui_dialog_callback(_slot, empty, -1);
    }
    [_picker dismissViewControllerAnimated:NO completion:nil];
    [activePickers removeObjectIdenticalTo:self];
}
- (void)documentPicker:(UIDocumentPickerViewController *)controller didPickDocumentsAtURLs:(NSArray<NSURL *> *)urls { (void)controller; [self finish:urls]; }
- (void)documentPickerWasCancelled:(UIDocumentPickerViewController *)controller { (void)controller; [self finish:nil]; }
- (void)presentationControllerDidDismiss:(UIPresentationController *)controller { (void)controller; [self finish:nil]; }
@end

bool pxui_dialog_ios_show(void *slot, SDL_Window *window, pxui_dialog_kind kind,
    const SDL_DialogFileFilter *filters, int count, const char *location) {
    @autoreleasepool {
        if (!NSThread.isMainThread) return SDL_SetError("Document pickers require the main thread");
        if (kind < PXUI_DIALOG_OPEN_FILE || kind > PXUI_DIALOG_OPEN_FOLDER) return SDL_SetError("Invalid document picker kind");
        // shortcut: open-only dialogs, add source-URL export when the sharing API is implemented.
        if (kind == PXUI_DIALOG_SAVE_FILE) return SDL_SetError("iOS export requires a prepared source URL");
        UIWindow *native = (__bridge UIWindow *)SDL_GetPointerProperty(SDL_GetWindowProperties(window), SDL_PROP_WINDOW_UIKIT_WINDOW_POINTER, NULL);
        UIViewController *parent = native.rootViewController;
        if (!parent || parent.presentedViewController) return SDL_SetError("The window cannot present another document picker");
#ifdef PXUI_SDL3_TEST_UTIL
        testPickerParent = parent;
#endif
        NSMutableArray<UTType *> *types = [NSMutableArray new];
        if (kind == PXUI_DIALOG_OPEN_FOLDER) [types addObject:UTTypeFolder];
        else {
            for (int index = 0; index < count; index++) {
                NSString *pattern = [NSString stringWithUTF8String:filters[index].pattern];
                for (NSString *extension in [pattern componentsSeparatedByString:@";"]) {
                    UTType *type = [extension isEqualToString:@"*"] ? UTTypeItem : [UTType typeWithFilenameExtension:extension];
                    if (type && ![types containsObject:type]) [types addObject:type];
                }
            }
            if (!types.count) [types addObject:UTTypeItem];
        }
        UIDocumentPickerViewController *picker = [[UIDocumentPickerViewController alloc] initForOpeningContentTypes:types asCopy:NO];
        if (!picker) return SDL_SetError("Could not create the document picker");
        picker.allowsMultipleSelection = kind == PXUI_DIALOG_OPEN_FILES;
        if (location) {
            NSString *path = [NSString stringWithUTF8String:location];
            if (path.isAbsolutePath) picker.directoryURL = [NSURL fileURLWithPath:path];
        }
        PXUIDocumentPicker *handler = [PXUIDocumentPicker new];
        handler.slot = slot;
        handler.window = SDL_GetWindowID(window);
        handler.picker = picker;
        picker.delegate = handler;
        if (!activePickers) activePickers = [NSMutableArray new];
        [activePickers addObject:handler];
        picker.popoverPresentationController.sourceView = parent.view;
        picker.popoverPresentationController.sourceRect = CGRectMake(CGRectGetMidX(parent.view.bounds), CGRectGetMidY(parent.view.bounds), 1, 1);
        picker.popoverPresentationController.permittedArrowDirections = 0;
        [parent presentViewController:picker animated:YES completion:nil];
        picker.presentationController.delegate = handler;
        return true;
    }
}
void pxui_dialog_ios_close(SDL_WindowID window) {
    @autoreleasepool {
        for (PXUIDocumentPicker *handler in [activePickers copy]) if (handler.window == window) [handler finish:nil];
    }
}
void pxui_dialog_release_urls(void *raw) { @autoreleasepool { id urls = (__bridge_transfer id)raw; (void)urls; } }

#ifdef PXUI_SDL3_TEST_UTIL
bool pxui_dialog_ios_test_idle(void) {
    return !activePickers.count && (!testPickerParent || !testPickerParent.presentedViewController);
}
bool pxui_dialog_ios_test_ready(void) {
    @autoreleasepool {
        PXUIDocumentPicker *handler = activePickers.firstObject;
        return handler && handler.picker.viewIfLoaded.window && !handler.picker.isBeingPresented;
    }
}
uintptr_t pxui_dialog_ios_test_finish(int mode, const char *path) {
    @autoreleasepool {
        PXUIDocumentPicker *handler = activePickers.firstObject;
        if (!NSThread.isMainThread || !handler || !pxui_dialog_ios_test_ready()) return 0;
        if (mode == 0) {
            [handler documentPickerWasCancelled:handler.picker];
            [handler presentationControllerDidDismiss:handler.picker.presentationController];
            return 1;
        }
        NSURL *url = mode == 1 ? [NSURL fileURLWithPath:[NSString stringWithUTF8String:path]] : [NSURL URLWithString:@"https://invalid.example/music"];
        uintptr_t identity = (uintptr_t)(__bridge void *)url;
        [handler documentPicker:handler.picker didPickDocumentsAtURLs:@[url]];
        [handler documentPickerWasCancelled:handler.picker];
        return identity;
    }
}
bool pxui_dialog_ios_test_close(void) {
    @autoreleasepool {
        PXUIDocumentPicker *handler = activePickers.firstObject;
        if (!NSThread.isMainThread || !handler) return false;
        pxui_dialog_close(handler.window);
        return activePickers.count == 0;
    }
}
#endif
