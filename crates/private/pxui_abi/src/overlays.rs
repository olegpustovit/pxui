use crate::state::{MAX_TOTAL_TEXT_BYTES, PxuiUi, checked, core_box, fail, finite, get, strings};
use crate::{PXUI_ERR_ARGUMENT, PxuiBox, Str};
use core::{cell::Cell, ffi::c_void};
use pxui_core::ui_text::{Menu, MenuItem, Chosen, Pick, PickerOpts, PickRows};
use pxui_types::{ffi::PxuiKeyedDesc, ui::Toast};
use pxui_types::ui::Key as NodeKey;
use pxui_types::ffi::{PxuiMenuExtra, Slice};

const MAX_ROWS: usize = 4096;

pub type PickerRows = Option<extern "C" fn(*mut c_void, Str, *mut Slice<pxui_types::ffi::PxuiPickRow>)>;
pub type PickerMark = Option<extern "C" fn(*mut c_void, i64, *mut pxui_types::geom::Rgba) -> u8>;
pub type PickerOff = Option<extern "C" fn(*mut c_void, i64) -> u8>;
/// Row `row`'s image into `out` (1) or none (0) (ABI 1.110).
pub type PickerImage = Option<extern "C" fn(*mut c_void, i64, *mut pxui_types::Image) -> u8>;

export!(trusted pub unsafe fn pxui_ui_tip(ui: *mut PxuiUi, box_: *const PxuiBox, label: Str) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if box_.is_null() { return unsafe { fail(ui, "null tooltip box") }; }
    let (Some(label), Some(_)) = (unsafe { checked(label) }, core_box(unsafe { *box_ })) else { return unsafe { fail(ui, "invalid tooltip arguments") }; };
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

export!(trusted pub unsafe fn pxui_ui_context_menu(
    ui: *mut PxuiUi, x: f32, y: f32, label: Str, items: Slice<Str>, enabled: Slice<u8>,
    has_width: u8, width: f32, has_selected: u8, selected: i64, extra: *const PxuiMenuExtra,
    out_tag: *mut u32, out_index: *mut i64,
) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out_tag.is_null() || out_index.is_null() || !finite(&[x, y]) || has_width > 1 || has_selected > 1
        || (has_width != 0 && (!width.is_finite() || width < 0.0)) || items.len > MAX_ROWS
        || enabled.len != items.len || (enabled.len != 0 && enabled.ptr.is_null()) {
        return unsafe { fail(ui, "invalid context menu arguments") };
    }
    let mut list = state.shared().take_strs();
    let Some(label) = (unsafe { crate::state::checked_label(label) }).filter(|_| unsafe { strings(items, MAX_ROWS, &mut list) }) else {
        state.shared().put_strs(list);
        return unsafe { fail(ui, "invalid context menu strings") };
    };
    let enabled = if enabled.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(enabled.ptr, enabled.len) } };
    if enabled.iter().any(|value| *value > 1) {
        state.shared().put_strs(list);
        return unsafe { fail(ui, "invalid context menu enabled flags") };
    }
    // the rows borrow the caller's strings for this call, in vectors reused between frames
    let (mut menu, mut subs) = { let shared = state.shared(); (core::mem::take(&mut shared.menu), core::mem::take(&mut shared.menu_sub)) };
    menu.extend(list.iter().zip(enabled).map(|(text, enabled)| MenuItem::new(text).disabled(*enabled == 0)));
    let mut options = Menu::DEFAULT;
    if has_width != 0 { options = options.w_px(width); }
    let mut marks = false;
    let mut valid = true;
    if !extra.is_null() {
        // the size is read before the descriptor is copied; only a size the record has had is read (one that ends inside a field is refused)
        let size = unsafe { (*extra).size } as usize;
        valid = (matches!(size, MENU_EXTRA_V1 | MENU_EXTRA_V149 | MENU_EXTRA_V155 | MENU_EXTRA_V158) || size >= core::mem::size_of::<PxuiMenuExtra>()) && unsafe { decode_extra(&*extra, size, &mut menu, &mut subs, &mut options, &mut marks) };
    }
    if valid && has_selected != 0 && !marks {
        // the current choice: the rows from `lead_from` keep the mark slot, the chosen one is marked
        let lead_from = if extra.is_null() { 0 } else { unsafe { (*extra).lead_from } };
        for (row, item) in menu.iter_mut().enumerate() {
            if row as i64 >= lead_from || row as i64 == selected { item.checked = Some(row as i64 == selected); }
        }
    }
    let mut result = Chosen::default();
    let status = if valid { unsafe { crate::trusted::menu(ui, NodeKey::Label(label), (x, y), &menu, &options, &mut result) } } else { 0 };
    menu.clear();
    subs.clear();
    { let shared = state.shared(); (shared.menu, shared.menu_sub) = (menu, subs); }
    state.shared().put_strs(list);
    if !valid { return unsafe { fail(ui, "invalid context menu extras") }; }
    if status != 0 { return status; }
    let (tag, index) = match (result.dismissed, result.chosen) { (true, _) => (2, -1), (_, Some(index)) => (1, index as i64), _ => (0, -1) };
    unsafe { *out_tag = tag; *out_index = index; }
    0
});

