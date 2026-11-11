use pxui_types::Str;
use crate::Ui;

#[cfg(feature="static")]
mod sys { pub use pxui_abi_link::pxui_ui_text_width; }
#[cfg(not(feature="static"))]
mod sys {
    use pxui_types::Str;
    use crate::sys::PxuiUi;
    #[link(name="pxui")]
    unsafe extern "C" { pub fn pxui_ui_text_width(ui:*mut PxuiUi,has_size:u8,size:i64,text:Str)->f32; }
}

#[inline]
fn as_str(value:&str)->Str { Str{ptr:value.as_ptr(),len:value.len()} }

impl Ui {
    /// Measures single-line kit text in logical points using the UI's font and theme.
    #[inline]
    pub fn text_width(&mut self,size:Option<i64>,text:&str)->f32 {
        unsafe{sys::pxui_ui_text_width(self.0.as_ptr(),size.is_some() as u8,size.unwrap_or_default(),as_str(text))}
    }
}
