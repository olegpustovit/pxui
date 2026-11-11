use core::{cell::Cell, ffi::c_void};
use pxui_types::{PxuiBox, Str, ffi::{PxuiMenuExtra, PxuiSubmenu, PxuiSwatch, Slice}, ui::{MenuResult, Pick}};
use crate::{composite::{defer_callback_panic, deferred}, check, Strs, Ui};

pub use pxui_types::ui::{MenuOpts, Submenu};
pub use crate::opts::PickerOpts;

#[cfg(feature = "static")]
mod raw {
    pub use pxui_abi_link::{pxui_ui_context_menu, pxui_ui_picker, pxui_ui_toasts};
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
    pub fn pxui_ui_tip(ui:*mut PxuiUi,box_:*const PxuiBox,label:Str)->i32;
    pub fn pxui_ui_toasts(ui:*mut PxuiUi,label:Str,items:Slice<Str>,seconds:f64,out_index:*mut i64)->i32;
    pub fn pxui_ui_context_menu(ui:*mut PxuiUi,x:f32,y:f32,label:Str,items:Slice<Str>,enabled:Slice<u8>,has_width:u8,width:f32,has_selected:u8,selected:i64,extra:*const PxuiMenuExtra,out_tag:*mut u32,out_index:*mut i64)->i32;
    pub fn pxui_ui_picker(ui:*mut PxuiUi,limit:i64,slash:u8,at_rest:u8,label:Str,query:Str,rows:PickerRows,mark:PickerMark,off:PickerOff,user:*mut c_void,out_query:*mut Str,out_tag:*mut u32,out_index:*mut i64)->i32;
    }
}

#[inline]
fn as_str(value: &str) -> Str { Str { ptr: value.as_ptr(), len: value.len() } }

// The rows a picker holds between its calls on this thread: its row strings (the app's fill reuses them) and their `Str` span, so a
// settled picker allocates nothing here. Taken for the call and put back, so a picker built inside a picker's callback finds them empty.
thread_local! { static ROWS: Cell<(Vec<(String, String)>, Vec<Str>)> = const { Cell::new((Vec::new(), Vec::new())) }; }

