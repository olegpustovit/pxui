use crate::Handle;
use core::{cell::Cell, ffi::c_void};
use pxui_types::{Str, ffi::{PxuiKeyedDesc, PxuiPopupDesc, Slice}, ui::{Key, Menu, MenuItem, Chosen, Overlay, Pick, PickRows, PickerResponse, Toast}};
#[cfg(not(feature = "static"))]
use pxui_types::ffi::{PxuiMenuOptions, PxuiMenuRow, Strided};
use crate::{composite::{defer_callback_panic, deferred}, check, OverlayResponse, Strs, Ui};

use crate::Picker;
library!(popup_lib {
    fn pxui_ui_popup(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, desc: *const PxuiPopupDesc, build: pxui_types::ffi::UiBuild, user: *mut c_void, out: *mut u8, root: *mut pxui_types::PxuiBox) -> i32;
});


#[cfg(feature = "static")]
mod raw {
    pub use pxui_abi_link::{pxui_ui_picker, pxui_ui_toasts};
}

#[cfg(not(feature = "static"))]
mod raw {
    use core::ffi::c_void;
    use pxui_types::{PxuiBox, Str, ffi::{PxuiKeyedDesc, PxuiMenuOptions, PxuiMenuRow, Slice, Strided}};
    use crate::sys::PxuiUi;
    /// Callback that supplies borrowed rows for a picker query.
    pub type PickerRows = Option<extern "C" fn(*mut c_void, Str, *mut Slice<pxui_types::ffi::PxuiPickRow>)>;
    /// Callback that supplies an optional colour mark for a row.
    pub type PickerMark = Option<extern "C" fn(*mut c_void, i64, *mut pxui_types::geom::Rgba) -> u8>;
    /// Callback that reports whether a row is unavailable.
    pub type PickerOff = Option<extern "C" fn(*mut c_void, i64) -> u8>;
    /// Callback that supplies an optional native image for a row.
    pub type PickerImage = Option<extern "C" fn(*mut c_void, i64, *mut crate::Image) -> u8>;
    #[link(name = "pxui")]
    unsafe extern "C" {
    /// The C entry `Ui::tooltip` calls: shows the tooltip `label` for `box_` while the pointer rests on it; an error code for a stale box or invalid text.
    /// Allocates: never on a warm frame.
    /// Identity: the tooltip is keyed by its text, so two tooltips of one text share one.
    /// Panics: never.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let b = ui.node("n", pxui::Node::DEFAULT);
    ///     ui.tooltip(&b, "Shuffle");
    /// });
    /// ```
    pub fn pxui_ui_tip(ui:*mut PxuiUi,box_:*const PxuiBox,label:Str)->i32;
    /// The C entry `Ui::toasts` calls: stacks the toast texts `items` in the window's lower end corner, toast `i` keyed by `ids[i]`, and writes
    /// the index of one clicked away or expired to `out_index`, or -1; an error code for a short or invalid `keyed`, `ids` not one an item,
    /// or a non-finite or negative `seconds`.
    /// Allocates: never on a warm frame.
    /// Identity: `keyed.key` keys the stack under the parent; each toast is keyed by its id, never its text.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::ui::Toast;
    /// pxui::doc::frames(1, &mut |ui| { ui.toasts("toasts", &[Toast::new(1, "Saved")], 3.0); });
    /// ```
    pub fn pxui_ui_toasts(ui:*mut PxuiUi,keyed:*const PxuiKeyedDesc,items:Slice<Str>,ids:Slice<i64>,seconds:f64,out_index:*mut i64)->i32;
    /// The C entry `Ui::menu` calls (the dynamic-link path): shows the menu `items` at (`x`, `y`) with the enabled flags, optional width and selected row and the `extra` columns (keys, swatches, danger rows, submenus, marks); writes the outcome tag (0 open, 1 chosen, 2 dismissed) and the chosen row to the outputs; an error code for invalid arguments.
    /// `extra` is read by its `size` field, so an older caller's shorter record is served.
    /// Allocates: never on a warm frame.
    /// Identity: `label` keys the menu and is its accessible name.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::ui::{Menu, MenuItem};
    /// pxui::doc::frames(1, &mut |ui| { ui.menu("m", pxui::Point::new(10.0, 10.0), &[MenuItem::new("Open")], &Menu::DEFAULT); });
    /// ```
    pub fn pxui_ui_context_menu(ui:*mut PxuiUi,x:f32,y:f32,label:Str,rows:Strided<PxuiMenuRow>,has_width:u8,width:f32,options:*const PxuiMenuOptions,out_tag:*mut u32,out_index:*mut i64)->i32;
    /// The C entry `Ui::picker` calls: a search field over the rows `rows` computes for the query, windowed to `limit` rows; writes the edited
    /// query, the outcome tag (0 none, 1 pick, 2 delete, 3 submit, 4 back, 5 cancel) and the row index; an error code for invalid arguments.
    /// The callbacks run during the call with `user`; the query and the rows they hand over are borrowed until the call returns.
    /// Allocates: never on a warm frame.
    /// Identity: `keyed.key` keys the field under the parent; `keyed.name` is its placeholder and accessible name, never part of the key.
    /// Panics: never.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     ui.picker("find", "Search", "", &pxui::Picker::DEFAULT, &mut |_query, rows| rows.push(pxui::PickRow::new("Song").detail("Artist")));
    /// });
    /// ```
    pub fn pxui_ui_picker(ui:*mut PxuiUi,keyed:*const PxuiKeyedDesc,limit:i64,slash:u8,at_rest:u8,query:Str,rows:PickerRows,mark:PickerMark,off:PickerOff,image:PickerImage,user:*mut c_void,out_query:*mut Str,out_tag:*mut u32,out_index:*mut i64)->i32;
    }
}

