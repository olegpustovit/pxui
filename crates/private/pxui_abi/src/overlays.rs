use crate::state::{MAX_TOTAL_TEXT_BYTES, PxuiUi, checked, fail, finite, get, strings};
use crate::{PXUI_ERR_ARGUMENT, PxuiBox, Str};
use core::{cell::Cell, ffi::c_void};
use pxui_core::ui_text::{Menu, MenuItem, Chosen, Pick, PickerOpts, PickRows};
use pxui_types::{ffi::PxuiKeyedDesc, ui::Toast};
use pxui_types::ui::Key as NodeKey;
use pxui_types::ffi::{PxuiMenuOptions, PxuiMenuRow, Slice, Strided};

const MAX_ROWS: usize = 4096;

pub type PickerRows = Option<extern "C" fn(*mut c_void, Str, *mut Slice<pxui_types::ffi::PxuiPickRow>)>;
pub type PickerMark = Option<extern "C" fn(*mut c_void, i64, *mut pxui_types::geom::Rgba) -> u8>;
pub type PickerOff = Option<extern "C" fn(*mut c_void, i64) -> u8>;
/// Row `row`'s image into `out` (1) or none (0) (ABI 1.110).
pub type PickerImage = Option<extern "C" fn(*mut c_void, i64, *mut pxui_types::Image) -> u8>;

export!(trusted pub unsafe fn pxui_ui_tip(ui: *mut PxuiUi, box_: *const PxuiBox, label: Str) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if box_.is_null() { return unsafe { fail(ui, "null tooltip box") }; }
    let Some(label) = (unsafe { checked(label) }) else { return unsafe { fail(ui, "invalid tooltip arguments") }; };
    unsafe { crate::trusted::tooltip(ui, &*box_, label) }
});

/// Toasts of `items` keyed by `key`, each by its id in `ids` (as many), the body of `pxui_ui_toasts`; up to 32 on the stack, more in a
/// buffer. `out_index` is written only when the call is not refused.
unsafe fn toasts(ui: *mut PxuiUi, key: &str, items: Slice<Str>, ids: Slice<i64>, seconds: f64, out_index: *mut i64) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out_index.is_null() || !seconds.is_finite() || seconds < 0.0 || ids.len != items.len || (ids.len != 0 && ids.ptr.is_null()) {
        return unsafe { fail(ui, "invalid toast arguments") };
    }
    let mut list = state.shared().take_strs();
    if !unsafe { strings(items, MAX_ROWS, &mut list) } { state.shared().put_strs(list); return unsafe { fail(ui, "invalid toast strings") }; }
    let ids = if ids.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(ids.ptr, ids.len) } };
    let toast = |i: usize| Toast::new(ids[i], list[i]);
    let mut stack = [Toast::new(0, ""); 32];
    let heap: Vec<Toast>;
    let items: &[Toast] = if list.len() <= 32 {
        for (i, t) in stack[..list.len()].iter_mut().enumerate() { *t = toast(i); }
        &stack[..list.len()]
    } else {
        heap = (0..list.len()).map(toast).collect();
        &heap
    };
    let mut gone = None;
    let rc = unsafe { crate::trusted::toasts(ui, key, items, seconds, &mut gone) };
    state.shared().put_strs(list);
    if rc == 0 { unsafe { *out_index = gone.map_or(-1, |index| index as i64); } }
    rc
}

// ABI 1.68: toasts keyed by `keyed.key` (the stack's accessible name `keyed.name`), toast `i` keyed by `ids[i]` (as many as `items`).
export!(build pub unsafe fn pxui_ui_toasts(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, items: Slice<Str>, ids: Slice<i64>, seconds: f64, out_index: *mut i64) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some((key, _name)) = (unsafe { crate::widgets::read_keyed(keyed) }) else { return unsafe { fail(ui, "invalid keyed descriptor") } };
    unsafe { toasts(ui, key, items, ids, seconds, out_index) }
});

