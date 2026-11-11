use core::{any::Any, ffi::c_void, mem::ManuallyDrop, ptr::NonNull};
use pxui_types::{PxuiBox, Size, Str, geom::Rect, layout::{Align, Placement, Side}, ffi::{PxuiAnchorDesc, PxuiDialogOut, PxuiSizeDesc, Slice}};
use crate::{sys, Ui};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogOut { Open, Closed, Button(usize) }

thread_local! { static CALLBACK_PANIC: std::cell::RefCell<Option<Box<dyn Any + Send>>> = const { std::cell::RefCell::new(None) }; }
pub(crate) fn take_callback_panic() -> Option<Box<dyn Any + Send>> { CALLBACK_PANIC.with(|slot| slot.borrow_mut().take()) }

pub(crate) fn defer_callback_panic(panic: Box<dyn Any + Send>) {
    CALLBACK_PANIC.with(|slot| { let mut slot = slot.borrow_mut(); if slot.is_none() { *slot = Some(panic); } });
}

extern "C" fn call<F: FnMut(&mut Ui)>(user: *mut c_void, raw: *mut sys::PxuiUi) {
    if raw.is_null() { return; }
    let has_panicked = CALLBACK_PANIC.with(|slot| slot.borrow().is_some());
    if has_panicked { return; }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut ui = ManuallyDrop::new(Ui(NonNull::new(raw).unwrap()));
        (unsafe { &mut *user.cast::<F>() })(&mut ui);
    }));
    if let Err(panic) = result { defer_callback_panic(panic); }
}

fn size(value: Size) -> PxuiSizeDesc {
    match value {
        Size::Px(value) => PxuiSizeDesc { kind: 0, value, relative: None },
        Size::Pct(value) => PxuiSizeDesc { kind: 1, value, relative: None },
        Size::Rel(relative) => PxuiSizeDesc { kind: 2, value: 0.0, relative: Some(relative) },
        Size::Grow => PxuiSizeDesc { kind: 3, value: 0.0, relative: None },
        Size::Fit => PxuiSizeDesc { kind: 4, value: 0.0, relative: None },
    }
}

fn strings(values: &[&str]) -> (Vec<Str>, Slice<Str>) {
    let values: Vec<_> = values.iter().map(|value| Str { ptr: value.as_ptr(), len: value.len() }).collect();
    let span = Slice { ptr: values.as_ptr(), len: values.len() };
    (values, span)
}

fn encode_placement(value: Placement) -> PxuiAnchorDesc {
    let side = match value.side { Side::Below => 0, Side::Above => 1, Side::Right => 2, Side::Left => 3 };
    let align = match value.align { Align::Start => 0, Align::Center => 1, Align::End => 2, Align::Stretch => 3, Align::Baseline => 4 };
    PxuiAnchorDesc { has_anchor: 1, reserved: [0; 3], rect: Rect::default(), side, align, gap: value.gap, margin: value.margin, flip: value.flip as u8, reserved_tail: [0; 3] }
}

fn status(rc: i32) -> Result<(), i32> { if rc == 0 { Ok(()) } else { Err(rc) } }

impl Ui {
    pub fn modal<F: FnMut(&mut Ui)>(&mut self, label: &str, width: Option<f32>, mut build: F) -> Result<bool, i32> {
        let mut out = 0;
        status(unsafe { sys::pxui_ui_modal(self.0.as_ptr(), as_str(label), width.is_some() as u8, width.unwrap_or_default(), Some(call::<F>), &mut build as *mut _ as *mut c_void, &mut out) })?;
        Ok(out != 0)
    }

    pub fn scroll_view<F: FnMut(&mut Ui)>(&mut self, label: &str, width: Size, height: Size, mut build: F) -> Result<PxuiBox, i32> {
        let mut out = PxuiBox::default();
        status(unsafe { sys::pxui_ui_scroll_view(self.0.as_ptr(), as_str(label), size(width), size(height), Some(call::<F>), &mut build as *mut _ as *mut c_void, &mut out) })?;
        Ok(out)
    }

    pub fn view<F: FnMut(&mut Ui)>(&mut self, label: &str, revision: u64, width: Size, height: Size, mut build: F) -> Result<PxuiBox, i32> {
        let mut out = PxuiBox::default();
        status(unsafe { sys::pxui_ui_view(self.0.as_ptr(), as_str(label), revision, size(width), size(height), Some(call::<F>), &mut build as *mut _ as *mut c_void, &mut out) })?;
        Ok(out)
    }

    pub fn anchored<F: FnMut(&mut Ui)>(&mut self, label: &str, anchor: Rect, placement: Placement, width: Size, height: Size, mut build: F) -> Result<PxuiBox, i32> {
        let mut descriptor = encode_placement(placement);
        descriptor.rect = anchor;
        let mut out = PxuiBox::default();
        status(unsafe { sys::pxui_ui_anchored(self.0.as_ptr(), as_str(label), &descriptor, size(width), size(height), Some(call::<F>), &mut build as *mut _ as *mut c_void, &mut out) })?;
        Ok(out)
    }

    pub fn dialog<F: FnMut(&mut Ui)>(&mut self, title: &str, buttons: &[&str], mut body: F) -> Result<DialogOut, i32> {
        let (_strings, buttons) = strings(buttons);
        let mut out = PxuiDialogOut::default();
        status(unsafe { sys::pxui_ui_dialog(self.0.as_ptr(), as_str(title), buttons, Some(call::<F>), &mut body as *mut _ as *mut c_void, &mut out) })?;
        match out.tag { 0 => Ok(DialogOut::Open), 1 => Ok(DialogOut::Closed), 2 => Ok(DialogOut::Button(out.index)), _ => Err(-1) }
    }
}

fn as_str(value: &str) -> Str { Str { ptr: value.as_ptr(), len: value.len() } }