#[inline]
fn as_str(value: &str) -> Str { crate::as_str(value) }

// The rows a picker holds between its calls on this thread: its row strings (the app's fill reuses them) and their `Str` span, so a
// settled picker allocates nothing here. Taken for the call and put back, so a picker built inside a picker's callback finds them empty.
// Bounded by the library's row cap (4096 rows and 16 MiB of text a call): a fill past it is refused, and the refusal's
// panic drops the taken rows instead of putting them back.
thread_local! { static ROWS: Cell<(PickRows, Vec<pxui_types::ffi::PxuiPickRow>)> = const { Cell::new((PickRows::new(), Vec::new())) }; }

/// The most rows a menu takes: the library's row cap (it refuses more); `Ui::menu` refuses a longer one before copying a row.
pub(crate) const MENU_ROWS: usize = 4096;

// Per-thread scratch pools, one buffer per simultaneous menu call (as the ABI's per-Ui string pool), reused after nested bodies return.
// A top-row buffer holds at most MENU_ROWS rows (capacity below twice that bound); borrowed rows are cleared before storage.
#[cfg(not(feature = "static"))]
thread_local! { static MENU_KEPT: Cell<Vec<Vec<PxuiMenuRow>>> = const { Cell::new(Vec::new()) }; }

// Call-scoped row scratch shared by menus on this thread, taken during callbacks: at most 64 * MENU_ROWS children (capacity below
// twice that bound). Strings remain borrowed only until the call returns; clear every row before returning the storage.
#[cfg(not(feature = "static"))]
thread_local! { static MENU_SUB_KEPT: Cell<Vec<Vec<PxuiMenuRow>>> = const { Cell::new(Vec::new()) }; }

