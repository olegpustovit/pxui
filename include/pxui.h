#ifndef PXUI_H
#define PXUI_H

#include <stddef.h>
#include <stdint.h>

typedef struct PxuiUi PxuiUi;
typedef struct PxuiRenderer PxuiRenderer;
typedef struct PxuiText PxuiText;
typedef struct PxuiPaint PxuiPaint;
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
    PxuiEventSpan keys;
    size_t text_len;
} PxuiSignal;
typedef struct { const uint32_t *ptr; size_t len; } PxuiU32Span;
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
    PXUI_KEY_UP=0x100, PXUI_KEY_DOWN=0x101, PXUI_KEY_LEFT=0x102, PXUI_KEY_RIGHT=0x103,
    PXUI_KEY_SPACE=0x104, PXUI_KEY_ENTER=0x105, PXUI_KEY_ESCAPE=0x106, PXUI_KEY_BACKSPACE=0x107,
    PXUI_KEY_TAB=0x108, PXUI_KEY_SHIFT=0x109, PXUI_KEY_CTRL=0x10a, PXUI_KEY_ALT=0x10b, PXUI_KEY_META=0x10c,
    PXUI_KEY_F1=0x120, PXUI_KEY_F12=0x12b, PXUI_KEY_HOME=0x130, PXUI_KEY_DELETE=0x135
};
enum PxuiTargetFormat { PXUI_TARGET_RGBA8=1, PXUI_TARGET_BGRA8=2 };
typedef struct {
    uint32_t size;
    float x, y, width, height;
    int64_t padding;
    uint8_t has_height, window;
} PxuiPanel;
typedef struct { uint64_t index; int64_t key; uint64_t slot; uint32_t generation; } PxuiBox;
typedef struct { uint16_t depth; uint8_t children,reserved; } PxuiTreeNode;
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
typedef struct { PxuiStr key,label; uint8_t has_width,has_pin,pin,reserved[5]; float width; PxuiBox row; float control_x,control_y,control_w; } PxuiInspectorRowDesc;
typedef struct { PxuiStr key,label; uint8_t expanded,has_set,set_expanded,reserved[5]; PxuiBuild build; void *user; uint8_t out_open; } PxuiInspectorSectionDesc;
typedef struct { PxuiStr key,label,value; uint8_t has_width,reserved[7]; float width; } PxuiInspectorValueDesc;
typedef struct { PxuiStr key,text; } PxuiInspectorTextDesc;
enum PxuiSplitAxis { PXUI_SPLIT_ROW=0, PXUI_SPLIT_COLUMN=1 };
enum PxuiKeyframeTarget { PXUI_KEY_OFFSET=0, PXUI_KEY_OPACITY=1, PXUI_KEY_LOOK=2 };

