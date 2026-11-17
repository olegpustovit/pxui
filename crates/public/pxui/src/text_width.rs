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
    /// The width in points of single-line kit text at `size` (`None`: the kit's font size), in the `Ui`'s font and theme, for layout during a build.
    /// Allocates: the first measurement of a text shapes and caches it; never for a cached one.
    /// Panics: never.
    ///
    /// ```
    /// let mut ui = pxui::doc::frames(1, &mut |ui| { ui.label("x", pxui::Label::DEFAULT); });
    /// assert!(ui.text_width(None, "Songs") < ui.text_width(None, "Songs and albums"));
    /// ```
    #[inline]
    pub fn text_width(&mut self,size:Option<i64>,text:&str)->f32 {
        unsafe{sys::pxui_ui_text_width(self.0.as_ptr(),size.is_some() as u8,size.unwrap_or_default(),as_str(text))}
    }
}