struct Rows<'a, 'f> {
    fill: &'a mut (dyn FnMut(&str, &mut PickRows) + 'f),
    owned: PickRows,
    raw: Vec<pxui_types::ffi::PxuiPickRow>,
    options: &'a Picker<'a>,
}
extern "C" fn mark(user: *mut c_void, row: i64, out: *mut pxui_types::geom::Rgba) -> u8 {
    if user.is_null() || out.is_null() || deferred() { return 0; }
    let ctx = unsafe { &*user.cast::<Rows>() };
    let Some(mark) = ctx.options.mark else { return 0 };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| mark(row.max(0) as usize))) {
        Ok(Some(color)) => { unsafe { *out = color; } 1 }
        Ok(None) => 0,
        Err(panic) => { defer_callback_panic(panic); 0 }
    }
}
extern "C" fn off(user: *mut c_void, row: i64) -> u8 {
    if user.is_null() || deferred() { return 0; }
    let ctx = unsafe { &*user.cast::<Rows>() };
    let Some(off) = ctx.options.off else { return 0 };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| off(row.max(0) as usize))) { Ok(v) => v as u8, Err(panic) => { defer_callback_panic(panic); 0 } }
}
extern "C" fn image(user: *mut c_void, row: i64, out: *mut crate::Image) -> u8 {
    if user.is_null() || out.is_null() || deferred() { return 0; }
    let ctx = unsafe { &*user.cast::<Rows>() };
    let Some(image) = ctx.options.image else { return 0 };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| image(row.max(0) as usize))) {
        Ok(Some(found)) => { unsafe { *out = found; } 1 }
        Ok(None) => 0,
        Err(panic) => { defer_callback_panic(panic); 0 }
    }
}
extern "C" fn rows(user: *mut c_void, query: Str, out: *mut Slice<pxui_types::ffi::PxuiPickRow>) {
    if user.is_null() || out.is_null() || deferred() { return; }
    let ctx = unsafe { &mut *user.cast::<Rows>() };
    let query_bytes = if query.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(query.ptr, query.len) } };
    let Ok(query) = core::str::from_utf8(query_bytes) else { unsafe { *out = Slice { ptr: core::ptr::null(), len: 0 }; } return; };
    let (fill, owned) = (&mut *ctx.fill, &mut ctx.owned);
    owned.clear();
    if let Err(panic) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| fill(query, owned))) { defer_callback_panic(panic); owned.clear(); }
    ctx.raw.clear();
    for row in (0..ctx.owned.len()).filter_map(|i| ctx.owned.get(i)) {
        let (art_kind, art) = match row.art { None => (0, 0), Some(crate::PickArt::Glyph(c)) => (1, c as u64), Some(crate::PickArt::Image(id)) => (2, id as u64), Some(crate::PickArt::Draw(word)) => (3, u64::from(word)) };
        ctx.raw.push(pxui_types::ffi::PxuiPickRow { art_kind, art, heading: as_str(row.heading), ..pxui_types::ffi::PxuiPickRow::new(row.label, row.detail) });
    }
    unsafe { *out = Slice { ptr: ctx.raw.as_ptr(), len: ctx.raw.len() }; }
}

impl Ui {
    /// Shows the kit tooltip `text` for `box_` once the pointer has rested on it for the hover delay.
    ///
    /// Call it every frame after declaring `box_`; nothing shows while the pointer is elsewhere, during a press, on touch or while a
    /// popup is open. The tooltip is a sheet at the root, placed by the pointer, built through the tip body. A stale box shows nothing.
    /// Allocates: when the shown tooltip’s retained layout or line storage grows, never on warm frames.
    /// Identity: the tooltip's owner supplies its identity. Updating its text keeps the same identity and hover timer.
    /// Panics: when the library refuses the call (outside a frame, text too long). A key over 256 bytes panics (a programmer error). Display text past 64 KiB is clipped at a character boundary, never refused.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let b = ui.node("shuffle", pxui::Node::DEFAULT);
    ///     ui.tooltip(&b, "Shuffle");
    /// });
    /// ```
    #[inline]
    #[track_caller]
    pub fn tooltip(&mut self, box_: &Handle, text: &str) {
        #[cfg(feature = "static")]
        check(self, unsafe { pxui_abi_link::trusted::tooltip(self.0.as_ptr(), box_, text) });
        #[cfg(not(feature = "static"))]
        check(self, unsafe { raw::pxui_ui_tip(self.0.as_ptr(), box_, as_str(text)) })
    }

    /// The one body of `toasts`: keyed by `key`, each toast by its id; texts and ids as the export's columns (on the stack up to 32).
    #[track_caller]
    pub(crate) fn toasts_key(&mut self, key: Key, items: &[Toast], seconds: f64) -> Chosen {
        let mut id_label = [0u8; 17];
        let keyed = PxuiKeyedDesc::new(key.label(&mut id_label), "");
        let texts = Strs::each(items.len(), &|i| items[i].text);
        let mut stack = [0i64; 32];
        let heap: Vec<i64>;
        let ids: &[i64] = if items.len() <= 32 {
            for (out, item) in stack.iter_mut().zip(items) { *out = item.id; }
            &stack[..items.len()]
        } else {
            heap = items.iter().map(|item| item.id).collect();
            &heap
        };
        let mut index: i64 = -1;
        check(self, unsafe { raw::pxui_ui_toasts(self.0.as_ptr(), &keyed, texts.span(), Slice { ptr: ids.as_ptr(), len: ids.len() }, seconds, &mut index) });
        let chosen = (index >= 0).then_some(index as usize);
        Chosen { chosen, chosen_id: chosen.and_then(|index| items.get(index).map(|item| item.id)), dismissed: false }
    }

