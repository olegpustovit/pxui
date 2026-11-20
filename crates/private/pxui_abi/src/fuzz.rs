//! The export fuzz (OS-grade criterion 3): every `pxui_*` export the manifests list is called outside a frame and inside one with
//! zero, null, garbage and oversized arguments. No call may panic (the export macro's `catch_unwind` is a backstop for library bugs,
//! not a way to refuse an argument) and no call may poison the `Ui`. A name in `abi/pxui.v1.txt` or `abi/pxui.ext.txt` that this file
//! does not call fails `every_manifest_export_is_fuzzed`, so a new export is covered the day it is added.
//!
//! Pointers are null or point at a zeroed buffer (whose first four bytes, a record's `size`, vary); a garbage pointer cannot be
//! checked by an in-process C library (handles and pointers are the caller's to keep valid) and is not tried. Strings, spans, numbers and records by value take
//! garbage: invalid UTF-8, lengths past every cap, NaN and infinities, all-ones boxes.
use crate::*;
use crate::state::PxuiUi;
use core::ffi::c_void;
use pxui_types::ffi::*;
use pxui_types::*;
use std::any::Any;
use std::cell::{Cell, RefCell};

/// Values per argument: argument `i` of round `k` takes variant `(k + i) % VARIANTS`, so every argument sees every variant and the
/// combinations differ between rounds.
const VARIANTS: u32 = 6;
const BUF_BYTES: usize = 1 << 20;
static GARBAGE: [u8; 8] = [0xff, 0xfe, 0xc0, 0x80, 0xed, 0xa0, 0x80, 0xff];
static LONG: [u8; 70_000] = [b'a'; 70_000];
/// Labels in the reserved resolved form that carry no usable key: 0 and 1 (the key tables' marks) and a 0x01 byte with non-hex digits.
static UNRESOLVED: [&[u8; 17]; 3] = [b"\x010000000000000000", b"\x010000000000000001", b"\x0100000000000000zz"];

