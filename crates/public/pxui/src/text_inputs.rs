//! Text widgets with options: the value field (slide, scrub and validity callbacks), the text area, and the text edit the `Ui` keeps.
use crate::composite::{defer_callback_panic, deferred};
use crate::{as_str, check, status, Ui};
use core::ffi::c_void;
use pxui_types::{ffi::PxuiValueFieldDesc, ui::TextEdit, PxuiBox, Str};
pub use pxui_types::ui::ValueFieldOpts;

const MAX_TEXT_BYTES: usize = 1 << 20;

#[derive(Clone, Copy, Debug, Default)]
pub struct TextAreaOpts { pub wrap: bool }

library!(lib {
    fn pxui_ui_text_area_submit(ui: *mut PxuiUi, at: *const f32, width: f32, height: f32, wrap: u8, label: Str, value: Str, out_text: *mut Str, out_changed: *mut u8, out_submitted: *mut u8) -> i32;
    fn pxui_ui_value_field(ui: *mut PxuiUi, desc: *const PxuiValueFieldDesc, valid: Option<extern "C" fn(*mut c_void, Str) -> u8>, slide: Option<extern "C" fn(*mut c_void, f32, *mut Str)>, scrub: Option<extern "C" fn(*mut c_void, Str, f32, u8, *mut Str)>, user: *mut c_void, out_text: *mut Str, out_changed: *mut u8, out_editing: *mut u8) -> i32;
    fn pxui_ui_load_text_edit(ui: *mut PxuiUi, key: i64, text: Str, out_text: *mut Str, caret: *mut u64, anchor: *mut u64) -> i32;
    fn pxui_ui_remember(ui: *mut PxuiUi, text: Str, caret: u64, anchor: u64, typing: u8) -> i32;
});

// What the library's callbacks reach during one `value_field` call: the app's closures and the text a callback produced
// (lent to the library until the next callback).
struct Field<'a> { valid: &'a dyn Fn(&str) -> bool, slide: Option<&'a dyn Fn(f32) -> String>, scrub: Option<&'a dyn Fn(&str, f32, bool) -> String>, produced: String }

fn text<'a>(value: Str) -> Option<&'a str> {
    if value.len > MAX_TEXT_BYTES || (value.len != 0 && value.ptr.is_null()) { return None; }
    core::str::from_utf8(if value.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(value.ptr, value.len) } }).ok()
}

/// Runs an app closure for a library callback: nothing once a panic is waiting, and a panic is kept to resume at the call.
fn guarded<T>(fallback: T, f: &mut dyn FnMut() -> T) -> T {
    if deferred() { return fallback; }
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) { Ok(value) => value, Err(panic) => { defer_callback_panic(panic); fallback } }
}

extern "C" fn valid_call(user: *mut c_void, value: Str) -> u8 {
    let field = unsafe { &*user.cast::<Field>() };
    let Some(value) = text(value) else { return 0 };
    guarded(0, &mut || (field.valid)(value) as u8)
}

extern "C" fn slide_call(user: *mut c_void, fraction: f32, out: *mut Str) {
    let field = unsafe { &mut *user.cast::<Field>() };
    let Some(slide) = field.slide else { return };
    field.produced = guarded(String::new(), &mut || slide(fraction));
    unsafe { *out = as_str(&field.produced); }
}

extern "C" fn scrub_call(user: *mut c_void, origin: Str, dx: f32, coarse: u8, out: *mut Str) {
    let field = unsafe { &mut *user.cast::<Field>() };
    let (Some(scrub), Some(origin)) = (field.scrub, text(origin)) else { return };
    field.produced = guarded(String::new(), &mut || scrub(origin, dx, coarse != 0));
    unsafe { *out = as_str(&field.produced); }
}

fn returned(value: Str) -> Result<String, i32> { text(value).map(str::to_owned).ok_or(-1) }

impl Ui {
    #[inline]
    pub fn text_area_submit(&mut self, at: (f32, f32), width: f32, height: f32, options: &TextAreaOpts, label: &str, value: &str) -> Result<(String, bool), i32> {
        let at = [at.0, at.1];
        let (mut out, mut changed, mut submitted) = (Str::default(), 0, 0);
        status(unsafe { lib::pxui_ui_text_area_submit(self.0.as_ptr(), at.as_ptr(), width, height, options.wrap as u8, as_str(label), as_str(value), &mut out, &mut changed, &mut submitted) })?;
        Ok((returned(out)?, submitted != 0))
    }

