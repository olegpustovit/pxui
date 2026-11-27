#ifndef PXUI_H
#define PXUI_H

/* The stable ABI (abi/pxui.v1.txt and abi/layout.v1.txt) is frozen at 2.13 (docs/abi.md). Later minors can add functions and records.
   Frozen record layouts are immutable. An app states the version it was built for (PxuiUiDesc.abi_built, PXUI_ABI_BUILT); the library refuses another major, or a
   newer minor than its own, at pxui_ui_create (PXUI_ERR_VERSION). Every call that can fail returns int32_t, 0 on success; the reason is
   pxui_last_error(ui). */

#include <stddef.h>
#include <stdint.h>

typedef struct PxuiUi PxuiUi;
typedef struct PxuiRenderer PxuiRenderer;
typedef struct PxuiPaint PxuiPaint;
typedef struct PxuiShaper PxuiShaper;
typedef struct PxuiPreparedText PxuiPreparedText;
typedef struct PxuiTextWorker PxuiTextWorker;
/* Error model. Every fallible call returns int32_t: 0 on success, else a PXUI_ERR_* status (below); the reason text is pxui_last_error(ui), or
   pxui_theme_last_error(theme) for a theme. A refused call writes none of its outputs and changes nothing. A call that returns a value instead of
   a status refuses by one documented failure value: NULL (the create calls, with the reason in *status for pxui_ui_create; a null Ui for
   pxui_last_error), UINT32_MAX (the pxui_*_add_font calls), SIZE_MAX (the label quads and rasterize calls), 0 or an empty string (a query on a
   handle that is not valid). pxui_paint_label and pxui_paint_label_chars return their measurement by value and return zeros for a refused input; pxui_paint_ratio and
   pxui_paint_input_region are void and draw nothing for one.
   shortcut: shapers, accessibility bridges and prepared text carry no error text, only a status or a failure value; give each object its own
   last_error when a host needs the reason.
   Threads and lifetimes. A PxuiUi, the renderers made from it (pxui_renderer_*), its paint handles and the handles a callback borrows belong to the
   thread that created the Ui; different Uis may live on different threads. shortcut: a call from another thread is undefined, not checked; add a
   creating-thread check (PXUI_ERR_STATE) when a host runs surfaces on threads. A PxuiShaper, PxuiPreparedText and PxuiTextWorker are used by one
   thread at a time and may be moved between threads. pxui_ui_destroy with renderers still attached keeps the Ui until the last renderer is
   destroyed (any drop order is safe). Every pointer an argument carries is borrowed for the call (strings, records, spans, callbacks and their
   `user`), and the library keeps none of them past it, except where a call below says otherwise: a PxuiIconDraw and its draw_user are called at
   paint time, so draw_user must stay valid until the next pxui_frame returns; a function pointer in PxuiSizeDesc/PxuiSize (`relative`) is kept in
   retained nodes and called in later layouts, so its code must stay loaded while any node holds it. Strings the library hands back are valid until
   the next frame call.
   Raw handles (D16: accepted). The worker, shaper, prepared-text, accessibility and renderer handles are not validated: the pointer must come
   from its create call and must not have been destroyed. A foreign or dangling pointer is undefined behaviour that affects only the calling
   application's process. The Ui handle and boxes are validated (a box carries the Ui serial: PXUI_ERR_STALE).
   Body names. A body name (pxui_body_register) belongs to its Ui: at most 64 bytes, at most 65,535 names a kind per Ui, freed with the Ui.
   GPU devices (OS-6, decision D24): a renderer creates its own device and queue; sharing one device across several windows is a host
   concern (pxui_host) outside this ABI, deferred past 2.0 with its own performance qualification. */
/* Conventions of the encodings, each tested in pxui_abi (safety.rs `encoding_rules`):
   - none: an index in a record or a result is below the item count, or SIZE_MAX for none. An index at or past the count is none as well (a selection kept past a
     shrink is not an error); no other value (0, -1) means none.
   - text key: the `key` of a text call is 0 or a layout key. 0 derives the key from the style, width, text and spans; a nonzero key names the caller's layout and a
     call whose content differs lays it out again, so a key is a cache name, never an identity.
   - zero: a record with a SET mask (PxuiLabelStyle) reads a field only when its bit is set (a clear bit is the theme's value and the field is ignored, whatever it holds;
     a set bit's zero is a value, subject to the field's own validation); a record without one reads a documented 0 as the default (line_height 0 is 1.2 times the size,
     max_lines 0 no limit, font_size 0 the default). A record never mixes the two.
   - repaint_at (PxuiOutput): seconds on the frame's time base at which the Ui next needs a frame; +INFINITY (HUGE_VAL) means none, an idle Ui draws no frame; never NaN,
     and a value not after the frame's time means now. */
typedef struct PxuiTheme PxuiTheme;
/* abi_built: the version the app was built against, PXUI_ABI_BUILT (major << 16 | minor); pxui_ui_create refuses 0, another major and a newer minor (PXUI_ERR_VERSION). */
typedef struct { uint32_t size, platform; int32_t font_size; uint32_t abi_built; } PxuiUiDesc;
#define PXUI_ABI_BUILT ((2u << 16) | 15u)
typedef struct { const uint8_t *ptr; size_t len; } PxuiStr;
/* Autocorrect: 0/1; capitalization: none/sentences/words/letters (0..3); action: Return/Done/Search (0..2). */
/* size-prefixed (16 bytes); pxui_ui_text_input_options needs the caller's size to cover it. */
typedef struct { uint32_t size, autocorrect, capitalization, action; } PxuiTextInputOptions;
/* a text field: key, accessible name (empty: none) and value, with the traits as PxuiTextInputOptions numbers as bytes. */
typedef struct { uint32_t size; uint8_t autocorrect, capitalization, action, reserved; PxuiStr key, name, value; uint16_t variant; uint8_t reserved_variant[6]; } PxuiTextFieldDesc;
typedef struct { const PxuiStr *ptr; size_t len; } PxuiStrSpan;
/* TOUCH borrows this numeric payload through Event.text; len is sizeof(PxuiTouch).
   IDs retain all 64 bits. Phase: 0 began, 1 moved, 2 ended, 3 cancelled. Positions are logical points
   and may be outside the window. Time is source monotonic seconds; finite pressure is in [0,1]. */
/* size-prefixed (56 bytes): size is sizeof(PxuiTouch) and covers the record without running past text.len; a longer payload is read as its first 56 bytes. */
typedef struct { uint32_t size, phase; uint64_t device, finger; double x, y, time; float pressure; uint32_t reserved; } PxuiTouch;
/* one input event, 64 bytes, size-prefixed (size >= 64: a longer record is read as its first 64 bytes, a shorter one refused). One kind
   per event; the twin FILE_DROPPED (11) is gone and stays reserved (FILES_DROPPED carries any number of paths). `surface` is the surface the event is
   for: it must be the surface of the frame call that carries it, else the frame is PXUI_ERR_ARGUMENT. Per-kind fields: pointer and scroll use x/y;
   TEXT_EDITING carries its start (x) and length (y) in text bytes, both not negative; FILE_DIALOG carries its id in `value`
   (shortcut: an f64, exact up to 2^53; upgrade with an integer id slot when a host numbers dialogs past that).
   `text` and a touch payload are borrowed for the frame call (pxui_frame, pxui_ui_wants_frame) only: the library copies what it
   keeps before the call returns. Events the library hands back (PxuiSignal.keys, pxui_ui_input) are borrowed from the Ui until its next frame. */
typedef struct { uint32_t size, kind; uint64_t surface; uint32_t code, reserved; double x, y, value; PxuiStr text; } PxuiEvent;
/* an array of records crosses as {ptr, len, stride}: record i starts at ptr + i * stride bytes. The writer's stride is its record size; a reader steps
   by it and reads the prefix it knows (each record's own size says how much it holds). A stride below the record size (or not a multiple of its alignment),
   or a null ptr with len > 0, is refused (PXUI_ERR_ARGUMENT). The library writes dense spans (stride = sizeof the record). */
typedef struct { const PxuiEvent *ptr; size_t len; size_t stride; } PxuiEventSpan;
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
/* ABI 2.14. Authoritative held state after the ordered events in PxuiFrameIn. */
typedef struct {
    uint32_t size;
    PxuiU32Span keys, buttons;
    double pointer[2], mouse_delta[2], pixel_scale[2], dt;
    int64_t frame, tick;
    double fps;
} PxuiInputSnapshot;
typedef struct { float x,y,w,h; } PxuiRect;
typedef struct { float x,y; } PxuiPoint;
typedef struct { uint8_t r,g,b,a; } PxuiColor;
/* Colour tokens: a colour with a 0 and r 0x54 names colour token b of the theme's palette (pxui_theme_set_color) drawn at opacity g: every
   value of g is its opacity (255 opaque; 0x4b is no longer read as 255). Token 64 is the theme's accent, 65 and past are transparent. The library
   resolves it for the theme in effect where a look or swatch is applied (never on an interpolated look); a body receives the caller's looks as given (pxui_ui_style resolves them), the theme's resolved.
   A paint call given a raw token paints nothing: resolve it with pxui_ui_color / pxui_paint_color. */
typedef struct { float fast,normal,slow,ease[4]; } PxuiMotion;
typedef struct { uint32_t size; PxuiColor panel,foreground,control,input,track,accent,ink_ground; uint8_t has_ink_ground,reserved[3]; float space[4],radius[3]; int64_t display_size,title_size,font_size,label_size; PxuiMotion motion; uint8_t reduced_motion,dark /* 0 or 1, the theme is dark (picks the dark colour tokens); stated, never guessed from panel */,reserved_tail[2]; float density; } PxuiThemeTokens;
typedef struct { uint32_t size,cursor; uint8_t has_cursor,has_ime,has_background,has_clipboard; int64_t ime_rect[4],ime_caret; PxuiStr clipboard; PxuiColor background; uint8_t paste,reserved[7]; } PxuiPlatformOut;
typedef struct { uint8_t has_damage,reserved[3]; PxuiRect rect; } PxuiDamage;
/* Entries are [x, y, width, height] in window points. A full list reports the viewport. */
typedef struct { uint8_t count,full,reserved[6]; float rects[8][4]; } PxuiDamageList;
/* Promoted vector stops use logical window points and seconds in the UI's clock. */
typedef struct { float time,rect[4],opacity,curve[4]; } PxuiPromotedKey;
typedef struct {
    uint32_t layer,instance;
    uint16_t view;
    uint8_t count,autoreverse;
    uint16_t hz,reserved;
    uint64_t content_revision;
    double start;
    float offset;
    uint8_t color[4];
    float swept[4],clip[4];
    PxuiPromotedKey keys[9];
} PxuiPromoted;
typedef struct { uint32_t tag,text_start,text_end,reserved; double now,min,max,step; } PxuiA11yValue;
/* size-prefixed (136 bytes): size is the writer's record size; a reader of a span steps by its stride. */
typedef struct { uint32_t size,parent; uint64_t id; uint32_t role,name_start,name_end,description_start,description_end; PxuiA11yValue value; PxuiRect rect; uint32_t states,actions,selection[2],child_count,first_child,next_sibling,last_child; } PxuiA11yNode;
/* size-prefixed (40 bytes); pxui_ui_inject_a11y refuses a short action or a surface that does not match its Ui. */
typedef struct { uint32_t size,action_code; uint64_t id; double payload; uint32_t end,reserved; uint64_t surface; } PxuiA11yAction;
typedef struct { const PxuiA11yNode *ptr; size_t len; size_t stride; } PxuiA11yNodeSpan;
typedef struct { const PxuiA11yAction *ptr; size_t len; size_t stride; } PxuiA11yActionSpan;
/* size-prefixed (88 bytes): pxui_a11y_update refuses a record whose size is short or a surface 0. nodes.stride is the tree's node stride (the writer's PxuiA11yNode size). */
typedef struct { uint32_t size, reserved; PxuiA11yNodeSpan nodes; PxuiStr strings; uint64_t revision; uint32_t first_child,child_count; PxuiA11yActionSpan actions; } PxuiA11yTree;
typedef struct { float l,t,r,b; } PxuiEdges;
typedef struct { uint32_t kind; float value; float (*relative)(float); } PxuiSizeDesc;
/* size-prefixed (40 bytes); a short size is refused. */
typedef struct { uint32_t size; uint8_t has_anchor,flip,reserved[2]; PxuiRect rect; uint32_t side,align; float gap,margin; } PxuiAnchorDesc;
/* PxuiAnchorDesc.align and PxuiPlacement.align: PXUI_ALIGN_START, _CENTER or _END (0 to 2). PXUI_ALIGN_STRETCH and _BASELINE (3, 4) are node
   alignments and are refused here (and in PxuiNode's anchor placement, PXUI_ERR_ARGUMENT). */
typedef struct { uint16_t family,weight; float size_points; /* read only while within half a point of size */ int64_t size; float tracking; uint8_t caps,reserved[3]; } PxuiFontDesc;
typedef struct { PxuiColor color; float offset[2],blur,spread; uint8_t inner,reserved[3]; } PxuiShadowDesc;
typedef struct { PxuiColor color; float width; uint32_t sides; } PxuiRingDesc;
/* size-prefixed (160 bytes): read through a pointer, a short size is refused before a byte past it is read; an array of looks (`states`, `track`, `knob`,
   `look`: six) is spaced by its first element's size and every element carries that size. */
typedef struct { uint32_t size; PxuiColor fill,gradient; uint8_t has_gradient,reserved[3]; float angle; PxuiColor border; float border_w; uint32_t sides; float radius[4]; PxuiShadowDesc shadows[2]; PxuiColor text; PxuiFontDesc font; float opacity; PxuiRingDesc ring; PxuiColor selection,caret; uint8_t dashed,reserved_tail[3]; } PxuiLookDesc;
typedef struct { uint32_t kind,direction; float width; uint8_t rising,reserved[3]; } PxuiMarkDesc;
/* size-prefixed (12 bytes), embedded in the body configs (the library writes it) and read by pxui_ui_style. */
typedef struct { uint32_t size; uint8_t hovered,pressed,focused,disabled,checked,reserved[3]; } PxuiStateDesc;
/* Table: count 2..16, strictly increasing positions from 0 to 1, finite values from 0 to 1 at the endpoints (overshoot allowed).
   Steps: count > 0. Duration is finite, nonnegative seconds; keyframes use their segment duration. Only active table entries are read. */
/* size-prefixed (164 bytes). */
typedef struct { uint32_t size, kind; float duration,points[4],omega; uint32_t count; float positions[16],values[16]; } PxuiCurveDesc;
typedef struct {
    uint64_t requested,interaction,transition,finite_keys,active_loop,idle_loop;
    uint64_t stepped,step_state,step_build,step_interaction,step_deadline;
    int64_t transition_key;
    PxuiCurveDesc transition_curve;
    uint32_t reserved;
} PxuiFrameReasons;
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
/* ceil intrinsic text width (including text insets) to the integer rendering density. */
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
/* size-prefixed (132 bytes): 40 named slots then 24 reserved ones (a kind added at the end takes a reserved slot: the record and the theme snapshot keep their size). */
typedef struct { uint32_t size; uint16_t toggle,slider,progress,button,icon_button,checkbox,radio,spinner,badge,label,footer,grip,panel,window,popup,modal,dialog,inspector_section,inspector_row,split,scroll_view,toasts,tooltip,tabs,segmented,dropdown,menu_bar,menu_item,menu_row,selection_list,tree,table_row,table_head,picker,search_field,text_field,line_field,value_field,picker_field,section_index; uint16_t reserved[24]; } PxuiBodies;
/* size is sizeof(PxuiThemeSnapshot) as the library was built, part_count the number of parts that follow; the parts are last (the part a later build adds
   comes after them), so a reader built with fewer reads the first min(part_count, its own) and the bodies, which are before them, at the same offsets. */
typedef struct { uint32_t size, part_count; PxuiTokens tokens; float row_h,control_h; PxuiRing ring; float scrollbar_w,scrollbar_gap; PxuiBodies bodies; PxuiStates parts[PXUI_THEME_PARTS]; } PxuiThemeSnapshot;
/* A handle's public head, frozen at 2.0 (owner, 21 Nov 2026): the one field of PxuiUi an app may read, at offset 0, read-only. theme is the
   theme in effect for this handle as pxui_ui_theme lends it, NULL until a theme read on the handle fills it; then read the field in place of the
   call (one load). Lifetime: the library maintains it and the caller must never write it; the pointer, once set, stays valid for as long as the
   handle does, and the snapshot behind it is not rewritten while the handle is usable (a theme swap is refused in a frame, and a themed scope
   builds through a handle of its own). A handle of a callback (a borrowed one) has its own head. The rest of the record is private and may change
   without a minor: never allocate, copy or size a PxuiUi, and never read past the head. */
struct PxuiUi { const PxuiThemeSnapshot *theme; };
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
/* PxuiTargetDesc.reserved_tail flags (2.10). Zero always recomputes the backdrop. The opt-in
   guarantees all backdrop changes are reported in Ui damage, one ordered queue and target per
   renderer, and exclusive draw acknowledgement. Call resize on canvas replacement, cancel,
   or GPU error even if the extent stays the same. Draw all UI layers in order. */
#define PXUI_TARGET_BACKDROP_UI_DAMAGE (1u << 0)
typedef struct { uint32_t size, format; uint8_t samples; uint8_t reserved[3]; uint32_t mask_width,mask_height,color_width,color_height,reserved_tail; } PxuiTargetDesc;
/* wrap: 0 none, 1 word, 2 glyph, 3 word with omitted soft-wrap separator advances in fitting width . */
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
   Flagged colours are independent; underline is a 1.5-point rule at the bottom of the line box. */
typedef struct { uint32_t start,end; uint16_t weight,flags; PxuiColor color,background,underline; } PxuiTextSpan;
/* the spans of one call are a borrowed run, not a fixed array: at most 256 (more is PXUI_ERR_ARGUMENT), `ptr` aligned for PxuiTextSpan when len > 0
   and `stride` sizeof(PxuiTextSpan) (a span carries no size, so no other stride is read; a record shorter than sizeof(PxuiTextSpans) is refused). The ranges must be
   ordered, nonempty, disjoint and on UTF-8 boundaries, the flags known. The library copies what it keeps before the call returns. */
typedef struct { uint32_t size, reserved; const PxuiTextSpan *ptr; size_t len; size_t stride; } PxuiTextSpans;
#define PXUI_TEXT_SPAN_COLOR 1u
#define PXUI_TEXT_SPAN_BACKGROUND 2u
#define PXUI_TEXT_SPAN_UNDERLINE 4u
/* size-prefixed (20 bytes): an out record's size must cover it (the call writes it whole), else PXUI_ERR_ARGUMENT; pxui_paint_label returns it by value. */
typedef struct { uint32_t size; float width,height,baseline; uint32_t lines; } PxuiTextMetrics;
typedef struct { float rect[4],uv[4]; uint32_t atlas,reserved; } PxuiTextGlyph;
#define PXUI_ATLAS_MASK 0u
#define PXUI_ATLAS_COLOR 1u
#define PXUI_ATLAS_MASK_2 2u
#define PXUI_ATLAS_COLOR_2 3u
/* Mask/image glyph instance flag: use the second fixed atlas page. */
#define PXUI_GLYPH_PAGE_2 2u
/* pixel-area coverage for a plain, unstroked RECT instance. */
#define PXUI_RECT_AREA 4u
/* a node's transition is a PxuiTransition (Bezier or spring), so PxuiNode is 488 bytes; tables and steps stay in PxuiCurveDesc. */
/* MASK instance flag for CoreText-matching light text (docs/abi.md).
   extra[0] packs two f16 (ppem in device pixels low, device scale high); the shader grows ink by the fill colour's luminance level.
   Renderers that predate it ignore the bit and the extra words and draw the plain mask. */
#define PXUI_LIGHT 16u
/* logical window points; insets are top/right/bottom/left, rectangles x/y/w/h. size-prefixed (80 bytes); the frame call refuses a record
   whose size is short and stores its own size. An out record (pxui_ui_environment) needs the caller's size to cover it (else PXUI_ERR_ARGUMENT). */
/* scrollers: 0 legacy (gutter, resting thumb) or PXUI_OVERLAY_SCROLLERS (Apple overlay: no gutter, thumb only while scrolling). */
#define PXUI_OVERLAY_SCROLLERS 1u
typedef struct {
    uint32_t size, keyboard_curve, keyboard_visible, scrollers;
    float safe_area[4], keyboard_frame[4], keyboard_from[4];
    double keyboard_started, keyboard_duration;
} PxuiEnvironment;
/* Current UIKit keyboard presentation frame, including intermediate geometry. size-prefixed (24 bytes), as PxuiEnvironment. */
typedef struct { uint32_t size, flags; float frame[4]; } PxuiKeyboard;
#define PXUI_KEYBOARD_VALID 1u
#define PXUI_KEYBOARD_ANIMATING 2u
typedef struct {
    uint32_t size;
    /* Event-only entry: held keys persist until release or focus loss. At most 4096 events; strided (stride below sizeof(PxuiEvent) is refused). */
    PxuiEventSpan events;
    float width, height, scale;
    double time;
    /* One size only: a shorter record (an earlier layout) is refused, a longer one is read as its first sizeof(PxuiFrameIn) bytes. */
    uint32_t appearance, reduced_motion;
    /* Read only when size includes this complete tail; otherwise defaults to zero. */
    PxuiEnvironment environment;
    PxuiKeyboard keyboard;
    /* the surface this frame is for: it must be registered on the Ui (pxui_ui_add_surface), else PXUI_ERR_ARGUMENT, and every event carries it.
       Focus, the accessibility tree, IME and keyboard geometry belong to a surface (today a Ui serves one, so they are the Ui's). */
    uint64_t surface;
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
    /* FILES_DROPPED : x/y position; code 0 NUL-separated paths, code 1 error text. */
    /* 11 (FILE_DROPPED, one path) is reserved and never reused. */
    PXUI_FILE_DRAG_MOVED=12, PXUI_FILE_DRAG_ENDED=13,
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
#define PXUI_ERR_STATE (-3)    /* not allowed now: inside a frame callback, on a borrowed handle, with a renderer attached, a builder (a call that
                                  declares nodes: widgets, containers, pxui_ui_node...) outside a frame callback, or past the returned-string caps below */
#define PXUI_ERR_VERSION (-6)  /* pxui_ui_create: the app was built for another major or a newer minor than this library */
#define PXUI_ERR_STALE (-5)    /* the box handle names a node that has gone (queries and patches treat it as "nothing there") */
#define PXUI_ERR_PANIC (-4)    /* a panic was caught in the library (a library bug: no argument causes one); the Ui fails every call until its next frame
                                  call. A shaper, theme or accessibility bridge whose call panicked fails every later call (its destroy still frees it). */
/* Limits (docs/abi.md, Limits). Every call refuses an argument past its cap with PXUI_ERR_ARGUMENT (or its failure value) rather
   than cuts; the only cap answering PXUI_ERR_STATE is the handed-back string budget below, which the next frame resets:
   - one string: an identifier (a key, a node or container label used as identity, a body, theme or app name, a carry kind) at most 256 bytes;
     display text (labels, titles, accessible names and values, tips, rows) at most 64 KiB; editor text (a text field's or area's value, a
     paste, an edit, text the text service measures or shapes) and one event's text at most 16 MiB; a font at most 32 MiB; a text
     pxui_shaper_prepare takes at most 1 MiB;
   - a call's string span (options, rows, hints, titles) at most 16 MiB together; one frame's event text at most 16 MiB together;
   - strings the library hands back (edited text, lent payloads): at most 4096 and 64 MiB between two frames, then PXUI_ERR_STATE;
   - events: x, y and value finite for every kind, text editing start/length not negative, a resize 0..=65536;
     FrameIn.width/height and PxuiInput.logical_size 0..=65536 points; held key codes must be known keys (PXUI_ERR_ARGUMENT). */
enum PxuiTargetFormat { PXUI_TARGET_RGBA8=1, PXUI_TARGET_BGRA8=2 };
typedef struct {
    uint32_t size;
    float x, y, width, height;
    float padding_points; /* padding in points, fractions kept; read only while it agrees with padding within half a point */
    int64_t padding;
    uint8_t has_height, window, has_max_height, has_stroke, reserved[4];
    float max_height; PxuiColor stroke;
    const PxuiLookDesc *look; /* read where it lies for the call; NULL for the theme's look */
} PxuiPanel;
/* `ui` is the serial of the Ui that made the handle (nonzero, one count for the process). Every call that takes a box compares it with its Ui's
   and refuses a handle of another Ui, or a zeroed one, as a stale box (PXUI_ERR_STALE). Still 32 bytes: `ui` fills the padding. */
typedef struct { uint64_t index; int64_t box_key; uint64_t box_slot; uint32_t generation, ui; } PxuiBox;
/* what happened to a kit button or icon button this frame, in one record: PXUI_RESPONSE_* bits and the button's box.
   The caller sets size; a null or shorter record is refused. PXUI_RESPONSE_SECONDARY, a secondary (right-button) press began
   on the box this frame. */
typedef struct { uint32_t size, flags; PxuiBox node; } PxuiResponse;
/* a modal dialog (at most 64 buttons; masks as PxuiDialogConfig) and what its frame did (chosen is valid when has_chosen) */
/* details, each button's line under its label (empty: none), as many as buttons or none. */
typedef struct { uint32_t size; float width; PxuiStr key, title; PxuiStrSpan buttons; int64_t default_at, cancel_at; uint64_t danger, disabled; PxuiStrSpan details; uint16_t variant; uint8_t reserved_variant[6]; } PxuiDialogDesc;
typedef struct { uint32_t size; uint8_t has_chosen, dismissed, reserved[2]; uint64_t chosen; } PxuiDialogResult;
/* what pxui_ui_segmented, pxui_ui_pick or pxui_ui_tabs chose: chosen when has_chosen is 1 (0: no option is chosen), secondary the option a secondary
   press began on this frame when has_secondary is 1 (a context-menu request; it never chooses). The caller sets size; shorter is refused. */
typedef struct { uint32_t size; uint8_t has_chosen, has_secondary, reserved[2]; uint64_t chosen, secondary; } PxuiSegmentedResult;
enum { PXUI_RESPONSE_CLICKED = 1, PXUI_RESPONSE_HOVERED = 2, PXUI_RESPONSE_HELD = 4, PXUI_RESPONSE_FOCUSED = 8, PXUI_RESPONSE_ACTIVATED = 16, PXUI_RESPONSE_SECONDARY = 32 };
/* a widget's identity and its shown and accessible text, apart (every call taking a PxuiKeyedDesc): `key` is hashed under the parent's
   seed and is identity only (a static string, or an item id as Key::label writes it: a NUL and 16 hex digits); `name` is never parsed for
   a key (no `##`/`###` rule). Set `size` to sizeof(PxuiKeyedDesc); a shorter record or a non-zero `reserved` is refused. */
typedef struct { uint32_t size; uint16_t variant, reserved; PxuiStr key, name; } PxuiKeyedDesc;
/* pxui_ui_pick's kinds (2 is reserved). */
enum { PXUI_CHOICE_RADIO = 0, PXUI_CHOICE_TABS = 1, PXUI_CHOICE_DROPDOWN = 3 };
/* flags bit 0 preview, bit 1 commit, bit 2 held. Initialize size before calling. */
typedef struct { uint32_t size, flags; PxuiBox node; float preview, commit; } PxuiScrubOut;
/* kinds bit 0 long press, 1 row swipe, 2 edge back, 3 pull to refresh, 5 touch drag (the node takes the touch at once as a held press and drag, never a tap, scroll or
   other gesture; PxuiGestureIn kinds & ~47 is refused); reveal is the row-swipe action
   width and threshold the pull distance (0: 60), in points. Initialize size before calling. */
