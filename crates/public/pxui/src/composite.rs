use core::{any::Any, ffi::c_void, mem::ManuallyDrop, ptr::NonNull};
use pxui_types::{PxuiBox, Size, Str, geom::Rect, layout::{Align, Placement, Side}, ffi::{PxuiAnchorDesc, PxuiDialogOut}};
#[cfg(not(feature = "static"))]
use crate::node::size;
use crate::{sys, Strs, Ui, check};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogOut { Open, Closed, Button(usize) }

// A panic in an app callback must not unwind through the library. The first one is kept here and every later callback
// of the call is skipped; the outermost call that can run callbacks (frame, prepare, draw) resumes it after it returns.
thread_local! { static CALLBACK_PANIC: std::cell::RefCell<Option<Box<dyn Any + Send>>> = const { std::cell::RefCell::new(None) }; }
/// How many threads hold a deferred panic: while none does (every call of a settled app), a callback skips the thread-local lookup.
static PENDING: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);
pub(crate) fn take_callback_panic() -> Option<Box<dyn Any + Send>> {
    if PENDING.load(core::sync::atomic::Ordering::Relaxed) == 0 { return None; }
    let taken = CALLBACK_PANIC.with(|slot| slot.borrow_mut().take());
    if taken.is_some() { PENDING.fetch_sub(1, core::sync::atomic::Ordering::Relaxed); }
    taken
}
#[inline]
pub(crate) fn deferred() -> bool { PENDING.load(core::sync::atomic::Ordering::Relaxed) != 0 && CALLBACK_PANIC.with(|slot| slot.borrow().is_some()) }
#[inline]
pub(crate) fn resume_deferred() { if let Some(panic) = take_callback_panic() { std::panic::resume_unwind(panic) } }

pub(crate) fn defer_callback_panic(panic: Box<dyn Any + Send>) {
    CALLBACK_PANIC.with(|slot| { let mut slot = slot.borrow_mut(); if slot.is_none() { *slot = Some(panic); PENDING.fetch_add(1, core::sync::atomic::Ordering::Relaxed); } });
}

pub(crate) extern "C" fn call<F: FnMut(&mut Ui)>(user: *mut c_void, raw: *mut c_void) {
    if raw.is_null() || deferred() { return; }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut ui = ManuallyDrop::new(Ui(NonNull::new(raw.cast()).unwrap()));
        (unsafe { &mut *user.cast::<F>() })(&mut ui);
    }));
    if let Err(panic) = result { defer_callback_panic(panic); }
}

fn encode_placement(value: Placement) -> PxuiAnchorDesc {
    let side = match value.side { Side::Below => 0, Side::Above => 1, Side::Right => 2, Side::Left => 3 };
    let align = match value.align { Align::Start => 0, Align::Center => 1, Align::End => 2, Align::Stretch => 3, Align::Baseline => 4 };
    PxuiAnchorDesc { has_anchor: 1, reserved: [0; 3], rect: Rect::default(), side, align, gap: value.gap, margin: value.margin, flip: value.flip as u8, reserved_tail: [0; 3] }
}


impl Ui {
    #[inline]
    #[track_caller] pub fn modal(&mut self, width: Option<f32>, label: &str, mut build: &mut dyn FnMut(&mut Ui)) -> bool {
        #[cfg(feature = "static")]
        { let mut out = false; check(self, unsafe { pxui_abi_link::trusted::modal(self.0.as_ptr(), label, width, inner!(|ui: &mut Ui| build(ui)), &mut out) }); out }
        #[cfg(not(feature = "static"))]
        { let mut out = 0; check(self, unsafe { sys::pxui_ui_modal(self.0.as_ptr(), as_str(label), width.is_some() as u8, width.unwrap_or_default(), Some(call::<&mut dyn FnMut(&mut Ui)>), &mut build as *mut _ as *mut c_void, &mut out) }); out != 0 }
    }

    #[inline]
    #[track_caller] pub fn scroll_view(&mut self, label: &str, width: Size, height: Size, mut build: &mut dyn FnMut(&mut Ui)) -> PxuiBox {
        let mut out = PxuiBox::default();
        #[cfg(feature = "static")]
        check(self, unsafe { pxui_abi_link::trusted::scroll_view(self.0.as_ptr(), label, width, height, inner!(|ui: &mut Ui| build(ui)), &mut out) });
        #[cfg(not(feature = "static"))]
        check(self, unsafe { sys::pxui_ui_scroll_view(self.0.as_ptr(), as_str(label), size(width), size(height), Some(call::<&mut dyn FnMut(&mut Ui)>), &mut build as *mut _ as *mut c_void, &mut out) });
        out
    }

    #[inline]
    #[track_caller] pub fn view(&mut self, label: &str, revision: u64, width: Size, height: Size, mut build: &mut dyn FnMut(&mut Ui)) -> PxuiBox {
        let mut out = PxuiBox::default();
        #[cfg(feature = "static")]
        let rc = unsafe { pxui_abi_link::trusted::view(self.0.as_ptr(), label, revision, width, height, false, inner!(|ui: &mut Ui| build(ui)), &mut out) };
        #[cfg(not(feature = "static"))]
        let rc = unsafe { sys::pxui_ui_view(self.0.as_ptr(), as_str(label), revision, size(width), size(height), Some(call::<&mut dyn FnMut(&mut Ui)>), &mut build as *mut _ as *mut c_void, &mut out) };
        check(self, rc);
        out
    }

    #[inline]
    #[track_caller] pub fn anchored(&mut self, label: &str, anchor: Rect, placement: Placement, width: Size, height: Size, mut build: &mut dyn FnMut(&mut Ui)) -> PxuiBox {
        let mut out = PxuiBox::default();
        #[cfg(feature = "static")]
        check(self, unsafe { pxui_abi_link::trusted::anchored(self.0.as_ptr(), label, anchor, placement, width, height, inner!(|ui: &mut Ui| build(ui)), &mut out) });
        #[cfg(not(feature = "static"))]
        {
            let mut descriptor = encode_placement(placement);
            descriptor.rect = anchor;
            check(self, unsafe { sys::pxui_ui_anchored(self.0.as_ptr(), as_str(label), &descriptor, size(width), size(height), Some(call::<&mut dyn FnMut(&mut Ui)>), &mut build as *mut _ as *mut c_void, &mut out) });
        }
        out
    }

    #[inline]
    #[track_caller] pub fn dialog(&mut self, title: &str, buttons: &[&str], mut body: &mut dyn FnMut(&mut Ui)) -> DialogOut {
        let buttons = Strs::new(buttons);
        let mut out = PxuiDialogOut::default();
        check(self, unsafe { sys::pxui_ui_dialog(self.0.as_ptr(), as_str(title), buttons.span(), Some(call::<&mut dyn FnMut(&mut Ui)>), &mut body as *mut _ as *mut c_void, &mut out) });
        match out.tag { 0 => DialogOut::Open, 1 => DialogOut::Closed, 2 => DialogOut::Button(out.index), _ => unreachable!("the library answers a dialog with tag 0, 1 or 2") }
    }
}

#[inline]
fn as_str(value: &str) -> Str { Str { ptr: value.as_ptr(), len: value.len() } }
