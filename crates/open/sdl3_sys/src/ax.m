// NSAccessibility tree for one SDL content view: one element per pxui node, kept by node id and
// updated only when pxui's accessibility revision changes. Everything runs on the main thread.
#import <AppKit/AppKit.h>
#import <QuartzCore/CALayer.h>
#include <math.h>
#include "ax.h"

enum { ActPress = 1, ActFocus = 2, ActSet = 4, ActExpand = 16, ActCollapse = 32, ActMoveUp = 128, ActMoveDown = 256 };
enum { StFocused = 1 << 1, StEnabled = 1 << 2, StChecked = 1 << 4, StMixed = 1 << 5, StCheckable = 1 << 3,
       StSelected = 1 << 7, StExpandable = 1 << 8, StExpanded = 1 << 9 };
enum { None = UINT32_MAX, QueueCapacity = 32, RoleCount = 42 };

@class PxuiAX;
@interface PxuiAXElement : NSAccessibilityElement
@property(nonatomic, weak) PxuiAX *host;
@property(nonatomic) uint64_t nodeId;
@property(nonatomic) NSRect local;
@property(nonatomic) uint32_t actions, states, sweep;
@property(nonatomic) double now, min, max, step;
@property(nonatomic) BOOL numeric;
@property(nonatomic) uint64_t contentHash;
@property(nonatomic) uint32_t parentIndex, firstChild, nextSibling;
@property(nonatomic, copy) NSString *shownName;
@property(nonatomic, strong) id shownValue;
@end
@interface PxuiAXActionElement : PxuiAXElement
@end

@interface PxuiAX : NSObject {
@public
    __weak NSView *view;
    NSMapTable<id, PxuiAXElement *> *elements; // keyed by node id + 1 as an integer, so lookups box nothing
    void **order, **previous; // unretained, parallel to the last two trees; the map owns the elements
    uint32_t orderCount, orderCapacity;
    void (*wake)(void);
    PxuiAXAction queue[QueueCapacity];
    uint32_t queued, sweep;
    uint64_t notes, focused;
}
- (void)post:(id)object name:(NSAccessibilityNotificationName)name;
- (void)push:(uint64_t)identifier action:(uint32_t)action payload:(double)payload;
@end

static BOOL isFlag(uint32_t value, uint32_t bit) { return (value & bit) != 0; }

@implementation PxuiAXElement
- (NSRect)accessibilityFrame {
    PxuiAX *host = self.host;
    NSView *v = host ? host->view : nil;
    NSWindow *window = v.window;
    if (!window) return NSZeroRect;
    NSRect r = self.local, b = v.bounds;
    r.origin.x += NSMinX(b);
    r.origin.y = v.isFlipped ? r.origin.y + NSMinY(b) : NSMaxY(b) - r.origin.y - r.size.height;
    return [window convertRectToScreen:[v convertRect:r toView:nil]];
}
- (BOOL)isAccessibilityFocused { return isFlag(self.states, StFocused); }
- (void)setAccessibilityFocused:(BOOL)focused {
    if (focused && isFlag(self.actions, ActFocus)) [self.host push:self.nodeId action:ActFocus payload:0];
}
- (void)storeValue:(id)value { [super setAccessibilityValue:value]; }
// A reorderable list row's Move up and Move down, built when a client asks (never per frame).
- (NSArray<NSAccessibilityCustomAction *> *)accessibilityCustomActions {
    if (!isFlag(self.actions, ActMoveUp | ActMoveDown)) return nil;
    NSMutableArray<NSAccessibilityCustomAction *> *list = [NSMutableArray arrayWithCapacity:2];
    __weak PxuiAXElement *weakSelf = self;
    uint32_t acts[2] = { ActMoveUp, ActMoveDown };
    NSString *names[2] = { @"Move up", @"Move down" };
    for (int k = 0; k < 2; k++) {
        uint32_t act = acts[k];
        if (!isFlag(self.actions, act)) continue;
        [list addObject:[[NSAccessibilityCustomAction alloc] initWithName:names[k] handler:^BOOL {
            PxuiAXElement *element = weakSelf;
            if (!element) return NO;
            [element.host push:element.nodeId action:act payload:0];
            return YES;
        }]];
    }
    return list;
}
@end