typedef struct { uint32_t size, kinds; float reveal, threshold; } PxuiGestureIn;
/* flags: 1 highlighted, 2 long press (this frame), 4 swiping, 8 swipe open, 16 back active, 32 back commit and
   64 back cancel (this frame), 128 refresh fired (this frame), 256 refreshing. swipe is the signed shift to paint a
   swiped row's content at (mirrored right to left), back is progress 0..1, pull the pull-down in points. */
typedef struct { uint32_t size, flags; float swipe, back, pull; } PxuiGestureOut;
/* row-swipe action widths in points, 0 disables a side (leading follows the layout direction), and the finger
   travel that commits the edge action on release (0: no full swipe). Initialize size before calling. */
typedef struct { uint32_t size; float leading, trailing, full; } PxuiSwipeIn;
/* drag-to-dismiss request: fraction of the node height (0: 0.25) and downward release speed in points per
   second (0: 600) that commit. Initialize size before calling. */
typedef struct { uint32_t size; float fraction, velocity; } PxuiDismissIn;
/* flags: 8192 dragging or settling, 16384 committed and 32768 cancelled (this frame); offset is the downward points to
   translate the node by (negative while rubber-banding up, the node height once a commit settled). */
typedef struct { uint32_t size, flags; float offset; } PxuiDismissOut;
typedef struct { uint16_t depth; uint8_t children; } PxuiTreeNode;
typedef struct { uint32_t size; uint8_t has_stroke,has_max_height,dismiss_initial,attached; PxuiColor stroke; float max_height,x,y,width,height; PxuiStr label; } PxuiPopupDesc;
/* Since 2.3, reserved[0] is 1 when shape instances may carry symbolic palette colors. */
typedef struct { uint32_t size; uint8_t rtl,reserved[3]; int64_t font_size,row_height,density; PxuiRect clip; float clip_radius; PxuiThemeTokens theme; } PxuiPaintInfo;
typedef void (*PxuiIconDraw)(void *user, PxuiPaint *paint, PxuiRect rect, PxuiColor ink, const PxuiPaintInfo *info); /* an icon drawing, called at paint time; info: valid during the call */
/* a label's style. A field whose PXUI_LABEL_SET_* bit in `set` is clear is the theme's label style (font, size,
   weight, tracking in ems, capitals, colour; width full; line height the row height, 0 for 1.2 times the size; pad 12 with 4
   above a first child). color is an app colour token; digits 1 draws one cached character at a time, digits on the advance of 0. */
enum { PXUI_LABEL_SET_FONT = 1, PXUI_LABEL_SET_SIZE = 2, PXUI_LABEL_SET_WEIGHT = 4, PXUI_LABEL_SET_TRACKING = 8, PXUI_LABEL_SET_CAPS = 16,
       PXUI_LABEL_SET_COLOR = 32, PXUI_LABEL_SET_WIDTH = 64, PXUI_LABEL_SET_LINE_HEIGHT = 128, PXUI_LABEL_SET_PAD = 256 };
typedef struct { uint32_t size, set; uint16_t font, weight, color, max_lines; uint8_t caps, digits, ellipsis, reserved;
                 float size_points, tracking, line_height, pad; uint32_t wrap, align, features; PxuiSizeDesc width; } PxuiLabelStyle;
/* a label's box beside its style: height (PXUI_LABEL_BOX_SET_HEIGHT) the node's height with its lines centred in it, role
   (PXUI_LABEL_BOX_SET_ROLE; clear is a label) its accessible role, whole_pixels 1 rounds a fitted width up to whole pixels. */
enum { PXUI_LABEL_BOX_SET_HEIGHT = 1, PXUI_LABEL_BOX_SET_ROLE = 2 };
typedef struct { uint32_t size, set; float height; uint8_t role, whole_pixels; uint16_t variant; } PxuiLabelBox;
/* a kit icon button with options. w_kind/h_kind 0 the kit square, 1 points, 2 fill; states NULL is the theme's icon button, else six
   PxuiLookDesc (idle, hover, pressed, focus, disabled, checked); press_scale is the plate's scale while held (1 = none); the icon is the
   glyph `icon`, or, when `draw` is set, the app's drawing called at paint time with draw_user, the 20-point square and the look's ink.
   press null for the kit's springs, else two PxuiCurve (in on press, out on
   release), 4-aligned and valid for the call. */
/* a curve as pxui's Rust `Curve` lays it out (repr(C, u8)): kind 0 bezier, 1 spring, 2 table, 3 steps. */
typedef struct { uint8_t kind, reserved[3]; union { struct { float duration, points[4]; } bezier; struct { float omega; } spring;
                 struct { float duration; uint32_t count; float positions[16], values[16]; } table; struct { float duration; uint32_t count; } steps; } data; } PxuiCurve;
typedef struct { uint32_t size; uint8_t enabled,w_kind,h_kind,reserved; float w,h,press_scale; const PxuiLookDesc *states; PxuiStr key,name,icon; PxuiIconDraw draw; void *draw_user;
                 const PxuiCurve *press;
                 /* checkable 1 makes the button an on/off key (accessible CHECKABLE, CHECKED and its
                    checked look when checked is 1); checked is 0 unless checkable is 1; reserved2 zero. */
                 uint8_t checkable, checked; uint16_t variant; uint8_t reserved2[4]; } PxuiIconButtonDesc;
/* a kit button with options. w_kind/h_kind 0 the kit's size, 1 points, 2 fill; justify 0 start, 1 centre, 2 end; states NULL is the theme's button, else six PxuiLookDesc (idle, hover, pressed, focus, disabled, checked).
   centered 1 makes the box exactly its size with nothing around it in its line, its text centred. */
typedef struct { uint32_t size; uint8_t enabled,selected,danger,justify,w_kind,h_kind,has_name,reserved; float w,h,padding[4]; const PxuiLookDesc *states; PxuiStr key,text,name; uint8_t centered, reserved2; uint16_t variant; uint8_t reserved5[4];
 uint8_t has_label,axis,content_align,focus_on_click; float gap; PxuiLabelStyle label; PxuiLabelBox label_box;
                 /* lead the leading icon's glyph (empty: none) or lead_draw, the app's
                    drawing with lead_user (as PxuiIconButtonDesc.draw; the word pxui_ui_icon_word set is its word), which wins; detail a line under the
                    text and accessory a trailing text at the box's end (empty: none), each in detail_label/detail_box or accessory_label/accessory_box
                    when has_detail_label/has_accessory_label is 1 (else the theme's label style); reserved3 zero. With a build callback the
                    callback's contents replace the text and these parts (they are not drawn or given to a body). */
                 PxuiStr lead; PxuiIconDraw lead_draw; void *lead_user; PxuiStr detail, accessory; uint8_t has_detail_label, has_accessory_label, reserved3[6];
                 PxuiLabelStyle detail_label; PxuiLabelBox detail_box; PxuiLabelStyle accessory_label; PxuiLabelBox accessory_box;
                 /* the leading icon's square in points (0: the control-high one) */ float icon_size; } PxuiButtonDesc;
/* a button's content callback; state is the button's interaction state this frame, valid for the call. */
typedef void (*PxuiButtonBuild)(void *user, PxuiUi *ui, const PxuiStateDesc *state);
/* PxuiToggleDesc grows by the title's text style (has_style 1: style, else the theme's toggle font; the whole record is read)
   and PxuiSegmentedDesc by the strip's width w (0: as grow says), padding (the text's inset, 8 the kit's), min_segment (0: none) and names
   (empty: each PxuiPickItem carries its own name; the whole record is read). */
typedef struct { uint32_t size; uint8_t enabled,on,press_row,reserved; float row_h,switch_w,switch_h,inset; const PxuiLookDesc *track,*knob; PxuiStr key,text,detail; uint8_t has_style, reserved2; uint16_t variant; uint8_t reserved3[4]; PxuiLabelStyle style; } PxuiToggleDesc;
/* a kit checkbox with its style (pxui_ui_checkbox; all zero but size, key and text is the plain checkbox). box_size 0 is the kit's 12 points (finite, not negative); look is NULL for the theme's checkbox, else six looks (idle, hover, pressed, focus, disabled, checked); disabled 1 shows the disabled look and blocks presses and focus; passive 1 makes a mark with no input of its own (with empty text the box alone). key is identity only, as PxuiKeyedDesc.key. */
typedef struct { uint32_t size; uint8_t disabled, on, passive, reserved; float box_size; const PxuiLookDesc *look; PxuiStr key, text; uint16_t variant; uint8_t reserved_variant[6]; } PxuiCheckboxDesc;
/* has_selected 1 when selected is the choice (clamped to
   the options), 0 for no choice (final; selected not read; the result reports none); reserved3 zero. */
/* one pick-one item (pxui_ui_pick, pxui_ui_segmented), borrowed for the call and must not change during the call, including from body callbacks: has_id 1 keys it by id (its node, focus and
   secondary press follow it through a reorder), 0 by its place; reserved zero; name empty for the text; detail extra body text (may be empty). */
typedef struct { uint8_t has_id, reserved[7]; int64_t id; PxuiStr text, name, detail; } PxuiPickItem;
typedef struct { const PxuiPickItem *ptr; size_t len; } PxuiPickItemSpan;
/* one row of a picker fill (the PxuiPickerRows callback): art_kind PXUI_PICK_ART_NONE 0 (art 0), _GLYPH 1 (art a Unicode scalar value, the icon font's character) _IMAGE 2 (art an Images id, drawn by a registered picker body) or _DRAW 3 (art the app's 32-bit icon word, drawn by a registered picker body; the built-in body leaves the slot empty); heading non-empty starts a group under that heading (a heading is not a row). size must be sizeof(PxuiPickRow) and reserved zero. */
enum { PXUI_PICK_ART_NONE = 0, PXUI_PICK_ART_GLYPH = 1, PXUI_PICK_ART_IMAGE = 2, PXUI_PICK_ART_DRAW = 3 };
typedef struct { uint32_t size; uint8_t art_kind, reserved[3]; uint64_t art; PxuiStr label, detail, heading; } PxuiPickRow;
typedef struct { const PxuiPickRow *ptr; size_t len; } PxuiPickRowSpan;
/* how pxui_ui_modal presents a modal: present PXUI_PRESENT_CENTERED 0, _BOTTOM 1 (on the content area's bottom edge), _FULL 2 (filling it) or _TOP 3 (centred across, its top edge top points below the content area's top); width read when has_width is 1 (finite, not negative); a bottom or full sheet takes a downward drag-to-dismiss committing past dismiss_fraction of its height or dismiss_velocity points/s (finite, not negative; 0: a quarter, 600). reserved zero. */
enum { PXUI_PRESENT_CENTERED = 0, PXUI_PRESENT_BOTTOM = 1, PXUI_PRESENT_FULL = 2, PXUI_PRESENT_TOP = 3 };
typedef struct { uint32_t size; uint8_t has_width, present, reserved[2]; float width, dismiss_fraction, dismiss_velocity, top, max_h /* bottom/top height cap, 0 none */; } PxuiSheetDesc;
typedef struct { uint32_t size; uint8_t enabled,grow,reserved[2]; float h; const PxuiLookDesc *states; PxuiStr key,name; size_t selected; float w, padding, min_segment; uint16_t variant, reserved2; PxuiStrSpan names;
 uint8_t has_selected, reserved3[7];
                 /* (a shorter record reads 0): 1 for a press on an option to focus it (it is then the Tab stop), 0 for a press that chooses without focusing; reserved4 zero. */
                 uint8_t focus_on_click, reserved4[7]; } PxuiSegmentedDesc;
typedef struct { uint32_t size; uint8_t active,usable,reserved[2]; PxuiStr caption,keycap; } PxuiHeadActionDesc;
typedef struct { const PxuiHeadActionDesc *ptr; size_t len; } PxuiHeadActionSpan;
typedef struct { uint32_t size; uint8_t has_kind,has_badge,has_index,has_reset,reset_enabled,reserved[3]; PxuiStr key,kind,badge,index,reset,title,detail; PxuiHeadActionSpan actions; } PxuiHeaderDesc;
typedef struct { uint32_t size; uint8_t has_chosen,reset_pressed,reserved[2]; PxuiStr renamed; int64_t chosen; } PxuiHeadOut;
typedef struct { float rect[4]; int16_t clip[4]; uint8_t color[4], color2[4]; uint32_t aux[2]; uint16_t radii[4]; uint16_t width; uint8_t kind, flags; uint16_t view, clip_radius; uint32_t extra[2]; } PxuiInstance; /* 64 bytes; layouts are frozen from the first release, which has not happened yet */
/* One layer of the last frame (a floating root's, in draw order): its root box key, retained id, instances (an empty layer draws nothing),
   texture break count and the instance ranges changed since it was last drawn. Borrowed until the next frame. */
typedef struct { uint32_t start, end; } PxuiDirty;
typedef struct { const PxuiDirty *ptr; size_t len; } PxuiDirtySpan;
/* kind 9 is a filled triangle; rect = Ax,Ay,Bx,By, extra = Cx,Cy as f32 bits. */
typedef struct { const PxuiInstance *ptr; size_t len; } PxuiInstanceSpan;
/* size-prefixed (56 bytes); the layer span is strided. */
/* role: 0 base (the window's content), 1 popup or menu, 2 modal sheet, 3 tooltip, 4 what a drag carries, 5 the merged last layer; a host may present
   a floating layer in a surface of its own (input for it then arrives with that surface's id); anchor is where it is placed, in window points. */
typedef struct { uint32_t size, id, breaks, role; int64_t key; PxuiInstanceSpan instances; PxuiDirtySpan dirty; PxuiRect anchor; } PxuiLayerInfo;
typedef struct { const PxuiLayerInfo *ptr; size_t len; size_t stride; } PxuiLayerSpan;
/* repaint_hz is the highest active ambient rate; zero is display rate or no active animation. */
typedef struct { uint32_t size,repaint_hz; uint64_t draw_revision; double repaint_at; uint64_t built_nodes,rebuilt_views,arranged_nodes; PxuiPlatformOut platform; PxuiDamage damage; PxuiA11yTree a11y; PxuiLayerSpan layers; PxuiDamageList damage_list; } PxuiOutput;
/* A window into a layer's instance storage: the caller stores 64-byte instances at at (stamping clip, clip_radius_f16 and view) and
   advances it; pxui_paint_span commits what was written (at is where it stopped) and hands out the next window. */
typedef struct { uint32_t size; uint16_t view,clip_radius_f16; int16_t clip[4]; PxuiInstance *at,*end; } PxuiSpan;
typedef void (*PxuiSlideText)(void *user, float fraction, PxuiStr *out);
typedef void (*PxuiScrubText)(void *user, PxuiStr origin, float dx, uint8_t coarse, PxuiStr *out);
/* flags: 1 left, 2 edit, 4 bare, 8 kept_drag, 16 has_size, 32 has_fraction, 64 has_tracking, 128 has_line, 256 has_lead, 512 has_trail, 1024 has_display, 2048 has_placeholder */
typedef struct { uint32_t size, flags; int64_t font_size; float fraction, tracking; PxuiColor line, lead_color, trail_color; uint8_t reserved[4]; float x,y,width,height; PxuiStr display,placeholder,lead,trail,label,value; } PxuiValueFieldDesc;
/* The callback UI pointer is borrowed and valid only for that callback; do not retain or destroy it. Only the handle of the innermost
   running callback may be used: while a container's callback runs, the handle of the callback it was called from, and the owner handle,
   are refused (PXUI_ERR_STATE, and zero or null for a call that returns a value, SIZE_MAX or UINT32_MAX for the pxui_text_* calls whose
   zero is an answer; pxui_last_error says why) before the engine is touched.
   Between callbacks of a frame (the engine's own phases) no handle is usable. pxui_last_error, pxui_ui_output (which builds nothing in
   a frame) and pxui_ui_destroy / pxui_frame / pxui_ui_use_theme (which refuse in a frame themselves) stay callable on the owner. Outside
   a frame the owner is the handle. A renderer's calls that reach the Ui (prepare, draw) are refused inside a frame too. */
typedef void (*PxuiBuild)(void *user, PxuiUi *ui);
typedef void (*PxuiPaintBuild)(void *user, PxuiPaint *paint, PxuiRect rect, const PxuiPaintInfo *info); /* info: valid during the call */
typedef void (*PxuiPaintRelease)(void *user);
typedef void (*PxuiSpanBuild)(void *user, float x, float width);
/* Extension set; the callback fills out with the rows for query (each checked once; at most the row cap). */
typedef void (*PxuiPickerRows)(void *user, PxuiStr query, PxuiPickRowSpan *out);
typedef uint8_t (*PxuiTextValid)(void *user, PxuiStr value);
typedef struct { uint32_t size,reserved; PxuiStr title; PxuiStrSpan items; } PxuiMenuDesc;
typedef struct { const PxuiMenuDesc *ptr; size_t len; } PxuiMenuDescSpan;
typedef void (*PxuiGridBuild)(void *user, PxuiUi *ui, size_t index);
typedef struct { uint32_t size,reserved; PxuiStr label; size_t count; float cell_w,cell_h; PxuiGridBuild build; void *user; } PxuiGridListDesc;
/* build: SIZE_MAX for leading content, SIZE_MAX-1 for the sticky heading, otherwise a row index. */
/* `key` returns the item id of row `index`; row boxes are keyed by it (a row's state follows its item). A record
   ending before `key` (56-byte `user` end, size 64 without it) or a null `key` keys rows by index. */
typedef int64_t (*PxuiRowKey)(void *user, size_t index);
/*: selection lists, tables and headings, and the section list with its trailer. One export a widget; every record is
   checked in one pass before any effect; results are library-written records (the caller sets size). Indices are SIZE_MAX for none. */
/* written by the library and lent for the callback; size-prefixed (sizeof(PxuiCellState), a later library may lend a longer record), flags 0 or 1 (read as != 0), reserved 0. */
typedef struct { uint32_t size; uint8_t selected, enabled, focused, held; PxuiBox handle; float y; uint32_t reserved; } PxuiCellState;
/* A section or selection list part builder: index SIZE_MAX the lead-in, SIZE_MAX-1 the heading, SIZE_MAX-2 the trailer, else a row;
   cell describes the part (library-written; a section list's row: its box and y, enabled, the rest 0). */
typedef void (*PxuiListCell)(void *user, void *ui, size_t index, const PxuiCellState *cell);
typedef void (*PxuiTableCellBuild)(void *user, void *ui, size_t row, size_t column, const PxuiCellState *cell);
typedef struct { uint32_t size; uint8_t overlay_scrollbar, measured, reorder, reserved; size_t count; float row_height, leading_height, header_height, trailing_height; } PxuiSectionListDesc;
typedef struct { uint32_t size, reserved; PxuiBox handle, row; size_t activated, selected, secondary, anchor, drop_at, moved_from, moved_to; } PxuiListResult;
/* rows: one flag byte a row (1 selected, written by the list; 2 disabled, 4 live and 8 no drop (with has_drop_look the row is no drop zone and keeps its look), written by the caller), at least count; anchor SIZE_MAX: none.
   with has_revision 1 a row is kept (its body and cells not built) while revision, its key and its selected, disabled and focused state hold;
   a pointer, key or action reaching it rebuilds it; a live row and every row of a reorderable list rebuild every frame.
   a kept row past the viewport (culled) keeps its build until it scrolls into view, a reorderable list's too. */
typedef struct { uint32_t size; uint8_t mode /* 0 single, 1 multi, 2 toggle, 3 none */, fit, open_on_click, overlay_scrollbar, measured, reorder, has_drop_look, bare /* no frame look or inset */; uint8_t has_revision, reserved[3]; size_t count; float row_height, leading_height, header_height, trailing_height; PxuiU8MutSpan rows; size_t anchor; PxuiLookDesc drop_look; uint64_t revision; } PxuiSelectionListDesc;
/* share > 0 splits the width the point columns leave by weight, else w points (0: hidden); id keys the column when has_id is 1 (else its index; unique). */
typedef struct { uint32_t size; uint8_t sortable, grip, has_id, reserved; PxuiStr title; int64_t id; float share, w; } PxuiColumn;
typedef struct { const PxuiColumn *ptr; size_t len; } PxuiColumnSpan;
typedef struct { int64_t id; float w; uint32_t reserved; } PxuiColumnWidth;
typedef struct { PxuiColumnWidth *ptr; size_t len; } PxuiColumnWidthSpan;
/* at most 64 columns; widths one a column, rewritten with every column's laid-out width; the sort is sort_id when has_sort is 1. */
typedef struct { uint32_t size; uint8_t has_sort, ascending, reserved[2]; PxuiColumnSpan columns; PxuiColumnWidthSpan widths; int64_t sort_id; float h; uint32_t reserved2; } PxuiTableHeadDesc;
typedef struct { uint32_t size; uint8_t sorted, resized, has_sort, ascending; int64_t sort_id; PxuiResponse response; } PxuiHeadResult;
typedef struct { uint32_t size; uint8_t mode /* 0 single, 1 multi, 2 toggle, 3 none */, has_sort, ascending, has_revision /* as PxuiSelectionListDesc's */; PxuiColumnSpan columns; PxuiColumnWidthSpan widths; int64_t sort_id; size_t rows; float row_h, head_h; PxuiU8MutSpan flags; size_t anchor; uint16_t head_variant; uint8_t open_on_click, bare /* as PxuiSelectionListDesc's */, reserved2[4]; uint64_t revision; } PxuiTableDesc;
/* head_variant: the table_head body variant of the heading (0: the theme's); the rows take the table_row variant of the call's PxuiKeyedDesc. */
typedef struct { uint32_t size, reserved; PxuiListResult rows; PxuiHeadResult head; } PxuiTableResult;
typedef struct { uint32_t size,axis; PxuiStr label; float ratio,min_a,min_b; uint32_t reserved; PxuiBuild build_a; void *user_a; PxuiBuild build_b; void *user_b; } PxuiSplitDesc;
typedef struct { uint32_t size; uint16_t variant, reserved; PxuiStr label; PxuiMenuDescSpan menus; } PxuiMenuBarDesc;
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
/* 1 when this library, as built, has the feature `id` (PXUI_FEATURE_*), else 0 (also for an id it does not know, a later minor's say). Features are the
   parts of this header a build may lack (a backend, the accessibility bridge, the extension set); every other call and record is in every build of the
   major version and is not asked about. Ids are append-only and never reused; none is named for a version. */
uint32_t pxui_supports(uint32_t id);
enum {
    PXUI_FEATURE_A11Y_BRIDGE = 1,        /* pxui_a11y_*: the AT-SPI2 bridge (Linux builds) */
    PXUI_FEATURE_METAL = 2,              /* the Metal renderer backend (answers from the target; becomes cfg!(feature) if a backend turns optional) */
    PXUI_FEATURE_VULKAN = 3,             /* the Vulkan renderer backend (same) */
    PXUI_FEATURE_EXTENSION_SET = 4       /* abi/pxui.ext.txt: the Rays vocabulary and the diagnostics, no stability promise */
};
/* Extension set (diagnostics; no stability promise, PXUI_FEATURE_EXTENSION_SET). pxui_counter: process counters for performance gates: which 1 =
   heap allocations the library's allocator has made (a counting build), 2.. the allocator's other counters; anything else reads 0. */
uint64_t pxui_counter(uint32_t which);
/* pxui_ui_diagnostics reads and writes one record: in, timing (0 leave the phase timing as it is, 1 on, 2 off) and rebuilt_prefix (the label prefix
   rebuilt counts; empty: every view); out, calls (calls into the library on this Ui: its owner, the handles its callbacks borrowed, its renderers and
   paint handles, since it was created; this call is not counted; per Ui, no counter is shared between Uis or threads), text_layouts (the text
   cache's layout count; has_text_layouts 0 when none), rebuilt (views whose label starts with rebuilt_prefix that the last frame built instead of
   kept), phase_nanos and, when has_cpu is 1, phase_cpu_nanos (the last frame's wall and thread CPU time in the phases begin, build, intrinsic,
   arrange, paint, a11y, finish; zero until timing is on and a frame has run). A refused call writes nothing. */
typedef struct { uint32_t size; uint8_t timing, has_text_layouts, has_cpu, reserved; PxuiStr rebuilt_prefix; uint64_t calls, text_layouts, rebuilt, phase_nanos[7], phase_cpu_nanos[7]; } PxuiDiagnostics;
int32_t pxui_ui_diagnostics(PxuiUi *ui, PxuiDiagnostics *io);
/* Creates a Ui with the `count` fonts of `fonts` (0, and `fonts` may be NULL: the library's embedded default face; the first font is the theme's;
   limits below). Returns NULL, with *status (when not NULL) set to PXUI_ERR_ARGUMENT for an invalid record, font or count or PXUI_ERR_VERSION for
   an app built for another major or a newer minor than this library; 0 on success. */
PxuiUi *pxui_ui_create(const PxuiUiDesc *desc, const PxuiStr *fonts, size_t count, int32_t *status);
/*  font limits (any entry point that adds a font: this one, pxui_shaper_create, pxui_shaper_add_font, pxui_text_add_font): a font is
   1 byte to 32 MiB (33,554,432), a text service holds at most 64 fonts and 256 MiB of font bytes in all, and bytes that are not a parseable font
   are refused (UINT32_MAX / NULL, nothing kept). The bytes are copied and fonts are never removed. */
int32_t pxui_ui_destroy(PxuiUi *ui); /* PXUI_ERR_STATE: borrowed handle or inside a frame. A renderer still attached keeps the Ui until the last one is destroyed; the handle is invalid for the caller either way */
PxuiStr pxui_last_error(const PxuiUi *ui);
int32_t pxui_ui_environment(PxuiUi *ui, PxuiEnvironment *out);
int32_t pxui_ui_keyboard(PxuiUi *ui, PxuiKeyboard *out);
int32_t pxui_ui_text_input_options(PxuiUi *ui, PxuiTextInputOptions *out);
int32_t pxui_ui_content_rect(PxuiUi *ui, PxuiRect *out);
int32_t pxui_frame(PxuiUi *ui, const PxuiFrameIn *in, PxuiBuild build, void *user, PxuiFrameOut *out);
/* Like pxui_frame, with authoritative held keys, buttons, pointer and timing. Held state creates no edge.
   The events retain their order and surface. The snapshot supplies the drawable scale on each axis. */
int32_t pxui_frame_with_snapshot(PxuiUi *ui, const PxuiFrameIn *in, const PxuiInputSnapshot *snapshot, PxuiBuild build, void *user, PxuiFrameOut *out);
/* register / remove the surface of a Ui (a nonzero id). A Ui is created with none: register one before the first frame (an unregistered
   surface is PXUI_ERR_ARGUMENT there). Adding while one is registered (a second id, or the same twice) is PXUI_ERR_STATE (shortcut: one surface per Ui,
   upgrade when the engine keeps per-surface focus/a11y/IME state); removing an id that is not registered is PXUI_ERR_ARGUMENT; both are refused in a frame. */