    /// The one body of `popup`: the overlay as the popup record, keyed by `key` and named by the overlay's name.
    #[track_caller]
    pub(crate) fn popup_key(&mut self, key: Key, overlay: &Overlay<'_>, mut f: &mut dyn FnMut(&mut Ui)) -> OverlayResponse {
        let mut id_label = [0u8; 17];
        let keyed = PxuiKeyedDesc::new(key.label(&mut id_label), crate::clip(overlay.name.unwrap_or("")));
        let desc = PxuiPopupDesc {
            size: core::mem::size_of::<PxuiPopupDesc>() as u32, has_stroke: overlay.stroke.is_some() as u8, has_max_height: overlay.max_h.is_some() as u8,
            dismiss_initial: overlay.dismiss_initial as u8, attached: overlay.attached as u8, stroke: overlay.stroke.unwrap_or_default(), max_height: overlay.max_h.unwrap_or_default(),
            x: overlay.at.0, y: overlay.at.1, width: overlay.w.unwrap_or(0.0), height: overlay.h.unwrap_or(0.0), label: Str::default(),
        };
        let (mut out, mut handle) = (0, crate::Handle::default());
        check(self, unsafe { popup_lib::pxui_ui_popup(self.0.as_ptr(), &keyed, &desc, Some(crate::composite::call), &mut f as *mut _ as *mut c_void, &mut out, &mut handle) });
        OverlayResponse { handle, open: out != 0 }
    }

    /// The one body of `menu`: linked statically, the library's trusted entry with the rows as they are; else the rows as the C export's
    /// rows (on the stack up to 32 rows, in bounded buffers kept on the thread past that and for submenus), then the call.
    #[track_caller]
    pub(crate) fn menu_keyed(&mut self, key: Key, at: (f32, f32), items: &[MenuItem], menu: &Menu) -> Chosen {
        // refused before any row is copied, so the columns kept on the thread never hold more than the library takes
        assert!(items.len() <= MENU_ROWS, "pxui: a menu takes at most {MENU_ROWS} rows");
        let mut submenus = 0;
        let mut bytes = 0;
        for item in items {
            if !item.submenu.is_empty() {
                submenus += 1;
                assert!(submenus <= 64 && item.submenu.len() <= MENU_ROWS, "pxui: a menu takes at most 64 submenus of {MENU_ROWS} rows");
            }
            for (index, row) in core::iter::once(item).chain(item.submenu).enumerate() {
                assert!(index == 0 || row.submenu.is_empty(), "pxui: submenus cannot contain submenus");
                bytes += crate::clip(row.text).len() + crate::clip(row.shortcut).len();
                assert!(bytes <= 16 << 20, "pxui: menu text takes at most 16 MiB");
            }
        }
        #[cfg(feature = "static")]
        {
            let mut out = Chosen::default();
            check(self, unsafe { pxui_abi_link::trusted::menu(self.0.as_ptr(), key, at, items, menu, &mut out) });
            out
        }
        #[cfg(not(feature = "static"))]
        self.menu_columns(key, at, items, menu)
    }

