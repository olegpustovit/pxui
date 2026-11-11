use crate::Str;
use crate::state::{PxuiUi, checked, get};

export!(pub unsafe fn pxui_ui_text_width(ui:*mut PxuiUi,has_size:u8,size:i64,text:Str)->f32 {
    let Some(ui)=(unsafe{get(ui)}) else{return 0.0};
    if has_size>1 { return 0.0; }
    let Some(text)=(unsafe{checked(text)}) else{return 0.0};
    unsafe{(&mut *ui.engine).text_width((has_size!=0).then_some(size),text)}
});