@implementation PxuiAXActionElement
- (BOOL)accessibilityPerformPress {
    uint32_t action = isFlag(self.actions, ActPress) ? ActPress
        : isFlag(self.actions, ActExpand | ActCollapse) ? (isFlag(self.states, StExpanded) ? ActCollapse : ActExpand) : 0;
    if (action && action != ActPress && !isFlag(self.actions, action)) action = 0;
    if (action) [self.host push:self.nodeId action:action payload:0];
    return action != 0;
}
- (BOOL)accessibilityPerformShowMenu {
    if (!isFlag(self.actions, ActExpand) || isFlag(self.states, StExpanded)) return NO;
    [self.host push:self.nodeId action:ActExpand payload:0];
    return YES;
}
- (BOOL)stepBy:(double)direction {
    if (!isFlag(self.actions, ActSet) || !self.numeric) return NO;
    double step = self.step > 0 ? self.step : (self.max > self.min ? (self.max - self.min) / 10 : 1);
    [self setValueNumber:self.now + direction * step];
    return YES;
}
- (void)setValueNumber:(double)value {
    if (!isfinite(value)) return;
    if (self.max > self.min) value = fmin(fmax(value, self.min), self.max);
    [self.host push:self.nodeId action:ActSet payload:value];
}
- (BOOL)accessibilityPerformIncrement { return [self stepBy:1]; }
- (BOOL)accessibilityPerformDecrement { return [self stepBy:-1]; }
- (void)setAccessibilityValue:(id)value {
    if (isFlag(self.actions, ActSet) && self.numeric && [value isKindOfClass:NSNumber.class]) [self setValueNumber:[value doubleValue]];
}
@end

@implementation PxuiAX
- (void)post:(id)object name:(NSAccessibilityNotificationName)name {
    notes++;
    NSAccessibilityPostNotification(object, name);
}
- (void)push:(uint64_t)identifier action:(uint32_t)action payload:(double)payload {
    if (queued == QueueCapacity) return; // a client that outruns one frame loses the newest request
    queue[queued++] = (PxuiAXAction){ identifier, action, 0, payload };
    if (wake) wake();
}
@end

static NSAccessibilityRole roleOf(uint32_t role, NSAccessibilitySubrole *subrole) {
    *subrole = nil;
    switch (role) {
    case 1: case 2: case 3: case 4: case 29: case 34: case 40: return NSAccessibilityGroupRole;
    case 5: case 7: case 26: return NSAccessibilityStaticTextRole;
    case 6: return @"AXHeading";
    case 8: return NSAccessibilityLinkRole;
    case 9: return NSAccessibilityButtonRole;
    case 10: *subrole = NSAccessibilityToggleSubrole; return NSAccessibilityCheckBoxRole;
    case 11: return NSAccessibilityCheckBoxRole;
    case 12: return NSAccessibilityRadioButtonRole;
    case 13: return NSAccessibilitySliderRole;
    case 14: return NSAccessibilityIncrementorRole;
    case 15: return NSAccessibilityProgressIndicatorRole;
    case 16: return NSAccessibilityTextFieldRole;
    case 17: return NSAccessibilityTextAreaRole;
    case 18: return NSAccessibilityComboBoxRole;
    case 19: return NSAccessibilityListRole;
    case 20: case 24: return NSAccessibilityRowRole;
    case 21: return NSAccessibilityOutlineRole;
    case 22: *subrole = NSAccessibilityOutlineRowSubrole; return NSAccessibilityRowRole;
    case 23: return NSAccessibilityTableRole;
    case 25: return NSAccessibilityCellRole;
    case 27: return NSAccessibilityTabGroupRole;
    case 28: *subrole = NSAccessibilityTabButtonSubrole; return NSAccessibilityRadioButtonRole;
    case 30: return NSAccessibilityMenuBarRole;
    case 31: return NSAccessibilityMenuRole;
    case 32: return NSAccessibilityMenuItemRole;
    case 33: return NSAccessibilityToolbarRole;
    case 35: return NSAccessibilityHelpTagRole;
    case 36: return NSAccessibilityScrollAreaRole;
    case 37: return NSAccessibilityScrollBarRole;
    case 38: return NSAccessibilitySplitGroupRole;
    case 39: return NSAccessibilityImageRole;
    default: return NSAccessibilityUnknownRole;
    }
}