thread_local! {
    static BUF: Cell<*mut u8> = Cell::new(Box::leak(vec![0u128; BUF_BYTES / 16].into_boxed_slice()).as_mut_ptr().cast());
    static CUR: Cell<*mut PxuiUi> = const { Cell::new(core::ptr::null_mut()) };
    static SHAPER: Cell<*mut PxuiShaper> = const { Cell::new(core::ptr::null_mut()) };
    static THEME: Cell<*mut PxuiTheme> = const { Cell::new(core::ptr::null_mut()) };
    static PANICS: Cell<u32> = const { Cell::new(0) };
    /// a box stale since an earlier frame, one of another `Ui`, one declared in the frame being fuzzed (`PxuiBox::arg`)
    static BOXES: Cell<[PxuiBox; 3]> = const { Cell::new([PxuiBox { index: 0, box_key: 0, box_slot: 0, generation: 0 }; 3]) };
    static BOX_SLOT: *mut PxuiBox = Box::leak(Box::new(PxuiBox::default()));
    /// `PXUI_FUZZ_TRACE=1` names each call before it runs: the last line names a call that aborted the process
    static TRACE: bool = std::env::var_os("PXUI_FUZZ_TRACE").is_some();
    static FAILURES: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn buf() -> *mut u8 { BUF.with(Cell::get) }
fn reset() { unsafe { core::ptr::write_bytes(buf(), 0, BUF_BYTES) } }

/// One argument of variant `v`.
trait Arg { fn arg(v: u32) -> Self; }

/// The `Arg` of one type, by kind (one arm per kind, a list of types at the end):
/// - `int`/`signed`/`float`: zero, one, a small number, the largest, -1 or the smallest, another small one; NaN and infinities for floats.
/// - `span`: empty, null with a length, three zeroed elements, a length past every cap, one element, half the address space.
/// - `sized` (a size-prefixed record behind a pointer): the whole size, null, size 0, `u32::MAX` (a newer, larger record), a size ending
///   inside a field, the whole size again.
/// - `plain` (any other value behind a pointer): null (variant 1) or the zeroed buffer.
/// - `null` (a handle the fuzz cannot make here: a renderer needs a GPU, a paint handle lives in a paint callback, a prepared text
///   comes from a worker): null.
/// - `none` (a callback: a garbage function pointer is not checkable): none.
/// - `bytes` (a record by value): zero, all ones (NaN floats, out-of-range tags, huge indices), zero again.
macro_rules! arg {
    (int $t:ty) => { impl Arg for $t { fn arg(v: u32) -> Self { [0, 1, 3, <$t>::MAX, 2, 7][(v % VARIANTS) as usize] } } };
    (signed $t:ty) => { impl Arg for $t { fn arg(v: u32) -> Self { [0, 1, -1, <$t>::MAX, <$t>::MIN, 7][(v % VARIANTS) as usize] } } };
    (float $t:ty) => { impl Arg for $t { fn arg(v: u32) -> Self { [0.0, <$t>::NAN, 1.0, <$t>::INFINITY, -1.0, 1e30][(v % VARIANTS) as usize] } } };
    (span $t:ty) => { impl Arg for Slice<$t> {
        fn arg(v: u32) -> Self {
            let ptr = buf().cast::<$t>().cast_const();
            match v % VARIANTS { 0 => Slice { ptr: core::ptr::null(), len: 0 }, 1 => Slice { ptr: core::ptr::null(), len: 4 }, 2 => Slice { ptr, len: 3 },
                3 => Slice { ptr, len: usize::MAX }, 4 => Slice { ptr, len: 1 }, _ => Slice { ptr, len: usize::MAX / 2 } }
        }
    } };
    (sized $t:ty) => {
        impl Arg for *const $t { fn arg(v: u32) -> Self { <*mut $t>::arg(v).cast_const() } }
        impl Arg for *mut $t {
            fn arg(v: u32) -> Self {
                let full = core::mem::size_of::<$t>() as u32;
                let size = match v % VARIANTS { 1 => return core::ptr::null_mut(), 2 => 0, 3 => u32::MAX, 4 => full.saturating_sub(4).max(4) - 1, _ => full };
                unsafe { buf().cast::<u32>().write(size) };
                buf().cast()
            }
        }
    };
    (plain $t:ty) => {
        impl Arg for *const $t { fn arg(v: u32) -> Self { <*mut $t>::arg(v).cast_const() } }
        impl Arg for *mut $t { fn arg(v: u32) -> Self { if v % VARIANTS == 1 { core::ptr::null_mut() } else { buf().cast() } } }
    };
    (null $t:ty) => { impl Arg for *mut $t { fn arg(_: u32) -> Self { core::ptr::null_mut() } } };
    (none $t:ty) => { impl Arg for $t { fn arg(_: u32) -> Self { None } } };
    (bytes $t:ty) => { impl Arg for $t {
        fn arg(v: u32) -> Self { let mut value: $t = unsafe { core::mem::zeroed() }; if v % 3 == 1 { unsafe { core::ptr::write_bytes(&mut value as *mut $t, 0xff, 1) } } value }
    } };
    ($kind:ident: $($t:ty),* $(,)?) => { $(arg!($kind $t);)* };
}
arg!(int: u8, u16, u32, u64, usize);
arg!(signed: i32, i64);
arg!(float: f32, f64);
arg!(span: Str, u8, i64, Event, A11yAction, PxuiHintDesc, PxuiKeyframeDesc, pxui_types::ffi::PxuiPickItem);
arg!(sized: PxuiTextFieldDesc, Environment, Gpu, Keyboard, Pass, pxui_text::TextSpans, Commands, A11yAction, Input, PxuiButtonDesc, PxuiDialogDesc, PxuiDialogResult, PxuiSegmentedResult,
    PxuiDismissIn, PxuiDismissOut, PxuiFooterDesc, PxuiGestureIn, PxuiGestureOut, PxuiGridListDesc, PxuiHeaderDesc, PxuiHeadOut, PxuiIconButtonDesc,
    PxuiInspectorRowDesc, PxuiInspectorSectionDesc, PxuiInspectorTextDesc, PxuiInspectorValueDesc, PxuiKeyframeDesc, PxuiLabelStyle, PxuiLookDesc, PxuiMenuBarDesc, PxuiMenuExtra, PxuiMenuOut, PxuiNodeFields, PxuiPopupDesc, PxuiResponse,
    PxuiScrubOut, PxuiSignal, PxuiSplitDesc, PxuiStateDesc, PxuiSwipeIn, PxuiTreeViewDesc, PxuiValueFieldDesc, ThemeTokens, FrameIn, FrameOut, Image, Node, PxuiPanel,
    PxuiTargetDesc, PxuiUiDesc, TextMetrics, PxuiTextStyle, PxuiAnchorDesc, PxuiAreaCallbacks, PxuiLabelBox, PxuiMarkDesc, PxuiSegmentedDesc,
    PxuiTextAreaDesc, PxuiTextInputOptions, PxuiToggleDesc, PxuiCheckboxDesc, PxuiSheetDesc, Span, TextGlyph,
    PxuiSectionListDesc, PxuiListResult, PxuiSelectionListDesc, PxuiTableHeadDesc, PxuiHeadResult, PxuiTableDesc, PxuiTableResult);
arg!(plain: u16, *mut PxuiRenderer, u8, u32, u64, i64, usize, f32, c_void, Str, geom::Rgba, geom::Rect, geom::Point, *const u32, *mut PxuiPreparedText, A11yTree, Slice<A11yAction>);
arg!(null: PxuiRenderer, PxuiPaint, PxuiPreparedText, PxuiA11y, PxuiTextWorker);
arg!(none: UiBuild, PxuiGridBuild, PxuiButtonBuild, PxuiBodyFn, crate::paint::PaintBuild, crate::paint::PaintRelease, crate::paint::SpanBuild,
    crate::overlays::PickerRows, crate::overlays::PickerMark, crate::overlays::PickerOff, crate::overlays::PickerImage, crate::text_inputs::TextValid, crate::text_inputs::SlideText,
    crate::text_inputs::ScrubText, pxui_types::ffi::PxuiRowKey, PxuiListCell, PxuiTableCellBuild);
arg!(bytes: geom::Rgba, geom::Rect, PxuiSlot);

/// Boxes: zero (the root slot), all ones (no slot), one stale since an earlier frame, one of another `Ui`, one live this frame (passed to
/// every call, so each takes a valid node of the wrong kind too: a plain node to `within`, `part`, `scroll_offset`, ...), zero again.
impl Arg for PxuiBox {
    fn arg(v: u32) -> Self {
        let [stale, foreign, live] = BOXES.with(Cell::get);
        match v % VARIANTS { 1 | 5 => PxuiBox { index: u64::MAX, box_key: -1, box_slot: u64::MAX, generation: u32::MAX }, 2 => stale, 3 => foreign, 4 => live, _ => PxuiBox::default() }
    }
}
/// A box behind a pointer: null for variant 1, else `PxuiBox::arg` in a slot of its own (the zeroed buffer may hold another argument).
impl Arg for *mut PxuiBox {
    fn arg(v: u32) -> Self {
        if v % VARIANTS == 1 { return core::ptr::null_mut(); }
        let slot = BOX_SLOT.with(|slot| *slot);
        unsafe { slot.write(PxuiBox::arg(v)) };
        slot
    }
}
impl Arg for *const PxuiBox { fn arg(v: u32) -> Self { <*mut PxuiBox>::arg(v).cast_const() } }

impl Arg for Str {
    fn arg(v: u32) -> Self {
        match v % VARIANTS {
            // the second round of the first three variants: an unusable resolved label (`UNRESOLVED`)
            0..=2 if v % 12 >= 6 => Str { ptr: UNRESOLVED[(v % 6) as usize].as_ptr(), len: 17 },
            0 => Str { ptr: core::ptr::null(), len: 0 },
            1 => Str { ptr: core::ptr::null(), len: 5 },
            2 => Str { ptr: GARBAGE.as_ptr(), len: GARBAGE.len() },
            3 => Str { ptr: GARBAGE.as_ptr(), len: 1 << 40 },
            4 => Str { ptr: b"k".as_ptr(), len: 1 },
            // past the key cap (300) and past the label cap (70 000)
            _ => Str { ptr: LONG.as_ptr(), len: if v % 12 == 5 { 300 } else { LONG.len() } },
        }
    }
}
impl Arg for *mut PxuiUi { fn arg(v: u32) -> Self { if v % VARIANTS == 5 { core::ptr::null_mut() } else { CUR.with(Cell::get) } } }
impl Arg for *const PxuiUi { fn arg(v: u32) -> Self { <*mut PxuiUi>::arg(v).cast_const() } }
impl Arg for *mut PxuiShaper { fn arg(v: u32) -> Self { if v % 2 == 1 { core::ptr::null_mut() } else { SHAPER.with(Cell::get) } } }
impl Arg for *mut PxuiTheme { fn arg(v: u32) -> Self { if v % 2 == 1 { core::ptr::null_mut() } else { THEME.with(Cell::get) } } }
impl Arg for *const PxuiTheme { fn arg(v: u32) -> Self { <*mut PxuiTheme>::arg(v).cast_const() } }
impl Arg for *const PxuiTextWorker { fn arg(_: u32) -> Self { core::ptr::null() } }
/// A keyed widget's identity record (the `*_keyed` calls): null, size 0, then the whole record with garbage in one field each: an invalid
/// UTF-8 key, a key past the identifier cap (300 bytes), a name past the display cap or with a huge length, a non-zero `reserved`.
impl Arg for *const PxuiKeyedDesc {
    fn arg(v: u32) -> Self {
        let size = core::mem::size_of::<PxuiKeyedDesc>() as u32;
        let ok = Str { ptr: b"k".as_ptr(), len: 1 };
        let desc = match v % VARIANTS {
            1 => return core::ptr::null(),
            2 => PxuiKeyedDesc { size: 0, reserved: 0, key: ok, name: ok },
            3 => PxuiKeyedDesc { size, reserved: 0, key: Str { ptr: GARBAGE.as_ptr(), len: GARBAGE.len() }, name: Str { ptr: LONG.as_ptr(), len: LONG.len() } },
            4 => PxuiKeyedDesc { size, reserved: 0, key: Str { ptr: LONG.as_ptr(), len: 300 }, name: Str { ptr: GARBAGE.as_ptr(), len: 1 << 40 } },
            5 if v % 12 == 11 => PxuiKeyedDesc { size, reserved: 0, key: Str { ptr: UNRESOLVED[0].as_ptr(), len: 17 }, name: ok },
            5 => PxuiKeyedDesc { size, reserved: 7, key: ok, name: ok },
            _ => PxuiKeyedDesc { size, reserved: 0, key: ok, name: Str { ptr: b"Name".as_ptr(), len: 4 } },
        };
        unsafe { buf().cast::<PxuiKeyedDesc>().write(desc) };
        buf().cast()
    }
}
impl Arg for PxuiSizeDesc { fn arg(v: u32) -> Self { PxuiSizeDesc { kind: u32::arg(v), value: f32::arg(v), relative: None } } }

/// Records the failures of one call: a panic (even one the export caught) and a poisoned `Ui`. The `Ui` is unpoisoned so the rest
/// of the fuzz runs.
fn check(name: &str, result: &dyn Any, panics: u32) {
    let ui = CUR.with(Cell::get);
    let shared = unsafe { (*ui).shared };
    let mut failure = None;
    if PANICS.with(Cell::get) != panics { failure = Some("panicked"); }
    if result.downcast_ref::<i32>() == Some(&PXUI_ERR_PANIC) { failure = Some("returned PXUI_ERR_PANIC"); }
    if unsafe { (*shared).poisoned } {
        failure = Some("poisoned the Ui");
        unsafe { (*shared).poisoned = false; (*shared).current = ui; }
    }
    if let Some(failure) = failure {
        let error = unsafe { (*shared).error.clone() };
        FAILURES.with(|f| f.borrow_mut().push(format!("{name} {failure} (in frame {}): {error}", unsafe { (*shared).in_frame })));
    }
}

/// The export list: `FUZZED` (their names) and `fuzz_all`, which calls each once per variant round. `@call` is one call: argument `i`
/// takes variant `k + i`; the buffer is zeroed first, and the call is named on stderr under `PXUI_FUZZ_TRACE` before it runs.
macro_rules! fuzz {
    ($($f:ident / $n:tt),* $(,)?) => {
        const FUZZED: &[&str] = &[$(stringify!($f)),*];
        fn fuzz_all() { for k in 0..VARIANTS { $(fuzz!(@one $f / $n, k);)* } }
    };
    (@call $f:ident, $k:ident; $($i:literal)*) => {{
        reset();
        if TRACE.with(|t| *t) { eprintln!("fuzz: {} round {}", stringify!($f), $k); }
        let panics = PANICS.with(Cell::get);
        let result = unsafe { $f($(Arg::arg($k + $i)),*) };
        check(stringify!($f), &result, panics);
    }};
    (@one $f:ident / 0, $k:ident) => { fuzz!(@call $f, $k;) };
    (@one $f:ident / 1, $k:ident) => { fuzz!(@call $f, $k; 0) };
    (@one $f:ident / 2, $k:ident) => { fuzz!(@call $f, $k; 0 1) };
    (@one $f:ident / 3, $k:ident) => { fuzz!(@call $f, $k; 0 1 2) };
    (@one $f:ident / 4, $k:ident) => { fuzz!(@call $f, $k; 0 1 2 3) };
    (@one $f:ident / 5, $k:ident) => { fuzz!(@call $f, $k; 0 1 2 3 4) };
    (@one $f:ident / 6, $k:ident) => { fuzz!(@call $f, $k; 0 1 2 3 4 5) };
    (@one $f:ident / 7, $k:ident) => { fuzz!(@call $f, $k; 0 1 2 3 4 5 6) };
    (@one $f:ident / 8, $k:ident) => { fuzz!(@call $f, $k; 0 1 2 3 4 5 6 7) };
    (@one $f:ident / 9, $k:ident) => { fuzz!(@call $f, $k; 0 1 2 3 4 5 6 7 8) };
    (@one $f:ident / 10, $k:ident) => { fuzz!(@call $f, $k; 0 1 2 3 4 5 6 7 8 9) };
    (@one $f:ident / 11, $k:ident) => { fuzz!(@call $f, $k; 0 1 2 3 4 5 6 7 8 9 10) };
    (@one $f:ident / 13, $k:ident) => { fuzz!(@call $f, $k; 0 1 2 3 4 5 6 7 8 9 10 11 12) };
    (@one $f:ident / 14, $k:ident) => { fuzz!(@call $f, $k; 0 1 2 3 4 5 6 7 8 9 10 11 12 13) };
}

// Every export but those `special` calls itself (constructors, destructors and process switches), as `name / arity`.
fuzz!(
    pxui_ui_inspector_bar/3, pxui_ui_inspector_body/3, pxui_ui_inspector_button/4, pxui_ui_inspector_header/5, pxui_ui_inspector_label_x/1,
    pxui_ui_inspector_message/2, pxui_ui_inspector_readout/2, pxui_ui_inspector_row/2, pxui_ui_inspector_section/2, pxui_ui_inspector_toggle/5, pxui_ui_inspector_width/1, pxui_ui_layout_node/3, pxui_ui_line_field/13, pxui_ui_menu_bar/4, pxui_ui_modal/7, pxui_ui_panel/6, pxui_ui_pick/8, pxui_ui_tabs/8, pxui_ui_popup/7, pxui_ui_scrub/8, pxui_ui_slider/7, pxui_ui_split/5, pxui_ui_value_field/11,
    pxui_a11y_fd/1, pxui_a11y_pump/2, pxui_a11y_update/2, pxui_abi_version/0, pxui_body_register/6, pxui_counter/1, pxui_default_font/0, pxui_frame/5, pxui_last_error/1, pxui_paint_caret_x/5,
    pxui_paint_color/2, pxui_paint_icon/7, pxui_paint_icon_word/1, pxui_paint_image/8, pxui_paint_input_region/6, pxui_paint_label/6, pxui_paint_label_chars/5, pxui_paint_label_measure/6, pxui_paint_label_spans/8,
    pxui_paint_ratio/5, pxui_paint_scroll_y/2, pxui_paint_selection/8, pxui_paint_span/3, pxui_part/4,
    pxui_part_box/4, pxui_renderer_atlas/3, pxui_renderer_create/4, pxui_renderer_draw/2,
    pxui_renderer_draw_calls/1, pxui_renderer_forget_image/3, pxui_renderer_prepare/4, pxui_renderer_resize/3, pxui_scope/4, pxui_shaper_add_font/2,
    pxui_shaper_prepare/7, pxui_shaper_rasterize_rgba/14, pxui_shaper_system_fonts/1, pxui_shaper_width/6,
    pxui_shaper_width_spans/7, pxui_slot/3, pxui_supports/1, pxui_text_add_font/2, pxui_text_install/2, pxui_text_system_fonts/1, pxui_theme_density/2, pxui_theme_last_error/1, pxui_theme_parse/2,
    pxui_theme_reduced_motion/2, pxui_theme_set_color/3, pxui_ui_a11y_name/3, pxui_ui_a11y_number/6, pxui_ui_a11y_value/5, pxui_ui_animate/4,
    pxui_ui_animate_keys/4, pxui_ui_animate_keys_repeat/7, pxui_ui_box_of_a11y/3, pxui_ui_box_of_key/3, pxui_ui_busy_step/4,
    pxui_ui_button/5, pxui_ui_button_plain/4, pxui_ui_bytes/3,
    pxui_ui_cached_label/5, pxui_ui_calls/1, pxui_ui_cancel_carry/1, pxui_ui_captured/1, pxui_ui_reordering/1, pxui_ui_carry/5, pxui_ui_carrying/4, pxui_ui_progress/5, pxui_ui_spinner/3,
    pxui_ui_badge/4, pxui_ui_anchored/8, pxui_ui_scroll_view/6, pxui_ui_view/6,
    pxui_ui_toasts/6, pxui_ui_picker/14, pxui_ui_checkbox/3, pxui_ui_section_index/5, pxui_ui_disclosure/6,
    pxui_ui_color/2, pxui_ui_content_rect/2, pxui_ui_context_at/4, pxui_ui_context_menu/13, pxui_ui_density/1,
    pxui_ui_dialog/5, pxui_ui_disabled/4, pxui_ui_dismiss/4, pxui_ui_dismiss_popup/1, pxui_ui_draw/6,
    pxui_ui_drop_target/5, pxui_ui_environment/2, pxui_ui_expanded/3, pxui_ui_field_drag/4, pxui_ui_focus_box/2, pxui_ui_focus_default/2,
    pxui_ui_focused/3, pxui_ui_font_points/2, pxui_ui_footer/2, pxui_ui_frame/5, pxui_ui_frame_index/1, pxui_ui_gesture/5, pxui_ui_grid_list/3, pxui_ui_grip/7,
    pxui_ui_hover_delay/3, pxui_ui_key_seed/2, pxui_ui_hovered/4, pxui_ui_icon_button_plain/5, pxui_ui_icon_button/3, pxui_ui_icon_word/2, pxui_ui_image/8, pxui_ui_inject_a11y/3, pxui_ui_input/4, pxui_ui_item_text/2, pxui_ui_key_down/3, pxui_ui_key_pressed/4, pxui_ui_keyboard/2, pxui_ui_label_node/5, pxui_ui_label_text_spans/5, pxui_ui_label_text_line_box/5, pxui_ui_label_measure/6,
    pxui_ui_label_quads/9, pxui_ui_label_rasterize_rgba/13, pxui_ui_label_prewarm/4, pxui_ui_layer_drawn/2, pxui_ui_load_text_edit/6, pxui_ui_local_rect/3, pxui_ui_node/4,
    pxui_ui_node_fields/3, pxui_ui_node_keyed/4, pxui_ui_node_layout/3, pxui_ui_now/1, pxui_ui_output/1, pxui_ui_passed_undo/1, pxui_ui_patch_field/5,
    pxui_ui_patch_look/4, pxui_ui_patch_mark/4, pxui_ui_patch_rect/4, pxui_ui_patch_text/4, pxui_ui_phase_cpu_times/3, pxui_ui_phase_times/3, pxui_ui_platform/1,
    pxui_ui_press_keys/4, pxui_ui_rebuilt_named/3, pxui_ui_rect/3, pxui_ui_remember/6,
    pxui_ui_repaint_after/2, pxui_ui_request_cursor/2, pxui_ui_request_frame/1, pxui_ui_resolve_label/3, pxui_ui_reveal/3, pxui_ui_root_at/6, pxui_ui_row_points/2,
    pxui_ui_scroll_offset/4, pxui_ui_search_field/10, pxui_ui_section_list/7, pxui_ui_selection_list/7, pxui_ui_table_head/4, pxui_ui_table/7,
    pxui_ui_segmented/5, pxui_ui_set_a11y/2, pxui_ui_set_at/4, pxui_ui_set_dir/2,
    pxui_ui_set_font_points/2, pxui_ui_set_node_fields/3, pxui_ui_set_node_layout/3, pxui_ui_set_role/4, pxui_ui_set_scroll_offset/3,
    pxui_ui_set_state/3, pxui_ui_set_text_state/4, pxui_ui_set_text_worker/2, pxui_ui_set_timing/2, pxui_ui_signal/3, pxui_ui_state/4, pxui_ui_style/4, pxui_ui_swipe_actions/3, pxui_ui_system_appearance/2,
    pxui_ui_text/3, pxui_ui_text_area/9, pxui_ui_text_field/5, pxui_ui_text_input_focused/1,
    pxui_ui_text_input_options/2, pxui_ui_text_layout_count/2, pxui_ui_text_state/4, pxui_ui_text_undo_bytes/1, pxui_ui_theme/1, pxui_ui_themed/4, pxui_ui_tip/3, pxui_ui_to_front/3, pxui_ui_toggle_plain/5, pxui_ui_toggle/3, pxui_ui_transfer_carry/3, pxui_ui_tree/3, pxui_ui_unfocus/1,
    pxui_ui_use_theme/2, pxui_ui_variant/3, pxui_ui_verify_views/2, pxui_ui_verifying/1, pxui_ui_view_backdrop_blur/3,
    pxui_ui_view_backdrop_saturation/3, pxui_ui_view_clip/3, pxui_ui_view_offset/4, pxui_ui_view_opacity/3, pxui_ui_view_scale/3,
    pxui_ui_view_size/2, pxui_ui_view_transform_about/6, pxui_ui_wants_frame/4, pxui_within/4,
);

/// The exports that make or free an object, or switch a process-wide setting: each is called on objects it may free (null and a
/// fresh one), and what it makes is freed.
const SPECIAL: &[&str] = &["pxui_ui_create", "pxui_ui_create_with_fonts", "pxui_ui_destroy", "pxui_shaper_create", "pxui_shaper_destroy", "pxui_text_fork",
    "pxui_theme_create", "pxui_theme_destroy", "pxui_prepared_destroy", "pxui_text_worker_create", "pxui_text_worker_destroy",
    "pxui_renderer_destroy", "pxui_a11y_start", "pxui_a11y_stop"];

fn special() {
    for k in 0..VARIANTS {
        let panics = PANICS.with(Cell::get);
        unsafe {
            reset();
            for made in [pxui_ui_create(Arg::arg(k)), pxui_ui_create_with_fonts(Arg::arg(k), Arg::arg(k + 1), Arg::arg(k + 2))] {
                if !made.is_null() { assert_eq!(pxui_ui_destroy(made), 0); }
            }
            assert_eq!(pxui_ui_destroy(core::ptr::null_mut()), 0);
            reset();
            let shaper = pxui_shaper_create(Arg::arg(k), Arg::arg(k + 1));
            pxui_shaper_destroy(shaper);
            let fork = pxui_text_fork(Arg::arg(k));
            pxui_shaper_destroy(fork);
            reset();
            let theme = pxui_theme_create(Arg::arg(k));
            pxui_theme_destroy(theme);
            pxui_prepared_destroy(core::ptr::null_mut());
            pxui_renderer_destroy(core::ptr::null_mut());
            pxui_a11y_stop(core::ptr::null_mut());
            // a name that is refused before any bus is reached
            assert!(pxui_a11y_start(Str { ptr: core::ptr::null(), len: 5 }).is_null());
        }
        check("special", &0i32, panics);
    }
    unsafe { pxui_text_worker_destroy(pxui_text_worker_create()); }
    // a poisoned shaper or theme answers its failure value to every call, with any arguments, and is still freed
    let font = unsafe { pxui_default_font() };
    let (shaper, theme) = unsafe { (pxui_shaper_create(&font, 1), pxui_theme_create(core::ptr::null())) };
    unsafe { (*shaper).poisoned = true; (*theme).poisoned = true; }
    let style = PxuiTextStyle::default();
    let mut metrics = TextMetrics::default();
    for k in 0..VARIANTS {
        let panics = PANICS.with(Cell::get);
        unsafe {
            assert_eq!(pxui_shaper_width(shaper, 0, Str { ptr: b"a".as_ptr(), len: 1 }, &style, 0.0, &mut metrics), PXUI_ERR_PANIC);
            assert_eq!(pxui_shaper_width(shaper, Arg::arg(k), Arg::arg(k + 1), Arg::arg(k + 2), Arg::arg(k + 3), Arg::arg(k + 4)), PXUI_ERR_PANIC);
            assert_eq!(pxui_shaper_add_font(shaper, font), u32::MAX);
            assert_eq!(pxui_shaper_system_fonts(shaper), PXUI_ERR_PANIC);
            assert_eq!(pxui_theme_parse(theme, Str { ptr: b"tokens.accent = #ff0000".as_ptr(), len: 23 }), PXUI_ERR_PANIC);
            assert_eq!(pxui_theme_parse(theme, Arg::arg(k)), PXUI_ERR_PANIC);
        }
        check("a poisoned shaper or theme", &0i32, panics);
    }
    unsafe { pxui_shaper_destroy(shaper); pxui_theme_destroy(theme); }
}

fn hook() {
    static HOOK: std::sync::Once = std::sync::Once::new();
    HOOK.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| { PANICS.with(|p| p.set(p.get() + 1)); previous(info) }));
    });
}

