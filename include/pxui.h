#ifndef PXUI_H
#define PXUI_H

#include <stddef.h>
#include <stdint.h>

typedef struct PxuiUi PxuiUi;
typedef struct PxuiRenderer PxuiRenderer;
typedef struct PxuiText PxuiText;
typedef struct PxuiPaint PxuiPaint;
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
typedef struct { PxuiColor panel,foreground,control,input,track,accent,ink_ground; uint8_t has_ink_ground,reserved[3]; float space[4],radius[3]; int64_t display_size,title_size,font_size,label_size; PxuiMotion motion; uint8_t reduced_motion,reserved_tail[3]; float density; } PxuiThemeTokens;
typedef struct { uint32_t size,cursor; uint8_t has_cursor,has_ime,has_background,has_clipboard; int64_t ime_rect[4],ime_caret; PxuiStr clipboard; PxuiColor background; uint8_t paste,reserved[7]; } PxuiPlatformOut;
typedef struct { uint8_t has_damage,reserved[3]; PxuiRect rect; } PxuiDamage;
typedef struct { uint32_t tag,text_start,text_end,reserved; double now,min,max,step; } PxuiA11yValue;
typedef struct { uint64_t id; uint32_t parent,role,name_start,name_end,description_start,description_end; PxuiA11yValue value; PxuiRect rect; uint32_t states,actions,selection[2],child_count,first_child,next_sibling,last_child; } PxuiA11yNode;
typedef struct { uint64_t id; uint32_t action_code,reserved; double payload; uint32_t end,reserved_tail; } PxuiA11yAction;
typedef struct { const PxuiA11yNode *ptr; size_t len; } PxuiA11yNodeSpan;
typedef struct { const PxuiA11yAction *ptr; size_t len; } PxuiA11yActionSpan;
typedef struct { PxuiA11yNodeSpan nodes; PxuiStr strings; uint64_t revision; uint32_t first_child,child_count; PxuiA11yActionSpan actions; } PxuiA11yTree;
typedef struct { uint32_t size,reserved; uint64_t draw_revision; double repaint_at; uint64_t built_nodes,rebuilt_views,arranged_nodes; PxuiPlatformOut platform; PxuiDamage damage; PxuiA11yTree a11y; } PxuiOutput;
typedef struct { uint32_t size,flags; PxuiStr source; } PxuiThemeSource;
typedef struct { float l,t,r,b; } PxuiEdges;
typedef struct { uint32_t kind; float value; float (*relative)(float); } PxuiSizeDesc;
typedef struct { uint32_t kind; float value; } PxuiTrackDesc;
typedef struct { const PxuiTrackDesc *ptr; size_t len; } PxuiTrackSpan;
typedef struct { uint8_t has_grid,reserved[7]; PxuiTrackSpan cols,rows; } PxuiGridDesc;
typedef struct { uint16_t col,row,col_span,row_span; } PxuiCellDesc;
typedef struct { uint8_t has_anchor,reserved[3]; PxuiRect rect; uint32_t side,align; float gap,margin; uint8_t flip,reserved_tail[3]; } PxuiAnchorDesc;
typedef struct { uint16_t family,weight; int64_t size; float tracking; uint8_t caps,reserved[3]; } PxuiFontDesc;
typedef struct { PxuiColor color; float offset[2],blur,spread; uint8_t inner,reserved[3]; } PxuiShadowDesc;
typedef struct { PxuiColor color; float width; uint32_t sides; } PxuiRingDesc;
typedef struct { PxuiColor fill,gradient; uint8_t has_gradient,reserved[3]; float angle; PxuiColor border; float border_w; uint32_t sides; float radius[4]; PxuiShadowDesc shadows[2]; PxuiColor text; PxuiFontDesc font; float opacity; PxuiRingDesc ring; PxuiColor selection,caret; uint8_t dashed,reserved_tail[3]; } PxuiLookDesc;
typedef struct { uint32_t size,reserved; PxuiThemeTokens tokens; PxuiLookDesc value,hint,field,footer; } PxuiThemeSnapshot;
typedef struct { uint32_t kind,direction; float width; uint8_t rising,reserved[3]; } PxuiMarkDesc;
typedef struct { uint8_t has_bar,whole,reserved[2]; PxuiColor color; float radius,width,gap,margin; } PxuiBarDesc;
typedef struct { uint8_t hovered,pressed,focused,disabled,checked,reserved[3]; } PxuiStateDesc;
typedef struct { uint32_t kind; float duration,points[4],omega; } PxuiCurveDesc;
typedef struct {
    uint32_t size; int64_t flags; PxuiSizeDesc w,h;
    float min_w,min_h,max_w,max_h,grow,shrink,aspect; uint32_t axis; PxuiEdges padding,margin; float gap,row_gap;
    uint32_t justify,align,align_self; uint8_t wrap,reserved[3]; PxuiGridDesc grid; PxuiCellDesc cell;
    int16_t z; uint16_t reserved_z; uint32_t role,a11y; PxuiAnchorDesc anchor; uint32_t has_at; float at[2]; float scroll_step;
    PxuiLookDesc look; PxuiEdges inset; PxuiMarkDesc mark; PxuiBarDesc bar; PxuiStateDesc state; PxuiCurveDesc transition;
} PxuiNodeDesc;
enum PxuiSizeKind { PXUI_SIZE_PX=0, PXUI_SIZE_PCT=1, PXUI_SIZE_REL=2, PXUI_SIZE_GROW=3, PXUI_SIZE_FIT=4 };
enum PxuiTrackKind { PXUI_TRACK_PX=0, PXUI_TRACK_FR=1, PXUI_TRACK_FIT=2 };
enum PxuiAxis { PXUI_AXIS_ROW=0, PXUI_AXIS_COLUMN=1, PXUI_AXIS_STACK=2 };
enum PxuiJustify { PXUI_JUSTIFY_START=0, PXUI_JUSTIFY_CENTER=1, PXUI_JUSTIFY_END=2, PXUI_JUSTIFY_SPACE_BETWEEN=3, PXUI_JUSTIFY_SPACE_AROUND=4, PXUI_JUSTIFY_SPACE_EVENLY=5 };
enum PxuiAlign { PXUI_ALIGN_START=0, PXUI_ALIGN_CENTER=1, PXUI_ALIGN_END=2, PXUI_ALIGN_STRETCH=3, PXUI_ALIGN_BASELINE=4, PXUI_ALIGN_SELF_NONE=UINT32_MAX };
enum PxuiAnchorSide { PXUI_SIDE_BELOW=0, PXUI_SIDE_ABOVE=1, PXUI_SIDE_RIGHT=2, PXUI_SIDE_LEFT=3 };
enum PxuiCurveKind { PXUI_CURVE_NONE=0, PXUI_CURVE_BEZIER=1, PXUI_CURVE_SPRING=2 };
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
typedef struct { uint32_t size; void *vk_command_buffer, *mtl_render_encoder; uint32_t width_px,height_px; float scale; void *mtl_command_buffer; } PxuiPass;
typedef struct { uint32_t size; void *vk_image_view, *mtl_texture; uint32_t width,height,format; } PxuiImage;
typedef struct { uint32_t size, format; uint8_t samples; uint8_t reserved[3]; uint32_t mask_width,mask_height,color_width,color_height,reserved_tail; } PxuiTargetDesc;
typedef struct { uint32_t size; uint16_t font,weight; float size_points,line_height; uint32_t wrap,align; uint16_t max_lines; uint8_t italic,ellipsis; uint32_t reserved; float tracking; } PxuiTextStyle;
typedef struct { float width,height,baseline; uint32_t lines; } PxuiTextMetrics;
typedef struct { float rect[4],uv[4]; uint32_t atlas,reserved; } PxuiTextGlyph;
typedef struct {
    uint32_t size;
    const PxuiEvent *events;
    size_t event_count;
    float width, height, scale;
    double time;
} PxuiFrameIn;
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
#define PXUI_ERR_PANIC (-4)    /* a panic was caught in the library; the Ui fails every call until its next frame call */
enum PxuiTargetFormat { PXUI_TARGET_RGBA8=1, PXUI_TARGET_BGRA8=2 };
typedef struct {
    uint32_t size;
    float x, y, width, height;
    int64_t padding;
    uint8_t has_height, window;
} PxuiPanel;
typedef struct { uint64_t index; int64_t box_key; uint64_t box_slot; uint32_t generation; } PxuiBox;
typedef struct { uint16_t depth; uint8_t children; } PxuiTreeNode;
typedef struct { uint32_t size; uint8_t has_stroke,has_max_height,dismiss_initial,attached; PxuiColor stroke; float max_height,x,y,width,height; PxuiStr label; } PxuiPopupDesc;
typedef struct { PxuiStr caption,keycap; uint8_t active,usable,reserved[6]; } PxuiHeadActionDesc;
typedef struct { const PxuiHeadActionDesc *ptr; size_t len; } PxuiHeadActionSpan;
typedef struct { uint32_t size; uint8_t has_kind,has_badge,has_index,has_reset,reset_enabled,reserved[3]; PxuiStr key,kind,badge,index,reset,title,detail; PxuiHeadActionSpan actions; } PxuiHeaderDesc;
typedef struct { PxuiStr renamed; int64_t chosen; uint8_t has_chosen,reset_pressed,reserved[6]; } PxuiHeadOut;
typedef struct { float rect[4]; int16_t clip[4]; uint8_t color[4], color2[4]; uint32_t aux[2]; uint16_t radii[4]; uint16_t width; uint8_t kind, flags; uint16_t view, clip_radius; uint32_t extra[2]; } PxuiInstance; /* 64 bytes, frozen for major version 1 */
/* A window into a layer's instance storage: the caller stores 64-byte instances at at (stamping clip, clip_radius_f16 and view) and
   advances it; pxui_paint_span commits what was written (at is where it stopped) and hands out the next window. */
