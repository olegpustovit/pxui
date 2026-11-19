use crate::Handle;
use core::{cell::Cell, ffi::c_void};
use pxui_types::{Str, ffi::{PxuiKeyedDesc, PxuiPopupDesc, Slice}, ui::{Key, Menu, MenuItem, Chosen, Overlay, Pick, PickRows, PickerResponse, Toast}};
#[cfg(not(feature = "static"))]
use pxui_types::ffi::{PxuiMenuExtra, PxuiSubmenu, PxuiSwatch};
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
    use pxui_types::{PxuiBox, Str, ffi::{PxuiKeyedDesc, PxuiMenuExtra, Slice}};
    use crate::sys::PxuiUi;
    pub type PickerRows = Option<extern "C" fn(*mut c_void, Str, *mut Slice<pxui_types::ffi::PxuiPickRow>)>;
    pub type PickerMark = Option<extern "C" fn(*mut c_void, i64, *mut pxui_types::geom::Rgba) -> u8>;
    pub type PickerOff = Option<extern "C" fn(*mut c_void, i64) -> u8>;
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
    pub fn pxui_ui_context_menu(ui:*mut PxuiUi,x:f32,y:f32,label:Str,items:Slice<Str>,enabled:Slice<u8>,has_width:u8,width:f32,has_selected:u8,selected:i64,extra:*const PxuiMenuExtra,out_tag:*mut u32,out_index:*mut i64)->i32;
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
    pub fn pxui_ui_picker(ui:*mut PxuiUi,keyed:*const PxuiKeyedDesc,limit:i64,slash:u8,at_rest:u8,query:Str,rows:PickerRows,mark:PickerMark,off:PickerOff,user:*mut c_void,out_query:*mut Str,out_tag:*mut u32,out_index:*mut i64)->i32;
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

// The columns of a menu of more than 32 rows, kept on this thread between calls so a settled long menu allocates nothing here; at most
// `MENU_ROWS` rows' worth (about 240 KiB), the longest menu built on the thread.
#[cfg(not(feature = "static"))]
type MenuColumns = (Vec<Str>, Vec<u8>, Vec<Str>, Vec<PxuiSwatch>, Vec<u8>, Vec<i64>, Vec<u8>, Vec<i64>, Vec<u8>);
#[cfg(not(feature = "static"))]
thread_local! { static MENU_COLUMNS: Cell<MenuColumns> = const { Cell::new((Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new())) }; }

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
        let (art_kind, art) = match row.art { None => (0, 0), Some(crate::PickArt::Glyph(c)) => (1, c as u64), Some(crate::PickArt::Image(id)) => (2, id as u64) };
        ctx.raw.push(pxui_types::ffi::PxuiPickRow { art_kind, art, heading: as_str(row.heading), ..pxui_types::ffi::PxuiPickRow::new(row.label, row.detail) });
    }
    unsafe { *out = Slice { ptr: ctx.raw.as_ptr(), len: ctx.raw.len() }; }
}