extern "C" fn fuzz_in_frame(_: *mut c_void, ui: *mut c_void) {
    let mut live = PxuiBox::default();
    assert_eq!(unsafe { pxui_ui_node_keyed(ui.cast(), 4343, &Node::DEFAULT, &mut live) }, 0);
    BOXES.with(|b| { let [stale, foreign, _] = b.get(); b.set([stale, foreign, live]); });
    let owner = CUR.with(|c| c.replace(ui.cast()));
    fuzz_all();
    CUR.with(|c| c.set(owner));
}

/// Every export, outside a frame and inside one, with every variant of its arguments: no panic, no `PXUI_ERR_PANIC`, no poison.
#[test]
fn every_export_refuses_bad_arguments_without_panicking() {
    hook();
    let ui = unsafe { pxui_ui_create(&PxuiUiDesc::default()) };
    CUR.with(|c| c.set(ui));
    SHAPER.with(|s| s.set({ let font = unsafe { pxui_default_font() }; unsafe { pxui_shaper_create(&font, 1) } }));
    THEME.with(|t| t.set(unsafe { pxui_theme_create(core::ptr::null()) }));
    // a stale box (declared in one frame, gone in the next) and a box of another `Ui`, kept alive meanwhile
    extern "C" fn declare(user: *mut c_void, ui: *mut c_void) { assert_eq!(unsafe { pxui_ui_node_keyed(ui.cast(), 4242, &Node::DEFAULT, &mut *user.cast::<PxuiBox>()) }, 0); }
    let input = FrameIn { width: 320.0, height: 240.0, ..FrameIn::default() };
    let mut out = FrameOut::default();
    let (mut stale, mut foreign) = (PxuiBox::default(), PxuiBox::default());
    let other = unsafe { pxui_ui_create(&PxuiUiDesc::default()) };
    unsafe {
        assert_eq!(pxui_frame(ui, &input, Some(declare), (&mut stale as *mut PxuiBox).cast(), &mut out), 0);
        assert_eq!(pxui_frame(ui, &input, None, core::ptr::null_mut(), &mut out), 0);
        assert_eq!(pxui_frame(other, &input, Some(declare), (&mut foreign as *mut PxuiBox).cast(), &mut out), 0);
    }
    BOXES.with(|b| b.set([stale, foreign, stale]));
    special();
    fuzz_all();
    let calls = unsafe { pxui_ui_calls(ui) };
    for _ in 0..2 {
        let panics = PANICS.with(Cell::get);
        let rc = unsafe { pxui_frame(ui, &input, Some(fuzz_in_frame), core::ptr::null_mut(), &mut out) };
        check("pxui_frame (the fuzzed frame)", &rc, panics);
    }
    // the frames ran the fuzz: every export at least once a round on the borrowed handle (counted on this `Ui`)
    assert!(unsafe { pxui_ui_calls(ui) } - calls > (FUZZED.len() as u64) * u64::from(VARIANTS));
    unsafe {
        pxui_shaper_destroy(SHAPER.with(Cell::get));
        pxui_theme_destroy(THEME.with(Cell::get));
        assert_eq!(pxui_ui_destroy(ui) | pxui_ui_destroy(other), 0);
    }
    let failures = FAILURES.with(|f| core::mem::take(&mut *f.borrow_mut()));
    assert!(failures.is_empty(), "{} failing calls:\n{}", failures.len(), failures.join("\n"));
}