// `PxuiMenuExtra` as ABI 1.0 had it: the fields before `row_h`.
const MENU_EXTRA_V1: usize = 80;
// `PxuiMenuExtra` as ABI 1.49 had it: the fields before `padding`.
const MENU_EXTRA_V149: usize = 104;
// `PxuiMenuExtra` as ABI 1.55 had it: the fields before `focus_on_click`.
const MENU_EXTRA_V155: usize = 144;
// `PxuiMenuExtra` as ABI 1.58 had it: the fields before `max_h`.
const MENU_EXTRA_V158: usize = 184;

// Applies a menu's extras to its rows (shortcuts, swatches, destructive rows, marks, submenus) and options, borrowing the caller's
// strings: false when a span, text or row is invalid. The submenus' rows go flat into `subs` first, then each opening row points
// into it (`subs` does not grow after that).
unsafe fn decode_extra(extra: &PxuiMenuExtra, size: usize, menu: &mut [MenuItem<'static>], subs: &mut Vec<MenuItem<'static>>, options: &mut Menu<'static>, marks: &mut bool) -> bool {
    fn span<'a, T>(s: Slice<T>, max: usize) -> Option<&'a [T]> {
        if s.len > max || (s.len != 0 && s.ptr.is_null()) { return None; }
        Some(if s.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(s.ptr, s.len) } })
    }
    let texts = |values: Slice<Str>| -> Option<&'static [Str]> { span(values, MAX_ROWS) };
    let (Some(keys), Some(swatches), Some(danger), Some(submenus)) = (texts(extra.keys), span(extra.swatches, MAX_ROWS), span(extra.danger, MAX_ROWS), span(extra.submenus, 64)) else { return false };
    if keys.len() > menu.len() || swatches.len() > menu.len() { return false; }
    for (item, key) in menu.iter_mut().zip(keys) {
        let Some(key) = (unsafe { checked(*key) }) else { return false };
        item.shortcut = key;
    }
    for (item, swatch) in menu.iter_mut().zip(swatches) { item.swatch = (swatch.has != 0).then_some(swatch.color); }
    for row in danger {
        let Some(item) = usize::try_from(*row).ok().and_then(|row| menu.get_mut(row)) else { return false };
        item.danger = true;
    }
    // ABI 1.49: the row height and the marks per row
    if size >= MENU_EXTRA_V149 {
        if !extra.row_h.is_finite() || extra.row_h < 0.0 { return false; }
        if extra.row_h > 0.0 { *options = options.row_h(extra.row_h); }
        if extra.max_rows > 0 { *options = options.max_rows(extra.max_rows as usize); }
        let Some(checked) = span(extra.checked, MAX_ROWS) else { return false };
        if checked.len() > menu.len() || checked.iter().any(|mark| *mark > 2) { return false; }
        *marks = !checked.is_empty();
        for (item, mark) in menu.iter_mut().zip(checked) { item.checked = (*mark != 0).then_some(*mark == 2); }
    }
    // ABI 1.55: the padding, the sheet's name and the current choice per row
    if size >= MENU_EXTRA_V155 {
        if !(0.0..=f32::MAX).contains(&extra.padding) || extra.has_name > 1 || extra.reserved2 != [0; 3] { return false; }
        *options = options.padding(extra.padding);
        if extra.has_name == 1 {
            let Some(name) = (unsafe { checked(extra.name) }) else { return false };
            *options = options.name(name);
        }
        let Some(selected) = span(extra.selected, MAX_ROWS) else { return false };
        if selected.len() > menu.len() || selected.iter().any(|on| *on > 1) { return false; }
        for (item, on) in menu.iter_mut().zip(selected) { item.selected = *on == 1; }
    }
    // ABI 1.58: a press focuses its row, and the rows' ids
    if size >= MENU_EXTRA_V158 {
        if extra.focus_on_click > 1 || extra.reserved3 != [0; 7] { return false; }
        *options = options.focus_on_click(extra.focus_on_click == 1);
        let (Some(ids), Some(has_id)) = (span(extra.ids, MAX_ROWS), span(extra.has_id, MAX_ROWS)) else { return false };
        if has_id.len() > menu.len() || has_id.len() > ids.len() || has_id.iter().any(|on| *on > 1) { return false; }
        for ((item, on), id) in menu.iter_mut().zip(has_id).zip(ids) { item.id = (*on == 1).then_some(*id); }
    }
    // ABI 1.67: the sheet's most height
    if size >= core::mem::size_of::<PxuiMenuExtra>() {
        if !(0.0..=f32::MAX).contains(&extra.max_h) || extra.reserved4 != 0 { return false; }
        if extra.max_h > 0.0 { *options = options.max_h(extra.max_h); }
    }
    subs.clear();
    let mut at = [(0usize, 0usize, 0usize); 64];
    let mut last = -1i64;
    for (n, sub) in submenus.iter().enumerate() {
        let (Some(rows), Some(enabled), Some(keys)) = (texts(sub.rows), span(sub.enabled, MAX_ROWS), texts(sub.keys)) else { return false };
        // one submenu per row, in row order: the rows of the submenus are numbered after the menu's in the order given
        if enabled.len() != rows.len() || keys.len() > rows.len() || sub.row <= last || sub.row >= menu.len() as i64 { return false; }
        last = sub.row;
        let start = subs.len();
        for (i, text) in rows.iter().enumerate() {
            let (Some(text), Some(shortcut)) = (unsafe { checked(*text) }, keys.get(i).map_or(Some(""), |key| unsafe { checked(*key) })) else { return false };
            let mut item = MenuItem::new(text).disabled(enabled[i] == 0).shortcut(shortcut);
            if sub.has_current != 0 { item = item.checked(i as i64 == sub.current); }
            subs.push(item);
        }
        at[n] = (sub.row as usize, start, rows.len());
    }
    for (row, start, count) in &at[..submenus.len()] {
        // the pointer outlives the borrow: `subs` is neither pushed to nor dropped until the call that reads the rows has returned
        menu[*row].submenu = unsafe { core::slice::from_raw_parts(subs.as_ptr().add(*start), *count) };
    }
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
    let Some((key, placeholder)) = (unsafe { crate::widgets::read_keyed(keyed) }) else { return unsafe { fail(ui, "invalid keyed descriptor") } };
    unsafe { picker(ui, limit, slash, at_rest, key, placeholder, query, rows_cb, mark_cb, off_cb, image_cb, user, out_query, out_tag, out_index) }
});