static NSView *windowView(void *raw) {
    if (!NSThread.isMainThread || !raw) return nil;
    id delegate = ((__bridge CALayer *)raw).delegate;
    return [delegate isKindOfClass:NSView.class] ? delegate : nil;
}

void *pxui_sdl3_ax_open(void *layer, void (*wake)(void)) {
    @autoreleasepool {
        NSView *view = windowView(layer);
        if (!view) return NULL;
        PxuiAX *host = [PxuiAX new];
        host->view = view;
        host->elements = [NSMapTable mapTableWithKeyOptions:NSPointerFunctionsOpaqueMemory | NSPointerFunctionsIntegerPersonality valueOptions:NSPointerFunctionsStrongMemory];
        host->wake = wake;
        return (void *)CFBridgingRetain(host);
    }
}

int pxui_sdl3_ax_voiceover(void) { return NSThread.isMainThread && NSWorkspace.sharedWorkspace.voiceOverEnabled; }

void pxui_sdl3_ax_clear(void *raw) {
    @autoreleasepool {
        PxuiAX *host = (__bridge PxuiAX *)raw;
        if (host->elements.count) [host->view setAccessibilityChildren:nil];
        for (PxuiAXElement *element in host->elements.objectEnumerator) { element.accessibilityChildren = nil; element.accessibilityParent = nil; }
        [host->elements removeAllObjects];
        host->orderCount = 0;
        host->focused = 0;
    }
}

void pxui_sdl3_ax_close(void *raw) {
    pxui_sdl3_ax_clear(raw);
    PxuiAX *host = (__bridge PxuiAX *)raw;
    free(host->order);
    free(host->previous);
    CFRelease(raw);
}

uint32_t pxui_sdl3_ax_take(void *raw, PxuiAXAction *out, uint32_t capacity) {
    PxuiAX *host = (__bridge PxuiAX *)raw;
    uint32_t n = MIN(capacity, host->queued);
    memcpy(out, host->queue, n * sizeof *out);
    memmove(host->queue, host->queue + n, (host->queued - n) * sizeof *out);
    host->queued -= n;
    return n;
}

uint64_t pxui_sdl3_ax_notifications(void *raw) { return ((__bridge PxuiAX *)raw)->notes; }

static BOOL valid(const PxuiAXNode *nodes, uint32_t n, uint32_t length, uint32_t first_top) {
    BOOL top = first_top == None || first_top < n;
    for (uint32_t i = 0; i < n && top; i++) {
        const PxuiAXNode *x = &nodes[i];
        uint32_t ranges[3][2] = { { x->name_start, x->name_end }, { x->description_start, x->description_end }, { x->value.text_start, x->value.text_end } };
        for (int k = 0; k < 3; k++) {
            if (k == 2 && x->value.tag != 1) continue;
            top &= ranges[k][0] <= ranges[k][1] && ranges[k][1] <= length;
        }
        top &= (x->parent == None || x->parent < i) && (x->first_child == None || x->first_child < n) && (x->next_sibling == None || x->next_sibling < n);
    }
    return top;
}

static NSString *string(const char *strings, uint32_t start, uint32_t end) {
    return [[NSString alloc] initWithBytes:strings + start length:end - start encoding:NSUTF8StringEncoding] ?: @"";
}