struct Rows<'a, F: ?Sized> { fill: &'a mut F, owned: Vec<(String, String)>, raw: Vec<Str>, options: &'a PickerOpts }
extern "C" fn mark<F: ?Sized>(user: *mut c_void, row: i64, out: *mut pxui_types::geom::Rgba) -> u8 {
    if user.is_null() || out.is_null() || deferred() { return 0; }
    let ctx = unsafe { &*user.cast::<Rows<F>>() };
    let Some(mark) = &ctx.options.mark else { return 0 };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| mark(row))) {
        Ok(Some(color)) => { unsafe { *out = color; } 1 }
        Ok(None) => 0,
        Err(panic) => { defer_callback_panic(panic); 0 }
    }
}
extern "C" fn off<F: ?Sized>(user: *mut c_void, row: i64) -> u8 {
    if user.is_null() || deferred() { return 0; }
    let ctx = unsafe { &*user.cast::<Rows<F>>() };
    let Some(off) = &ctx.options.off else { return 0 };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| off(row))) { Ok(v) => v as u8, Err(panic) => { defer_callback_panic(panic); 0 } }
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

    #[inline]
    #[track_caller] pub fn context_menu(&mut self, at: (f32, f32), options: &MenuOpts, label: &str, items: &[(String, bool)]) -> MenuResult {
        // up to 32 rows live on the stack, more on the heap
        let (mut labels, mut enabled) = ([Str::default(); 32], [0u8; 32]);
        let (heap_labels, heap_enabled): (Vec<Str>, Vec<u8>);
        let (labels, enabled): (&[Str], &[u8]) = if items.len() <= 32 {
            for ((label, flag), (text, on)) in labels.iter_mut().zip(enabled.iter_mut()).zip(items) { *label = as_str(text); *flag = *on as u8; }
            (&labels[..items.len()], &enabled[..items.len()])
        } else {
            heap_labels = items.iter().map(|(label, _)| as_str(label)).collect();
            heap_enabled = items.iter().map(|(_, on)| *on as u8).collect();
            (&heap_labels, &heap_enabled)
        };
        let (has_width, width) = options.width.map_or((0, 0.0), |width| (1, width));
        let (has_selected, selected) = options.selected.map_or((0, 0), |selected| (1, selected));
        // the extras (keys, swatches, danger rows, submenus) cross by descriptor; a plain menu passes none and allocates nothing
        let keys: Vec<Str> = options.keys.iter().map(|k| as_str(k)).collect();
        let swatches: Vec<PxuiSwatch> = options.swatches.iter().map(|s| PxuiSwatch { has: s.is_some() as u8, color: s.unwrap_or_default(), ..PxuiSwatch::default() }).collect();
        let sub_rows: Vec<(Vec<Str>, Vec<u8>, Vec<Str>)> = options.submenus.iter().map(|m| (m.rows.iter().map(|(t, _)| as_str(t)).collect(), m.rows.iter().map(|(_, on)| *on as u8).collect(), m.keys.iter().map(|k| as_str(k)).collect())).collect();
        let submenus: Vec<PxuiSubmenu> = options.submenus.iter().zip(&sub_rows).map(|(m, (rows, enabled, keys))| PxuiSubmenu {
            row: m.row, current: m.current.unwrap_or_default(), has_current: m.current.is_some() as u8, reserved: [0; 7],
            rows: Slice { ptr: rows.as_ptr(), len: rows.len() }, enabled: Slice { ptr: enabled.as_ptr(), len: enabled.len() }, keys: Slice { ptr: keys.as_ptr(), len: keys.len() },
        }).collect();
        let extra = PxuiMenuExtra {
            size: core::mem::size_of::<PxuiMenuExtra>() as u32, reserved: 0, lead_from: options.lead_from,
            keys: Slice { ptr: keys.as_ptr(), len: keys.len() }, swatches: Slice { ptr: swatches.as_ptr(), len: swatches.len() },
            danger: Slice { ptr: options.danger.as_ptr(), len: options.danger.len() }, submenus: Slice { ptr: submenus.as_ptr(), len: submenus.len() },
        };
        let plain = options.keys.is_empty() && options.swatches.is_empty() && options.danger.is_empty() && options.submenus.is_empty() && options.lead_from == 0;
        let mut tag = 0;
        let mut index: i64 = -1;
        check(self, unsafe { raw::pxui_ui_context_menu(
            self.0.as_ptr(), at.0, at.1, as_str(label), Slice { ptr: labels.as_ptr(), len: labels.len() },
            Slice { ptr: enabled.as_ptr(), len: enabled.len() }, has_width, width, has_selected, selected, if plain { core::ptr::null() } else { &extra }, &mut tag, &mut index,
        ) });
        match tag { 0 => MenuResult::Open, 1 => MenuResult::Pick(index), 2 => MenuResult::Dismiss, _ => unreachable!("the library answers a menu with tag 0, 1 or 2") }
    }

    /// A picker whose rows come from `rows_of(query)`, a fresh vector each call.
    #[inline]
    #[track_caller]
    pub fn picker<F: Fn(&str) -> Vec<(String, String)> + ?Sized>(&mut self, options: &PickerOpts, label: &str, query: &str, rows_of: &F) -> (String, Pick) {
        self.picker_filled(options, label, query, &mut |query: &str, rows: &mut Vec<(String, String)>| *rows = rows_of(query))
    }

    /// `picker` that asks for its rows by filling a vector kept between frames: `fill(query, rows)` replaces `rows` with the rows
    /// for `query`, reusing the strings already in it, so a settled picker allocates nothing.
    #[inline]
    #[track_caller]
    pub fn picker_filled<F: FnMut(&str, &mut Vec<(String, String)>) + ?Sized>(&mut self, options: &PickerOpts, label: &str, query: &str, fill: &mut F) -> (String, Pick) {
        let (owned, raw) = ROWS.take();
        let mut rows_ctx = Rows { fill, owned, raw, options };
        let mut out_query = Str::default();
        let mut tag = 0;
        let mut index: i64 = -1;
        check(self, unsafe { raw::pxui_ui_picker(
            self.0.as_ptr(), options.limit, options.slash as u8, options.at_rest as u8,
            as_str(label), as_str(query), Some(rows::<F>), options.mark.as_ref().map(|_| mark::<F> as extern "C" fn(*mut c_void, i64, *mut pxui_types::geom::Rgba) -> u8), options.off.as_ref().map(|_| off::<F> as extern "C" fn(*mut c_void, i64) -> u8), &mut rows_ctx as *mut _ as *mut c_void,
            &mut out_query, &mut tag, &mut index,
        ) });
        let Rows { mut owned, mut raw, .. } = rows_ctx;
        raw.clear();
        ROWS.set((core::mem::take(&mut owned), raw));
        let pick = match tag { 0 => Pick::None, 1 => Pick::Pick(index), 2 => Pick::Delete(index), 3 => Pick::Submit, 4 => Pick::Back, 5 => Pick::Cancel, _ => unreachable!("the library answers a picker with tag 0 to 5") };
        (crate::lent_string(out_query), pick)
    }
}
