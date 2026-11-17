#ifndef PXUI_H
#define PXUI_H

/* ABI status: version 1 is unreleased, so its layouts (abi/layout.v1.txt) and functions (abi/pxui.v1.txt) may still change.
   From the first release both are append-only: a layout is never altered, a descriptor grows by new trailing fields
   (its leading `size` says how much the caller wrote), and a change that needs more is a new function.
   Every call that can fail returns int32_t, 0 on success; the reason is pxui_last_error(ui). */

#include <stddef.h>
#include <stdint.h>

typedef struct PxuiUi PxuiUi;
typedef struct PxuiRenderer PxuiRenderer;
typedef struct PxuiPaint PxuiPaint;
typedef struct PxuiShaper PxuiShaper;
typedef struct PxuiPreparedText PxuiPreparedText;
typedef struct PxuiTheme PxuiTheme;
typedef struct { uint32_t size, platform; int32_t font_size; } PxuiUiDesc;
typedef struct { const uint8_t *ptr; size_t len; } PxuiStr;
/* Autocorrect: 0/1; capitalization: none/sentences/words/letters (0..3); action: Return/Done/Search (0..2). */
typedef struct { uint32_t autocorrect, capitalization, action; } PxuiTextInputOptions;
typedef struct { const PxuiStr *ptr; size_t len; } PxuiStrSpan;
typedef struct { uint32_t tag; size_t index; } PxuiDialogOut;
/* ABI 1.25: TOUCH borrows this numeric payload through Event.text; len is sizeof(PxuiTouch).
   IDs retain all 64 bits. Phase: 0 began, 1 moved, 2 ended, 3 cancelled. Positions are logical points
   and may be outside the window. Time is source monotonic seconds; finite pressure is in [0,1]. */
typedef struct { uint64_t device, finger; double x, y, time; float pressure; uint32_t phase; } PxuiTouch;
typedef struct { uint32_t kind, code; double x, y, value; PxuiStr text; } PxuiEvent;
typedef struct { const PxuiEvent *ptr; size_t len; } PxuiEventSpan;
typedef struct { uint32_t size, reserved; size_t parents, seeds; } PxuiSaved;
typedef struct {
    uint32_t size, flags;
    uint8_t has_subtree_press, button, has_a11y_value, has_a11y_select, reserved[4];
    int64_t subtree_press, clicks;
    float drag[2], pointer[2], press_point[2], release_point[2], scroll[2];
    double pinch, a11y_value;
    uint32_t a11y_select[2];
    PxuiEventSpan keys; /* borrowed from the Ui until its next frame, with the text its events point at */
} PxuiSignal;
typedef struct { const uint32_t *ptr; size_t len; } PxuiU32Span;
typedef struct { const int64_t *ptr; size_t len; } PxuiI64Span;
typedef struct { const uint8_t *ptr; size_t len; } PxuiU8Span;
typedef struct { uint8_t *ptr; size_t len; } PxuiU8MutSpan;
typedef struct { float *ptr; size_t len; } PxuiF32MutSpan;
typedef struct { uint32_t *ptr; size_t len; } PxuiU32MutSpan;
typedef struct { uint32_t size; double time,dt; int64_t frame,tick; int64_t logical_size[2]; double pointer[2]; PxuiU32Span keys,buttons; PxuiEventSpan events; double pixel_scale[2],mouse_delta[2],fps; } PxuiInput;
typedef struct { float x,y,w,h; } PxuiRect;
typedef struct { float x,y; } PxuiPoint;
typedef struct { uint8_t r,g,b,a; } PxuiColor;
typedef struct { float fast,normal,slow,ease[4]; } PxuiMotion;
typedef struct { uint32_t size; PxuiColor panel,foreground,control,input,track,accent,ink_ground; uint8_t has_ink_ground,reserved[3]; float space[4],radius[3]; int64_t display_size,title_size,font_size,label_size; PxuiMotion motion; uint8_t reduced_motion,reserved_tail[3]; float density; } PxuiThemeTokens;
typedef struct { uint32_t size,cursor; uint8_t has_cursor,has_ime,has_background,has_clipboard; int64_t ime_rect[4],ime_caret; PxuiStr clipboard; PxuiColor background; uint8_t paste,reserved[7]; } PxuiPlatformOut;
typedef struct { uint8_t has_damage,reserved[3]; PxuiRect rect; } PxuiDamage;
typedef struct { uint32_t tag,text_start,text_end,reserved; double now,min,max,step; } PxuiA11yValue;
typedef struct { uint64_t id; uint32_t parent,role,name_start,name_end,description_start,description_end; PxuiA11yValue value; PxuiRect rect; uint32_t states,actions,selection[2],child_count,first_child,next_sibling,last_child; } PxuiA11yNode;
typedef struct { uint64_t id; uint32_t action_code,reserved; double payload; uint32_t end,reserved_tail; } PxuiA11yAction;
typedef struct { const PxuiA11yNode *ptr; size_t len; } PxuiA11yNodeSpan;
typedef struct { const PxuiA11yAction *ptr; size_t len; } PxuiA11yActionSpan;
typedef struct { PxuiA11yNodeSpan nodes; PxuiStr strings; uint64_t revision; uint32_t first_child,child_count; PxuiA11yActionSpan actions; } PxuiA11yTree;
typedef struct { uint32_t size,flags; PxuiStr source; } PxuiThemeSource;
typedef struct { float l,t,r,b; } PxuiEdges;
typedef struct { uint32_t kind; float value; float (*relative)(float); } PxuiSizeDesc;
typedef struct { uint8_t has_anchor,reserved[3]; PxuiRect rect; uint32_t side,align; float gap,margin; uint8_t flip,reserved_tail[3]; } PxuiAnchorDesc;
typedef struct { uint16_t family,weight; int64_t size; float tracking; uint8_t caps,reserved[3]; } PxuiFontDesc;
typedef struct { PxuiColor color; float offset[2],blur,spread; uint8_t inner,reserved[3]; } PxuiShadowDesc;
typedef struct { PxuiColor color; float width; uint32_t sides; } PxuiRingDesc;
typedef struct { PxuiColor fill,gradient; uint8_t has_gradient,reserved[3]; float angle; PxuiColor border; float border_w; uint32_t sides; float radius[4]; PxuiShadowDesc shadows[2]; PxuiColor text; PxuiFontDesc font; float opacity; PxuiRingDesc ring; PxuiColor selection,caret; uint8_t dashed,reserved_tail[3]; } PxuiLookDesc;
typedef struct { uint32_t kind,direction; float width; uint8_t rising,reserved[3]; } PxuiMarkDesc;
typedef struct { uint8_t hovered,pressed,focused,disabled,checked,reserved[3]; } PxuiStateDesc;
/* Table: count 2..16, strictly increasing positions from 0 to 1, finite values from 0 to 1 at the endpoints (overshoot allowed).
   Steps: count > 0. Duration is finite, nonnegative seconds; keyframes use their segment duration. Only active table entries are read. */
typedef struct { uint32_t kind; float duration,points[4],omega; uint32_t count; float positions[16],values[16]; } PxuiCurveDesc;
/* The engine's own value types, as they cross: a node, a look and the theme are these bytes (abi/layout.v1.txt pins them,
   the asserts below check the C side). An enum is a byte tag (an optional field: 0 none, 1 some) before its value, padded
   to the value's alignment; a flag byte is 0 or 1. The library checks a node's tags and flags where it lies and refuses a
   node with any other value. */
typedef struct { uint32_t kind; union { float value; float (*relative)(float); } u; } PxuiSize;   /* enum PxuiSizeKind */
typedef struct { uint8_t kind; float value; } PxuiTrack;                                        /* enum PxuiTrackKind */
typedef struct { const PxuiTrack *cols; size_t cols_len; const PxuiTrack *rows; size_t rows_len; } PxuiGrid; /* at most 64 each; copied */
typedef struct { uint16_t col,row,col_span,row_span; } PxuiCell;
typedef struct { uint8_t side,align; float gap,margin; uint8_t flip; } PxuiPlacement;
typedef struct { PxuiRect rect; PxuiPlacement placement; } PxuiAnchor;
typedef struct { uint8_t kind; union { uint8_t direction; struct { float width; uint8_t rising; } line; } u; } PxuiMark; /* enum PxuiMarkKind */
typedef struct { uint8_t kind; union { struct { float duration,points[4]; } bezier; float omega; } u; } PxuiTransition; /* 0 bezier, 1 spring */
typedef struct { PxuiColor color; float radius,width,gap,margin; uint8_t whole; } PxuiBar;
typedef struct { uint8_t has; uint8_t value; } PxuiOptAlign;
typedef struct { uint8_t has; PxuiGrid value; } PxuiOptGrid;
typedef struct { uint8_t has; PxuiAnchor value; } PxuiOptAnchor;
typedef struct { uint8_t has; float x,y; } PxuiOptPoint;
typedef struct { uint8_t has; PxuiBar value; } PxuiOptBar;
typedef struct { uint8_t has; PxuiTransition value; } PxuiOptTransition;
typedef struct { uint8_t has; PxuiColor value; } PxuiOptColor;
typedef struct { uint16_t family; int64_t size; uint16_t weight; float tracking; uint8_t caps; } PxuiFont;
typedef struct { PxuiColor color; float offset[2],blur,spread; uint8_t inner; } PxuiShadow;
typedef struct { PxuiColor color; float width; uint8_t sides; } PxuiRing;
typedef struct {
    PxuiColor fill; PxuiOptColor gradient; float angle; PxuiColor border; float border_w; uint8_t sides; float radius[4];
    PxuiShadow shadows[2]; PxuiColor text; PxuiFont font; float opacity; PxuiRing ring; PxuiColor selection,caret; uint8_t dashed;
} PxuiLook;
typedef struct { uint8_t hovered,pressed,focused,disabled,checked; } PxuiState;
/* ABI 1.29: ceil intrinsic text width (including text insets) to the integer rendering density. */
#define PXUI_FIT_TEXT_PIXEL INT64_C(2097152)
typedef struct {
    uint32_t size; /* sizeof(PxuiNode) */
    uint8_t axis,justify,align,wrap,role; PxuiState state; int16_t z; /* the first 16 bytes are tested as two words */
    int64_t flags; PxuiSize w,h; float min_w,min_h,max_w,max_h,grow,shrink,aspect; PxuiEdges padding,margin;
    float gap,row_gap; PxuiOptGrid grid; PxuiCell cell; PxuiOptAlign align_self;
    PxuiOptAnchor anchor; uint32_t a11y; PxuiOptPoint at; float scroll_step; PxuiLook look; PxuiEdges inset;
    PxuiMark mark; PxuiOptBar bar; PxuiOptTransition transition;
} PxuiNode;
typedef struct {
    PxuiColor panel,foreground,control,input,track,accent,invalid; PxuiOptColor ink_ground; float space[4],radius[3];
    int64_t display_size,title_size,font_size,label_size; PxuiMotion motion; uint8_t reduced_motion; float density;
} PxuiTokens;
typedef struct { PxuiLook idle,hover,pressed,focus,disabled,checked; } PxuiStates;
#define PXUI_THEME_PARTS 59
/* The theme in effect, lent by pxui_ui_theme: the kit parts in the order of `theme_widgets!` (pxui_types/src/theme.rs). */
/* The body each widget kind builds with: 0 the built-in, n the nth body registered for the kind. */
typedef struct { uint16_t toggle,slider,progress,button,icon_button,checkbox,radio,spinner,badge,label,footer,grip,panel,window,popup,modal,dialog,inspector_section,inspector_row,split,scroll_view,toast,tip,tab,segment,dropdown,menu_bar,menu_item,context_menu,list_view,tree,table_view,table_head,table,picker,search_field,text_field,line_field,value_field,picker_field; } PxuiBodies;
typedef struct { PxuiTokens tokens; float row_h,control_h; PxuiRing ring; float scrollbar_w,scrollbar_gap; PxuiStates parts[PXUI_THEME_PARTS]; PxuiBodies bodies; } PxuiThemeSnapshot;
#if defined(__LP64__) || defined(_WIN64)
#endif
enum PxuiSizeKind { PXUI_SIZE_PX=0, PXUI_SIZE_PCT=1, PXUI_SIZE_REL=2, PXUI_SIZE_GROW=3, PXUI_SIZE_FIT=4 };
enum PxuiTrackKind { PXUI_TRACK_PX=0, PXUI_TRACK_FR=1, PXUI_TRACK_FIT=2 };
enum PxuiAxis { PXUI_AXIS_ROW=0, PXUI_AXIS_COLUMN=1, PXUI_AXIS_STACK=2 };
enum PxuiJustify { PXUI_JUSTIFY_START=0, PXUI_JUSTIFY_CENTER=1, PXUI_JUSTIFY_END=2, PXUI_JUSTIFY_SPACE_BETWEEN=3, PXUI_JUSTIFY_SPACE_AROUND=4, PXUI_JUSTIFY_SPACE_EVENLY=5 };
enum PxuiAlign { PXUI_ALIGN_START=0, PXUI_ALIGN_CENTER=1, PXUI_ALIGN_END=2, PXUI_ALIGN_STRETCH=3, PXUI_ALIGN_BASELINE=4 };
enum PxuiAnchorSide { PXUI_SIDE_BELOW=0, PXUI_SIDE_ABOVE=1, PXUI_SIDE_RIGHT=2, PXUI_SIDE_LEFT=3 };
enum PxuiCurveKind { PXUI_CURVE_NONE=0, PXUI_CURVE_BEZIER=1, PXUI_CURVE_SPRING=2, PXUI_CURVE_TABLE=3, PXUI_CURVE_STEPS=4 };
enum PxuiMarkKind { PXUI_MARK_NONE=0, PXUI_MARK_CHEVRON=1, PXUI_MARK_LINE=2 };
enum PxuiMarkDirection { PXUI_DIR_DOWN=0, PXUI_DIR_UP=1, PXUI_DIR_LEFT=2, PXUI_DIR_RIGHT=3 };
typedef struct {
    uint32_t size, backend;
    void *instance, *physical_device, *device, *queue;
    uint32_t queue_family, api_version;
    uint64_t features;
    void *get_instance_proc_addr, *pipeline_cache, *mtl_device;
    void *wl_display, *wl_surface, *metal_layer, *hwnd;
    void *owner_token;
    void (*release_owner)(void *owner_token);
} PxuiGpu;
typedef struct { uint32_t size; void *vk_command_buffer, *mtl_command_buffer; } PxuiCommands;
typedef struct { uint32_t size; void *vk_image_view, *mtl_texture; uint32_t width,height,format; } PxuiImage;
typedef struct PxuiPass PxuiPass;
typedef int32_t (*PxuiPassRecord)(void *user, const PxuiImage *target);
/* A layer with a backdrop blur needs what the frame holds so far: split(user, record, record_user, out) ends the app's open pass,
   calls record(record_user, &target) (the colour target, sampleable, everything drawn so far), reopens a pass over the same target
   (load, not clear; same viewport and scissor), writes it to `out` and returns 0. */
typedef int32_t (*PxuiPassSplit)(void *user, PxuiPassRecord record, void *record_user, PxuiPass *out);
/* Draws layers first_layer.. (layer_count of them; 0 means every layer from first_layer), so an app can draw its own content between. */
struct PxuiPass { uint32_t size; void *vk_command_buffer, *mtl_render_encoder; uint32_t width_px,height_px; float scale; void *mtl_command_buffer; uint32_t first_layer,layer_count; PxuiPassSplit split; void *split_user; };
typedef struct { uint32_t size, format; uint8_t samples; uint8_t reserved[3]; uint32_t mask_width,mask_height,color_width,color_height,reserved_tail; } PxuiTargetDesc;
/* wrap: 0 none, 1 word, 2 glyph, 3 word with omitted soft-wrap separator advances in fitting width (ABI 1.18). */
typedef struct { uint32_t size; uint16_t font,weight; float size_points,line_height; uint32_t wrap,align; uint16_t max_lines; uint8_t italic,ellipsis; uint32_t features; float tracking; } PxuiTextStyle;
#define PXUI_TEXT_WRAP_NONE 0u
#define PXUI_TEXT_WRAP_WORD 1u
#define PXUI_TEXT_WRAP_GLYPH 2u
#define PXUI_TEXT_WRAP_WORD_SPACES 3u
#define PXUI_TEXT_FEATURE_TABULAR_NUMBERS 1u
/* Truncate the last visible line (one when max_lines is 0). */
#define PXUI_TEXT_ELLIPSIS_NONE 0u
#define PXUI_TEXT_ELLIPSIS_END 1u
#define PXUI_TEXT_ELLIPSIS_MIDDLE 2u
#define PXUI_TEXT_ELLIPSIS_HEAD 3u
/* Ordered, disjoint, nonempty UTF-8 byte ranges. Weight 0 inherits the style.
   Only active spans and flagged colours are read. Flagged colours are independent;
   underline is a 1.5-point rule at the bottom of the line box. */
typedef struct { uint32_t start,end; uint16_t weight,flags; PxuiColor color,background,underline; } PxuiTextSpan;
typedef struct { uint32_t size,count; PxuiTextSpan spans[8]; } PxuiTextSpans;
#define PXUI_TEXT_SPAN_COLOR 1u
#define PXUI_TEXT_SPAN_BACKGROUND 2u
#define PXUI_TEXT_SPAN_UNDERLINE 4u
typedef struct { float width,height,baseline; uint32_t lines; } PxuiTextMetrics;
typedef struct { float rect[4],uv[4]; uint32_t atlas,reserved; } PxuiTextGlyph;
#define PXUI_ATLAS_MASK 0u
#define PXUI_ATLAS_COLOR 1u
#define PXUI_ATLAS_MASK_2 2u
#define PXUI_ATLAS_COLOR_2 3u
/* Mask/image glyph instance flag: use the second fixed atlas page. */
#define PXUI_GLYPH_PAGE_2 2u
/* ABI 1.14: pixel-area coverage for a plain, unstroked RECT instance. */
#define PXUI_RECT_AREA 4u
/* ABI 1.39: a node's transition is a PxuiTransition (Bezier or spring), so PxuiNode is 488 bytes; tables and steps stay in PxuiCurveDesc. */
/* ABI 1.37: MASK instance flag for CoreText-matching light text (docs/abi.md 1.37).
   extra[0] packs two f16 (ppem in device pixels low, device scale high); the shader grows ink by the fill colour's luminance level.
   Renderers that predate it ignore the bit and the extra words and draw the plain mask. */
#define PXUI_LIGHT 16u
/* ABI 1.24: logical window points; insets are top/right/bottom/left, rectangles x/y/w/h. */
typedef struct {
    float safe_area[4], keyboard_frame[4], keyboard_from[4];
    double keyboard_started, keyboard_duration;
    uint32_t keyboard_curve, keyboard_visible;
} PxuiEnvironment;
/* ABI 1.26: current UIKit keyboard presentation frame, including intermediate geometry. */
typedef struct { float frame[4]; uint32_t flags; } PxuiKeyboard;
#define PXUI_KEYBOARD_VALID 1u
#define PXUI_KEYBOARD_ANIMATING 2u
typedef struct {
    uint32_t size;
    /* Event-only entry: held keys persist until release or focus loss. */
    const PxuiEvent *events;
    size_t event_count;
    float width, height, scale;
    double time;
    /* ABI 1.8 tail; legacy size=48 defaults both to zero. Partial tails are refused. */
    uint32_t appearance, reduced_motion;
    /* Read only when size includes this complete tail; otherwise defaults to zero. */
    PxuiEnvironment environment;
    PxuiKeyboard keyboard;
} PxuiFrameIn;
#define PXUI_APPEARANCE_UNKNOWN 0u
#define PXUI_APPEARANCE_LIGHT 1u
#define PXUI_APPEARANCE_DARK 2u
typedef struct {
    uint32_t size;
    float damage[4];
    uint8_t has_damage, reserved[3];
    uint32_t layer_count;
    const void *layers;
} PxuiFrameOut;
enum PxuiEventKind {
    PXUI_KEY_PRESSED=1, PXUI_KEY_RELEASED=2, PXUI_POINTER_MOVED=3,
    PXUI_POINTER_PRESSED=4, PXUI_POINTER_RELEASED=5, PXUI_POINTER_CANCELLED=6,
    PXUI_SCROLLED=7, PXUI_TEXT_INPUT=8, PXUI_PASTE=9, PXUI_TEXT_EDITING=10,
    /* FILES_DROPPED (ABI 1.33): x/y position; code 0 NUL-separated paths, code 1 error text. */
    PXUI_FILE_DROPPED=11, PXUI_FILE_DRAG_MOVED=12, PXUI_FILE_DRAG_ENDED=13,
    PXUI_PINCHED=14, PXUI_FILE_DIALOG=15, PXUI_DIALOG_OPENED=16, PXUI_RESIZED=17,
    PXUI_FOCUS_LOST=18, PXUI_FOCUS_GAINED=19, PXUI_CLOSED=20,
    PXUI_PIXEL_SIZE_CHANGED=21, PXUI_VISIBILITY_CHANGED=22, PXUI_TRACKPAD_SCROLLED=23, PXUI_ENVIRONMENT_CHANGED=24, PXUI_TOUCH=25, PXUI_FILES_DROPPED=26
};
enum PxuiValueCode {
    PXUI_MOUSE_LEFT=0, PXUI_MOUSE_RIGHT=1, PXUI_MOUSE_MIDDLE=2, PXUI_MOUSE_X1=3, PXUI_MOUSE_X2=4,
    /* Key codes: a Unicode scalar value (below PXUI_KEY_SPECIAL) is that character; PXUI_KEY_SPECIAL + n is special key n;
       PXUI_KEY_UNKNOWN | code carries a platform code. F2..F11 follow F1 in order. */
    PXUI_KEY_SPECIAL=0x110000,
    PXUI_KEY_UP=PXUI_KEY_SPECIAL+0, PXUI_KEY_DOWN=PXUI_KEY_SPECIAL+1, PXUI_KEY_LEFT=PXUI_KEY_SPECIAL+2, PXUI_KEY_RIGHT=PXUI_KEY_SPECIAL+3,
    PXUI_KEY_SPACE=PXUI_KEY_SPECIAL+4, PXUI_KEY_ENTER=PXUI_KEY_SPECIAL+5, PXUI_KEY_ESCAPE=PXUI_KEY_SPECIAL+6, PXUI_KEY_BACKSPACE=PXUI_KEY_SPECIAL+7,
    PXUI_KEY_TAB=PXUI_KEY_SPECIAL+8, PXUI_KEY_SHIFT=PXUI_KEY_SPECIAL+9, PXUI_KEY_CTRL=PXUI_KEY_SPECIAL+10, PXUI_KEY_ALT=PXUI_KEY_SPECIAL+11, PXUI_KEY_META=PXUI_KEY_SPECIAL+12,
    PXUI_KEY_F1=PXUI_KEY_SPECIAL+13, PXUI_KEY_F12=PXUI_KEY_SPECIAL+24,
    PXUI_KEY_HOME=PXUI_KEY_SPECIAL+25, PXUI_KEY_END=PXUI_KEY_SPECIAL+26, PXUI_KEY_PAGE_UP=PXUI_KEY_SPECIAL+27, PXUI_KEY_PAGE_DOWN=PXUI_KEY_SPECIAL+28,
    PXUI_KEY_INSERT=PXUI_KEY_SPECIAL+29, PXUI_KEY_DELETE=PXUI_KEY_SPECIAL+30
};
#define PXUI_KEY_UNKNOWN 0x80000000u
/* Status codes of every call that returns int32_t: 0 is success. */
#define PXUI_ERR_ARGUMENT (-1) /* invalid argument; pxui_last_error says which */
#define PXUI_ERR_GPU (-2)      /* GPU adoption or recording failed (pxui_renderer_*) */
#define PXUI_ERR_STATE (-3)    /* not allowed now: inside a frame callback, on a borrowed handle, or with a renderer attached */
#define PXUI_ERR_STALE (-5)    /* the box handle names a node that has gone (queries and patches treat it as "nothing there") */
#define PXUI_ERR_PANIC (-4)    /* a panic was caught in the library; the Ui fails every call until its next frame call */
enum PxuiTargetFormat { PXUI_TARGET_RGBA8=1, PXUI_TARGET_BGRA8=2 };
typedef struct {
    uint32_t size;
    float x, y, width, height;
    int64_t padding;
    uint8_t has_height, window, has_max_height, has_stroke, has_look, reserved[3];
    float max_height; PxuiColor stroke; PxuiLookDesc look;
} PxuiPanel;
typedef struct { uint64_t index; int64_t box_key; uint64_t box_slot; uint32_t generation; } PxuiBox;
/* ABI 1.48: what happened to a kit button or icon button this frame, in one record: PXUI_RESPONSE_* bits and the button's box.
   The caller sets size; a null or shorter record is refused. */