int32_t pxui_ui_add_surface(PxuiUi *ui, uint64_t surface);
int32_t pxui_ui_remove_surface(PxuiUi *ui, uint64_t surface);
/* The last frame's input as routed to `owner` (a viewport's root key; has_owner 0: the app's), for the app's own content: the record
   an output-only PxuiInput (pxui_frame takes a PxuiFrameIn), borrowed until the next call. PXUI_ERR_STATE before the first frame. */
int32_t pxui_ui_input(PxuiUi *ui, uint8_t has_owner, int64_t owner, PxuiInput *out);
/* Marks layer `layer` of the last frame drawn (its dirty ranges clear), as pxui_renderer_draw does: for a host that reads the layers itself. */
int32_t pxui_ui_layer_drawn(PxuiUi *ui, uint64_t layer);
const PxuiOutput *pxui_ui_output(PxuiUi *ui); /* borrowed from the Ui until its next frame */
/* Copies the latest damage, including between-frame patches. PxuiDamage and PxuiFrameOut report its bounding box. */
int32_t pxui_ui_damage_list(PxuiUi *ui, PxuiDamageList *out);
/* Enable the compositor contract between frames. The host draws these runs separately. */
int32_t pxui_ui_set_promotion(PxuiUi *ui, uint8_t enabled);
/* Returned runs borrow the UI until its next mutation. */
int32_t pxui_ui_promoted(PxuiUi *ui, const PxuiPromoted **ptr, size_t *len);
int32_t pxui_ui_promote(PxuiUi *ui, const PxuiBox *box);
/* The theme in effect, lent: valid until the Ui next changes it (a theme swap, a themed scope) or is freed. NULL when the call is refused
   (a poisoned Ui, a handle that is not the current one): see pxui_last_error. */
/* The theme as the engine keeps it: each PxuiTokens text size is a float in the low half of its int64_t slot (the other half 0),
   fonts carry a fractional size in PxuiFont's tail padding. it also records the theme in the handle's head (PxuiUi.theme). */
const PxuiThemeSnapshot *pxui_ui_theme(PxuiUi *ui);
/* Measured virtual rows: count <= 1,000,000, finite positive estimate and finite estimated total.
   Called inside a frame; build may repeat while measured heights settle. The returned box scrolls the list. */
int32_t pxui_ui_grid_list(PxuiUi *ui, const PxuiGridListDesc *desc, PxuiBox *out);
/* one scroll region of a scrolling lead-in (index SIZE_MAX), a sticky heading (SIZE_MAX-1), fixed or measured rows and a trailer (SIZE_MAX-2, when trailing_height > 0); build gets each part's cell (its box and logical content Y, the PxuiListCell of a selection list). */
int32_t pxui_ui_section_list(PxuiUi *ui, const PxuiKeyedDesc *keyed, const PxuiSectionListDesc *desc, PxuiRowKey key, PxuiListCell build, void *user, PxuiListResult *out);
/* a selectable list over the section geometry (selection semantics in the list; rows and anchor updated in place); replaces the 1.x list views. */
int32_t pxui_ui_selection_list(PxuiUi *ui, const PxuiKeyedDesc *keyed, const PxuiSelectionListDesc *desc, PxuiRowKey key, PxuiListCell cell, void *user, PxuiListResult *out);
/* a heading of columns (tri-state sort, weighted and gripped widths). */
int32_t pxui_ui_table_head(PxuiUi *ui, const PxuiKeyedDesc *keyed, const PxuiTableHeadDesc *desc, PxuiHeadResult *out);
/* a table: a heading over selectable rows of cells (replaces the text table and the 1.x table views). */
int32_t pxui_ui_table(PxuiUi *ui, const PxuiKeyedDesc *keyed, const PxuiTableDesc *desc, PxuiRowKey key, PxuiTableCellBuild cell, void *user, PxuiTableResult *out);
int32_t pxui_ui_inspector_row(PxuiUi *ui, PxuiInspectorRowDesc *desc);
int32_t pxui_ui_inspector_section(PxuiUi *ui, PxuiInspectorSectionDesc *desc);
int32_t pxui_ui_inspector_toggle(PxuiUi *ui, PxuiStr key, PxuiStr label, uint8_t value, uint8_t *out);
int32_t pxui_ui_inspector_button(PxuiUi *ui, PxuiStr key, PxuiStr label, uint8_t *out);
int32_t pxui_ui_inspector_readout(PxuiUi *ui, const PxuiInspectorValueDesc *desc);
int32_t pxui_ui_inspector_message(PxuiUi *ui, const PxuiInspectorTextDesc *desc);
int32_t pxui_ui_view_size(PxuiUi *ui, PxuiPoint *out);
int32_t pxui_ui_wants_frame(PxuiUi *ui, PxuiEventSpan events, double now, uint8_t *out);
int32_t pxui_ui_footer(PxuiUi *ui, const PxuiFooterDesc *desc);
/* repeat 1 plays once, 0 forever, otherwise 1..65535 cycles (a count includes the reverse leg); autoreverse is 0/1; start_offset is seconds in. */
int32_t pxui_ui_animate_keys(PxuiUi *ui, const PxuiBox *box, PxuiKeyframeSpan keys, uint32_t repeat, uint8_t autoreverse, float start_offset, uint8_t *out);
/* Same playback options, with an explicit idle cadence in Hz (0: display rate). */
int32_t pxui_ui_animate_keys_at_rate(PxuiUi *ui, const PxuiBox *box, PxuiKeyframeSpan keys, uint32_t repeat, uint8_t autoreverse, float start_offset, uint8_t rate_hz, uint8_t *out);
/* Last host appearance; themes remain authored by the application. use_theme selects a parsed root
   theme between frames, avoiding reparsing source when appearance changes. */
int32_t pxui_ui_system_appearance(PxuiUi *ui, uint32_t *out);
int32_t pxui_ui_use_theme(PxuiUi *ui, const PxuiTheme *theme);
/* app colour tokens. set_colors installs light[i]/dark[i] as token i between frames (count at most 64, 0 clears;
   a changed table rebuilds every view once). color reads a token for the theme in effect: the dark entry while that theme
   is dark (panel channel sum below 384), else the light one; transparent past the table or on a dead handle. */
PxuiColor pxui_ui_color(PxuiUi *ui, uint16_t token);
int32_t pxui_ui_set_dir(PxuiUi *ui, uint32_t dir);
int32_t pxui_ui_set_a11y(PxuiUi *ui, uint8_t on);
/* action_code: one bit, 1 press, 2 focus, 4 set value, 8 scroll, 16 expand, 32 collapse, 64 select, 128 move up, 256 move down
   (a reorderable list row). */
/* Each action has finite payload and zero reserved; scroll deltas saturate to finite points before content clamping.
   At most 4096 pending actions per Ui until the next frame; a whole batch past the available room is refused with PXUI_ERR_STATE. */
int32_t pxui_ui_inject_a11y(PxuiUi *ui, PxuiA11yActionSpan actions);
int32_t pxui_ui_a11y_name(PxuiUi *ui, const PxuiBox *box, PxuiStr name);
int32_t pxui_ui_a11y_value(PxuiUi *ui, const PxuiBox *box, PxuiStr value, uint32_t anchor, uint32_t caret);
int32_t pxui_ui_a11y_number(PxuiUi *ui, const PxuiBox *box, double now, double min, double max, double step);
int32_t pxui_ui_set_role(PxuiUi *ui, const PxuiBox *box, uint32_t role, uint32_t states);
int32_t pxui_ui_focus_box(PxuiUi *ui, const PxuiBox *box);
/* the box a key press takes as its focus while no node holds it (its nearest focusable box; stated each build). */
int32_t pxui_ui_focus_default(PxuiUi *ui, const PxuiBox *box);
int32_t pxui_ui_focused(PxuiUi *ui, const PxuiBox *box, uint8_t *out);
int32_t pxui_ui_state(PxuiUi *ui, const PxuiBox *box, int64_t default_value, int64_t *out);
int32_t pxui_ui_set_state(PxuiUi *ui, const PxuiBox *box, int64_t value);
/*  retains numeric routing storage; signal events remain borrowed through the next frame. */
int32_t pxui_ui_signal(PxuiUi *ui, const PxuiBox *box, PxuiSignal *out);
int32_t pxui_ui_patch_text(PxuiUi *ui, const PxuiBox *box, PxuiStr value, uint8_t *out); /* out: whether the text was written; a stale box is PXUI_ERR_STALE */
int32_t pxui_ui_busy_step(PxuiUi *ui, const PxuiBox *box, double now, uint8_t *out);
int32_t pxui_ui_request_frame(PxuiUi *ui);
/* 2.4: advances running transitions to `time` without a build (patching animated nodes in place); *out 0 when the next frame
   needs a build (input, a timer, a frame request, layout settling) and nothing changed. A host with nothing new calls it in place of a frame. */
int32_t pxui_ui_step(PxuiUi *ui, double time, uint8_t *out);
/* 2.5: copies cumulative scheduling counters, enabled at Ui creation by PXUI_FRAME_REASONS=1. */
int32_t pxui_ui_frame_reasons(PxuiUi *ui, PxuiFrameReasons *out);
int32_t pxui_ui_node(PxuiUi *ui, PxuiStr label, const PxuiNode *node, PxuiBox *out); /* node->size = sizeof(PxuiNode); copied in the call */
/* a keyless layout container: the n-th of the current scope, keyed by the library in a positional namespace no label
   or id reaches (labels and images are keyed the same way, each kind counted apart). No caller text crosses. */
int32_t pxui_ui_layout_node(PxuiUi *ui, const PxuiNode *node, PxuiBox *out);
int32_t pxui_ui_node_keyed(PxuiUi *ui, int64_t key, const PxuiNode *node, PxuiBox *out); /* keys 0 and 1 are reserved */
int32_t pxui_ui_rect(PxuiUi *ui, const PxuiBox *box, PxuiRect *out);
uint8_t pxui_ui_box_of_key(PxuiUi *ui, int64_t key, PxuiBox *out);
/* 1 and the live box, or 0 without changing out for a retired/unknown identity. */
uint8_t pxui_ui_box_of_a11y(PxuiUi *ui, uint64_t id, PxuiBox *out);
/* The text of a node: value alone (style, spans NULL, line_box 0), in a label style with spans (resolved against the Ui's theme; digits
   ignored), or with line_box 1 as CSS line boxes in style (a positive line height). Spans or a line box need a style. */
int32_t pxui_ui_text(PxuiUi *ui, const PxuiBox *box, PxuiStr value, const PxuiLabelStyle *style, const PxuiTextSpans *spans, uint8_t line_box);
int32_t pxui_ui_style(PxuiUi *ui, const PxuiBox *box, const PxuiLookDesc *look, const PxuiStateDesc *state); /* the one look the state picks (idle, else as States::pick: disabled, pressed, hovered, checked, focused), checked and applied */
int32_t pxui_ui_draw(PxuiUi *ui, const PxuiBox *box, uint8_t over, PxuiPaintBuild build, void *user, PxuiPaintRelease release); /* a span must be committed (pxui_paint_span with at_least 0) before the callback returns; one left open, or a pxui_paint_* call made under an open span, is refused and the frame that ran the callback returns PXUI_ERR_STATE */
int32_t pxui_ui_within(PxuiUi *ui, const PxuiBox *box, PxuiBuild build, void *user);
/* Builds `build` under the id scope of `label` (the Rust ui.id). */
int32_t pxui_ui_scope(PxuiUi *ui, PxuiStr label, PxuiBuild build, void *user);
/* a label of `text` in `style` with the label's box (null for none), writing the label's node to `out` (for a fill, an
   accessible state or text laid into it); a null style is the theme's label. A short box, unknown set bits, a non-finite height or an
   unknown role is refused. */
/* A keyed label within the specified parent-relative rectangle, using rounded line-box baselines. */
int32_t pxui_ui_label_at(PxuiUi *ui, PxuiStr key, PxuiRect rect, PxuiStr text, const PxuiLabelStyle *style, PxuiBox *out);
int32_t pxui_ui_label(PxuiUi *ui, PxuiStr text, const PxuiLabelStyle *style, const PxuiLabelBox *label_box, PxuiBox *out);
/* the PxuiTextStyle `style` shapes with, resolved against the Ui's theme label as pxui_ui_label resolves it (unset
   fields the theme label's, tracking in points); `digits` is ignored. Refused for a style pxui_ui_label would refuse. */
int32_t pxui_ui_resolve_label(PxuiUi *ui, const PxuiLabelStyle *style, PxuiTextStyle *out);
/* A text field keyed by desc->key and named desc->name to a screen reader (empty: unnamed). changed receives the new text (borrowed
   until the next call) when out_changed is 1. Each text input reports what happened to its box in one call: `response` (size set by the
   caller; a shorter record is refused) takes the box, hover, press, focus and click; `name` (search, line and value fields, text area)
   names the box to a screen reader. A refused text input call (outside a frame among them) writes none of its outputs. */
int32_t pxui_ui_text_field(PxuiUi *ui, const PxuiTextFieldDesc *desc, PxuiStr *changed, uint8_t *out_changed, PxuiResponse *response);
/* action : the on-screen keyboard's return key, 0 Return, 1 Done, 2 Search (no autocorrection or capitalization). */
int32_t pxui_ui_search_field(PxuiUi *ui, PxuiStr label, PxuiStr icon, PxuiStr value, PxuiStr placeholder, PxuiStr name, uint8_t action, uint16_t variant, uint8_t *changed, PxuiStr *out, PxuiResponse *response);
int32_t pxui_ui_line_field(PxuiUi *ui, float x, float y, float w, float h, uint8_t grab, PxuiStr placeholder, PxuiStr label, PxuiStr value, PxuiStr name, PxuiStr *text, uint32_t *step, PxuiResponse *response);
/* Caller-painted horizontal control: finite ordered range (equal ends are read-only), positive finite step. */
/* an image with no key (the n-th image among its siblings, as a label is), `name` its accessible name only (never parsed for a
   key). PXUI_ERR_ARGUMENT, nothing added, for an invalid descriptor (zero size, no native handle, a depth format, a short size), an
   unknown fit, a size or a name the library refuses. */
/* an image keyed by `key` (as pxui_ui_node keys a label) and named `name` for accessibility only. */
int32_t pxui_ui_image(PxuiUi *ui, PxuiStr key, PxuiStr name, const PxuiImage *image, uint32_t fit, PxuiSizeDesc width, PxuiSizeDesc height, PxuiBox *out);
int32_t pxui_ui_tip(PxuiUi *ui, const PxuiBox *box, PxuiStr label);
/* One row of a context menu, strided like every span and read at its own size: text and shortcut (empty: none) are UTF-8; enabled, danger,
   selected (the current choice of a settings menu: CHECKED and SELECTED), has_swatch and has_id are 0 or 1; checked is 0 (no mark slot), 1
   (a slot) or 2 (a slot with the mark); swatch is read when has_swatch is 1, id (the row's key in place of its position) when has_id is 1.
   submenu holds the rows this row opens (none: no submenu); a submenu's rows have no submenu of their own. reserved is zero. */
typedef struct PxuiMenuRow PxuiMenuRow;
typedef struct { const PxuiMenuRow *ptr; size_t len; size_t stride; } PxuiMenuRowSpan;
struct PxuiMenuRow { uint32_t size; uint8_t enabled, danger, checked, selected, has_swatch, has_id, reserved[2]; PxuiColor swatch; int64_t id; PxuiStr text, shortcut; PxuiMenuRowSpan submenu; };
/* What a context menu has beyond its rows (`options` may be null): row_h the row height in points (0: the theme's), max_rows the rows shown
   before the column scrolls (0: all), padding above the first row and below the last (finite, not negative), the sheet's name when has_name
   is 1, focus_on_click 1 for a press to focus its row, max_h the most height of the sheet in points, its edges and padding included (finite,
   not negative; 0 no bound), the smaller of it and max_rows bounding the sheet, which scrolls past it; variant the menu_row body variant. */
typedef struct { uint32_t size; uint16_t variant, reserved; float row_h; uint32_t max_rows; float padding; uint8_t has_name, focus_on_click, reserved2[6]; PxuiStr name;
                 float max_h; uint32_t reserved3; } PxuiMenuOptions;
int32_t pxui_ui_context_menu(PxuiUi *ui, float x, float y, PxuiStr label, PxuiMenuRowSpan rows, uint8_t has_width, float width, const PxuiMenuOptions *options, uint32_t *out_tag, int64_t *out_index);
/* mark(user, row, &color) returns 1 and sets the colour of a row's accent square, else 0; off(user, row) returns 1 for a row drawn off. */
typedef uint8_t (*PxuiPickerMark)(void *user, int64_t row, PxuiColor *out);
typedef uint8_t (*PxuiPickerOff)(void *user, int64_t row);
/* row `row`'s image into out (return 1), or none (0); an invalid descriptor refuses the call. Borrowed for the call. */
typedef uint8_t (*PxuiPickerImage)(void *user, int64_t row, PxuiImage *out);
/* Widget bodies: an app builds a widget kind's nodes itself. pxui_body_register names a body of kind `kind` (PxuiBodyKind, in the field order of
   PxuiBodies; before 2.0 kinds are renumbered in place, so build against this header) `name`, for a theme's `KIND.body = NAME` (pxui_theme_parse, pxui_ui_use_theme) and writes its id to out, answering the refusal status
   (see pxui_last_error). A name has one id in its Ui (see Body names above); register a name again to change its body: that keeps the id and uses none. A
   theme carried from the Ui that parsed it to another selects the body of that name there, or the built-in body where that Ui has none; the function is called as body(user, ui, config) with the kind's record below, `ui` borrowed and valid for the call.
   Records start with `size` and grow at the end. A body builds with the ordinary calls, finds the parts its widget's behaviour reads through
   pxui_ui_part, and runs the caller's content through its slots (pxui_ui_slot); a slot is valid only during the body call that received it. */
/* Extension body kinds (no stability promise, abi/pxui.ext.txt): PXUI_BODY_INSPECTOR_ROW, FOOTER, GRIP, LINE_FIELD, VALUE_FIELD, PICKER and PICKER_FIELD
   build the Rays vocabulary; their numbers and PxuiBodies slots are fixed like every kind's, their Pxui*Config records may change between minors.
   PXUI_BODY_INSPECTOR_SECTION is also the body of pxui_ui_disclosure and stays frozen. */
enum PxuiBodyKind { PXUI_BODY_TOGGLE=0, PXUI_BODY_SLIDER=1, PXUI_BODY_PROGRESS=2, PXUI_BODY_BUTTON=3, PXUI_BODY_ICON_BUTTON=4, PXUI_BODY_CHECKBOX=5, PXUI_BODY_RADIO=6, PXUI_BODY_SPINNER=7, PXUI_BODY_BADGE=8, PXUI_BODY_LABEL=9, PXUI_BODY_FOOTER=10, PXUI_BODY_GRIP=11, PXUI_BODY_PANEL=12, PXUI_BODY_WINDOW=13, PXUI_BODY_POPUP=14, PXUI_BODY_MODAL=15, PXUI_BODY_DIALOG=16, PXUI_BODY_INSPECTOR_SECTION=17, PXUI_BODY_INSPECTOR_ROW=18, PXUI_BODY_SPLIT=19, PXUI_BODY_SCROLL_VIEW=20, PXUI_BODY_TOASTS=21, PXUI_BODY_TOOLTIP=22, PXUI_BODY_TABS=23, PXUI_BODY_SEGMENTED=24, PXUI_BODY_DROPDOWN=25, PXUI_BODY_MENU_BAR=26, PXUI_BODY_MENU_ITEM=27, PXUI_BODY_MENU_ROW=28, PXUI_BODY_SELECTION_LIST=29, PXUI_BODY_TREE=30, PXUI_BODY_TABLE_ROW=31, PXUI_BODY_TABLE_HEAD=32, PXUI_BODY_PICKER=33, PXUI_BODY_SEARCH_FIELD=34, PXUI_BODY_TEXT_FIELD=35, PXUI_BODY_LINE_FIELD=36, PXUI_BODY_VALUE_FIELD=37, PXUI_BODY_PICKER_FIELD=38, PXUI_BODY_SECTION_INDEX=39 };
/* A kind's field in PxuiBodies sits at its number (toasts, tooltip, tabs, segmented, menu_row, selection_list, table_row). */
_Static_assert(offsetof(PxuiBodies, toasts) == sizeof(uint32_t) + sizeof(uint16_t) * PXUI_BODY_TOASTS, "PxuiBodies.toasts is body kind TOASTS");
_Static_assert(offsetof(PxuiBodies, tooltip) == sizeof(uint32_t) + sizeof(uint16_t) * PXUI_BODY_TOOLTIP, "PxuiBodies.tooltip is body kind TOOLTIP");
_Static_assert(offsetof(PxuiBodies, tabs) == sizeof(uint32_t) + sizeof(uint16_t) * PXUI_BODY_TABS, "PxuiBodies.tabs is body kind TABS");
_Static_assert(offsetof(PxuiBodies, segmented) == sizeof(uint32_t) + sizeof(uint16_t) * PXUI_BODY_SEGMENTED, "PxuiBodies.segmented is body kind SEGMENTED");
_Static_assert(offsetof(PxuiBodies, menu_row) == sizeof(uint32_t) + sizeof(uint16_t) * PXUI_BODY_MENU_ROW, "PxuiBodies.menu_row is body kind MENU_ROW");
_Static_assert(offsetof(PxuiBodies, selection_list) == sizeof(uint32_t) + sizeof(uint16_t) * PXUI_BODY_SELECTION_LIST, "PxuiBodies.selection_list is body kind SELECTION_LIST");
_Static_assert(offsetof(PxuiBodies, table_row) == sizeof(uint32_t) + sizeof(uint16_t) * PXUI_BODY_TABLE_ROW, "PxuiBodies.table_row is body kind TABLE_ROW");
typedef struct { uint64_t serial; } PxuiSlot;
typedef void (*PxuiBodyFn)(void *user, PxuiUi *ui, const void *config);
typedef void (*PxuiCellBuild)(void *user, PxuiUi *ui, size_t row, size_t column);
typedef struct { uint32_t size; PxuiStateDesc state; uint8_t on; PxuiBox row; PxuiStr label; float row_h, switch_w, switch_h; const PxuiStates * track; const PxuiStates * knob; float inset; uint8_t press_row; PxuiStr detail; uint16_t variant; uint8_t has_style, reserved2[5]; PxuiLabelStyle style; uint8_t stored, clicked, reserved3[6]; } PxuiToggleConfig;
typedef struct { uint32_t size; PxuiStateDesc state; float fraction; PxuiBox track; PxuiStr label; uint16_t variant; } PxuiSliderConfig;
typedef struct { uint32_t size; uint8_t has_value; float value; PxuiBox row; PxuiStr label; uint16_t variant; } PxuiProgressConfig;
typedef struct { uint32_t size; PxuiStateDesc state; uint8_t bare; uint8_t icon; uint8_t at_end; PxuiBox line; PxuiBox row; const PxuiStates * states; PxuiStr label; uint8_t w_kind,h_kind,justify,reserved; float w,h,padding[4]; uint16_t variant; uint8_t centered,danger,reserved2[8];
 uint8_t has_label,axis,content_align,reserved3; float gap; PxuiLabelStyle text_style; PxuiLabelBox text_box;
                 /* (the whole record is passed): the caller's leading icon (glyph lead, or lead_draw/lead_user as PxuiIconButtonConfig.draw),
                    detail and accessory (empty: none), in detail_style/detail_box and accessory_style/accessory_box when has_detail_style/has_accessory_style is 1. */
                 PxuiStr lead; PxuiIconDraw lead_draw; void *lead_user; PxuiStr detail, accessory; uint8_t has_detail_style, has_accessory_style, reserved4[6];
                 PxuiLabelStyle detail_style; PxuiLabelBox detail_box; PxuiLabelStyle accessory_style; PxuiLabelBox accessory_box;
                 /* the leading drawing's word (D18; 0 for a glyph) and the icon's square (0: control-high) */ uint32_t lead_word; float icon_size;
                 /* the kit's built-in button body for this config, run with pxui_ui_slot */ PxuiSlot built_in; } PxuiButtonConfig;
typedef struct { uint32_t size; PxuiStateDesc state; PxuiBox node; PxuiStr label; PxuiStr icon; const PxuiStates *states; uint8_t w_kind,h_kind,reserved; float w,h,press_scale; PxuiIconDraw draw; void *draw_user; PxuiColor ink; uint16_t variant; const PxuiCurve *press; /* the drawing's word (D18; 0 for a glyph) */ uint32_t draw_word;
                 /* the kit's built-in icon button body for this config, run with pxui_ui_slot */ PxuiSlot built_in; } PxuiIconButtonConfig;
typedef struct { uint32_t size; PxuiStateDesc state; uint8_t on; PxuiBox row; PxuiStr label; uint16_t variant;
                 /* (the whole record is passed): the box's look per state (the caller's or the theme's; valid for the call), its side in
                    points, and passive 1 for a mark that takes no input (with empty text, no label) */
                 const PxuiStates *states; float box_size; uint8_t passive, reserved2[3]; } PxuiCheckConfig;
typedef struct { uint32_t size; PxuiBox node; PxuiStr label; uint16_t variant; } PxuiSpinnerConfig;
typedef struct { uint32_t size; PxuiBox node; PxuiStr label; PxuiStr text; uint16_t variant; } PxuiBadgeConfig;
/* grows at its end by style, the label's style as given (unset fields: the theme's); read it only when size covers it. */
typedef struct { uint32_t size; uint8_t first; PxuiBox node; PxuiStr text; PxuiLabelStyle style; uint16_t variant; PxuiLabelBox label_box; } PxuiLabelConfig;
typedef struct { uint32_t size; PxuiBox row; uint8_t has_right; PxuiStr right; PxuiHintSpan hints; uint16_t variant; } PxuiFooterConfig;
typedef struct { uint32_t size; PxuiStateDesc state; uint8_t vertical; uint8_t floating; PxuiBox grip; uint16_t variant; } PxuiGripConfig;
typedef struct { uint32_t size; PxuiBox node; PxuiStr label; PxuiSlot content; uint16_t variant;
                 /* (the whole record is passed): 0 centred, 1 bottom sheet, 2 full sheet, 3 top-anchored (draw an unknown value as 0) (pxui_ui_modal's present) */ uint64_t present; } PxuiPanelConfig;
/*  appends default_at, cancel_at (a button index or -1) and bitmasks danger, disabled (bit i: button i), before variant */
/* (the whole record is passed): details, each button's line under its label (empty: none), as many as buttons. */
typedef struct { uint32_t size; PxuiStr title; PxuiStrSpan buttons; PxuiSlot content; int64_t default_at, cancel_at; uint64_t danger, disabled; uint16_t variant; PxuiStrSpan details;
                 /* the kit's dialog buttons (each through the button body with the dialog's variant), run with pxui_ui_slot */ PxuiSlot built_in; } PxuiDialogConfig;
