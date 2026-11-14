#import <Foundation/Foundation.h>
#import <TargetConditionals.h>
#include <stdlib.h>
#include <string.h>
#include <stdatomic.h>
#include <stdint.h>
#ifdef PXUI_FILE_TEST_UTIL
static atomic_uintptr_t lastBookmarkURL;
static atomic_uint bookmarkCalls, scopeStarts, scopeStops;
#endif

size_t pxui_selection_count(void *raw) {
    @autoreleasepool { id batch = (__bridge id)raw; return [batch isKindOfClass:NSArray.class] ? [batch count] : 0; }
}
void *pxui_selection_url(void *raw, size_t index) {
    @autoreleasepool {
        NSArray *batch = (__bridge NSArray *)raw;
        if (index >= batch.count) return NULL;
        id url = batch[index];
        if (![url isKindOfClass:NSURL.class] || ![url isFileURL]) return NULL;
        return (__bridge_retained void *)url;
    }
}
char *pxui_selection_path(void *raw) {
    @autoreleasepool {
        NSURL *url = (__bridge NSURL *)raw;
        NSString *path = url.path;
        if (!url.isFileURL || !path.isAbsolutePath || !path.UTF8String ||
            [path rangeOfString:[NSString stringWithCharacters:(unichar[]){0} length:1]].location != NSNotFound) return NULL;
        return strdup(path.UTF8String);
    }
}
bool pxui_selection_bookmark(void *raw, unsigned char **out, size_t *len, char **error) {
    @autoreleasepool {
        if (NSThread.isMainThread) { *error = strdup("Create picker bookmarks on a worker thread"); return false; }
        NSURL *url = (__bridge NSURL *)raw;
        BOOL scoped = [url startAccessingSecurityScopedResource];
#ifdef PXUI_FILE_TEST_UTIL
        atomic_fetch_add(&bookmarkCalls, 1);
        if (scoped) atomic_fetch_add(&scopeStarts, 1);
#endif
        NSError *failure = nil;
#if TARGET_OS_OSX
        NSURLBookmarkCreationOptions options = NSURLBookmarkCreationWithSecurityScope;
#else
        NSURLBookmarkCreationOptions options = NSURLBookmarkCreationMinimalBookmark;
#endif
        NSData *data = nil;
        @try {
#ifdef PXUI_FILE_TEST_UTIL
            atomic_store(&lastBookmarkURL, (uintptr_t)(__bridge void *)url);
#endif
            data = [url bookmarkDataWithOptions:options includingResourceValuesForKeys:nil relativeToURL:nil error:&failure];
        }
        @catch (NSException *exception) { failure = [NSError errorWithDomain:@"PXUIFileAccess" code:1 userInfo:@{NSLocalizedDescriptionKey:exception.reason ?: @"Bookmark creation failed"}]; }
        @finally {
            if (scoped) {
                [url stopAccessingSecurityScopedResource];
#ifdef PXUI_FILE_TEST_UTIL
                atomic_fetch_add(&scopeStops, 1);
#endif
            }
        }
        if (!data || data.length == 0 || data.length > 64 * 1024 * 1024) {
            *error = strdup((failure.localizedDescription ?: @"Could not create the selected URL bookmark").UTF8String);
            return false;
        }
        *out = malloc(data.length);
        if (!*out) { *error = strdup("Could not allocate bookmark data"); return false; }
        memcpy(*out, data.bytes, data.length);
        *len = data.length;
        return true;
    }
}
void pxui_selection_release(void *raw) { @autoreleasepool { id value = (__bridge_transfer id)raw; (void)value; } }
void pxui_selection_free(void *raw) { free(raw); }

#ifdef PXUI_FILE_TEST_UTIL
void *pxui_selection_test_batch(const char *path, uintptr_t *identity) {
    @autoreleasepool {
        NSURL *url = [NSURL fileURLWithPath:[NSString stringWithUTF8String:path]];
        *identity = (uintptr_t)(__bridge void *)url;
        return (__bridge_retained void *)@[url];
    }
}
uintptr_t pxui_selection_test_status(unsigned int *calls, unsigned int *starts, unsigned int *stops) {
    *calls = atomic_load(&bookmarkCalls); *starts = atomic_load(&scopeStarts); *stops = atomic_load(&scopeStops);
    return atomic_load(&lastBookmarkURL);
}
#endif
