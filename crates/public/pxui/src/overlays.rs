use core::{cell::Cell, ffi::c_void};
use pxui_types::{PxuiBox, Str, ffi::Slice, ui::{Key, Menu, MenuItem, MenuResponse, Pick}};
#[cfg(not(feature = "static"))]
use pxui_types::ffi::{PxuiMenuExtra, PxuiSubmenu, PxuiSwatch};
use crate::{composite::{defer_callback_panic, deferred}, check, Strs, Ui};

use crate::PickerOpts;

#[cfg(feature = "static")]
mod raw {
    pub use pxui_abi_link::{pxui_ui_picker, pxui_ui_toasts};
}

#[cfg(not(feature = "static"))]
mod raw {
    use core::ffi::c_void;
    use pxui_types::{PxuiBox, Str, ffi::{PxuiMenuExtra, Slice}};
    use crate::sys::PxuiUi;
    pub type PickerRows = Option<extern "C" fn(*mut c_void, Str, *mut Slice<Str>)>;
    pub type PickerMark = Option<extern "C" fn(*mut c_void, i64, *mut pxui_types::geom::Rgba) -> u8>;
    pub type PickerOff = Option<extern "C" fn(*mut c_void, i64) -> u8>;
    #[link(name = "pxui")]
    unsafe extern "C" {
    /// The C entry `Ui::tip` calls: shows the tooltip `label` for `box_` while the pointer rests on it; an error code for a stale box or invalid text.
    /// Allocates: never on a warm frame.
    /// Identity: the tooltip is keyed by its text, so two tooltips of one text share one.
    /// Panics: never.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let b = ui.node("n".into(), pxui::Node::DEFAULT);
    ///     ui.tip(&b, "Shuffle");
    /// });
    /// ```
    pub fn pxui_ui_tip(ui:*mut PxuiUi,box_:*const PxuiBox,label:Str)->i32;
    /// The C entry `Ui::toasts` calls: stacks the toast texts `items` in the window's lower end corner and writes the index of one clicked away or expired to `out_index`, or -1; an error code for a non-finite or negative `seconds`.
    /// Allocates: never on a warm frame.
    /// Identity: `label` names the stack (its accessible name); one stack per label in a parent.
    /// Panics: never.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| { ui.toasts("toasts", &["Saved"], 3.0); });
    /// ```
    pub fn pxui_ui_toasts(ui:*mut PxuiUi,label:Str,items:Slice<Str>,seconds:f64,out_index:*mut i64)->i32;
    /// The C entry `Ui::menu` calls (the dynamic-link path): shows the menu `items` at (`x`, `y`) with the enabled flags, optional width and selected row and the `extra` columns (keys, swatches, danger rows, submenus, marks); writes the outcome tag (0 open, 1 chosen, 2 dismissed) and the chosen row to the outputs; an error code for invalid arguments.
    /// `extra` is read by its `size` field, so an older caller's shorter record is served.
    /// Allocates: never on a warm frame.
    /// Identity: `label` keys the menu and is its accessible name.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::ui::{Menu, MenuItem};
    /// pxui::doc::frames(1, &mut |ui| { ui.menu("m", (10.0, 10.0), &[MenuItem::new("Open")], &Menu::DEFAULT); });
    /// ```
    pub fn pxui_ui_context_menu(ui:*mut PxuiUi,x:f32,y:f32,label:Str,items:Slice<Str>,enabled:Slice<u8>,has_width:u8,width:f32,has_selected:u8,selected:i64,extra:*const PxuiMenuExtra,out_tag:*mut u32,out_index:*mut i64)->i32;
    /// The C entry `Ui::picker` and `Ui::picker_filled` call: a search field over the rows `rows` computes for the query, windowed to `limit` rows; writes the edited query, the outcome tag (0 none, 1 pick, 2 delete, 3 submit, 4 back, 5 cancel) and the row index; an error code for invalid arguments.
    /// The callbacks run during the call with `user`; the query and the rows they hand over are borrowed until the call returns.
    /// Allocates: never on a warm frame.
    /// Identity: `label` is the field's placeholder and its key.
    /// Panics: never.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     ui.picker(&pxui::PickerOpts::DEFAULT, "find", "", &|_query| vec![("Song".to_owned(), String::new())]);
    /// });
    /// ```
    pub fn pxui_ui_picker(ui:*mut PxuiUi,limit:i64,slash:u8,at_rest:u8,label:Str,query:Str,rows:PickerRows,mark:PickerMark,off:PickerOff,user:*mut c_void,out_query:*mut Str,out_tag:*mut u32,out_index:*mut i64)->i32;
    }
}

#[inline]
fn as_str(value: &str) -> Str { crate::as_str(value) }

// The rows a picker holds between its calls on this thread: its row strings (the app's fill reuses them) and their `Str` span, so a
// settled picker allocates nothing here. Taken for the call and put back, so a picker built inside a picker's callback finds them empty.
thread_local! { static ROWS: Cell<(Vec<(String, String)>, Vec<Str>)> = const { Cell::new((Vec::new(), Vec::new())) }; }