typedef struct { uint32_t size, flags; PxuiBox node; } PxuiResponse;
/* ABI 1.53: a modal dialog (at most 64 buttons; masks as PxuiDialogConfig) and what its frame did (chosen is valid when has_chosen) */
typedef struct { uint32_t size; float width; PxuiStr key, title; PxuiStrSpan buttons; int64_t default_at, cancel_at; uint64_t danger, disabled; } PxuiDialogDesc;
typedef struct { uint32_t size; uint8_t has_chosen, dismissed, reserved[2]; uint64_t chosen; } PxuiDialogResult;
enum { PXUI_RESPONSE_CLICKED = 1, PXUI_RESPONSE_HOVERED = 2, PXUI_RESPONSE_HELD = 4, PXUI_RESPONSE_FOCUSED = 8, PXUI_RESPONSE_ACTIVATED = 16 };
/* ABI 1.31: flags bit 0 preview, bit 1 commit, bit 2 held. Initialize size before calling. */
typedef struct { uint32_t size, flags; PxuiBox node; float preview, commit; } PxuiScrubOut;
/* ABI 1.38: kinds bit 0 long press, 1 row swipe, 2 edge back, 3 pull to refresh, 5 touch drag (ABI 1.42: the node takes the touch at once as a held press and drag, never a tap, scroll or
   other gesture; PxuiGestureIn kinds & ~47 is refused); reveal is the row-swipe action
   width and threshold the pull distance (0: 60), in points. Initialize size before calling. */
typedef struct { uint32_t size, kinds; float reveal, threshold; } PxuiGestureIn;
/* flags: 1 highlighted, 2 long press (this frame), 4 swiping, 8 swipe open, 16 back active, 32 back commit and
   64 back cancel (this frame), 128 refresh fired (this frame), 256 refreshing. swipe is the signed shift to paint a
   swiped row's content at (mirrored right to left), back is progress 0..1, pull the pull-down in points. */
typedef struct { uint32_t size, flags; float swipe, back, pull; } PxuiGestureOut;
/* ABI 1.40: row-swipe action widths in points, 0 disables a side (leading follows the layout direction), and the finger
   travel that commits the edge action on release (0: no full swipe). Initialize size before calling. */
typedef struct { uint32_t size; float leading, trailing, full; } PxuiSwipeIn;
/* ABI 1.41: drag-to-dismiss request: fraction of the node height (0: 0.25) and downward release speed in points per
   second (0: 600) that commit. Initialize size before calling. */
typedef struct { uint32_t size; float fraction, velocity; } PxuiDismissIn;
/* flags: 8192 dragging or settling, 16384 committed and 32768 cancelled (this frame); offset is the downward points to
   translate the node by (negative while rubber-banding up, the node height once a commit settled). */
typedef struct { uint32_t size, flags; float offset; } PxuiDismissOut;
typedef struct { uint16_t depth; uint8_t children; } PxuiTreeNode;
typedef struct { uint32_t size; uint8_t has_stroke,has_max_height,dismiss_initial,attached; PxuiColor stroke; float max_height,x,y,width,height; PxuiStr label; } PxuiPopupDesc;
typedef struct { uint32_t size; uint8_t rtl,reserved[3]; int64_t font_size,row_height,density; PxuiRect clip; float clip_radius; PxuiThemeTokens theme; } PxuiPaintInfo;
typedef void (*PxuiIconDraw)(void *user, PxuiPaint *paint, PxuiRect rect, PxuiColor ink, const PxuiPaintInfo *info); /* an icon drawing, called at paint time; info: valid during the call */
/* ABI 1.45: a label's style. A field whose PXUI_LABEL_SET_* bit in `set` is clear is the theme's label style (font, size,
   weight, tracking in ems, capitals, colour; width full; line height the row height, 0 for 1.2 times the size; pad 12 with 4
   above a first child). color is an app colour token; digits 1 draws one cached character at a time, digits on the advance of 0. */
enum { PXUI_LABEL_SET_FONT = 1, PXUI_LABEL_SET_SIZE = 2, PXUI_LABEL_SET_WEIGHT = 4, PXUI_LABEL_SET_TRACKING = 8, PXUI_LABEL_SET_CAPS = 16,
       PXUI_LABEL_SET_COLOR = 32, PXUI_LABEL_SET_WIDTH = 64, PXUI_LABEL_SET_LINE_HEIGHT = 128, PXUI_LABEL_SET_PAD = 256 };
typedef struct { uint32_t size, set; uint16_t font, weight, color, max_lines; uint8_t caps, digits, ellipsis, reserved;
                 float size_points, tracking, line_height, pad; uint32_t wrap, align, features; PxuiSizeDesc width; } PxuiLabelStyle;
/* ABI 1.54: a label's box beside its style: height (PXUI_LABEL_BOX_SET_HEIGHT) the node's height with its lines centred in it, role
   (PXUI_LABEL_BOX_SET_ROLE; clear is a label) its accessible role, whole_pixels 1 rounds a fitted width up to whole pixels. */
enum { PXUI_LABEL_BOX_SET_HEIGHT = 1, PXUI_LABEL_BOX_SET_ROLE = 2 };
typedef struct { uint32_t size, set; float height; uint8_t role, whole_pixels, reserved[2]; } PxuiLabelBox;
/* ABI 1.46: a kit icon button with options. w_kind/h_kind 0 the kit square, 1 points, 2 fill; states NULL is the theme's icon button, else six
   PxuiLookDesc (idle, hover, pressed, focus, disabled, checked); press_scale is the plate's scale while held (1 = none); the icon is the
   glyph `icon`, or, when `draw` is set, the app's drawing called at paint time with draw_user, the 20-point square and the look's ink.
   ABI 1.54 (a 96-byte record is still read, without it): press null for the kit's springs, else two PxuiCurve (in on press, out on
   release), 4-aligned and valid for the call. */
/* ABI 1.54: a curve as pxui's Rust `Curve` lays it out (repr(C, u8)): kind 0 bezier, 1 spring, 2 table, 3 steps. */
typedef struct { uint8_t kind, reserved[3]; union { struct { float duration, points[4]; } bezier; struct { float omega; } spring;
                 struct { float duration; uint32_t count; float positions[16], values[16]; } table; struct { float duration; uint32_t count; } steps; } data; } PxuiCurve;
typedef struct { uint32_t size; uint8_t enabled,w_kind,h_kind,reserved; float w,h,press_scale; const PxuiLookDesc *states; PxuiStr key,name,icon; PxuiIconDraw draw; void *draw_user;
                 const PxuiCurve *press; } PxuiIconButtonDesc;
/* ABI 1.44: a kit button with options. w_kind/h_kind 0 the kit's size, 1 points, 2 fill; justify 0 start, 1 centre, 2 end; states NULL is the theme's button, else six PxuiLookDesc (idle, hover, pressed, focus, disabled, checked).
   ABI 1.54 (a 96-byte record is still read, without it): centered 1 makes the box exactly its size with nothing around it in its line, its text centred. */
typedef struct { uint32_t size; uint8_t enabled,selected,danger,justify,w_kind,h_kind,has_name,reserved; float w,h,padding[4]; const PxuiLookDesc *states; PxuiStr key,text,name; uint8_t centered,reserved2[7];
                 /* ABI 1.56 */ uint8_t has_label,axis,content_align,focus_on_click; float gap; PxuiLabelStyle label; PxuiLabelBox label_box; } PxuiButtonDesc;
/* ABI 1.56: a button's content callback; state is the button's interaction state this frame, valid for the call. */
typedef void (*PxuiButtonBuild)(void *user, PxuiUi *ui, const PxuiStateDesc *state);
/* ABI 1.55: PxuiToggleDesc grows by the title's text style (has_style 1: style, else the theme's toggle font; an 88-byte record is still read)
   and PxuiSegmentedDesc by the strip's width w (0: as grow says), padding (the text's inset, 8 the kit's), min_segment (0: none) and names
   (empty, or one accessible name per option, an empty one the option's text; a 64-byte record is still read, padding 8). */
typedef struct { uint32_t size; uint8_t enabled,on,press_row,reserved; float row_h,switch_w,switch_h,inset; const PxuiLookDesc *track,*knob; PxuiStr key,text,detail; uint8_t has_style, reserved2[7]; PxuiLabelStyle style; } PxuiToggleDesc;
typedef struct { uint32_t size; uint8_t enabled,grow,reserved[2]; float h; const PxuiLookDesc *states; PxuiStr key,name; size_t selected; float w, padding, min_segment; uint32_t reserved2; PxuiStrSpan names; } PxuiSegmentedDesc;
typedef struct { uint32_t size; uint8_t active,usable,reserved[2]; PxuiStr caption,keycap; } PxuiHeadActionDesc;
typedef struct { const PxuiHeadActionDesc *ptr; size_t len; } PxuiHeadActionSpan;
typedef struct { uint32_t size; uint8_t has_kind,has_badge,has_index,has_reset,reset_enabled,reserved[3]; PxuiStr key,kind,badge,index,reset,title,detail; PxuiHeadActionSpan actions; } PxuiHeaderDesc;
typedef struct { uint32_t size; uint8_t has_chosen,reset_pressed,reserved[2]; PxuiStr renamed; int64_t chosen; } PxuiHeadOut;
typedef struct { float rect[4]; int16_t clip[4]; uint8_t color[4], color2[4]; uint32_t aux[2]; uint16_t radii[4]; uint16_t width; uint8_t kind, flags; uint16_t view, clip_radius; uint32_t extra[2]; } PxuiInstance; /* 64 bytes; layouts are frozen from the first release, which has not happened yet */
/* One layer of the last frame (a floating root's, in draw order): its root box key, retained id, instances (an empty layer draws nothing),
   texture break count and the instance ranges changed since it was last drawn. Borrowed until the next frame. */
typedef struct { uint32_t start, end; } PxuiDirty;
typedef struct { const PxuiDirty *ptr; size_t len; } PxuiDirtySpan;
/* ABI 1.22: kind 9 is a filled triangle; rect = Ax,Ay,Bx,By, extra = Cx,Cy as f32 bits. */
typedef struct { const PxuiInstance *ptr; size_t len; } PxuiInstanceSpan;
typedef struct { int64_t key; uint32_t id, breaks; PxuiInstanceSpan instances; PxuiDirtySpan dirty; } PxuiLayerInfo;
typedef struct { const PxuiLayerInfo *ptr; size_t len; } PxuiLayerSpan;
typedef struct { uint32_t size,reserved; uint64_t draw_revision; double repaint_at; uint64_t built_nodes,rebuilt_views,arranged_nodes; PxuiPlatformOut platform; PxuiDamage damage; PxuiA11yTree a11y; PxuiLayerSpan layers; } PxuiOutput;
/* A window into a layer's instance storage: the caller stores 64-byte instances at at (stamping clip, clip_radius_f16 and view) and
   advances it; pxui_paint_span commits what was written (at is where it stopped) and hands out the next window. */
typedef struct { uint32_t size; uint16_t view,clip_radius_f16; int16_t clip[4]; PxuiInstance *at,*end; } PxuiSpan;
typedef void (*PxuiSlideText)(void *user, float fraction, PxuiStr *out);
typedef void (*PxuiScrubText)(void *user, PxuiStr origin, float dx, uint8_t coarse, PxuiStr *out);
/* flags: 1 left, 2 edit, 4 bare, 8 kept_drag, 16 has_size, 32 has_fraction, 64 has_tracking, 128 has_line, 256 has_lead, 512 has_trail, 1024 has_display, 2048 has_placeholder */
typedef struct { uint32_t size, flags; int64_t font_size; float fraction, tracking; PxuiColor line, lead_color, trail_color; uint8_t reserved[4]; float x,y,width,height; PxuiStr display,placeholder,lead,trail,label,value; } PxuiValueFieldDesc;
typedef PxuiRect (*PxuiHit)(void *user, PxuiRect rect);
typedef void (*PxuiRelease)(void *user);
/* A box declaration: axis is 0 row, 1 column, 2 stack. The hit callback and its release belong to the box's slot once the call is made. */
typedef struct { uint32_t size, axis; int64_t flags; PxuiSizeDesc w,h; float max_h,padding,scroll_step; uint8_t has_at,has_hit_rect,reserved[2]; float at[2]; PxuiRect hit_rect; PxuiHit hit; void *hit_user; PxuiRelease hit_release; } PxuiBoxDesc;
/* The callback UI pointer is borrowed and valid only for that callback; do not retain or destroy it. Only the handle of the innermost
   running callback may be used: while a container's callback runs, the handle of the callback it was called from, and the owner handle,
   are refused (PXUI_ERR_STATE, and zero or null for a call that returns a value, SIZE_MAX or UINT32_MAX for the pxui_text_* calls whose
   zero is an answer; pxui_last_error says why) before the engine is touched.
   Between callbacks of a frame (the engine's own phases) no handle is usable. pxui_last_error, pxui_ui_output (which builds nothing in
   a frame) and pxui_ui_destroy / pxui_frame / pxui_ui_set_theme (which refuse in a frame themselves) stay callable on the owner. Outside
   a frame the owner is the handle. A renderer's calls that reach the Ui (prepare, draw) are refused inside a frame too. */
typedef void (*PxuiBuild)(void *user, PxuiUi *ui);
typedef void (*PxuiPaintBuild)(void *user, PxuiPaint *paint, PxuiRect rect, const PxuiPaintInfo *info); /* info: valid during the call */
typedef void (*PxuiPaintRelease)(void *user);
typedef void (*PxuiSpanBuild)(void *user, float x, float width);
typedef void (*PxuiPickerRows)(void *user, PxuiStr query, PxuiStrSpan *out);
typedef uint8_t (*PxuiTextValid)(void *user, PxuiStr value);
typedef void (*PxuiCollectionEmit)(void *user, PxuiStr text);
typedef void (*PxuiCollectionText)(void *user, size_t row, size_t column, PxuiCollectionEmit emit, void *emit_user);
typedef PxuiStr (*PxuiTableCell)(void *user, int64_t row, int64_t column);
/* Collection flags (selected, open, a node's children) are bytes: not 0 is set; the library writes 0 or 1 and scans none.
   A tree writes its shown rows into the start of `visible` (at least a row per node) and their number to `visible_len`. */
typedef struct { uint32_t size; uint8_t multi,reserved[3]; PxuiStr label; size_t count; PxuiU8MutSpan selected; PxuiCollectionText text; void *user; } PxuiListViewDesc;
typedef struct { uint32_t size,reserved; PxuiStr label; const PxuiTreeNode *ptr; size_t len; PxuiU8MutSpan open; size_t *selected; PxuiU32MutSpan visible; size_t *visible_len; PxuiCollectionText text; void *user; } PxuiTreeViewDesc;
typedef struct { uint32_t size; uint8_t has_sort,ascending,reserved[2]; PxuiStr label; PxuiStrSpan titles; PxuiF32MutSpan widths; size_t rows; size_t sort_column; size_t *selected; PxuiCollectionText text; void *user; } PxuiTableViewDesc;
typedef struct { uint32_t size,reserved; PxuiStr label; float at[2],width,height; PxuiStrSpan headers; int64_t rows; PxuiTableCell cell; void *user; } PxuiTableDesc;
typedef struct { uint32_t size,reserved; PxuiStr title; PxuiStrSpan items; } PxuiMenuDesc;
typedef struct { const PxuiMenuDesc *ptr; size_t len; } PxuiMenuDescSpan;
typedef void (*PxuiGridBuild)(void *user, PxuiUi *ui, size_t index);
typedef struct { uint32_t size,reserved; PxuiStr label; size_t count; float cell_w,cell_h; PxuiGridBuild build; void *user; } PxuiGridListDesc;
/* build: SIZE_MAX for leading content, SIZE_MAX-1 for the sticky heading, otherwise a row index. */
/* ABI 1.49: `key` returns the item id of row `index`; row boxes are keyed by it (a row's state follows its item). A record
   ending before `key` (56-byte `user` end, size 64 without it) or a null `key` keys rows by index. */
typedef int64_t (*PxuiRowKey)(void *user, size_t index);
typedef struct { uint32_t size; uint8_t overlay_scrollbar,reserved[3]; PxuiStr label; size_t count; float row_height,leading_height,header_height,trailing_height; PxuiGridBuild build; void *user; PxuiRowKey key; } PxuiSectionListDesc;
typedef void (*PxuiMeasuredSectionBuild)(void *user, void *ui, size_t index, float content_y);
typedef struct { uint32_t size; uint8_t overlay_scrollbar,reserved[3]; PxuiStr label; size_t count; float row_height,leading_height,header_height,trailing_height; PxuiMeasuredSectionBuild build; void *user; PxuiRowKey key; } PxuiMeasuredSectionDesc;
typedef struct { uint32_t size,axis; PxuiStr label; float ratio,min_a,min_b; uint32_t reserved; PxuiBuild build_a; void *user_a; PxuiBuild build_b; void *user_b; } PxuiSplitDesc;
typedef struct { uint32_t size,reserved; PxuiStr label; PxuiMenuDescSpan menus; } PxuiMenuBarDesc;
typedef struct { uint32_t size,reserved; size_t menu,item; } PxuiMenuOut;
typedef struct { uint32_t size,reserved; PxuiStr key,value; } PxuiHintDesc;
typedef struct { uint32_t size; uint8_t has_right,reserved[3]; PxuiStr right; const PxuiHintDesc *ptr; size_t len; } PxuiFooterDesc;
typedef struct { uint32_t size; float time; uint32_t target,reserved; float values[4]; PxuiLookDesc look; PxuiCurveDesc curve; } PxuiKeyframeDesc;
typedef struct { const PxuiKeyframeDesc *ptr; size_t len; } PxuiKeyframeSpan;
typedef struct { const PxuiHintDesc *ptr; size_t len; } PxuiHintSpan;
typedef struct { uint32_t size; uint8_t has_width,has_pin,pin,reserved; PxuiStr key,label; float width; uint32_t reserved_tail; PxuiBox row; float control_x,control_y,control_w; } PxuiInspectorRowDesc;
typedef struct { uint32_t size; uint8_t expanded,has_set,set_expanded,reserved; PxuiStr key,label; PxuiBuild build; void *user; uint8_t out_open; } PxuiInspectorSectionDesc;
typedef struct { uint32_t size; uint8_t has_width,reserved[3]; PxuiStr key,label,value; float width; } PxuiInspectorValueDesc;
typedef struct { uint32_t size,reserved; PxuiStr key,text; } PxuiInspectorTextDesc;
enum PxuiSplitAxis { PXUI_SPLIT_ROW=0, PXUI_SPLIT_COLUMN=1 };
enum PxuiKeyframeTarget { PXUI_KEY_OFFSET=0, PXUI_KEY_OPACITY=1, PXUI_KEY_LOOK=2, PXUI_KEY_SCALE=3 }; /* scale values: x, y, fractional anchor x/y; requires a view with its own painter entry */

uint32_t pxui_abi_version(void);
/* Process counters for performance gates: which 0 = calls made into the library (this one included), 1 = heap allocations the library's allocator has made; anything else reads 0. */
uint64_t pxui_counter(uint32_t which);
PxuiUi *pxui_ui_create(const PxuiUiDesc *desc);
PxuiUi *pxui_ui_create_with_fonts(const PxuiUiDesc *desc, const PxuiStr *fonts, size_t count);
int32_t pxui_ui_destroy(PxuiUi *ui); /* PXUI_ERR_STATE: borrowed handle, inside a frame, or a renderer still attached */
PxuiStr pxui_last_error(const PxuiUi *ui);
int32_t pxui_ui_environment(PxuiUi *ui, PxuiEnvironment *out);
int32_t pxui_ui_keyboard(PxuiUi *ui, PxuiKeyboard *out);
int32_t pxui_ui_text_input_options(PxuiUi *ui, PxuiTextInputOptions *out);
int32_t pxui_ui_content_rect(PxuiUi *ui, PxuiRect *out);
int32_t pxui_frame(PxuiUi *ui, const PxuiFrameIn *in, PxuiBuild build, void *user, PxuiFrameOut *out);
int32_t pxui_ui_frame(PxuiUi *ui, const PxuiInput *in, PxuiBuild build, void *user, PxuiFrameOut *out);
/* The last frame's input as routed to `owner` (a viewport's root key; has_owner 0: the app's), for the app's own content: the record
   pxui_ui_frame takes, borrowed until the next call. PXUI_ERR_STATE before the first frame. */
int32_t pxui_ui_input(PxuiUi *ui, uint8_t has_owner, int64_t owner, PxuiInput *out);
/* Marks layer `layer` of the last frame drawn (its dirty ranges clear), as pxui_renderer_draw does: for a host that reads the layers itself. */
int32_t pxui_ui_layer_drawn(PxuiUi *ui, uint64_t layer);
const PxuiOutput *pxui_ui_output(PxuiUi *ui); /* borrowed from the Ui until its next frame */
/* The theme in effect, lent: valid until the Ui next changes it (a theme swap, a themed scope) or is freed. NULL when the call is refused
   (a poisoned Ui, a handle that is not the current one): see pxui_last_error. */
const PxuiThemeSnapshot *pxui_ui_theme(PxuiUi *ui);
/* Measured virtual rows: count <= 1,000,000, finite positive estimate and finite estimated total.
   Called inside a frame; build may repeat while measured heights settle. The returned box scrolls the list. */
int32_t pxui_ui_list_measured(PxuiUi *ui, PxuiStr label, size_t count, float estimate_h, PxuiGridBuild build, void *user, PxuiBox *out);
int32_t pxui_ui_grid_list(PxuiUi *ui, const PxuiGridListDesc *desc, PxuiBox *out);
/* One scroll region with fixed rows and a scrolling lead-in/sticky heading (ABI 1.20). */
int32_t pxui_ui_section_list(PxuiUi *ui, const PxuiSectionListDesc *desc, PxuiBox *out);
/* Measured virtual rows; row_height estimates unseen rows (ABI 1.21). */
int32_t pxui_ui_section_list_measured(PxuiUi *ui, const PxuiMeasuredSectionDesc *desc, PxuiBox *out);
int32_t pxui_ui_split(PxuiUi *ui, const PxuiSplitDesc *desc, uint8_t *changed, float *ratio);
int32_t pxui_ui_menu_bar(PxuiUi *ui, const PxuiMenuBarDesc *desc, PxuiMenuOut *out);
int32_t pxui_ui_inspector_row(PxuiUi *ui, PxuiInspectorRowDesc *desc);
int32_t pxui_ui_inspector_section(PxuiUi *ui, PxuiInspectorSectionDesc *desc);
int32_t pxui_ui_inspector_toggle(PxuiUi *ui, PxuiStr key, PxuiStr label, uint8_t value, uint8_t *out);
int32_t pxui_ui_inspector_button(PxuiUi *ui, PxuiStr key, PxuiStr label, uint8_t *out);
int32_t pxui_ui_inspector_readout(PxuiUi *ui, const PxuiInspectorValueDesc *desc);
int32_t pxui_ui_inspector_message(PxuiUi *ui, const PxuiInspectorTextDesc *desc);
int32_t pxui_ui_view_size(PxuiUi *ui, PxuiPoint *out);
int32_t pxui_ui_wants_frame(PxuiUi *ui, PxuiEventSpan events, double now, uint8_t *out);
int32_t pxui_ui_footer(PxuiUi *ui, const PxuiFooterDesc *desc);
int32_t pxui_ui_animate_keys(PxuiUi *ui, const PxuiBox *box, PxuiKeyframeSpan keys, uint8_t *out);
/* ABI 1.30: repeat 0 means forever, otherwise 1..65535 cycles; autoreverse is 0/1. */
int32_t pxui_ui_animate_keys_repeat(PxuiUi *ui, const PxuiBox *box, PxuiKeyframeSpan keys, uint32_t repeat, uint8_t autoreverse, float start_offset, uint8_t *out);
int32_t pxui_ui_table(PxuiUi *ui, const PxuiTableDesc *desc, int64_t *clicked_row);
/* Last host appearance; themes remain authored by the application. use_theme selects a parsed root
   theme between frames, avoiding reparsing source when appearance changes. */
int32_t pxui_ui_system_appearance(PxuiUi *ui, uint32_t *out);
int32_t pxui_ui_use_theme(PxuiUi *ui, const PxuiTheme *theme);
/* ABI 1.43: app colour tokens. set_colors installs light[i]/dark[i] as token i between frames (count at most 64, 0 clears;
   a changed table rebuilds every view once). color reads a token for the theme in effect: the dark entry while that theme
   is dark (panel channel sum below 384), else the light one; transparent past the table or on a dead handle. */
