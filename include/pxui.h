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
typedef struct PxuiTheme PxuiTheme;
typedef struct { uint32_t size, platform; int32_t font_size; } PxuiUiDesc;
typedef struct { const uint8_t *ptr; size_t len; } PxuiStr;
typedef struct { const PxuiStr *ptr; size_t len; } PxuiStrSpan;
typedef struct { uint32_t tag; size_t index; } PxuiDialogOut;
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
typedef struct { uint8_t kind; union { struct { float duration,points[4]; } bezier; float omega; struct { float duration; uint32_t count; float positions[16],values[16]; } table; struct { float duration; uint32_t count; } steps; } u; } PxuiCurve; /* 0 bezier, 1 spring, 2 table, 3 steps */
typedef struct { PxuiColor color; float radius,width,gap,margin; uint8_t whole; } PxuiBar;
typedef struct { uint8_t has; uint8_t value; } PxuiOptAlign;
typedef struct { uint8_t has; PxuiGrid value; } PxuiOptGrid;
typedef struct { uint8_t has; PxuiAnchor value; } PxuiOptAnchor;
typedef struct { uint8_t has; float x,y; } PxuiOptPoint;
typedef struct { uint8_t has; PxuiBar value; } PxuiOptBar;
typedef struct { uint8_t has; PxuiCurve value; } PxuiOptCurve;
typedef struct { uint8_t has; PxuiColor value; } PxuiOptColor;
typedef struct { uint16_t family; int64_t size; uint16_t weight; float tracking; uint8_t caps; } PxuiFont;
typedef struct { PxuiColor color; float offset[2],blur,spread; uint8_t inner; } PxuiShadow;
typedef struct { PxuiColor color; float width; uint8_t sides; } PxuiRing;
typedef struct {
    PxuiColor fill; PxuiOptColor gradient; float angle; PxuiColor border; float border_w; uint8_t sides; float radius[4];
    PxuiShadow shadows[2]; PxuiColor text; PxuiFont font; float opacity; PxuiRing ring; PxuiColor selection,caret; uint8_t dashed;
} PxuiLook;
typedef struct { uint8_t hovered,pressed,focused,disabled,checked; } PxuiState;
typedef struct {
    uint32_t size; /* sizeof(PxuiNode) */
    uint8_t axis,justify,align,wrap,role; PxuiState state; int16_t z; /* the first 16 bytes are tested as two words */
    int64_t flags; PxuiSize w,h; float min_w,min_h,max_w,max_h,grow,shrink,aspect; PxuiEdges padding,margin;
    float gap,row_gap; PxuiOptGrid grid; PxuiCell cell; PxuiOptAlign align_self;
    PxuiOptAnchor anchor; uint32_t a11y; PxuiOptPoint at; float scroll_step; PxuiLook look; PxuiEdges inset;
    PxuiMark mark; PxuiOptBar bar; PxuiOptCurve transition;
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
_Static_assert(sizeof(PxuiSize) == 16 && sizeof(PxuiGrid) == 32 && sizeof(PxuiOptGrid) == 40, "PxuiSize/PxuiGrid layout");
_Static_assert(sizeof(PxuiLook) == 160 && offsetof(PxuiLook, dashed) == 152, "PxuiLook layout");
_Static_assert(sizeof(PxuiCurve) == 140 && sizeof(PxuiOptCurve) == 144 && sizeof(PxuiCurveDesc) == 160 && offsetof(PxuiCurve, u.table.positions) == 12 && offsetof(PxuiCurveDesc, values) == 96, "PxuiCurve layout");
_Static_assert(sizeof(PxuiNode) == 600 && offsetof(PxuiNode, flags) == 16 && offsetof(PxuiNode, look) == 240 && offsetof(PxuiNode, transition) == 456, "PxuiNode layout");
_Static_assert(sizeof(PxuiTokens) == 136 && sizeof(PxuiThemeSnapshot) == 56888, "PxuiThemeSnapshot layout");
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
typedef struct { uint32_t size; uint16_t font,weight; float size_points,line_height; uint32_t wrap,align; uint16_t max_lines; uint8_t italic,ellipsis; uint32_t features; float tracking; } PxuiTextStyle;
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
typedef struct {
    uint32_t size;
    const PxuiEvent *events;
    size_t event_count;
    float width, height, scale;
    double time;
    /* ABI 1.8 tail; legacy size=48 defaults both to zero. Partial tails are refused. */
    uint32_t appearance, reduced_motion;
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
    PXUI_FILE_DROPPED=11, PXUI_FILE_DRAG_MOVED=12, PXUI_FILE_DRAG_ENDED=13,
    PXUI_PINCHED=14, PXUI_FILE_DIALOG=15, PXUI_DIALOG_OPENED=16, PXUI_RESIZED=17,
    PXUI_FOCUS_LOST=18, PXUI_FOCUS_GAINED=19, PXUI_CLOSED=20,
    PXUI_PIXEL_SIZE_CHANGED=21, PXUI_VISIBILITY_CHANGED=22, PXUI_TRACKPAD_SCROLLED=23
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
typedef struct { uint16_t depth; uint8_t children; } PxuiTreeNode;
typedef struct { uint32_t size; uint8_t has_stroke,has_max_height,dismiss_initial,attached; PxuiColor stroke; float max_height,x,y,width,height; PxuiStr label; } PxuiPopupDesc;
typedef struct { uint32_t size; uint8_t active,usable,reserved[2]; PxuiStr caption,keycap; } PxuiHeadActionDesc;
typedef struct { const PxuiHeadActionDesc *ptr; size_t len; } PxuiHeadActionSpan;
typedef struct { uint32_t size; uint8_t has_kind,has_badge,has_index,has_reset,reset_enabled,reserved[3]; PxuiStr key,kind,badge,index,reset,title,detail; PxuiHeadActionSpan actions; } PxuiHeaderDesc;
typedef struct { uint32_t size; uint8_t has_chosen,reset_pressed,reserved[2]; PxuiStr renamed; int64_t chosen; } PxuiHeadOut;
typedef struct { float rect[4]; int16_t clip[4]; uint8_t color[4], color2[4]; uint32_t aux[2]; uint16_t radii[4]; uint16_t width; uint8_t kind, flags; uint16_t view, clip_radius; uint32_t extra[2]; } PxuiInstance; /* 64 bytes; layouts are frozen from the first release, which has not happened yet */
/* One layer of the last frame (a floating root's, in draw order): its root box key, retained id, instances (an empty layer draws nothing),
   texture break count and the instance ranges changed since it was last drawn. Borrowed until the next frame. */
typedef struct { uint32_t start, end; } PxuiDirty;
typedef struct { const PxuiDirty *ptr; size_t len; } PxuiDirtySpan;
typedef struct { const PxuiInstance *ptr; size_t len; } PxuiInstanceSpan;
typedef struct { int64_t key; uint32_t id, breaks; PxuiInstanceSpan instances; PxuiDirtySpan dirty; } PxuiLayerInfo;
typedef struct { const PxuiLayerInfo *ptr; size_t len; } PxuiLayerSpan;
typedef struct { uint32_t size,reserved; uint64_t draw_revision; double repaint_at; uint64_t built_nodes,rebuilt_views,arranged_nodes; PxuiPlatformOut platform; PxuiDamage damage; PxuiA11yTree a11y; PxuiLayerSpan layers; } PxuiOutput;
/* A window into a layer's instance storage: the caller stores 64-byte instances at at (stamping clip, clip_radius_f16 and view) and
   advances it; pxui_paint_span commits what was written (at is where it stopped) and hands out the next window. */
typedef struct { uint32_t size; uint16_t view,clip_radius_f16; int16_t clip[4]; PxuiInstance *at,*end; } PxuiSpan;
typedef struct { uint32_t size; uint8_t rtl,reserved[3]; int64_t font_size,row_height,density; PxuiRect clip; float clip_radius; PxuiThemeTokens theme; } PxuiPaintInfo;
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
int32_t pxui_ui_table(PxuiUi *ui, const PxuiTableDesc *desc, int64_t *clicked_row);
/* Last host appearance; themes remain authored by the application. use_theme selects a parsed root
   theme between frames, avoiding reparsing source when appearance changes. */
int32_t pxui_ui_system_appearance(PxuiUi *ui, uint32_t *out);
int32_t pxui_ui_use_theme(PxuiUi *ui, const PxuiTheme *theme);
int32_t pxui_ui_set_theme(PxuiUi *ui, const PxuiThemeSource *source);
int32_t pxui_ui_set_dir(PxuiUi *ui, uint32_t dir);
int32_t pxui_ui_set_a11y(PxuiUi *ui, uint8_t on);
int32_t pxui_ui_inject_a11y(PxuiUi *ui, const PxuiA11yAction *actions, size_t count);
int32_t pxui_ui_a11y_name(PxuiUi *ui, const PxuiBox *box, PxuiStr name);
int32_t pxui_ui_a11y_number(PxuiUi *ui, const PxuiBox *box, double now, double min, double max, double step);
int32_t pxui_ui_set_role(PxuiUi *ui, const PxuiBox *box, uint32_t role, uint32_t states);
int32_t pxui_ui_enter(PxuiUi *ui, const PxuiBox *box, PxuiSaved *out);
int32_t pxui_ui_leave(PxuiUi *ui, const PxuiSaved *saved);
int32_t pxui_ui_focus_box(PxuiUi *ui, const PxuiBox *box);
int32_t pxui_ui_focused(PxuiUi *ui, const PxuiBox *box, uint8_t *out);
int32_t pxui_ui_state(PxuiUi *ui, const PxuiBox *box, int64_t default_value, int64_t *out);
int32_t pxui_ui_set_state(PxuiUi *ui, const PxuiBox *box, int64_t value);
int32_t pxui_ui_signal(PxuiUi *ui, const PxuiBox *box, PxuiSignal *out);
int32_t pxui_ui_patch_text(PxuiUi *ui, const PxuiBox *box, PxuiStr value, uint8_t *out); /* out: whether the text was written; a stale box is PXUI_ERR_STALE */
int32_t pxui_ui_busy_step(PxuiUi *ui, const PxuiBox *box, double now, uint8_t *out);
int32_t pxui_ui_request_frame(PxuiUi *ui);
int32_t pxui_ui_node(PxuiUi *ui, PxuiStr label, const PxuiNode *node, PxuiBox *out); /* node->size = sizeof(PxuiNode); copied in the call */
int32_t pxui_ui_node_keyed(PxuiUi *ui, int64_t key, const PxuiNode *node, PxuiBox *out); /* keys 0 and 1 are reserved */
int32_t pxui_ui_rect(PxuiUi *ui, const PxuiBox *box, PxuiRect *out);
uint8_t pxui_ui_box_of_key(PxuiUi *ui, int64_t key, PxuiBox *out);
int32_t pxui_ui_text(PxuiUi *ui, const PxuiBox *box, PxuiStr value);
int32_t pxui_ui_set_look(PxuiUi *ui, const PxuiBox *box, const PxuiLookDesc *look, const PxuiStateDesc *state);
int32_t pxui_ui_style(PxuiUi *ui, const PxuiBox *box, const PxuiLookDesc states[6], const PxuiStateDesc *state);
float pxui_ui_text_width(PxuiUi *ui, uint8_t has_size, int64_t size, PxuiStr text);
int32_t pxui_ui_draw(PxuiUi *ui, const PxuiBox *box, uint8_t over, PxuiPaintBuild build, void *user, PxuiPaintRelease release); /* a span must be committed (pxui_paint_span with at_least 0) before the callback returns; one left open, or a pxui_paint_* call made under an open span, is refused and the frame that ran the callback returns PXUI_ERR_STATE */
int32_t pxui_within(PxuiUi *ui, const PxuiBox *box, PxuiBuild build, void *user);
int32_t pxui_scope(PxuiUi *ui, PxuiStr label, PxuiBuild build, void *user);
int32_t pxui_col(PxuiUi *ui, float padding, PxuiStr label, PxuiBuild build, void *user);
int32_t pxui_panel(PxuiUi *ui, const PxuiPanel *panel, PxuiStr label, PxuiBuild build, void *user);
int32_t pxui_label(PxuiUi *ui, PxuiStr text);
int32_t pxui_button(PxuiUi *ui, PxuiStr label, uint8_t *out);
int32_t pxui_toggle(PxuiUi *ui, PxuiStr label, uint8_t on, uint8_t *out);
int32_t pxui_text_field(PxuiUi *ui, PxuiStr label, PxuiStr value, PxuiStr *changed, uint8_t *out_changed); /* changed: the new text, borrowed until the next frame, when out_changed */
int32_t pxui_ui_checkbox(PxuiUi *ui, PxuiStr label, uint8_t value, uint8_t *out);
int32_t pxui_ui_radio(PxuiUi *ui, PxuiStr label, PxuiStrSpan options, size_t selected, size_t *out);
int32_t pxui_ui_tabs(PxuiUi *ui, PxuiStr label, PxuiStrSpan options, size_t selected, size_t *out);
int32_t pxui_ui_segmented(PxuiUi *ui, PxuiStr label, PxuiStrSpan options, size_t selected, size_t *out);
int32_t pxui_ui_dropdown(PxuiUi *ui, PxuiStr label, PxuiStrSpan options, size_t selected, size_t *out);
int32_t pxui_ui_slider(PxuiUi *ui, PxuiStr label, float value, float min, float max, float *out);
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
typedef struct { uint32_t size, reserved; int64_t lead_from; PxuiStrSpan keys; PxuiSwatchSpan swatches; PxuiI64Span danger; PxuiSubmenuSpan submenus; } PxuiMenuExtra;
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
typedef struct { uint32_t size; PxuiStateDesc state; uint8_t on; PxuiBox row; PxuiStr label; } PxuiToggleConfig;
_Static_assert(sizeof(PxuiToggleConfig) == 64, "PxuiToggleConfig layout");
typedef struct { uint32_t size; PxuiStateDesc state; float fraction; PxuiBox track; PxuiStr label; } PxuiSliderConfig;
_Static_assert(sizeof(PxuiSliderConfig) == 64, "PxuiSliderConfig layout");
typedef struct { uint32_t size; uint8_t has_value; float value; PxuiBox row; PxuiStr label; } PxuiProgressConfig;
_Static_assert(sizeof(PxuiProgressConfig) == 64, "PxuiProgressConfig layout");
typedef struct { uint32_t size; PxuiStateDesc state; uint8_t bare; uint8_t icon; uint8_t at_end; PxuiBox line; PxuiBox row; const PxuiStates * states; PxuiStr label; } PxuiButtonConfig;
_Static_assert(sizeof(PxuiButtonConfig) == 104, "PxuiButtonConfig layout");
typedef struct { uint32_t size; PxuiStateDesc state; PxuiBox node; PxuiStr label; PxuiStr icon; } PxuiIconButtonConfig;
_Static_assert(sizeof(PxuiIconButtonConfig) == 80, "PxuiIconButtonConfig layout");
typedef struct { uint32_t size; PxuiStateDesc state; uint8_t on; PxuiBox row; PxuiStr label; } PxuiCheckConfig;
_Static_assert(sizeof(PxuiCheckConfig) == 64, "PxuiCheckConfig layout");
typedef struct { uint32_t size; PxuiBox node; PxuiStr label; } PxuiSpinnerConfig;
_Static_assert(sizeof(PxuiSpinnerConfig) == 56, "PxuiSpinnerConfig layout");
typedef struct { uint32_t size; PxuiBox node; PxuiStr label; PxuiStr text; } PxuiBadgeConfig;
_Static_assert(sizeof(PxuiBadgeConfig) == 72, "PxuiBadgeConfig layout");
typedef struct { uint32_t size; uint8_t first; PxuiBox node; PxuiStr text; } PxuiLabelConfig;
_Static_assert(sizeof(PxuiLabelConfig) == 56, "PxuiLabelConfig layout");
typedef struct { uint32_t size; PxuiBox row; uint8_t has_right; PxuiStr right; PxuiHintSpan hints; } PxuiFooterConfig;
_Static_assert(sizeof(PxuiFooterConfig) == 80, "PxuiFooterConfig layout");
typedef struct { uint32_t size; PxuiStateDesc state; uint8_t vertical; uint8_t floating; PxuiBox grip; } PxuiGripConfig;
_Static_assert(sizeof(PxuiGripConfig) == 48, "PxuiGripConfig layout");
typedef struct { uint32_t size; PxuiBox node; PxuiStr label; PxuiSlot content; } PxuiPanelConfig;
_Static_assert(sizeof(PxuiPanelConfig) == 64, "PxuiPanelConfig layout");
typedef struct { uint32_t size; PxuiStr title; PxuiStrSpan buttons; PxuiSlot content; } PxuiDialogConfig;
_Static_assert(sizeof(PxuiDialogConfig) == 48, "PxuiDialogConfig layout");
typedef struct { uint32_t size; uint8_t open; float gap; PxuiBox title; PxuiStr label; PxuiSlot content; } PxuiSectionConfig;
_Static_assert(sizeof(PxuiSectionConfig) == 72, "PxuiSectionConfig layout");
typedef struct { uint32_t size; PxuiStateDesc state; uint8_t has_pin; uint8_t pin; float label_w; PxuiBox row; PxuiStr label; } PxuiInspectorRowConfig;
_Static_assert(sizeof(PxuiInspectorRowConfig) == 72, "PxuiInspectorRowConfig layout");
typedef struct { uint32_t size; uint32_t axis; float ratio; float min_a; float min_b; PxuiBox split; PxuiSlot first; PxuiSlot second; } PxuiSplitConfig;
_Static_assert(sizeof(PxuiSplitConfig) == 72, "PxuiSplitConfig layout");
typedef struct { uint32_t size; PxuiBox node; PxuiSlot content; } PxuiScrollConfig;
_Static_assert(sizeof(PxuiScrollConfig) == 48, "PxuiScrollConfig layout");
typedef struct { uint32_t size; PxuiStateDesc state; PxuiBox node; PxuiStr text; } PxuiToastConfig;
_Static_assert(sizeof(PxuiToastConfig) == 64, "PxuiToastConfig layout");
typedef struct { uint32_t size; float width; float height; PxuiBox tip; PxuiStrSpan lines; } PxuiTipConfig;
_Static_assert(sizeof(PxuiTipConfig) == 64, "PxuiTipConfig layout");
typedef struct { uint32_t size; PxuiStateDesc state; size_t index; PxuiBox node; PxuiStr text; } PxuiTabConfig;
_Static_assert(sizeof(PxuiTabConfig) == 72, "PxuiTabConfig layout");
typedef struct { uint32_t size; PxuiStateDesc state; size_t index; size_t selected; int64_t group; PxuiBox node; PxuiStr text; } PxuiSegmentConfig;
_Static_assert(sizeof(PxuiSegmentConfig) == 88, "PxuiSegmentConfig layout");
typedef struct { uint32_t size; PxuiStateDesc state; uint8_t open; int64_t key; PxuiBox button; PxuiStr text; } PxuiDropdownConfig;
_Static_assert(sizeof(PxuiDropdownConfig) == 72, "PxuiDropdownConfig layout");
typedef struct { uint32_t size; PxuiStateDesc state; size_t index; PxuiBox node; PxuiStr text; } PxuiMenuItemConfig;
_Static_assert(sizeof(PxuiMenuItemConfig) == 72, "PxuiMenuItemConfig layout");
typedef struct { uint32_t size; PxuiStateDesc state; uint8_t selected; uint8_t chevron; uint8_t danger; uint8_t has_swatch; PxuiColor swatch; uint8_t has_shortcut; int64_t index; float lead; float swatch_pad; PxuiBox row; PxuiStr text; PxuiStr shortcut; } PxuiMenuRowConfig;
_Static_assert(sizeof(PxuiMenuRowConfig) == 104, "PxuiMenuRowConfig layout");
typedef struct { uint32_t size; PxuiStateDesc state; size_t index; PxuiBox row; PxuiSlot content; } PxuiRowConfig;
_Static_assert(sizeof(PxuiRowConfig) == 64, "PxuiRowConfig layout");
typedef struct { uint32_t size; PxuiStateDesc state; uint8_t has; uint8_t open; size_t index; size_t node; size_t depth; PxuiBox row; PxuiSlot content; } PxuiTreeRowConfig;
_Static_assert(sizeof(PxuiTreeRowConfig) == 80, "PxuiTreeRowConfig layout");
typedef struct { uint32_t size; PxuiStateDesc state; uint8_t has_ascending; uint8_t ascending; size_t index; PxuiBox node; PxuiStr title; } PxuiTableHeadConfig;
_Static_assert(sizeof(PxuiTableHeadConfig) == 72, "PxuiTableHeadConfig layout");
typedef struct { uint32_t size; uint8_t header; float left; float right; float height; int64_t row; int64_t column; int64_t key; PxuiStr text; } PxuiTableCellConfig;
_Static_assert(sizeof(PxuiTableCellConfig) == 64, "PxuiTableCellConfig layout");
typedef struct { uint32_t size; PxuiStateDesc state; uint8_t current; uint8_t doomed; uint8_t unavailable; uint8_t marked; uint8_t has_swatch; PxuiColor swatch; int64_t index; PxuiBox row; PxuiStr label; PxuiStr detail; PxuiStr query; } PxuiPickerRowConfig;
_Static_assert(sizeof(PxuiPickerRowConfig) == 112, "PxuiPickerRowConfig layout");
typedef struct { uint32_t size; PxuiStateDesc state; int64_t key; PxuiBox well; PxuiStr icon; PxuiSlot editor; } PxuiSearchFieldConfig;
_Static_assert(sizeof(PxuiSearchFieldConfig) == 80, "PxuiSearchFieldConfig layout");
typedef struct { uint32_t size; PxuiStateDesc state; PxuiBox row; PxuiStr label; PxuiStr value; } PxuiTextFieldConfig;
_Static_assert(sizeof(PxuiTextFieldConfig) == 80, "PxuiTextFieldConfig layout");
typedef struct { uint32_t size; PxuiStateDesc state; uint8_t empty; PxuiBox field; PxuiStr text; } PxuiLineFieldConfig;
_Static_assert(sizeof(PxuiLineFieldConfig) == 64, "PxuiLineFieldConfig layout");
typedef struct { uint32_t size; PxuiStateDesc state; uint8_t invalid; float w; float h; PxuiBox field; PxuiValueFieldDesc opts; PxuiStr text; } PxuiValueFieldConfig;
_Static_assert(sizeof(PxuiValueFieldConfig) == 224, "PxuiValueFieldConfig layout");
typedef struct { uint32_t size; PxuiStateDesc state; uint8_t empty; uint8_t slash; float prefix; float inset; PxuiBox search; PxuiStr placeholder; } PxuiPickerFieldConfig;
_Static_assert(sizeof(PxuiPickerFieldConfig) == 72, "PxuiPickerFieldConfig layout");
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
/* A text shaper of its own, for text laid out off the Ui on any thread: its own font system, no GPU atlas. */
/* A shaper is a plain library object with no thread-local state: it may be created on one thread and used on another, but one thread
   uses one shaper at a time (calls on it never overlap). It shares nothing with a PxuiUi's text service. */
PxuiShaper *pxui_shaper_create(const PxuiStr *fonts, size_t count);
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
uint64_t pxui_renderer_draw_calls(PxuiRenderer *renderer);
int32_t pxui_renderer_atlas(PxuiRenderer *renderer, uint32_t kind, PxuiImage *out); /* PXUI_ATLAS_*: mask R8, colour RGBA8; second pages return ERR_STATE until prepared. The texture stays the renderer's. */
void pxui_renderer_destroy(PxuiRenderer *renderer);

/* Queries and switches on the Ui as a whole. Text results are borrowed until the next frame. */
int64_t pxui_ui_font_size(PxuiUi *ui);
int64_t pxui_ui_row_height(PxuiUi *ui);
uint64_t pxui_ui_frame_index(PxuiUi *ui);
uint8_t pxui_ui_captured(PxuiUi *ui);
uint8_t pxui_ui_text_input_focused(PxuiUi *ui);
uint8_t pxui_ui_verifying(PxuiUi *ui);
uint64_t pxui_ui_text_undo_bytes(PxuiUi *ui);
float pxui_ui_inspector_width(PxuiUi *ui);
float pxui_ui_inspector_label_x(PxuiUi *ui);
uint32_t pxui_ui_passed_undo(PxuiUi *ui); /* 0 none, 1 undo, 2 redo */
int32_t pxui_ui_key_pressed(PxuiUi *ui, uint32_t code, uint8_t *out);
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
int32_t pxui_ui_text_state(PxuiUi *ui, const PxuiBox *box, uint8_t *present, PxuiStr *out);
int32_t pxui_ui_set_text_state(PxuiUi *ui, const PxuiBox *box, uint8_t present, PxuiStr value);
int32_t pxui_ui_cached_label(PxuiUi *ui, const PxuiBox *box, uint64_t index, PxuiStr text, PxuiStr *out); /* borrowed from the slot */
int32_t pxui_ui_to_front(PxuiUi *ui, int64_t order, const PxuiBox *box);
int32_t pxui_ui_carrying(PxuiUi *ui, uint8_t *present, PxuiStr *kind, PxuiStr *value);
int32_t pxui_ui_carry(PxuiUi *ui, uint8_t has_from, const PxuiBox *from, PxuiStr kind, PxuiStr value);
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
int32_t pxui_paint_text_spans(PxuiPaint *p, float x, float y, const PxuiTextStyle *style, float width, uint8_t has_color, PxuiColor color, PxuiStr text, const PxuiTextSpans *spans, PxuiTextMetrics *out);

/* Apple native fallback, cold discovery only. Ui must enable before its first frame.
   Explicit-font constructors remain deterministic. No provider: PXUI_ERR_STATE. */
int32_t pxui_shaper_system_fonts(PxuiShaper *shaper);
int32_t pxui_text_system_fonts(PxuiUi *ui);

#endif