/// A label in the 0x01 resolved form that carries 0 or 1, or is malformed, is refused with `PXUI_ERR_ARGUMENT` by every export that takes a
/// key label (creation, scope, keyed record, image, lookup, hover wait, and each legacy label-keyed call): never the engine's reserved-key
/// assertion, never hashed as text.
#[test]
fn unusable_resolved_labels_are_refused_with_the_argument_error() {
    hook();
    extern "C" fn build(_: *mut c_void, ui: *mut c_void) {
        let ui: *mut PxuiUi = ui.cast();
        let panics = PANICS.with(Cell::get);
        for label in UNRESOLVED {
            let key = Str { ptr: label.as_ptr(), len: 17 };
            let keyed = PxuiKeyedDesc { size: core::mem::size_of::<PxuiKeyedDesc>() as u32, reserved: 0, key, name: Str { ptr: b"n".as_ptr(), len: 1 } };
            let (mut b, mut flag) = (PxuiBox::default(), 0u8);
            let image = VALID_IMAGE;
            let px = PX;
            unsafe {
                assert_eq!(pxui_ui_node(ui, key, &Node::DEFAULT, &mut b), PXUI_ERR_ARGUMENT);
                assert_eq!(pxui_scope(ui, key, None, core::ptr::null_mut()), PXUI_ERR_ARGUMENT);
                assert_eq!(pxui_ui_progress(ui, &keyed, 0, 0.0, &mut b), PXUI_ERR_ARGUMENT);
                assert_eq!(pxui_ui_image(ui, key, keyed.name, &image, 0, px, px, &mut b), PXUI_ERR_ARGUMENT);
                assert_eq!(pxui_ui_expanded(ui, key, &mut flag), PXUI_ERR_ARGUMENT);
                assert_eq!(pxui_ui_hover_delay(ui, key, &mut flag), PXUI_ERR_ARGUMENT);
            }
        }
        // each legacy label-keyed export, called with arguments it takes with a plain label, refuses the unusable resolved ones
        for (name, call) in LEGACY_KEYED {
            let failed = |what: String| FAILURES.with(|f| f.borrow_mut().push(format!("{name}: {what}")));
            let rc = call(ui, Str { ptr: b"k".as_ptr(), len: 1 });
            if rc == PXUI_ERR_ARGUMENT { failed(format!("a plain label refused: {}", lent(unsafe { pxui_last_error(ui) }))); }
            for label in UNRESOLVED { let rc = call(ui, Str { ptr: label.as_ptr(), len: 17 }); if rc != PXUI_ERR_ARGUMENT { failed(format!("{label:?} answered {rc}")); } }
        }
        // each keyed-record export (`read_keyed`, whose key's resolved form is checked once, by the entry or the export), likewise
        for (name, call) in KEYED {
            let failed = |what: String| FAILURES.with(|f| f.borrow_mut().push(format!("{name}: {what}")));
            let rc = call(ui, Str { ptr: b"k".as_ptr(), len: 1 });
            if rc == PXUI_ERR_ARGUMENT { failed(format!("a plain key refused: {}", lent(unsafe { pxui_last_error(ui) }))); }
            for label in UNRESOLVED { let rc = call(ui, Str { ptr: label.as_ptr(), len: 17 }); if rc != PXUI_ERR_ARGUMENT { failed(format!("{label:?} answered {rc}")); } }
        }
        // a usable resolved label is still taken as the key itself
        let mut b = PxuiBox::default();
        assert_eq!(unsafe { pxui_ui_image(ui, Str { ptr: b"img".as_ptr(), len: 3 }, Str::default(), &VALID_IMAGE, 0, PX, PX, &mut b) }, 0);
        assert_eq!(unsafe { pxui_ui_node(ui, Str { ptr: b"\x010000000000000abc".as_ptr(), len: 17 }, &Node::DEFAULT, &mut b) }, 0);
        assert_eq!(b.box_key, 0xabc);
        assert_eq!(PANICS.with(Cell::get), panics, "no call panicked");
        assert!(!unsafe { (*(*ui).shared).poisoned });
    }
    let ui = unsafe { pxui_ui_create(&PxuiUiDesc::default()) };
    let input = FrameIn { width: 320.0, height: 240.0, ..FrameIn::default() };
    let mut out = FrameOut::default();
    assert_eq!(unsafe { pxui_frame(ui, &input, Some(build), core::ptr::null_mut(), &mut out) }, 0);
    assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
    let failures = FAILURES.with(|f| core::mem::take(&mut *f.borrow_mut()));
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The text of a string the library lends.
fn lent(s: Str) -> String { if s.ptr.is_null() { String::new() } else { String::from_utf8_lossy(unsafe { core::slice::from_raw_parts(s.ptr, s.len) }).into_owned() } }

/// A zeroed response record with its size.
fn response() -> PxuiResponse { PxuiResponse { size: core::mem::size_of::<PxuiResponse>() as u32, ..unsafe { core::mem::zeroed() } } }
/// A zeroed menu-bar output with its size.
fn menu_out() -> PxuiMenuOut { PxuiMenuOut { size: core::mem::size_of::<PxuiMenuOut>() as u32, ..unsafe { core::mem::zeroed() } } }

/// Every legacy export whose label argument is the key, each called with otherwise valid arguments and `label`.
type LegacyCall = fn(*mut PxuiUi, Str) -> i32;
const PX: PxuiSizeDesc = PxuiSizeDesc { kind: 0, value: 10.0, relative: None };
/// An image the image calls take (its texture is only an identity here: nothing is drawn).
const VALID_IMAGE: pxui_types::Image = pxui_types::Image {
    size: core::mem::size_of::<pxui_types::Image>() as u32, vk_image_view: core::ptr::null_mut(), mtl_texture: core::ptr::dangling_mut(), width: 1, height: 1,
    format: pxui_types::Image::FORMAT_RGBA8,
};
/// A keyed record of `key`, named `n`.
fn keyed(key: Str) -> PxuiKeyedDesc { PxuiKeyedDesc { size: core::mem::size_of::<PxuiKeyedDesc>() as u32, reserved: 0, key, name: Str { ptr: b"n".as_ptr(), len: 1 } } }
/// Every export that takes its key in a `PxuiKeyedDesc`, each called with otherwise valid arguments and `key`.
static KEYED: &[(&str, LegacyCall)] = &[
        ("pxui_ui_panel", |ui, k| unsafe { pxui_ui_panel(ui, &keyed(k), &PxuiPanel { width: 100.0, ..PxuiPanel::default() }, None, core::ptr::null_mut(), &mut PxuiBox::default()) }),
        ("pxui_ui_view", |ui, k| unsafe { pxui_ui_view(ui, &PxuiKeyedDesc { name: Str::default(), ..keyed(k) }, 0, None, core::ptr::null_mut(), &mut PxuiBox::default()) }),
        ("pxui_ui_scroll_view", |ui, k| unsafe { pxui_ui_scroll_view(ui, &keyed(k), &Node::DEFAULT, None, core::ptr::null_mut(), &mut PxuiBox::default()) }),
        ("pxui_ui_modal", |ui, k| { let d = PxuiSheetDesc { size: core::mem::size_of::<PxuiSheetDesc>() as u32, ..unsafe { core::mem::zeroed() } }; unsafe { pxui_ui_modal(ui, &keyed(k), &d, None, core::ptr::null_mut(), &mut 0, &mut PxuiBox::default()) } }),
        ("pxui_ui_anchored", |ui, k| { let d = PxuiAnchorDesc { has_anchor: 1, ..unsafe { core::mem::zeroed() } }; unsafe { pxui_ui_anchored(ui, &keyed(k), &d, PX, PX, None, core::ptr::null_mut(), &mut PxuiBox::default()) } }),
        ("pxui_ui_popup", |ui, k| { let d = PxuiPopupDesc { size: core::mem::size_of::<PxuiPopupDesc>() as u32, width: 10.0, height: 10.0, ..unsafe { core::mem::zeroed() } }; unsafe { pxui_ui_popup(ui, &keyed(k), &d, None, core::ptr::null_mut(), &mut 0, &mut PxuiBox::default()) } }),
        ("pxui_ui_toasts", |ui, k| unsafe { pxui_ui_toasts(ui, &keyed(k), Slice { ptr: core::ptr::null(), len: 0 }, Slice { ptr: core::ptr::null(), len: 0 }, 1.0, &mut 0) }),
        ("pxui_ui_slider", |ui, k| unsafe { pxui_ui_slider(ui, &keyed(k), 0.5, 0.0, 1.0, &mut response(), &mut PxuiScrubOut::default()) }),
        ("pxui_ui_spinner", |ui, k| unsafe { pxui_ui_spinner(ui, &keyed(k), &mut PxuiBox::default()) }),
        ("pxui_ui_badge", |ui, k| unsafe { pxui_ui_badge(ui, &keyed(k), Str { ptr: b"1".as_ptr(), len: 1 }, &mut PxuiBox::default()) }),
        ("pxui_ui_disclosure", |ui, k| unsafe { pxui_ui_disclosure(ui, &keyed(k), 0, None, core::ptr::null_mut(), &mut response()) }),
        ("pxui_ui_pick", |ui, k| unsafe { pxui_ui_pick(ui, &keyed(k), 0, Slice { ptr: core::ptr::null(), len: 0 }, 0, 0, &mut response(), &mut { PxuiSegmentedResult::EMPTY }) }),
        ("pxui_ui_section_index", |ui, k| unsafe { pxui_ui_section_index(ui, &keyed(k), Slice { ptr: core::ptr::null(), len: 0 }, &mut response(), &mut { PxuiSegmentedResult::EMPTY }) }),
        ("pxui_ui_picker", |ui, k| unsafe { pxui_ui_picker(ui, &keyed(k), 8, 0, 0, Str::default(), None, None, None, None, core::ptr::null_mut(), &mut Str::default(), &mut 0, &mut 0) }),
];
static LEGACY_KEYED: &[(&str, LegacyCall)] = &[
        ("pxui_ui_grip", |ui, l| unsafe { pxui_ui_grip(ui, l, 0, 0, geom::Rect::default(), &mut PxuiBox::default(), &mut PxuiSignal::default()) }),
        ("pxui_ui_grid_list", |ui, l| { let d = PxuiGridListDesc { label: l, cell_w: 10.0, cell_h: 10.0, size: core::mem::size_of::<PxuiGridListDesc>() as u32, ..unsafe { core::mem::zeroed() } }; unsafe { pxui_ui_grid_list(ui, &d, &mut PxuiBox::default()) } }),
        ("pxui_ui_split", |ui, l| { let d = PxuiSplitDesc { label: l, ratio: 0.5, size: core::mem::size_of::<PxuiSplitDesc>() as u32, ..unsafe { core::mem::zeroed() } }; unsafe { pxui_ui_split(ui, &d, &mut response(), &mut 0, &mut 0.0) } }),
        ("pxui_ui_menu_bar", |ui, l| { let d = PxuiMenuBarDesc { label: l, size: core::mem::size_of::<PxuiMenuBarDesc>() as u32, ..unsafe { core::mem::zeroed() } }; unsafe { pxui_ui_menu_bar(ui, &d, &mut menu_out(), &mut 0) } }),
        ("pxui_ui_tree", |ui, l| { let (mut selected, mut len) = (0usize, 0usize); let d = PxuiTreeViewDesc { label: l, selected: &mut selected, visible_len: &mut len, size: core::mem::size_of::<PxuiTreeViewDesc>() as u32, ..unsafe { core::mem::zeroed() } }; unsafe { pxui_ui_tree(ui, &d, &mut 0) } }),
        ("pxui_ui_scrub", |ui, l| unsafe { pxui_ui_scrub(ui, l, 0.0, 0.0, 1.0, 0.1, &mut response(), &mut PxuiScrubOut::default()) }),
        ("pxui_ui_search_field", |ui, l| unsafe { pxui_ui_search_field(ui, l, Str::default(), Str::default(), Str::default(), Str::default(), 2, &mut 0, &mut Str::default(), &mut response()) }),
        ("pxui_ui_line_field", |ui, l| unsafe { pxui_ui_line_field(ui, 0.0, 0.0, 10.0, 10.0, 0, Str::default(), l, Str::default(), Str::default(), &mut Str::default(), &mut 0, &mut response()) }),
        ("pxui_ui_text_area", |ui, l| { let d = PxuiTextAreaDesc { label: l, size: core::mem::size_of::<PxuiTextAreaDesc>() as u32, ..unsafe { core::mem::zeroed() } }; unsafe { pxui_ui_text_area(ui, &d, &Node::DEFAULT, core::ptr::null(), Str::default(), &mut Str::default(), &mut 0, &mut 0, &mut response()) } }),
        ("pxui_ui_value_field", |ui, l| { let d = PxuiValueFieldDesc { label: l, width: 10.0, height: 10.0, size: core::mem::size_of::<PxuiValueFieldDesc>() as u32, ..unsafe { core::mem::zeroed() } }; unsafe { pxui_ui_value_field(ui, &d, None, None, None, core::ptr::null_mut(), Str::default(), &mut Str::default(), &mut 0, &mut 0, &mut response()) } }),
        ("pxui_ui_context_menu", |ui, l| { let e = Slice { ptr: core::ptr::null(), len: 0 }; unsafe { pxui_ui_context_menu(ui, 0.0, 0.0, l, e, Slice { ptr: core::ptr::null(), len: 0 }, 0, 0.0, 0, 0, core::ptr::null(), &mut 0, &mut 0) } }),
];

/// The fuzz calls every export the manifests name: a new export without a row here fails.
#[test]
fn every_manifest_export_is_fuzzed() {
    let manifests = [include_str!("../../../../abi/pxui.v1.txt"), include_str!("../../../../abi/pxui.ext.txt")];
    let mut missing: Vec<&str> = manifests.iter().flat_map(|m| m.lines()).filter_map(|line| line.strip_prefix("function ")).filter_map(|rest| rest.split(' ').next())
        .filter(|name| !FUZZED.contains(name) && !SPECIAL.contains(name)).collect();
    missing.sort_unstable();
    missing.dedup();
    assert!(missing.is_empty(), "exports the fuzz does not call: {missing:?}");
}