static BOOL same(id a, id b) { return a == b || [a isEqual:b]; }

static uint64_t mix(uint64_t h, const void *p, size_t n) {
    for (const unsigned char *b = p; n--; b++) h = (h ^ *b) * 0x100000001b3ull;
    return h;
}

// Everything the element shows except its rect and links, with text by content (ranges shift when other text changes).
static uint64_t contentHash(const PxuiAXNode *x, const char *strings) {
    uint64_t h = 0xcbf29ce484222325ull;
    h = mix(h, &x->id, sizeof x->id);
    h = mix(h, &x->role, sizeof x->role);
    h = mix(h, &x->states, 2 * sizeof x->states); // states, actions
    h = mix(h, &x->value.tag, sizeof x->value.tag);
    h = mix(h, &x->value.now, 4 * sizeof(double));
    h = mix(h, strings + x->name_start, x->name_end - x->name_start);
    h = mix(h, &x->name_end, 0);
    h = mix(h, strings + x->description_start, x->description_end - x->description_start);
    if (x->value.tag == 1) h = mix(h, strings + x->value.text_start, x->value.text_end - x->value.text_start);
    uint32_t lengths[3] = { x->name_end - x->name_start, x->description_end - x->description_start, x->value.text_end - x->value.text_start };
    return mix(h, lengths, sizeof lengths);
}

static PxuiAXElement *elementAt(PxuiAX *host, uint32_t i) { return (__bridge PxuiAXElement *)host->order[i]; }