typedef struct { uint32_t size; uint8_t open; float gap; PxuiBox title; PxuiStr label; PxuiSlot content; uint16_t variant; } PxuiSectionConfig;
typedef struct { uint32_t size; PxuiStateDesc state; uint8_t has_pin; uint8_t pin; float label_w; PxuiBox row; PxuiStr label; uint16_t variant; } PxuiInspectorRowConfig;
typedef struct { uint32_t size; uint32_t axis; float ratio; float min_a; float min_b; PxuiBox split; PxuiSlot first; PxuiSlot second; uint16_t variant; } PxuiSplitConfig;
typedef struct { uint32_t size; PxuiBox node; PxuiSlot content; uint16_t variant; } PxuiScrollConfig;
typedef struct { uint32_t size; PxuiStateDesc state; PxuiBox node; PxuiStr text; uint16_t variant; } PxuiToastsConfig;
typedef struct { uint32_t size; float width; float height; PxuiBox tip; PxuiStrSpan lines; uint16_t variant; } PxuiTooltipConfig;
typedef struct { uint32_t size; PxuiStateDesc state; size_t index; PxuiBox node; PxuiStr text; uint16_t variant;
                 /* (the whole record is passed): the tab's detail after its text (a count or badge; empty: none) */ PxuiStr detail;
                 /* (the whole record is passed): the bar, the tab list holding the tabs; a body may restyle or resize it, as a segment body its strip */ PxuiBox bar; } PxuiTabsConfig;
/* a section index body (PXUI_BODY_SECTION_INDEX), entered on the strip `node`; letters valid for the call; active is the letter held
   or last chosen when has_active is 1 */
typedef struct { uint32_t size; PxuiStateDesc state; uint8_t has_active; size_t active; PxuiBox node; PxuiStrSpan letters; uint16_t variant; } PxuiSectionIndexConfig;
/* (the whole record is passed): has_selected 1 when selected is the choice, 0 when none is chosen (selected then 0; final);
   state.focused marks the option holding the focus. (the whole record is passed): stored is the choice the caller passed this frame (clamped to
   the options) before a press or key moved selected, when has_stored is 1; reserved4 zero. */
typedef struct { uint32_t size; PxuiStateDesc state; size_t index; size_t selected; int64_t group; PxuiBox node; PxuiStr text; const PxuiStates * states; uint8_t grow; uint16_t variant; float padding, min_segment; PxuiBox line, strip; uint8_t has_selected, reserved3[7]; size_t stored; uint8_t has_stored, reserved4[7]; } PxuiSegmentedConfig;
typedef struct { uint32_t size; PxuiStateDesc state; uint8_t open; int64_t key; PxuiBox button; PxuiStr text; uint16_t variant; } PxuiDropdownConfig;
typedef struct { uint32_t size; PxuiStateDesc state; size_t index; PxuiBox node; PxuiStr text; uint16_t variant; } PxuiMenuItemConfig;
typedef struct { uint32_t size; PxuiStateDesc state; uint8_t selected; uint8_t chevron; uint8_t danger; uint8_t has_swatch; PxuiColor swatch; uint8_t has_shortcut; int64_t index; float lead; float swatch_pad; PxuiBox row; PxuiStr text; PxuiStr shortcut; float row_h; uint16_t variant; } PxuiMenuRowConfig;
typedef struct { uint32_t size; PxuiStateDesc state; size_t index; PxuiBox row; PxuiSlot content; uint8_t drop; uint16_t variant; } PxuiRowConfig;
/* PxuiRowConfig.drop, a carried payload: none, would land before the row, after it (a reorderable list's insertion line), or over
   a row of a list with a drop look (applied by behaviour). */
enum { PXUI_ROW_DROP_NONE = 0, PXUI_ROW_DROP_BEFORE = 1, PXUI_ROW_DROP_AFTER = 2, PXUI_ROW_DROP_OVER = 3 };
/* SelectMode (PxuiSelectionListDesc.mode, PxuiTableDesc.mode): rows select one, many, toggle on a plain click, or nothing (rows activate). */
enum { PXUI_SELECT_SINGLE = 0, PXUI_SELECT_MULTI = 1, PXUI_SELECT_TOGGLE = 2, PXUI_SELECT_NONE = 3 };
typedef struct { uint32_t size; PxuiStateDesc state; uint8_t has; uint8_t open; size_t index; size_t node; size_t depth; PxuiBox row; PxuiSlot content; uint16_t variant; } PxuiTreeRowConfig;
typedef struct { uint32_t size; PxuiStateDesc state; uint8_t has_ascending; uint8_t ascending; size_t index; PxuiBox node; PxuiStr title; uint16_t variant;
                 /* the kit's built-in table heading body for this config, run with pxui_ui_slot */ PxuiSlot built_in; } PxuiTableHeadConfig;
typedef struct { uint32_t size; PxuiStateDesc state; uint8_t current; uint8_t doomed; uint8_t unavailable; uint8_t marked; uint8_t has_swatch; PxuiColor swatch; int64_t index; PxuiBox row; PxuiStr label; PxuiStr detail; PxuiStr query; uint16_t variant;
                 /* (extension set; the whole record is passed): the row's art slot, art_kind as PxuiPickRow's. */
                 uint8_t art_kind; uint8_t reserved3[7]; uint64_t art;
                 /*: has_image 1 when image is the row's image (the picker's image callback; the built-in body draws it
                    in the art slot); has_head 1 when head is the heading of the group the row starts, a box above the row that behaviour made
                    (its text, the heading role and name), which the body lays out (pxui_ui_set_node_layout) and styles. */
                 uint8_t has_image, has_head, reserved4[6]; PxuiImage image; PxuiBox head; } PxuiPickerRowConfig;
typedef struct { uint32_t size; PxuiStateDesc state; int64_t key; PxuiBox well; PxuiStr icon; PxuiSlot editor; uint16_t variant;
                 /* the field's line (the kit's row, well inside it), which a body sizes with pxui_ui_set_node_layout */ PxuiBox line;
                 /* the clear key's state (Part::CLEAR: hovered, pressed), which behaviour owns; the body styles the key for it */ PxuiStateDesc clear; } PxuiSearchFieldConfig;
typedef struct { uint32_t size; PxuiStateDesc state; PxuiBox row; PxuiStr label; PxuiStr value; uint16_t variant; } PxuiTextFieldConfig;
typedef struct { uint32_t size; PxuiStateDesc state; uint8_t empty; PxuiBox field; PxuiStr text; uint16_t variant; } PxuiLineFieldConfig;
typedef struct { uint32_t size; PxuiStateDesc state; uint8_t invalid; float w; float h; PxuiBox field; PxuiValueFieldDesc opts; PxuiStr text; uint16_t variant; } PxuiValueFieldConfig;
typedef struct { uint32_t size; PxuiStateDesc state; uint8_t empty; uint8_t slash; float prefix; float inset; PxuiBox search; PxuiStr placeholder; uint16_t variant;
                 /* the results region (the scrolling list), which a body lays out with pxui_ui_set_node_layout */ PxuiBox list; } PxuiPickerFieldConfig;
int32_t pxui_body_register(PxuiUi *ui, uint32_t kind, PxuiStr name, PxuiBodyFn body, void *user, uint16_t *out);
/* Builds a node as part `part` (PXUI_PART_*) of the widget whose body is running; PXUI_ERR_STATE outside a body call. */
int32_t pxui_ui_part(PxuiUi *ui, uint32_t part, const PxuiNode *node, PxuiBox *out);
/* The box of `part` of the widget keyed `owner` as the last build left it: 0 and the box in `out`, 1 when it was not built. */
int32_t pxui_ui_part_box(PxuiUi *ui, int64_t owner, uint32_t part, PxuiBox *out);
/* Runs a slot of the body call in progress; `out` (may be NULL) gets a search field's editor answer (1: the query is not empty). PXUI_ERR_STALE for a slot of a finished call or another Ui, PXUI_ERR_STATE inside its own content or a nested body. */
int32_t pxui_ui_slot(PxuiUi *ui, PxuiSlot slot, uint8_t *out);
enum PxuiPart { PXUI_PART_HIT=1, PXUI_PART_RAIL=2, PXUI_PART_THUMB=3, PXUI_PART_TRACK=4, PXUI_PART_BAR=5, PXUI_PART_TITLE=6, PXUI_PART_DISCLOSURE=7, PXUI_PART_CLEAR=8, PXUI_PART_EDITOR=9, PXUI_PART_DOT0=16 /* dot i: 16 + i, i < 256 */, PXUI_PART_BUTTON0=1024 /* dialog button i: 1024 + i, i < 256 */ };
/* Collection flags (selected, open, a node's children) are bytes: not 0 is set; the library writes 0 or 1 and scans none.
   A tree writes its shown rows into the start of `visible` (at least a row per node) and their number to `visible_len`. With
   has_revision 1 it keeps them while revision and the spans hold; kept rows that no longer name a node are re-derived. */
typedef struct { uint32_t size; uint8_t has_revision, reserved[3]; PxuiStr label; const PxuiTreeNode *ptr; size_t len; PxuiU8MutSpan open; size_t *selected; PxuiU32MutSpan visible; size_t *visible_len; PxuiCellBuild build; void *user; uint64_t revision; } PxuiTreeViewDesc;
int32_t pxui_ui_tree(PxuiUi *ui, const PxuiTreeViewDesc *desc, size_t *activated);
/* from inside a selection list or tree row or a table cell callback, the kit's text of that item: a list
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
/* pxui_ui_text_area is laid out by node under the current parent (checked as pxui_ui_node checks it; no grid); desc's
   x, y, width and height are ignored. The wrap width is node's Px width, else the area's width from the last layout. */
int32_t pxui_ui_patch_field(PxuiUi *ui, const PxuiBox *field, const PxuiValueFieldDesc *desc, PxuiStr text, uint8_t *out); /* the field's value and soft-range line written by patches, as its build lays them out; out is 0 when a patch cannot show it (a lead, a value too wide beside its trail): build the field's view */
/* the value field and the text area, named `name`, report into `response` (see pxui_ui_text_field). */
int32_t pxui_ui_value_field(PxuiUi *ui, const PxuiValueFieldDesc *desc, PxuiTextValid valid, PxuiSlideText slide, PxuiScrubText scrub, void *user, PxuiStr name, PxuiStr *out_text, uint8_t *out_changed, uint8_t *out_editing, PxuiResponse *response);
int32_t pxui_ui_text_area(PxuiUi *ui, const PxuiTextAreaDesc *desc, const PxuiNode *node, const PxuiAreaCallbacks *cbs, PxuiStr name, PxuiStr *out_text, uint8_t *out_changed, uint8_t *out_submitted, PxuiResponse *response);
/* ABI 2.15: optional range callbacks use absolute byte offsets, a half-open request,
   and the same output lifetime and order as the original colour and bracket callbacks. */
typedef void (*PxuiAreaColorRange)(void *, PxuiStr, int64_t, int64_t, PxuiColorSpanSpan *);
typedef void (*PxuiAreaBracketsRange)(void *, PxuiStr, int64_t, int64_t, PxuiRangeSpan *);
int32_t pxui_ui_text_area_ranges(PxuiUi *ui, const PxuiTextAreaDesc *desc, const PxuiNode *node, const PxuiAreaCallbacks *cbs, PxuiAreaColorRange colors, PxuiAreaBracketsRange brackets, PxuiStr name, PxuiStr *out_text, uint8_t *out_changed, uint8_t *out_submitted, PxuiResponse *response);
int32_t pxui_ui_load_text_edit(PxuiUi *ui, int64_t key, PxuiStr text, PxuiStr *out_text, uint64_t *caret, uint64_t *anchor);
/* pxui_ui_remember into the history of the field keyed `key` (a box key); the active editor never changes. */
int32_t pxui_ui_remember(PxuiUi *ui, int64_t key, PxuiStr text, uint64_t caret, uint64_t anchor, uint8_t typing);
/* key=0 on text width/quads/raster calls selects library-owned content identity.
   Nonzero legacy keys remain accepted; content/style/width/span validation still fences cache reuse.
   Layout storage is bounded even for standalone shapers without a frame clock. and by bytes: 64 MiB of estimated shaped size, never evicting a layout used in the current frame; text over 256 MiB is
   PXUI_ERR_ARGUMENT (SIZE_MAX) on every shaper/text/spans entry. System fonts (pxui_*_system_fonts) are mmapped, demand-paged and outside the font
   budget: at most 128 files, 256 MiB each and 1 GiB mapped in all. */
/* A text shaper of its own, for text laid out off the Ui on any thread: its own font system, no GPU atlas. */
/* A shaper is a plain library object with no thread-local state: it may be created on one thread and used on another, but one thread
   uses one shaper at a time (calls on it never overlap). It shares nothing with a PxuiUi's text service. */
PxuiShaper *pxui_shaper_create(const PxuiStr *fonts, size_t count); /* NULL for count 0 (a shaper needs a font) */
/* fork between frames, then move the owned shaper to a worker. This reserves the
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
size_t pxui_shaper_rasterize_rgba(PxuiShaper *shaper, uint64_t key, PxuiStr value, const PxuiTextStyle *style, float max_width, float scale, uint8_t color_r, uint8_t color_g, uint8_t color_b, uint8_t color_a, uint32_t *width, uint32_t *height, uint8_t *out, size_t capacity);
PxuiStr pxui_default_font(void); /* Pragmasevka, embedded: valid as long as the library */
/* The text service of a Ui: its one font system and glyph atlas, for canvas text an app lays out itself (atlases: pxui_renderer_atlas).
   pxui_text_add_font answers UINT32_MAX, pxui_ui_label_quads and pxui_ui_label_rasterize_rgba SIZE_MAX, for bad arguments and a refused call. */
uint32_t pxui_text_add_font(PxuiUi *ui, PxuiStr font);
/* The prepare queue. A worker is one thread the host owns and attaches to any number of Uis; each attached Ui
   gets a lane: a fork of its fonts on the worker (font bytes shared) and fixed buffers (128 requests in flight, 16 KiB of text per
   flush, 32 installs per frame). Attached, a Ui's virtual lists request the unseen text of rows they build past the viewport; it is
   shaped on the worker and installed before a later frame builds, without shaping or allocating on the UI thread. A Ui without a
   worker shapes inline. pxui_text_worker_create answers null when no thread can start; destroying a worker waits for at most one
   shaping call, and the Uis still attached shape inline from then on. pxui_ui_set_text_worker (between frames; null detaches,
   without waiting) keeps no pointer to the worker object. pxui_ui_label_prewarm requests value in style within max_width points
   (0: unbounded): nothing without a worker or for cached or requested text, a full lane drops it; PXUI_ERR_ARGUMENT for a null
   style, a negative or non-finite width, invalid UTF-8 or text over 16 KiB.
   Lanes: a worker serves at most 64 Uis; pxui_ui_set_text_worker answers PXUI_ERR_STATE past that (the Ui is left detached) and
   inside a frame. A preparation that panics is delivered as a failure (shaped inline) and the lane's font fork is renewed.
   Threads: pxui_text_worker_create and pxui_text_worker_destroy may run on any thread; pxui_ui_set_text_worker and
   pxui_ui_label_prewarm run on the Ui's thread, like every pxui_ui_* call. Several threads may attach their Uis to one worker at once,
   but the host must not destroy a worker concurrently with pxui_ui_set_text_worker on it: serialize destroy against attach.
   Handles: the worker pointer is not validated (as a shaper's). It must be one pxui_text_worker_create returned and not destroyed;
   using it after pxui_text_worker_destroy is undefined. A Ui attached to a destroyed worker stays valid and shapes inline. */
PxuiTextWorker *pxui_text_worker_create(void);
void pxui_text_worker_destroy(PxuiTextWorker *worker);
int32_t pxui_ui_set_text_worker(PxuiUi *ui, const PxuiTextWorker *worker);
/* The text calls on a label style, resolved against the Ui's theme as pxui_ui_resolve_label
   resolves it, in the same call (no resolve round trip), with the content's layout key. The style's digits byte is ignored. A null,
   short or invalid label record (unknown set bits, wrap, align or ellipsis numbers, feature bits) or one that does not resolve is
   refused, as is a resolved style that is invalid.
   pxui_ui_label_measure's spans may be null (the plain measurement). */
int32_t pxui_ui_label_measure(PxuiUi *ui, PxuiStr value, const PxuiLabelStyle *style, float max_width, const PxuiTextSpans *spans, PxuiTextMetrics *out);
size_t pxui_ui_label_quads(PxuiUi *ui, PxuiStr value, const PxuiLabelStyle *style, float max_width, float origin_x, float origin_y, float scale, PxuiTextGlyph *out, size_t capacity);
size_t pxui_ui_label_rasterize_rgba(PxuiUi *ui, PxuiStr value, const PxuiLabelStyle *style, float max_width, float scale, uint8_t color_r, uint8_t color_g, uint8_t color_b, uint8_t color_a, uint32_t *width, uint32_t *height, uint8_t *out, size_t capacity);
int32_t pxui_ui_label_prewarm(PxuiUi *ui, PxuiStr value, const PxuiLabelStyle *style, float max_width);
/* bit 2 on plain, unstroked RECTs selects pixel-area coverage; no layout change. */
int32_t pxui_paint_span(PxuiPaint *paint, size_t at_least, PxuiSpan *span); /* at_least 0 only commits; call it before any other pxui_paint_* call */
float pxui_paint_icon(PxuiPaint *paint, float x, float y, uint16_t font, float size, uint32_t codepoint, PxuiColor color);
float pxui_paint_caret_x(PxuiPaint *paint, uint8_t has_size, int64_t size, PxuiStr text, size_t index);
int32_t pxui_paint_selection(PxuiPaint *paint, uint8_t has_size, int64_t size, PxuiStr text, size_t a, size_t b, PxuiSpanBuild build, void *user);
void pxui_paint_ratio(PxuiPaint *paint, float x, float y, int64_t a, int64_t b);
void pxui_paint_input_region(PxuiPaint *paint, float cursor, float x, float y, float w, float h);
/* The scroll offset of the retained slot `slot` (a box handle's box_slot) as the painter reads it; 0 out of range. */
float pxui_paint_scroll_y(PxuiPaint *paint, uint64_t slot);
/* colour token `token` resolved for the painter's theme, as pxui_ui_color; transparent past the table. */
PxuiColor pxui_paint_color(PxuiPaint *paint, uint16_t token);
/* in a PxuiIconDraw call, the word pxui_ui_icon_word gave that icon button; 0 in any other paint callback. */
uint32_t pxui_paint_icon_word(PxuiPaint *paint);
/* text in a label style, resolved as a label body resolves it against the painter's theme (its label look, rows and
   colour tokens). paint_label lays out within width (0: unbounded) and draws; paint_label_chars draws one cached character
   layout at a time, digits on the advance of 0, and returns the width drawn. An invalid style draws nothing and returns 0. */
PxuiTextMetrics pxui_paint_label(PxuiPaint *paint, float x, float y, const PxuiLabelStyle *style, float width, PxuiStr text);
float pxui_paint_label_chars(PxuiPaint *paint, float x, float y, const PxuiLabelStyle *style, PxuiStr text);
/* text in a label style resolved against this painter's theme (as pxui_paint_label, capitals included): measured within
   max_width (0: unbounded) with spans (NULL: none) without drawing, or drawn with spans. PXUI_ERR_ARGUMENT for invalid arguments. */
int32_t pxui_paint_label_measure(PxuiPaint *paint, PxuiStr text, const PxuiLabelStyle *style, float max_width, const PxuiTextSpans *spans, PxuiTextMetrics *out);
int32_t pxui_paint_label_spans(PxuiPaint *paint, float x, float y, const PxuiLabelStyle *style, float width, PxuiStr text, const PxuiTextSpans *spans, PxuiTextMetrics *out);
/* an image drawn by the painter, reporting a refusal: PXUI_ERR_ARGUMENT (nothing drawn) for a null painter or descriptor, an invalid
   descriptor, an unknown fit or a non-finite rectangle; 0 when drawn. */
int32_t pxui_paint_image(PxuiPaint *paint, const PxuiImage *image, float x, float y, float width, float height, uint32_t fit, PxuiColor tint);
/* Shared GPU: the renderer records into the app's command buffers and passes and never submits, waits,
 * presents or destroys what the app created. create takes the lease in gpu->owner_token (also on failure).
 * prepare(commands, frame, completed): frame grows by at least 1 per call; completed (the last frame the app
 * knows finished) never goes back and is at least frame - 3; else PXUI_ERR_ARGUMENT and nothing is recorded.
 * draw leaves PXUI's pipeline, descriptors, viewport and scissor bound in the app's pass: draw last, or
 * rebind everything after it (OGPU: ogpu_core::backend::invalidate_state). PxuiImage: Vulkan views stay in
 * VK_IMAGE_LAYOUT_GENERAL while frames sampling them run; PXUI only samples them. destroy: the app has
 * finished every frame it prepared; a teardown failure is the Ui's last error. */
/* Writes out on success. With a valid out pointer, a complete GPU record transfers its device lease even on failure. */
int32_t pxui_renderer_create(PxuiUi *ui, const PxuiGpu *gpu, const PxuiTargetDesc *target, PxuiRenderer **out);
int32_t pxui_renderer_prepare(PxuiRenderer *renderer, const PxuiCommands *commands, uint64_t frame, uint64_t completed);
/* Prepare blur storage at startup/resize, outside frame recording. Nonzero drawable-pixel dimensions. */
int32_t pxui_renderer_resize(PxuiRenderer *renderer, uint32_t width, uint32_t height);
/* an extent above the device's maximum 2D texture dimension is PXUI_ERR_ARGUMENT, nothing allocated. pxui_renderer_create reads
   gpu and target only after their size covers the record (a shorter one is refused, NULL, with the reason in pxui_last_error). */
int32_t pxui_renderer_draw(PxuiRenderer *renderer, const PxuiPass *pass);
/* Same as pxui_renderer_draw with a nonempty scissor in target pixels. The caller must set this
   exact scissor before drawing. PXUI may skip geometry outside it. */
int32_t pxui_renderer_draw_damage(PxuiRenderer *renderer, const PxuiPass *pass, uint32_t x, uint32_t y, uint32_t width, uint32_t height);
/* ABI 2.11. Two to eight nonempty device-pixel rectangles as consecutive [x, y, width, height] records.
   The host clears these pixels before drawing. PXUI replays selected instances under each scissor
   and paints overlapping pixels once. The records are borrowed until the call returns. */
int32_t pxui_renderer_draw_damage_rects(PxuiRenderer *renderer, const PxuiPass *pass, const uint32_t *rects, uint32_t count);
/* Set before prepare. When enabled, prepare retains layer content and draw omits the promoted runs. */
int32_t pxui_renderer_exclude_promoted(PxuiRenderer *renderer, uint8_t exclude);
/* ABI 2.13. Blend the prepared layers over the canvas in the open single-sample pass. An optional
   separate opaque RGBA canvas restores base pixels. Damage requires a canvas and points to four
   target-pixel words [x, y, width, height], or NULL for the whole target. */
int32_t pxui_renderer_composite(PxuiRenderer *renderer, const PxuiPass *pass, const PxuiImage *canvas, const uint32_t *damage);
/* The draw calls the renderer has issued so far. */
/* forget on every sampling renderer before destroying/reusing an app texture.
   completed is the host's actual completed serial on the prepare timeline and must not go back.
   Refuses ERR_STATE while a draw of the image exceeds completed. No extra frame; never destroys the app image. */
int32_t pxui_renderer_forget_image(PxuiRenderer *renderer, const PxuiImage *image, uint64_t completed);
uint64_t pxui_renderer_draw_calls(PxuiRenderer *renderer);
int32_t pxui_renderer_atlas(PxuiRenderer *renderer, uint32_t kind, PxuiImage *out); /* PXUI_ATLAS_*: mask R8, colour RGBA8; second pages return ERR_STATE until prepared. The texture stays the renderer's. */
void pxui_renderer_destroy(PxuiRenderer *renderer);

/* Queries and switches on the Ui as a whole. Text results are borrowed until the next frame. */
/* Monotonic host frame time, shared by cached views . */
double pxui_ui_now(PxuiUi *ui);
/* Integer rendering density shared with Paint: host scale rounded to at least one . */
int64_t pxui_ui_density(PxuiUi *ui);
/* A widget's body variant (0: the theme's) is a field of its descriptor: PxuiKeyedDesc.variant for the keyed widgets, `variant` in the button,
   icon button, toggle, checkbox, segmented, dialog, text field, menu bar and menu extra records, PxuiLabelBox.variant for a label and the
   `variant` argument of pxui_ui_search_field. A registered body of the widget's kind reads it as its config's `variant`. */
/* the word the next pxui_ui_icon_button call (or pxui_ui_button, for its leading icon) hands its drawing (read there with pxui_paint_icon_word), so one
   callback draws many icons by id; that call takes it (the call after reads 0 unless set again). A registered icon button body that calls
   `draw` itself from its own paint callback passes no word (pxui_paint_icon_word reads 0 there). */
int32_t pxui_ui_icon_word(PxuiUi *ui, uint32_t word);
uint64_t pxui_ui_frame_index(PxuiUi *ui);
uint8_t pxui_ui_captured(PxuiUi *ui);
/* 1 while a reorderable list's row is lifted by its handle (Escape, a cancelled pointer or lost focus puts it back). */
uint8_t pxui_ui_reordering(PxuiUi *ui);
/* 1 while a text editor holds the keyboard focus (a focused list or control is not one). */
uint8_t pxui_ui_text_input_focused(PxuiUi *ui);
uint8_t pxui_ui_verifying(PxuiUi *ui);
uint64_t pxui_ui_text_undo_bytes(PxuiUi *ui);
float pxui_ui_inspector_width(PxuiUi *ui);
float pxui_ui_inspector_label_x(PxuiUi *ui);
uint32_t pxui_ui_passed_undo(PxuiUi *ui); /* 0 none, 1 undo, 2 redo */
/* Raw held state, including modifiers when a widget or popup owns focus . */
int32_t pxui_ui_key_down(PxuiUi *ui, uint32_t code, uint8_t *out);
/* raw press query with Shift/Ctrl/Alt/Meta held at the event, including same-frame release. */
/* Match all requested modifiers on one press (Shift=1, Ctrl=2, Alt=4, Meta=8). 0 requires none and 16 accepts any. */
/* a press a focused widget handled (a list's arrows, Return and Command-A; a focused control's Space and Return) is not reported. */
int32_t pxui_ui_key_pressed(PxuiUi *ui, uint32_t code, uint32_t modifiers, uint8_t *out);
/* the body size and kit row exactly; the setter keeps fractions (0.5..=1024). */
int32_t pxui_ui_font_points(PxuiUi *ui, float *out);
int32_t pxui_ui_row_points(PxuiUi *ui, float *out);
int32_t pxui_ui_set_font_points(PxuiUi *ui, float points);
int32_t pxui_ui_unfocus(PxuiUi *ui);
int32_t pxui_ui_dismiss_popup(PxuiUi *ui);
int32_t pxui_ui_request_cursor(PxuiUi *ui, uint32_t shape); /* 1 horizontal resize, 2 vertical resize, 3 text */
int32_t pxui_ui_repaint_after(PxuiUi *ui, double seconds);
int32_t pxui_ui_verify_views(PxuiUi *ui, uint8_t on);
int32_t pxui_ui_bytes(PxuiUi *ui, int64_t *out, size_t len); /* atlas, glyph lists, text undo; len must be 3 */
uint32_t pxui_ui_platform(PxuiUi *ui); /* 0 other, 1 Apple */
int32_t pxui_ui_expanded(PxuiUi *ui, PxuiStr key, uint8_t *out); /* 0 unknown, 1 collapsed, 2 expanded */
/* a key of a 0x01 byte and 16 lowercase hex digits waits on that resolved key itself; other text waits as text. */
int32_t pxui_ui_hover_delay(PxuiUi *ui, PxuiStr key, uint8_t *out);
/* the seed an authored key resolves under for a lookup (the active scope during a build, the root seed outside one). */
int32_t pxui_ui_key_seed(PxuiUi *ui, int64_t *out);