// The columns of a menu of more than 32 rows, kept on this thread between calls so a settled long menu allocates nothing here.
#[cfg(not(feature = "static"))]
type MenuColumns = (Vec<Str>, Vec<u8>, Vec<Str>, Vec<PxuiSwatch>, Vec<u8>, Vec<i64>, Vec<u8>, Vec<i64>, Vec<u8>);
#[cfg(not(feature = "static"))]
thread_local! { static MENU_COLUMNS: Cell<MenuColumns> = const { Cell::new((Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new())) }; }

struct Rows<'a, F: ?Sized> {
    fill: &'a mut F,
    owned: Vec<(String, String)>,
    raw: Vec<Str>,
    options: &'a PickerOpts<'a>,
}
extern "C" fn mark<F: ?Sized>(user: *mut c_void, row: i64, out: *mut pxui_types::geom::Rgba) -> u8 {
    if user.is_null() || out.is_null() || deferred() { return 0; }
    let ctx = unsafe { &*user.cast::<Rows<F>>() };
    let Some(mark) = ctx.options.mark else { return 0 };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| mark(row.max(0) as usize))) {
        Ok(Some(color)) => { unsafe { *out = color; } 1 }
        Ok(None) => 0,
        Err(panic) => { defer_callback_panic(panic); 0 }
    }
}
extern "C" fn off<F: ?Sized>(user: *mut c_void, row: i64) -> u8 {
    if user.is_null() || deferred() { return 0; }
    let ctx = unsafe { &*user.cast::<Rows<F>>() };
    let Some(off) = ctx.options.off else { return 0 };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| off(row.max(0) as usize))) { Ok(v) => v as u8, Err(panic) => { defer_callback_panic(panic); 0 } }
}
extern "C" fn rows<F: FnMut(&str, &mut Vec<(String, String)>) + ?Sized>(user: *mut c_void, query: Str, out: *mut Slice<Str>) {
    if user.is_null() || out.is_null() || deferred() { return; }
    let ctx = unsafe { &mut *user.cast::<Rows<F>>() };
    let query_bytes = if query.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(query.ptr, query.len) } };
    let Ok(query) = core::str::from_utf8(query_bytes) else { unsafe { *out = Slice { ptr: core::ptr::null(), len: 0 }; } return; };
    let (fill, owned) = (&mut *ctx.fill, &mut ctx.owned);
    if let Err(panic) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| fill(query, owned))) { defer_callback_panic(panic); owned.clear(); }
    ctx.raw.clear();
    for (label, detail) in &ctx.owned { ctx.raw.push(as_str(label)); ctx.raw.push(as_str(detail)); }
    unsafe { *out = Slice { ptr: ctx.raw.as_ptr(), len: ctx.raw.len() }; }
}