int32_t pxui_ui_set_colors(PxuiUi *ui, const PxuiColor *light, const PxuiColor *dark, size_t count);
PxuiColor pxui_ui_color(PxuiUi *ui, uint16_t token);
int32_t pxui_ui_set_theme(PxuiUi *ui, const PxuiThemeSource *source);
int32_t pxui_ui_set_dir(PxuiUi *ui, uint32_t dir);
int32_t pxui_ui_set_a11y(PxuiUi *ui, uint8_t on);
int32_t pxui_ui_inject_a11y(PxuiUi *ui, const PxuiA11yAction *actions, size_t count);
int32_t pxui_ui_a11y_name(PxuiUi *ui, const PxuiBox *box, PxuiStr name);
int32_t pxui_ui_a11y_value(PxuiUi *ui, const PxuiBox *box, PxuiStr value, uint32_t anchor, uint32_t caret);
int32_t pxui_ui_a11y_number(PxuiUi *ui, const PxuiBox *box, double now, double min, double max, double step);
int32_t pxui_ui_set_role(PxuiUi *ui, const PxuiBox *box, uint32_t role, uint32_t states);
int32_t pxui_ui_enter(PxuiUi *ui, const PxuiBox *box, PxuiSaved *out);
int32_t pxui_ui_leave(PxuiUi *ui, const PxuiSaved *saved);
int32_t pxui_ui_focus_box(PxuiUi *ui, const PxuiBox *box);
int32_t pxui_ui_focused(PxuiUi *ui, const PxuiBox *box, uint8_t *out);
int32_t pxui_ui_state(PxuiUi *ui, const PxuiBox *box, int64_t default_value, int64_t *out);
int32_t pxui_ui_set_state(PxuiUi *ui, const PxuiBox *box, int64_t value);
/* ABI 1.10 retains numeric routing storage; signal events remain borrowed through the next frame. */
int32_t pxui_ui_signal(PxuiUi *ui, const PxuiBox *box, PxuiSignal *out);
int32_t pxui_ui_patch_text(PxuiUi *ui, const PxuiBox *box, PxuiStr value, uint8_t *out); /* out: whether the text was written; a stale box is PXUI_ERR_STALE */
int32_t pxui_ui_busy_step(PxuiUi *ui, const PxuiBox *box, double now, uint8_t *out);
int32_t pxui_ui_request_frame(PxuiUi *ui);
int32_t pxui_ui_node(PxuiUi *ui, PxuiStr label, const PxuiNode *node, PxuiBox *out); /* node->size = sizeof(PxuiNode); copied in the call */
int32_t pxui_ui_node_keyed(PxuiUi *ui, int64_t key, const PxuiNode *node, PxuiBox *out); /* keys 0 and 1 are reserved */
int32_t pxui_ui_rect(PxuiUi *ui, const PxuiBox *box, PxuiRect *out);
uint8_t pxui_ui_box_of_key(PxuiUi *ui, int64_t key, PxuiBox *out);
/* ABI 1.30: 1 and the live box, or 0 without changing out for a retired/unknown identity. */
uint8_t pxui_ui_box_of_a11y(PxuiUi *ui, uint64_t id, PxuiBox *out);
int32_t pxui_ui_text(PxuiUi *ui, const PxuiBox *box, PxuiStr value);
int32_t pxui_ui_set_look(PxuiUi *ui, const PxuiBox *box, const PxuiLookDesc *look, const PxuiStateDesc *state);
int32_t pxui_ui_style(PxuiUi *ui, const PxuiBox *box, const PxuiLookDesc states[6], const PxuiStateDesc *state);
float pxui_ui_text_width(PxuiUi *ui, uint8_t has_size, int64_t size, PxuiStr text);
int32_t pxui_ui_draw(PxuiUi *ui, const PxuiBox *box, uint8_t over, PxuiPaintBuild build, void *user, PxuiPaintRelease release); /* a span must be committed (pxui_paint_span with at_least 0) before the callback returns; one left open, or a pxui_paint_* call made under an open span, is refused and the frame that ran the callback returns PXUI_ERR_STATE */
int32_t pxui_within(PxuiUi *ui, const PxuiBox *box, PxuiBuild build, void *user);
int32_t pxui_scope(PxuiUi *ui, PxuiStr label, PxuiBuild build, void *user);
int32_t pxui_col(PxuiUi *ui, float padding, PxuiStr label, PxuiBuild build, void *user);
int32_t pxui_panel(PxuiUi *ui, const PxuiPanel *panel, PxuiStr label, PxuiBuild build, void *user);
/* Legacy: superseded by pxui_ui_label_node (ABI 1.54); kept for the append-only ABI. */
int32_t pxui_label(PxuiUi *ui, PxuiStr text);
/* Legacy since ABI 1.54: superseded by pxui_ui_label_node; kept for the append-only ABI.
   ABI 1.45: a label in `style` through the label body (PxuiLabelConfig.style), keyed by its place among its siblings (as
   pxui_label now is). A null or short style, unknown set bits, invalid UTF-8, a non-finite or non-positive size, an invalid
   wrap, align, ellipsis, feature or width, or a font the Ui lacks is refused before anything is built. */
int32_t pxui_ui_label(PxuiUi *ui, PxuiStr text, const PxuiLabelStyle *style);
/* ABI 1.54: pxui_ui_label with the label's box (null for none) writing the label's node to `out` (for a fill, an accessible state or
   text laid into it); a null style is the theme's label, as pxui_label. A short box, unknown set bits, a non-finite height or an
   unknown role is refused. */
int32_t pxui_ui_label_node(PxuiUi *ui, PxuiStr text, const PxuiLabelStyle *style, const PxuiLabelBox *label_box, PxuiBox *out);
/* ABI 1.49: the PxuiTextStyle `style` shapes with, resolved against the Ui's theme label as pxui_ui_label resolves it (unset fields
   the theme label's, tracking in points); `digits` is ignored. Refused for a style pxui_ui_label would refuse. */
int32_t pxui_ui_resolve_label(PxuiUi *ui, const PxuiLabelStyle *style, PxuiTextStyle *out);
int32_t pxui_button(PxuiUi *ui, PxuiStr label, uint8_t *out);
int32_t pxui_toggle(PxuiUi *ui, PxuiStr label, uint8_t on, uint8_t *out);
int32_t pxui_text_field(PxuiUi *ui, PxuiStr label, PxuiStr value, PxuiStr *changed, uint8_t *out_changed); /* changed: the new text, borrowed until the next frame, when out_changed */
int32_t pxui_text_field_with_options(PxuiUi *ui, PxuiStr label, PxuiStr value, const PxuiTextInputOptions *options, PxuiStr *changed, uint8_t *out_changed);
int32_t pxui_ui_checkbox(PxuiUi *ui, PxuiStr label, uint8_t value, uint8_t *out);
int32_t pxui_ui_radio(PxuiUi *ui, PxuiStr label, PxuiStrSpan options, size_t selected, size_t *out);
int32_t pxui_ui_tabs(PxuiUi *ui, PxuiStr label, PxuiStrSpan options, size_t selected, size_t *out);
int32_t pxui_ui_segmented(PxuiUi *ui, PxuiStr label, PxuiStrSpan options, size_t selected, size_t *out);
int32_t pxui_ui_dropdown(PxuiUi *ui, PxuiStr label, PxuiStrSpan options, size_t selected, size_t *out);
int32_t pxui_ui_slider(PxuiUi *ui, PxuiStr label, float value, float min, float max, float *out);
/* Caller-painted horizontal control: finite ordered range (equal ends are read-only), positive finite step. */
int32_t pxui_ui_scrub(PxuiUi *ui, PxuiStr label, float value, float min, float max, float step, PxuiScrubOut *out);
int32_t pxui_ui_progress(PxuiUi *ui, PxuiStr label, uint8_t has_value, float value, PxuiBox *out);
int32_t pxui_ui_spinner(PxuiUi *ui, PxuiStr label, PxuiBox *out);
int32_t pxui_ui_icon_button(PxuiUi *ui, PxuiStr label, PxuiStr icon, uint8_t *out);
int32_t pxui_ui_badge(PxuiUi *ui, PxuiStr label, PxuiStr value, PxuiBox *out);
int32_t pxui_ui_search_field(PxuiUi *ui, PxuiStr label, PxuiStr icon, PxuiStr value, PxuiStr placeholder, uint8_t *changed, PxuiStr *out);
int32_t pxui_ui_image(PxuiUi *ui, PxuiStr label, const PxuiImage *image, uint32_t fit, PxuiSizeDesc width, PxuiSizeDesc height, PxuiBox *out);
int32_t pxui_ui_modal(PxuiUi *ui, PxuiStr label, uint8_t has_width, float width, PxuiBuild build, void *user, uint8_t *out);
int32_t pxui_ui_scroll_view(PxuiUi *ui, PxuiStr label, PxuiSizeDesc width, PxuiSizeDesc height, PxuiBuild build, void *user, PxuiBox *out);
int32_t pxui_ui_view(PxuiUi *ui, PxuiStr label, uint64_t revision, PxuiSizeDesc width, PxuiSizeDesc height, PxuiBuild build, void *user, PxuiBox *out);
int32_t pxui_ui_anchored(PxuiUi *ui, PxuiStr label, const PxuiAnchorDesc *anchor, PxuiSizeDesc width, PxuiSizeDesc height, PxuiBuild build, void *user, PxuiBox *out);
int32_t pxui_ui_dialog(PxuiUi *ui, PxuiStr title, PxuiStrSpan buttons, PxuiBuild build, void *user, PxuiDialogOut *out);
int32_t pxui_ui_tip(PxuiUi *ui, const PxuiBox *box, PxuiStr label);
int32_t pxui_ui_toasts(PxuiUi *ui, PxuiStr label, PxuiStrSpan items, double seconds, int64_t *out_index);
typedef struct { uint8_t has, reserved[3]; PxuiColor color; } PxuiSwatch;
typedef struct { int64_t row, current; uint8_t has_current, reserved[7]; PxuiStrSpan rows; PxuiU8Span enabled; PxuiStrSpan keys; } PxuiSubmenu;
typedef struct { const PxuiSubmenu *ptr; size_t len; } PxuiSubmenuSpan;
typedef struct { const PxuiSwatch *ptr; size_t len; } PxuiSwatchSpan;
/* What a context menu has beyond its rows: keys and swatches per row (shorter than the rows is fine), the destructive rows, the
   submenus and the first row drawn without a leading mark. `extra` may be null. */
/* ABI 1.55: padding above the first row and below the last (a 104-byte record is 6), the sheet's name when has_name is 1, and selected per row
   (1: the current choice, CHECKED and SELECTED with the checked look). */
typedef struct { uint32_t size, reserved; int64_t lead_from; PxuiStrSpan keys; PxuiSwatchSpan swatches; PxuiI64Span danger; PxuiSubmenuSpan submenus; float row_h; uint32_t max_rows; PxuiU8Span checked;
                 float padding; uint8_t has_name, reserved2[3]; PxuiStr name; PxuiU8Span selected; } PxuiMenuExtra;
int32_t pxui_ui_context_menu(PxuiUi *ui, float x, float y, PxuiStr label, PxuiStrSpan items, PxuiU8Span enabled, uint8_t has_width, float width, uint8_t has_selected, int64_t selected, const PxuiMenuExtra *extra, uint32_t *out_tag, int64_t *out_index);
/* mark(user, row, &color) returns 1 and sets the colour of a row's accent square, else 0; off(user, row) returns 1 for a row drawn off. */
typedef uint8_t (*PxuiPickerMark)(void *user, int64_t row, PxuiColor *out);
typedef uint8_t (*PxuiPickerOff)(void *user, int64_t row);
int32_t pxui_ui_picker(PxuiUi *ui, int64_t limit, uint8_t slash, uint8_t at_rest, PxuiStr label, PxuiStr query, PxuiPickerRows rows, PxuiPickerMark mark, PxuiPickerOff off, void *user, PxuiStr *out_query, uint32_t *out_tag, int64_t *out_index);
int32_t pxui_ui_list_view(PxuiUi *ui, const PxuiListViewDesc *desc, size_t *activated);
int32_t pxui_ui_tree(PxuiUi *ui, const PxuiTreeViewDesc *desc, size_t *activated);
int32_t pxui_ui_table_view(PxuiUi *ui, const PxuiTableViewDesc *desc, size_t *sort_column);
/* Widget bodies: an app builds a widget kind's nodes itself. pxui_body_register names a body of kind `kind` (PxuiBodyKind, in the field order of
   PxuiBodies; append-only) `name`, for a theme's `KIND.body = NAME` (pxui_ui_set_theme, pxui_ui_theme_apply) and returns its id, 0 when refused
   (see pxui_last_error). A name has one id for the whole process, whichever Ui registers it, and is never released (65,535 names a kind at most, in the process; choose names statically, and register a name again to change its body: that keeps the id and uses none): a
   theme carried from the Ui that parsed it to another selects the body of that name there, or the built-in body where that Ui has none; the function is called as body(user, ui, config) with the kind's record below, `ui` borrowed and valid for the call.
   Records start with `size` and grow at the end. A body builds with the ordinary calls, finds the parts its widget's behaviour reads through
   pxui_part, and runs the caller's content through its slots (pxui_slot); a slot is valid only during the body call that received it. */
enum PxuiBodyKind { PXUI_BODY_TOGGLE=0, PXUI_BODY_SLIDER=1, PXUI_BODY_PROGRESS=2, PXUI_BODY_BUTTON=3, PXUI_BODY_ICON_BUTTON=4, PXUI_BODY_CHECKBOX=5, PXUI_BODY_RADIO=6, PXUI_BODY_SPINNER=7, PXUI_BODY_BADGE=8, PXUI_BODY_LABEL=9, PXUI_BODY_FOOTER=10, PXUI_BODY_GRIP=11, PXUI_BODY_PANEL=12, PXUI_BODY_WINDOW=13, PXUI_BODY_POPUP=14, PXUI_BODY_MODAL=15, PXUI_BODY_DIALOG=16, PXUI_BODY_INSPECTOR_SECTION=17, PXUI_BODY_INSPECTOR_ROW=18, PXUI_BODY_SPLIT=19, PXUI_BODY_SCROLL_VIEW=20, PXUI_BODY_TOAST=21, PXUI_BODY_TIP=22, PXUI_BODY_TAB=23, PXUI_BODY_SEGMENT=24, PXUI_BODY_DROPDOWN=25, PXUI_BODY_MENU_BAR=26, PXUI_BODY_MENU_ITEM=27, PXUI_BODY_CONTEXT_MENU=28, PXUI_BODY_LIST_VIEW=29, PXUI_BODY_TREE=30, PXUI_BODY_TABLE_VIEW=31, PXUI_BODY_TABLE_HEAD=32, PXUI_BODY_TABLE=33, PXUI_BODY_PICKER=34, PXUI_BODY_SEARCH_FIELD=35, PXUI_BODY_TEXT_FIELD=36, PXUI_BODY_LINE_FIELD=37, PXUI_BODY_VALUE_FIELD=38, PXUI_BODY_PICKER_FIELD=39 };
typedef struct { uint64_t serial; } PxuiSlot;
typedef void (*PxuiBodyFn)(void *user, PxuiUi *ui, const void *config);
typedef void (*PxuiCellBuild)(void *user, PxuiUi *ui, size_t row, size_t column);
typedef struct { uint32_t size; PxuiStateDesc state; uint8_t on; PxuiBox row; PxuiStr label; float row_h, switch_w, switch_h; const PxuiStates * track; const PxuiStates * knob; float inset; uint8_t press_row; PxuiStr detail; uint16_t variant; uint8_t has_style, reserved2[5]; PxuiLabelStyle style; } PxuiToggleConfig;
typedef struct { uint32_t size; PxuiStateDesc state; float fraction; PxuiBox track; PxuiStr label; uint16_t variant; } PxuiSliderConfig;
typedef struct { uint32_t size; uint8_t has_value; float value; PxuiBox row; PxuiStr label; uint16_t variant; } PxuiProgressConfig;
typedef struct { uint32_t size; PxuiStateDesc state; uint8_t bare; uint8_t icon; uint8_t at_end; PxuiBox line; PxuiBox row; const PxuiStates * states; PxuiStr label; uint8_t w_kind,h_kind,justify,reserved; float w,h,padding[4]; uint16_t variant; uint8_t centered,reserved2[9];
                 /* ABI 1.56 */ uint8_t has_label,axis,content_align,reserved3; float gap; PxuiLabelStyle text_style; PxuiLabelBox text_box; } PxuiButtonConfig;
typedef struct { uint32_t size; PxuiStateDesc state; PxuiBox node; PxuiStr label; PxuiStr icon; const PxuiStates *states; uint8_t w_kind,h_kind,reserved; float w,h,press_scale; PxuiIconDraw draw; void *draw_user; PxuiColor ink; uint16_t variant; const PxuiCurve *press; } PxuiIconButtonConfig;
typedef struct { uint32_t size; PxuiStateDesc state; uint8_t on; PxuiBox row; PxuiStr label; uint16_t variant; } PxuiCheckConfig;
typedef struct { uint32_t size; PxuiBox node; PxuiStr label; uint16_t variant; } PxuiSpinnerConfig;
typedef struct { uint32_t size; PxuiBox node; PxuiStr label; PxuiStr text; uint16_t variant; } PxuiBadgeConfig;
/* ABI 1.45: grows at its end by style, the label's style as given (unset fields: the theme's); read it only when size covers it. */
typedef struct { uint32_t size; uint8_t first; PxuiBox node; PxuiStr text; PxuiLabelStyle style; uint16_t variant; PxuiLabelBox label_box; } PxuiLabelConfig;
typedef struct { uint32_t size; PxuiBox row; uint8_t has_right; PxuiStr right; PxuiHintSpan hints; uint16_t variant; } PxuiFooterConfig;
typedef struct { uint32_t size; PxuiStateDesc state; uint8_t vertical; uint8_t floating; PxuiBox grip; uint16_t variant; } PxuiGripConfig;
typedef struct { uint32_t size; PxuiBox node; PxuiStr label; PxuiSlot content; uint16_t variant; } PxuiPanelConfig;
/* ABI 1.53 appends default_at, cancel_at (a button index or -1) and bitmasks danger, disabled (bit i: button i), before variant */
typedef struct { uint32_t size; PxuiStr title; PxuiStrSpan buttons; PxuiSlot content; int64_t default_at, cancel_at; uint64_t danger, disabled; uint16_t variant; } PxuiDialogConfig;
typedef struct { uint32_t size; uint8_t open; float gap; PxuiBox title; PxuiStr label; PxuiSlot content; uint16_t variant; } PxuiSectionConfig;
typedef struct { uint32_t size; PxuiStateDesc state; uint8_t has_pin; uint8_t pin; float label_w; PxuiBox row; PxuiStr label; uint16_t variant; } PxuiInspectorRowConfig;
typedef struct { uint32_t size; uint32_t axis; float ratio; float min_a; float min_b; PxuiBox split; PxuiSlot first; PxuiSlot second; uint16_t variant; } PxuiSplitConfig;
typedef struct { uint32_t size; PxuiBox node; PxuiSlot content; uint16_t variant; } PxuiScrollConfig;
typedef struct { uint32_t size; PxuiStateDesc state; PxuiBox node; PxuiStr text; uint16_t variant; } PxuiToastConfig;
typedef struct { uint32_t size; float width; float height; PxuiBox tip; PxuiStrSpan lines; uint16_t variant; } PxuiTipConfig;
typedef struct { uint32_t size; PxuiStateDesc state; size_t index; PxuiBox node; PxuiStr text; uint16_t variant; } PxuiTabConfig;
typedef struct { uint32_t size; PxuiStateDesc state; size_t index; size_t selected; int64_t group; PxuiBox node; PxuiStr text; const PxuiStates * states; uint8_t grow; uint16_t variant; float padding, min_segment; } PxuiSegmentConfig;
typedef struct { uint32_t size; PxuiStateDesc state; uint8_t open; int64_t key; PxuiBox button; PxuiStr text; uint16_t variant; } PxuiDropdownConfig;
typedef struct { uint32_t size; PxuiStateDesc state; size_t index; PxuiBox node; PxuiStr text; uint16_t variant; } PxuiMenuItemConfig;
typedef struct { uint32_t size; PxuiStateDesc state; uint8_t selected; uint8_t chevron; uint8_t danger; uint8_t has_swatch; PxuiColor swatch; uint8_t has_shortcut; int64_t index; float lead; float swatch_pad; PxuiBox row; PxuiStr text; PxuiStr shortcut; float row_h; uint16_t variant; } PxuiMenuRowConfig;
typedef struct { uint32_t size; PxuiStateDesc state; size_t index; PxuiBox row; PxuiSlot content; uint16_t variant; } PxuiRowConfig;
typedef struct { uint32_t size; PxuiStateDesc state; uint8_t has; uint8_t open; size_t index; size_t node; size_t depth; PxuiBox row; PxuiSlot content; uint16_t variant; } PxuiTreeRowConfig;
typedef struct { uint32_t size; PxuiStateDesc state; uint8_t has_ascending; uint8_t ascending; size_t index; PxuiBox node; PxuiStr title; uint16_t variant; } PxuiTableHeadConfig;
typedef struct { uint32_t size; uint8_t header; float left; float right; float height; int64_t row; int64_t column; int64_t key; PxuiStr text; uint16_t variant; } PxuiTableCellConfig;
typedef struct { uint32_t size; PxuiStateDesc state; uint8_t current; uint8_t doomed; uint8_t unavailable; uint8_t marked; uint8_t has_swatch; PxuiColor swatch; int64_t index; PxuiBox row; PxuiStr label; PxuiStr detail; PxuiStr query; uint16_t variant; } PxuiPickerRowConfig;
typedef struct { uint32_t size; PxuiStateDesc state; int64_t key; PxuiBox well; PxuiStr icon; PxuiSlot editor; uint16_t variant; } PxuiSearchFieldConfig;
typedef struct { uint32_t size; PxuiStateDesc state; PxuiBox row; PxuiStr label; PxuiStr value; uint16_t variant; } PxuiTextFieldConfig;
typedef struct { uint32_t size; PxuiStateDesc state; uint8_t empty; PxuiBox field; PxuiStr text; uint16_t variant; } PxuiLineFieldConfig;
typedef struct { uint32_t size; PxuiStateDesc state; uint8_t invalid; float w; float h; PxuiBox field; PxuiValueFieldDesc opts; PxuiStr text; uint16_t variant; } PxuiValueFieldConfig;
typedef struct { uint32_t size; PxuiStateDesc state; uint8_t empty; uint8_t slash; float prefix; float inset; PxuiBox search; PxuiStr placeholder; uint16_t variant; } PxuiPickerFieldConfig;
uint16_t pxui_body_register(PxuiUi *ui, uint32_t kind, PxuiStr name, PxuiBodyFn body, void *user);
/* Builds a node as part `part` (PXUI_PART_*) of the widget whose body is running; PXUI_ERR_STATE outside a body call. */
int32_t pxui_part(PxuiUi *ui, uint32_t part, const PxuiNode *node, PxuiBox *out);
/* The box of `part` of the widget keyed `owner` as the last build left it: 0 and the box in `out`, 1 when it was not built. */
int32_t pxui_part_box(PxuiUi *ui, int64_t owner, uint32_t part, PxuiBox *out);
/* Runs a slot of the body call in progress; `out` (may be NULL) gets a search field's editor answer (1: the query is not empty). PXUI_ERR_STALE for a slot of a finished call or another Ui, PXUI_ERR_STATE inside its own content or a nested body. */
int32_t pxui_slot(PxuiUi *ui, PxuiSlot slot, uint8_t *out);
enum PxuiPart { PXUI_PART_HIT=1, PXUI_PART_RAIL=2, PXUI_PART_THUMB=3, PXUI_PART_TRACK=4, PXUI_PART_BAR=5, PXUI_PART_TITLE=6, PXUI_PART_DISCLOSURE=7, PXUI_PART_CLEAR=8, PXUI_PART_EDITOR=9, PXUI_PART_DOT0=16 /* dot i: 16 + i, i < 256 */, PXUI_PART_BUTTON0=1024 /* dialog button i: 1024 + i, i < 256 */ };
typedef struct { uint32_t size; uint8_t multi,reserved[3]; PxuiStr label; size_t count; PxuiU8MutSpan selected; PxuiCellBuild build; void *user; } PxuiListViewWithDesc;
typedef struct { uint32_t size,reserved; PxuiStr label; const PxuiTreeNode *ptr; size_t len; PxuiU8MutSpan open; size_t *selected; PxuiU32MutSpan visible; size_t *visible_len; PxuiCellBuild build; void *user; } PxuiTreeViewWithDesc;
typedef struct { uint32_t size; uint8_t has_sort,ascending,reserved[2]; PxuiStr label; PxuiStrSpan titles; PxuiF32MutSpan widths; size_t rows; size_t sort_column; size_t *selected; PxuiCellBuild build; void *user; } PxuiTableViewWithDesc;
/* The three collections with the caller's own rows or cells: `build` builds the children of one row or cell (`column` 0 in a list or a tree) inside the row's or cell's node. */
int32_t pxui_ui_list_view_with(PxuiUi *ui, const PxuiListViewWithDesc *desc, size_t *activated);
int32_t pxui_ui_tree_with(PxuiUi *ui, const PxuiTreeViewWithDesc *desc, size_t *activated);
int32_t pxui_ui_table_view_with(PxuiUi *ui, const PxuiTableViewWithDesc *desc, size_t *sort_column);
/* ABI 1.49: from inside a list_view/tree row or table_view cell callback of the _with calls, the kit's text of that item: a list
   row's row label, a tree row's (naming the row), a table cell's (naming the cell). Refused outside a collection item. */
int32_t pxui_ui_item_text(PxuiUi *ui, PxuiStr text);
typedef struct { int64_t start, end; } PxuiRange;
typedef struct { int64_t start, end; PxuiColor color; uint32_t reserved; } PxuiColorSpan;
typedef struct { int64_t line, start, end; uint8_t has_range, reserved[7]; PxuiStr text; } PxuiNote;
typedef struct { int64_t start, end; PxuiStr text; } PxuiSplice;
typedef struct { int64_t start, end; PxuiStr insert, label, detail, doc; } PxuiCompletion;
typedef struct { uint8_t has, reserved[7]; int64_t start, end; PxuiStr text; } PxuiDescription;
typedef struct { uint8_t has, reserved[7]; int64_t start, end; } PxuiMaybeRange;
typedef struct { PxuiStr text; int64_t caret; } PxuiRewrite;
typedef struct { const PxuiNote *ptr; size_t len; } PxuiNoteSpan;
typedef struct { const PxuiRange *ptr; size_t len; } PxuiRangeSpan;
typedef struct { const PxuiSplice *ptr; size_t len; } PxuiSpliceSpan;
typedef struct { const PxuiColorSpan *ptr; size_t len; } PxuiColorSpanSpan;
/* A text area (flags: 1 wrap, 2 reveal, 4 select, 8 language, 16 rewrite): its place, its text and the data options;
   `comment` and `pairs` (opener, closer as Unicode scalar values) are the language's. */