// A context menu at (`x`, `y`): `rows` (strided, each read at its own size, at most `MAX_ROWS`) with the options `options` (null: the
// theme's), answering the row chosen (`out_tag` 1, `out_index` the row's position), a dismissal (2) or neither (0).
export!(trusted pub unsafe fn pxui_ui_context_menu(
    ui: *mut PxuiUi, x: f32, y: f32, label: Str, rows: Strided<PxuiMenuRow>, has_width: u8, width: f32, options: *const PxuiMenuOptions,
    out_tag: *mut u32, out_index: *mut i64,
) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out_tag.is_null() || out_index.is_null() || !finite(&[x, y]) || has_width > 1 || (has_width != 0 && (!width.is_finite() || width < 0.0))
        || rows.len > MAX_ROWS || !rows.is_valid() {
        return unsafe { fail(ui, "invalid context menu arguments") };
    }
    let Some(label) = (unsafe { crate::state::checked_label(label) }) else { return unsafe { fail(ui, "invalid context menu strings") }; };
    // the rows borrow the caller's strings for this call, in vectors reused between frames
    let (mut menu, mut subs) = { let shared = state.shared(); (core::mem::take(&mut shared.menu), core::mem::take(&mut shared.menu_sub)) };
    let mut valid = unsafe { decode_rows(&rows, &mut menu, &mut subs) };
    let mut menu_options = Menu::DEFAULT;
    if has_width != 0 { menu_options = menu_options.w_px(width); }
    if valid && !options.is_null() {
        // the whole record (its size, or more): an earlier size is refused
        valid = unsafe { (*options).size } as usize >= core::mem::size_of::<PxuiMenuOptions>() && unsafe { decode_options(&*options, &mut menu_options) };
    }
    let variant = if options.is_null() || !valid { 0 } else { unsafe { (*options).variant } };
    let mut result = Chosen::default();
    let status = if valid { with_variant!(ui, pxui_types::theme::BodyKind::menu_row, variant, unsafe { crate::trusted::menu(ui, NodeKey::Label(label), (x, y), &menu, &menu_options, &mut result) }) } else { 0 };
    menu.clear();
    subs.clear();
    { let shared = state.shared(); (shared.menu, shared.menu_sub) = (menu, subs); }
    if !valid { return unsafe { fail(ui, "invalid context menu rows or options") }; }
    if status != 0 { return status; }
    let (tag, index) = match (result.dismissed, result.chosen) { (true, _) => (2, -1), (_, Some(index)) => (1, index as i64), _ => (0, -1) };
    unsafe { *out_tag = tag; *out_index = index; }
    0
});

/// `row` as a menu item, borrowing the caller's strings: `None` for a short record, a flag past 1, a mark past 2, a non-zero reserved byte
/// or a string that is not UTF-8.
unsafe fn menu_item(row: &PxuiMenuRow) -> Option<MenuItem<'static>> {
    if (row.size as usize) < core::mem::size_of::<PxuiMenuRow>() || [row.enabled, row.danger, row.selected, row.has_swatch, row.has_id].iter().any(|flag| *flag > 1)
        || row.checked > 2 || row.reserved != [0; 2] { return None; }
    let (text, shortcut) = (unsafe { checked(row.text) }?, unsafe { checked(row.shortcut) }?);
    let mut item = MenuItem::new(text).disabled(row.enabled == 0).shortcut(shortcut);
    item.danger = row.danger == 1;
    item.selected = row.selected == 1;
    item.checked = (row.checked != 0).then_some(row.checked == 2);
    item.swatch = (row.has_swatch == 1).then_some(row.swatch);
    item.id = (row.has_id == 1).then_some(row.id);
    Some(item)
}