typedef struct { uint32_t size; uint16_t view,clip_radius_f16; int16_t clip[4]; PxuiInstance *at,*end; } PxuiSpan;
typedef struct { uint32_t size; uint8_t rtl,reserved[3]; int64_t font_size,row_height,density; PxuiRect clip; float clip_radius; PxuiThemeTokens theme; } PxuiPaintInfo;
typedef PxuiRect (*PxuiHit)(void *user, PxuiRect rect);
typedef void (*PxuiRelease)(void *user);
/* A box declaration: axis is 0 row, 1 column, 2 stack. The hit callback and its release belong to the box's slot once the call is made. */
typedef struct { uint32_t size, axis; int64_t flags; PxuiSizeDesc w,h; float max_h,padding,scroll_step; uint8_t has_at,has_hit_rect,reserved[2]; float at[2]; PxuiRect hit_rect; PxuiHit hit; void *hit_user; PxuiRelease hit_release; } PxuiBoxDesc;
/* The callback UI pointer is borrowed and valid only for that callback; do not retain or destroy it. */
typedef void (*PxuiBuild)(void *user, PxuiUi *ui);
typedef void (*PxuiPaintBuild)(void *user, PxuiPaint *paint, PxuiRect rect);
typedef void (*PxuiPaintRelease)(void *user);
typedef void (*PxuiSpanBuild)(void *user, float x, float width);
typedef void (*PxuiPickerRows)(void *user, PxuiStr query, PxuiStrSpan *out);
typedef uint8_t (*PxuiTextValid)(void *user, PxuiStr value);
typedef void (*PxuiCollectionEmit)(void *user, PxuiStr text);
typedef void (*PxuiCollectionText)(void *user, size_t row, size_t column, PxuiCollectionEmit emit, void *emit_user);
typedef PxuiStr (*PxuiTableCell)(void *user, int64_t row, int64_t column);
typedef struct { PxuiStr label; size_t count; uint8_t multi,reserved[7]; PxuiU8MutSpan selected; PxuiCollectionText text; void *user; } PxuiListViewDesc;
typedef struct { PxuiStr label; const PxuiTreeNode *ptr; size_t len; PxuiU8MutSpan open; size_t *selected; PxuiU32MutSpan visible; size_t *visible_len; PxuiCollectionText text; void *user; } PxuiTreeViewDesc;
typedef struct { PxuiStr label; PxuiStrSpan titles; PxuiF32MutSpan widths; size_t rows; uint8_t has_sort,ascending,reserved[6]; size_t sort_column; size_t *selected; PxuiCollectionText text; void *user; } PxuiTableViewDesc;
typedef struct { PxuiStr label; float at[2],width,height; PxuiStrSpan headers; int64_t rows; PxuiTableCell cell; void *user; } PxuiTableDesc;
typedef struct { PxuiStr title; PxuiStrSpan items; } PxuiMenuDesc;
typedef struct { const PxuiMenuDesc *ptr; size_t len; } PxuiMenuDescSpan;
typedef void (*PxuiGridBuild)(void *user, PxuiUi *ui, size_t index);
typedef struct { PxuiStr label; size_t count; float cell_w,cell_h; PxuiGridBuild build; void *user; } PxuiGridListDesc;
typedef struct { PxuiStr label; uint32_t axis; float ratio,min_a,min_b; PxuiBuild build_a; void *user_a; PxuiBuild build_b; void *user_b; } PxuiSplitDesc;
typedef struct { PxuiStr label; PxuiMenuDescSpan menus; } PxuiMenuBarDesc;
typedef struct { size_t menu,item; } PxuiMenuOut;
typedef struct { PxuiStr key,value; } PxuiHintDesc;
typedef struct { uint8_t has_right,reserved[7]; PxuiStr right; const PxuiHintDesc *ptr; size_t len; } PxuiFooterDesc;
typedef struct { float time; uint32_t target; uint8_t reserved[4]; float values[4]; PxuiLookDesc look; PxuiCurveDesc curve; } PxuiKeyframeDesc;
typedef struct { const PxuiKeyframeDesc *ptr; size_t len; } PxuiKeyframeSpan;
typedef struct { const PxuiHintDesc *ptr; size_t len; } PxuiHintSpan;
typedef struct { PxuiStr key,label; uint8_t has_width,has_pin,pin,reserved[5]; float width; PxuiBox row; float control_x,control_y,control_w; } PxuiInspectorRowDesc;
typedef struct { PxuiStr key,label; uint8_t expanded,has_set,set_expanded,reserved[5]; PxuiBuild build; void *user; uint8_t out_open; } PxuiInspectorSectionDesc;
typedef struct { PxuiStr key,label,value; uint8_t has_width,reserved[7]; float width; } PxuiInspectorValueDesc;
typedef struct { PxuiStr key,text; } PxuiInspectorTextDesc;
enum PxuiSplitAxis { PXUI_SPLIT_ROW=0, PXUI_SPLIT_COLUMN=1 };
enum PxuiKeyframeTarget { PXUI_KEY_OFFSET=0, PXUI_KEY_OPACITY=1, PXUI_KEY_LOOK=2 };