impl Ui {
    /// Shows the kit tooltip `text` for `box_` once the pointer has rested on it for the hover delay.
    ///
    /// Call it every frame after declaring `box_`; nothing shows while the pointer is elsewhere, during a press, on touch or while a
    /// popup is open. The tooltip is a sheet at the root, placed by the pointer, built through the tip body. A stale box shows nothing.
    /// Allocates: once for the rested text; never while nothing shows.
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
    /// columns (on the stack up to 32 rows, in buffers kept on the thread past that; submenus always on the heap), then the call.
    #[track_caller]
    pub(crate) fn menu_keyed(&mut self, key: Key, at: (f32, f32), items: &[MenuItem], menu: &Menu) -> Chosen {
        // refused before any row is copied, so the columns kept on the thread never hold more than the library takes
        assert!(items.len() <= MENU_ROWS, "pxui: a menu takes at most {MENU_ROWS} rows");
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
        let n = items.len();
        let (mut sl, mut se, mut sk, mut ss, mut sm, mut sd, mut sc, mut si, mut sh) = ([Str::default(); ROWS], [0u8; ROWS], [Str::default(); ROWS], [PxuiSwatch::default(); ROWS], [0u8; ROWS], [0i64; ROWS], [0u8; ROWS], [0i64; ROWS], [0u8; ROWS]);
        let (mut hl, mut he, mut hk, mut hs, mut hm, mut hd, mut hc, mut hi, mut hh) = if n > ROWS { MENU_COLUMNS.take() } else { Default::default() };
        #[allow(clippy::type_complexity)]
        let (labels, enabled, keys, swatches, marks, danger, current, ids, has_id): (&mut [Str], &mut [u8], &mut [Str], &mut [PxuiSwatch], &mut [u8], &mut [i64], &mut [u8], &mut [i64], &mut [u8]) = if n <= ROWS {
            (&mut sl[..n], &mut se[..n], &mut sk[..n], &mut ss[..n], &mut sm[..n], &mut sd[..n], &mut sc[..n], &mut si[..n], &mut sh[..n])
        } else {
            (hl.clear(), he.clear(), hk.clear(), hs.clear(), hm.clear(), hd.clear(), hc.clear(), hi.clear(), hh.clear());
            (hl.resize(n, Str::default()), he.resize(n, 0), hk.resize(n, Str::default()), hs.resize(n, PxuiSwatch::default()), hm.resize(n, 0), hd.resize(n, 0), hc.resize(n, 0), hi.resize(n, 0), hh.resize(n, 0));
            (&mut hl, &mut he, &mut hk, &mut hs, &mut hm, &mut hd, &mut hc, &mut hi, &mut hh)
        };
        let (mut dangers, mut extras) = (0, menu.row_h.is_some() || menu.max_rows.is_some() || menu.padding != 6.0 || menu.name.is_some() || menu.focus_on_click || menu.max_h.is_some());
        let (mut selected, mut with_ids) = (false, false);
        let mut with_submenus = false;
        for (row, item) in items.iter().enumerate() {
            (labels[row], enabled[row], keys[row]) = (as_str(item.text), item.enabled as u8, as_str(item.shortcut));
            if let Some(color) = item.swatch { swatches[row] = PxuiSwatch { has: 1, color, ..PxuiSwatch::default() }; extras = true; }
            if let Some(mark) = item.checked { marks[row] = 1 + mark as u8; extras = true; }
            if item.danger { danger[dangers] = row as i64; dangers += 1; extras = true; }
            if item.selected { current[row] = 1; selected = true; extras = true; }
            if let Some(id) = item.id { (ids[row], has_id[row]) = (id, 1); with_ids = true; extras = true; }
            extras |= !item.shortcut.is_empty();
            with_submenus |= !item.submenu.is_empty();
        }
        // the submenus' rows, flat: each opening row's descriptor points at its run
        let (mut sub_rows, mut sub_enabled, mut sub_keys, mut runs): (Vec<Str>, Vec<u8>, Vec<Str>, Vec<(i64, usize, usize, Option<i64>)>) = Default::default();
        if with_submenus {
            extras = true;
            for (row, item) in items.iter().enumerate().filter(|(_, item)| !item.submenu.is_empty()) {
                let start = sub_rows.len();
                for child in item.submenu {
                    (sub_rows.push(as_str(child.text)), sub_enabled.push(child.enabled as u8), sub_keys.push(as_str(child.shortcut)));
                }
                // the ABI carries one current row per submenu: the first checked
                let current = item.submenu.iter().position(|child| child.checked == Some(true)).map(|i| i as i64);
                let marked = item.submenu.iter().any(|child| child.checked.is_some());
                runs.push((row as i64, start, item.submenu.len(), if marked { Some(current.unwrap_or(-1)) } else { None }));
            }
        }
        let descriptors: Vec<PxuiSubmenu> = runs.iter().map(|&(row, start, count, current)| PxuiSubmenu {
            row, current: current.unwrap_or_default(), has_current: current.is_some() as u8, reserved: [0; 7],
            rows: Slice { ptr: sub_rows[start..].as_ptr(), len: count }, enabled: Slice { ptr: sub_enabled[start..].as_ptr(), len: count }, keys: Slice { ptr: sub_keys[start..].as_ptr(), len: count },
        }).collect();
        let marks_len = if marks.contains(&1) || marks.contains(&2) { n } else { 0 };
        let extra = PxuiMenuExtra {
            size: core::mem::size_of::<PxuiMenuExtra>() as u32, reserved: 0, lead_from: 0,
            keys: Slice { ptr: keys.as_ptr(), len: n }, swatches: Slice { ptr: swatches.as_ptr(), len: n },
            danger: Slice { ptr: danger.as_ptr(), len: dangers }, submenus: Slice { ptr: descriptors.as_ptr(), len: descriptors.len() },
            row_h: menu.row_h.unwrap_or(0.0), max_rows: menu.max_rows.map_or(0, |rows| rows.min(u32::MAX as usize) as u32), checked: Slice { ptr: marks.as_ptr(), len: marks_len },
            padding: menu.padding, has_name: menu.name.is_some() as u8, reserved2: [0; 3], name: as_str(menu.name.unwrap_or("")),
            selected: Slice { ptr: current.as_ptr(), len: if selected { n } else { 0 } },
            focus_on_click: menu.focus_on_click as u8, reserved3: [0; 7],
            ids: Slice { ptr: ids.as_ptr(), len: if with_ids { n } else { 0 } }, has_id: Slice { ptr: has_id.as_ptr(), len: if with_ids { n } else { 0 } },
            max_h: menu.max_h.unwrap_or(0.0), reserved4: 0,
        };
        let (has_width, width) = menu.w.map_or((0, 0.0), |width| (1, width));
        let (mut tag, mut index) = (0, -1i64);
        check(self, unsafe { raw::pxui_ui_context_menu(
            self.0.as_ptr(), at.0, at.1, as_str(label), Slice { ptr: labels.as_ptr(), len: n },
            Slice { ptr: enabled.as_ptr(), len: n }, has_width, width, 0, 0, if extras { &extra } else { core::ptr::null() }, &mut tag, &mut index,
        ) });
        if n > ROWS { MENU_COLUMNS.set((hl, he, hk, hs, hm, hd, hc, hi, hh)); }
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
            as_str(query), Some(rows), options.mark.map(|_| mark as extern "C" fn(*mut c_void, i64, *mut pxui_types::geom::Rgba) -> u8), options.off.map(|_| off as extern "C" fn(*mut c_void, i64) -> u8), &mut rows_ctx as *mut _ as *mut c_void,
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
    use super::Slice;
    #[cfg(not(feature = "static"))]
    use super::raw::pxui_ui_context_menu;
    use crate::{as_str, FrameIn, Ui};
    use pxui_types::{ffi::{PxuiMenuExtra, PxuiSubmenu}, Str};

    fn extra(size: u32) -> PxuiMenuExtra {
        let none = Slice { ptr: core::ptr::null(), len: 0 };
        PxuiMenuExtra {
            size, reserved: 0, lead_from: 0, keys: none, swatches: Slice { ptr: core::ptr::null(), len: 0 }, danger: Slice { ptr: core::ptr::null(), len: 0 },
            submenus: Slice { ptr: core::ptr::null(), len: 0 }, row_h: 0.0, max_rows: 0, checked: Slice { ptr: core::ptr::null(), len: 0 },
            padding: 6.0, has_name: 0, reserved2: [0; 3], name: Str::default(), selected: Slice { ptr: core::ptr::null(), len: 0 },
            focus_on_click: 0, reserved3: [0; 7], ids: Slice { ptr: core::ptr::null(), len: 0 }, has_id: Slice { ptr: core::ptr::null(), len: 0 },
            max_h: 0.0, reserved4: 0,
        }
    }

    /// Calls the export with `extra` over two rows and returns its status.
    fn call(ui: &mut Ui, extra: &PxuiMenuExtra) -> i32 {
        let (rows, enabled) = ([as_str("One"), as_str("Two")], [1u8, 1]);
        let (mut tag, mut index) = (0, -1i64);
        unsafe { pxui_ui_context_menu(ui.0.as_ptr(), 10.0, 10.0, as_str("m"), Slice { ptr: rows.as_ptr(), len: 2 }, Slice { ptr: enabled.as_ptr(), len: 2 }, 0, 0.0, 0, 0, extra, &mut tag, &mut index) }
    }

    /// A caller built before 1.49 passes the 80-byte extra and is served (and one of 1.49, 104 bytes); a shorter one, a negative row height,
    /// a submenu on a row that is not there or out of order, a mark byte over 2, and from 1.55 a negative padding, a name flag or a
    /// current-choice byte over 1 or more choices than rows are refused; from 1.58 an id flag or focus flag over 1, more id flags than ids,
    /// or a reserved byte set; from 1.67 a negative or non-finite `max_h` or a reserved word set (the 1.58 record is still read).
    #[test]
    fn the_extra_is_read_by_its_size_and_validated_whole() {
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
        let mut results = Vec::new();
        ui.frame(&FrameIn::new(320.0, 240.0), &mut |ui: &mut Ui| {
            let danger = [1i64];
            let old = PxuiMenuExtra { danger: Slice { ptr: danger.as_ptr(), len: 1 }, ..extra(80) };
            results.push(call(ui, &old));
            results.push(call(ui, &extra(79)));
            results.push(call(ui, &PxuiMenuExtra { row_h: -1.0, ..extra(104) }));
            results.push(call(ui, &PxuiMenuExtra { row_h: 30.0, ..extra(104) }));
            let marks = [0u8, 3];
            results.push(call(ui, &PxuiMenuExtra { checked: Slice { ptr: marks.as_ptr(), len: 2 }, ..extra(104) }));
            let (texts, enabled) = ([as_str("a")], [1u8]);
            let sub = |row| PxuiSubmenu { row, current: 0, has_current: 0, reserved: [0; 7], rows: Slice { ptr: texts.as_ptr(), len: 1 }, enabled: Slice { ptr: enabled.as_ptr(), len: 1 }, keys: Slice { ptr: core::ptr::null::<Str>(), len: 0 } };
            let subs = [sub(1), sub(0)];
            results.push(call(ui, &PxuiMenuExtra { submenus: Slice { ptr: subs.as_ptr(), len: 2 }, ..extra(104) }));
            let subs = [sub(2)];
            results.push(call(ui, &PxuiMenuExtra { submenus: Slice { ptr: subs.as_ptr(), len: 1 }, ..extra(104) }));
            let subs = [sub(0), sub(1)];
            results.push(call(ui, &PxuiMenuExtra { submenus: Slice { ptr: subs.as_ptr(), len: 2 }, ..extra(104) }));
            let full = core::mem::size_of::<PxuiMenuExtra>() as u32;
            let (on, over, three) = ([0u8, 1], [2u8], [1u8, 0, 0]);
            results.push(call(ui, &PxuiMenuExtra { padding: 10.0, has_name: 1, name: as_str("Sizes"), selected: Slice { ptr: on.as_ptr(), len: 2 }, ..extra(full) }));
            results.push(call(ui, &PxuiMenuExtra { padding: -1.0, ..extra(full) }));
            results.push(call(ui, &PxuiMenuExtra { has_name: 2, ..extra(full) }));
            results.push(call(ui, &PxuiMenuExtra { selected: Slice { ptr: over.as_ptr(), len: 1 }, ..extra(full) }));
            results.push(call(ui, &PxuiMenuExtra { selected: Slice { ptr: three.as_ptr(), len: 3 }, ..extra(full) }));
            // 1.58: the 144-byte record of 1.55 is still read; ids with their flags, a flag over 1, more flags than ids or rows, a
            // focus flag over 1 and a non-zero reserved byte
            let (ids, has, bad) = ([7i64, 9], [1u8, 0], [2u8]);
            results.push(call(ui, &PxuiMenuExtra { padding: 10.0, ..extra(144) }));
            results.push(call(ui, &PxuiMenuExtra { focus_on_click: 1, ids: Slice { ptr: ids.as_ptr(), len: 2 }, has_id: Slice { ptr: has.as_ptr(), len: 2 }, ..extra(full) }));
            results.push(call(ui, &PxuiMenuExtra { ids: Slice { ptr: ids.as_ptr(), len: 1 }, has_id: Slice { ptr: bad.as_ptr(), len: 1 }, ..extra(full) }));
            results.push(call(ui, &PxuiMenuExtra { ids: Slice { ptr: ids.as_ptr(), len: 1 }, has_id: Slice { ptr: has.as_ptr(), len: 2 }, ..extra(full) }));
            results.push(call(ui, &PxuiMenuExtra { focus_on_click: 2, ..extra(full) }));
            results.push(call(ui, &PxuiMenuExtra { reserved3: [0, 1, 0, 0, 0, 0, 0], ..extra(full) }));
            // 1.67: the 184-byte record of 1.58 is still read (its tail never), one ending inside `max_h` is refused; a negative or
            // non-finite height and a non-zero reserved word are refused
            results.push(call(ui, &PxuiMenuExtra { focus_on_click: 1, max_h: -1.0, reserved4: 9, ..extra(184) }));
            results.push(call(ui, &extra(186)));
            results.push(call(ui, &PxuiMenuExtra { max_h: 120.0, ..extra(full) }));
            results.push(call(ui, &PxuiMenuExtra { max_h: -1.0, ..extra(full) }));
            results.push(call(ui, &PxuiMenuExtra { max_h: f32::INFINITY, ..extra(full) }));
            results.push(call(ui, &PxuiMenuExtra { max_h: 120.0, reserved4: 1, ..extra(full) }));
        }).ok();
        let accepted: Vec<bool> = results.iter().map(|status| *status == 0).collect();
        assert_eq!(accepted, [true, false, false, true, false, false, false, true, true, false, false, false, false, true, true, false, false, false, false,
            true, false, true, false, false, false]);
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
        { let kept = super::MENU_COLUMNS.take(); assert_eq!(kept.0.capacity(), 0, "nothing was copied for the refused menu"); }
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
        ui.frame(&FrameIn::new(200.0, 100.0), &mut |ui: &mut Ui| { ui.menu("long", Point::new(0.0, 0.0), &items[..super::MENU_ROWS], &Menu::DEFAULT); }).expect("a menu at the cap is served");
        #[cfg(not(feature = "static"))]
        { let kept = super::MENU_COLUMNS.take(); assert!(kept.0.capacity() <= super::MENU_ROWS + super::MENU_ROWS / 2, "kept columns stay near the cap"); }
    }
}