/// The rows of `rows` as menu items in `menu`, the submenus' rows flat in `subs` (at most 64 submenus, `MAX_ROWS` rows each, a submenu's
/// rows with none of their own): false when a record is invalid. Each opening row points into `subs` once it is complete (it does not
/// grow after that).
unsafe fn decode_rows(rows: &Strided<PxuiMenuRow>, menu: &mut Vec<MenuItem<'static>>, subs: &mut Vec<MenuItem<'static>>) -> bool {
    menu.clear();
    subs.clear();
    let mut at = [(0usize, 0usize, 0usize); 64];
    let mut opening = 0;
    for i in 0..rows.len {
        let row = unsafe { rows.at(i) };
        let Some(item) = (unsafe { menu_item(row) }) else { return false };
        menu.push(item);
        if row.submenu.len == 0 { continue; }
        let sub = &row.submenu;
        if opening == at.len() || sub.len > MAX_ROWS || !sub.is_valid() { return false; }
        let start = subs.len();
        for j in 0..sub.len {
            let child = unsafe { sub.at(j) };
            let Some(item) = (unsafe { menu_item(child) }).filter(|_| child.submenu.len == 0) else { return false };
            subs.push(item);
        }
        at[opening] = (i, start, sub.len);
        opening += 1;
    }
    for (row, start, count) in &at[..opening] {
        // the pointer outlives the borrow: `subs` is neither pushed to nor dropped until the call that reads the rows has returned
        menu[*row].submenu = unsafe { core::slice::from_raw_parts(subs.as_ptr().add(*start), *count) };
    }
    true
}

/// Applies a menu's options (`options` read whole, its `size` checked by the caller): false when a number or flag is invalid.
unsafe fn decode_options(options: &PxuiMenuOptions, menu: &mut Menu<'static>) -> bool {
    if !options.row_h.is_finite() || options.row_h < 0.0 || !(0.0..=f32::MAX).contains(&options.padding) || options.has_name > 1 || options.focus_on_click > 1
        || options.reserved != 0 || options.reserved2 != [0; 6] || !(0.0..=f32::MAX).contains(&options.max_h) || options.reserved3 != 0 { return false; }
    if options.row_h > 0.0 { *menu = menu.row_h(options.row_h); }
    if options.max_rows > 0 { *menu = menu.max_rows(options.max_rows as usize); }
    *menu = menu.padding(options.padding).focus_on_click(options.focus_on_click == 1);
    if options.has_name == 1 {
        let Some(name) = (unsafe { checked(options.name) }) else { return false };
        *menu = menu.name(name);
    }
    if options.max_h > 0.0 { *menu = menu.max_h(options.max_h); }
    true
}

