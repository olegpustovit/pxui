use core::ffi::c_void;
use pxui_types::{PxuiBox, Str, ffi::Slice, ui::{MenuResult, Pick}};
use crate::{composite::{defer_callback_panic, deferred}, status, Strs, Ui};

#[derive(Clone, Debug, Default)]
pub struct MenuOpts { pub width: Option<f32>, pub selected: Option<i64> }

#[derive(Clone, Debug)]
pub struct PickerOpts { pub limit: i64, pub slash: bool, pub at_rest: bool }
impl Default for PickerOpts { fn default() -> Self { Self { limit: 10, slash: true, at_rest: false } } }

#[cfg(feature = "static")]
mod raw {
    pub use pxui_abi_link::{pxui_ui_context_menu, pxui_ui_picker, pxui_ui_tip, pxui_ui_toasts};
}

#[cfg(not(feature = "static"))]
mod raw {
    use core::ffi::c_void;
    use pxui_types::{PxuiBox, Str, ffi::Slice};
    use crate::sys::PxuiUi;
    pub type PickerRows = Option<extern "C" fn(*mut c_void, Str, *mut Slice<Str>)>;
    #[link(name = "pxui")]
    unsafe extern "C" {
    pub fn pxui_ui_tip(ui:*mut PxuiUi,box_:*const PxuiBox,label:Str)->i32;
    pub fn pxui_ui_toasts(ui:*mut PxuiUi,label:Str,items:Slice<Str>,seconds:f64,out_index:*mut i64)->i32;
    pub fn pxui_ui_context_menu(ui:*mut PxuiUi,x:f32,y:f32,label:Str,items:Slice<Str>,enabled:Slice<u8>,has_width:u8,width:f32,has_selected:u8,selected:i64,out_tag:*mut u32,out_index:*mut i64)->i32;
    pub fn pxui_ui_picker(ui:*mut PxuiUi,limit:i64,slash:u8,at_rest:u8,label:Str,query:Str,rows:PickerRows,user:*mut c_void,out_query:*mut Str,out_tag:*mut u32,out_index:*mut i64)->i32;
    }
}

#[inline]
fn as_str(value: &str) -> Str { Str { ptr: value.as_ptr(), len: value.len() } }

struct Rows<F> { rows_of: F, owned: Vec<(String, String)>, raw: Vec<Str> }
extern "C" fn rows<F: Fn(&str) -> Vec<(String, String)>>(user: *mut c_void, query: Str, out: *mut Slice<Str>) {
    if user.is_null() || out.is_null() || deferred() { return; }
    let ctx = unsafe { &mut *user.cast::<Rows<F>>() };
    let query_bytes = if query.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(query.ptr, query.len) } };
    let Ok(query) = core::str::from_utf8(query_bytes) else { unsafe { *out = Slice { ptr: core::ptr::null(), len: 0 }; } return; };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (ctx.rows_of)(query)));
    ctx.owned = match result { Ok(rows) => rows, Err(panic) => { defer_callback_panic(panic); Vec::new() } };
    ctx.raw.clear();
    for (label, detail) in &ctx.owned { ctx.raw.push(as_str(label)); ctx.raw.push(as_str(detail)); }
    unsafe { *out = Slice { ptr: ctx.raw.as_ptr(), len: ctx.raw.len() }; }
}

impl Ui {
    #[inline]
    pub fn tip(&mut self, box_: &PxuiBox, text: &str) -> Result<(), i32> {
        status(unsafe { raw::pxui_ui_tip(self.0.as_ptr(), box_, as_str(text)) })
    }

    #[inline]
    pub fn toasts(&mut self, label: &str, items: &[&str], seconds: f64) -> Result<Option<usize>, i32> {
        let items = Strs::new(items);
        let mut index: i64 = -1;
        status(unsafe { raw::pxui_ui_toasts(self.0.as_ptr(), as_str(label), items.span(), seconds, &mut index) })?;
        Ok((index >= 0).then_some(index as usize))
    }

    #[inline]
    pub fn context_menu(&mut self, at: (f32, f32), options: &MenuOpts, label: &str, items: &[(String, bool)]) -> Result<MenuResult, i32> {
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
        let mut tag = 0;
        let mut index: i64 = -1;
        status(unsafe { raw::pxui_ui_context_menu(
            self.0.as_ptr(), at.0, at.1, as_str(label), Slice { ptr: labels.as_ptr(), len: labels.len() },
            Slice { ptr: enabled.as_ptr(), len: enabled.len() }, has_width, width, has_selected, selected, &mut tag, &mut index,
        ) })?;
        match tag { 0 => Ok(MenuResult::Open), 1 => Ok(MenuResult::Pick(index)), 2 => Ok(MenuResult::Dismiss), _ => Err(-1) }
    }

    #[inline]
    pub fn picker<F: Fn(&str) -> Vec<(String, String)>>(&mut self, options: &PickerOpts, label: &str, query: &str, rows_of: F) -> Result<(String, Pick), i32> {
        let mut rows_ctx = Rows { rows_of, owned: Vec::new(), raw: Vec::new() };
        let mut out_query = Str::default();
        let mut tag = 0;
        let mut index: i64 = -1;
        status(unsafe { raw::pxui_ui_picker(
            self.0.as_ptr(), options.limit, options.slash as u8, options.at_rest as u8,
            as_str(label), as_str(query), Some(rows::<F>), &mut rows_ctx as *mut _ as *mut c_void,
            &mut out_query, &mut tag, &mut index,
        ) })?;
        if out_query.len > 1 << 20 || (out_query.len != 0 && out_query.ptr.is_null()) { return Err(-1); }
        let bytes = if out_query.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(out_query.ptr, out_query.len) } };
        let query = String::from_utf8(bytes.to_vec()).map_err(|_| -1)?;
        let pick = match tag { 0 => Pick::None, 1 => Pick::Pick(index), 2 => Pick::Delete(index), 3 => Pick::Submit, 4 => Pick::Back, 5 => Pick::Cancel, _ => return Err(-1) };
        Ok((query, pick))
    }
}