/* Boxes, patches, scroll and text state, carries, view transforms. */
int32_t pxui_ui_set_at(PxuiUi *ui, const PxuiBox *box, float x, float y);
int32_t pxui_ui_patch_look(PxuiUi *ui, const PxuiBox *box, const PxuiLookDesc *value, uint8_t *out);
int32_t pxui_ui_patch_rect(PxuiUi *ui, const PxuiBox *box, PxuiRect value, uint8_t *out);
int32_t pxui_ui_patch_mark(PxuiUi *ui, const PxuiBox *box, const PxuiMarkDesc *value, uint8_t *out);
/* The look, mark and inset of a node declared this frame, read and written back whole (the engine's node_mut). */
typedef struct { uint32_t size, reserved; PxuiLookDesc look; PxuiMarkDesc mark; PxuiEdges inset; } PxuiNodeFields;
int32_t pxui_ui_node_fields(PxuiUi *ui, const PxuiBox *box, PxuiNodeFields *out);
int32_t pxui_ui_set_node_fields(PxuiUi *ui, const PxuiBox *box, const PxuiNodeFields *fields);
/* The node a widget declared this frame as the engine holds it (written whole to out, refused when out->size is shorter), and the layout of a node the frame being built declared (axis, justify, align, wrap, size, minimum and maximum, grow, shrink, aspect, padding, margin, gaps, own alignment, offset) set from `node`'s same fields, and its transition when it has one (the node's own, which runs its changes of state instead of the theme's; U2b.4.5), the others ignored: how a widget body sizes the node it was given. */
int32_t pxui_ui_node_layout(PxuiUi *ui, const PxuiBox *box, PxuiNode *out);
int32_t pxui_ui_set_node_layout(PxuiUi *ui, const PxuiBox *box, const PxuiNode *node);
int32_t pxui_ui_local_rect(PxuiUi *ui, const PxuiBox *box, PxuiRect *out);
int32_t pxui_ui_scroll_offset(PxuiUi *ui, const PxuiBox *box, uint32_t which, float *out); /* 0 offset, 1 painted position */
int32_t pxui_ui_set_scroll_offset(PxuiUi *ui, const PxuiBox *box, float value);
/* minimum scroll to reveal a virtual list item/grid cell; out-of-range indices do nothing. */
int32_t pxui_ui_reveal(PxuiUi *ui, const PxuiBox *list, size_t index);
/* read, and with `in` set (may be NULL) request a node's touch gestures; commands bit 0 closes a swiped row,
   bit 1 ends a refresh. Touch only; mouse and trackpad are unchanged. */
int32_t pxui_ui_gesture(PxuiUi *ui, const PxuiBox *box, const PxuiGestureIn *in, uint32_t commands, PxuiGestureOut *out);
/* enable a node's row swipe with actions on both sides and a full-swipe commit, keeping its other gestures;
   call after pxui_ui_gesture each build. Extra PxuiGestureOut flags: 512 commit leading and 1024 commit trailing (this
   frame), 2048 open leading and 4096 open trailing (8 stays set for either). swipe stays the physical shift. */
int32_t pxui_ui_swipe_actions(PxuiUi *ui, const PxuiBox *box, const PxuiSwipeIn *in);
/* take a node's vertical drag-to-dismiss (a downward drag, handed over by an inner scroll view only at its top;
   touch only; call after pxui_ui_gesture each build) and read its state. */
int32_t pxui_ui_dismiss(PxuiUi *ui, const PxuiBox *box, const PxuiDismissIn *in, PxuiDismissOut *out);
int32_t pxui_ui_text_state(PxuiUi *ui, const PxuiBox *box, uint8_t *present, PxuiStr *out);
int32_t pxui_ui_set_text_state(PxuiUi *ui, const PxuiBox *box, uint8_t present, PxuiStr value);
int32_t pxui_ui_cached_label(PxuiUi *ui, const PxuiBox *box, uint64_t index, PxuiStr text, PxuiStr *out); /* borrowed from the slot */
int32_t pxui_ui_to_front(PxuiUi *ui, int64_t order, const PxuiBox *box);
/*  reuses carry/drop text buffers; returned strings survive cancellation until the next frame. */
int32_t pxui_ui_carrying(PxuiUi *ui, uint8_t *present, PxuiStr *kind, PxuiStr *value);
int32_t pxui_ui_carry(PxuiUi *ui, uint8_t has_from, const PxuiBox *from, PxuiStr kind, PxuiStr value);
/* transfer an active pointer carry to a live hit ancestor (out=1); otherwise out=0.
   Scroll ancestors follow the pointer at their edges. Release/cancellation does not click the ancestor. */
int32_t pxui_ui_transfer_carry(PxuiUi *ui, const PxuiBox *to, uint8_t *out);
/* a cancelled carry spends the frame's Escape press (pxui_ui_key_pressed no longer reports it). */
int32_t pxui_ui_cancel_carry(PxuiUi *ui);
int32_t pxui_ui_drop_target(PxuiUi *ui, const PxuiBox *box, uint32_t *tag, PxuiStr *kind, PxuiStr *value); /* 0 none, 1 hover, 2 dropped */
int32_t pxui_ui_context_at(PxuiUi *ui, const PxuiBox *box, uint8_t *present, PxuiPoint *out);
int32_t pxui_ui_hovered(PxuiUi *ui, const PxuiBox *box, uint8_t within, uint8_t *out);
int32_t pxui_ui_press_keys(PxuiUi *ui, const PxuiBox *box, const uint32_t **ptr, size_t *len); /* key codes, borrowed until the next frame */
int32_t pxui_ui_root_at(PxuiUi *ui, float x, float y, PxuiI64Span roots, uint8_t *present, int64_t *out);
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
int32_t pxui_ui_grip(PxuiUi *ui, PxuiStr label, uint32_t axis, uint8_t has_rect, PxuiRect rect, PxuiBox *out, PxuiSignal *signal);
/* The kit button and icon button, each reporting one PxuiResponse; the _plain forms are the kit button keyed by key (the theme's look,
   the kit's size, at the start of its line) and the kit icon button showing the glyph icon named name (the theme's look, the kit's
   square), with no descriptor. The _plain calls (button, icon button, toggle) are the string-only fast path for the common call: the same
   widget as the descriptor form with every option at its default, so no descriptor record is built or checked. They are not duplicates: a
   call that sets any option, a variant, contents or a drawing uses the descriptor form. */
int32_t pxui_ui_button_plain(PxuiUi *ui, PxuiStr key, PxuiStr text, PxuiResponse *out);
/* The kit button with options (PxuiButtonDesc): build (when set) replaces its text inside the box and is given the button's interaction
   state (hovered, pressed, focused, disabled, checked) this frame; build may be null for a text button. */
int32_t pxui_ui_button(PxuiUi *ui, const PxuiButtonDesc *desc, PxuiButtonBuild build, void *user, PxuiResponse *out);
int32_t pxui_ui_icon_button(PxuiUi *ui, const PxuiIconButtonDesc *desc, PxuiResponse *out);
/* the kit toggle and segmented control, plain or with options, each reporting one PxuiResponse (a click on the toggle is the flip of `on`, which the caller keeps; the segmented control also reports the choice), and a scope in which every interactive node built is disabled (disabled 1; scopes nest by OR). */
int32_t pxui_ui_toggle_plain(PxuiUi *ui, PxuiStr key, PxuiStr text, uint8_t on, PxuiResponse *out);
int32_t pxui_ui_toggle(PxuiUi *ui, const PxuiToggleDesc *desc, PxuiResponse *out);
int32_t pxui_ui_disabled(PxuiUi *ui, uint8_t disabled, PxuiBuild build, void *user);
int32_t pxui_ui_icon_button_plain(PxuiUi *ui, PxuiStr key, PxuiStr icon, PxuiStr name, PxuiResponse *out);
/* The modal dialog with the buttons' default, cancel, danger and disabled flags, the sheet's key and width, and a PxuiDialogResult. */
int32_t pxui_ui_dialog(PxuiUi *ui, const PxuiDialogDesc *desc, PxuiBuild build, void *user, PxuiDialogResult *out);
/* The segmented control: desc->has_selected 0 is no choice, and result has the choice (none until a press or key chooses) and the
   option a secondary press began on. It is a radio group: every option focusable to assistive technology, one Tab stop (the chosen option, the
   first with none); Left and Right on the focused option move the focus and the choice; the focus follows a choice that changes while an
   option holds it, and moves to the Tab stop (the strip with no option left) when its option goes. */
int32_t pxui_ui_inspector_bar(PxuiUi *ui, PxuiHintSpan hints, int64_t count);
int32_t pxui_ui_inspector_body(PxuiUi *ui, PxuiBuild build, void *user);
int32_t pxui_ui_inspector_header(PxuiUi *ui, const PxuiHeaderDesc *desc, PxuiTextValid rename, void *user, PxuiHeadOut *out);
int32_t pxui_ui_field_drag(PxuiUi *ui, const PxuiBox *box, uint8_t *present, PxuiPoint *out);
/* the keyed calls, each widget's key and name apart in a PxuiKeyedDesc. A keyed popup takes no `desc->label` (it must be
   empty). Toast `i` is keyed by `ids.ptr[i]` (`ids.len` equals `items.len`). */
/* the kit checkbox with its style; a click flips on, which the caller keeps. */
int32_t pxui_ui_checkbox(PxuiUi *ui, const PxuiCheckboxDesc *desc, PxuiResponse *out);
/* the section index strip (a focusable slider over letters, each checked once, at most 4096), named keyed->name; result: the letter chosen this frame (has_chosen 0: none moved it), never a secondary. */
int32_t pxui_ui_section_index(PxuiUi *ui, const PxuiKeyedDesc *keyed, PxuiStrSpan letters, PxuiResponse *out, PxuiSegmentedResult *result);
/* a collapsible section, its header showing and named keyed->name, built through the inspector_section body; open is the caller's value (0 or 1), build runs the content while open. A click (or Enter/Space while focused) in out is a toggle; the caller keeps the value. */
int32_t pxui_ui_disclosure(PxuiUi *ui, const PxuiKeyedDesc *keyed, uint8_t open, PxuiBuild build, void *user, PxuiResponse *out);
int32_t pxui_ui_progress(PxuiUi *ui, const PxuiKeyedDesc *keyed, uint8_t has_value, float value, PxuiBox *out);
int32_t pxui_ui_spinner(PxuiUi *ui, const PxuiKeyedDesc *keyed, PxuiBox *out);
int32_t pxui_ui_badge(PxuiUi *ui, const PxuiKeyedDesc *keyed, PxuiStr value, PxuiBox *out);
/* the one modal route with a presentation (see PxuiSheetDesc); writes out (1 while open) and the modal's root (the empty box once dismissed). Every field of sheet is checked before the build. */
int32_t pxui_ui_modal(PxuiUi *ui, const PxuiKeyedDesc *keyed, const PxuiSheetDesc *sheet, PxuiBuild build, void *user, uint8_t *out, PxuiBox *root);
int32_t pxui_ui_anchored(PxuiUi *ui, const PxuiKeyedDesc *keyed, const PxuiAnchorDesc *anchor, PxuiSizeDesc width, PxuiSizeDesc height, PxuiBuild build, void *user, PxuiBox *out);
/* the scroll view laid out by node (checked as pxui_ui_node checks it; no grid): the frame adds its scroll flags, look,
   thumbs, row scroll step and role, and insets the content one point inside the node's padding. */
int32_t pxui_ui_scroll_view(PxuiUi *ui, const PxuiKeyedDesc *keyed, const PxuiNode *node, PxuiBuild build, void *user, PxuiBox *out);
/* one result per call. pxui_ui_pick is a radio group (kind 0) or dropdown (3) of items, selected read when has_selected is 1,
   reporting the choice (none for none) and the item a secondary press began on; pxui_ui_segmented takes its options as items
   (desc->names empty). The slider and scrub fill a PxuiScrubOut: flags 1
   a held preview, 2 a commit (a release, a key or an assistive SetValue, with preview equal to it), 4 held; a slider's preview is the value
   shown. pxui_ui_panel and pxui_ui_popup also write the overlay's root (the empty box when it is dismissed). pxui_ui_split reports the grip's
   response (empty without a grip) and has_ratio 1 with the ratio under the pointer while the grip is held and on its release frame.
   pxui_ui_menu_bar sets dismissed to 1 when the menu open at the frame's start closed without a pick. A refused call (pxui_ui_modal,
   pxui_ui_popup, pxui_ui_split and pxui_ui_toasts among them) writes none of its outputs. */
int32_t pxui_ui_segmented(PxuiUi *ui, const PxuiSegmentedDesc *desc, PxuiPickItemSpan items, PxuiResponse *out, PxuiSegmentedResult *result);
int32_t pxui_ui_pick(PxuiUi *ui, const PxuiKeyedDesc *keyed, uint32_t kind, PxuiPickItemSpan items, uint8_t has_selected, size_t selected, PxuiResponse *out, PxuiSegmentedResult *result);
/* a tab bar of pick-one items (an item's detail beside its text, its id keying it), named keyed->name; selected is the tab in use when has_selected is 1 (0: none is); focus_on_click 1 makes a press on a tab focus the bar. result: the tab in use and the tab a secondary press began on. */
int32_t pxui_ui_tabs(PxuiUi *ui, const PxuiKeyedDesc *keyed, PxuiPickItemSpan items, uint8_t has_selected, size_t selected, uint8_t focus_on_click, PxuiResponse *out, PxuiSegmentedResult *result);
int32_t pxui_ui_slider(PxuiUi *ui, const PxuiKeyedDesc *keyed, float value, float min, float max, PxuiResponse *out, PxuiScrubOut *result);
int32_t pxui_ui_scrub(PxuiUi *ui, PxuiStr label, float value, float min, float max, float step, PxuiResponse *response, PxuiScrubOut *out);
int32_t pxui_ui_panel(PxuiUi *ui, const PxuiKeyedDesc *keyed, const PxuiPanel *panel, PxuiBuild build, void *user, PxuiBox *root);
int32_t pxui_ui_popup(PxuiUi *ui, const PxuiKeyedDesc *keyed, const PxuiPopupDesc *desc, PxuiBuild build, void *user, uint8_t *out, PxuiBox *root);
int32_t pxui_ui_split(PxuiUi *ui, const PxuiSplitDesc *desc, PxuiResponse *out, uint8_t *has_ratio, float *ratio);
int32_t pxui_ui_menu_bar(PxuiUi *ui, const PxuiMenuBarDesc *desc, PxuiMenuOut *out, uint8_t *dismissed);
/* Opt the current view containing only palette-independent positioned labels into instant palette reuse. */
int32_t pxui_ui_retain_palette_labels(PxuiUi *ui);
/* ABI 2.3. Opt palette-independent canvas shapes into instant palette replay. */
int32_t pxui_ui_retain_palette_paint(PxuiUi *ui);
/* Opt the current viewport-independent view into reuse across changes to the window's size. */
int32_t pxui_ui_retain_view_on_resize(PxuiUi *ui);
/* the authoring view keyed by keyed->key (keyed->name must be empty). It fills its parent, or takes its content's size on the
   main axis of a row or column whose size there is fit; build runs only when revision (a hash of what it reads) changed. */
int32_t pxui_ui_view(PxuiUi *ui, const PxuiKeyedDesc *keyed, uint64_t revision, PxuiBuild build, void *user, PxuiBox *out);
int32_t pxui_ui_toasts(PxuiUi *ui, const PxuiKeyedDesc *keyed, PxuiStrSpan items, PxuiI64Span ids, double seconds, int64_t *out_index);
int32_t pxui_ui_picker(PxuiUi *ui, const PxuiKeyedDesc *keyed, int64_t limit, uint8_t slash, uint8_t at_rest, PxuiStr query, PxuiPickerRows rows, PxuiPickerMark mark, PxuiPickerOff off, PxuiPickerImage image, void *user, PxuiStr *out_query, uint32_t *out_tag, int64_t *out_index);

/* Themes a caller builds and scopes over a subtree. set and load apply over what the theme holds, as a theme file applies over
   its base: a tokens.* key re-derives every look from the new tokens, so set tokens before the keys that restyle single looks. */
PxuiTheme *pxui_theme_create(const PxuiThemeTokens *tokens); /* tokens may be NULL for the defaults; NULL result: the tokens are invalid (not finite, a size or density at or below zero, a flag above 1, reserved bytes set) */
void pxui_theme_destroy(PxuiTheme *theme);
PxuiStr pxui_theme_last_error(const PxuiTheme *theme); /* borrowed from the theme until its next call */
int32_t pxui_ui_themed(PxuiUi *ui, const PxuiTheme *theme, PxuiBuild build, void *user);
/* Detached source. Root installation checks all named bodies in the destination Ui. Scopes use the built-in body for missing names. */
int32_t pxui_theme_parse(PxuiTheme *theme, PxuiStr source);
int32_t pxui_theme_set_color(PxuiTheme *theme, uint16_t token, PxuiColor color);
/* Controls the transition started by installing this theme (1 by default). */
int32_t pxui_theme_animate_swap(PxuiTheme *theme, uint8_t on);
int32_t pxui_theme_density(PxuiTheme *theme, float density);
int32_t pxui_theme_reduced_motion(PxuiTheme *theme, uint8_t on);


/* Accessibility bridge of an app's window (Linux: AT-SPI2 on the accessibility bus; elsewhere start returns null, the platform's bridge
   belongs to the window host). The app feeds each frame's tree and takes the actions assistive technology asked for. */
typedef struct PxuiA11y PxuiA11y;
PxuiA11y *pxui_a11y_start(PxuiStr name);
int32_t pxui_a11y_fd(PxuiA11y *a11y);
/* Publishes `tree` as the tree of `surface` (not 0, the id the frame call carries): one bridge serves one application whose windows are its surfaces. */
int32_t pxui_a11y_update(PxuiA11y *a11y, uint64_t surface, const PxuiA11yTree *tree);
/* Stops serving a closed surface. Unknown nonzero ids are ignored; 0 is refused. */
int32_t pxui_a11y_remove_surface(PxuiA11y *a11y, uint64_t surface);
/* 1: the connection is alive, 0: gone (stop it). `out` holds actions with their destination surface, borrowed until the next pump. */
int32_t pxui_a11y_pump(PxuiA11y *a11y, PxuiA11yActionSpan *out);
void pxui_a11y_stop(PxuiA11y *a11y);