/// The picker keyed by `key`, `placeholder` its placeholder and name: the body of `pxui_ui_picker`.
#[allow(clippy::too_many_arguments)]
unsafe fn picker(
    ui: *mut PxuiUi, limit: i64, slash: u8, at_rest: u8, key: &str, placeholder: &str, query: Str,
    rows_cb: PickerRows, mark_cb: PickerMark, off_cb: PickerOff, image_cb: PickerImage, user: *mut c_void, out_query: *mut Str, out_tag: *mut u32, out_index: *mut i64,
) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out_query.is_null() || out_tag.is_null() || out_index.is_null() || !(1..=1024).contains(&limit) || slash > 1 || at_rest > 1 {
        return unsafe { fail(ui, "invalid picker arguments") };
    }
    let Some(query) = (unsafe { checked(query) }) else { return unsafe { fail(ui, "invalid picker strings") }; };
    let invalid = Cell::new(false);
    // The engine keeps the rows between frames; each call pushes them in place, reusing their strings.
    let mut fill = |query: &str, rows: &mut PickRows| {
        let mut span = Slice { ptr: core::ptr::null(), len: 0 };
        if let Some(callback) = rows_cb { callback(user, Str { ptr: query.as_ptr(), len: query.len() }, &mut span); }
        const ROW: usize = core::mem::size_of::<pxui_types::ffi::PxuiPickRow>();
        if span.len > MAX_ROWS || (span.len != 0 && span.ptr.is_null()) { invalid.set(true); return; }
        // the first row's leading size before the span is taken as rows: a caller built before 1.86 fills 16-byte `PxuiStr`s (twice
        // as many), whose buffer is shorter than `len` rows and whose pointers must never be read as rows
        if span.len != 0 && unsafe { span.ptr.cast::<u32>().read_unaligned() } as usize != ROW { invalid.set(true); return; }
        let values = if span.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(span.ptr, span.len) } };
        let mut total = 0usize;
        // every row checked once before it is pushed: its size and reserved bytes first, then its art kind and value and texts
        for row in values {
            if row.size as usize != ROW || row.reserved != [0; 3] { invalid.set(true); return rows.clear(); }
            total = total.saturating_add(row.label.len).saturating_add(row.detail.len).saturating_add(row.heading.len);
            let art = match (row.art_kind, row.art) {
                (0, 0) => Some(None),
                (1, code) => u32::try_from(code).ok().and_then(char::from_u32).map(|c| Some(pxui_types::ui::PickArt::Glyph(c))),
                (2, id) => usize::try_from(id).ok().map(|id| Some(pxui_types::ui::PickArt::Image(id))),
                (3, word) => u32::try_from(word).ok().map(|word| Some(pxui_types::ui::PickArt::Draw(word))),
                _ => None,
            };
            let texts = (unsafe { checked(row.label) }, unsafe { checked(row.detail) }, unsafe { checked(row.heading) });
            let (Some(art), Some(label), Some(detail), Some(heading)) = (art, texts.0, texts.1, texts.2) else { invalid.set(true); return rows.clear(); };
            if total > MAX_TOTAL_TEXT_BYTES { invalid.set(true); return rows.clear(); }
            rows.push(pxui_types::ui::PickRow { label, detail, art, heading });
        }
    };

    // the closures run only during this call, so they may hold `user`
    let user_addr = user as usize;
    let mark = mark_cb.map(|cb| move |row: i64| { let mut c = pxui_types::geom::Rgba::default(); (cb(user_addr as *mut c_void, row, &mut c) != 0).then_some(c) });
    let off = off_cb.map(|cb| move |row: i64| cb(user_addr as *mut c_void, row) != 0);
    // a row's image: the caller's descriptor checked, then the `Ui`'s identity for it (a kept handle: nothing allocated once seen); an
    // invalid descriptor refuses the call
    let shared = state.shared;
    let invalid_image = &invalid;
    let image = image_cb.map(|cb| move |row: i64| {
        let mut raw = pxui_types::Image::default();
        if cb(user_addr as *mut c_void, row, &mut raw) == 0 { return None; }
        let image = crate::image::core_image(raw, unsafe { &mut (*shared).images });
        if image.is_none() { invalid_image.set(true); }
        image
    });
    let opts = PickerOpts { limit, slash: slash != 0, at_rest: at_rest != 0 };
    let fns = (mark.as_ref().map(|f| f as &dyn Fn(i64) -> Option<pxui_types::geom::Rgba>), off.as_ref().map(|f| f as &dyn Fn(i64) -> bool),
        image.as_ref().map(|f| f as &dyn Fn(i64) -> Option<pxui_core::ui::Image>));
    let r = unsafe { (&mut *state.engine).picker(key, placeholder, query, &opts, fns, &mut fill) };
    let (tag, index) = match r.pick { Pick::None => (0, -1), Pick::Pick(index) => (1, index), Pick::Delete(index) => (2, index), Pick::Submit => (3, -1), Pick::Back => (4, -1), Pick::Cancel => (5, -1) };
    if invalid.get() { return unsafe { fail(ui, "invalid picker rows") }; }
    // lent from a buffer reused next frame: no per-frame copy of the query (#82)
    let Ok(query) = state.lend(r.query) else { return crate::PXUI_ERR_STATE };
    unsafe { *out_query = query; }
    unsafe { *out_tag = tag; *out_index = index; }
    0
}

// ABI 1.68: the picker keyed by `keyed.key`, `keyed.name` its placeholder and accessible name.
export!(build pub unsafe fn pxui_ui_picker(
    ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, limit: i64, slash: u8, at_rest: u8, query: Str,
    rows_cb: PickerRows, mark_cb: PickerMark, off_cb: PickerOff, image_cb: PickerImage, user: *mut c_void, out_query: *mut Str, out_tag: *mut u32, out_index: *mut i64,
) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some((key, placeholder)) = (unsafe { crate::widgets::read_keyed(keyed) }).filter(|(key, _)| !crate::state::unusable(key)) else { return unsafe { fail(ui, "invalid keyed descriptor") } };
    unsafe { picker(ui, limit, slash, at_rest, key, placeholder, query, rows_cb, mark_cb, off_cb, image_cb, user, out_query, out_tag, out_index) }
});