uint32_t pxui_abi_version(void);
PxuiUi *pxui_ui_create(const PxuiUiDesc *desc);
PxuiUi *pxui_ui_create_with_fonts(const PxuiUiDesc *desc, const PxuiStr *fonts, size_t count);
int32_t pxui_ui_destroy(PxuiUi *ui); /* PXUI_ERR_STATE: borrowed handle, inside a frame, or a renderer still attached */
PxuiStr pxui_last_error(const PxuiUi *ui);
int32_t pxui_frame(PxuiUi *ui, const PxuiFrameIn *in, PxuiBuild build, void *user, PxuiFrameOut *out);
int32_t pxui_ui_frame(PxuiUi *ui, const PxuiInput *in, PxuiBuild build, void *user, PxuiFrameOut *out);
const PxuiOutput *pxui_ui_output(PxuiUi *ui); /* borrowed from the Ui until its next frame */
PxuiThemeTokens pxui_ui_tokens(PxuiUi *ui);
PxuiThemeSnapshot pxui_ui_theme(PxuiUi *ui);
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
uint8_t pxui_ui_patch_text(PxuiUi *ui, const PxuiBox *box, PxuiStr value);
uint8_t pxui_ui_busy_step(PxuiUi *ui, const PxuiBox *box, double now);
int32_t pxui_ui_request_frame(PxuiUi *ui);
int32_t pxui_ui_node(PxuiUi *ui, PxuiStr label, const PxuiNodeDesc *node, PxuiBox *out);
int32_t pxui_ui_node_keyed(PxuiUi *ui, int64_t key, const PxuiNodeDesc *node, PxuiBox *out);
int32_t pxui_ui_rect(PxuiUi *ui, const PxuiBox *box, PxuiRect *out);
uint8_t pxui_ui_box_of_key(PxuiUi *ui, int64_t key, PxuiBox *out);
int32_t pxui_ui_text(PxuiUi *ui, const PxuiBox *box, PxuiStr value);
int32_t pxui_ui_set_look(PxuiUi *ui, const PxuiBox *box, const PxuiLookDesc *look, const PxuiStateDesc *state);
int32_t pxui_ui_style(PxuiUi *ui, const PxuiBox *box, const PxuiLookDesc states[6], const PxuiStateDesc *state);
float pxui_ui_text_width(PxuiUi *ui, uint8_t has_size, int64_t size, PxuiStr text);
int32_t pxui_ui_draw(PxuiUi *ui, const PxuiBox *box, uint8_t over, PxuiPaintBuild build, void *user, PxuiPaintRelease release);
void pxui_within(PxuiUi *ui, const PxuiBox *box, PxuiBuild build, void *user);
void pxui_scope(PxuiUi *ui, PxuiStr label, PxuiBuild build, void *user);
void pxui_col(PxuiUi *ui, float padding, PxuiStr label, PxuiBuild build, void *user);
void pxui_panel(PxuiUi *ui, const PxuiPanel *panel, PxuiStr label, PxuiBuild build, void *user);
void pxui_label(PxuiUi *ui, PxuiStr text);
uint8_t pxui_button(PxuiUi *ui, PxuiStr label);
uint8_t pxui_toggle(PxuiUi *ui, PxuiStr label, uint8_t on);
uint8_t pxui_text_field(PxuiUi *ui, PxuiStr label, PxuiStr value, PxuiStr *changed);
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
int32_t pxui_ui_context_menu(PxuiUi *ui, float x, float y, PxuiStr label, PxuiStrSpan items, PxuiU8Span enabled, uint8_t has_width, float width, uint8_t has_selected, int64_t selected, uint32_t *out_tag, int64_t *out_index);
int32_t pxui_ui_picker(PxuiUi *ui, int64_t limit, uint8_t slash, uint8_t at_rest, PxuiStr label, PxuiStr query, PxuiPickerRows rows, void *user, PxuiStr *out_query, uint32_t *out_tag, int64_t *out_index);
int32_t pxui_ui_list_view(PxuiUi *ui, const PxuiListViewDesc *desc, size_t *activated);
int32_t pxui_ui_tree(PxuiUi *ui, const PxuiTreeViewDesc *desc, size_t *activated);
int32_t pxui_ui_table_view(PxuiUi *ui, const PxuiTableViewDesc *desc, size_t *sort_column);
int32_t pxui_ui_text_area_submit(PxuiUi *ui, const float at[2], float width, float height, uint8_t wrap, PxuiStr label, PxuiStr value, PxuiStr *out_text, uint8_t *out_changed, uint8_t *out_submitted);
int32_t pxui_ui_value_field(PxuiUi *ui, const float at[2], float width, float height, PxuiStr label, PxuiStr value, PxuiTextValid valid, void *user, PxuiStr *out_text, uint8_t *out_changed, uint8_t *out_editing);
PxuiText *pxui_text_create(const PxuiStr *fonts, size_t count);
void pxui_text_destroy(PxuiText *text);
uint32_t pxui_text_add_font(PxuiText *text, PxuiStr font);
int32_t pxui_text_width(PxuiText *text, uint64_t key, PxuiStr value, const PxuiTextStyle *style, float max_width, PxuiTextMetrics *out);
size_t pxui_text_quads(PxuiText *text, uint64_t key, PxuiStr value, const PxuiTextStyle *style, float max_width, float x, float y, float scale, PxuiTextGlyph *out, size_t capacity);
int32_t pxui_text_end_frame(PxuiText *text);
size_t pxui_text_rasterize_rgba(PxuiText *text, uint64_t key, PxuiStr value, const PxuiTextStyle *style, float max_width, float scale, uint8_t color_r, uint8_t color_g, uint8_t color_b, uint8_t color_a, uint32_t *width, uint32_t *height, uint8_t *out, size_t capacity);
int32_t pxui_paint_info(PxuiPaint *paint, PxuiPaintInfo *out);
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
int32_t pxui_renderer_draw(PxuiRenderer *renderer, const PxuiPass *pass);
int32_t pxui_renderer_atlas(PxuiRenderer *renderer, uint32_t kind, PxuiImage *out); /* kind 0: mask atlas (one byte per texel), 1: colour atlas (RGBA8); the texture stays the renderer's */
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
int32_t pxui_ui_view_transform_about(PxuiUi *ui, const PxuiBox *box, float scale, const float *transform, float px, float py);
int32_t pxui_ui_view_backdrop_blur(PxuiUi *ui, const PxuiBox *box, float sigma);

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
PxuiTheme *pxui_theme_create(const PxuiThemeTokens *tokens); /* tokens may be NULL for the defaults */
void pxui_theme_destroy(PxuiTheme *theme);
int32_t pxui_theme_set(PxuiTheme *theme, PxuiStr key, PxuiStr value);
int32_t pxui_theme_load(PxuiTheme *theme, PxuiStr path);
PxuiStr pxui_theme_last_error(const PxuiTheme *theme); /* borrowed from the theme until its next call */
int32_t pxui_ui_themed(PxuiUi *ui, const PxuiTheme *theme, PxuiBuild build, void *user);

#endif