    /// A compact field: the new text and whether a text editor is open. Only text `valid` accepts commits.
    #[allow(clippy::too_many_arguments)]
    #[track_caller]
    pub fn value_field(&mut self, at: (f32, f32), width: f32, height: f32, o: &ValueFieldOpts, valid: &dyn Fn(&str) -> bool, label: &str, value: &str) -> (String, bool) {
        let has = |present: bool, flag: u32| if present { flag } else { 0 };
        let (lead, lead_color) = o.lead.as_ref().map_or(("", Default::default()), |(text, color)| (text.as_str(), *color));
        let (trail, trail_color) = o.trail.as_ref().map_or(("", Default::default()), |(text, color)| (text.as_str(), *color));
        let desc = PxuiValueFieldDesc {
            size: core::mem::size_of::<PxuiValueFieldDesc>() as u32,
            flags: has(o.left, PxuiValueFieldDesc::LEFT) | has(o.edit, PxuiValueFieldDesc::EDIT) | has(o.bare, PxuiValueFieldDesc::BARE) | has(o.kept_drag, PxuiValueFieldDesc::KEPT_DRAG)
                | has(o.size.is_some(), PxuiValueFieldDesc::HAS_SIZE) | has(o.fraction.is_some(), PxuiValueFieldDesc::HAS_FRACTION) | has(o.tracking.is_some(), PxuiValueFieldDesc::HAS_TRACKING)
                | has(o.line.is_some(), PxuiValueFieldDesc::HAS_LINE) | has(o.lead.is_some(), PxuiValueFieldDesc::HAS_LEAD) | has(o.trail.is_some(), PxuiValueFieldDesc::HAS_TRAIL)
                | has(o.display.is_some(), PxuiValueFieldDesc::HAS_DISPLAY) | has(o.placeholder.is_some(), PxuiValueFieldDesc::HAS_PLACEHOLDER),
            font_size: o.size.unwrap_or_default(), fraction: o.fraction.unwrap_or_default(), tracking: o.tracking.unwrap_or_default(),
            line: o.line.unwrap_or_default(), lead_color, trail_color, reserved: [0; 4], x: at.0, y: at.1, width, height,
            display: as_str(o.display.as_deref().unwrap_or("")), placeholder: as_str(o.placeholder.as_deref().unwrap_or("")),
            lead: as_str(lead), trail: as_str(trail), label: as_str(label), value: as_str(value),
        };
        let slide = o.slide.as_deref();
        let scrub = o.scrub.as_deref();
        let mut field = Field { valid, slide, scrub, produced: String::new() };
        let (mut out, mut changed, mut editing) = (Str::default(), 0, 0);
        check(self, unsafe { lib::pxui_ui_value_field(self.0.as_ptr(), &desc, Some(valid_call), slide.map(|_| slide_call as extern "C" fn(*mut c_void, f32, *mut Str)), scrub.map(|_| scrub_call as extern "C" fn(*mut c_void, Str, f32, u8, *mut Str)), &mut field as *mut Field as *mut c_void, &mut out, &mut changed, &mut editing) });
        (returned(out).unwrap_or_default(), editing != 0)
    }

    /// A value field's value and soft-range line written by patches, as its build lays them out without a lead: false when a
    /// patch cannot show it (a lead, a value too wide beside its trail, the line had no width when built, a stale handle).
    #[track_caller]
    pub fn patch_field(&mut self, field: &PxuiBox, o: &ValueFieldOpts, value_text: &str) -> bool {
        let (Some(value), None) = (self.box_of_key(pxui_types::ui::int_key(field.box_key, 1)), &o.lead) else { return false };
        let at = self.local_rect(field);
        let mut ok = true;
        if let Some(f) = o.fraction {
            let width = (at.w * 0.0f32.max(1.0f32.min(f))).max(0.0);
            match self.box_of_key(pxui_types::ui::int_key(field.box_key, 0)) {
                Some(line) if self.local_rect(&line).w > 0.0 || width == 0.0 => {
                    let r = self.local_rect(&line);
                    if r.w != width { ok &= self.patch_rect(&line, pxui_types::geom::Rect::new(r.x, r.y, width, r.h)); }
                }
                _ => return false,
            }
        }
        // a trail (a unit) stays put: the value sits against it, as long as it fits unshortened
        let trail = match &o.trail { Some((t, _)) => self.text_width(o.size, t) + 6.0, None => 0.0 };
        let width = self.text_width(o.size, value_text);
        if trail > 0.0 && width > at.w - 4.0 - trail { return false; }
        let r = self.local_rect(&value);
        let x = at.x + if o.left { 2.0 } else { at.w - 2.0 - trail - width };
        if (r.x, r.w) != (x, width.max(1.0)) { ok &= self.patch_rect(&value, pxui_types::geom::Rect::new(x, r.y, width.max(1.0), r.h)); }
        ok && self.patch_text(&value, value_text)
    }

    /// The edit the `Ui` keeps for the field under `key`, loaded with `text`.
    #[track_caller]
    pub fn load_text_edit(&mut self, key: i64, text: &str) -> TextEdit {
        let (mut out, mut caret, mut anchor) = (Str::default(), 0, 0);
        check(self, unsafe { lib::pxui_ui_load_text_edit(self.0.as_ptr(), key, as_str(text), &mut out, &mut caret, &mut anchor) });
        TextEdit { text: returned(out).unwrap_or_default(), caret: caret as usize, anchor: anchor as usize }
    }

    /// Snapshots `edit` for undo; `typing` joins consecutive typing into one step.
    #[track_caller]
    pub fn remember(&mut self, edit: &TextEdit, typing: bool) {
        check(self, unsafe { lib::pxui_ui_remember(self.0.as_ptr(), as_str(&edit.text), edit.caret as u64, edit.anchor as u64, typing as u8) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validator_panics_are_deferred_across_the_c_callback() {
        let valid = |_: &str| -> bool { panic!("validator failed") };
        let mut field = Field { valid: &valid, slide: None, scrub: None, produced: String::new() };
        let value = b"x";
        assert_eq!(valid_call(&mut field as *mut Field as *mut c_void, Str { ptr: value.as_ptr(), len: value.len() }), 0);
        assert!(crate::composite::take_callback_panic().is_some());
    }
}