    #[cfg(not(feature = "static"))]
    #[track_caller]
    fn menu_columns(&mut self, key: Key, at: (f32, f32), items: &[MenuItem], menu: &Menu) -> Chosen {
        let mut id_label = [0u8; 17];
        let label = key.label(&mut id_label);
        const ROWS: usize = 32;
        let row_of = |item: &MenuItem| PxuiMenuRow {
            enabled: item.enabled as u8, danger: item.danger as u8, checked: item.checked.map_or(0, |mark| 1 + mark as u8), selected: item.selected as u8,
            has_swatch: item.swatch.is_some() as u8, swatch: item.swatch.unwrap_or_default(), has_id: item.id.is_some() as u8, id: item.id.unwrap_or(0),
            shortcut: as_str(item.shortcut), ..PxuiMenuRow::new(crate::clip(item.text))
        };
        let n = items.len();
        let mut stack = [PxuiMenuRow::new(""); ROWS];
        let mut kept = if n > ROWS {
            let mut pool = MENU_KEPT.take();
            let rows = pool.pop().unwrap_or_default();
            MENU_KEPT.set(pool);
            rows
        } else { Vec::new() };
        let rows: &mut [PxuiMenuRow] = if n <= ROWS { &mut stack[..n] } else { kept.clear(); kept.resize(n, PxuiMenuRow::new("")); &mut kept };
        for (row, item) in rows.iter_mut().zip(items) { *row = row_of(item); }
        // the submenus' rows, flat: each opening row's span points at its run (the vector grows no more once they are set)
        let children: usize = items.iter().map(|item| item.submenu.len()).sum();
        let mut flat = if children != 0 {
            let mut pool = MENU_SUB_KEPT.take();
            let rows = pool.pop().unwrap_or_default();
            MENU_SUB_KEPT.set(pool);
            rows
        } else { Vec::new() };
        flat.clear();
        if children != 0 {
            for item in items { flat.extend(item.submenu.iter().map(row_of)); }
            let mut start = 0;
            for (row, item) in rows.iter_mut().zip(items).filter(|(_, item)| !item.submenu.is_empty()) {
                row.submenu = Strided { ptr: flat[start..].as_ptr(), len: item.submenu.len(), stride: core::mem::size_of::<PxuiMenuRow>() };
                start += item.submenu.len();
            }
        }
        let options = PxuiMenuOptions {
            size: core::mem::size_of::<PxuiMenuOptions>() as u32, variant: menu.variant, reserved: 0,
            row_h: menu.row_h.unwrap_or(0.0), max_rows: menu.max_rows.map_or(0, |rows| rows.min(u32::MAX as usize) as u32),
            padding: menu.padding, has_name: menu.name.is_some() as u8, focus_on_click: menu.focus_on_click as u8, reserved2: [0; 6], name: as_str(menu.name.unwrap_or("")),
            max_h: menu.max_h.unwrap_or(0.0), reserved3: 0,
        };
        let optioned = menu.row_h.is_some() || menu.max_rows.is_some() || menu.padding != 6.0 || menu.name.is_some() || menu.focus_on_click || menu.max_h.is_some() || menu.variant != 0;
        let (has_width, width) = menu.w.map_or((0, 0.0), |width| (1, width));
        let (mut tag, mut index) = (0, -1i64);
        check(self, unsafe { raw::pxui_ui_context_menu(
            self.0.as_ptr(), at.0, at.1, as_str(label), Strided { ptr: rows.as_ptr(), len: n, stride: core::mem::size_of::<PxuiMenuRow>() }, has_width, width,
            if optioned { &options } else { core::ptr::null() }, &mut tag, &mut index,
        ) });
        if n > ROWS { kept.clear(); let mut pool = MENU_KEPT.take(); pool.push(kept); MENU_KEPT.set(pool); }
        if children != 0 { flat.clear(); let mut pool = MENU_SUB_KEPT.take(); pool.push(flat); MENU_SUB_KEPT.set(pool); }
        match tag { 0 => Chosen::default(), 1 => Chosen { chosen: Some(index as usize), ..Chosen::default() }, 2 => Chosen { dismissed: true, ..Chosen::default() }, _ => unreachable!("the library answers a menu with tag 0, 1 or 2") }
    }

    /// The one body of `picker`: key and placeholder apart in a `PxuiKeyedDesc`; the rows the app pushes kept on the thread between calls.
    #[track_caller]
    pub(crate) fn picker_key(&mut self, key: Key, placeholder: &str, query: &str, options: &Picker<'_>, fill: &mut dyn FnMut(&str, &mut PickRows)) -> PickerResponse<'_> {
        let mut id_label = [0u8; 17];
        let keyed = PxuiKeyedDesc::new(key.label(&mut id_label), crate::clip(placeholder));
        let (owned, raw) = ROWS.take();
        let mut rows_ctx: Rows = Rows { fill, owned, raw, options };
        let mut out_query = Str::default();
        let mut tag = 0;
        let mut index: i64 = -1;
        check(self, unsafe { raw::pxui_ui_picker(
            self.0.as_ptr(), &keyed, i64::try_from(options.limit).expect("pxui: a picker row limit over i64::MAX"), options.slash as u8, options.at_rest as u8,
            as_str(query), Some(rows), options.mark.map(|_| mark as extern "C" fn(*mut c_void, i64, *mut pxui_types::geom::Rgba) -> u8), options.off.map(|_| off as extern "C" fn(*mut c_void, i64) -> u8),
            options.image.map(|_| image as extern "C" fn(*mut c_void, i64, *mut crate::Image) -> u8), &mut rows_ctx as *mut _ as *mut c_void,
            &mut out_query, &mut tag, &mut index,
        ) });
        let Rows { mut owned, mut raw, .. } = rows_ctx;
        raw.clear();
        ROWS.set((core::mem::take(&mut owned), raw));
        let pick = match tag { 0 => Pick::None, 1 => Pick::Pick(index), 2 => Pick::Delete(index), 3 => Pick::Submit, 4 => Pick::Back, 5 => Pick::Cancel, _ => unreachable!("the library answers a picker with tag 0 to 5") };
        PickerResponse { query: crate::lent_str(out_query), pick }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "static")]
    use pxui_abi_link::pxui_ui_context_menu;
    #[cfg(not(feature = "static"))]
    use super::raw::pxui_ui_context_menu;
    use crate::{as_str, FrameIn, Ui};
    use pxui_types::ffi::{PxuiMenuOptions, PxuiMenuRow, Strided};

