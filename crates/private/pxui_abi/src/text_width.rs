use pxui_types::Str;
use crate::{get, PxuiUi};

const MAX_TEXT_BYTES: usize = 1 << 20;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_text_width(ui:*mut PxuiUi,has_size:u8,size:i64,text:Str)->f32 {
    let Some(ui)=(unsafe{get(ui)}) else{return 0.0};
    if has_size>1 || text.len>MAX_TEXT_BYTES { return 0.0; }
    let Some(text)=(unsafe{crate::text(text)}) else{return 0.0};
    unsafe{(&mut *ui.engine).text_width((has_size!=0).then_some(size),text)}
}