typedef struct {
    uint32_t size, flags; float x, y, width, height; int64_t reveal, reveal_nonce; PxuiRange select; PxuiStr label, value;
    PxuiI64Span errors; PxuiNoteSpan messages; PxuiRangeSpan spans; PxuiSpliceSpan splice; PxuiColorSpanSpan chips; PxuiStr comment; PxuiU32Span pairs;
} PxuiTextAreaDesc;
typedef struct { const PxuiCompletion *ptr; size_t len; } PxuiCompletionSpan;
/* The app's closures of a text area (TextAreaOpts::on_* and Language), called only during the widget's call with `user`. A callback
   that returns data writes it to its `out`, borrowed until its next callback. The language members are all set or all null (rewrite
   alone may be null); on_scrub gets 0 for live, 1 for done; on_drop 0 for hover, 1 for dropped; context_at returns 0 code, 1 string, 2 comment. */
typedef struct {
    uint32_t size, reserved; void *user;
    void (*on_context)(void *user, float x, float y); void (*on_scrub)(void *user, uint32_t phase);
    void (*on_scrub_edit)(void *user, int64_t start, int64_t end, PxuiStr text); void (*on_click)(void *user, int64_t at, uint8_t flag);
    void (*on_caret)(void *user, int64_t at); void (*on_caret_move)(void *user, int64_t at); void (*on_select)(void *user, int64_t start, int64_t end);
    void (*on_drop)(void *user, int64_t at, uint8_t dropped, PxuiStr kind, PxuiStr value);
    void (*colorize)(void *user, PxuiStr text, PxuiColorSpanSpan *out); void (*brackets)(void *user, PxuiStr text, PxuiRangeSpan *out);
    void (*indent)(void *user, PxuiStr text, int64_t caret, PxuiStr *out); void (*complete)(void *user, PxuiStr text, int64_t caret, PxuiCompletionSpan *out);
    void (*describe)(void *user, PxuiStr text, int64_t at, PxuiDescription *out); void (*number_at)(void *user, PxuiStr text, int64_t at, PxuiMaybeRange *out);
    uint32_t (*context_at)(void *user, PxuiStr text, int64_t caret); void (*rewrite)(void *user, PxuiStr text, int64_t caret, PxuiRewrite *out);
} PxuiAreaCallbacks;
int32_t pxui_ui_text_area_submit(PxuiUi *ui, const PxuiTextAreaDesc *desc, const PxuiAreaCallbacks *callbacks, PxuiStr *out_text, uint8_t *out_changed, uint8_t *out_submitted);
int32_t pxui_ui_patch_field(PxuiUi *ui, const PxuiBox *field, const PxuiValueFieldDesc *desc, PxuiStr text, uint8_t *out); /* the field's value and soft-range line written by patches, as its build lays them out; out is 0 when a patch cannot show it (a lead, a value too wide beside its trail): build the field's view */
int32_t pxui_ui_value_field(PxuiUi *ui, const PxuiValueFieldDesc *desc, PxuiTextValid valid, PxuiSlideText slide, PxuiScrubText scrub, void *user, PxuiStr *out_text, uint8_t *out_changed, uint8_t *out_editing); /* the callbacks run only during the call; slide and scrub write their text, borrowed until their next callback */
int32_t pxui_ui_load_text_edit(PxuiUi *ui, int64_t key, PxuiStr text, PxuiStr *out_text, uint64_t *caret, uint64_t *anchor);
int32_t pxui_ui_remember(PxuiUi *ui, PxuiStr text, uint64_t caret, uint64_t anchor, uint8_t typing);
/* ABI 1.28: key=0 on text width/quads/raster calls selects library-owned content identity.
   Nonzero legacy keys remain accepted; content/style/width/span validation still fences cache reuse.
   Layout storage is bounded even for standalone shapers without a frame clock. */
/* A text shaper of its own, for text laid out off the Ui on any thread: its own font system, no GPU atlas. */
/* A shaper is a plain library object with no thread-local state: it may be created on one thread and used on another, but one thread
   uses one shaper at a time (calls on it never overlap). It shares nothing with a PxuiUi's text service. */
PxuiShaper *pxui_shaper_create(const PxuiStr *fonts, size_t count);
/* ABI 1.36: fork between frames, then move the owned shaper to a worker. This reserves the
   UI's bounded receiving cache and clones immutable font sources with matching face identities. */
PxuiShaper *pxui_text_fork(PxuiUi *ui);
/* Worker-only shaping, <=1 MiB UTF-8 and <=4 MiB glyph-list capacity per record.
   Both outputs remain unchanged on failure. Spans follow the usual eight-range validation. */
int32_t pxui_shaper_prepare(PxuiShaper *shaper, PxuiStr value, const PxuiTextStyle *style, float max_width, const PxuiTextSpans *spans, PxuiPreparedText **out, PxuiTextMetrics *out_metrics);
/* Between frames, swap a ready record into its originating UI without shaping/allocation.
   Success turns the SAME record into retired storage: send it back to the worker and destroy it.
   Failure leaves it ready and unchanged. Foreign/stale fonts, retired records and callbacks refuse.
   UI font additions/discovery invalidate outstanding forks; worker-discovered new faces cannot
   be installed. Glyph rasterization/atlas growth remain separate cold allocation paths. */
int32_t pxui_text_install(PxuiUi *ui, PxuiPreparedText *record);
/* Any owning thread, null is allowed. Destroy ready or retired records exactly once. */
void pxui_prepared_destroy(PxuiPreparedText *record);
void pxui_shaper_destroy(PxuiShaper *shaper);
uint32_t pxui_shaper_add_font(PxuiShaper *shaper, PxuiStr font);
int32_t pxui_shaper_width(PxuiShaper *shaper, uint64_t key, PxuiStr value, const PxuiTextStyle *style, float max_width, PxuiTextMetrics *out);
size_t pxui_shaper_rasterize_rgba(PxuiShaper *shaper, uint64_t key, PxuiStr value, const PxuiTextStyle *style, float max_width, float scale, uint8_t color_r, uint8_t color_g, uint8_t color_b, uint8_t color_a, uint32_t *width, uint32_t *height, uint8_t *out, size_t capacity);
PxuiStr pxui_default_font(void); /* Pragmasevka, embedded: valid as long as the library */
/* The text service of a Ui: its one font system and glyph atlas, for canvas text an app lays out itself (atlases: pxui_renderer_atlas).
   pxui_text_add_font answers UINT32_MAX, pxui_text_quads and pxui_text_rasterize_rgba SIZE_MAX, for bad arguments and a refused call. */
uint32_t pxui_text_add_font(PxuiUi *ui, PxuiStr font);
int32_t pxui_text_width(PxuiUi *ui, uint64_t key, PxuiStr value, const PxuiTextStyle *style, float max_width, PxuiTextMetrics *out);
size_t pxui_text_quads(PxuiUi *ui, uint64_t key, PxuiStr value, const PxuiTextStyle *style, float max_width, float x, float y, float scale, PxuiTextGlyph *out, size_t capacity);
size_t pxui_text_rasterize_rgba(PxuiUi *ui, uint64_t key, PxuiStr value, const PxuiTextStyle *style, float max_width, float scale, uint8_t color_r, uint8_t color_g, uint8_t color_b, uint8_t color_a, uint32_t *width, uint32_t *height, uint8_t *out, size_t capacity);
/* ABI 1.14: bit 2 on plain, unstroked RECTs selects pixel-area coverage; no layout change. */
int32_t pxui_paint_span(PxuiPaint *paint, size_t at_least, PxuiSpan *span); /* at_least 0 only commits; call it before any other pxui_paint_* call */
float pxui_paint_icon(PxuiPaint *paint, float x, float y, uint16_t font, float size, uint32_t codepoint, PxuiColor color);
void pxui_paint_text(PxuiPaint *paint, float x, float y, PxuiStr text);
void pxui_paint_text_color(PxuiPaint *paint, float x, float y, PxuiColor color, PxuiStr text);
void pxui_paint_text_size(PxuiPaint *paint, float x, float y, int64_t size, PxuiColor color, PxuiStr text);
void pxui_paint_text_with(PxuiPaint *paint, float x, float y, uint8_t has_size, int64_t size, float tracking, uint8_t has_color, PxuiColor color, PxuiStr text);
PxuiTextMetrics pxui_paint_text_styled(PxuiPaint *paint, float x, float y, const PxuiTextStyle *style, float width, uint8_t has_color, PxuiColor color, PxuiStr text);
float pxui_paint_text_width(PxuiPaint *paint, uint8_t has_size, int64_t size, PxuiStr text);
PxuiTextMetrics pxui_paint_text_metrics(PxuiPaint *paint, const PxuiTextStyle *style, float width, PxuiStr text);
float pxui_paint_caret_x(PxuiPaint *paint, uint8_t has_size, int64_t size, PxuiStr text, size_t index);
int32_t pxui_paint_selection(PxuiPaint *paint, uint8_t has_size, int64_t size, PxuiStr text, size_t a, size_t b, PxuiSpanBuild build, void *user);
void pxui_paint_cap(PxuiPaint *paint, float x, float y, uint8_t has_color, PxuiColor color, PxuiStr text);
float pxui_paint_cap_width(PxuiPaint *paint, PxuiStr text);
void pxui_paint_ratio(PxuiPaint *paint, float x, float y, int64_t a, int64_t b);
void pxui_paint_input_region(PxuiPaint *paint, float cursor, float x, float y, float w, float h);
/* The scroll offset of the retained slot `slot` (a box handle's box_slot) as the painter reads it; 0 out of range. */
float pxui_paint_scroll_y(PxuiPaint *paint, uint64_t slot);
/* ABI 1.43: colour token `token` resolved for the painter's theme, as pxui_ui_color; transparent past the table. */
PxuiColor pxui_paint_color(PxuiPaint *paint, uint16_t token);
/* ABI 1.45: text in a label style, resolved as a label body resolves it against the painter's theme (its label look, rows and
   colour tokens). paint_label lays out within width (0: unbounded) and draws; paint_label_chars draws one cached character
   layout at a time, digits on the advance of 0, and returns the width drawn. An invalid style draws nothing and returns 0. */
PxuiTextMetrics pxui_paint_label(PxuiPaint *paint, float x, float y, const PxuiLabelStyle *style, float width, PxuiStr text);
float pxui_paint_label_chars(PxuiPaint *paint, float x, float y, const PxuiLabelStyle *style, PxuiStr text);
void pxui_paint_image(PxuiPaint *paint, const PxuiImage *image, float x, float y, float width, float height, uint32_t fit, PxuiColor tint);
/* Shared GPU: the renderer records into the app's command buffers and passes and never submits, waits,
 * presents or destroys what the app created. create takes the lease in gpu->owner_token (also on failure).
 * prepare(commands, frame, completed): frame grows by at least 1 per call; completed (the last frame the app
 * knows finished) never goes back and is at least frame - 3; else PXUI_ERR_ARGUMENT and nothing is recorded.
 * draw leaves PXUI's pipeline, descriptors, viewport and scissor bound in the app's pass: draw last, or
 * rebind everything after it (OGPU: ogpu_core::backend::invalidate_state). PxuiImage: Vulkan views stay in
 * VK_IMAGE_LAYOUT_GENERAL while frames sampling them run; PXUI only samples them. destroy: the app has
 * finished every frame it prepared; a teardown failure is the Ui's last error. */
PxuiRenderer *pxui_renderer_create(PxuiUi *ui, const PxuiGpu *gpu, const PxuiTargetDesc *target);
int32_t pxui_renderer_prepare(PxuiRenderer *renderer, const PxuiCommands *commands, uint64_t frame, uint64_t completed);
/* Prepare blur storage at startup/resize, outside frame recording. Nonzero drawable-pixel dimensions. */
int32_t pxui_renderer_resize(PxuiRenderer *renderer, uint32_t width, uint32_t height);
int32_t pxui_renderer_draw(PxuiRenderer *renderer, const PxuiPass *pass);
/* The draw calls the renderer has issued so far. */
/* ABI 1.13: forget on every sampling renderer before destroying/reusing an app texture.
   completed is the host's actual completed serial on the prepare timeline and must not go back.
   Refuses ERR_STATE while a draw of the image exceeds completed. No extra frame; never destroys the app image. */
int32_t pxui_renderer_forget_image(PxuiRenderer *renderer, const PxuiImage *image, uint64_t completed);
uint64_t pxui_renderer_draw_calls(PxuiRenderer *renderer);
int32_t pxui_renderer_atlas(PxuiRenderer *renderer, uint32_t kind, PxuiImage *out); /* PXUI_ATLAS_*: mask R8, colour RGBA8; second pages return ERR_STATE until prepared. The texture stays the renderer's. */
void pxui_renderer_destroy(PxuiRenderer *renderer);

/* Queries and switches on the Ui as a whole. Text results are borrowed until the next frame. */
int64_t pxui_ui_font_size(PxuiUi *ui);
int64_t pxui_ui_row_height(PxuiUi *ui);
/* Monotonic host frame time, shared by cached views (ABI 1.16). */
double pxui_ui_now(PxuiUi *ui);
/* Integer rendering density shared with Paint: host scale rounded to at least one (ABI 1.19). */
int64_t pxui_ui_density(PxuiUi *ui);
/* ABI 1.52: the variant the widget call in progress hands its registered bodies of kind `kind` (the number of the kind in
   pxui_body_register; the `variant` field every Pxui*Config record now ends with; 0 none). Set it just before the widget call and
   0 again after: only a body of that kind reads it, and a body's trampoline keeps it 0 while the body runs. A frame starts with it cleared. */
int32_t pxui_ui_variant(PxuiUi *ui, uint32_t kind, uint16_t variant);
uint64_t pxui_ui_frame_index(PxuiUi *ui);
uint8_t pxui_ui_captured(PxuiUi *ui);
uint8_t pxui_ui_text_input_focused(PxuiUi *ui);
uint8_t pxui_ui_verifying(PxuiUi *ui);
uint64_t pxui_ui_text_undo_bytes(PxuiUi *ui);
float pxui_ui_inspector_width(PxuiUi *ui);
float pxui_ui_inspector_label_x(PxuiUi *ui);
uint32_t pxui_ui_passed_undo(PxuiUi *ui); /* 0 none, 1 undo, 2 redo */
int32_t pxui_ui_key_pressed(PxuiUi *ui, uint32_t code, uint8_t *out);
/* Raw held state, including modifiers when a widget or popup owns focus (ABI 1.15). */
int32_t pxui_ui_key_down(PxuiUi *ui, uint32_t code, uint8_t *out);
/* ABI 1.23: raw press query with Shift/Ctrl/Alt/Meta held at the event, including same-frame release. */
int32_t pxui_ui_key_pressed_with(PxuiUi *ui, uint32_t code, uint32_t modifier, uint8_t *out);
int32_t pxui_ui_set_font_size(PxuiUi *ui, int64_t size);
int32_t pxui_ui_unfocus(PxuiUi *ui);
int32_t pxui_ui_dismiss_popup(PxuiUi *ui);
int32_t pxui_ui_request_cursor(PxuiUi *ui, uint32_t shape); /* 1 horizontal resize, 2 vertical resize, 3 text */
int32_t pxui_ui_repaint_after(PxuiUi *ui, double seconds);
/* verify_views(on) for every Ui created from now on. */
int32_t pxui_verify_new_views(uint8_t on);
int32_t pxui_ui_verify_views(PxuiUi *ui, uint8_t on);
int32_t pxui_ui_set_timing(PxuiUi *ui, uint8_t on);
int32_t pxui_ui_phase_times(PxuiUi *ui, uint64_t *out, size_t len); /* nanoseconds, len must be 7 */
/* ABI 1.35: thread CPU ns, same order, sum of settling passes; len=7.
   set_timing enables it on Apple; ERR_STATE if unavailable, without changing out. */
int32_t pxui_ui_phase_cpu_times(PxuiUi *ui, uint64_t *out, size_t len);
int32_t pxui_ui_bytes(PxuiUi *ui, int64_t *out, size_t len); /* atlas, glyph lists, text undo; len must be 3 */
uint32_t pxui_ui_platform(PxuiUi *ui); /* 0 other, 1 Apple */
int32_t pxui_ui_rebuilt_named(PxuiUi *ui, PxuiStr prefix, uint64_t *out);
int32_t pxui_ui_expanded(PxuiUi *ui, PxuiStr key, uint8_t *out); /* 0 unknown, 1 collapsed, 2 expanded */
int32_t pxui_ui_hover_delay(PxuiUi *ui, PxuiStr key, uint8_t *out);
int32_t pxui_ui_tooltip(PxuiUi *ui, PxuiStr key, PxuiStr text);
int32_t pxui_ui_cap_width(PxuiUi *ui, PxuiStr text, float *out);
float pxui_ui_text_top(PxuiUi *ui, uint8_t has_size, int64_t size, float y, float h);

/* Boxes, patches, scroll and text state, carries, view transforms. */
int32_t pxui_ui_box(PxuiUi *ui, PxuiStr label, const PxuiBoxDesc *desc, PxuiBox *out);
int32_t pxui_ui_set_at(PxuiUi *ui, const PxuiBox *box, float x, float y);
int32_t pxui_ui_patch_look(PxuiUi *ui, const PxuiBox *box, const PxuiLookDesc *value, uint8_t *out);
int32_t pxui_ui_patch_rect(PxuiUi *ui, const PxuiBox *box, PxuiRect value, uint8_t *out);
int32_t pxui_ui_patch_mark(PxuiUi *ui, const PxuiBox *box, const PxuiMarkDesc *value, uint8_t *out);
/* The look, mark and inset of a node declared this frame, read and written back whole (the engine's node_mut). */
typedef struct { uint32_t size, reserved; PxuiLookDesc look; PxuiMarkDesc mark; PxuiEdges inset; } PxuiNodeFields;
int32_t pxui_ui_node_fields(PxuiUi *ui, const PxuiBox *box, PxuiNodeFields *out);
int32_t pxui_ui_set_node_fields(PxuiUi *ui, const PxuiBox *box, const PxuiNodeFields *fields);
/* The node a widget declared this frame as the engine holds it (written whole to out, refused when out->size is shorter), and the layout of a node the frame being built declared (axis, justify, align, wrap, size, minimum and maximum, grow, shrink, aspect, padding, margin, gaps, own alignment, offset) set from `node`'s same fields, the others ignored: how a widget body sizes the node it was given. */
int32_t pxui_ui_node_layout(PxuiUi *ui, const PxuiBox *box, PxuiNode *out);
int32_t pxui_ui_set_node_layout(PxuiUi *ui, const PxuiBox *box, const PxuiNode *node);
int32_t pxui_ui_local_rect(PxuiUi *ui, const PxuiBox *box, PxuiRect *out);
int32_t pxui_ui_scroll_offset(PxuiUi *ui, const PxuiBox *box, uint32_t which, float *out); /* 0 offset, 1 painted position */
int32_t pxui_ui_set_scroll_offset(PxuiUi *ui, const PxuiBox *box, float value);
/* ABI 1.32: minimum scroll to reveal a virtual list item/grid cell; out-of-range indices do nothing. */
int32_t pxui_ui_reveal(PxuiUi *ui, const PxuiBox *list, size_t index);
/* ABI 1.38: read, and with `in` set (may be NULL) request a node's touch gestures; commands bit 0 closes a swiped row,
   bit 1 ends a refresh. Touch only; mouse and trackpad are unchanged. */
int32_t pxui_ui_gesture(PxuiUi *ui, const PxuiBox *box, const PxuiGestureIn *in, uint32_t commands, PxuiGestureOut *out);
/* ABI 1.40: enable a node's row swipe with actions on both sides and a full-swipe commit, keeping its other gestures;
   call after pxui_ui_gesture each build. Extra PxuiGestureOut flags: 512 commit leading and 1024 commit trailing (this
   frame), 2048 open leading and 4096 open trailing (8 stays set for either). swipe stays the physical shift. */
int32_t pxui_ui_swipe_actions(PxuiUi *ui, const PxuiBox *box, const PxuiSwipeIn *in);
/* ABI 1.41: take a node's vertical drag-to-dismiss (a downward drag, handed over by an inner scroll view only at its top;
   touch only; call after pxui_ui_gesture each build) and read its state. */
int32_t pxui_ui_dismiss(PxuiUi *ui, const PxuiBox *box, const PxuiDismissIn *in, PxuiDismissOut *out);
/* ABI 1.34: cumulative text cache-miss layouts; read between frames. */
int32_t pxui_ui_text_layout_count(PxuiUi *ui, uint64_t *out);
int32_t pxui_ui_text_state(PxuiUi *ui, const PxuiBox *box, uint8_t *present, PxuiStr *out);
int32_t pxui_ui_set_text_state(PxuiUi *ui, const PxuiBox *box, uint8_t present, PxuiStr value);
int32_t pxui_ui_cached_label(PxuiUi *ui, const PxuiBox *box, uint64_t index, PxuiStr text, PxuiStr *out); /* borrowed from the slot */
int32_t pxui_ui_to_front(PxuiUi *ui, int64_t order, const PxuiBox *box);
/* ABI 1.11 reuses carry/drop text buffers; returned strings survive cancellation until the next frame. */
int32_t pxui_ui_carrying(PxuiUi *ui, uint8_t *present, PxuiStr *kind, PxuiStr *value);
int32_t pxui_ui_carry(PxuiUi *ui, uint8_t has_from, const PxuiBox *from, PxuiStr kind, PxuiStr value);
/* ABI 1.12: transfer an active pointer carry to a live hit ancestor (out=1); otherwise out=0.
   Scroll ancestors follow the pointer at their edges. Release/cancellation does not click the ancestor. */
int32_t pxui_ui_transfer_carry(PxuiUi *ui, const PxuiBox *to, uint8_t *out);
int32_t pxui_ui_cancel_carry(PxuiUi *ui);
int32_t pxui_ui_drop_target(PxuiUi *ui, const PxuiBox *box, uint32_t *tag, PxuiStr *kind, PxuiStr *value); /* 0 none, 1 hover, 2 dropped */
int32_t pxui_ui_context_at(PxuiUi *ui, const PxuiBox *box, uint8_t *present, PxuiPoint *out);
int32_t pxui_ui_hovered(PxuiUi *ui, const PxuiBox *box, uint8_t within, uint8_t *out);
int32_t pxui_ui_press_keys(PxuiUi *ui, const PxuiBox *box, const uint32_t **ptr, size_t *len); /* key codes, borrowed until the next frame */
int32_t pxui_ui_root_at(PxuiUi *ui, float x, float y, PxuiI64Span roots, uint8_t *present, int64_t *out);
int32_t pxui_ui_view_flat(PxuiUi *ui, PxuiStr label, uint64_t revision, PxuiSizeDesc width, PxuiSizeDesc height, PxuiBuild build, void *user, PxuiBox *out);
int32_t pxui_ui_view_offset(PxuiUi *ui, const PxuiBox *box, float dx, float dy);
/* Per-instance alpha in [0,1], finite uniform centre scale (zero/negative allowed), and an additional
   window-point clip (NULL removes it). Use regular views with their own entries; no offscreen group opacity. */
int32_t pxui_ui_view_opacity(PxuiUi *ui, const PxuiBox *box, float opacity);
int32_t pxui_ui_view_scale(PxuiUi *ui, const PxuiBox *box, float scale);
int32_t pxui_ui_view_clip(PxuiUi *ui, const PxuiBox *box, const PxuiRect *clip);
int32_t pxui_ui_view_transform_about(PxuiUi *ui, const PxuiBox *box, float scale, const float *transform, float px, float py);
int32_t pxui_ui_view_backdrop_blur(PxuiUi *ui, const PxuiBox *box, float sigma);
/* Saturation of the blurred backdrop: finite and nonnegative; 0 grayscale, 1 unchanged (default). */
int32_t pxui_ui_view_backdrop_saturation(PxuiUi *ui, const PxuiBox *box, float saturation);

/* Kit and text widgets: styled buttons, grips, popups, inspector pieces, the line field. Text results are borrowed until the next frame. */
int32_t pxui_ui_animate(PxuiUi *ui, const PxuiBox *box, const PxuiKeyframeDesc *key, uint8_t *out);
int32_t pxui_ui_button_styled(PxuiUi *ui, PxuiStr text, uint8_t bare, uint8_t icon, uint8_t at_end, uint8_t has_ink, PxuiColor ink, uint8_t *out);
int32_t pxui_ui_grip(PxuiUi *ui, PxuiStr label, uint32_t axis, uint8_t has_rect, PxuiRect rect, PxuiBox *out, PxuiSignal *signal);
/* ABI 1.44: the kit button with options; build (when set) replaces its text inside the box. Reports the box, its signal and whether it has focus. */
/* ABI 1.46: the icon button with options (PxuiIconButtonDesc); replaces pxui_ui_icon_button. */
int32_t pxui_ui_icon_button_with(PxuiUi *ui, const PxuiIconButtonDesc *desc, PxuiBox *out, PxuiSignal *signal, uint8_t *focused);
int32_t pxui_ui_button_with(PxuiUi *ui, const PxuiButtonDesc *desc, PxuiBuild build, void *user, PxuiBox *out, PxuiSignal *signal, uint8_t *focused);
/* ABI 1.48: the same calls reporting one PxuiResponse instead of the box, the whole signal and the focus; the _plain forms are the
   kit button keyed by key (the theme's look, the kit's size, at the start of its line) and the kit icon button showing the glyph
   icon named name (the theme's look, the kit's square), with no descriptor. */