    fn options(size: u32) -> PxuiMenuOptions {
        PxuiMenuOptions { size, variant: 0, reserved: 0, row_h: 0.0, max_rows: 0, padding: 6.0, has_name: 0, focus_on_click: 0, reserved2: [0; 6], name: pxui_types::Str::default(), max_h: 0.0, reserved3: 0 }
    }

    /// Calls the export over `rows` with `options` and returns its status.
    fn call(ui: &mut Ui, rows: &[PxuiMenuRow], options: &PxuiMenuOptions) -> i32 {
        let (mut tag, mut index) = (0, -1i64);
        unsafe { pxui_ui_context_menu(ui.0.as_ptr(), 10.0, 10.0, as_str("m"), Strided::of(rows), 0, 0.0, options, &mut tag, &mut index) }
    }

    /// The rows are read at their own size and the options whole: a short record, a flag or mark past its range, a reserved byte, a submenu
    /// with a submenu of its own or a stride below the record are refused, and so are a negative row height, padding or `max_h`, a name or
    /// focus flag over 1, a non-finite height and a reserved word set.
    #[test]
    fn the_rows_and_options_are_read_by_their_size_and_validated_whole() {
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
        let mut results = Vec::new();
        ui.frame(&FrameIn::new(320.0, 240.0), &mut |ui: &mut Ui| {
            let (full, row_size) = (core::mem::size_of::<PxuiMenuOptions>() as u32, core::mem::size_of::<PxuiMenuRow>() as u32);
            let two = [PxuiMenuRow::new("One"), PxuiMenuRow::new("Two")];
            let with = |edit: &dyn Fn(&mut PxuiMenuRow)| { let mut rows = two; edit(&mut rows[1]); rows };
            results.push(call(ui, &two, &options(full)));
            results.push(call(ui, &with(&|r| r.size = row_size - 1), &options(full)));
            results.push(call(ui, &with(&|r| r.enabled = 2), &options(full)));
            results.push(call(ui, &with(&|r| r.checked = 3), &options(full)));
            results.push(call(ui, &with(&|r| r.checked = 2), &options(full)));
            results.push(call(ui, &with(&|r| r.reserved = [0, 1]), &options(full)));
            results.push(call(ui, &with(&|r| (r.has_id, r.id) = (1, 9)), &options(full)));
            results.push(call(ui, &with(&|r| r.has_id = 2), &options(full)));
            results.push(call(ui, &with(&|r| r.text = { let mut record = pxui_types::Str::default(); record.ptr = b"\xff".as_ptr(); record.len = 1; record }), &options(full)));
            let child = [PxuiMenuRow::new("child")];
            results.push(call(ui, &with(&|r| r.submenu = Strided::of(&child)), &options(full)));
            let nested = [PxuiMenuRow { submenu: Strided::of(&child), ..PxuiMenuRow::new("nested") }];
            results.push(call(ui, &with(&|r| r.submenu = Strided::of(&nested)), &options(full)));
            results.push(call(ui, &with(&|r| r.submenu = Strided { stride: 8, ..Strided::of(&child) }), &options(full)));
            results.push(call(ui, &two, &options(full - 1)));
            results.push(call(ui, &two, &PxuiMenuOptions { row_h: -1.0, ..options(full) }));
            results.push(call(ui, &two, &PxuiMenuOptions { row_h: 30.0, ..options(full) }));
            results.push(call(ui, &two, &PxuiMenuOptions { padding: -1.0, ..options(full) }));
            results.push(call(ui, &two, &PxuiMenuOptions { padding: 10.0, has_name: 1, name: as_str("Sizes"), focus_on_click: 1, ..options(full) }));
            results.push(call(ui, &two, &PxuiMenuOptions { has_name: 2, ..options(full) }));
            results.push(call(ui, &two, &PxuiMenuOptions { focus_on_click: 2, ..options(full) }));
            results.push(call(ui, &two, &PxuiMenuOptions { reserved2: [0, 1, 0, 0, 0, 0], ..options(full) }));
            results.push(call(ui, &two, &PxuiMenuOptions { max_h: 120.0, ..options(full) }));
            results.push(call(ui, &two, &PxuiMenuOptions { max_h: -1.0, ..options(full) }));
            results.push(call(ui, &two, &PxuiMenuOptions { max_h: f32::INFINITY, ..options(full) }));
            results.push(call(ui, &two, &PxuiMenuOptions { reserved3: 1, ..options(full) }));
            results.push(call(ui, &two, &PxuiMenuOptions { reserved: 1, ..options(full) }));
        }).ok();
        let accepted: Vec<bool> = results.iter().map(|status| *status == 0).collect();
        assert_eq!(accepted, [true, false, false, false, true, false, true, false, false, true, false, false, false, false, true, false, true, false, false, false, true, false, false, false, false]);
    }

