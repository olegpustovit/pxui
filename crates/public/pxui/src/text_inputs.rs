use core::ffi::c_void;
use pxui_types::Str;
use crate::{composite::{defer_callback_panic, deferred}, status, Ui};

const MAX_TEXT_BYTES: usize = 1 << 20;

#[derive(Clone, Copy, Debug, Default)]
pub struct TextAreaOpts { pub wrap: bool }

#[derive(Clone, Copy, Debug, Default)]
pub struct ValueFieldOpts;

#[cfg(feature = "static")]
mod raw {
    pub use pxui_abi_link::{pxui_ui_text_area_submit, pxui_ui_value_field};
}

#[cfg(not(feature = "static"))]
mod raw {
    use core::ffi::c_void;
    use pxui_types::Str;
    use crate::sys::PxuiUi;
    pub type TextValid = Option<extern "C" fn(*mut c_void, Str) -> u8>;
    #[link(name = "pxui")]
    unsafe extern "C" {
    pub fn pxui_ui_text_area_submit(ui:*mut PxuiUi,at:*const f32,width:f32,height:f32,wrap:u8,label:Str,value:Str,out_text:*mut Str,out_changed:*mut u8,out_submitted:*mut u8)->i32;
    pub fn pxui_ui_value_field(ui:*mut PxuiUi,at:*const f32,width:f32,height:f32,label:Str,value:Str,valid:TextValid,user:*mut c_void,out_text:*mut Str,out_changed:*mut u8,out_editing:*mut u8)->i32;
    }
}

struct Validator<F>(F);

extern "C" fn validate<F: Fn(&str) -> bool>(user: *mut c_void, value: Str) -> u8 {
    if user.is_null() || deferred() || value.len > MAX_TEXT_BYTES || (value.len != 0 && value.ptr.is_null()) { return 0; }
    let bytes = if value.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(value.ptr, value.len) } };
    let Ok(value) = core::str::from_utf8(bytes) else { return 0; };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (unsafe { &*user.cast::<Validator<F>>() }).0(value))) {
        Ok(valid) => valid as u8,
        Err(panic) => { defer_callback_panic(panic); 0 }
    }
}

#[inline]
fn as_str(value: &str) -> Str { Str { ptr: value.as_ptr(), len: value.len() } }

fn returned(value: Str) -> Result<String, i32> {
    if value.len > MAX_TEXT_BYTES || (value.len != 0 && value.ptr.is_null()) { return Err(-1); }
    let bytes = if value.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(value.ptr, value.len) } };
    String::from_utf8(bytes.to_vec()).map_err(|_| -1)
}

impl Ui {
    #[inline]
    pub fn text_area_submit(&mut self, at: (f32, f32), width: f32, height: f32, options: &TextAreaOpts, label: &str, value: &str) -> Result<(String, bool), i32> {
        let at = [at.0, at.1];
        let (mut text, mut changed, mut submitted) = (Str::default(), 0, 0);
        status(unsafe { raw::pxui_ui_text_area_submit(self.0.as_ptr(), at.as_ptr(), width, height, options.wrap as u8, as_str(label), as_str(value), &mut text, &mut changed, &mut submitted) })?;
        Ok((returned(text)?, submitted != 0))
    }

    #[inline]
    pub fn value_field<F: Fn(&str) -> bool>(&mut self, at: (f32, f32), width: f32, height: f32, _options: &ValueFieldOpts, valid: F, label: &str, value: &str) -> Result<(String, bool), i32> {
        let at = [at.0, at.1];
        let mut validator = Validator(valid);
        let (mut text, mut _changed, mut editing) = (Str::default(), 0, 0);
        status(unsafe { raw::pxui_ui_value_field(self.0.as_ptr(), at.as_ptr(), width, height, as_str(label), as_str(value), Some(validate::<F>), (&mut validator as *mut Validator<F>).cast::<c_void>(), &mut text, &mut _changed, &mut editing) })?;
        let text = returned(text)?;
        Ok((text, editing != 0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validator_panics_are_deferred_across_the_c_callback() {
        let mut validator = Validator((|_: &str| -> bool { panic!("validator failed") }) as fn(&str) -> bool);
        let value = b"x";
        assert_eq!(validate::<fn(&str) -> bool>(&mut validator as *mut _ as *mut c_void, Str { ptr: value.as_ptr(), len: value.len() }), 0);
        assert!(crate::composite::take_callback_panic().is_some());
    }
}