// An unchanged node costs one lookup and one hash: no ObjC objects are created for it, and a
// tree whose shape did not change leaves the children arrays and the view alone.
int pxui_sdl3_ax_sync(void *raw, const PxuiAXNode *nodes, uint32_t n, const char *strings, uint32_t length, uint32_t first_top) {
    @autoreleasepool {
        PxuiAX *host = (__bridge PxuiAX *)raw;
        NSView *view = host->view;
        if (!NSThread.isMainThread || !view || (n && (!nodes || !strings)) || !valid(nodes, n, length, first_top)) return 0;
        if (n > host->orderCapacity) {
            uint32_t capacity = MAX(n, host->orderCapacity * 2);
            void **order = realloc(host->order, capacity * sizeof(void *)), **previous = realloc(host->previous, capacity * sizeof(void *));
            if (order) host->order = order;
            if (previous) host->previous = previous;
            if (!order || !previous) return 0;
            host->orderCapacity = capacity;
        }
        void **swap = host->previous; host->previous = host->order; host->order = swap;
        uint32_t previousCount = host->orderCount;
        uint32_t sweep = ++host->sweep;
        BOOL structure = n != previousCount;
        NSMutableArray<PxuiAXElement *> *values = nil, *names = nil;
        PxuiAXElement *focus = nil;
        for (uint32_t i = 0; i < n; i++) {
            const PxuiAXNode *x = &nodes[i];
            const void *key = (const void *)(uintptr_t)(x->id + 1);
            Class wanted = isFlag(x->actions, ActPress | ActSet | ActExpand | ActCollapse) ? PxuiAXActionElement.class : PxuiAXElement.class;
            PxuiAXElement *element = (__bridge PxuiAXElement *)NSMapGet(host->elements, key);
            if (element && element.class != wanted) { [host post:element name:NSAccessibilityUIElementDestroyedNotification]; element = nil; }
            if (!element) {
                element = [wanted new];
                element.host = host;
                element.nodeId = x->id;
                element.accessibilityElement = x->role != 0 && x->role != 41;
                element.parentIndex = element.firstChild = element.nextSibling = None - 1;
                NSMapInsert(host->elements, key, (__bridge const void *)element);
            }
            host->order[i] = (__bridge void *)element;
            structure |= i >= previousCount || host->previous[i] != host->order[i];
            element.sweep = sweep;
            element.local = NSMakeRect(x->rect[0], x->rect[1], x->rect[2], x->rect[3]);
            if (element.parentIndex != x->parent || element.firstChild != x->first_child || element.nextSibling != x->next_sibling) {
                element.parentIndex = x->parent; element.firstChild = x->first_child; element.nextSibling = x->next_sibling;
                structure = YES;
            }
            if (isFlag(x->states, StFocused)) focus = element;
            uint64_t hash = contentHash(x, strings);
            if (hash == element.contentHash) continue;
            element.contentHash = hash;
            NSAccessibilitySubrole sub;
            element.accessibilityRole = roleOf(x->role, &sub);
            element.accessibilitySubrole = sub;
            element.actions = x->actions;
            BOOL selection = isFlag(x->states, StSelected) != isFlag(element.states, StSelected);
            element.states = x->states;
            element.accessibilityEnabled = isFlag(x->states, StEnabled);
            element.accessibilitySelected = isFlag(x->states, StSelected);
            if (isFlag(x->states, StExpandable)) element.accessibilityExpanded = isFlag(x->states, StExpanded);
            if (selection && element.shownName) [host post:element name:NSAccessibilitySelectedChildrenChangedNotification];
            if (x->description_end > x->description_start) element.accessibilityHelp = string(strings, x->description_start, x->description_end);
            NSString *name = string(strings, x->name_start, x->name_end);
            if (element.shownName && !same(name, element.shownName)) (names = names ?: [NSMutableArray new], [names addObject:element]);
            element.shownName = name;
            element.accessibilityLabel = name.length ? name : nil;
            id value = nil;
            element.numeric = x->value.tag == 2;
            if (element.numeric) {
                value = @(x->value.now);
                element.now = x->value.now; element.min = x->value.min; element.max = x->value.max; element.step = x->value.step;
                element.accessibilityMinValue = @(x->value.min);
                element.accessibilityMaxValue = @(x->value.max);
            } else if (x->value.tag == 1) {
                value = string(strings, x->value.text_start, x->value.text_end);
            } else if (isFlag(x->states, StCheckable)) {
                value = @(isFlag(x->states, StMixed) ? 2 : isFlag(x->states, StChecked) ? 1 : 0);
            } else if (x->role == 5 || x->role == 6 || x->role == 7) {
                value = name;
            }
            id old = element.shownValue;
            if (!same(old, value)) {
                [element storeValue:value];
                element.shownValue = value;
                if (old || value) (values = values ?: [NSMutableArray new], [values addObject:element]);
            }
        }
        if (host->elements.count > n) {
            NSMutableArray<PxuiAXElement *> *gone = [NSMutableArray new];
            for (PxuiAXElement *element in host->elements.objectEnumerator) if (element.sweep != sweep) [gone addObject:element];
            for (PxuiAXElement *element in gone) {
                [host post:element name:NSAccessibilityUIElementDestroyedNotification];
                element.accessibilityChildren = nil;
                element.accessibilityParent = nil;
                NSMapRemove(host->elements, (const void *)(uintptr_t)(element.nodeId + 1));
            }
            structure = YES;
        }
        host->orderCount = n;
        if (structure) {
            for (uint32_t i = 0; i < n; i++) {
                PxuiAXElement *element = elementAt(host, i);
                NSMutableArray *kids = [NSMutableArray arrayWithCapacity:nodes[i].child_count];
                uint32_t steps = 0;
                for (uint32_t c = nodes[i].first_child; c != None && steps++ < n; c = nodes[c].next_sibling) {
                    PxuiAXElement *child = elementAt(host, c);
                    [kids addObject:child];
                    if (child.accessibilityParent != element) child.accessibilityParent = element;
                }
                if (![kids isEqualToArray:element.accessibilityChildren ?: @[]]) element.accessibilityChildren = kids;
            }
            NSMutableArray *top = [NSMutableArray new];
            uint32_t steps = 0;
            for (uint32_t c = first_top; c != None && steps++ < n; c = nodes[c].next_sibling) {
                PxuiAXElement *child = elementAt(host, c);
                [top addObject:child];
                if (child.accessibilityParent != view) child.accessibilityParent = view;
            }
            if (![top isEqualToArray:view.accessibilityChildren ?: @[]]) [view setAccessibilityChildren:top];
            [host post:view name:NSAccessibilityLayoutChangedNotification];
        }
        for (PxuiAXElement *element in values) [host post:element name:NSAccessibilityValueChangedNotification];
        for (PxuiAXElement *element in names) [host post:element name:NSAccessibilityTitleChangedNotification];
        uint64_t focusId = focus ? focus.nodeId : 0;
        if (focus && focusId != host->focused) [host post:focus name:NSAccessibilityFocusedUIElementChangedNotification];
        host->focused = focusId;
        return 1;
    }
}