    /// U1c.2 (OS-grade 4): a menu past the library's row cap is refused before a row is copied, so the columns the dynamic wrapper keeps
    /// on the thread never grow past `MENU_ROWS`; a menu at the cap is served.
    #[test]
    fn a_menu_past_the_row_cap_is_refused_before_its_columns_grow() {
        use crate::{Menu, MenuItem, Point};
        let items: Vec<MenuItem> = (0..super::MENU_ROWS + 1).map(|_| MenuItem::new("row")).collect();
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
        let refused = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = ui.frame(&FrameIn::new(200.0, 100.0), &mut |ui: &mut Ui| { ui.menu("long", Point::new(0.0, 0.0), &items, &Menu::DEFAULT); });
        }));
        let panic = refused.expect_err("a menu of 4097 rows is refused");
        let message = panic.downcast_ref::<String>().map(String::as_str).or_else(|| panic.downcast_ref::<&str>().copied()).unwrap_or("");
        assert!(message.contains("at most 4096 rows"), "{message}");
        #[cfg(not(feature = "static"))]
        assert!(super::MENU_KEPT.take().is_empty(), "nothing was copied for the refused menu");
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
        ui.frame(&FrameIn::new(200.0, 100.0), &mut |ui: &mut Ui| { ui.menu("long", Point::new(0.0, 0.0), &items[..super::MENU_ROWS], &Menu::DEFAULT); }).expect("a menu at the cap is served");
        #[cfg(not(feature = "static"))]
        { let kept = super::MENU_KEPT.take().pop().unwrap(); assert!(kept.capacity() <= super::MENU_ROWS + super::MENU_ROWS / 2, "kept columns stay near the cap"); }
    }

    #[test]
    fn submenu_limits_are_refused_before_the_wrapper_copies_rows() {
        use crate::{Menu, MenuItem, Point};
        let child = [MenuItem::new("Child")];
        let openings = [MenuItem::new("Parent").submenu(&child); 65];
        let children = [MenuItem::new("Child"); super::MENU_ROWS + 1];
        let oversized = [MenuItem::new("Parent").submenu(&children)];
        let nested_child = [MenuItem::new("Child").submenu(&child)];
        let nested = [MenuItem::new("Parent").submenu(&nested_child)];
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
        for rows in [&openings[..], &oversized[..], &nested[..]] {
            let refused = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ui.frame(&FrameIn::new(200.0, 100.0), &mut |ui| { ui.menu("invalid", Point::new(0.0, 0.0), rows, &Menu::DEFAULT); }).unwrap();
            }));
            assert!(refused.is_err());
            #[cfg(not(feature = "static"))]
            assert!(super::MENU_SUB_KEPT.take().is_empty(), "no child row copied before refusal");
        }
    }
}