int32_t pxui_ui_button_response(PxuiUi *ui, const PxuiButtonDesc *desc, PxuiBuild build, void *user, PxuiResponse *out);
int32_t pxui_ui_button_plain(PxuiUi *ui, PxuiStr key, PxuiStr text, PxuiResponse *out);
/* ABI 1.56: pxui_ui_button_response whose content callback is also given the button's interaction state (hovered, pressed, focused,
   disabled, checked) this frame; build may be null for a text button. */
int32_t pxui_ui_button_content(PxuiUi *ui, const PxuiButtonDesc *desc, PxuiButtonBuild build, void *user, PxuiResponse *out);
int32_t pxui_ui_icon_button_response(PxuiUi *ui, const PxuiIconButtonDesc *desc, PxuiResponse *out);
/* ABI 1.51: the kit toggle and segmented control, plain or with options, each reporting one PxuiResponse (a click on the toggle is the flip of `on`, which the caller keeps; the segmented control also reports the choice), and a scope in which every interactive node built is disabled (disabled 1; scopes nest by OR). */
int32_t pxui_ui_toggle_plain(PxuiUi *ui, PxuiStr key, PxuiStr text, uint8_t on, PxuiResponse *out);
int32_t pxui_ui_toggle_response(PxuiUi *ui, const PxuiToggleDesc *desc, PxuiResponse *out);
int32_t pxui_ui_segmented_plain(PxuiUi *ui, PxuiStr key, PxuiStr name, PxuiStrSpan options, size_t selected, PxuiResponse *out, size_t *chosen);
int32_t pxui_ui_segmented_response(PxuiUi *ui, const PxuiSegmentedDesc *desc, PxuiStrSpan options, PxuiResponse *out, size_t *chosen);
int32_t pxui_ui_disabled(PxuiUi *ui, uint8_t disabled, PxuiBuild build, void *user);
int32_t pxui_ui_icon_button_plain(PxuiUi *ui, PxuiStr key, PxuiStr icon, PxuiStr name, PxuiResponse *out);
/* ABI 1.53: pxui_ui_dialog with the buttons' default, cancel, danger and disabled flags, the sheet's key and width, and a PxuiDialogResult. */
int32_t pxui_ui_dialog_response(PxuiUi *ui, const PxuiDialogDesc *desc, PxuiBuild build, void *user, PxuiDialogResult *out);
int32_t pxui_ui_popup(PxuiUi *ui, const PxuiPopupDesc *desc, PxuiBuild build, void *user, uint8_t *out);
int32_t pxui_ui_inspector_bar(PxuiUi *ui, PxuiHintSpan hints, int64_t count);
int32_t pxui_ui_inspector_body(PxuiUi *ui, PxuiBuild build, void *user);
int32_t pxui_ui_inspector_header(PxuiUi *ui, const PxuiHeaderDesc *desc, PxuiTextValid rename, void *user, PxuiHeadOut *out);
int32_t pxui_ui_inspector_section_view(PxuiUi *ui, PxuiStr key, uint64_t revision, uint8_t expanded, uint8_t has_set, uint8_t set_expanded, PxuiStr label, PxuiBuild build, void *user, uint8_t *out);
int32_t pxui_ui_inspector_toggle_value(PxuiUi *ui, PxuiStr key, float x, float y, uint8_t value, uint8_t *out);
int32_t pxui_ui_line_field(PxuiUi *ui, float x, float y, float w, float h, uint8_t grab, PxuiStr placeholder, PxuiStr label, PxuiStr value, PxuiStr *text, uint32_t *step); /* step: 0 none, 1 next, 2 previous, 3 close */
int32_t pxui_ui_field_drag(PxuiUi *ui, const PxuiBox *box, uint8_t *present, PxuiPoint *out);

/* Themes a caller builds and scopes over a subtree. set and load apply over what the theme holds, as a theme file applies over
   its base: a tokens.* key re-derives every look from the new tokens, so set tokens before the keys that restyle single looks. */
PxuiTheme *pxui_theme_create(const PxuiThemeTokens *tokens); /* tokens may be NULL for the defaults; NULL result: the tokens are invalid (not finite, a size or density at or below zero, a flag above 1, reserved bytes set) */
void pxui_theme_destroy(PxuiTheme *theme);
int32_t pxui_theme_set(PxuiTheme *theme, PxuiStr key, PxuiStr value);
int32_t pxui_theme_load(PxuiTheme *theme, PxuiStr path);
PxuiStr pxui_theme_last_error(const PxuiTheme *theme); /* borrowed from the theme until its next call */
int32_t pxui_ui_themed(PxuiUi *ui, const PxuiTheme *theme, PxuiBuild build, void *user);
/* pxui_theme_set for a whole theme file: `KIND.body = NAME` names the bodies `ui` registered; an unregistered name is refused (pxui_theme_last_error). */
int32_t pxui_ui_theme_apply(PxuiUi *ui, PxuiTheme *theme, PxuiStr text);


/* Accessibility bridge of an app's window (Linux: AT-SPI2 on the accessibility bus; elsewhere start returns null, the platform's bridge
   belongs to the window host). The app feeds each frame's tree and takes the actions assistive technology asked for. */
typedef struct PxuiA11y PxuiA11y;
PxuiA11y *pxui_a11y_start(PxuiStr name);
int32_t pxui_a11y_fd(PxuiA11y *a11y);
int32_t pxui_a11y_update(PxuiA11y *a11y, const PxuiA11yTree *tree);
/* 1: the connection is alive, 0: gone (stop it). `out` holds the actions, borrowed until the next pump. */
int32_t pxui_a11y_pump(PxuiA11y *a11y, PxuiA11yActionSpan *out);
void pxui_a11y_stop(PxuiA11y *a11y);

int32_t pxui_shaper_width_spans(PxuiShaper *shaper, uint64_t key, PxuiStr value, const PxuiTextStyle *style, float max_width, const PxuiTextSpans *spans, PxuiTextMetrics *out);
int32_t pxui_text_width_spans(PxuiUi *ui, uint64_t key, PxuiStr value, const PxuiTextStyle *style, float max_width, const PxuiTextSpans *spans, PxuiTextMetrics *out);
int32_t pxui_ui_text_spans(PxuiUi *ui, const PxuiBox *box_, PxuiStr value, const PxuiTextStyle *style, const PxuiTextSpans *spans);
/* Positive line height; rounded primary-font metrics place CSS line-box baselines (ABI 1.17). */
int32_t pxui_ui_text_line_box(PxuiUi *ui, const PxuiBox *box_, PxuiStr value, const PxuiTextStyle *style, const PxuiTextSpans *spans);
int32_t pxui_paint_text_spans(PxuiPaint *p, float x, float y, const PxuiTextStyle *style, float width, uint8_t has_color, PxuiColor color, PxuiStr text, const PxuiTextSpans *spans, PxuiTextMetrics *out);

/* Apple native fallback, cold discovery only. Ui must enable before its first frame.
   Explicit-font constructors remain deterministic. No provider: PXUI_ERR_STATE. */