#ifdef PXUI_SDL3_TEST_UTIL
// Queries go through the NSAccessibility protocol, as an assistive client's would.
static id findLabeled(id<NSAccessibility> object, NSString *label) {
    for (id<NSAccessibility> child in object.accessibilityChildren) {
        if ([child.accessibilityLabel isEqualToString:label]) return child;
        id deeper = findLabeled(child, label);
        if (deeper) return deeper;
    }
    return nil;
}

static void copy(char *to, size_t size, NSString *from) { snprintf(to, size, "%s", from.UTF8String ?: ""); }

int pxui_sdl3_ax_test_find(void *raw, const char *label, PxuiAXInfo *info) {
    @autoreleasepool {
        PxuiAX *host = (__bridge PxuiAX *)raw;
        if (!label) { info->children = (int)host->view.accessibilityChildren.count; return 1; }
        id<NSAccessibility> e = findLabeled(host->view, @(label));
        if (!e) return 0;
        *info = (PxuiAXInfo){ 0 };
        copy(info->role, sizeof info->role, e.accessibilityRole);
        copy(info->label, sizeof info->label, e.accessibilityLabel);
        id value = e.accessibilityValue;
        if ([value isKindOfClass:NSString.class]) copy(info->value, sizeof info->value, value);
        if ([value isKindOfClass:NSNumber.class]) { info->has_number = 1; info->number = [value doubleValue]; }
        info->min = [e.accessibilityMinValue doubleValue];
        info->max = [e.accessibilityMaxValue doubleValue];
        NSRect f = e.accessibilityFrame;
        info->frame[0] = f.origin.x; info->frame[1] = f.origin.y; info->frame[2] = f.size.width; info->frame[3] = f.size.height;
        info->enabled = e.accessibilityEnabled; info->selected = e.accessibilitySelected;
        info->expanded = e.accessibilityExpanded; info->focused = e.accessibilityFocused;
        info->actionable = [(NSObject *)e isKindOfClass:PxuiAXActionElement.class];
        info->children = (int)e.accessibilityChildren.count;
        info->identity = (uintptr_t)(__bridge void *)e;
        return 1;
    }
}

int pxui_sdl3_ax_test_perform(void *raw, const char *label, int kind, double number) {
    @autoreleasepool {
        id<NSAccessibility> e = findLabeled(((__bridge PxuiAX *)raw)->view, @(label));
        if (!e) return -1;
        // Only action elements implement these; AppKit's base class throws for them.
        if (kind >= 0 && kind <= 3 && ![(NSObject *)e isKindOfClass:PxuiAXActionElement.class]) return 0;
        switch (kind) {
        case 0: return [e accessibilityPerformPress];
        case 1: return [e accessibilityPerformIncrement];
        case 2: return [e accessibilityPerformDecrement];
        case 3: return [e accessibilityPerformShowMenu];
        case 4: e.accessibilityValue = @(number); return 1;
        default: e.accessibilityFocused = YES; return 1;
        }
    }
}
#endif