int32_t pxui_shaper_measure(PxuiShaper *shaper, uint64_t key, PxuiStr value, const PxuiTextStyle *style, float max_width, const PxuiTextSpans *spans, PxuiTextMetrics *out);
/* Positive line height; rounded primary-font metrics place CSS line-box baselines . */

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
_Static_assert(sizeof(PxuiEvent) == 64 && offsetof(PxuiEvent, kind) == 4 && offsetof(PxuiEvent, code) == 16 && offsetof(PxuiEvent, x) == 24 && offsetof(PxuiEvent, y) == 32 && offsetof(PxuiEvent, value) == 40 && offsetof(PxuiEvent, text) == 48, "Event layout");
_Static_assert(sizeof(PxuiFrameIn) == 176 && offsetof(PxuiFrameIn, size) == 0 && offsetof(PxuiFrameIn, events) == 8 && offsetof(PxuiFrameIn, events.len) == 16 && offsetof(PxuiFrameIn, events.stride) == 24 && offsetof(PxuiFrameIn, width) == 32 && offsetof(PxuiFrameIn, height) == 36 && offsetof(PxuiFrameIn, scale) == 40 && offsetof(PxuiFrameIn, time) == 48 && offsetof(PxuiFrameIn, appearance) == 56 && offsetof(PxuiFrameIn, reduced_motion) == 60 && offsetof(PxuiFrameIn, environment) == 64 && offsetof(PxuiFrameIn, keyboard) == 144 && offsetof(PxuiFrameIn, surface) == 168, "FrameIn layout");
_Static_assert(sizeof(PxuiFrameOut) == 40 && offsetof(PxuiFrameOut, size) == 0 && offsetof(PxuiFrameOut, damage) == 4 && offsetof(PxuiFrameOut, has_damage) == 20 && offsetof(PxuiFrameOut, reserved) == 21 && offsetof(PxuiFrameOut, layer_count) == 24 && offsetof(PxuiFrameOut, layers) == 32, "FrameOut layout");
_Static_assert(sizeof(PxuiGpu) == 128 && offsetof(PxuiGpu, size) == 0 && offsetof(PxuiGpu, backend) == 4 && offsetof(PxuiGpu, instance) == 8 && offsetof(PxuiGpu, physical_device) == 16 && offsetof(PxuiGpu, device) == 24 && offsetof(PxuiGpu, queue) == 32 && offsetof(PxuiGpu, queue_family) == 40 && offsetof(PxuiGpu, api_version) == 44 && offsetof(PxuiGpu, features) == 48 && offsetof(PxuiGpu, get_instance_proc_addr) == 56 && offsetof(PxuiGpu, pipeline_cache) == 64 && offsetof(PxuiGpu, mtl_device) == 72 && offsetof(PxuiGpu, wl_display) == 80 && offsetof(PxuiGpu, wl_surface) == 88 && offsetof(PxuiGpu, metal_layer) == 96 && offsetof(PxuiGpu, hwnd) == 104 && offsetof(PxuiGpu, owner_token) == 112 && offsetof(PxuiGpu, release_owner) == 120, "Gpu layout");
_Static_assert(sizeof(PxuiCommands) == 24 && offsetof(PxuiCommands, size) == 0 && offsetof(PxuiCommands, vk_command_buffer) == 8 && offsetof(PxuiCommands, mtl_command_buffer) == 16, "Commands layout");
_Static_assert(sizeof(PxuiPass) == 72 && offsetof(PxuiPass, size) == 0 && offsetof(PxuiPass, vk_command_buffer) == 8 && offsetof(PxuiPass, mtl_render_encoder) == 16 && offsetof(PxuiPass, width_px) == 24 && offsetof(PxuiPass, height_px) == 28 && offsetof(PxuiPass, scale) == 32 && offsetof(PxuiPass, mtl_command_buffer) == 40 && offsetof(PxuiPass, first_layer) == 48 && offsetof(PxuiPass, layer_count) == 52 && offsetof(PxuiPass, split) == 56 && offsetof(PxuiPass, split_user) == 64, "Pass layout");
_Static_assert(sizeof(PxuiImage) == 40 && offsetof(PxuiImage, size) == 0 && offsetof(PxuiImage, vk_image_view) == 8 && offsetof(PxuiImage, mtl_texture) == 16 && offsetof(PxuiImage, width) == 24 && offsetof(PxuiImage, height) == 28 && offsetof(PxuiImage, format) == 32, "Image layout");
_Static_assert(sizeof(PxuiSpan) == 32 && offsetof(PxuiSpan, size) == 0 && offsetof(PxuiSpan, view) == 4 && offsetof(PxuiSpan, clip_radius_f16) == 6 && offsetof(PxuiSpan, clip) == 8 && offsetof(PxuiSpan, at) == 16 && offsetof(PxuiSpan, end) == 24, "Span layout");
_Static_assert(sizeof(PxuiBox) == 32 && offsetof(PxuiBox, index) == 0 && offsetof(PxuiBox, box_key) == 8 && offsetof(PxuiBox, box_slot) == 16 && offsetof(PxuiBox, generation) == 24, "PxuiBox layout");
_Static_assert(sizeof(PxuiPanel) == 56 && offsetof(PxuiPanel, size) == 0 && offsetof(PxuiPanel, x) == 4 && offsetof(PxuiPanel, y) == 8 && offsetof(PxuiPanel, width) == 12 && offsetof(PxuiPanel, height) == 16 && offsetof(PxuiPanel, padding) == 24 && offsetof(PxuiPanel, has_height) == 32 && offsetof(PxuiPanel, window) == 33 && offsetof(PxuiPanel, has_max_height) == 34 && offsetof(PxuiPanel, has_stroke) == 35 && offsetof(PxuiPanel, reserved) == 36 && offsetof(PxuiPanel, max_height) == 40 && offsetof(PxuiPanel, stroke) == 44 && offsetof(PxuiPanel, look) == 48, "PxuiPanel layout");
_Static_assert(sizeof(PxuiTargetDesc) == 32 && offsetof(PxuiTargetDesc, size) == 0 && offsetof(PxuiTargetDesc, format) == 4 && offsetof(PxuiTargetDesc, samples) == 8 && offsetof(PxuiTargetDesc, reserved) == 9 && offsetof(PxuiTargetDesc, mask_width) == 12 && offsetof(PxuiTargetDesc, mask_height) == 16 && offsetof(PxuiTargetDesc, color_width) == 20 && offsetof(PxuiTargetDesc, color_height) == 24 && offsetof(PxuiTargetDesc, reserved_tail) == 28, "PxuiTargetDesc layout");
_Static_assert(offsetof(PxuiUiDesc, abi_built) == 12 && offsetof(PxuiUiDesc, size) == 0 && offsetof(PxuiUiDesc, platform) == 4 && offsetof(PxuiUiDesc, font_size) == 8, "PxuiUiDesc layout");
_Static_assert(sizeof(PxuiInput) == 168 && offsetof(PxuiInput, size) == 0 && offsetof(PxuiInput, time) == 8 && offsetof(PxuiInput, dt) == 16 && offsetof(PxuiInput, frame) == 24 && offsetof(PxuiInput, tick) == 32 && offsetof(PxuiInput, logical_size) == 40 && offsetof(PxuiInput, pointer) == 56 && offsetof(PxuiInput, keys) == 72 && offsetof(PxuiInput, buttons) == 88 && offsetof(PxuiInput, events) == 104 && offsetof(PxuiInput, pixel_scale) == 128 && offsetof(PxuiInput, mouse_delta) == 144 && offsetof(PxuiInput, fps) == 160, "Input layout");
_Static_assert(sizeof(PxuiPlatformOut) == 88 && offsetof(PxuiPlatformOut, size) == 0 && offsetof(PxuiPlatformOut, cursor) == 4 && offsetof(PxuiPlatformOut, has_cursor) == 8 && offsetof(PxuiPlatformOut, has_ime) == 9 && offsetof(PxuiPlatformOut, has_background) == 10 && offsetof(PxuiPlatformOut, has_clipboard) == 11 && offsetof(PxuiPlatformOut, ime_rect) == 16 && offsetof(PxuiPlatformOut, ime_caret) == 48 && offsetof(PxuiPlatformOut, clipboard) == 56 && offsetof(PxuiPlatformOut, background) == 72 && offsetof(PxuiPlatformOut, paste) == 76 && offsetof(PxuiPlatformOut, reserved) == 77, "PlatformOut layout");
_Static_assert(sizeof(PxuiDamage) == 20 && offsetof(PxuiDamage, has_damage) == 0 && offsetof(PxuiDamage, reserved) == 1 && offsetof(PxuiDamage, rect) == 4, "Damage layout");
_Static_assert(sizeof(PxuiA11yValue) == 48 && offsetof(PxuiA11yValue, tag) == 0 && offsetof(PxuiA11yValue, text_start) == 4 && offsetof(PxuiA11yValue, text_end) == 8 && offsetof(PxuiA11yValue, reserved) == 12 && offsetof(PxuiA11yValue, now) == 16 && offsetof(PxuiA11yValue, min) == 24 && offsetof(PxuiA11yValue, max) == 32 && offsetof(PxuiA11yValue, step) == 40, "A11yValue layout");
_Static_assert(sizeof(PxuiA11yNode) == 136 && offsetof(PxuiA11yNode, size) == 0 && offsetof(PxuiA11yNode, parent) == 4 && offsetof(PxuiA11yNode, id) == 8 && offsetof(PxuiA11yNode, role) == 16 && offsetof(PxuiA11yNode, name_start) == 20 && offsetof(PxuiA11yNode, name_end) == 24 && offsetof(PxuiA11yNode, description_start) == 28 && offsetof(PxuiA11yNode, description_end) == 32 && offsetof(PxuiA11yNode, value) == 40 && offsetof(PxuiA11yNode, rect) == 88 && offsetof(PxuiA11yNode, states) == 104 && offsetof(PxuiA11yNode, actions) == 108 && offsetof(PxuiA11yNode, selection) == 112 && offsetof(PxuiA11yNode, child_count) == 120 && offsetof(PxuiA11yNode, first_child) == 124 && offsetof(PxuiA11yNode, next_sibling) == 128 && offsetof(PxuiA11yNode, last_child) == 132, "A11yNode layout");
_Static_assert(sizeof(PxuiA11yAction) == 40 && offsetof(PxuiA11yAction, size) == 0 && offsetof(PxuiA11yAction, action_code) == 4 && offsetof(PxuiA11yAction, id) == 8 && offsetof(PxuiA11yAction, payload) == 16 && offsetof(PxuiA11yAction, end) == 24 && offsetof(PxuiA11yAction, reserved) == 28 && offsetof(PxuiA11yAction, surface) == 32, "A11yAction layout");
_Static_assert(sizeof(PxuiA11yTree) == 88 && offsetof(PxuiA11yTree, size) == 0 && offsetof(PxuiA11yTree, reserved) == 4 && offsetof(PxuiA11yTree, nodes) == 8 && offsetof(PxuiA11yTree, nodes.len) == 16 && offsetof(PxuiA11yTree, nodes.stride) == 24 && offsetof(PxuiA11yTree, strings) == 32 && offsetof(PxuiA11yTree, revision) == 48 && offsetof(PxuiA11yTree, first_child) == 56 && offsetof(PxuiA11yTree, child_count) == 60 && offsetof(PxuiA11yTree, actions) == 64 && offsetof(PxuiA11yTree, actions.len) == 72 && offsetof(PxuiA11yTree, actions.stride) == 80, "A11yTree layout");
_Static_assert(sizeof(PxuiOutput) == 408 && offsetof(PxuiOutput, size) == 0 && offsetof(PxuiOutput, repaint_hz) == 4 && offsetof(PxuiOutput, draw_revision) == 8 && offsetof(PxuiOutput, repaint_at) == 16 && offsetof(PxuiOutput, built_nodes) == 24 && offsetof(PxuiOutput, rebuilt_views) == 32 && offsetof(PxuiOutput, arranged_nodes) == 40 && offsetof(PxuiOutput, platform) == 48 && offsetof(PxuiOutput, damage) == 136 && offsetof(PxuiOutput, a11y) == 160 && offsetof(PxuiOutput, layers) == 248 && offsetof(PxuiOutput, damage_list) == 272, "Output layout");
_Static_assert(sizeof(PxuiMotion) == 28 && offsetof(PxuiMotion, fast) == 0 && offsetof(PxuiMotion, normal) == 4 && offsetof(PxuiMotion, slow) == 8 && offsetof(PxuiMotion, ease) == 12, "PxuiMotion layout");
_Static_assert(sizeof(PxuiThemeTokens) == 136 && offsetof(PxuiThemeTokens, size) == 0 && offsetof(PxuiThemeTokens, panel) == 4 && offsetof(PxuiThemeTokens, foreground) == 8 && offsetof(PxuiThemeTokens, control) == 12 && offsetof(PxuiThemeTokens, input) == 16 && offsetof(PxuiThemeTokens, track) == 20 && offsetof(PxuiThemeTokens, accent) == 24 && offsetof(PxuiThemeTokens, ink_ground) == 28 && offsetof(PxuiThemeTokens, has_ink_ground) == 32 && offsetof(PxuiThemeTokens, reserved) == 33 && offsetof(PxuiThemeTokens, space) == 36 && offsetof(PxuiThemeTokens, radius) == 52 && offsetof(PxuiThemeTokens, display_size) == 64 && offsetof(PxuiThemeTokens, title_size) == 72 && offsetof(PxuiThemeTokens, font_size) == 80 && offsetof(PxuiThemeTokens, label_size) == 88 && offsetof(PxuiThemeTokens, motion) == 96 && offsetof(PxuiThemeTokens, reduced_motion) == 124 && offsetof(PxuiThemeTokens, dark) == 125 && offsetof(PxuiThemeTokens, reserved_tail) == 126 && offsetof(PxuiThemeTokens, density) == 128, "PxuiThemeTokens layout");
_Static_assert(sizeof(PxuiSizeDesc) == 16 && offsetof(PxuiSizeDesc, kind) == 0 && offsetof(PxuiSizeDesc, value) == 4 && offsetof(PxuiSizeDesc, relative) == 8, "PxuiSizeDesc layout");
_Static_assert(sizeof(PxuiAnchorDesc) == 40 && offsetof(PxuiAnchorDesc, size) == 0 && offsetof(PxuiAnchorDesc, has_anchor) == 4 && offsetof(PxuiAnchorDesc, rect) == 8 && offsetof(PxuiAnchorDesc, side) == 24 && offsetof(PxuiAnchorDesc, align) == 28 && offsetof(PxuiAnchorDesc, gap) == 32 && offsetof(PxuiAnchorDesc, margin) == 36 && offsetof(PxuiAnchorDesc, flip) == 5 && offsetof(PxuiAnchorDesc, reserved) == 6, "PxuiAnchorDesc layout");
_Static_assert(sizeof(PxuiFontDesc) == 24 && offsetof(PxuiFontDesc, family) == 0 && offsetof(PxuiFontDesc, weight) == 2 && offsetof(PxuiFontDesc, size) == 8 && offsetof(PxuiFontDesc, tracking) == 16 && offsetof(PxuiFontDesc, caps) == 20 && offsetof(PxuiFontDesc, reserved) == 21, "PxuiFontDesc layout");
_Static_assert(sizeof(PxuiShadowDesc) == 24 && offsetof(PxuiShadowDesc, color) == 0 && offsetof(PxuiShadowDesc, offset) == 4 && offsetof(PxuiShadowDesc, blur) == 12 && offsetof(PxuiShadowDesc, spread) == 16 && offsetof(PxuiShadowDesc, inner) == 20 && offsetof(PxuiShadowDesc, reserved) == 21, "PxuiShadowDesc layout");
_Static_assert(sizeof(PxuiRingDesc) == 12 && offsetof(PxuiRingDesc, color) == 0 && offsetof(PxuiRingDesc, width) == 4 && offsetof(PxuiRingDesc, sides) == 8, "PxuiRingDesc layout");
_Static_assert(sizeof(PxuiLookDesc) == 160 && offsetof(PxuiLookDesc, size) == 0 && offsetof(PxuiLookDesc, fill) == 4 && offsetof(PxuiLookDesc, gradient) == 8 && offsetof(PxuiLookDesc, has_gradient) == 12 && offsetof(PxuiLookDesc, angle) == 16 && offsetof(PxuiLookDesc, border) == 20 && offsetof(PxuiLookDesc, border_w) == 24 && offsetof(PxuiLookDesc, sides) == 28 && offsetof(PxuiLookDesc, radius) == 32 && offsetof(PxuiLookDesc, shadows) == 48 && offsetof(PxuiLookDesc, text) == 96 && offsetof(PxuiLookDesc, font) == 104 && offsetof(PxuiLookDesc, opacity) == 128 && offsetof(PxuiLookDesc, ring) == 132 && offsetof(PxuiLookDesc, selection) == 144 && offsetof(PxuiLookDesc, caret) == 148 && offsetof(PxuiLookDesc, dashed) == 152 && offsetof(PxuiLookDesc, reserved_tail) == 153, "PxuiLookDesc layout");
_Static_assert(sizeof(PxuiMarkDesc) == 16 && offsetof(PxuiMarkDesc, kind) == 0 && offsetof(PxuiMarkDesc, direction) == 4 && offsetof(PxuiMarkDesc, width) == 8 && offsetof(PxuiMarkDesc, rising) == 12 && offsetof(PxuiMarkDesc, reserved) == 13, "PxuiMarkDesc layout");
_Static_assert(sizeof(PxuiStateDesc) == 12 && offsetof(PxuiStateDesc, size) == 0 && offsetof(PxuiStateDesc, hovered) == 4 && offsetof(PxuiStateDesc, pressed) == 5 && offsetof(PxuiStateDesc, focused) == 6 && offsetof(PxuiStateDesc, disabled) == 7 && offsetof(PxuiStateDesc, checked) == 8 && offsetof(PxuiStateDesc, reserved) == 9, "PxuiStateDesc layout");
_Static_assert(sizeof(PxuiCurveDesc) == 164 && offsetof(PxuiCurveDesc, size) == 0 && offsetof(PxuiCurveDesc, kind) == 4 && offsetof(PxuiCurveDesc, duration) == 8 && offsetof(PxuiCurveDesc, points) == 12 && offsetof(PxuiCurveDesc, omega) == 28 && offsetof(PxuiCurveDesc, count) == 32 && offsetof(PxuiCurveDesc, positions) == 36 && offsetof(PxuiCurveDesc, values) == 100, "PxuiCurveDesc layout");
_Static_assert(sizeof(PxuiMenuDesc) == 40 && offsetof(PxuiMenuDesc, size) == 0 && offsetof(PxuiMenuDesc, reserved) == 4 && offsetof(PxuiMenuDesc, title) == 8 && offsetof(PxuiMenuDesc, items) == 24, "PxuiMenuDesc layout");
_Static_assert(sizeof(PxuiGridListDesc) == 56 && offsetof(PxuiGridListDesc, size) == 0 && offsetof(PxuiGridListDesc, reserved) == 4 && offsetof(PxuiGridListDesc, label) == 8 && offsetof(PxuiGridListDesc, count) == 24 && offsetof(PxuiGridListDesc, cell_w) == 32 && offsetof(PxuiGridListDesc, cell_h) == 36 && offsetof(PxuiGridListDesc, build) == 40 && offsetof(PxuiGridListDesc, user) == 48, "PxuiGridListDesc layout");
_Static_assert(sizeof(PxuiSplitDesc) == 72 && offsetof(PxuiSplitDesc, size) == 0 && offsetof(PxuiSplitDesc, axis) == 4 && offsetof(PxuiSplitDesc, label) == 8 && offsetof(PxuiSplitDesc, ratio) == 24 && offsetof(PxuiSplitDesc, min_a) == 28 && offsetof(PxuiSplitDesc, min_b) == 32 && offsetof(PxuiSplitDesc, reserved) == 36 && offsetof(PxuiSplitDesc, build_a) == 40 && offsetof(PxuiSplitDesc, user_a) == 48 && offsetof(PxuiSplitDesc, build_b) == 56 && offsetof(PxuiSplitDesc, user_b) == 64, "PxuiSplitDesc layout");
_Static_assert(sizeof(PxuiMenuBarDesc) == 40 && offsetof(PxuiMenuBarDesc, size) == 0 && offsetof(PxuiMenuBarDesc, variant) == 4 && offsetof(PxuiMenuBarDesc, reserved) == 6 && offsetof(PxuiMenuBarDesc, label) == 8 && offsetof(PxuiMenuBarDesc, menus) == 24, "PxuiMenuBarDesc layout");
_Static_assert(sizeof(PxuiMenuOut) == 24 && offsetof(PxuiMenuOut, size) == 0 && offsetof(PxuiMenuOut, reserved) == 4 && offsetof(PxuiMenuOut, menu) == 8 && offsetof(PxuiMenuOut, item) == 16, "PxuiMenuOut layout");
_Static_assert(sizeof(PxuiKeyframeDesc) == 360 && offsetof(PxuiKeyframeDesc, size) == 0 && offsetof(PxuiKeyframeDesc, time) == 4 && offsetof(PxuiKeyframeDesc, target) == 8 && offsetof(PxuiKeyframeDesc, reserved) == 12 && offsetof(PxuiKeyframeDesc, values) == 16 && offsetof(PxuiKeyframeDesc, look) == 32 && offsetof(PxuiKeyframeDesc, curve) == 192, "PxuiKeyframeDesc layout");
_Static_assert(sizeof(PxuiTableDesc) == 104 && offsetof(PxuiTableDesc, size) == 0 && offsetof(PxuiTableDesc, mode) == 4 && offsetof(PxuiTableDesc, has_sort) == 5 && offsetof(PxuiTableDesc, ascending) == 6 && offsetof(PxuiTableDesc, has_revision) == 7 && offsetof(PxuiTableDesc, columns) == 8 && offsetof(PxuiTableDesc, widths) == 24 && offsetof(PxuiTableDesc, sort_id) == 40 && offsetof(PxuiTableDesc, rows) == 48 && offsetof(PxuiTableDesc, row_h) == 56 && offsetof(PxuiTableDesc, head_h) == 60 && offsetof(PxuiTableDesc, flags) == 64 && offsetof(PxuiTableDesc, anchor) == 80 && offsetof(PxuiTableDesc, head_variant) == 88 && offsetof(PxuiTableDesc, open_on_click) == 90 && offsetof(PxuiTableDesc, bare) == 91 && offsetof(PxuiTableDesc, reserved2) == 92 && offsetof(PxuiTableDesc, revision) == 96, "PxuiTableDesc layout");
_Static_assert(sizeof(PxuiSignal) == 120 && offsetof(PxuiSignal, size) == 0 && offsetof(PxuiSignal, flags) == 4 && offsetof(PxuiSignal, has_subtree_press) == 8 && offsetof(PxuiSignal, button) == 9 && offsetof(PxuiSignal, has_a11y_value) == 10 && offsetof(PxuiSignal, has_a11y_select) == 11 && offsetof(PxuiSignal, reserved) == 12 && offsetof(PxuiSignal, subtree_press) == 16 && offsetof(PxuiSignal, clicks) == 24 && offsetof(PxuiSignal, drag) == 32 && offsetof(PxuiSignal, pointer) == 40 && offsetof(PxuiSignal, press_point) == 48 && offsetof(PxuiSignal, release_point) == 56 && offsetof(PxuiSignal, scroll) == 64 && offsetof(PxuiSignal, pinch) == 72 && offsetof(PxuiSignal, a11y_value) == 80 && offsetof(PxuiSignal, a11y_select) == 88 && offsetof(PxuiSignal, keys) == 96, "PxuiSignal layout");
_Static_assert(sizeof(PxuiTextStyle) == 36 && offsetof(PxuiTextStyle, size) == 0 && offsetof(PxuiTextStyle, font) == 4 && offsetof(PxuiTextStyle, weight) == 6 && offsetof(PxuiTextStyle, size_points) == 8 && offsetof(PxuiTextStyle, line_height) == 12 && offsetof(PxuiTextStyle, wrap) == 16 && offsetof(PxuiTextStyle, align) == 20 && offsetof(PxuiTextStyle, max_lines) == 24 && offsetof(PxuiTextStyle, italic) == 26 && offsetof(PxuiTextStyle, ellipsis) == 27 && offsetof(PxuiTextStyle, features) == 28 && offsetof(PxuiTextStyle, tracking) == 32, "TextStyle layout");
_Static_assert(sizeof(PxuiTextMetrics) == 20 && offsetof(PxuiTextMetrics, size) == 0 && offsetof(PxuiTextMetrics, width) == 4 && offsetof(PxuiTextMetrics, height) == 8 && offsetof(PxuiTextMetrics, baseline) == 12 && offsetof(PxuiTextMetrics, lines) == 16, "TextMetrics layout");
_Static_assert(sizeof(PxuiTextGlyph) == 40 && offsetof(PxuiTextGlyph, rect) == 0 && offsetof(PxuiTextGlyph, uv) == 16 && offsetof(PxuiTextGlyph, atlas) == 32 && offsetof(PxuiTextGlyph, reserved) == 36, "TextGlyph layout");
_Static_assert(sizeof(PxuiPopupDesc) == 48 && offsetof(PxuiPopupDesc, size) == 0 && offsetof(PxuiPopupDesc, has_stroke) == 4 && offsetof(PxuiPopupDesc, has_max_height) == 5 && offsetof(PxuiPopupDesc, dismiss_initial) == 6 && offsetof(PxuiPopupDesc, attached) == 7 && offsetof(PxuiPopupDesc, stroke) == 8 && offsetof(PxuiPopupDesc, max_height) == 12 && offsetof(PxuiPopupDesc, x) == 16 && offsetof(PxuiPopupDesc, y) == 20 && offsetof(PxuiPopupDesc, width) == 24 && offsetof(PxuiPopupDesc, height) == 28 && offsetof(PxuiPopupDesc, label) == 32, "PxuiPopupDesc layout");
_Static_assert(sizeof(PxuiHeadActionDesc) == 40 && offsetof(PxuiHeadActionDesc, size) == 0 && offsetof(PxuiHeadActionDesc, active) == 4 && offsetof(PxuiHeadActionDesc, usable) == 5 && offsetof(PxuiHeadActionDesc, reserved) == 6 && offsetof(PxuiHeadActionDesc, caption) == 8 && offsetof(PxuiHeadActionDesc, keycap) == 24, "PxuiHeadActionDesc layout");
_Static_assert(sizeof(PxuiPaintInfo) == 192 && offsetof(PxuiPaintInfo, size) == 0 && offsetof(PxuiPaintInfo, rtl) == 4 && offsetof(PxuiPaintInfo, reserved) == 5 && offsetof(PxuiPaintInfo, font_size) == 8 && offsetof(PxuiPaintInfo, row_height) == 16 && offsetof(PxuiPaintInfo, density) == 24 && offsetof(PxuiPaintInfo, clip) == 32 && offsetof(PxuiPaintInfo, clip_radius) == 48 && offsetof(PxuiPaintInfo, theme) == 56, "PxuiPaintInfo layout");
_Static_assert(sizeof(PxuiU8MutSpan) == 16 && offsetof(PxuiU8MutSpan, ptr) == 0 && offsetof(PxuiU8MutSpan, len) == 8, "PxuiU8MutSpan layout");
_Static_assert(sizeof(PxuiF32MutSpan) == 16 && offsetof(PxuiF32MutSpan, ptr) == 0 && offsetof(PxuiF32MutSpan, len) == 8, "PxuiF32MutSpan layout");
_Static_assert(sizeof(PxuiU32MutSpan) == 16 && offsetof(PxuiU32MutSpan, ptr) == 0 && offsetof(PxuiU32MutSpan, len) == 8, "PxuiU32MutSpan layout");
_Static_assert(sizeof(PxuiTreeNode) == 4 && offsetof(PxuiTreeNode, depth) == 0 && offsetof(PxuiTreeNode, children) == 2, "PxuiTreeNode layout");
_Static_assert(sizeof(PxuiTreeViewDesc) == 112 && offsetof(PxuiTreeViewDesc, size) == 0 && offsetof(PxuiTreeViewDesc, has_revision) == 4 && offsetof(PxuiTreeViewDesc, reserved) == 5 && offsetof(PxuiTreeViewDesc, label) == 8 && offsetof(PxuiTreeViewDesc, ptr) == 24 && offsetof(PxuiTreeViewDesc, open) == 40 && offsetof(PxuiTreeViewDesc, selected) == 56 && offsetof(PxuiTreeViewDesc, visible) == 64 && offsetof(PxuiTreeViewDesc, visible_len) == 80 && offsetof(PxuiTreeViewDesc, build) == 88 && offsetof(PxuiTreeViewDesc, user) == 96 && offsetof(PxuiTreeViewDesc, revision) == 104, "PxuiTreeViewDesc layout");
_Static_assert(sizeof(PxuiNodeFields) == 200 && offsetof(PxuiNodeFields, size) == 0 && offsetof(PxuiNodeFields, reserved) == 4 && offsetof(PxuiNodeFields, look) == 8 && offsetof(PxuiNodeFields, mark) == 168 && offsetof(PxuiNodeFields, inset) == 184, "PxuiNodeFields layout");
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
_Static_assert(sizeof(PxuiLayerInfo) == 72 && offsetof(PxuiLayerInfo, size) == 0 && offsetof(PxuiLayerInfo, id) == 4 && offsetof(PxuiLayerInfo, breaks) == 8 && offsetof(PxuiLayerInfo, role) == 12 && offsetof(PxuiLayerInfo, key) == 16 && offsetof(PxuiLayerInfo, instances) == 24 && offsetof(PxuiLayerInfo, dirty) == 40 && offsetof(PxuiLayerInfo, anchor) == 56, "PxuiLayerInfo layout");
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
_Static_assert(sizeof(PxuiThemeSnapshot) == 56944 && offsetof(PxuiThemeSnapshot, size) == 0 && offsetof(PxuiThemeSnapshot, part_count) == 4 && offsetof(PxuiThemeSnapshot, tokens) == 8 && offsetof(PxuiThemeSnapshot, row_h) == 144 && offsetof(PxuiThemeSnapshot, control_h) == 148 && offsetof(PxuiThemeSnapshot, ring) == 152 && offsetof(PxuiThemeSnapshot, scrollbar_w) == 164 && offsetof(PxuiThemeSnapshot, scrollbar_gap) == 168 && offsetof(PxuiThemeSnapshot, bodies) == 172 && offsetof(PxuiThemeSnapshot, parts) == 304, "PxuiThemeSnapshot layout");
_Static_assert(sizeof(PxuiToggleConfig) == 208 && offsetof(PxuiToggleConfig, size) == 0 && offsetof(PxuiToggleConfig, state) == 4 && offsetof(PxuiToggleConfig, on) == 16 && offsetof(PxuiToggleConfig, row) == 24 && offsetof(PxuiToggleConfig, label) == 56 && offsetof(PxuiToggleConfig, row_h) == 72 && offsetof(PxuiToggleConfig, switch_w) == 76 && offsetof(PxuiToggleConfig, switch_h) == 80 && offsetof(PxuiToggleConfig, track) == 88 && offsetof(PxuiToggleConfig, knob) == 96 && offsetof(PxuiToggleConfig, inset) == 104 && offsetof(PxuiToggleConfig, press_row) == 108 && offsetof(PxuiToggleConfig, detail) == 112 && offsetof(PxuiToggleConfig, variant) == 128 && offsetof(PxuiToggleConfig, has_style) == 130 && offsetof(PxuiToggleConfig, reserved2) == 131 && offsetof(PxuiToggleConfig, style) == 136 && offsetof(PxuiToggleConfig, stored) == 200 && offsetof(PxuiToggleConfig, clicked) == 201 && offsetof(PxuiToggleConfig, reserved3) == 202, "PxuiToggleConfig layout");
_Static_assert(sizeof(PxuiSliderConfig) == 80 && offsetof(PxuiSliderConfig, size) == 0 && offsetof(PxuiSliderConfig, state) == 4 && offsetof(PxuiSliderConfig, fraction) == 16 && offsetof(PxuiSliderConfig, track) == 24 && offsetof(PxuiSliderConfig, label) == 56 && offsetof(PxuiSliderConfig, variant) == 72, "PxuiSliderConfig layout");
_Static_assert(sizeof(PxuiProgressConfig) == 72 && offsetof(PxuiProgressConfig, size) == 0 && offsetof(PxuiProgressConfig, has_value) == 4 && offsetof(PxuiProgressConfig, value) == 8 && offsetof(PxuiProgressConfig, row) == 16 && offsetof(PxuiProgressConfig, label) == 48 && offsetof(PxuiProgressConfig, variant) == 64, "PxuiProgressConfig layout");
_Static_assert(sizeof(PxuiButtonConfig) == 488 && offsetof(PxuiButtonConfig, size) == 0 && offsetof(PxuiButtonConfig, state) == 4 && offsetof(PxuiButtonConfig, bare) == 16 && offsetof(PxuiButtonConfig, icon) == 17 && offsetof(PxuiButtonConfig, at_end) == 18 && offsetof(PxuiButtonConfig, line) == 24 && offsetof(PxuiButtonConfig, row) == 56 && offsetof(PxuiButtonConfig, states) == 88 && offsetof(PxuiButtonConfig, label) == 96 && offsetof(PxuiButtonConfig, w_kind) == 112 && offsetof(PxuiButtonConfig, h_kind) == 113 && offsetof(PxuiButtonConfig, justify) == 114 && offsetof(PxuiButtonConfig, reserved) == 115 && offsetof(PxuiButtonConfig, w) == 116 && offsetof(PxuiButtonConfig, h) == 120 && offsetof(PxuiButtonConfig, padding) == 124 && offsetof(PxuiButtonConfig, variant) == 140 && offsetof(PxuiButtonConfig, centered) == 142 && offsetof(PxuiButtonConfig, danger) == 143 && offsetof(PxuiButtonConfig, reserved2) == 144 && offsetof(PxuiButtonConfig, has_label) == 152 && offsetof(PxuiButtonConfig, axis) == 153 && offsetof(PxuiButtonConfig, content_align) == 154 && offsetof(PxuiButtonConfig, reserved3) == 155 && offsetof(PxuiButtonConfig, gap) == 156 && offsetof(PxuiButtonConfig, text_style) == 160 && offsetof(PxuiButtonConfig, text_box) == 224 && offsetof(PxuiButtonConfig, lead) == 240 && offsetof(PxuiButtonConfig, lead_draw) == 256 && offsetof(PxuiButtonConfig, lead_user) == 264 && offsetof(PxuiButtonConfig, detail) == 272 && offsetof(PxuiButtonConfig, accessory) == 288 && offsetof(PxuiButtonConfig, has_detail_style) == 304 && offsetof(PxuiButtonConfig, has_accessory_style) == 305 && offsetof(PxuiButtonConfig, reserved4) == 306 && offsetof(PxuiButtonConfig, detail_style) == 312 && offsetof(PxuiButtonConfig, detail_box) == 376 && offsetof(PxuiButtonConfig, accessory_style) == 392 && offsetof(PxuiButtonConfig, accessory_box) == 456 && offsetof(PxuiButtonConfig, lead_word) == 472 && offsetof(PxuiButtonConfig, icon_size) == 476 && offsetof(PxuiButtonConfig, built_in) == 480, "PxuiButtonConfig layout");
_Static_assert(sizeof(PxuiIconButtonConfig) == 152 && offsetof(PxuiIconButtonConfig, size) == 0 && offsetof(PxuiIconButtonConfig, state) == 4 && offsetof(PxuiIconButtonConfig, node) == 16 && offsetof(PxuiIconButtonConfig, label) == 48 && offsetof(PxuiIconButtonConfig, icon) == 64 && offsetof(PxuiIconButtonConfig, states) == 80 && offsetof(PxuiIconButtonConfig, w_kind) == 88 && offsetof(PxuiIconButtonConfig, h_kind) == 89 && offsetof(PxuiIconButtonConfig, reserved) == 90 && offsetof(PxuiIconButtonConfig, w) == 92 && offsetof(PxuiIconButtonConfig, h) == 96 && offsetof(PxuiIconButtonConfig, press_scale) == 100 && offsetof(PxuiIconButtonConfig, draw) == 104 && offsetof(PxuiIconButtonConfig, draw_user) == 112 && offsetof(PxuiIconButtonConfig, ink) == 120 && offsetof(PxuiIconButtonConfig, variant) == 124 && offsetof(PxuiIconButtonConfig, press) == 128 && offsetof(PxuiIconButtonConfig, draw_word) == 136 && offsetof(PxuiIconButtonConfig, built_in) == 144, "PxuiIconButtonConfig layout");
_Static_assert(sizeof(PxuiCheckConfig) == 96 && offsetof(PxuiCheckConfig, size) == 0 && offsetof(PxuiCheckConfig, state) == 4 && offsetof(PxuiCheckConfig, on) == 16 && offsetof(PxuiCheckConfig, row) == 24 && offsetof(PxuiCheckConfig, label) == 56 && offsetof(PxuiCheckConfig, variant) == 72 && offsetof(PxuiCheckConfig, states) == 80 && offsetof(PxuiCheckConfig, box_size) == 88 && offsetof(PxuiCheckConfig, passive) == 92 && offsetof(PxuiCheckConfig, reserved2) == 93, "PxuiCheckConfig layout");
_Static_assert(sizeof(PxuiSpinnerConfig) == 64 && offsetof(PxuiSpinnerConfig, size) == 0 && offsetof(PxuiSpinnerConfig, node) == 8 && offsetof(PxuiSpinnerConfig, label) == 40 && offsetof(PxuiSpinnerConfig, variant) == 56, "PxuiSpinnerConfig layout");
_Static_assert(sizeof(PxuiBadgeConfig) == 80 && offsetof(PxuiBadgeConfig, size) == 0 && offsetof(PxuiBadgeConfig, node) == 8 && offsetof(PxuiBadgeConfig, label) == 40 && offsetof(PxuiBadgeConfig, text) == 56 && offsetof(PxuiBadgeConfig, variant) == 72, "PxuiBadgeConfig layout");
_Static_assert(sizeof(PxuiLabelConfig) == 144 && offsetof(PxuiLabelConfig, size) == 0 && offsetof(PxuiLabelConfig, first) == 4 && offsetof(PxuiLabelConfig, node) == 8 && offsetof(PxuiLabelConfig, text) == 40 && offsetof(PxuiLabelConfig, style) == 56 && offsetof(PxuiLabelConfig, variant) == 120 && offsetof(PxuiLabelConfig, label_box) == 124, "PxuiLabelConfig layout");
_Static_assert(sizeof(PxuiPanelConfig) == 80 && offsetof(PxuiPanelConfig, size) == 0 && offsetof(PxuiPanelConfig, node) == 8 && offsetof(PxuiPanelConfig, label) == 40 && offsetof(PxuiPanelConfig, content) == 56 && offsetof(PxuiPanelConfig, variant) == 64 && offsetof(PxuiPanelConfig, present) == 72, "PxuiPanelConfig layout");
_Static_assert(sizeof(PxuiDialogConfig) == 112 && offsetof(PxuiDialogConfig, size) == 0 && offsetof(PxuiDialogConfig, title) == 8 && offsetof(PxuiDialogConfig, buttons) == 24 && offsetof(PxuiDialogConfig, content) == 40 && offsetof(PxuiDialogConfig, default_at) == 48 && offsetof(PxuiDialogConfig, cancel_at) == 56 && offsetof(PxuiDialogConfig, danger) == 64 && offsetof(PxuiDialogConfig, disabled) == 72 && offsetof(PxuiDialogConfig, variant) == 80 && offsetof(PxuiDialogConfig, details) == 88 && offsetof(PxuiDialogConfig, built_in) == 104, "PxuiDialogConfig layout");
_Static_assert(sizeof(PxuiSectionConfig) == 80 && offsetof(PxuiSectionConfig, size) == 0 && offsetof(PxuiSectionConfig, open) == 4 && offsetof(PxuiSectionConfig, gap) == 8 && offsetof(PxuiSectionConfig, title) == 16 && offsetof(PxuiSectionConfig, label) == 48 && offsetof(PxuiSectionConfig, content) == 64 && offsetof(PxuiSectionConfig, variant) == 72, "PxuiSectionConfig layout");
_Static_assert(sizeof(PxuiSplitConfig) == 80 && offsetof(PxuiSplitConfig, size) == 0 && offsetof(PxuiSplitConfig, axis) == 4 && offsetof(PxuiSplitConfig, ratio) == 8 && offsetof(PxuiSplitConfig, min_a) == 12 && offsetof(PxuiSplitConfig, min_b) == 16 && offsetof(PxuiSplitConfig, split) == 24 && offsetof(PxuiSplitConfig, first) == 56 && offsetof(PxuiSplitConfig, second) == 64 && offsetof(PxuiSplitConfig, variant) == 72, "PxuiSplitConfig layout");
_Static_assert(sizeof(PxuiScrollConfig) == 56 && offsetof(PxuiScrollConfig, size) == 0 && offsetof(PxuiScrollConfig, node) == 8 && offsetof(PxuiScrollConfig, content) == 40 && offsetof(PxuiScrollConfig, variant) == 48, "PxuiScrollConfig layout");
_Static_assert(sizeof(PxuiDropdownConfig) == 88 && offsetof(PxuiDropdownConfig, size) == 0 && offsetof(PxuiDropdownConfig, state) == 4 && offsetof(PxuiDropdownConfig, open) == 16 && offsetof(PxuiDropdownConfig, key) == 24 && offsetof(PxuiDropdownConfig, button) == 32 && offsetof(PxuiDropdownConfig, text) == 64 && offsetof(PxuiDropdownConfig, variant) == 80, "PxuiDropdownConfig layout");
_Static_assert(sizeof(PxuiMenuItemConfig) == 80 && offsetof(PxuiMenuItemConfig, size) == 0 && offsetof(PxuiMenuItemConfig, state) == 4 && offsetof(PxuiMenuItemConfig, index) == 16 && offsetof(PxuiMenuItemConfig, node) == 24 && offsetof(PxuiMenuItemConfig, text) == 56 && offsetof(PxuiMenuItemConfig, variant) == 72, "PxuiMenuItemConfig layout");
_Static_assert(sizeof(PxuiMenuRowConfig) == 120 && offsetof(PxuiMenuRowConfig, size) == 0 && offsetof(PxuiMenuRowConfig, state) == 4 && offsetof(PxuiMenuRowConfig, selected) == 16 && offsetof(PxuiMenuRowConfig, chevron) == 17 && offsetof(PxuiMenuRowConfig, danger) == 18 && offsetof(PxuiMenuRowConfig, has_swatch) == 19 && offsetof(PxuiMenuRowConfig, swatch) == 20 && offsetof(PxuiMenuRowConfig, has_shortcut) == 24 && offsetof(PxuiMenuRowConfig, index) == 32 && offsetof(PxuiMenuRowConfig, lead) == 40 && offsetof(PxuiMenuRowConfig, swatch_pad) == 44 && offsetof(PxuiMenuRowConfig, row) == 48 && offsetof(PxuiMenuRowConfig, text) == 80 && offsetof(PxuiMenuRowConfig, shortcut) == 96 && offsetof(PxuiMenuRowConfig, row_h) == 112 && offsetof(PxuiMenuRowConfig, variant) == 116, "PxuiMenuRowConfig layout");
_Static_assert(sizeof(PxuiRowConfig) == 72 && offsetof(PxuiRowConfig, size) == 0 && offsetof(PxuiRowConfig, state) == 4 && offsetof(PxuiRowConfig, index) == 16 && offsetof(PxuiRowConfig, row) == 24 && offsetof(PxuiRowConfig, content) == 56 && offsetof(PxuiRowConfig, drop) == 64 && offsetof(PxuiRowConfig, variant) == 66, "PxuiRowConfig layout");
_Static_assert(sizeof(PxuiTreeRowConfig) == 96 && offsetof(PxuiTreeRowConfig, size) == 0 && offsetof(PxuiTreeRowConfig, state) == 4 && offsetof(PxuiTreeRowConfig, has) == 16 && offsetof(PxuiTreeRowConfig, open) == 17 && offsetof(PxuiTreeRowConfig, index) == 24 && offsetof(PxuiTreeRowConfig, node) == 32 && offsetof(PxuiTreeRowConfig, depth) == 40 && offsetof(PxuiTreeRowConfig, row) == 48 && offsetof(PxuiTreeRowConfig, content) == 80 && offsetof(PxuiTreeRowConfig, variant) == 88, "PxuiTreeRowConfig layout");
_Static_assert(sizeof(PxuiTableHeadConfig) == 96 && offsetof(PxuiTableHeadConfig, size) == 0 && offsetof(PxuiTableHeadConfig, state) == 4 && offsetof(PxuiTableHeadConfig, has_ascending) == 16 && offsetof(PxuiTableHeadConfig, ascending) == 17 && offsetof(PxuiTableHeadConfig, index) == 24 && offsetof(PxuiTableHeadConfig, node) == 32 && offsetof(PxuiTableHeadConfig, title) == 64 && offsetof(PxuiTableHeadConfig, variant) == 80 && offsetof(PxuiTableHeadConfig, built_in) == 88, "PxuiTableHeadConfig layout");
_Static_assert(sizeof(PxuiSearchFieldConfig) == 136 && offsetof(PxuiSearchFieldConfig, size) == 0 && offsetof(PxuiSearchFieldConfig, state) == 4 && offsetof(PxuiSearchFieldConfig, key) == 16 && offsetof(PxuiSearchFieldConfig, well) == 24 && offsetof(PxuiSearchFieldConfig, icon) == 56 && offsetof(PxuiSearchFieldConfig, editor) == 72 && offsetof(PxuiSearchFieldConfig, variant) == 80 && offsetof(PxuiSearchFieldConfig, line) == 88 && offsetof(PxuiSearchFieldConfig, clear) == 120, "PxuiSearchFieldConfig layout");
_Static_assert(sizeof(PxuiTextFieldConfig) == 88 && offsetof(PxuiTextFieldConfig, size) == 0 && offsetof(PxuiTextFieldConfig, state) == 4 && offsetof(PxuiTextFieldConfig, row) == 16 && offsetof(PxuiTextFieldConfig, label) == 48 && offsetof(PxuiTextFieldConfig, value) == 64 && offsetof(PxuiTextFieldConfig, variant) == 80, "PxuiTextFieldConfig layout");
_Static_assert(sizeof(PxuiSlot) == 8 && offsetof(PxuiSlot, serial) == 0, "PxuiSlot layout");
_Static_assert(sizeof(PxuiTextSpan) == 24 && offsetof(PxuiTextSpan, start) == 0 && offsetof(PxuiTextSpan, end) == 4 && offsetof(PxuiTextSpan, weight) == 8 && offsetof(PxuiTextSpan, flags) == 10 && offsetof(PxuiTextSpan, color) == 12 && offsetof(PxuiTextSpan, background) == 16 && offsetof(PxuiTextSpan, underline) == 20, "TextSpan layout");
_Static_assert(sizeof(PxuiTextSpans) == 32 && offsetof(PxuiTextSpans, size) == 0 && offsetof(PxuiTextSpans, reserved) == 4 && offsetof(PxuiTextSpans, ptr) == 8 && offsetof(PxuiTextSpans, len) == 16 && offsetof(PxuiTextSpans, stride) == 24, "TextSpans layout");
_Static_assert(sizeof(PxuiSectionListDesc) == 32 && offsetof(PxuiSectionListDesc, size) == 0 && offsetof(PxuiSectionListDesc, overlay_scrollbar) == 4 && offsetof(PxuiSectionListDesc, measured) == 5 && offsetof(PxuiSectionListDesc, reserved) == 7 && offsetof(PxuiSectionListDesc, count) == 8 && offsetof(PxuiSectionListDesc, row_height) == 16 && offsetof(PxuiSectionListDesc, leading_height) == 20 && offsetof(PxuiSectionListDesc, header_height) == 24 && offsetof(PxuiSectionListDesc, trailing_height) == 28, "PxuiSectionListDesc layout");
_Static_assert(sizeof(PxuiEnvironment) == 80 && offsetof(PxuiEnvironment, safe_area) == 16 && offsetof(PxuiEnvironment, keyboard_frame) == 32 && offsetof(PxuiEnvironment, keyboard_from) == 48 && offsetof(PxuiEnvironment, keyboard_started) == 64 && offsetof(PxuiEnvironment, keyboard_duration) == 72 && offsetof(PxuiEnvironment, keyboard_curve) == 4 && offsetof(PxuiEnvironment, keyboard_visible) == 8, "Environment layout");
_Static_assert(sizeof(PxuiTouch) == 56 && offsetof(PxuiTouch, device) == 8 && offsetof(PxuiTouch, finger) == 16 && offsetof(PxuiTouch, x) == 24 && offsetof(PxuiTouch, y) == 32 && offsetof(PxuiTouch, time) == 40 && offsetof(PxuiTouch, pressure) == 48 && offsetof(PxuiTouch, phase) == 4, "Touch layout");
_Static_assert(sizeof(PxuiKeyboard) == 24 && offsetof(PxuiKeyboard, frame) == 8 && offsetof(PxuiKeyboard, flags) == 4, "Keyboard layout");
_Static_assert(sizeof(PxuiTextInputOptions) == 16 && offsetof(PxuiTextInputOptions, size) == 0 && offsetof(PxuiTextInputOptions, autocorrect) == 4 && offsetof(PxuiTextInputOptions, capitalization) == 8 && offsetof(PxuiTextInputOptions, action) == 12, "TextInputOptions layout");
_Static_assert(sizeof(PxuiScrubOut) == 48 && offsetof(PxuiScrubOut, size) == 0 && offsetof(PxuiScrubOut, flags) == 4 && offsetof(PxuiScrubOut, node) == 8 && offsetof(PxuiScrubOut, preview) == 40 && offsetof(PxuiScrubOut, commit) == 44, "PxuiScrubOut layout");
_Static_assert(sizeof(PxuiGestureIn) == 16 && offsetof(PxuiGestureIn, size) == 0 && offsetof(PxuiGestureIn, kinds) == 4 && offsetof(PxuiGestureIn, reveal) == 8 && offsetof(PxuiGestureIn, threshold) == 12, "PxuiGestureIn layout");
_Static_assert(sizeof(PxuiGestureOut) == 20 && offsetof(PxuiGestureOut, size) == 0 && offsetof(PxuiGestureOut, flags) == 4 && offsetof(PxuiGestureOut, swipe) == 8 && offsetof(PxuiGestureOut, back) == 12 && offsetof(PxuiGestureOut, pull) == 16, "PxuiGestureOut layout");
_Static_assert(sizeof(PxuiSwipeIn) == 16 && offsetof(PxuiSwipeIn, size) == 0 && offsetof(PxuiSwipeIn, leading) == 4 && offsetof(PxuiSwipeIn, trailing) == 8 && offsetof(PxuiSwipeIn, full) == 12, "PxuiSwipeIn layout");
_Static_assert(sizeof(PxuiDismissIn) == 12 && offsetof(PxuiDismissIn, size) == 0 && offsetof(PxuiDismissIn, fraction) == 4 && offsetof(PxuiDismissIn, velocity) == 8, "PxuiDismissIn layout");
_Static_assert(sizeof(PxuiDismissOut) == 12 && offsetof(PxuiDismissOut, size) == 0 && offsetof(PxuiDismissOut, flags) == 4 && offsetof(PxuiDismissOut, offset) == 8, "PxuiDismissOut layout");
_Static_assert(sizeof(PxuiButtonDesc) == 432 && offsetof(PxuiButtonDesc, size) == 0 && offsetof(PxuiButtonDesc, enabled) == 4 && offsetof(PxuiButtonDesc, selected) == 5 && offsetof(PxuiButtonDesc, danger) == 6 && offsetof(PxuiButtonDesc, justify) == 7 && offsetof(PxuiButtonDesc, w_kind) == 8 && offsetof(PxuiButtonDesc, h_kind) == 9 && offsetof(PxuiButtonDesc, has_name) == 10 && offsetof(PxuiButtonDesc, reserved) == 11 && offsetof(PxuiButtonDesc, w) == 12 && offsetof(PxuiButtonDesc, h) == 16 && offsetof(PxuiButtonDesc, padding) == 20 && offsetof(PxuiButtonDesc, states) == 40 && offsetof(PxuiButtonDesc, key) == 48 && offsetof(PxuiButtonDesc, text) == 64 && offsetof(PxuiButtonDesc, name) == 80 && offsetof(PxuiButtonDesc, centered) == 96 && offsetof(PxuiButtonDesc, reserved2) == 97 && offsetof(PxuiButtonDesc, variant) == 98 && offsetof(PxuiButtonDesc, reserved5) == 100 && offsetof(PxuiButtonDesc, has_label) == 104 && offsetof(PxuiButtonDesc, axis) == 105 && offsetof(PxuiButtonDesc, content_align) == 106 && offsetof(PxuiButtonDesc, focus_on_click) == 107 && offsetof(PxuiButtonDesc, gap) == 108 && offsetof(PxuiButtonDesc, label) == 112 && offsetof(PxuiButtonDesc, label_box) == 176 && offsetof(PxuiButtonDesc, lead) == 192 && offsetof(PxuiButtonDesc, lead_draw) == 208 && offsetof(PxuiButtonDesc, lead_user) == 216 && offsetof(PxuiButtonDesc, detail) == 224 && offsetof(PxuiButtonDesc, accessory) == 240 && offsetof(PxuiButtonDesc, has_detail_label) == 256 && offsetof(PxuiButtonDesc, has_accessory_label) == 257 && offsetof(PxuiButtonDesc, reserved3) == 258 && offsetof(PxuiButtonDesc, detail_label) == 264 && offsetof(PxuiButtonDesc, detail_box) == 328 && offsetof(PxuiButtonDesc, accessory_label) == 344 && offsetof(PxuiButtonDesc, accessory_box) == 408, "PxuiButtonDesc layout");
_Static_assert(sizeof(PxuiLabelStyle) == 64 && offsetof(PxuiLabelStyle, size) == 0 && offsetof(PxuiLabelStyle, set) == 4 && offsetof(PxuiLabelStyle, font) == 8 && offsetof(PxuiLabelStyle, weight) == 10 && offsetof(PxuiLabelStyle, color) == 12 && offsetof(PxuiLabelStyle, max_lines) == 14 && offsetof(PxuiLabelStyle, caps) == 16 && offsetof(PxuiLabelStyle, digits) == 17 && offsetof(PxuiLabelStyle, ellipsis) == 18 && offsetof(PxuiLabelStyle, reserved) == 19 && offsetof(PxuiLabelStyle, size_points) == 20 && offsetof(PxuiLabelStyle, tracking) == 24 && offsetof(PxuiLabelStyle, line_height) == 28 && offsetof(PxuiLabelStyle, pad) == 32 && offsetof(PxuiLabelStyle, wrap) == 36 && offsetof(PxuiLabelStyle, align) == 40 && offsetof(PxuiLabelStyle, features) == 44 && offsetof(PxuiLabelStyle, width) == 48, "PxuiLabelStyle layout");
_Static_assert(sizeof(PxuiIconButtonDesc) == 112 && offsetof(PxuiIconButtonDesc, size) == 0 && offsetof(PxuiIconButtonDesc, enabled) == 4 && offsetof(PxuiIconButtonDesc, w_kind) == 5 && offsetof(PxuiIconButtonDesc, h_kind) == 6 && offsetof(PxuiIconButtonDesc, reserved) == 7 && offsetof(PxuiIconButtonDesc, w) == 8 && offsetof(PxuiIconButtonDesc, h) == 12 && offsetof(PxuiIconButtonDesc, press_scale) == 16 && offsetof(PxuiIconButtonDesc, states) == 24 && offsetof(PxuiIconButtonDesc, key) == 32 && offsetof(PxuiIconButtonDesc, name) == 48 && offsetof(PxuiIconButtonDesc, icon) == 64 && offsetof(PxuiIconButtonDesc, draw) == 80 && offsetof(PxuiIconButtonDesc, draw_user) == 88 && offsetof(PxuiIconButtonDesc, press) == 96 && offsetof(PxuiIconButtonDesc, checkable) == 104 && offsetof(PxuiIconButtonDesc, checked) == 105 && offsetof(PxuiIconButtonDesc, variant) == 106 && offsetof(PxuiIconButtonDesc, reserved2) == 108, "PxuiIconButtonDesc layout");
_Static_assert(sizeof(PxuiResponse) == 40 && offsetof(PxuiResponse, size) == 0 && offsetof(PxuiResponse, flags) == 4 && offsetof(PxuiResponse, node) == 8, "PxuiResponse layout");
_Static_assert(sizeof(PxuiToggleDesc) == 160 && offsetof(PxuiToggleDesc, size) == 0 && offsetof(PxuiToggleDesc, enabled) == 4 && offsetof(PxuiToggleDesc, on) == 5 && offsetof(PxuiToggleDesc, press_row) == 6 && offsetof(PxuiToggleDesc, reserved) == 7 && offsetof(PxuiToggleDesc, row_h) == 8 && offsetof(PxuiToggleDesc, switch_w) == 12 && offsetof(PxuiToggleDesc, switch_h) == 16 && offsetof(PxuiToggleDesc, inset) == 20 && offsetof(PxuiToggleDesc, track) == 24 && offsetof(PxuiToggleDesc, knob) == 32 && offsetof(PxuiToggleDesc, key) == 40 && offsetof(PxuiToggleDesc, text) == 56 && offsetof(PxuiToggleDesc, detail) == 72 && offsetof(PxuiToggleDesc, has_style) == 88 && offsetof(PxuiToggleDesc, reserved2) == 89 && offsetof(PxuiToggleDesc, variant) == 90 && offsetof(PxuiToggleDesc, reserved3) == 92 && offsetof(PxuiToggleDesc, style) == 96, "PxuiToggleDesc layout");
_Static_assert(sizeof(PxuiSegmentedDesc) == 112 && offsetof(PxuiSegmentedDesc, size) == 0 && offsetof(PxuiSegmentedDesc, enabled) == 4 && offsetof(PxuiSegmentedDesc, grow) == 5 && offsetof(PxuiSegmentedDesc, reserved) == 6 && offsetof(PxuiSegmentedDesc, h) == 8 && offsetof(PxuiSegmentedDesc, states) == 16 && offsetof(PxuiSegmentedDesc, key) == 24 && offsetof(PxuiSegmentedDesc, name) == 40 && offsetof(PxuiSegmentedDesc, selected) == 56 && offsetof(PxuiSegmentedDesc, w) == 64 && offsetof(PxuiSegmentedDesc, padding) == 68 && offsetof(PxuiSegmentedDesc, min_segment) == 72 && offsetof(PxuiSegmentedDesc, variant) == 76 && offsetof(PxuiSegmentedDesc, reserved2) == 78 && offsetof(PxuiSegmentedDesc, names) == 80 && offsetof(PxuiSegmentedDesc, has_selected) == 96 && offsetof(PxuiSegmentedDesc, reserved3) == 97 && offsetof(PxuiSegmentedDesc, focus_on_click) == 104 && offsetof(PxuiSegmentedDesc, reserved4) == 105, "PxuiSegmentedDesc layout");
_Static_assert(sizeof(PxuiDialogDesc) == 112 && offsetof(PxuiDialogDesc, size) == 0 && offsetof(PxuiDialogDesc, width) == 4 && offsetof(PxuiDialogDesc, key) == 8 && offsetof(PxuiDialogDesc, title) == 24 && offsetof(PxuiDialogDesc, buttons) == 40 && offsetof(PxuiDialogDesc, default_at) == 56 && offsetof(PxuiDialogDesc, cancel_at) == 64 && offsetof(PxuiDialogDesc, danger) == 72 && offsetof(PxuiDialogDesc, disabled) == 80 && offsetof(PxuiDialogDesc, details) == 88 && offsetof(PxuiDialogDesc, variant) == 104 && offsetof(PxuiDialogDesc, reserved_variant) == 106, "PxuiDialogDesc layout");
_Static_assert(sizeof(PxuiDialogResult) == 16 && offsetof(PxuiDialogResult, size) == 0 && offsetof(PxuiDialogResult, has_chosen) == 4 && offsetof(PxuiDialogResult, dismissed) == 5 && offsetof(PxuiDialogResult, reserved) == 6 && offsetof(PxuiDialogResult, chosen) == 8, "PxuiDialogResult layout");
_Static_assert(sizeof(PxuiCurve) == 140 && offsetof(PxuiCurve, kind) == 0 && offsetof(PxuiCurve, data) == 4, "PxuiCurve layout");
_Static_assert(sizeof(PxuiLabelBox) == 16 && offsetof(PxuiLabelBox, size) == 0 && offsetof(PxuiLabelBox, set) == 4 && offsetof(PxuiLabelBox, height) == 8 && offsetof(PxuiLabelBox, role) == 12 && offsetof(PxuiLabelBox, whole_pixels) == 13 && offsetof(PxuiLabelBox, variant) == 14, "PxuiLabelBox layout");
_Static_assert(sizeof(PxuiUiDesc) == 16 && offsetof(PxuiUiDesc, size) == 0 && offsetof(PxuiUiDesc, platform) == 4 && offsetof(PxuiUiDesc, font_size) == 8 && offsetof(PxuiUiDesc, abi_built) == 12, "PxuiUiDescV1_60 layout");
_Static_assert(sizeof(PxuiTextFieldDesc) == 64 && offsetof(PxuiTextFieldDesc, size) == 0 && offsetof(PxuiTextFieldDesc, autocorrect) == 4 && offsetof(PxuiTextFieldDesc, capitalization) == 5 && offsetof(PxuiTextFieldDesc, action) == 6 && offsetof(PxuiTextFieldDesc, reserved) == 7 && offsetof(PxuiTextFieldDesc, key) == 8 && offsetof(PxuiTextFieldDesc, name) == 24 && offsetof(PxuiTextFieldDesc, value) == 40 && offsetof(PxuiTextFieldDesc, variant) == 56 && offsetof(PxuiTextFieldDesc, reserved_variant) == 58, "PxuiTextFieldDesc layout");
_Static_assert(sizeof(PxuiSegmentedResult) == 24 && offsetof(PxuiSegmentedResult, size) == 0 && offsetof(PxuiSegmentedResult, has_chosen) == 4 && offsetof(PxuiSegmentedResult, has_secondary) == 5 && offsetof(PxuiSegmentedResult, reserved) == 6 && offsetof(PxuiSegmentedResult, chosen) == 8 && offsetof(PxuiSegmentedResult, secondary) == 16, "PxuiSegmentedResult layout");
_Static_assert(sizeof(PxuiKeyedDesc) == 40 && offsetof(PxuiKeyedDesc, size) == 0 && offsetof(PxuiKeyedDesc, variant) == 4 && offsetof(PxuiKeyedDesc, reserved) == 6 && offsetof(PxuiKeyedDesc, key) == 8 && offsetof(PxuiKeyedDesc, name) == 24, "PxuiKeyedDesc layout");
_Static_assert(sizeof(PxuiPickItem) == 64 && offsetof(PxuiPickItem, has_id) == 0 && offsetof(PxuiPickItem, reserved) == 1 && offsetof(PxuiPickItem, id) == 8 && offsetof(PxuiPickItem, text) == 16 && offsetof(PxuiPickItem, name) == 32 && offsetof(PxuiPickItem, detail) == 48, "PxuiPickItem layout");
_Static_assert(sizeof(PxuiSectionIndexConfig) == 88 && offsetof(PxuiSectionIndexConfig, size) == 0 && offsetof(PxuiSectionIndexConfig, state) == 4 && offsetof(PxuiSectionIndexConfig, has_active) == 16 && offsetof(PxuiSectionIndexConfig, active) == 24 && offsetof(PxuiSectionIndexConfig, node) == 32 && offsetof(PxuiSectionIndexConfig, letters) == 64 && offsetof(PxuiSectionIndexConfig, variant) == 80, "PxuiSectionIndexConfig layout");
_Static_assert(sizeof(PxuiCheckboxDesc) == 64 && offsetof(PxuiCheckboxDesc, size) == 0 && offsetof(PxuiCheckboxDesc, disabled) == 4 && offsetof(PxuiCheckboxDesc, on) == 5 && offsetof(PxuiCheckboxDesc, passive) == 6 && offsetof(PxuiCheckboxDesc, reserved) == 7 && offsetof(PxuiCheckboxDesc, box_size) == 8 && offsetof(PxuiCheckboxDesc, look) == 16 && offsetof(PxuiCheckboxDesc, key) == 24 && offsetof(PxuiCheckboxDesc, text) == 40 && offsetof(PxuiCheckboxDesc, variant) == 56 && offsetof(PxuiCheckboxDesc, reserved_variant) == 58, "PxuiCheckboxDesc layout");
_Static_assert(sizeof(PxuiSheetDesc) == 28 && offsetof(PxuiSheetDesc, size) == 0 && offsetof(PxuiSheetDesc, has_width) == 4 && offsetof(PxuiSheetDesc, present) == 5 && offsetof(PxuiSheetDesc, reserved) == 6 && offsetof(PxuiSheetDesc, width) == 8 && offsetof(PxuiSheetDesc, dismiss_fraction) == 12 && offsetof(PxuiSheetDesc, dismiss_velocity) == 16, "PxuiSheetDesc layout");
_Static_assert(sizeof(PxuiPickRow) == 64 && offsetof(PxuiPickRow, size) == 0 && offsetof(PxuiPickRow, art_kind) == 4 && offsetof(PxuiPickRow, reserved) == 5 && offsetof(PxuiPickRow, art) == 8 && offsetof(PxuiPickRow, label) == 16 && offsetof(PxuiPickRow, detail) == 32 && offsetof(PxuiPickRow, heading) == 48, "PxuiPickRow layout");
_Static_assert(sizeof(PxuiCellState) == 48 && offsetof(PxuiCellState, size) == 0 && offsetof(PxuiCellState, selected) == 4 && offsetof(PxuiCellState, enabled) == 5 && offsetof(PxuiCellState, focused) == 6 && offsetof(PxuiCellState, held) == 7 && offsetof(PxuiCellState, handle) == 8 && offsetof(PxuiCellState, y) == 40 && offsetof(PxuiCellState, reserved) == 44, "PxuiCellState layout");
_Static_assert(sizeof(PxuiListResult) == 128 && offsetof(PxuiListResult, size) == 0 && offsetof(PxuiListResult, reserved) == 4 && offsetof(PxuiListResult, handle) == 8 && offsetof(PxuiListResult, row) == 40 && offsetof(PxuiListResult, activated) == 72 && offsetof(PxuiListResult, selected) == 80 && offsetof(PxuiListResult, secondary) == 88 && offsetof(PxuiListResult, anchor) == 96, "PxuiListResult layout");
_Static_assert(sizeof(PxuiSelectionListDesc) == 232 && offsetof(PxuiSelectionListDesc, size) == 0 && offsetof(PxuiSelectionListDesc, mode) == 4 && offsetof(PxuiSelectionListDesc, fit) == 5 && offsetof(PxuiSelectionListDesc, open_on_click) == 6 && offsetof(PxuiSelectionListDesc, overlay_scrollbar) == 7 && offsetof(PxuiSelectionListDesc, measured) == 8 && offsetof(PxuiSelectionListDesc, bare) == 11 && offsetof(PxuiSelectionListDesc, has_revision) == 12 && offsetof(PxuiSelectionListDesc, reserved) == 13 && offsetof(PxuiSelectionListDesc, count) == 16 && offsetof(PxuiSelectionListDesc, row_height) == 24 && offsetof(PxuiSelectionListDesc, leading_height) == 28 && offsetof(PxuiSelectionListDesc, header_height) == 32 && offsetof(PxuiSelectionListDesc, trailing_height) == 36 && offsetof(PxuiSelectionListDesc, rows) == 40 && offsetof(PxuiSelectionListDesc, anchor) == 56 && offsetof(PxuiSelectionListDesc, revision) == 224, "PxuiSelectionListDesc layout");
_Static_assert(sizeof(PxuiColumn) == 40 && offsetof(PxuiColumn, size) == 0 && offsetof(PxuiColumn, sortable) == 4 && offsetof(PxuiColumn, grip) == 5 && offsetof(PxuiColumn, has_id) == 6 && offsetof(PxuiColumn, reserved) == 7 && offsetof(PxuiColumn, title) == 8 && offsetof(PxuiColumn, id) == 24 && offsetof(PxuiColumn, share) == 32 && offsetof(PxuiColumn, w) == 36, "PxuiColumn layout");
_Static_assert(sizeof(PxuiColumnWidth) == 16 && offsetof(PxuiColumnWidth, id) == 0 && offsetof(PxuiColumnWidth, w) == 8 && offsetof(PxuiColumnWidth, reserved) == 12, "PxuiColumnWidth layout");
_Static_assert(sizeof(PxuiColumnWidthSpan) == 16 && offsetof(PxuiColumnWidthSpan, ptr) == 0 && offsetof(PxuiColumnWidthSpan, len) == 8, "PxuiColumnWidthSpan layout");
_Static_assert(sizeof(PxuiTableHeadDesc) == 56 && offsetof(PxuiTableHeadDesc, size) == 0 && offsetof(PxuiTableHeadDesc, has_sort) == 4 && offsetof(PxuiTableHeadDesc, ascending) == 5 && offsetof(PxuiTableHeadDesc, reserved) == 6 && offsetof(PxuiTableHeadDesc, columns) == 8 && offsetof(PxuiTableHeadDesc, widths) == 24 && offsetof(PxuiTableHeadDesc, sort_id) == 40 && offsetof(PxuiTableHeadDesc, h) == 48 && offsetof(PxuiTableHeadDesc, reserved2) == 52, "PxuiTableHeadDesc layout");
_Static_assert(sizeof(PxuiHeadResult) == 56 && offsetof(PxuiHeadResult, size) == 0 && offsetof(PxuiHeadResult, sorted) == 4 && offsetof(PxuiHeadResult, resized) == 5 && offsetof(PxuiHeadResult, has_sort) == 6 && offsetof(PxuiHeadResult, ascending) == 7 && offsetof(PxuiHeadResult, sort_id) == 8 && offsetof(PxuiHeadResult, response) == 16, "PxuiHeadResult layout");
_Static_assert(sizeof(PxuiTableResult) == 192 && offsetof(PxuiTableResult, size) == 0 && offsetof(PxuiTableResult, reserved) == 4 && offsetof(PxuiTableResult, rows) == 8 && offsetof(PxuiTableResult, head) == 136, "PxuiTableResult layout");
_Static_assert(sizeof(PxuiBodies) == 132 && offsetof(PxuiBodies, size) == 0 && offsetof(PxuiBodies, toggle) == 4 && offsetof(PxuiBodies, section_index) == 82 && offsetof(PxuiBodies, reserved) == 84, "PxuiBodies layout");
_Static_assert(sizeof(PxuiToastsConfig) == 72 && offsetof(PxuiToastsConfig, size) == 0 && offsetof(PxuiToastsConfig, state) == 4 && offsetof(PxuiToastsConfig, node) == 16 && offsetof(PxuiToastsConfig, text) == 48 && offsetof(PxuiToastsConfig, variant) == 64, "PxuiToastsConfig layout");
_Static_assert(sizeof(PxuiTooltipConfig) == 72 && offsetof(PxuiTooltipConfig, size) == 0 && offsetof(PxuiTooltipConfig, width) == 4 && offsetof(PxuiTooltipConfig, height) == 8 && offsetof(PxuiTooltipConfig, tip) == 16 && offsetof(PxuiTooltipConfig, lines) == 48 && offsetof(PxuiTooltipConfig, variant) == 64, "PxuiTooltipConfig layout");
_Static_assert(sizeof(PxuiTabsConfig) == 128 && offsetof(PxuiTabsConfig, size) == 0 && offsetof(PxuiTabsConfig, state) == 4 && offsetof(PxuiTabsConfig, index) == 16 && offsetof(PxuiTabsConfig, node) == 24 && offsetof(PxuiTabsConfig, text) == 56 && offsetof(PxuiTabsConfig, variant) == 72 && offsetof(PxuiTabsConfig, detail) == 80 && offsetof(PxuiTabsConfig, bar) == 96, "PxuiTabsConfig layout");
_Static_assert(sizeof(PxuiSegmentedConfig) == 200 && offsetof(PxuiSegmentedConfig, size) == 0 && offsetof(PxuiSegmentedConfig, state) == 4 && offsetof(PxuiSegmentedConfig, index) == 16 && offsetof(PxuiSegmentedConfig, selected) == 24 && offsetof(PxuiSegmentedConfig, group) == 32 && offsetof(PxuiSegmentedConfig, node) == 40 && offsetof(PxuiSegmentedConfig, text) == 72 && offsetof(PxuiSegmentedConfig, states) == 88 && offsetof(PxuiSegmentedConfig, grow) == 96 && offsetof(PxuiSegmentedConfig, variant) == 98 && offsetof(PxuiSegmentedConfig, padding) == 100 && offsetof(PxuiSegmentedConfig, min_segment) == 104 && offsetof(PxuiSegmentedConfig, line) == 112 && offsetof(PxuiSegmentedConfig, strip) == 144 && offsetof(PxuiSegmentedConfig, has_selected) == 176 && offsetof(PxuiSegmentedConfig, reserved3) == 177 && offsetof(PxuiSegmentedConfig, stored) == 184 && offsetof(PxuiSegmentedConfig, has_stored) == 192 && offsetof(PxuiSegmentedConfig, reserved4) == 193, "PxuiSegmentedConfig layout");
_Static_assert(sizeof(PxuiMenuRow) == 80 && offsetof(PxuiMenuRow, size) == 0 && offsetof(PxuiMenuRow, enabled) == 4 && offsetof(PxuiMenuRow, danger) == 5 && offsetof(PxuiMenuRow, checked) == 6 && offsetof(PxuiMenuRow, selected) == 7 && offsetof(PxuiMenuRow, has_swatch) == 8 && offsetof(PxuiMenuRow, has_id) == 9 && offsetof(PxuiMenuRow, reserved) == 10 && offsetof(PxuiMenuRow, swatch) == 12 && offsetof(PxuiMenuRow, id) == 16 && offsetof(PxuiMenuRow, text) == 24 && offsetof(PxuiMenuRow, shortcut) == 40 && offsetof(PxuiMenuRow, submenu) == 56, "PxuiMenuRow layout");
_Static_assert(sizeof(PxuiMenuOptions) == 56 && offsetof(PxuiMenuOptions, size) == 0 && offsetof(PxuiMenuOptions, variant) == 4 && offsetof(PxuiMenuOptions, reserved) == 6 && offsetof(PxuiMenuOptions, row_h) == 8 && offsetof(PxuiMenuOptions, max_rows) == 12 && offsetof(PxuiMenuOptions, padding) == 16 && offsetof(PxuiMenuOptions, has_name) == 20 && offsetof(PxuiMenuOptions, focus_on_click) == 21 && offsetof(PxuiMenuOptions, reserved2) == 22 && offsetof(PxuiMenuOptions, name) == 32 && offsetof(PxuiMenuOptions, max_h) == 48 && offsetof(PxuiMenuOptions, reserved3) == 52, "PxuiMenuOptions layout");
_Static_assert(sizeof(PxuiFrameReasons) == 264 && offsetof(PxuiFrameReasons, requested) == 0 && offsetof(PxuiFrameReasons, interaction) == 8 && offsetof(PxuiFrameReasons, transition) == 16 && offsetof(PxuiFrameReasons, finite_keys) == 24 && offsetof(PxuiFrameReasons, active_loop) == 32 && offsetof(PxuiFrameReasons, idle_loop) == 40 && offsetof(PxuiFrameReasons, stepped) == 48 && offsetof(PxuiFrameReasons, step_state) == 56 && offsetof(PxuiFrameReasons, step_build) == 64 && offsetof(PxuiFrameReasons, step_interaction) == 72 && offsetof(PxuiFrameReasons, step_deadline) == 80 && offsetof(PxuiFrameReasons, transition_key) == 88 && offsetof(PxuiFrameReasons, transition_curve) == 96 && offsetof(PxuiFrameReasons, reserved) == 260, "PxuiFrameReasons layout");
_Static_assert(sizeof(PxuiDamageList) == 136 && offsetof(PxuiDamageList, count) == 0 && offsetof(PxuiDamageList, full) == 1 && offsetof(PxuiDamageList, reserved) == 2 && offsetof(PxuiDamageList, rects) == 8, "PxuiDamageList layout");
_Static_assert(sizeof(PxuiPromotedKey) == 40 && offsetof(PxuiPromotedKey, time) == 0 && offsetof(PxuiPromotedKey, rect) == 4 && offsetof(PxuiPromotedKey, opacity) == 20 && offsetof(PxuiPromotedKey, curve) == 24, "PxuiPromotedKey layout");
_Static_assert(sizeof(PxuiPromoted) == 432 && offsetof(PxuiPromoted, layer) == 0 && offsetof(PxuiPromoted, instance) == 4 && offsetof(PxuiPromoted, view) == 8 && offsetof(PxuiPromoted, count) == 10 && offsetof(PxuiPromoted, autoreverse) == 11 && offsetof(PxuiPromoted, hz) == 12 && offsetof(PxuiPromoted, reserved) == 14 && offsetof(PxuiPromoted, content_revision) == 16 && offsetof(PxuiPromoted, start) == 24 && offsetof(PxuiPromoted, offset) == 32 && offsetof(PxuiPromoted, color) == 36 && offsetof(PxuiPromoted, swept) == 40 && offsetof(PxuiPromoted, clip) == 56 && offsetof(PxuiPromoted, keys) == 72, "PxuiPromoted layout");
_Static_assert(sizeof(PxuiInputSnapshot) == 120 && offsetof(PxuiInputSnapshot, size) == 0 && offsetof(PxuiInputSnapshot, keys) == 8 && offsetof(PxuiInputSnapshot, buttons) == 24 && offsetof(PxuiInputSnapshot, pointer) == 40 && offsetof(PxuiInputSnapshot, mouse_delta) == 56 && offsetof(PxuiInputSnapshot, pixel_scale) == 72 && offsetof(PxuiInputSnapshot, dt) == 88 && offsetof(PxuiInputSnapshot, frame) == 96 && offsetof(PxuiInputSnapshot, tick) == 104 && offsetof(PxuiInputSnapshot, fps) == 112, "PxuiInputSnapshot layout");
_Static_assert(sizeof(PxuiDiagnostics) == 160 && offsetof(PxuiDiagnostics, size) == 0 && offsetof(PxuiDiagnostics, timing) == 4 && offsetof(PxuiDiagnostics, has_text_layouts) == 5 && offsetof(PxuiDiagnostics, has_cpu) == 6 && offsetof(PxuiDiagnostics, reserved) == 7 && offsetof(PxuiDiagnostics, rebuilt_prefix) == 8 && offsetof(PxuiDiagnostics, calls) == 24 && offsetof(PxuiDiagnostics, text_layouts) == 32 && offsetof(PxuiDiagnostics, rebuilt) == 40 && offsetof(PxuiDiagnostics, phase_nanos) == 48 && offsetof(PxuiDiagnostics, phase_cpu_nanos) == 104, "PxuiDiagnostics layout");
_Static_assert(sizeof(PxuiFooterConfig) == 88 && offsetof(PxuiFooterConfig, size) == 0 && offsetof(PxuiFooterConfig, row) == 8 && offsetof(PxuiFooterConfig, has_right) == 40 && offsetof(PxuiFooterConfig, right) == 48 && offsetof(PxuiFooterConfig, hints) == 64 && offsetof(PxuiFooterConfig, variant) == 80, "PxuiFooterConfig layout");
_Static_assert(sizeof(PxuiFooterDesc) == 40 && offsetof(PxuiFooterDesc, size) == 0 && offsetof(PxuiFooterDesc, has_right) == 4 && offsetof(PxuiFooterDesc, reserved) == 5 && offsetof(PxuiFooterDesc, right) == 8 && offsetof(PxuiFooterDesc, ptr) == 24, "PxuiFooterDesc layout");
_Static_assert(sizeof(PxuiGripConfig) == 64 && offsetof(PxuiGripConfig, size) == 0 && offsetof(PxuiGripConfig, state) == 4 && offsetof(PxuiGripConfig, vertical) == 16 && offsetof(PxuiGripConfig, floating) == 17 && offsetof(PxuiGripConfig, grip) == 24 && offsetof(PxuiGripConfig, variant) == 56, "PxuiGripConfig layout");
_Static_assert(sizeof(PxuiHeadOut) == 32 && offsetof(PxuiHeadOut, size) == 0 && offsetof(PxuiHeadOut, has_chosen) == 4 && offsetof(PxuiHeadOut, reset_pressed) == 5 && offsetof(PxuiHeadOut, reserved) == 6 && offsetof(PxuiHeadOut, renamed) == 8 && offsetof(PxuiHeadOut, chosen) == 24, "PxuiHeadOut layout");
_Static_assert(sizeof(PxuiHeaderDesc) == 144 && offsetof(PxuiHeaderDesc, size) == 0 && offsetof(PxuiHeaderDesc, has_kind) == 4 && offsetof(PxuiHeaderDesc, has_badge) == 5 && offsetof(PxuiHeaderDesc, has_index) == 6 && offsetof(PxuiHeaderDesc, has_reset) == 7 && offsetof(PxuiHeaderDesc, reset_enabled) == 8 && offsetof(PxuiHeaderDesc, reserved) == 9 && offsetof(PxuiHeaderDesc, key) == 16 && offsetof(PxuiHeaderDesc, kind) == 32 && offsetof(PxuiHeaderDesc, badge) == 48 && offsetof(PxuiHeaderDesc, index) == 64 && offsetof(PxuiHeaderDesc, reset) == 80 && offsetof(PxuiHeaderDesc, title) == 96 && offsetof(PxuiHeaderDesc, detail) == 112 && offsetof(PxuiHeaderDesc, actions) == 128, "PxuiHeaderDesc layout");
_Static_assert(sizeof(PxuiHintDesc) == 40 && offsetof(PxuiHintDesc, size) == 0 && offsetof(PxuiHintDesc, reserved) == 4 && offsetof(PxuiHintDesc, key) == 8 && offsetof(PxuiHintDesc, value) == 24, "PxuiHintDesc layout");
_Static_assert(sizeof(PxuiInspectorRowConfig) == 80 && offsetof(PxuiInspectorRowConfig, size) == 0 && offsetof(PxuiInspectorRowConfig, state) == 4 && offsetof(PxuiInspectorRowConfig, has_pin) == 16 && offsetof(PxuiInspectorRowConfig, pin) == 17 && offsetof(PxuiInspectorRowConfig, label_w) == 20 && offsetof(PxuiInspectorRowConfig, row) == 24 && offsetof(PxuiInspectorRowConfig, label) == 56 && offsetof(PxuiInspectorRowConfig, variant) == 72, "PxuiInspectorRowConfig layout");
_Static_assert(sizeof(PxuiInspectorRowDesc) == 96 && offsetof(PxuiInspectorRowDesc, size) == 0 && offsetof(PxuiInspectorRowDesc, has_width) == 4 && offsetof(PxuiInspectorRowDesc, has_pin) == 5 && offsetof(PxuiInspectorRowDesc, pin) == 6 && offsetof(PxuiInspectorRowDesc, reserved) == 7 && offsetof(PxuiInspectorRowDesc, key) == 8 && offsetof(PxuiInspectorRowDesc, label) == 24 && offsetof(PxuiInspectorRowDesc, width) == 40 && offsetof(PxuiInspectorRowDesc, reserved_tail) == 44 && offsetof(PxuiInspectorRowDesc, row) == 48 && offsetof(PxuiInspectorRowDesc, control_x) == 80 && offsetof(PxuiInspectorRowDesc, control_y) == 84 && offsetof(PxuiInspectorRowDesc, control_w) == 88, "PxuiInspectorRowDesc layout");
_Static_assert(sizeof(PxuiInspectorSectionDesc) == 64 && offsetof(PxuiInspectorSectionDesc, size) == 0 && offsetof(PxuiInspectorSectionDesc, expanded) == 4 && offsetof(PxuiInspectorSectionDesc, has_set) == 5 && offsetof(PxuiInspectorSectionDesc, set_expanded) == 6 && offsetof(PxuiInspectorSectionDesc, reserved) == 7 && offsetof(PxuiInspectorSectionDesc, key) == 8 && offsetof(PxuiInspectorSectionDesc, label) == 24 && offsetof(PxuiInspectorSectionDesc, build) == 40 && offsetof(PxuiInspectorSectionDesc, user) == 48 && offsetof(PxuiInspectorSectionDesc, out_open) == 56, "PxuiInspectorSectionDesc layout");
_Static_assert(sizeof(PxuiInspectorTextDesc) == 40 && offsetof(PxuiInspectorTextDesc, size) == 0 && offsetof(PxuiInspectorTextDesc, reserved) == 4 && offsetof(PxuiInspectorTextDesc, key) == 8 && offsetof(PxuiInspectorTextDesc, text) == 24, "PxuiInspectorTextDesc layout");
_Static_assert(sizeof(PxuiInspectorValueDesc) == 64 && offsetof(PxuiInspectorValueDesc, size) == 0 && offsetof(PxuiInspectorValueDesc, has_width) == 4 && offsetof(PxuiInspectorValueDesc, reserved) == 5 && offsetof(PxuiInspectorValueDesc, key) == 8 && offsetof(PxuiInspectorValueDesc, label) == 24 && offsetof(PxuiInspectorValueDesc, value) == 40 && offsetof(PxuiInspectorValueDesc, width) == 56, "PxuiInspectorValueDesc layout");
_Static_assert(sizeof(PxuiLineFieldConfig) == 80 && offsetof(PxuiLineFieldConfig, size) == 0 && offsetof(PxuiLineFieldConfig, state) == 4 && offsetof(PxuiLineFieldConfig, empty) == 16 && offsetof(PxuiLineFieldConfig, field) == 24 && offsetof(PxuiLineFieldConfig, text) == 56 && offsetof(PxuiLineFieldConfig, variant) == 72, "PxuiLineFieldConfig layout");
_Static_assert(sizeof(PxuiPickerFieldConfig) == 120 && offsetof(PxuiPickerFieldConfig, size) == 0 && offsetof(PxuiPickerFieldConfig, state) == 4 && offsetof(PxuiPickerFieldConfig, empty) == 16 && offsetof(PxuiPickerFieldConfig, slash) == 17 && offsetof(PxuiPickerFieldConfig, prefix) == 20 && offsetof(PxuiPickerFieldConfig, inset) == 24 && offsetof(PxuiPickerFieldConfig, search) == 32 && offsetof(PxuiPickerFieldConfig, placeholder) == 64 && offsetof(PxuiPickerFieldConfig, variant) == 80 && offsetof(PxuiPickerFieldConfig, list) == 88, "PxuiPickerFieldConfig layout");
_Static_assert(sizeof(PxuiPickerRowConfig) == 224 && offsetof(PxuiPickerRowConfig, size) == 0 && offsetof(PxuiPickerRowConfig, state) == 4 && offsetof(PxuiPickerRowConfig, current) == 16 && offsetof(PxuiPickerRowConfig, doomed) == 17 && offsetof(PxuiPickerRowConfig, unavailable) == 18 && offsetof(PxuiPickerRowConfig, marked) == 19 && offsetof(PxuiPickerRowConfig, has_swatch) == 20 && offsetof(PxuiPickerRowConfig, swatch) == 21 && offsetof(PxuiPickerRowConfig, index) == 32 && offsetof(PxuiPickerRowConfig, row) == 40 && offsetof(PxuiPickerRowConfig, label) == 72 && offsetof(PxuiPickerRowConfig, detail) == 88 && offsetof(PxuiPickerRowConfig, query) == 104 && offsetof(PxuiPickerRowConfig, variant) == 120 && offsetof(PxuiPickerRowConfig, art_kind) == 122 && offsetof(PxuiPickerRowConfig, reserved3) == 123 && offsetof(PxuiPickerRowConfig, art) == 136 && offsetof(PxuiPickerRowConfig, has_image) == 144 && offsetof(PxuiPickerRowConfig, has_head) == 145 && offsetof(PxuiPickerRowConfig, reserved4) == 146 && offsetof(PxuiPickerRowConfig, image) == 152 && offsetof(PxuiPickerRowConfig, head) == 192, "PxuiPickerRowConfig layout");
_Static_assert(sizeof(PxuiValueFieldConfig) == 240 && offsetof(PxuiValueFieldConfig, size) == 0 && offsetof(PxuiValueFieldConfig, state) == 4 && offsetof(PxuiValueFieldConfig, invalid) == 16 && offsetof(PxuiValueFieldConfig, w) == 20 && offsetof(PxuiValueFieldConfig, h) == 24 && offsetof(PxuiValueFieldConfig, field) == 32 && offsetof(PxuiValueFieldConfig, opts) == 64 && offsetof(PxuiValueFieldConfig, text) == 216 && offsetof(PxuiValueFieldConfig, variant) == 232, "PxuiValueFieldConfig layout");
_Static_assert(sizeof(PxuiValueFieldDesc) == 152 && offsetof(PxuiValueFieldDesc, size) == 0 && offsetof(PxuiValueFieldDesc, flags) == 4 && offsetof(PxuiValueFieldDesc, font_size) == 8 && offsetof(PxuiValueFieldDesc, fraction) == 16 && offsetof(PxuiValueFieldDesc, tracking) == 20 && offsetof(PxuiValueFieldDesc, line) == 24 && offsetof(PxuiValueFieldDesc, lead_color) == 28 && offsetof(PxuiValueFieldDesc, trail_color) == 32 && offsetof(PxuiValueFieldDesc, reserved) == 36 && offsetof(PxuiValueFieldDesc, x) == 40 && offsetof(PxuiValueFieldDesc, y) == 44 && offsetof(PxuiValueFieldDesc, width) == 48 && offsetof(PxuiValueFieldDesc, height) == 52 && offsetof(PxuiValueFieldDesc, display) == 56 && offsetof(PxuiValueFieldDesc, placeholder) == 72 && offsetof(PxuiValueFieldDesc, lead) == 88 && offsetof(PxuiValueFieldDesc, trail) == 104 && offsetof(PxuiValueFieldDesc, label) == 120 && offsetof(PxuiValueFieldDesc, value) == 136, "PxuiValueFieldDesc layout");
/* abi-layout end */

#endif