int32_t pxui_shaper_system_fonts(PxuiShaper *shaper);
int32_t pxui_text_system_fonts(PxuiUi *ui);
/* abi-layout begin: generated from the Rust types by `PXUI_LAYOUT_WRITE=1 cargo t -p pxui_types abi_layout_rows`; do not edit */
_Static_assert(sizeof(PxuiPoint) == 8 && offsetof(PxuiPoint, x) == 0 && offsetof(PxuiPoint, y) == 4, "Point layout");
_Static_assert(sizeof(PxuiPoint) == 8 && offsetof(PxuiPoint, x) == 0 && offsetof(PxuiPoint, y) == 4, "PxuiPoint layout");
_Static_assert(sizeof(PxuiRect) == 16 && offsetof(PxuiRect, x) == 0 && offsetof(PxuiRect, y) == 4 && offsetof(PxuiRect, w) == 8 && offsetof(PxuiRect, h) == 12, "Rect layout");
_Static_assert(sizeof(PxuiEdges) == 16 && offsetof(PxuiEdges, l) == 0 && offsetof(PxuiEdges, t) == 4 && offsetof(PxuiEdges, r) == 8 && offsetof(PxuiEdges, b) == 12, "Edges layout");
_Static_assert(sizeof(PxuiSize) == 16, "Size layout");
_Static_assert(sizeof(PxuiInstance) == 64 && offsetof(PxuiInstance, rect) == 0 && offsetof(PxuiInstance, clip) == 16 && offsetof(PxuiInstance, color) == 24 && offsetof(PxuiInstance, color2) == 28 && offsetof(PxuiInstance, aux) == 32 && offsetof(PxuiInstance, radii) == 40 && offsetof(PxuiInstance, width) == 48 && offsetof(PxuiInstance, kind) == 50 && offsetof(PxuiInstance, flags) == 51 && offsetof(PxuiInstance, view) == 52 && offsetof(PxuiInstance, clip_radius) == 54 && offsetof(PxuiInstance, extra) == 56, "Instance layout");
_Static_assert(sizeof(PxuiStr) == 16 && offsetof(PxuiStr, ptr) == 0 && offsetof(PxuiStr, len) == 8, "Str layout");
_Static_assert(sizeof(PxuiEvent) == 48 && offsetof(PxuiEvent, kind) == 0 && offsetof(PxuiEvent, code) == 4 && offsetof(PxuiEvent, x) == 8 && offsetof(PxuiEvent, y) == 16 && offsetof(PxuiEvent, value) == 24 && offsetof(PxuiEvent, text) == 32, "Event layout");
_Static_assert(offsetof(PxuiFrameIn, appearance) == 48 && offsetof(PxuiFrameIn, size) == 0 && offsetof(PxuiFrameIn, events) == 8 && offsetof(PxuiFrameIn, event_count) == 16 && offsetof(PxuiFrameIn, width) == 24 && offsetof(PxuiFrameIn, height) == 28 && offsetof(PxuiFrameIn, scale) == 32 && offsetof(PxuiFrameIn, time) == 40, "FrameIn layout");
_Static_assert(sizeof(PxuiFrameOut) == 40 && offsetof(PxuiFrameOut, size) == 0 && offsetof(PxuiFrameOut, damage) == 4 && offsetof(PxuiFrameOut, has_damage) == 20 && offsetof(PxuiFrameOut, reserved) == 21 && offsetof(PxuiFrameOut, layer_count) == 24 && offsetof(PxuiFrameOut, layers) == 32, "FrameOut layout");
_Static_assert(sizeof(PxuiGpu) == 128 && offsetof(PxuiGpu, size) == 0 && offsetof(PxuiGpu, backend) == 4 && offsetof(PxuiGpu, instance) == 8 && offsetof(PxuiGpu, physical_device) == 16 && offsetof(PxuiGpu, device) == 24 && offsetof(PxuiGpu, queue) == 32 && offsetof(PxuiGpu, queue_family) == 40 && offsetof(PxuiGpu, api_version) == 44 && offsetof(PxuiGpu, features) == 48 && offsetof(PxuiGpu, get_instance_proc_addr) == 56 && offsetof(PxuiGpu, pipeline_cache) == 64 && offsetof(PxuiGpu, mtl_device) == 72 && offsetof(PxuiGpu, wl_display) == 80 && offsetof(PxuiGpu, wl_surface) == 88 && offsetof(PxuiGpu, metal_layer) == 96 && offsetof(PxuiGpu, hwnd) == 104 && offsetof(PxuiGpu, owner_token) == 112 && offsetof(PxuiGpu, release_owner) == 120, "Gpu layout");
_Static_assert(sizeof(PxuiCommands) == 24 && offsetof(PxuiCommands, size) == 0 && offsetof(PxuiCommands, vk_command_buffer) == 8 && offsetof(PxuiCommands, mtl_command_buffer) == 16, "Commands layout");
_Static_assert(sizeof(PxuiPass) == 72 && offsetof(PxuiPass, size) == 0 && offsetof(PxuiPass, vk_command_buffer) == 8 && offsetof(PxuiPass, mtl_render_encoder) == 16 && offsetof(PxuiPass, width_px) == 24 && offsetof(PxuiPass, height_px) == 28 && offsetof(PxuiPass, scale) == 32 && offsetof(PxuiPass, mtl_command_buffer) == 40 && offsetof(PxuiPass, first_layer) == 48 && offsetof(PxuiPass, layer_count) == 52 && offsetof(PxuiPass, split) == 56 && offsetof(PxuiPass, split_user) == 64, "Pass layout");
_Static_assert(sizeof(PxuiImage) == 40 && offsetof(PxuiImage, size) == 0 && offsetof(PxuiImage, vk_image_view) == 8 && offsetof(PxuiImage, mtl_texture) == 16 && offsetof(PxuiImage, width) == 24 && offsetof(PxuiImage, height) == 28 && offsetof(PxuiImage, format) == 32, "Image layout");
_Static_assert(sizeof(PxuiSpan) == 32 && offsetof(PxuiSpan, size) == 0 && offsetof(PxuiSpan, view) == 4 && offsetof(PxuiSpan, clip_radius_f16) == 6 && offsetof(PxuiSpan, clip) == 8 && offsetof(PxuiSpan, at) == 16 && offsetof(PxuiSpan, end) == 24, "Span layout");
_Static_assert(sizeof(PxuiBox) == 32 && offsetof(PxuiBox, index) == 0 && offsetof(PxuiBox, box_key) == 8 && offsetof(PxuiBox, box_slot) == 16 && offsetof(PxuiBox, generation) == 24, "PxuiBox layout");
_Static_assert(sizeof(PxuiPanel) == 200 && offsetof(PxuiPanel, size) == 0 && offsetof(PxuiPanel, x) == 4 && offsetof(PxuiPanel, y) == 8 && offsetof(PxuiPanel, width) == 12 && offsetof(PxuiPanel, height) == 16 && offsetof(PxuiPanel, padding) == 24 && offsetof(PxuiPanel, has_height) == 32 && offsetof(PxuiPanel, window) == 33 && offsetof(PxuiPanel, has_max_height) == 34 && offsetof(PxuiPanel, has_stroke) == 35 && offsetof(PxuiPanel, has_look) == 36 && offsetof(PxuiPanel, reserved) == 37 && offsetof(PxuiPanel, max_height) == 40 && offsetof(PxuiPanel, stroke) == 44 && offsetof(PxuiPanel, look) == 48, "PxuiPanel layout");
_Static_assert(sizeof(PxuiTargetDesc) == 32 && offsetof(PxuiTargetDesc, size) == 0 && offsetof(PxuiTargetDesc, format) == 4 && offsetof(PxuiTargetDesc, samples) == 8 && offsetof(PxuiTargetDesc, reserved) == 9 && offsetof(PxuiTargetDesc, mask_width) == 12 && offsetof(PxuiTargetDesc, mask_height) == 16 && offsetof(PxuiTargetDesc, color_width) == 20 && offsetof(PxuiTargetDesc, color_height) == 24 && offsetof(PxuiTargetDesc, reserved_tail) == 28, "PxuiTargetDesc layout");
_Static_assert(sizeof(PxuiUiDesc) == 12 && offsetof(PxuiUiDesc, size) == 0 && offsetof(PxuiUiDesc, platform) == 4 && offsetof(PxuiUiDesc, font_size) == 8, "PxuiUiDesc layout");
_Static_assert(sizeof(PxuiInput) == 160 && offsetof(PxuiInput, size) == 0 && offsetof(PxuiInput, time) == 8 && offsetof(PxuiInput, dt) == 16 && offsetof(PxuiInput, frame) == 24 && offsetof(PxuiInput, tick) == 32 && offsetof(PxuiInput, logical_size) == 40 && offsetof(PxuiInput, pointer) == 56 && offsetof(PxuiInput, keys) == 72 && offsetof(PxuiInput, buttons) == 88 && offsetof(PxuiInput, events) == 104 && offsetof(PxuiInput, pixel_scale) == 120 && offsetof(PxuiInput, mouse_delta) == 136 && offsetof(PxuiInput, fps) == 152, "Input layout");
_Static_assert(sizeof(PxuiPlatformOut) == 88 && offsetof(PxuiPlatformOut, size) == 0 && offsetof(PxuiPlatformOut, cursor) == 4 && offsetof(PxuiPlatformOut, has_cursor) == 8 && offsetof(PxuiPlatformOut, has_ime) == 9 && offsetof(PxuiPlatformOut, has_background) == 10 && offsetof(PxuiPlatformOut, has_clipboard) == 11 && offsetof(PxuiPlatformOut, ime_rect) == 16 && offsetof(PxuiPlatformOut, ime_caret) == 48 && offsetof(PxuiPlatformOut, clipboard) == 56 && offsetof(PxuiPlatformOut, background) == 72 && offsetof(PxuiPlatformOut, paste) == 76 && offsetof(PxuiPlatformOut, reserved) == 77, "PlatformOut layout");
_Static_assert(sizeof(PxuiDamage) == 20 && offsetof(PxuiDamage, has_damage) == 0 && offsetof(PxuiDamage, reserved) == 1 && offsetof(PxuiDamage, rect) == 4, "Damage layout");
_Static_assert(sizeof(PxuiA11yValue) == 48 && offsetof(PxuiA11yValue, tag) == 0 && offsetof(PxuiA11yValue, text_start) == 4 && offsetof(PxuiA11yValue, text_end) == 8 && offsetof(PxuiA11yValue, reserved) == 12 && offsetof(PxuiA11yValue, now) == 16 && offsetof(PxuiA11yValue, min) == 24 && offsetof(PxuiA11yValue, max) == 32 && offsetof(PxuiA11yValue, step) == 40, "A11yValue layout");
_Static_assert(sizeof(PxuiA11yNode) == 128 && offsetof(PxuiA11yNode, id) == 0 && offsetof(PxuiA11yNode, parent) == 8 && offsetof(PxuiA11yNode, role) == 12 && offsetof(PxuiA11yNode, name_start) == 16 && offsetof(PxuiA11yNode, name_end) == 20 && offsetof(PxuiA11yNode, description_start) == 24 && offsetof(PxuiA11yNode, description_end) == 28 && offsetof(PxuiA11yNode, value) == 32 && offsetof(PxuiA11yNode, rect) == 80 && offsetof(PxuiA11yNode, states) == 96 && offsetof(PxuiA11yNode, actions) == 100 && offsetof(PxuiA11yNode, selection) == 104 && offsetof(PxuiA11yNode, child_count) == 112 && offsetof(PxuiA11yNode, first_child) == 116 && offsetof(PxuiA11yNode, next_sibling) == 120 && offsetof(PxuiA11yNode, last_child) == 124, "A11yNode layout");
_Static_assert(sizeof(PxuiA11yAction) == 32 && offsetof(PxuiA11yAction, id) == 0 && offsetof(PxuiA11yAction, action_code) == 8 && offsetof(PxuiA11yAction, reserved) == 12 && offsetof(PxuiA11yAction, payload) == 16 && offsetof(PxuiA11yAction, end) == 24 && offsetof(PxuiA11yAction, reserved_tail) == 28, "A11yAction layout");
_Static_assert(sizeof(PxuiA11yTree) == 64 && offsetof(PxuiA11yTree, nodes) == 0 && offsetof(PxuiA11yTree, strings) == 16 && offsetof(PxuiA11yTree, revision) == 32 && offsetof(PxuiA11yTree, first_child) == 40 && offsetof(PxuiA11yTree, child_count) == 44 && offsetof(PxuiA11yTree, actions) == 48 && offsetof(PxuiA11yTree, actions.len) == 56, "A11yTree layout");
_Static_assert(sizeof(PxuiOutput) == 240 && offsetof(PxuiOutput, size) == 0 && offsetof(PxuiOutput, reserved) == 4 && offsetof(PxuiOutput, draw_revision) == 8 && offsetof(PxuiOutput, repaint_at) == 16 && offsetof(PxuiOutput, built_nodes) == 24 && offsetof(PxuiOutput, rebuilt_views) == 32 && offsetof(PxuiOutput, arranged_nodes) == 40 && offsetof(PxuiOutput, platform) == 48 && offsetof(PxuiOutput, damage) == 136 && offsetof(PxuiOutput, a11y) == 160 && offsetof(PxuiOutput, layers) == 224, "Output layout");
_Static_assert(sizeof(PxuiThemeSource) == 24 && offsetof(PxuiThemeSource, size) == 0 && offsetof(PxuiThemeSource, flags) == 4 && offsetof(PxuiThemeSource, source) == 8, "ThemeSource layout");
_Static_assert(sizeof(PxuiMotion) == 28 && offsetof(PxuiMotion, fast) == 0 && offsetof(PxuiMotion, normal) == 4 && offsetof(PxuiMotion, slow) == 8 && offsetof(PxuiMotion, ease) == 12, "PxuiMotion layout");
_Static_assert(sizeof(PxuiThemeTokens) == 136 && offsetof(PxuiThemeTokens, size) == 0 && offsetof(PxuiThemeTokens, panel) == 4 && offsetof(PxuiThemeTokens, foreground) == 8 && offsetof(PxuiThemeTokens, control) == 12 && offsetof(PxuiThemeTokens, input) == 16 && offsetof(PxuiThemeTokens, track) == 20 && offsetof(PxuiThemeTokens, accent) == 24 && offsetof(PxuiThemeTokens, ink_ground) == 28 && offsetof(PxuiThemeTokens, has_ink_ground) == 32 && offsetof(PxuiThemeTokens, reserved) == 33 && offsetof(PxuiThemeTokens, space) == 36 && offsetof(PxuiThemeTokens, radius) == 52 && offsetof(PxuiThemeTokens, display_size) == 64 && offsetof(PxuiThemeTokens, title_size) == 72 && offsetof(PxuiThemeTokens, font_size) == 80 && offsetof(PxuiThemeTokens, label_size) == 88 && offsetof(PxuiThemeTokens, motion) == 96 && offsetof(PxuiThemeTokens, reduced_motion) == 124 && offsetof(PxuiThemeTokens, reserved_tail) == 125 && offsetof(PxuiThemeTokens, density) == 128, "PxuiThemeTokens layout");
_Static_assert(sizeof(PxuiDialogOut) == 16 && offsetof(PxuiDialogOut, tag) == 0 && offsetof(PxuiDialogOut, index) == 8, "PxuiDialogOut layout");
_Static_assert(sizeof(PxuiSizeDesc) == 16 && offsetof(PxuiSizeDesc, kind) == 0 && offsetof(PxuiSizeDesc, value) == 4 && offsetof(PxuiSizeDesc, relative) == 8, "PxuiSizeDesc layout");
_Static_assert(sizeof(PxuiAnchorDesc) == 40 && offsetof(PxuiAnchorDesc, has_anchor) == 0 && offsetof(PxuiAnchorDesc, rect) == 4 && offsetof(PxuiAnchorDesc, side) == 20 && offsetof(PxuiAnchorDesc, align) == 24 && offsetof(PxuiAnchorDesc, gap) == 28 && offsetof(PxuiAnchorDesc, margin) == 32 && offsetof(PxuiAnchorDesc, flip) == 36 && offsetof(PxuiAnchorDesc, reserved_tail) == 37, "PxuiAnchorDesc layout");
_Static_assert(sizeof(PxuiFontDesc) == 24 && offsetof(PxuiFontDesc, family) == 0 && offsetof(PxuiFontDesc, weight) == 2 && offsetof(PxuiFontDesc, size) == 8 && offsetof(PxuiFontDesc, tracking) == 16 && offsetof(PxuiFontDesc, caps) == 20 && offsetof(PxuiFontDesc, reserved) == 21, "PxuiFontDesc layout");
_Static_assert(sizeof(PxuiShadowDesc) == 24 && offsetof(PxuiShadowDesc, color) == 0 && offsetof(PxuiShadowDesc, offset) == 4 && offsetof(PxuiShadowDesc, blur) == 12 && offsetof(PxuiShadowDesc, spread) == 16 && offsetof(PxuiShadowDesc, inner) == 20 && offsetof(PxuiShadowDesc, reserved) == 21, "PxuiShadowDesc layout");
_Static_assert(sizeof(PxuiRingDesc) == 12 && offsetof(PxuiRingDesc, color) == 0 && offsetof(PxuiRingDesc, width) == 4 && offsetof(PxuiRingDesc, sides) == 8, "PxuiRingDesc layout");
_Static_assert(sizeof(PxuiLookDesc) == 152 && offsetof(PxuiLookDesc, fill) == 0 && offsetof(PxuiLookDesc, gradient) == 4 && offsetof(PxuiLookDesc, has_gradient) == 8 && offsetof(PxuiLookDesc, angle) == 12 && offsetof(PxuiLookDesc, border) == 16 && offsetof(PxuiLookDesc, border_w) == 20 && offsetof(PxuiLookDesc, sides) == 24 && offsetof(PxuiLookDesc, radius) == 28 && offsetof(PxuiLookDesc, shadows) == 44 && offsetof(PxuiLookDesc, text) == 92 && offsetof(PxuiLookDesc, font) == 96 && offsetof(PxuiLookDesc, opacity) == 120 && offsetof(PxuiLookDesc, ring) == 124 && offsetof(PxuiLookDesc, selection) == 136 && offsetof(PxuiLookDesc, caret) == 140 && offsetof(PxuiLookDesc, dashed) == 144 && offsetof(PxuiLookDesc, reserved_tail) == 145, "PxuiLookDesc layout");
_Static_assert(sizeof(PxuiMarkDesc) == 16 && offsetof(PxuiMarkDesc, kind) == 0 && offsetof(PxuiMarkDesc, direction) == 4 && offsetof(PxuiMarkDesc, width) == 8 && offsetof(PxuiMarkDesc, rising) == 12 && offsetof(PxuiMarkDesc, reserved) == 13, "PxuiMarkDesc layout");
_Static_assert(sizeof(PxuiStateDesc) == 8 && offsetof(PxuiStateDesc, hovered) == 0 && offsetof(PxuiStateDesc, pressed) == 1 && offsetof(PxuiStateDesc, focused) == 2 && offsetof(PxuiStateDesc, disabled) == 3 && offsetof(PxuiStateDesc, checked) == 4 && offsetof(PxuiStateDesc, reserved) == 5, "PxuiStateDesc layout");
_Static_assert(sizeof(PxuiCurveDesc) == 160 && offsetof(PxuiCurveDesc, kind) == 0 && offsetof(PxuiCurveDesc, duration) == 4 && offsetof(PxuiCurveDesc, points) == 8 && offsetof(PxuiCurveDesc, omega) == 24 && offsetof(PxuiCurveDesc, count) == 28 && offsetof(PxuiCurveDesc, positions) == 32 && offsetof(PxuiCurveDesc, values) == 96, "PxuiCurveDesc layout");
_Static_assert(sizeof(PxuiMenuDesc) == 40 && offsetof(PxuiMenuDesc, size) == 0 && offsetof(PxuiMenuDesc, reserved) == 4 && offsetof(PxuiMenuDesc, title) == 8 && offsetof(PxuiMenuDesc, items) == 24, "PxuiMenuDesc layout");
_Static_assert(sizeof(PxuiGridListDesc) == 56 && offsetof(PxuiGridListDesc, size) == 0 && offsetof(PxuiGridListDesc, reserved) == 4 && offsetof(PxuiGridListDesc, label) == 8 && offsetof(PxuiGridListDesc, count) == 24 && offsetof(PxuiGridListDesc, cell_w) == 32 && offsetof(PxuiGridListDesc, cell_h) == 36 && offsetof(PxuiGridListDesc, build) == 40 && offsetof(PxuiGridListDesc, user) == 48, "PxuiGridListDesc layout");
_Static_assert(sizeof(PxuiSplitDesc) == 72 && offsetof(PxuiSplitDesc, size) == 0 && offsetof(PxuiSplitDesc, axis) == 4 && offsetof(PxuiSplitDesc, label) == 8 && offsetof(PxuiSplitDesc, ratio) == 24 && offsetof(PxuiSplitDesc, min_a) == 28 && offsetof(PxuiSplitDesc, min_b) == 32 && offsetof(PxuiSplitDesc, reserved) == 36 && offsetof(PxuiSplitDesc, build_a) == 40 && offsetof(PxuiSplitDesc, user_a) == 48 && offsetof(PxuiSplitDesc, build_b) == 56 && offsetof(PxuiSplitDesc, user_b) == 64, "PxuiSplitDesc layout");
_Static_assert(sizeof(PxuiMenuBarDesc) == 40 && offsetof(PxuiMenuBarDesc, size) == 0 && offsetof(PxuiMenuBarDesc, reserved) == 4 && offsetof(PxuiMenuBarDesc, label) == 8 && offsetof(PxuiMenuBarDesc, menus) == 24, "PxuiMenuBarDesc layout");
_Static_assert(sizeof(PxuiMenuOut) == 24 && offsetof(PxuiMenuOut, size) == 0 && offsetof(PxuiMenuOut, reserved) == 4 && offsetof(PxuiMenuOut, menu) == 8 && offsetof(PxuiMenuOut, item) == 16, "PxuiMenuOut layout");
_Static_assert(sizeof(PxuiHintDesc) == 40 && offsetof(PxuiHintDesc, size) == 0 && offsetof(PxuiHintDesc, reserved) == 4 && offsetof(PxuiHintDesc, key) == 8 && offsetof(PxuiHintDesc, value) == 24, "PxuiHintDesc layout");
_Static_assert(sizeof(PxuiFooterDesc) == 40 && offsetof(PxuiFooterDesc, size) == 0 && offsetof(PxuiFooterDesc, has_right) == 4 && offsetof(PxuiFooterDesc, reserved) == 5 && offsetof(PxuiFooterDesc, right) == 8 && offsetof(PxuiFooterDesc, ptr) == 24, "PxuiFooterDesc layout");
_Static_assert(sizeof(PxuiKeyframeDesc) == 344 && offsetof(PxuiKeyframeDesc, size) == 0 && offsetof(PxuiKeyframeDesc, time) == 4 && offsetof(PxuiKeyframeDesc, target) == 8 && offsetof(PxuiKeyframeDesc, reserved) == 12 && offsetof(PxuiKeyframeDesc, values) == 16 && offsetof(PxuiKeyframeDesc, look) == 32 && offsetof(PxuiKeyframeDesc, curve) == 184, "PxuiKeyframeDesc layout");
_Static_assert(sizeof(PxuiInspectorRowDesc) == 96 && offsetof(PxuiInspectorRowDesc, size) == 0 && offsetof(PxuiInspectorRowDesc, has_width) == 4 && offsetof(PxuiInspectorRowDesc, has_pin) == 5 && offsetof(PxuiInspectorRowDesc, pin) == 6 && offsetof(PxuiInspectorRowDesc, reserved) == 7 && offsetof(PxuiInspectorRowDesc, key) == 8 && offsetof(PxuiInspectorRowDesc, label) == 24 && offsetof(PxuiInspectorRowDesc, width) == 40 && offsetof(PxuiInspectorRowDesc, reserved_tail) == 44 && offsetof(PxuiInspectorRowDesc, row) == 48 && offsetof(PxuiInspectorRowDesc, control_x) == 80 && offsetof(PxuiInspectorRowDesc, control_y) == 84 && offsetof(PxuiInspectorRowDesc, control_w) == 88, "PxuiInspectorRowDesc layout");
_Static_assert(sizeof(PxuiInspectorSectionDesc) == 64 && offsetof(PxuiInspectorSectionDesc, size) == 0 && offsetof(PxuiInspectorSectionDesc, expanded) == 4 && offsetof(PxuiInspectorSectionDesc, has_set) == 5 && offsetof(PxuiInspectorSectionDesc, set_expanded) == 6 && offsetof(PxuiInspectorSectionDesc, reserved) == 7 && offsetof(PxuiInspectorSectionDesc, key) == 8 && offsetof(PxuiInspectorSectionDesc, label) == 24 && offsetof(PxuiInspectorSectionDesc, build) == 40 && offsetof(PxuiInspectorSectionDesc, user) == 48 && offsetof(PxuiInspectorSectionDesc, out_open) == 56, "PxuiInspectorSectionDesc layout");
_Static_assert(sizeof(PxuiInspectorValueDesc) == 64 && offsetof(PxuiInspectorValueDesc, size) == 0 && offsetof(PxuiInspectorValueDesc, has_width) == 4 && offsetof(PxuiInspectorValueDesc, reserved) == 5 && offsetof(PxuiInspectorValueDesc, key) == 8 && offsetof(PxuiInspectorValueDesc, label) == 24 && offsetof(PxuiInspectorValueDesc, value) == 40 && offsetof(PxuiInspectorValueDesc, width) == 56, "PxuiInspectorValueDesc layout");
_Static_assert(sizeof(PxuiInspectorTextDesc) == 40 && offsetof(PxuiInspectorTextDesc, size) == 0 && offsetof(PxuiInspectorTextDesc, reserved) == 4 && offsetof(PxuiInspectorTextDesc, key) == 8 && offsetof(PxuiInspectorTextDesc, text) == 24, "PxuiInspectorTextDesc layout");
_Static_assert(sizeof(PxuiTableDesc) == 80 && offsetof(PxuiTableDesc, size) == 0 && offsetof(PxuiTableDesc, reserved) == 4 && offsetof(PxuiTableDesc, label) == 8 && offsetof(PxuiTableDesc, at) == 24 && offsetof(PxuiTableDesc, width) == 32 && offsetof(PxuiTableDesc, height) == 36 && offsetof(PxuiTableDesc, headers) == 40 && offsetof(PxuiTableDesc, rows) == 56 && offsetof(PxuiTableDesc, cell) == 64 && offsetof(PxuiTableDesc, user) == 72, "PxuiTableDesc layout");
_Static_assert(sizeof(PxuiSaved) == 24 && offsetof(PxuiSaved, size) == 0 && offsetof(PxuiSaved, reserved) == 4 && offsetof(PxuiSaved, parents) == 8 && offsetof(PxuiSaved, seeds) == 16, "PxuiSaved layout");
_Static_assert(sizeof(PxuiSignal) == 112 && offsetof(PxuiSignal, size) == 0 && offsetof(PxuiSignal, flags) == 4 && offsetof(PxuiSignal, has_subtree_press) == 8 && offsetof(PxuiSignal, button) == 9 && offsetof(PxuiSignal, has_a11y_value) == 10 && offsetof(PxuiSignal, has_a11y_select) == 11 && offsetof(PxuiSignal, reserved) == 12 && offsetof(PxuiSignal, subtree_press) == 16 && offsetof(PxuiSignal, clicks) == 24 && offsetof(PxuiSignal, drag) == 32 && offsetof(PxuiSignal, pointer) == 40 && offsetof(PxuiSignal, press_point) == 48 && offsetof(PxuiSignal, release_point) == 56 && offsetof(PxuiSignal, scroll) == 64 && offsetof(PxuiSignal, pinch) == 72 && offsetof(PxuiSignal, a11y_value) == 80 && offsetof(PxuiSignal, a11y_select) == 88 && offsetof(PxuiSignal, keys) == 96, "PxuiSignal layout");
_Static_assert(sizeof(PxuiTextStyle) == 36 && offsetof(PxuiTextStyle, size) == 0 && offsetof(PxuiTextStyle, font) == 4 && offsetof(PxuiTextStyle, weight) == 6 && offsetof(PxuiTextStyle, size_points) == 8 && offsetof(PxuiTextStyle, line_height) == 12 && offsetof(PxuiTextStyle, wrap) == 16 && offsetof(PxuiTextStyle, align) == 20 && offsetof(PxuiTextStyle, max_lines) == 24 && offsetof(PxuiTextStyle, italic) == 26 && offsetof(PxuiTextStyle, ellipsis) == 27 && offsetof(PxuiTextStyle, features) == 28 && offsetof(PxuiTextStyle, tracking) == 32, "TextStyle layout");
_Static_assert(sizeof(PxuiTextMetrics) == 16 && offsetof(PxuiTextMetrics, width) == 0 && offsetof(PxuiTextMetrics, height) == 4 && offsetof(PxuiTextMetrics, baseline) == 8 && offsetof(PxuiTextMetrics, lines) == 12, "TextMetrics layout");
_Static_assert(sizeof(PxuiTextGlyph) == 40 && offsetof(PxuiTextGlyph, rect) == 0 && offsetof(PxuiTextGlyph, uv) == 16 && offsetof(PxuiTextGlyph, atlas) == 32 && offsetof(PxuiTextGlyph, reserved) == 36, "TextGlyph layout");
_Static_assert(sizeof(PxuiBoxDesc) == 112 && offsetof(PxuiBoxDesc, size) == 0 && offsetof(PxuiBoxDesc, axis) == 4 && offsetof(PxuiBoxDesc, flags) == 8 && offsetof(PxuiBoxDesc, w) == 16 && offsetof(PxuiBoxDesc, h) == 32 && offsetof(PxuiBoxDesc, max_h) == 48 && offsetof(PxuiBoxDesc, padding) == 52 && offsetof(PxuiBoxDesc, scroll_step) == 56 && offsetof(PxuiBoxDesc, has_at) == 60 && offsetof(PxuiBoxDesc, has_hit_rect) == 61 && offsetof(PxuiBoxDesc, reserved) == 62 && offsetof(PxuiBoxDesc, at) == 64 && offsetof(PxuiBoxDesc, hit_rect) == 72 && offsetof(PxuiBoxDesc, hit) == 88 && offsetof(PxuiBoxDesc, hit_user) == 96 && offsetof(PxuiBoxDesc, hit_release) == 104, "PxuiBoxDesc layout");
_Static_assert(sizeof(PxuiPopupDesc) == 48 && offsetof(PxuiPopupDesc, size) == 0 && offsetof(PxuiPopupDesc, has_stroke) == 4 && offsetof(PxuiPopupDesc, has_max_height) == 5 && offsetof(PxuiPopupDesc, dismiss_initial) == 6 && offsetof(PxuiPopupDesc, attached) == 7 && offsetof(PxuiPopupDesc, stroke) == 8 && offsetof(PxuiPopupDesc, max_height) == 12 && offsetof(PxuiPopupDesc, x) == 16 && offsetof(PxuiPopupDesc, y) == 20 && offsetof(PxuiPopupDesc, width) == 24 && offsetof(PxuiPopupDesc, height) == 28 && offsetof(PxuiPopupDesc, label) == 32, "PxuiPopupDesc layout");
_Static_assert(sizeof(PxuiHeadActionDesc) == 40 && offsetof(PxuiHeadActionDesc, size) == 0 && offsetof(PxuiHeadActionDesc, active) == 4 && offsetof(PxuiHeadActionDesc, usable) == 5 && offsetof(PxuiHeadActionDesc, reserved) == 6 && offsetof(PxuiHeadActionDesc, caption) == 8 && offsetof(PxuiHeadActionDesc, keycap) == 24, "PxuiHeadActionDesc layout");
_Static_assert(sizeof(PxuiHeaderDesc) == 144 && offsetof(PxuiHeaderDesc, size) == 0 && offsetof(PxuiHeaderDesc, has_kind) == 4 && offsetof(PxuiHeaderDesc, has_badge) == 5 && offsetof(PxuiHeaderDesc, has_index) == 6 && offsetof(PxuiHeaderDesc, has_reset) == 7 && offsetof(PxuiHeaderDesc, reset_enabled) == 8 && offsetof(PxuiHeaderDesc, reserved) == 9 && offsetof(PxuiHeaderDesc, key) == 16 && offsetof(PxuiHeaderDesc, kind) == 32 && offsetof(PxuiHeaderDesc, badge) == 48 && offsetof(PxuiHeaderDesc, index) == 64 && offsetof(PxuiHeaderDesc, reset) == 80 && offsetof(PxuiHeaderDesc, title) == 96 && offsetof(PxuiHeaderDesc, detail) == 112 && offsetof(PxuiHeaderDesc, actions) == 128, "PxuiHeaderDesc layout");
_Static_assert(sizeof(PxuiHeadOut) == 32 && offsetof(PxuiHeadOut, size) == 0 && offsetof(PxuiHeadOut, has_chosen) == 4 && offsetof(PxuiHeadOut, reset_pressed) == 5 && offsetof(PxuiHeadOut, reserved) == 6 && offsetof(PxuiHeadOut, renamed) == 8 && offsetof(PxuiHeadOut, chosen) == 24, "PxuiHeadOut layout");
_Static_assert(sizeof(PxuiPaintInfo) == 192 && offsetof(PxuiPaintInfo, size) == 0 && offsetof(PxuiPaintInfo, rtl) == 4 && offsetof(PxuiPaintInfo, reserved) == 5 && offsetof(PxuiPaintInfo, font_size) == 8 && offsetof(PxuiPaintInfo, row_height) == 16 && offsetof(PxuiPaintInfo, density) == 24 && offsetof(PxuiPaintInfo, clip) == 32 && offsetof(PxuiPaintInfo, clip_radius) == 48 && offsetof(PxuiPaintInfo, theme) == 56, "PxuiPaintInfo layout");
_Static_assert(sizeof(PxuiValueFieldDesc) == 152 && offsetof(PxuiValueFieldDesc, size) == 0 && offsetof(PxuiValueFieldDesc, flags) == 4 && offsetof(PxuiValueFieldDesc, font_size) == 8 && offsetof(PxuiValueFieldDesc, fraction) == 16 && offsetof(PxuiValueFieldDesc, tracking) == 20 && offsetof(PxuiValueFieldDesc, line) == 24 && offsetof(PxuiValueFieldDesc, lead_color) == 28 && offsetof(PxuiValueFieldDesc, trail_color) == 32 && offsetof(PxuiValueFieldDesc, reserved) == 36 && offsetof(PxuiValueFieldDesc, x) == 40 && offsetof(PxuiValueFieldDesc, y) == 44 && offsetof(PxuiValueFieldDesc, width) == 48 && offsetof(PxuiValueFieldDesc, height) == 52 && offsetof(PxuiValueFieldDesc, display) == 56 && offsetof(PxuiValueFieldDesc, placeholder) == 72 && offsetof(PxuiValueFieldDesc, lead) == 88 && offsetof(PxuiValueFieldDesc, trail) == 104 && offsetof(PxuiValueFieldDesc, label) == 120 && offsetof(PxuiValueFieldDesc, value) == 136, "PxuiValueFieldDesc layout");
_Static_assert(sizeof(PxuiU8MutSpan) == 16 && offsetof(PxuiU8MutSpan, ptr) == 0 && offsetof(PxuiU8MutSpan, len) == 8, "PxuiU8MutSpan layout");
_Static_assert(sizeof(PxuiF32MutSpan) == 16 && offsetof(PxuiF32MutSpan, ptr) == 0 && offsetof(PxuiF32MutSpan, len) == 8, "PxuiF32MutSpan layout");
_Static_assert(sizeof(PxuiU32MutSpan) == 16 && offsetof(PxuiU32MutSpan, ptr) == 0 && offsetof(PxuiU32MutSpan, len) == 8, "PxuiU32MutSpan layout");
_Static_assert(sizeof(PxuiTreeNode) == 4 && offsetof(PxuiTreeNode, depth) == 0 && offsetof(PxuiTreeNode, children) == 2, "PxuiTreeNode layout");
_Static_assert(sizeof(PxuiListViewDesc) == 64 && offsetof(PxuiListViewDesc, size) == 0 && offsetof(PxuiListViewDesc, multi) == 4 && offsetof(PxuiListViewDesc, reserved) == 5 && offsetof(PxuiListViewDesc, label) == 8 && offsetof(PxuiListViewDesc, count) == 24 && offsetof(PxuiListViewDesc, selected) == 32 && offsetof(PxuiListViewDesc, text) == 48 && offsetof(PxuiListViewDesc, user) == 56, "PxuiListViewDesc layout");
_Static_assert(sizeof(PxuiTreeViewDesc) == 104 && offsetof(PxuiTreeViewDesc, size) == 0 && offsetof(PxuiTreeViewDesc, reserved) == 4 && offsetof(PxuiTreeViewDesc, label) == 8 && offsetof(PxuiTreeViewDesc, ptr) == 24 && offsetof(PxuiTreeViewDesc, open) == 40 && offsetof(PxuiTreeViewDesc, selected) == 56 && offsetof(PxuiTreeViewDesc, visible) == 64 && offsetof(PxuiTreeViewDesc, visible_len) == 80 && offsetof(PxuiTreeViewDesc, text) == 88 && offsetof(PxuiTreeViewDesc, user) == 96, "PxuiTreeViewDesc layout");
_Static_assert(sizeof(PxuiTableViewDesc) == 96 && offsetof(PxuiTableViewDesc, size) == 0 && offsetof(PxuiTableViewDesc, has_sort) == 4 && offsetof(PxuiTableViewDesc, ascending) == 5 && offsetof(PxuiTableViewDesc, reserved) == 6 && offsetof(PxuiTableViewDesc, label) == 8 && offsetof(PxuiTableViewDesc, titles) == 24 && offsetof(PxuiTableViewDesc, widths) == 40 && offsetof(PxuiTableViewDesc, rows) == 56 && offsetof(PxuiTableViewDesc, sort_column) == 64 && offsetof(PxuiTableViewDesc, selected) == 72 && offsetof(PxuiTableViewDesc, text) == 80 && offsetof(PxuiTableViewDesc, user) == 88, "PxuiTableViewDesc layout");
_Static_assert(sizeof(PxuiNodeFields) == 192 && offsetof(PxuiNodeFields, size) == 0 && offsetof(PxuiNodeFields, reserved) == 4 && offsetof(PxuiNodeFields, look) == 8 && offsetof(PxuiNodeFields, mark) == 160 && offsetof(PxuiNodeFields, inset) == 176, "PxuiNodeFields layout");
_Static_assert(sizeof(PxuiRange) == 16 && offsetof(PxuiRange, start) == 0 && offsetof(PxuiRange, end) == 8, "PxuiRange layout");
_Static_assert(sizeof(PxuiColorSpan) == 24 && offsetof(PxuiColorSpan, start) == 0 && offsetof(PxuiColorSpan, end) == 8 && offsetof(PxuiColorSpan, color) == 16 && offsetof(PxuiColorSpan, reserved) == 20, "PxuiColorSpan layout");
_Static_assert(sizeof(PxuiNote) == 48 && offsetof(PxuiNote, line) == 0 && offsetof(PxuiNote, start) == 8 && offsetof(PxuiNote, end) == 16 && offsetof(PxuiNote, has_range) == 24 && offsetof(PxuiNote, reserved) == 25 && offsetof(PxuiNote, text) == 32, "PxuiNote layout");
_Static_assert(sizeof(PxuiSplice) == 32 && offsetof(PxuiSplice, start) == 0 && offsetof(PxuiSplice, end) == 8 && offsetof(PxuiSplice, text) == 16, "PxuiSplice layout");
_Static_assert(sizeof(PxuiCompletion) == 80 && offsetof(PxuiCompletion, start) == 0 && offsetof(PxuiCompletion, end) == 8 && offsetof(PxuiCompletion, insert) == 16 && offsetof(PxuiCompletion, label) == 32 && offsetof(PxuiCompletion, detail) == 48 && offsetof(PxuiCompletion, doc) == 64, "PxuiCompletion layout");
_Static_assert(sizeof(PxuiDescription) == 40 && offsetof(PxuiDescription, has) == 0 && offsetof(PxuiDescription, reserved) == 1 && offsetof(PxuiDescription, start) == 8 && offsetof(PxuiDescription, end) == 16 && offsetof(PxuiDescription, text) == 24, "PxuiDescription layout");
_Static_assert(sizeof(PxuiMaybeRange) == 24 && offsetof(PxuiMaybeRange, has) == 0 && offsetof(PxuiMaybeRange, reserved) == 1 && offsetof(PxuiMaybeRange, start) == 8 && offsetof(PxuiMaybeRange, end) == 16, "PxuiMaybeRange layout");
_Static_assert(sizeof(PxuiRewrite) == 24 && offsetof(PxuiRewrite, text) == 0 && offsetof(PxuiRewrite, caret) == 16, "PxuiRewrite layout");
_Static_assert(sizeof(PxuiTextAreaDesc) == 200 && offsetof(PxuiTextAreaDesc, size) == 0 && offsetof(PxuiTextAreaDesc, flags) == 4 && offsetof(PxuiTextAreaDesc, x) == 8 && offsetof(PxuiTextAreaDesc, y) == 12 && offsetof(PxuiTextAreaDesc, width) == 16 && offsetof(PxuiTextAreaDesc, height) == 20 && offsetof(PxuiTextAreaDesc, reveal) == 24 && offsetof(PxuiTextAreaDesc, reveal_nonce) == 32 && offsetof(PxuiTextAreaDesc, select) == 40 && offsetof(PxuiTextAreaDesc, label) == 56 && offsetof(PxuiTextAreaDesc, value) == 72 && offsetof(PxuiTextAreaDesc, errors) == 88 && offsetof(PxuiTextAreaDesc, messages) == 104 && offsetof(PxuiTextAreaDesc, spans) == 120 && offsetof(PxuiTextAreaDesc, splice) == 136 && offsetof(PxuiTextAreaDesc, chips) == 152 && offsetof(PxuiTextAreaDesc, comment) == 168 && offsetof(PxuiTextAreaDesc, pairs) == 184, "PxuiTextAreaDesc layout");
_Static_assert(sizeof(PxuiAreaCallbacks) == 144 && offsetof(PxuiAreaCallbacks, size) == 0 && offsetof(PxuiAreaCallbacks, reserved) == 4 && offsetof(PxuiAreaCallbacks, user) == 8 && offsetof(PxuiAreaCallbacks, on_context) == 16 && offsetof(PxuiAreaCallbacks, on_scrub) == 24 && offsetof(PxuiAreaCallbacks, on_scrub_edit) == 32 && offsetof(PxuiAreaCallbacks, on_click) == 40 && offsetof(PxuiAreaCallbacks, on_caret) == 48 && offsetof(PxuiAreaCallbacks, on_caret_move) == 56 && offsetof(PxuiAreaCallbacks, on_select) == 64 && offsetof(PxuiAreaCallbacks, on_drop) == 72 && offsetof(PxuiAreaCallbacks, colorize) == 80 && offsetof(PxuiAreaCallbacks, brackets) == 88 && offsetof(PxuiAreaCallbacks, indent) == 96 && offsetof(PxuiAreaCallbacks, complete) == 104 && offsetof(PxuiAreaCallbacks, describe) == 112 && offsetof(PxuiAreaCallbacks, number_at) == 120 && offsetof(PxuiAreaCallbacks, context_at) == 128 && offsetof(PxuiAreaCallbacks, rewrite) == 136, "PxuiAreaCallbacks layout");
_Static_assert(sizeof(PxuiSwatch) == 8 && offsetof(PxuiSwatch, has) == 0 && offsetof(PxuiSwatch, reserved) == 1 && offsetof(PxuiSwatch, color) == 4, "PxuiSwatch layout");
_Static_assert(sizeof(PxuiSubmenu) == 72 && offsetof(PxuiSubmenu, row) == 0 && offsetof(PxuiSubmenu, current) == 8 && offsetof(PxuiSubmenu, has_current) == 16 && offsetof(PxuiSubmenu, reserved) == 17 && offsetof(PxuiSubmenu, rows) == 24 && offsetof(PxuiSubmenu, enabled) == 40 && offsetof(PxuiSubmenu, keys) == 56, "PxuiSubmenu layout");
_Static_assert(sizeof(PxuiMenuExtra) == 144 && offsetof(PxuiMenuExtra, size) == 0 && offsetof(PxuiMenuExtra, reserved) == 4 && offsetof(PxuiMenuExtra, lead_from) == 8 && offsetof(PxuiMenuExtra, keys) == 16 && offsetof(PxuiMenuExtra, swatches) == 32 && offsetof(PxuiMenuExtra, danger) == 48 && offsetof(PxuiMenuExtra, submenus) == 64 && offsetof(PxuiMenuExtra, row_h) == 80 && offsetof(PxuiMenuExtra, max_rows) == 84 && offsetof(PxuiMenuExtra, checked) == 88 && offsetof(PxuiMenuExtra, padding) == 104 && offsetof(PxuiMenuExtra, has_name) == 108 && offsetof(PxuiMenuExtra, reserved2) == 109 && offsetof(PxuiMenuExtra, name) == 112 && offsetof(PxuiMenuExtra, selected) == 128, "PxuiMenuExtra layout");
_Static_assert(sizeof(PxuiLayerInfo) == 48 && offsetof(PxuiLayerInfo, key) == 0 && offsetof(PxuiLayerInfo, id) == 8 && offsetof(PxuiLayerInfo, breaks) == 12 && offsetof(PxuiLayerInfo, instances) == 16 && offsetof(PxuiLayerInfo, dirty) == 32, "PxuiLayerInfo layout");
_Static_assert(sizeof(PxuiDirty) == 8 && offsetof(PxuiDirty, start) == 0 && offsetof(PxuiDirty, end) == 4, "PxuiDirty layout");
_Static_assert(sizeof(PxuiSize) == 16, "PxuiSize layout");
_Static_assert(sizeof(PxuiTrack) == 8, "PxuiTrack layout");
_Static_assert(sizeof(PxuiMark) == 12, "PxuiMark layout");
_Static_assert(sizeof(PxuiTransition) == 24, "PxuiTransition layout");
_Static_assert(sizeof(PxuiOptAlign) == 2, "PxuiOptAlign layout");
_Static_assert(sizeof(PxuiOptGrid) == 40, "PxuiOptGrid layout");
_Static_assert(sizeof(PxuiOptAnchor) == 36, "PxuiOptAnchor layout");
_Static_assert(sizeof(PxuiOptPoint) == 12, "PxuiOptPoint layout");
_Static_assert(sizeof(PxuiOptBar) == 28, "PxuiOptBar layout");
_Static_assert(sizeof(PxuiOptTransition) == 28, "PxuiOptTransition layout");
_Static_assert(sizeof(PxuiOptColor) == 5, "PxuiOptColor layout");
_Static_assert(sizeof(PxuiGrid) == 32, "PxuiGrid layout");
_Static_assert(sizeof(PxuiCell) == 8 && offsetof(PxuiCell, col) == 0 && offsetof(PxuiCell, row) == 2 && offsetof(PxuiCell, col_span) == 4 && offsetof(PxuiCell, row_span) == 6, "PxuiCell layout");
_Static_assert(sizeof(PxuiPlacement) == 16 && offsetof(PxuiPlacement, side) == 0 && offsetof(PxuiPlacement, align) == 1 && offsetof(PxuiPlacement, gap) == 4 && offsetof(PxuiPlacement, margin) == 8 && offsetof(PxuiPlacement, flip) == 12, "PxuiPlacement layout");
_Static_assert(sizeof(PxuiAnchor) == 32 && offsetof(PxuiAnchor, rect) == 0 && offsetof(PxuiAnchor, placement) == 16, "PxuiAnchor layout");
_Static_assert(sizeof(PxuiFont) == 32 && offsetof(PxuiFont, family) == 0 && offsetof(PxuiFont, size) == 8 && offsetof(PxuiFont, weight) == 16 && offsetof(PxuiFont, tracking) == 20 && offsetof(PxuiFont, caps) == 24, "PxuiFont layout");
_Static_assert(sizeof(PxuiShadow) == 24 && offsetof(PxuiShadow, color) == 0 && offsetof(PxuiShadow, offset) == 4 && offsetof(PxuiShadow, blur) == 12 && offsetof(PxuiShadow, spread) == 16 && offsetof(PxuiShadow, inner) == 20, "PxuiShadow layout");
_Static_assert(sizeof(PxuiRing) == 12 && offsetof(PxuiRing, color) == 0 && offsetof(PxuiRing, width) == 4 && offsetof(PxuiRing, sides) == 8, "PxuiRing layout");
_Static_assert(sizeof(PxuiLook) == 160 && offsetof(PxuiLook, fill) == 0 && offsetof(PxuiLook, gradient) == 4 && offsetof(PxuiLook, angle) == 12 && offsetof(PxuiLook, border) == 16 && offsetof(PxuiLook, border_w) == 20 && offsetof(PxuiLook, sides) == 24 && offsetof(PxuiLook, radius) == 28 && offsetof(PxuiLook, shadows) == 44 && offsetof(PxuiLook, text) == 92 && offsetof(PxuiLook, font) == 96 && offsetof(PxuiLook, opacity) == 128 && offsetof(PxuiLook, ring) == 132 && offsetof(PxuiLook, selection) == 144 && offsetof(PxuiLook, caret) == 148 && offsetof(PxuiLook, dashed) == 152, "PxuiLook layout");
_Static_assert(sizeof(PxuiState) == 5 && offsetof(PxuiState, hovered) == 0 && offsetof(PxuiState, pressed) == 1 && offsetof(PxuiState, focused) == 2 && offsetof(PxuiState, disabled) == 3 && offsetof(PxuiState, checked) == 4, "PxuiState layout");
_Static_assert(sizeof(PxuiBar) == 24 && offsetof(PxuiBar, color) == 0 && offsetof(PxuiBar, radius) == 4 && offsetof(PxuiBar, width) == 8 && offsetof(PxuiBar, gap) == 12 && offsetof(PxuiBar, margin) == 16 && offsetof(PxuiBar, whole) == 20, "PxuiBar layout");
_Static_assert(sizeof(PxuiNode) == 488 && offsetof(PxuiNode, size) == 0 && offsetof(PxuiNode, axis) == 4 && offsetof(PxuiNode, justify) == 5 && offsetof(PxuiNode, align) == 6 && offsetof(PxuiNode, wrap) == 7 && offsetof(PxuiNode, role) == 8 && offsetof(PxuiNode, state) == 9 && offsetof(PxuiNode, z) == 14 && offsetof(PxuiNode, flags) == 16 && offsetof(PxuiNode, w) == 24 && offsetof(PxuiNode, h) == 40 && offsetof(PxuiNode, min_w) == 56 && offsetof(PxuiNode, min_h) == 60 && offsetof(PxuiNode, max_w) == 64 && offsetof(PxuiNode, max_h) == 68 && offsetof(PxuiNode, grow) == 72 && offsetof(PxuiNode, shrink) == 76 && offsetof(PxuiNode, aspect) == 80 && offsetof(PxuiNode, padding) == 84 && offsetof(PxuiNode, margin) == 100 && offsetof(PxuiNode, gap) == 116 && offsetof(PxuiNode, row_gap) == 120 && offsetof(PxuiNode, grid) == 128 && offsetof(PxuiNode, cell) == 168 && offsetof(PxuiNode, align_self) == 176 && offsetof(PxuiNode, anchor) == 180 && offsetof(PxuiNode, a11y) == 216 && offsetof(PxuiNode, at) == 220 && offsetof(PxuiNode, scroll_step) == 232 && offsetof(PxuiNode, look) == 240 && offsetof(PxuiNode, inset) == 400 && offsetof(PxuiNode, mark) == 416 && offsetof(PxuiNode, bar) == 428 && offsetof(PxuiNode, transition) == 456, "PxuiNode layout");
_Static_assert(sizeof(PxuiTokens) == 136 && offsetof(PxuiTokens, panel) == 0 && offsetof(PxuiTokens, foreground) == 4 && offsetof(PxuiTokens, control) == 8 && offsetof(PxuiTokens, input) == 12 && offsetof(PxuiTokens, track) == 16 && offsetof(PxuiTokens, accent) == 20 && offsetof(PxuiTokens, invalid) == 24 && offsetof(PxuiTokens, ink_ground) == 28 && offsetof(PxuiTokens, space) == 36 && offsetof(PxuiTokens, radius) == 52 && offsetof(PxuiTokens, display_size) == 64 && offsetof(PxuiTokens, title_size) == 72 && offsetof(PxuiTokens, font_size) == 80 && offsetof(PxuiTokens, label_size) == 88 && offsetof(PxuiTokens, motion) == 96 && offsetof(PxuiTokens, reduced_motion) == 124 && offsetof(PxuiTokens, density) == 128, "PxuiTokens layout");
_Static_assert(sizeof(PxuiStates) == 960 && offsetof(PxuiStates, idle) == 0 && offsetof(PxuiStates, hover) == 160 && offsetof(PxuiStates, pressed) == 320 && offsetof(PxuiStates, focus) == 480 && offsetof(PxuiStates, disabled) == 640 && offsetof(PxuiStates, checked) == 800, "PxuiStates layout");
_Static_assert(sizeof(PxuiThemeSnapshot) == 56888 && offsetof(PxuiThemeSnapshot, tokens) == 0 && offsetof(PxuiThemeSnapshot, row_h) == 136 && offsetof(PxuiThemeSnapshot, control_h) == 140 && offsetof(PxuiThemeSnapshot, ring) == 144 && offsetof(PxuiThemeSnapshot, scrollbar_w) == 156 && offsetof(PxuiThemeSnapshot, scrollbar_gap) == 160 && offsetof(PxuiThemeSnapshot, parts) == 168 && offsetof(PxuiThemeSnapshot, bodies) == 56808, "PxuiThemeSnapshot layout");
_Static_assert(sizeof(PxuiToggleConfig) == 192 && offsetof(PxuiToggleConfig, size) == 0 && offsetof(PxuiToggleConfig, state) == 4 && offsetof(PxuiToggleConfig, on) == 12 && offsetof(PxuiToggleConfig, row) == 16 && offsetof(PxuiToggleConfig, label) == 48 && offsetof(PxuiToggleConfig, row_h) == 64 && offsetof(PxuiToggleConfig, switch_w) == 68 && offsetof(PxuiToggleConfig, switch_h) == 72 && offsetof(PxuiToggleConfig, track) == 80 && offsetof(PxuiToggleConfig, knob) == 88 && offsetof(PxuiToggleConfig, inset) == 96 && offsetof(PxuiToggleConfig, press_row) == 100 && offsetof(PxuiToggleConfig, detail) == 104 && offsetof(PxuiToggleConfig, variant) == 120 && offsetof(PxuiToggleConfig, has_style) == 122 && offsetof(PxuiToggleConfig, reserved2) == 123 && offsetof(PxuiToggleConfig, style) == 128, "PxuiToggleConfig layout");
_Static_assert(sizeof(PxuiSliderConfig) == 72 && offsetof(PxuiSliderConfig, size) == 0 && offsetof(PxuiSliderConfig, state) == 4 && offsetof(PxuiSliderConfig, fraction) == 12 && offsetof(PxuiSliderConfig, track) == 16 && offsetof(PxuiSliderConfig, label) == 48 && offsetof(PxuiSliderConfig, variant) == 64, "PxuiSliderConfig layout");
_Static_assert(sizeof(PxuiProgressConfig) == 72 && offsetof(PxuiProgressConfig, size) == 0 && offsetof(PxuiProgressConfig, has_value) == 4 && offsetof(PxuiProgressConfig, value) == 8 && offsetof(PxuiProgressConfig, row) == 16 && offsetof(PxuiProgressConfig, label) == 48 && offsetof(PxuiProgressConfig, variant) == 64, "PxuiProgressConfig layout");
_Static_assert(sizeof(PxuiButtonConfig) == 232 && offsetof(PxuiButtonConfig, size) == 0 && offsetof(PxuiButtonConfig, state) == 4 && offsetof(PxuiButtonConfig, bare) == 12 && offsetof(PxuiButtonConfig, icon) == 13 && offsetof(PxuiButtonConfig, at_end) == 14 && offsetof(PxuiButtonConfig, line) == 16 && offsetof(PxuiButtonConfig, row) == 48 && offsetof(PxuiButtonConfig, states) == 80 && offsetof(PxuiButtonConfig, label) == 88 && offsetof(PxuiButtonConfig, w_kind) == 104 && offsetof(PxuiButtonConfig, h_kind) == 105 && offsetof(PxuiButtonConfig, justify) == 106 && offsetof(PxuiButtonConfig, reserved) == 107 && offsetof(PxuiButtonConfig, w) == 108 && offsetof(PxuiButtonConfig, h) == 112 && offsetof(PxuiButtonConfig, padding) == 116 && offsetof(PxuiButtonConfig, variant) == 132 && offsetof(PxuiButtonConfig, centered) == 134 && offsetof(PxuiButtonConfig, reserved2) == 135 && offsetof(PxuiButtonConfig, has_label) == 144 && offsetof(PxuiButtonConfig, axis) == 145 && offsetof(PxuiButtonConfig, content_align) == 146 && offsetof(PxuiButtonConfig, reserved3) == 147 && offsetof(PxuiButtonConfig, gap) == 148 && offsetof(PxuiButtonConfig, text_style) == 152 && offsetof(PxuiButtonConfig, text_box) == 216, "PxuiButtonConfig layout");
_Static_assert(sizeof(PxuiIconButtonConfig) == 136 && offsetof(PxuiIconButtonConfig, size) == 0 && offsetof(PxuiIconButtonConfig, state) == 4 && offsetof(PxuiIconButtonConfig, node) == 16 && offsetof(PxuiIconButtonConfig, label) == 48 && offsetof(PxuiIconButtonConfig, icon) == 64 && offsetof(PxuiIconButtonConfig, states) == 80 && offsetof(PxuiIconButtonConfig, w_kind) == 88 && offsetof(PxuiIconButtonConfig, h_kind) == 89 && offsetof(PxuiIconButtonConfig, reserved) == 90 && offsetof(PxuiIconButtonConfig, w) == 92 && offsetof(PxuiIconButtonConfig, h) == 96 && offsetof(PxuiIconButtonConfig, press_scale) == 100 && offsetof(PxuiIconButtonConfig, draw) == 104 && offsetof(PxuiIconButtonConfig, draw_user) == 112 && offsetof(PxuiIconButtonConfig, ink) == 120 && offsetof(PxuiIconButtonConfig, variant) == 124 && offsetof(PxuiIconButtonConfig, press) == 128, "PxuiIconButtonConfig layout");
_Static_assert(sizeof(PxuiCheckConfig) == 72 && offsetof(PxuiCheckConfig, size) == 0 && offsetof(PxuiCheckConfig, state) == 4 && offsetof(PxuiCheckConfig, on) == 12 && offsetof(PxuiCheckConfig, row) == 16 && offsetof(PxuiCheckConfig, label) == 48 && offsetof(PxuiCheckConfig, variant) == 64, "PxuiCheckConfig layout");
_Static_assert(sizeof(PxuiSpinnerConfig) == 64 && offsetof(PxuiSpinnerConfig, size) == 0 && offsetof(PxuiSpinnerConfig, node) == 8 && offsetof(PxuiSpinnerConfig, label) == 40 && offsetof(PxuiSpinnerConfig, variant) == 56, "PxuiSpinnerConfig layout");
_Static_assert(sizeof(PxuiBadgeConfig) == 80 && offsetof(PxuiBadgeConfig, size) == 0 && offsetof(PxuiBadgeConfig, node) == 8 && offsetof(PxuiBadgeConfig, label) == 40 && offsetof(PxuiBadgeConfig, text) == 56 && offsetof(PxuiBadgeConfig, variant) == 72, "PxuiBadgeConfig layout");
_Static_assert(sizeof(PxuiLabelConfig) == 144 && offsetof(PxuiLabelConfig, size) == 0 && offsetof(PxuiLabelConfig, first) == 4 && offsetof(PxuiLabelConfig, node) == 8 && offsetof(PxuiLabelConfig, text) == 40 && offsetof(PxuiLabelConfig, style) == 56 && offsetof(PxuiLabelConfig, variant) == 120 && offsetof(PxuiLabelConfig, label_box) == 124, "PxuiLabelConfig layout");
_Static_assert(sizeof(PxuiFooterConfig) == 88 && offsetof(PxuiFooterConfig, size) == 0 && offsetof(PxuiFooterConfig, row) == 8 && offsetof(PxuiFooterConfig, has_right) == 40 && offsetof(PxuiFooterConfig, right) == 48 && offsetof(PxuiFooterConfig, hints) == 64 && offsetof(PxuiFooterConfig, variant) == 80, "PxuiFooterConfig layout");
_Static_assert(sizeof(PxuiGripConfig) == 56 && offsetof(PxuiGripConfig, size) == 0 && offsetof(PxuiGripConfig, state) == 4 && offsetof(PxuiGripConfig, vertical) == 12 && offsetof(PxuiGripConfig, floating) == 13 && offsetof(PxuiGripConfig, grip) == 16 && offsetof(PxuiGripConfig, variant) == 48, "PxuiGripConfig layout");
_Static_assert(sizeof(PxuiPanelConfig) == 72 && offsetof(PxuiPanelConfig, size) == 0 && offsetof(PxuiPanelConfig, node) == 8 && offsetof(PxuiPanelConfig, label) == 40 && offsetof(PxuiPanelConfig, content) == 56 && offsetof(PxuiPanelConfig, variant) == 64, "PxuiPanelConfig layout");
_Static_assert(sizeof(PxuiDialogConfig) == 88 && offsetof(PxuiDialogConfig, size) == 0 && offsetof(PxuiDialogConfig, title) == 8 && offsetof(PxuiDialogConfig, buttons) == 24 && offsetof(PxuiDialogConfig, content) == 40 && offsetof(PxuiDialogConfig, default_at) == 48 && offsetof(PxuiDialogConfig, cancel_at) == 56 && offsetof(PxuiDialogConfig, danger) == 64 && offsetof(PxuiDialogConfig, disabled) == 72 && offsetof(PxuiDialogConfig, variant) == 80, "PxuiDialogConfig layout");
_Static_assert(sizeof(PxuiSectionConfig) == 80 && offsetof(PxuiSectionConfig, size) == 0 && offsetof(PxuiSectionConfig, open) == 4 && offsetof(PxuiSectionConfig, gap) == 8 && offsetof(PxuiSectionConfig, title) == 16 && offsetof(PxuiSectionConfig, label) == 48 && offsetof(PxuiSectionConfig, content) == 64 && offsetof(PxuiSectionConfig, variant) == 72, "PxuiSectionConfig layout");
_Static_assert(sizeof(PxuiInspectorRowConfig) == 80 && offsetof(PxuiInspectorRowConfig, size) == 0 && offsetof(PxuiInspectorRowConfig, state) == 4 && offsetof(PxuiInspectorRowConfig, has_pin) == 12 && offsetof(PxuiInspectorRowConfig, pin) == 13 && offsetof(PxuiInspectorRowConfig, label_w) == 16 && offsetof(PxuiInspectorRowConfig, row) == 24 && offsetof(PxuiInspectorRowConfig, label) == 56 && offsetof(PxuiInspectorRowConfig, variant) == 72, "PxuiInspectorRowConfig layout");
_Static_assert(sizeof(PxuiSplitConfig) == 80 && offsetof(PxuiSplitConfig, size) == 0 && offsetof(PxuiSplitConfig, axis) == 4 && offsetof(PxuiSplitConfig, ratio) == 8 && offsetof(PxuiSplitConfig, min_a) == 12 && offsetof(PxuiSplitConfig, min_b) == 16 && offsetof(PxuiSplitConfig, split) == 24 && offsetof(PxuiSplitConfig, first) == 56 && offsetof(PxuiSplitConfig, second) == 64 && offsetof(PxuiSplitConfig, variant) == 72, "PxuiSplitConfig layout");
_Static_assert(sizeof(PxuiScrollConfig) == 56 && offsetof(PxuiScrollConfig, size) == 0 && offsetof(PxuiScrollConfig, node) == 8 && offsetof(PxuiScrollConfig, content) == 40 && offsetof(PxuiScrollConfig, variant) == 48, "PxuiScrollConfig layout");
_Static_assert(sizeof(PxuiToastConfig) == 72 && offsetof(PxuiToastConfig, size) == 0 && offsetof(PxuiToastConfig, state) == 4 && offsetof(PxuiToastConfig, node) == 16 && offsetof(PxuiToastConfig, text) == 48 && offsetof(PxuiToastConfig, variant) == 64, "PxuiToastConfig layout");
_Static_assert(sizeof(PxuiTipConfig) == 72 && offsetof(PxuiTipConfig, size) == 0 && offsetof(PxuiTipConfig, width) == 4 && offsetof(PxuiTipConfig, height) == 8 && offsetof(PxuiTipConfig, tip) == 16 && offsetof(PxuiTipConfig, lines) == 48 && offsetof(PxuiTipConfig, variant) == 64, "PxuiTipConfig layout");
_Static_assert(sizeof(PxuiTabConfig) == 80 && offsetof(PxuiTabConfig, size) == 0 && offsetof(PxuiTabConfig, state) == 4 && offsetof(PxuiTabConfig, index) == 16 && offsetof(PxuiTabConfig, node) == 24 && offsetof(PxuiTabConfig, text) == 56 && offsetof(PxuiTabConfig, variant) == 72, "PxuiTabConfig layout");
_Static_assert(sizeof(PxuiSegmentConfig) == 112 && offsetof(PxuiSegmentConfig, size) == 0 && offsetof(PxuiSegmentConfig, state) == 4 && offsetof(PxuiSegmentConfig, index) == 16 && offsetof(PxuiSegmentConfig, selected) == 24 && offsetof(PxuiSegmentConfig, group) == 32 && offsetof(PxuiSegmentConfig, node) == 40 && offsetof(PxuiSegmentConfig, text) == 72 && offsetof(PxuiSegmentConfig, states) == 88 && offsetof(PxuiSegmentConfig, grow) == 96 && offsetof(PxuiSegmentConfig, variant) == 98 && offsetof(PxuiSegmentConfig, padding) == 100 && offsetof(PxuiSegmentConfig, min_segment) == 104, "PxuiSegmentConfig layout");
_Static_assert(sizeof(PxuiDropdownConfig) == 80 && offsetof(PxuiDropdownConfig, size) == 0 && offsetof(PxuiDropdownConfig, state) == 4 && offsetof(PxuiDropdownConfig, open) == 12 && offsetof(PxuiDropdownConfig, key) == 16 && offsetof(PxuiDropdownConfig, button) == 24 && offsetof(PxuiDropdownConfig, text) == 56 && offsetof(PxuiDropdownConfig, variant) == 72, "PxuiDropdownConfig layout");
_Static_assert(sizeof(PxuiMenuItemConfig) == 80 && offsetof(PxuiMenuItemConfig, size) == 0 && offsetof(PxuiMenuItemConfig, state) == 4 && offsetof(PxuiMenuItemConfig, index) == 16 && offsetof(PxuiMenuItemConfig, node) == 24 && offsetof(PxuiMenuItemConfig, text) == 56 && offsetof(PxuiMenuItemConfig, variant) == 72, "PxuiMenuItemConfig layout");
_Static_assert(sizeof(PxuiMenuRowConfig) == 112 && offsetof(PxuiMenuRowConfig, size) == 0 && offsetof(PxuiMenuRowConfig, state) == 4 && offsetof(PxuiMenuRowConfig, selected) == 12 && offsetof(PxuiMenuRowConfig, chevron) == 13 && offsetof(PxuiMenuRowConfig, danger) == 14 && offsetof(PxuiMenuRowConfig, has_swatch) == 15 && offsetof(PxuiMenuRowConfig, swatch) == 16 && offsetof(PxuiMenuRowConfig, has_shortcut) == 20 && offsetof(PxuiMenuRowConfig, index) == 24 && offsetof(PxuiMenuRowConfig, lead) == 32 && offsetof(PxuiMenuRowConfig, swatch_pad) == 36 && offsetof(PxuiMenuRowConfig, row) == 40 && offsetof(PxuiMenuRowConfig, text) == 72 && offsetof(PxuiMenuRowConfig, shortcut) == 88 && offsetof(PxuiMenuRowConfig, row_h) == 104 && offsetof(PxuiMenuRowConfig, variant) == 108, "PxuiMenuRowConfig layout");
_Static_assert(sizeof(PxuiRowConfig) == 72 && offsetof(PxuiRowConfig, size) == 0 && offsetof(PxuiRowConfig, state) == 4 && offsetof(PxuiRowConfig, index) == 16 && offsetof(PxuiRowConfig, row) == 24 && offsetof(PxuiRowConfig, content) == 56 && offsetof(PxuiRowConfig, variant) == 64, "PxuiRowConfig layout");
_Static_assert(sizeof(PxuiTreeRowConfig) == 88 && offsetof(PxuiTreeRowConfig, size) == 0 && offsetof(PxuiTreeRowConfig, state) == 4 && offsetof(PxuiTreeRowConfig, has) == 12 && offsetof(PxuiTreeRowConfig, open) == 13 && offsetof(PxuiTreeRowConfig, index) == 16 && offsetof(PxuiTreeRowConfig, node) == 24 && offsetof(PxuiTreeRowConfig, depth) == 32 && offsetof(PxuiTreeRowConfig, row) == 40 && offsetof(PxuiTreeRowConfig, content) == 72 && offsetof(PxuiTreeRowConfig, variant) == 80, "PxuiTreeRowConfig layout");
_Static_assert(sizeof(PxuiTableHeadConfig) == 80 && offsetof(PxuiTableHeadConfig, size) == 0 && offsetof(PxuiTableHeadConfig, state) == 4 && offsetof(PxuiTableHeadConfig, has_ascending) == 12 && offsetof(PxuiTableHeadConfig, ascending) == 13 && offsetof(PxuiTableHeadConfig, index) == 16 && offsetof(PxuiTableHeadConfig, node) == 24 && offsetof(PxuiTableHeadConfig, title) == 56 && offsetof(PxuiTableHeadConfig, variant) == 72, "PxuiTableHeadConfig layout");
_Static_assert(sizeof(PxuiTableCellConfig) == 72 && offsetof(PxuiTableCellConfig, size) == 0 && offsetof(PxuiTableCellConfig, header) == 4 && offsetof(PxuiTableCellConfig, left) == 8 && offsetof(PxuiTableCellConfig, right) == 12 && offsetof(PxuiTableCellConfig, height) == 16 && offsetof(PxuiTableCellConfig, row) == 24 && offsetof(PxuiTableCellConfig, column) == 32 && offsetof(PxuiTableCellConfig, key) == 40 && offsetof(PxuiTableCellConfig, text) == 48 && offsetof(PxuiTableCellConfig, variant) == 64, "PxuiTableCellConfig layout");
_Static_assert(sizeof(PxuiPickerRowConfig) == 120 && offsetof(PxuiPickerRowConfig, size) == 0 && offsetof(PxuiPickerRowConfig, state) == 4 && offsetof(PxuiPickerRowConfig, current) == 12 && offsetof(PxuiPickerRowConfig, doomed) == 13 && offsetof(PxuiPickerRowConfig, unavailable) == 14 && offsetof(PxuiPickerRowConfig, marked) == 15 && offsetof(PxuiPickerRowConfig, has_swatch) == 16 && offsetof(PxuiPickerRowConfig, swatch) == 17 && offsetof(PxuiPickerRowConfig, index) == 24 && offsetof(PxuiPickerRowConfig, row) == 32 && offsetof(PxuiPickerRowConfig, label) == 64 && offsetof(PxuiPickerRowConfig, detail) == 80 && offsetof(PxuiPickerRowConfig, query) == 96 && offsetof(PxuiPickerRowConfig, variant) == 112, "PxuiPickerRowConfig layout");
_Static_assert(sizeof(PxuiSearchFieldConfig) == 88 && offsetof(PxuiSearchFieldConfig, size) == 0 && offsetof(PxuiSearchFieldConfig, state) == 4 && offsetof(PxuiSearchFieldConfig, key) == 16 && offsetof(PxuiSearchFieldConfig, well) == 24 && offsetof(PxuiSearchFieldConfig, icon) == 56 && offsetof(PxuiSearchFieldConfig, editor) == 72 && offsetof(PxuiSearchFieldConfig, variant) == 80, "PxuiSearchFieldConfig layout");
_Static_assert(sizeof(PxuiTextFieldConfig) == 88 && offsetof(PxuiTextFieldConfig, size) == 0 && offsetof(PxuiTextFieldConfig, state) == 4 && offsetof(PxuiTextFieldConfig, row) == 16 && offsetof(PxuiTextFieldConfig, label) == 48 && offsetof(PxuiTextFieldConfig, value) == 64 && offsetof(PxuiTextFieldConfig, variant) == 80, "PxuiTextFieldConfig layout");
_Static_assert(sizeof(PxuiLineFieldConfig) == 72 && offsetof(PxuiLineFieldConfig, size) == 0 && offsetof(PxuiLineFieldConfig, state) == 4 && offsetof(PxuiLineFieldConfig, empty) == 12 && offsetof(PxuiLineFieldConfig, field) == 16 && offsetof(PxuiLineFieldConfig, text) == 48 && offsetof(PxuiLineFieldConfig, variant) == 64, "PxuiLineFieldConfig layout");
_Static_assert(sizeof(PxuiValueFieldConfig) == 232 && offsetof(PxuiValueFieldConfig, size) == 0 && offsetof(PxuiValueFieldConfig, state) == 4 && offsetof(PxuiValueFieldConfig, invalid) == 12 && offsetof(PxuiValueFieldConfig, w) == 16 && offsetof(PxuiValueFieldConfig, h) == 20 && offsetof(PxuiValueFieldConfig, field) == 24 && offsetof(PxuiValueFieldConfig, opts) == 56 && offsetof(PxuiValueFieldConfig, text) == 208 && offsetof(PxuiValueFieldConfig, variant) == 224, "PxuiValueFieldConfig layout");
_Static_assert(sizeof(PxuiPickerFieldConfig) == 80 && offsetof(PxuiPickerFieldConfig, size) == 0 && offsetof(PxuiPickerFieldConfig, state) == 4 && offsetof(PxuiPickerFieldConfig, empty) == 12 && offsetof(PxuiPickerFieldConfig, slash) == 13 && offsetof(PxuiPickerFieldConfig, prefix) == 16 && offsetof(PxuiPickerFieldConfig, inset) == 20 && offsetof(PxuiPickerFieldConfig, search) == 24 && offsetof(PxuiPickerFieldConfig, placeholder) == 56 && offsetof(PxuiPickerFieldConfig, variant) == 72, "PxuiPickerFieldConfig layout");
_Static_assert(sizeof(PxuiSlot) == 8 && offsetof(PxuiSlot, serial) == 0, "PxuiSlot layout");
_Static_assert(sizeof(PxuiListViewWithDesc) == 64 && offsetof(PxuiListViewWithDesc, size) == 0 && offsetof(PxuiListViewWithDesc, multi) == 4 && offsetof(PxuiListViewWithDesc, reserved) == 5 && offsetof(PxuiListViewWithDesc, label) == 8 && offsetof(PxuiListViewWithDesc, count) == 24 && offsetof(PxuiListViewWithDesc, selected) == 32 && offsetof(PxuiListViewWithDesc, build) == 48 && offsetof(PxuiListViewWithDesc, user) == 56, "PxuiListViewWithDesc layout");
_Static_assert(sizeof(PxuiTreeViewWithDesc) == 104 && offsetof(PxuiTreeViewWithDesc, size) == 0 && offsetof(PxuiTreeViewWithDesc, reserved) == 4 && offsetof(PxuiTreeViewWithDesc, label) == 8 && offsetof(PxuiTreeViewWithDesc, ptr) == 24 && offsetof(PxuiTreeViewWithDesc, open) == 40 && offsetof(PxuiTreeViewWithDesc, selected) == 56 && offsetof(PxuiTreeViewWithDesc, visible) == 64 && offsetof(PxuiTreeViewWithDesc, visible_len) == 80 && offsetof(PxuiTreeViewWithDesc, build) == 88 && offsetof(PxuiTreeViewWithDesc, user) == 96, "PxuiTreeViewWithDesc layout");
_Static_assert(sizeof(PxuiTableViewWithDesc) == 96 && offsetof(PxuiTableViewWithDesc, size) == 0 && offsetof(PxuiTableViewWithDesc, has_sort) == 4 && offsetof(PxuiTableViewWithDesc, ascending) == 5 && offsetof(PxuiTableViewWithDesc, reserved) == 6 && offsetof(PxuiTableViewWithDesc, label) == 8 && offsetof(PxuiTableViewWithDesc, titles) == 24 && offsetof(PxuiTableViewWithDesc, widths) == 40 && offsetof(PxuiTableViewWithDesc, rows) == 56 && offsetof(PxuiTableViewWithDesc, sort_column) == 64 && offsetof(PxuiTableViewWithDesc, selected) == 72 && offsetof(PxuiTableViewWithDesc, build) == 80 && offsetof(PxuiTableViewWithDesc, user) == 88, "PxuiTableViewWithDesc layout");
_Static_assert(sizeof(PxuiTextSpan) == 24 && offsetof(PxuiTextSpan, start) == 0 && offsetof(PxuiTextSpan, end) == 4 && offsetof(PxuiTextSpan, weight) == 8 && offsetof(PxuiTextSpan, flags) == 10 && offsetof(PxuiTextSpan, color) == 12 && offsetof(PxuiTextSpan, background) == 16 && offsetof(PxuiTextSpan, underline) == 20, "TextSpan layout");
_Static_assert(sizeof(PxuiTextSpans) == 200 && offsetof(PxuiTextSpans, size) == 0 && offsetof(PxuiTextSpans, count) == 4 && offsetof(PxuiTextSpans, spans) == 8, "TextSpans layout");
_Static_assert(offsetof(PxuiFrameIn, environment) == 56 && offsetof(PxuiFrameIn, size) == 0 && offsetof(PxuiFrameIn, events) == 8 && offsetof(PxuiFrameIn, event_count) == 16 && offsetof(PxuiFrameIn, width) == 24 && offsetof(PxuiFrameIn, height) == 28 && offsetof(PxuiFrameIn, scale) == 32 && offsetof(PxuiFrameIn, time) == 40 && offsetof(PxuiFrameIn, appearance) == 48 && offsetof(PxuiFrameIn, reduced_motion) == 52, "FrameInV1_8 layout");
_Static_assert(sizeof(PxuiSectionListDesc) == 72 && offsetof(PxuiSectionListDesc, size) == 0 && offsetof(PxuiSectionListDesc, overlay_scrollbar) == 4 && offsetof(PxuiSectionListDesc, reserved) == 5 && offsetof(PxuiSectionListDesc, label) == 8 && offsetof(PxuiSectionListDesc, count) == 24 && offsetof(PxuiSectionListDesc, row_height) == 32 && offsetof(PxuiSectionListDesc, leading_height) == 36 && offsetof(PxuiSectionListDesc, header_height) == 40 && offsetof(PxuiSectionListDesc, trailing_height) == 44 && offsetof(PxuiSectionListDesc, build) == 48 && offsetof(PxuiSectionListDesc, user) == 56 && offsetof(PxuiSectionListDesc, key) == 64, "PxuiSectionListDesc layout");
_Static_assert(sizeof(PxuiMeasuredSectionDesc) == 72 && offsetof(PxuiMeasuredSectionDesc, size) == 0 && offsetof(PxuiMeasuredSectionDesc, overlay_scrollbar) == 4 && offsetof(PxuiMeasuredSectionDesc, reserved) == 5 && offsetof(PxuiMeasuredSectionDesc, label) == 8 && offsetof(PxuiMeasuredSectionDesc, count) == 24 && offsetof(PxuiMeasuredSectionDesc, row_height) == 32 && offsetof(PxuiMeasuredSectionDesc, leading_height) == 36 && offsetof(PxuiMeasuredSectionDesc, header_height) == 40 && offsetof(PxuiMeasuredSectionDesc, trailing_height) == 44 && offsetof(PxuiMeasuredSectionDesc, build) == 48 && offsetof(PxuiMeasuredSectionDesc, user) == 56 && offsetof(PxuiMeasuredSectionDesc, key) == 64, "PxuiMeasuredSectionDesc layout");
_Static_assert(sizeof(PxuiEnvironment) == 72 && offsetof(PxuiEnvironment, safe_area) == 0 && offsetof(PxuiEnvironment, keyboard_frame) == 16 && offsetof(PxuiEnvironment, keyboard_from) == 32 && offsetof(PxuiEnvironment, keyboard_started) == 48 && offsetof(PxuiEnvironment, keyboard_duration) == 56 && offsetof(PxuiEnvironment, keyboard_curve) == 64 && offsetof(PxuiEnvironment, keyboard_visible) == 68, "Environment layout");
_Static_assert(offsetof(PxuiFrameIn, keyboard) == 128 && offsetof(PxuiFrameIn, size) == 0 && offsetof(PxuiFrameIn, events) == 8 && offsetof(PxuiFrameIn, event_count) == 16 && offsetof(PxuiFrameIn, width) == 24 && offsetof(PxuiFrameIn, height) == 28 && offsetof(PxuiFrameIn, scale) == 32 && offsetof(PxuiFrameIn, time) == 40 && offsetof(PxuiFrameIn, appearance) == 48 && offsetof(PxuiFrameIn, reduced_motion) == 52 && offsetof(PxuiFrameIn, environment) == 56, "FrameInV1_24 layout");
_Static_assert(sizeof(PxuiTouch) == 48 && offsetof(PxuiTouch, device) == 0 && offsetof(PxuiTouch, finger) == 8 && offsetof(PxuiTouch, x) == 16 && offsetof(PxuiTouch, y) == 24 && offsetof(PxuiTouch, time) == 32 && offsetof(PxuiTouch, pressure) == 40 && offsetof(PxuiTouch, phase) == 44, "Touch layout");
_Static_assert(sizeof(PxuiKeyboard) == 20 && offsetof(PxuiKeyboard, frame) == 0 && offsetof(PxuiKeyboard, flags) == 16, "Keyboard layout");
_Static_assert(sizeof(PxuiFrameIn) == 152 && offsetof(PxuiFrameIn, size) == 0 && offsetof(PxuiFrameIn, events) == 8 && offsetof(PxuiFrameIn, event_count) == 16 && offsetof(PxuiFrameIn, width) == 24 && offsetof(PxuiFrameIn, height) == 28 && offsetof(PxuiFrameIn, scale) == 32 && offsetof(PxuiFrameIn, time) == 40 && offsetof(PxuiFrameIn, appearance) == 48 && offsetof(PxuiFrameIn, reduced_motion) == 52 && offsetof(PxuiFrameIn, environment) == 56 && offsetof(PxuiFrameIn, keyboard) == 128, "FrameInV1_26 layout");
_Static_assert(sizeof(PxuiTextInputOptions) == 12 && offsetof(PxuiTextInputOptions, autocorrect) == 0 && offsetof(PxuiTextInputOptions, capitalization) == 4 && offsetof(PxuiTextInputOptions, action) == 8, "TextInputOptions layout");
_Static_assert(sizeof(PxuiScrubOut) == 48 && offsetof(PxuiScrubOut, size) == 0 && offsetof(PxuiScrubOut, flags) == 4 && offsetof(PxuiScrubOut, node) == 8 && offsetof(PxuiScrubOut, preview) == 40 && offsetof(PxuiScrubOut, commit) == 44, "PxuiScrubOut layout");
_Static_assert(sizeof(PxuiGestureIn) == 16 && offsetof(PxuiGestureIn, size) == 0 && offsetof(PxuiGestureIn, kinds) == 4 && offsetof(PxuiGestureIn, reveal) == 8 && offsetof(PxuiGestureIn, threshold) == 12, "PxuiGestureIn layout");
_Static_assert(sizeof(PxuiGestureOut) == 20 && offsetof(PxuiGestureOut, size) == 0 && offsetof(PxuiGestureOut, flags) == 4 && offsetof(PxuiGestureOut, swipe) == 8 && offsetof(PxuiGestureOut, back) == 12 && offsetof(PxuiGestureOut, pull) == 16, "PxuiGestureOut layout");
_Static_assert(sizeof(PxuiSwipeIn) == 16 && offsetof(PxuiSwipeIn, size) == 0 && offsetof(PxuiSwipeIn, leading) == 4 && offsetof(PxuiSwipeIn, trailing) == 8 && offsetof(PxuiSwipeIn, full) == 12, "PxuiSwipeIn layout");
_Static_assert(sizeof(PxuiDismissIn) == 12 && offsetof(PxuiDismissIn, size) == 0 && offsetof(PxuiDismissIn, fraction) == 4 && offsetof(PxuiDismissIn, velocity) == 8, "PxuiDismissIn layout");
_Static_assert(sizeof(PxuiDismissOut) == 12 && offsetof(PxuiDismissOut, size) == 0 && offsetof(PxuiDismissOut, flags) == 4 && offsetof(PxuiDismissOut, offset) == 8, "PxuiDismissOut layout");
_Static_assert(sizeof(PxuiButtonDesc) == 192 && offsetof(PxuiButtonDesc, size) == 0 && offsetof(PxuiButtonDesc, enabled) == 4 && offsetof(PxuiButtonDesc, selected) == 5 && offsetof(PxuiButtonDesc, danger) == 6 && offsetof(PxuiButtonDesc, justify) == 7 && offsetof(PxuiButtonDesc, w_kind) == 8 && offsetof(PxuiButtonDesc, h_kind) == 9 && offsetof(PxuiButtonDesc, has_name) == 10 && offsetof(PxuiButtonDesc, reserved) == 11 && offsetof(PxuiButtonDesc, w) == 12 && offsetof(PxuiButtonDesc, h) == 16 && offsetof(PxuiButtonDesc, padding) == 20 && offsetof(PxuiButtonDesc, states) == 40 && offsetof(PxuiButtonDesc, key) == 48 && offsetof(PxuiButtonDesc, text) == 64 && offsetof(PxuiButtonDesc, name) == 80 && offsetof(PxuiButtonDesc, centered) == 96 && offsetof(PxuiButtonDesc, reserved2) == 97 && offsetof(PxuiButtonDesc, has_label) == 104 && offsetof(PxuiButtonDesc, axis) == 105 && offsetof(PxuiButtonDesc, content_align) == 106 && offsetof(PxuiButtonDesc, focus_on_click) == 107 && offsetof(PxuiButtonDesc, gap) == 108 && offsetof(PxuiButtonDesc, label) == 112 && offsetof(PxuiButtonDesc, label_box) == 176, "PxuiButtonDesc layout");
_Static_assert(sizeof(PxuiLabelStyle) == 64 && offsetof(PxuiLabelStyle, size) == 0 && offsetof(PxuiLabelStyle, set) == 4 && offsetof(PxuiLabelStyle, font) == 8 && offsetof(PxuiLabelStyle, weight) == 10 && offsetof(PxuiLabelStyle, color) == 12 && offsetof(PxuiLabelStyle, max_lines) == 14 && offsetof(PxuiLabelStyle, caps) == 16 && offsetof(PxuiLabelStyle, digits) == 17 && offsetof(PxuiLabelStyle, ellipsis) == 18 && offsetof(PxuiLabelStyle, reserved) == 19 && offsetof(PxuiLabelStyle, size_points) == 20 && offsetof(PxuiLabelStyle, tracking) == 24 && offsetof(PxuiLabelStyle, line_height) == 28 && offsetof(PxuiLabelStyle, pad) == 32 && offsetof(PxuiLabelStyle, wrap) == 36 && offsetof(PxuiLabelStyle, align) == 40 && offsetof(PxuiLabelStyle, features) == 44 && offsetof(PxuiLabelStyle, width) == 48, "PxuiLabelStyle layout");
_Static_assert(sizeof(PxuiIconButtonDesc) == 104 && offsetof(PxuiIconButtonDesc, size) == 0 && offsetof(PxuiIconButtonDesc, enabled) == 4 && offsetof(PxuiIconButtonDesc, w_kind) == 5 && offsetof(PxuiIconButtonDesc, h_kind) == 6 && offsetof(PxuiIconButtonDesc, reserved) == 7 && offsetof(PxuiIconButtonDesc, w) == 8 && offsetof(PxuiIconButtonDesc, h) == 12 && offsetof(PxuiIconButtonDesc, press_scale) == 16 && offsetof(PxuiIconButtonDesc, states) == 24 && offsetof(PxuiIconButtonDesc, key) == 32 && offsetof(PxuiIconButtonDesc, name) == 48 && offsetof(PxuiIconButtonDesc, icon) == 64 && offsetof(PxuiIconButtonDesc, draw) == 80 && offsetof(PxuiIconButtonDesc, draw_user) == 88 && offsetof(PxuiIconButtonDesc, press) == 96, "PxuiIconButtonDesc layout");
_Static_assert(sizeof(PxuiResponse) == 40 && offsetof(PxuiResponse, size) == 0 && offsetof(PxuiResponse, flags) == 4 && offsetof(PxuiResponse, node) == 8, "PxuiResponse layout");
_Static_assert(sizeof(PxuiToggleDesc) == 160 && offsetof(PxuiToggleDesc, size) == 0 && offsetof(PxuiToggleDesc, enabled) == 4 && offsetof(PxuiToggleDesc, on) == 5 && offsetof(PxuiToggleDesc, press_row) == 6 && offsetof(PxuiToggleDesc, reserved) == 7 && offsetof(PxuiToggleDesc, row_h) == 8 && offsetof(PxuiToggleDesc, switch_w) == 12 && offsetof(PxuiToggleDesc, switch_h) == 16 && offsetof(PxuiToggleDesc, inset) == 20 && offsetof(PxuiToggleDesc, track) == 24 && offsetof(PxuiToggleDesc, knob) == 32 && offsetof(PxuiToggleDesc, key) == 40 && offsetof(PxuiToggleDesc, text) == 56 && offsetof(PxuiToggleDesc, detail) == 72 && offsetof(PxuiToggleDesc, has_style) == 88 && offsetof(PxuiToggleDesc, reserved2) == 89 && offsetof(PxuiToggleDesc, style) == 96, "PxuiToggleDesc layout");
_Static_assert(sizeof(PxuiSegmentedDesc) == 96 && offsetof(PxuiSegmentedDesc, size) == 0 && offsetof(PxuiSegmentedDesc, enabled) == 4 && offsetof(PxuiSegmentedDesc, grow) == 5 && offsetof(PxuiSegmentedDesc, reserved) == 6 && offsetof(PxuiSegmentedDesc, h) == 8 && offsetof(PxuiSegmentedDesc, states) == 16 && offsetof(PxuiSegmentedDesc, key) == 24 && offsetof(PxuiSegmentedDesc, name) == 40 && offsetof(PxuiSegmentedDesc, selected) == 56 && offsetof(PxuiSegmentedDesc, w) == 64 && offsetof(PxuiSegmentedDesc, padding) == 68 && offsetof(PxuiSegmentedDesc, min_segment) == 72 && offsetof(PxuiSegmentedDesc, reserved2) == 76 && offsetof(PxuiSegmentedDesc, names) == 80, "PxuiSegmentedDesc layout");
_Static_assert(sizeof(PxuiDialogDesc) == 88 && offsetof(PxuiDialogDesc, size) == 0 && offsetof(PxuiDialogDesc, width) == 4 && offsetof(PxuiDialogDesc, key) == 8 && offsetof(PxuiDialogDesc, title) == 24 && offsetof(PxuiDialogDesc, buttons) == 40 && offsetof(PxuiDialogDesc, default_at) == 56 && offsetof(PxuiDialogDesc, cancel_at) == 64 && offsetof(PxuiDialogDesc, danger) == 72 && offsetof(PxuiDialogDesc, disabled) == 80, "PxuiDialogDesc layout");
_Static_assert(sizeof(PxuiDialogResult) == 16 && offsetof(PxuiDialogResult, size) == 0 && offsetof(PxuiDialogResult, has_chosen) == 4 && offsetof(PxuiDialogResult, dismissed) == 5 && offsetof(PxuiDialogResult, reserved) == 6 && offsetof(PxuiDialogResult, chosen) == 8, "PxuiDialogResult layout");
_Static_assert(sizeof(PxuiCurve) == 140 && offsetof(PxuiCurve, kind) == 0 && offsetof(PxuiCurve, data) == 4, "PxuiCurve layout");
_Static_assert(sizeof(PxuiLabelBox) == 16 && offsetof(PxuiLabelBox, size) == 0 && offsetof(PxuiLabelBox, set) == 4 && offsetof(PxuiLabelBox, height) == 8 && offsetof(PxuiLabelBox, role) == 12 && offsetof(PxuiLabelBox, whole_pixels) == 13 && offsetof(PxuiLabelBox, reserved) == 14, "PxuiLabelBox layout");
/* abi-layout end */

#endif