impl Ui {
    #[inline]
    #[track_caller] pub fn tip(&mut self, box_: &PxuiBox, text: &str)  {
        #[cfg(feature = "static")]
        check(self, unsafe { pxui_abi_link::trusted::tip(self.0.as_ptr(), box_, text) });
        #[cfg(not(feature = "static"))]
        check(self, unsafe { raw::pxui_ui_tip(self.0.as_ptr(), box_, as_str(text)) })
    }

    #[inline]
    #[track_caller] pub fn toasts(&mut self, label: &str, items: &[&str], seconds: f64) -> Option<usize> {
        let items = Strs::new(items);
        let mut index: i64 = -1;
        check(self, unsafe { raw::pxui_ui_toasts(self.0.as_ptr(), as_str(label), items.span(), seconds, &mut index) });
        (index >= 0).then_some(index as usize)
    }

    /// The one body of `menu`: linked statically, the library's trusted entry with the rows as they are; else the rows as the C export's
    /// columns (on the stack up to 32 rows, in buffers kept on the thread past that; submenus always on the heap), then the call.
    #[track_caller]
    pub(crate) fn menu_keyed(&mut self, key: Key, at: (f32, f32), items: &[MenuItem], menu: &Menu) -> MenuResponse {
        #[cfg(feature = "static")]
        {
            let mut out = MenuResponse::default();
            check(self, unsafe { pxui_abi_link::trusted::menu(self.0.as_ptr(), key, at, items, menu, &mut out) });
            out
        }
        #[cfg(not(feature = "static"))]
        self.menu_columns(key, at, items, menu)
    }

    #[cfg(not(feature = "static"))]
    #[track_caller]
    fn menu_columns(&mut self, key: Key, at: (f32, f32), items: &[MenuItem], menu: &Menu) -> MenuResponse {
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
        let (mut dangers, mut extras) = (0, menu.row_h.is_some() || menu.max_rows.is_some() || menu.padding != 6.0 || menu.name.is_some() || menu.focus_on_click);
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
        };
        let (has_width, width) = menu.width.map_or((0, 0.0), |width| (1, width));
        let (mut tag, mut index) = (0, -1i64);
        check(self, unsafe { raw::pxui_ui_context_menu(
            self.0.as_ptr(), at.0, at.1, as_str(label), Slice { ptr: labels.as_ptr(), len: n },
            Slice { ptr: enabled.as_ptr(), len: n }, has_width, width, 0, 0, if extras { &extra } else { core::ptr::null() }, &mut tag, &mut index,
        ) });
        if n > ROWS { MENU_COLUMNS.set((hl, he, hk, hs, hm, hd, hc, hi, hh)); }
        match tag { 0 => MenuResponse::default(), 1 => MenuResponse { chosen: Some(index as usize), ..MenuResponse::default() }, 2 => MenuResponse { dismissed: true, ..MenuResponse::default() }, _ => unreachable!("the library answers a menu with tag 0, 1 or 2") }
    }

    /// A focused search row over the `(label, detail)` rows `rows_of(query)` returns for the current query, windowed to `options.limit` (10) around the cursor; returns the edited query and what happened to it.
    /// Indices in `Pick` refer to the rows of the returned query. `Pick::Pick` comes on Enter or a row click, `Submit` on Enter with no rows, `Delete` on a second Delete over the same armed row, `Back` on Backspace or Left with an empty query, `Cancel` on Escape. Build it inside a panel or `modal`.
    /// Allocates: a fresh row vector and the strings in it on every call; use `picker_filled` to reuse them.
    /// Identity: `label` is the placeholder and keys the field; fields with the same label in one parent collide.
    /// Panics: when the library refuses the call (outside a frame, or text over the limit).
    ///
    /// ```
    /// use pxui::ui::Pick;
    /// let mut ui = pxui::doc::frames(1, &mut |_| {});
    /// pxui::testing::frame(&mut ui, &[], &mut |ui| {
    ///     let (query, pick) = ui.picker(&pxui::PickerOpts::DEFAULT, "find", "", &|_q| vec![("Song".to_owned(), "Artist".to_owned())]);
    ///     assert_eq!((query.as_str(), pick), ("", Pick::None));
    /// });
    /// ```
    #[inline]
    #[track_caller]
    pub fn picker<F: Fn(&str) -> Vec<(String, String)> + ?Sized>(&mut self, options: &PickerOpts<'_>, label: &str, query: &str, rows_of: &F) -> (String, Pick) {
        self.picker_filled(options, label, query, &mut |query: &str, rows: &mut Vec<(String, String)>| *rows = rows_of(query))
    }

    /// `picker` that gets its rows by filling a vector kept between frames: `fill(query, rows)` replaces `rows` with the rows for `query`, reusing the strings already in it.
    /// Allocates: never once the rows have settled (the row strings and their span are kept on the thread); a first fill allocates them.
    /// Identity: as `picker`.
    /// Panics: as `picker`.
    ///
    /// ```
    /// let mut ui = pxui::doc::frames(1, &mut |_| {});
    /// pxui::testing::frame(&mut ui, &[], &mut |ui| {
    ///     ui.picker_filled(&pxui::PickerOpts::DEFAULT, "find", "", &mut |_query, rows| {
    ///         rows.clear();
    ///         rows.push(("Song".to_owned(), "Artist".to_owned()));
    ///     });
    /// });
    /// ```
    #[inline]
    #[track_caller]
    pub fn picker_filled<F: FnMut(&str, &mut Vec<(String, String)>) + ?Sized>(&mut self, options: &PickerOpts<'_>, label: &str, query: &str, fill: &mut F) -> (String, Pick) {
        let (owned, raw) = ROWS.take();
        let mut rows_ctx = Rows { fill, owned, raw, options };
        let mut out_query = Str::default();
        let mut tag = 0;
        let mut index: i64 = -1;
        check(self, unsafe { raw::pxui_ui_picker(
            self.0.as_ptr(), i64::try_from(options.limit).expect("pxui: a picker row limit over i64::MAX"), options.slash as u8, options.at_rest as u8,
            as_str(label), as_str(query), Some(rows::<F>), options.mark.map(|_| mark::<F> as extern "C" fn(*mut c_void, i64, *mut pxui_types::geom::Rgba) -> u8), options.off.map(|_| off::<F> as extern "C" fn(*mut c_void, i64) -> u8), &mut rows_ctx as *mut _ as *mut c_void,
            &mut out_query, &mut tag, &mut index,
        ) });
        let Rows { mut owned, mut raw, .. } = rows_ctx;
        raw.clear();
        ROWS.set((core::mem::take(&mut owned), raw));
        let pick = match tag { 0 => Pick::None, 1 => Pick::Pick(index), 2 => Pick::Delete(index), 3 => Pick::Submit, 4 => Pick::Back, 5 => Pick::Cancel, _ => unreachable!("the library answers a picker with tag 0 to 5") };
        (crate::lent_string(out_query), pick)
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
    /// or a reserved byte set.
    #[test]
    fn the_extra_is_read_by_its_size_and_validated_whole() {
        let mut ui = Ui::new().unwrap();
        let mut results = Vec::new();
        ui.frame(&FrameIn { width: 320.0, height: 240.0, ..FrameIn::default() }, &mut |ui: &mut Ui| {
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
        }).ok();
        let accepted: Vec<bool> = results.iter().map(|status| *status == 0).collect();
        assert_eq!(accepted, [true, false, false, true, false, false, false, true, true, false, false, false, false, true, true, false, false, false, false]);
    }
}