uint32_t pxui_abi_version(void);
PxuiUi *pxui_ui_create(const PxuiUiDesc *desc);
PxuiUi *pxui_ui_create_with_fonts(const PxuiUiDesc *desc, const PxuiStr *fonts, size_t count);
void pxui_ui_destroy(PxuiUi *ui);
PxuiStr pxui_last_error(const PxuiUi *ui);
int32_t pxui_frame(PxuiUi *ui, const PxuiFrameIn *in, PxuiBuild build, void *user, PxuiFrameOut *out);
int32_t pxui_ui_frame(PxuiUi *ui, const PxuiInput *in, PxuiBuild build, void *user, PxuiFrameOut *out);
PxuiOutput pxui_ui_output(PxuiUi *ui);
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
int32_t pxui_ui_signal(PxuiUi *ui, const PxuiBox *box, PxuiSignal *out, PxuiEvent *events, size_t event_capacity, uint8_t *text, size_t text_capacity);
uint8_t pxui_ui_patch_text(PxuiUi *ui, const PxuiBox *box, PxuiStr value);
uint8_t pxui_ui_busy_step(PxuiUi *ui, const PxuiBox *box, double now);
int32_t pxui_ui_request_frame(PxuiUi *ui);
int32_t pxui_ui_node(PxuiUi *ui, PxuiStr label, const PxuiNodeDesc *node, PxuiBox *out);
int32_t pxui_ui_node_keyed(PxuiUi *ui, int64_t key, const PxuiNodeDesc *node, PxuiBox *out);
int32_t pxui_ui_rect(PxuiUi *ui, const PxuiBox *box, PxuiRect *out);
uint8_t pxui_ui_box_of_key(PxuiUi *ui, int64_t key, PxuiBox *out);
int64_t pxui_key_of(int64_t seed, PxuiStr label);
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
int64_t pxui_paint_font_size(PxuiPaint *paint);
int64_t pxui_paint_row_height(PxuiPaint *paint);
int64_t pxui_paint_density(PxuiPaint *paint);
PxuiRect pxui_paint_clip(PxuiPaint *paint);
float pxui_paint_clip_radius(PxuiPaint *paint);
float pxui_paint_text_top(PxuiPaint *paint, uint8_t has_size, int64_t size, float y, float h);
int64_t pxui_paint_label_size(PxuiPaint *paint);
float pxui_paint_cap_tracking(PxuiPaint *paint);
uint8_t pxui_paint_rtl(PxuiPaint *paint);
PxuiThemeTokens pxui_paint_theme(PxuiPaint *paint);
void pxui_paint_fill(PxuiPaint *paint, float x, float y, float w, float h, PxuiColor color);
void pxui_paint_stroke_width(PxuiPaint *paint, float x, float y, float w, float h, float width, PxuiColor color);
void pxui_paint_stroke(PxuiPaint *paint, float x, float y, float w, float h, PxuiColor color);
void pxui_paint_frame_width(PxuiPaint *paint, float x, float y, float w, float h, float width, PxuiColor color);
void pxui_paint_frame(PxuiPaint *paint, float x, float y, float w, float h, PxuiColor color);
void pxui_paint_rect(PxuiPaint *paint, float x, float y, float w, float h, PxuiColor fill, uint8_t has_fill, PxuiColor stroke, uint8_t has_stroke);
void pxui_paint_line_width(PxuiPaint *paint, float x0, float y0, float x1, float y1, float width, PxuiColor color);
void pxui_paint_line(PxuiPaint *paint, float x0, float y0, float x1, float y1, PxuiColor color);
void pxui_paint_circle(PxuiPaint *paint, float x, float y, float radius, PxuiColor fill, uint8_t has_fill, PxuiColor stroke, uint8_t has_stroke);
void pxui_paint_grid(PxuiPaint *paint, float x, float y, float w, float h, float ox, float oy, float spacing, float dot, PxuiColor color);
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
void pxui_paint_chevron(PxuiPaint *paint, float x, float y, uint32_t direction, PxuiColor color);
void pxui_paint_brackets(PxuiPaint *paint, float x, float y, float w, float h, float offset, float length, float width, PxuiColor color);
void pxui_paint_dashed_width(PxuiPaint *paint, float x0, float y0, float x1, float y1, float width, PxuiColor color);
void pxui_paint_dashed(PxuiPaint *paint, float x0, float y0, float x1, float y1, PxuiColor color);
void pxui_paint_dashed_rect(PxuiPaint *paint, float x, float y, float w, float h, PxuiColor color);
void pxui_paint_flag(PxuiPaint *paint, float x, float y, uint8_t round, uint8_t on);
void pxui_paint_ratio(PxuiPaint *paint, float x, float y, int64_t a, int64_t b);
void pxui_paint_input_region(PxuiPaint *paint, float cursor, float x, float y, float w, float h);
void pxui_paint_image(PxuiPaint *paint, const PxuiImage *image, float x, float y, float width, float height, uint32_t fit, PxuiColor tint);
PxuiRenderer *pxui_renderer_create(PxuiUi *ui, const PxuiGpu *gpu, const PxuiTargetDesc *target);
int32_t pxui_renderer_prepare(PxuiRenderer *renderer, const PxuiCommands *commands, uint64_t frame, uint64_t completed);
int32_t pxui_renderer_draw(PxuiRenderer *renderer, const PxuiPass *pass);
void pxui_renderer_destroy(PxuiRenderer *renderer);

#endif
