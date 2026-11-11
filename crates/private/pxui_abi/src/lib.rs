#![allow(clippy::all)]

mod text;
mod node;
mod paint;
mod widgets;
mod composite;
mod theme;
mod overlays;
mod collections;
mod image;
mod text_inputs;
mod kit;
mod a11y;
mod a11y_state;
mod style;
mod text_width;
mod interaction;
pub use text::*;
pub use node::*;
pub use paint::*;
pub use widgets::*;
pub use composite::*;
pub use theme::*;
pub use overlays::*;
pub use collections::*;
pub use image::*;
pub use text_inputs::*;
pub use kit::*;
pub use a11y::*;
pub use a11y_state::*;
pub use style::*;
pub use text_width::*;
pub use interaction::*;

use core::ffi::c_void;
use std::ffi::CString;
use input::{frame, Event, Key, MouseButton, Platform, ScrollPhase};
use pxui_core::ui::{PanelOpts, Ui};
pub use pxui_types::{FrameIn, FrameOut, PxuiBox, PxuiPanel, PxuiTargetDesc, PxuiUiDesc, Str};
pub use pxui_types::ffi::{PxuiDialogOut, PxuiThemeSnapshot};
// the renderer's status codes (pxui.h): bad argument, GPU failure, call not allowed now
const PXUI_ERR_ARGUMENT:i32=-1; const PXUI_ERR_GPU:i32=-2; const PXUI_ERR_STATE:i32=-3;
use pxui_types::ffi::{A11yNode as FfiA11yNode, A11yTree as FfiA11yTree, A11yValue as FfiA11yValue, Damage as FfiDamage, Output as FfiOutput, PlatformOut as FfiPlatformOut, Slice as FfiSlice};
use ogpu_core::backend::{self as gpu_backend, Device, Queue, Texture};
use ogpu_core::pipeline::{ColorFormat, DepthFormat};
use ogpu_core::types::{Extent, Origin, TextureFormat};
use pxui_render::{AtlasKind, Break, Painter, View};
use pxui_core::text::AtlasKind as TextKind;

pub const ABI_VERSION: u32 = 1 << 16;

macro_rules! export {
    ($vis:vis fn $name:ident($($arg:ident: $ty:ty),* $(,)?) $(-> $ret:ty)? $body:block) => {
        #[unsafe(no_mangle)]
        $vis extern "C" fn $name($($arg: $ty),*) $(-> $ret)? $body
    };
    ($vis:vis unsafe fn $name:ident($($arg:ident: $ty:ty),* $(,)?) $(-> $ret:ty)? $body:block) => {
        #[unsafe(no_mangle)]
        $vis unsafe extern "C" fn $name($($arg: $ty),*) $(-> $ret)? $body
    };
}

#[repr(C)]
pub struct PxuiUi {
    engine: *mut Ui,
    error: *mut CString,
    returned: *mut Vec<CString>,
    a11y_nodes: *mut Vec<FfiA11yNode>,
    grid_tracks: *mut std::collections::HashMap<i64, Vec<Box<[pxui_types::layout::Track]>>>,
    grid_updated: *mut std::collections::HashMap<i64, bool>,
    owner: bool,
}

unsafe fn text(value: Str) -> Option<&'static str> {
    if value.len == 0 { return Some(""); }
    if value.ptr.is_null() { return None; }
    let bytes = unsafe { std::slice::from_raw_parts(value.ptr, value.len) };
    std::str::from_utf8(bytes).ok().map(|s| unsafe { &*(s as *const str) })
}

unsafe fn get<'a>(ui: *mut PxuiUi) -> Option<&'a mut PxuiUi> {
    if ui.is_null() { None } else { Some(unsafe { &mut *ui }) }
}
unsafe fn fail(ui:*mut PxuiUi, message:&str) -> i32 {
    if let Some(ui)=unsafe { get(ui) } {
        if let Ok(error)=CString::new(message) { unsafe { *ui.error=error; } }
    }
    -1
}

fn platform() -> Platform {
    #[cfg(target_os = "macos")] { Platform::Apple }
    #[cfg(not(target_os = "macos"))] { Platform::Other }
}

fn key(code: u32) -> Option<Key> {
    Some(match code {
        1..=127 => Key::KeyChar(char::from_u32(code)?),
        0x100 => Key::ArrowUp, 0x101 => Key::ArrowDown, 0x102 => Key::ArrowLeft, 0x103 => Key::ArrowRight,
        0x104 => Key::Space, 0x105 => Key::Enter, 0x106 => Key::Escape, 0x107 => Key::Backspace,
        0x108 => Key::Tab, 0x109 => Key::Shift, 0x10a => Key::Ctrl, 0x10b => Key::Alt, 0x10c => Key::Meta,
        0x120..=0x12b => [Key::F1,Key::F2,Key::F3,Key::F4,Key::F5,Key::F6,Key::F7,Key::F8,Key::F9,Key::F10,Key::F11,Key::F12][(code-0x120) as usize],
        0x130 => Key::Home, 0x131 => Key::End, 0x132 => Key::PageUp, 0x133 => Key::PageDown, 0x134 => Key::Insert, 0x135 => Key::Delete,
        _ => Key::Unknown(code as i64),
    })
}

fn button(code: u32) -> Option<MouseButton> {
    Some(match code { 0=>MouseButton::LeftButton,1=>MouseButton::RightButton,2=>MouseButton::MiddleButton,3=>MouseButton::MouseX1,4=>MouseButton::MouseX2,_=>return None })
}

pub type Build = Option<extern "C" fn(*mut c_void, *mut PxuiUi)>;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_view_size(ui: *mut PxuiUi, out: *mut pxui_types::geom::Point) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return -1; };
    if out.is_null() { return unsafe { fail(ui, "null view size output") }; }
    let (x, y) = unsafe { (&*state.engine).view_size() };
    unsafe { *out = pxui_types::geom::Point { x, y }; }
    0
}

export!(pub fn pxui_abi_version() -> u32 { ABI_VERSION });

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_create(desc:*const PxuiUiDesc) -> *mut PxuiUi {
    if desc.is_null() { return core::ptr::null_mut(); }
    let desc=unsafe { *desc };
    if desc.size < core::mem::size_of::<PxuiUiDesc>() as u32 || desc.font_size < 0 || desc.platform > 2 { return core::ptr::null_mut(); }
    unsafe { create_ui(desc, &[pxui_core::text::PRAGMASEVKA]) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_create_with_fonts(desc:*const PxuiUiDesc, fonts:*const Str, count:usize) -> *mut PxuiUi {
    if desc.is_null() || count == 0 || count > 128 || fonts.is_null() { return core::ptr::null_mut(); }
    let desc=unsafe { *desc };
    if desc.size < core::mem::size_of::<PxuiUiDesc>() as u32 || desc.font_size < 0 || desc.platform > 2 { return core::ptr::null_mut(); }
    let mut bytes=Vec::with_capacity(count);
    for font in unsafe { std::slice::from_raw_parts(fonts,count) } {
        if font.len == 0 || font.len > 16*1024*1024 || font.ptr.is_null() { return core::ptr::null_mut(); }
        bytes.push(unsafe { std::slice::from_raw_parts(font.ptr,font.len) });
    }
    unsafe { create_ui(desc,&bytes) }
}

unsafe fn create_ui(desc:PxuiUiDesc, fonts:&[&[u8]]) -> *mut PxuiUi {
    let text = pxui_core::text::Text::new(fonts);
    let platform=match desc.platform { 0=>platform(), 1=>Platform::Apple, _=>Platform::Other };
    let font_size=(desc.font_size != 0).then_some(i64::from(desc.font_size));
    let engine = Box::into_raw(Box::new(Ui::create(platform, font_size, text)));
    let error = Box::into_raw(Box::new(CString::default()));
    let returned = Box::into_raw(Box::new(Vec::new()));
    let a11y_nodes=Box::into_raw(Box::new(Vec::new()));
    let grid_tracks=Box::into_raw(Box::new(std::collections::HashMap::new()));
    let grid_updated=Box::into_raw(Box::new(std::collections::HashMap::new()));
    Box::into_raw(Box::new(PxuiUi { engine, error, returned, a11y_nodes, grid_tracks, grid_updated, owner: true }))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_destroy(ui: *mut PxuiUi) {
    if ui.is_null() { return; }
    if !unsafe { (*ui).owner } { return; }
    let ui = unsafe { Box::from_raw(ui) };
    drop(unsafe { Box::from_raw(ui.engine) });
    drop(unsafe { Box::from_raw(ui.error) });
    drop(unsafe { Box::from_raw(ui.returned) });
    drop(unsafe { Box::from_raw(ui.a11y_nodes) });
    drop(unsafe { Box::from_raw(ui.grid_tracks) });
    drop(unsafe { Box::from_raw(ui.grid_updated) });
}

#[cfg(test)]
mod borrowed_ui_tests {
    use super::*;

    #[test]
    fn destroying_a_borrowed_callback_ui_is_a_noop() {
        let mut borrowed = PxuiUi {
            engine: core::ptr::null_mut(), error: core::ptr::null_mut(), returned: core::ptr::null_mut(),
            a11y_nodes: core::ptr::null_mut(), grid_tracks: core::ptr::null_mut(), grid_updated: core::ptr::null_mut(), owner: false,
        };
        unsafe { pxui_ui_destroy(&mut borrowed); }
        assert!(!borrowed.owner);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_last_error(ui: *const PxuiUi) -> Str {
    if ui.is_null() { return Str::default(); }
    let bytes = unsafe { &*(*ui).error }.as_bytes();
    Str { ptr: bytes.as_ptr(), len: bytes.len() }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_output(ui:*mut PxuiUi) -> FfiOutput {
    let Some(state)=(unsafe { get(ui) }) else { return unsafe { core::mem::zeroed() }; };
    let engine=unsafe { &mut *state.engine };
    let output=engine.output();
    let platform=output.platform;
    let a11y_nodes=unsafe { &mut *state.a11y_nodes };
    a11y_nodes.clear();
    let mut last_children = vec![u32::MAX; output.a11y.nodes.len()];
    for (index, node) in output.a11y.nodes.iter().enumerate() {
        if node.parent != u32::MAX {
            last_children[node.parent as usize] = index as u32;
        }
    }
    for (index, n) in output.a11y.nodes.iter().enumerate() {
        let value=match n.value {
            pxui_core::a11y::Value::None=>FfiA11yValue::default(),
            pxui_core::a11y::Value::Text(r)=>FfiA11yValue { tag:1,text_start:r.start,text_end:r.end,..FfiA11yValue::default() },
            pxui_core::a11y::Value::Number{now,min,max,step}=>FfiA11yValue { tag:2,now,min,max,step,..FfiA11yValue::default() },
        };
        a11y_nodes.push(FfiA11yNode { id:n.id,parent:n.parent,role:n.role as u32,name_start:n.name.start,name_end:n.name.end,description_start:n.description.start,description_end:n.description.end,value,rect:n.rect,states:n.states,actions:n.actions,selection:[n.selection.0,n.selection.1],child_count:n.child_count,first_child:n.first_child,next_sibling:n.next_sibling,last_child:last_children[index] });
    }
    let (cursor,has_cursor)=match platform.cursor { Some(pxui_core::ui::Cursor::HorizontalResize)=>(1,true),Some(pxui_core::ui::Cursor::VerticalResize)=>(2,true),Some(pxui_core::ui::Cursor::Text)=>(3,true),None=>(0,false) };
    let (ime_rect,ime_caret,has_ime)=platform.ime.map_or(([0;4],0,false),|(r,c)|([r.0,r.1,r.2,r.3],c,true));
    let (background,has_background)=platform.background.map_or((pxui_types::geom::Rgba::default(),false),|c|(c,true));
    let (clipboard,has_clipboard)=platform.clipboard.as_ref().map_or((Str::default(),false),|s| {
        unsafe { (&mut *state.returned).push(CString::new(s.as_str()).unwrap_or_default()); }
        let c=unsafe { (&*state.returned).last().unwrap() };
        (Str { ptr:c.as_ptr().cast(),len:c.as_bytes().len() },true)
    });
    let tree=&output.a11y;
    FfiOutput {
        size:core::mem::size_of::<FfiOutput>() as u32,reserved:0,draw_revision:engine.draw_revision() as u64,
        repaint_at:engine.repaint_at().unwrap_or(f64::INFINITY),built_nodes:engine.built_nodes() as u64,
        rebuilt_views:engine.rebuilt_views() as u64,arranged_nodes:engine.arranged_nodes() as u64,
        platform:FfiPlatformOut { size:core::mem::size_of::<FfiPlatformOut>() as u32,cursor,has_cursor:has_cursor as u8,has_ime:has_ime as u8,has_background:has_background as u8,has_clipboard:has_clipboard as u8,ime_rect,ime_caret,clipboard,background,paste:platform.paste as u8,reserved:[0;7] },
        damage:output.damage.map_or_else(FfiDamage::default,|r|FfiDamage { has_damage:1,reserved:[0;3],rect:r }),
        a11y:FfiA11yTree { nodes:FfiSlice { ptr:a11y_nodes.as_ptr(),len:a11y_nodes.len() },strings:Str { ptr:tree.strings.as_ptr(),len:tree.strings.len() },revision:tree.revision,first_child:tree.first_child(u32::MAX),child_count:tree.child_count(u32::MAX),actions:FfiSlice { ptr:core::ptr::null(),len:0 } },
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_tokens(ui:*mut PxuiUi) -> pxui_types::ffi::ThemeTokens {
    let Some(ui)=(unsafe{get(ui)}) else { return unsafe{core::mem::zeroed()}; };
    let t=unsafe{(&*ui.engine).tokens()};
    pxui_types::ffi::ThemeTokens {
        panel:t.panel, foreground:t.foreground, control:t.control, input:t.input, track:t.track, accent:t.accent,
        ink_ground:t.ink_ground.unwrap_or_default(), has_ink_ground:t.ink_ground.is_some() as u8, reserved:[0;3],
        space:t.space, radius:t.radius, display_size:t.display_size, title_size:t.title_size,
        font_size:t.font_size, label_size:t.label_size,
        motion:pxui_types::ffi::PxuiMotion { fast:t.motion.fast, normal:t.motion.normal, slow:t.motion.slow, ease:t.motion.ease },
        reduced_motion:t.reduced_motion as u8, reserved_tail:[0;3], density:t.density,
    }
}


#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_set_theme(ui:*mut PxuiUi,source:*const pxui_types::ffi::ThemeSource)->i32 {
    let Some(ui)=(unsafe { get(ui) }) else {return -1};
    if source.is_null(){return unsafe {fail(ui as *mut _,"null theme source")};}
    let source=unsafe{*source};
    if source.size<core::mem::size_of::<pxui_types::ffi::ThemeSource>() as u32 || source.flags & !3 != 0 {return unsafe{fail(ui as *mut _,"invalid theme descriptor")};}
    let parsed=if source.source.len==0 {Ok(pxui_core::Theme::default())} else {
        let Some(text)=(unsafe{text(source.source)}) else{return unsafe{fail(ui as *mut _,"invalid theme UTF-8")};};
        pxui_core::Theme::parse(text,&pxui_core::Theme::default()).map_err(|_|())
    };
    let Ok(mut theme)=parsed else{return unsafe{fail(ui as *mut _,"theme parse failed")};};
    theme.tokens.density=if source.flags & pxui_types::ffi::ThemeSource::COMPACT !=0 {0.84} else {1.0};
    theme.tokens.reduced_motion=source.flags & pxui_types::ffi::ThemeSource::REDUCED_MOTION !=0;
    unsafe{(&mut *ui.engine).set_theme(theme);}
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_set_dir(ui:*mut PxuiUi,dir:u32)->i32 {
    let Some(ui)=(unsafe{get(ui)}) else{return -1};
    let dir=match dir {0=>pxui_core::layout::Direction::Ltr,1=>pxui_core::layout::Direction::Rtl,_=>return unsafe{fail(ui as *mut _,"invalid direction")}};
    unsafe{(&mut *ui.engine).set_dir(dir);}
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_set_a11y(ui:*mut PxuiUi,on:u8)->i32 {
    let Some(ui)=(unsafe{get(ui)}) else{return -1};
    if on>1{return unsafe{fail(ui as *mut _,"invalid accessibility flag")};}
    unsafe{(&mut *ui.engine).set_a11y(on!=0);}
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_inject_a11y(ui:*mut PxuiUi,actions:*const pxui_types::ffi::A11yAction,count:usize)->i32 {
    let Some(ui)=(unsafe{get(ui)}) else{return -1};
    if count>4096||(count!=0&&actions.is_null()){return unsafe{fail(ui as *mut _,"invalid accessibility action span")};}
    let mut translated=Vec::with_capacity(count);
    if count!=0 { for a in unsafe{std::slice::from_raw_parts(actions,count)} {
        let action=match a.action_code {1=>pxui_core::a11y::Action::Press,2=>pxui_core::a11y::Action::Focus,4=>pxui_core::a11y::Action::SetValue,8=>pxui_core::a11y::Action::Scroll,16=>pxui_core::a11y::Action::Expand,32=>pxui_core::a11y::Action::Collapse,64=>pxui_core::a11y::Action::Select,_=>return unsafe{fail(ui as *mut _,"invalid accessibility action code")}};
        translated.push(pxui_core::a11y::A11yAction{id:a.id,action,payload:a.payload,end:a.end as f64});
    }}
    unsafe{(&mut *ui.engine).inject_a11y(&translated);}
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_patch_text(ui:*mut PxuiUi,box_:*const PxuiBox,value:Str)->u8 {
    let Some(ui)=(unsafe{get(ui)}) else{return 0};
    if box_.is_null(){return 0;}
    let Some(value)=(unsafe{text(value)}) else{return 0};
    let raw=unsafe{*box_};
    let handle=pxui_core::ui::UiBox{index:raw.index as usize,box_key:raw.key,box_slot:raw.slot as usize,generation:raw.generation};
    unsafe{(&mut *ui.engine).patch_text(&handle,value) as u8}
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_busy_step(ui:*mut PxuiUi,box_:*const PxuiBox,now:f64)->u8 {
    let Some(ui)=(unsafe{get(ui)}) else{return 0};
    if box_.is_null()||!now.is_finite(){return 0;}
    let raw=unsafe{*box_};
    let handle=pxui_core::ui::UiBox{index:raw.index as usize,box_key:raw.key,box_slot:raw.slot as usize,generation:raw.generation};
    unsafe{(&mut *ui.engine).busy_step(&handle,now) as u8}
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_request_frame(ui:*mut PxuiUi)->i32 {
    let Some(ui)=(unsafe{get(ui)}) else{return -1};
    unsafe{(&mut *ui.engine).request_frame();}
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_frame(ui: *mut PxuiUi, input: *const FrameIn, build: Build, user: *mut c_void, out: *mut FrameOut) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return -1 };
    if input.is_null() || out.is_null() { return unsafe { fail(ui,"null frame argument") }; }
    let input = unsafe { &*input };
    if input.size < core::mem::size_of::<FrameIn>() as u32 || !input.time.is_finite() || !input.scale.is_finite() || input.scale <= 0.0 || !input.width.is_finite() || !input.height.is_finite() { return unsafe { fail(ui,"invalid frame descriptor") }; }
    if input.event_count > 4096 || (input.event_count != 0 && input.events.is_null()) { return unsafe { fail(ui,"invalid frame event slice") }; }
    unsafe { *state.error=CString::default(); }
    unsafe { (&mut *state.returned).clear(); }
    let mut base = input::at_time(input.time);
    base.size = (input.width as i64, input.height as i64);
    base.frame = input.time.to_bits() as i64;
    if input.event_count != 0 {
        for e in unsafe { std::slice::from_raw_parts(input.events, input.event_count) } {
            let s = unsafe { text(e.text) };
            let translated = match e.kind {
                1 => key(e.code).map(Event::KeyPressed),
                2 => key(e.code).map(Event::KeyReleased),
                3 => Some(Event::PointerMoved((e.x,e.y))),
                4 => button(e.code).map(|b| Event::PointerPressed(b,(e.x,e.y))),
                5 => button(e.code).map(|b| Event::PointerReleased(b,(e.x,e.y))),
                6 => button(e.code).map(Event::PointerCancelled),
                7 => Some(Event::Scrolled((e.x,e.y))),
                8 => s.map(|s| Event::TextInput(s.to_owned())),
                9 => s.map(|s| Event::Paste(s.to_owned())),
                10 => s.map(|s| Event::TextEditing { text:s.to_owned(), start:e.x as i64, length:e.y as i64 }),
                11 => s.map(|s| Event::FileDropped(s.to_owned())),
                12 => Some(Event::FileDragMoved((e.x,e.y))),
                13 => Some(Event::FileDragEnded),
                14 => Some(Event::Pinched(e.value)),
                15 => s.and_then(|s| match e.code {
                    0 => Some(Event::FileDialog { id:e.value as i64, result:Ok(s.split('\0').map(str::to_owned).collect()) }),
                    1 => Some(Event::FileDialog { id:e.value as i64, result:Err(s.to_owned()) }),
                    _ => None,
                }),
                16 => Some(Event::DialogOpened(e.code as i64)),
                17 => Some(Event::Resized((e.x as i64,e.y as i64))),
                18 => Some(Event::FocusLost),
                19 => Some(Event::FocusGained),
                20 => Some(Event::Closed),
                21 => Some(Event::PixelSizeChanged((e.x as i64,e.y as i64))),
                22 => Some(Event::VisibilityChanged(e.value != 0.0)),
                23 => Some(Event::TrackpadScrolled { delta:(e.x,e.y), phase:match e.code {0=>ScrollPhase::Touched,1=>ScrollPhase::Moved,2=>ScrollPhase::Lifted,3=>ScrollPhase::Momentum,_=>return unsafe { fail(ui,"unknown scroll phase") }}, time:e.value }),
                _ => None,
            };
            let Some(event) = translated else { return unsafe { fail(ui,"invalid event kind, code, or UTF-8 payload") }; };
            match &event {
                Event::KeyPressed(k) => { if !base.keys.contains(k) { base.keys.push(*k); } }
                Event::KeyReleased(k) => base.keys.retain(|held| held != k),
                Event::PointerMoved(p) => base.pointer = *p,
                Event::PointerPressed(b, p) => { base.pointer = *p; if !base.buttons.contains(b) { base.buttons.push(*b); } }
                Event::PointerReleased(b, p) => { base.pointer = *p; base.buttons.retain(|held| held != b); }
                Event::PointerCancelled(b) => base.buttons.retain(|held| held != b),
                _ => {}
            }
            base.events.push(event);
        }
    }
    let frame = frame::Frame { input: base, pixel_scale: (input.scale as f64, input.scale as f64), fps: 60.0, mouse_delta: (0.0, 0.0) };
    let (error, returned, a11y_nodes, grid_tracks, grid_updated) = (state.error, state.returned, state.a11y_nodes, state.grid_tracks, state.grid_updated);
    unsafe { (&mut *grid_updated).clear(); }
    let mut callback = |engine: &mut Ui| if let Some(build) = build {
        let mut borrowed = PxuiUi { engine: engine as *mut Ui, error, returned, a11y_nodes, grid_tracks, grid_updated, owner: false };
        build(user, &mut borrowed);
    };
    let output = unsafe { (&mut *state.engine).frame(&frame, &mut callback) };
    unsafe { node::finish_frame(ui); }
    let damage = output.damage.map_or([0.0; 4], |r| [r.x, r.y, r.w, r.h]);
    unsafe { *out = FrameOut { size: core::mem::size_of::<FrameOut>() as u32, damage, has_damage: output.damage.is_some() as u8, reserved: [0; 3], layer_count: output.draw.len() as u32, layers: output.draw.as_ptr().cast() }; }
    0
}

fn translate_event(e:pxui_types::Event)->Option<Event> {
    let s=unsafe { text(e.text) };
    Some(match e.kind {
        pxui_types::Event::KEY_PRESSED=>Event::KeyPressed(key(e.code)?),
        pxui_types::Event::KEY_RELEASED=>Event::KeyReleased(key(e.code)?),
        pxui_types::Event::POINTER_MOVED=>Event::PointerMoved((e.x,e.y)),
        pxui_types::Event::POINTER_PRESSED=>Event::PointerPressed(button(e.code)?,(e.x,e.y)),
        pxui_types::Event::POINTER_RELEASED=>Event::PointerReleased(button(e.code)?,(e.x,e.y)),
        pxui_types::Event::POINTER_CANCELLED=>Event::PointerCancelled(button(e.code)?),
        pxui_types::Event::SCROLLED=>Event::Scrolled((e.x,e.y)),
        pxui_types::Event::TEXT_INPUT=>Event::TextInput(s?.to_owned()),
        pxui_types::Event::PASTE=>Event::Paste(s?.to_owned()),
        pxui_types::Event::TEXT_EDITING=>Event::TextEditing { text:s?.to_owned(),start:e.x as i64,length:e.y as i64 },
        pxui_types::Event::FILE_DROPPED=>Event::FileDropped(s?.to_owned()),
        pxui_types::Event::FILE_DRAG_MOVED=>Event::FileDragMoved((e.x,e.y)),
        pxui_types::Event::FILE_DRAG_ENDED=>Event::FileDragEnded,
        pxui_types::Event::PINCHED=>Event::Pinched(e.value),
        pxui_types::Event::FILE_DIALOG=>match e.code { 0=>Event::FileDialog { id:e.value as i64,result:Ok(s?.split('\0').map(str::to_owned).collect()) },1=>Event::FileDialog { id:e.value as i64,result:Err(s?.to_owned()) },_=>return None },
        pxui_types::Event::DIALOG_OPENED=>Event::DialogOpened(e.code as i64),
        pxui_types::Event::RESIZED=>Event::Resized((e.x as i64,e.y as i64)),
        pxui_types::Event::FOCUS_LOST=>Event::FocusLost,
        pxui_types::Event::FOCUS_GAINED=>Event::FocusGained,
        pxui_types::Event::CLOSED=>Event::Closed,
        pxui_types::Event::PIXEL_SIZE_CHANGED=>Event::PixelSizeChanged((e.x as i64,e.y as i64)),
        pxui_types::Event::VISIBILITY_CHANGED=>Event::VisibilityChanged(e.value!=0.0),
        pxui_types::Event::TRACKPAD_SCROLLED=>Event::TrackpadScrolled { delta:(e.x,e.y),phase:match e.code {0=>ScrollPhase::Touched,1=>ScrollPhase::Moved,2=>ScrollPhase::Lifted,3=>ScrollPhase::Momentum,_=>return None},time:e.value },
        _=>return None,
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_frame(ui:*mut PxuiUi,input:*const pxui_types::ffi::Input,build:Build,user:*mut c_void,out:*mut FrameOut)->i32 {
    let Some(state)=(unsafe { get(ui) }) else { return -1; };
    if input.is_null() || out.is_null() { return unsafe { fail(ui,"null frame argument") }; }
    let input=unsafe { *input };
    if input.size < core::mem::size_of::<pxui_types::ffi::Input>() as u32 || !input.time.is_finite() || !input.dt.is_finite() || input.dt<0.0 || !input.fps.is_finite() || input.fps<0.0 || !input.pointer.iter().all(|v|v.is_finite()) || !input.pixel_scale.iter().all(|v|v.is_finite()&&*v>0.0) || !input.mouse_delta.iter().all(|v|v.is_finite()) { return unsafe { fail(ui,"invalid input frame") }; }
    if input.events.len>4096 || input.keys.len>512 || input.buttons.len>32 || (input.events.len!=0&&input.events.ptr.is_null()) || (input.keys.len!=0&&input.keys.ptr.is_null()) || (input.buttons.len!=0&&input.buttons.ptr.is_null()) { return unsafe { fail(ui,"invalid input spans") }; }
    let mut base=input::at_time(input.time);
    base.dt=input.dt;base.frame=input.frame;base.tick=input.tick;base.size=(input.logical_size[0],input.logical_size[1]);base.pointer=(input.pointer[0],input.pointer[1]);
    let keys=if input.keys.len==0 { &[] } else { unsafe { std::slice::from_raw_parts(input.keys.ptr,input.keys.len) } };
    let buttons=if input.buttons.len==0 { &[] } else { unsafe { std::slice::from_raw_parts(input.buttons.ptr,input.buttons.len) } };
    let events=if input.events.len==0 { &[] } else { unsafe { std::slice::from_raw_parts(input.events.ptr,input.events.len) } };
    for code in keys { base.keys.push(key(*code).unwrap_or(Key::Unknown(*code as i64))); }
    for code in buttons { let Some(value)=button(*code) else { return unsafe { fail(ui,"invalid held mouse button") }; }; base.buttons.push(value); }
    for raw in events {
        let Some(event)=translate_event(*raw) else { return unsafe { fail(ui,"invalid event kind, code, or UTF-8 payload") }; };
        match &event {
            Event::KeyPressed(k)=>if !base.keys.contains(k){base.keys.push(*k)},
            Event::KeyReleased(k)=>base.keys.retain(|held|held!=k),
            Event::PointerMoved(p)=>base.pointer=*p,
            Event::PointerPressed(b,p)=>{base.pointer=*p;if !base.buttons.contains(b){base.buttons.push(*b)}},
            Event::PointerReleased(b,p)=>{base.pointer=*p;base.buttons.retain(|held|held!=b)},
            Event::PointerCancelled(b)=>base.buttons.retain(|held|held!=b),
            _=>{}
        }
        base.events.push(event);
    }
    unsafe { *state.error=CString::default();(&mut *state.returned).clear(); }
    let engine_frame=frame::Frame { input:base,pixel_scale:(input.pixel_scale[0],input.pixel_scale[1]),fps:input.fps,mouse_delta:(input.mouse_delta[0],input.mouse_delta[1]) };
    unsafe { (&mut *state.grid_updated).clear(); }
    let mut callback=|engine:&mut Ui| if let Some(build)=build {let mut borrowed=PxuiUi {engine:engine as *mut Ui,error:state.error,returned:state.returned,a11y_nodes:state.a11y_nodes,grid_tracks:state.grid_tracks,grid_updated:state.grid_updated,owner:false};build(user,&mut borrowed);};
    let output=unsafe { (&mut *state.engine).frame(&engine_frame,&mut callback) };
    unsafe { node::finish_frame(ui); }
    let damage=output.damage.map_or([0.0;4],|r|[r.x,r.y,r.w,r.h]);
    unsafe { *out=FrameOut {size:core::mem::size_of::<FrameOut>() as u32,damage,has_damage:output.damage.is_some() as u8,reserved:[0;3],layer_count:output.draw.len() as u32,layers:output.draw.as_ptr().cast()}; }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_within(ui: *mut PxuiUi, box_: *const PxuiBox, build: Build, user: *mut c_void) {
    let (Some(ui), false) = (unsafe { get(ui) }, box_.is_null()) else { return; };
    let raw = unsafe { *box_ };
    let b = pxui_core::ui::UiBox { index: raw.index as usize, box_key: raw.key, box_slot: raw.slot as usize, generation: raw.generation };
    let mut callback = |engine: &mut Ui| if let Some(build) = build { let mut borrowed=PxuiUi { engine: engine as *mut Ui, error:ui.error, returned:ui.returned, a11y_nodes:ui.a11y_nodes, grid_tracks:ui.grid_tracks, grid_updated:ui.grid_updated, owner:false }; build(user,&mut borrowed); };
    unsafe { (&mut *ui.engine).within(&b, &mut callback); }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_scope(ui: *mut PxuiUi, label: Str, build: Build, user: *mut c_void) {
    let Some(ui) = (unsafe { get(ui) }) else { return; };
    let Some(label) = (unsafe { text(label) }) else { return; };
    let mut callback = |engine: &mut Ui| if let Some(build) = build { let mut borrowed=PxuiUi { engine: engine as *mut Ui, error:ui.error, returned:ui.returned, a11y_nodes:ui.a11y_nodes, grid_tracks:ui.grid_tracks, grid_updated:ui.grid_updated, owner:false }; build(user,&mut borrowed); };
    unsafe { (&mut *ui.engine).scope(label, &mut callback); }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_col(ui: *mut PxuiUi, padding: f32, label: Str, build: Build, user: *mut c_void) {
    let Some(ui) = (unsafe { get(ui) }) else { return; };
    let Some(label) = (unsafe { text(label) }) else { return; };
    let mut callback = |engine: &mut Ui| if let Some(build) = build { let mut borrowed=PxuiUi { engine: engine as *mut Ui, error:ui.error, returned:ui.returned, a11y_nodes:ui.a11y_nodes, grid_tracks:ui.grid_tracks, grid_updated:ui.grid_updated, owner:false }; build(user,&mut borrowed); };
    unsafe { (&mut *ui.engine).col(padding, label, &mut callback); }
}

pub struct PxuiRenderer {
    painter: Painter,
    queue: Queue,
    device: Device,
    textures: std::collections::HashMap<usize, Texture>,
    engine: *mut Ui,
    error: *mut CString,
    format: TextureFormat,
    samples: i64,
    /// the last frame passed to `prepare`, and the last `completed` the app reported
    frame_serial: u64,
    completed: u64,
    atlas_initialized: bool,
}

impl PxuiRenderer {
    /// Record `message` as the owning `Ui`'s last error and return `code`.
    fn fail(&mut self, code: i32, message: &str) -> i32 {
        if let Ok(m)=CString::new(message) { unsafe { *self.error=m; } }
        code
    }
    fn fail_gpu(&mut self, error: ogpu_core::error::Error) -> i32 { self.fail(PXUI_ERR_GPU, &ogpu_core::error::to_string(&error)) }
}

struct GpuLease { token:*mut c_void, release:Option<extern "C" fn(*mut c_void)>, active:bool }
impl Drop for GpuLease {
    fn drop(&mut self) {
        if self.active {
            if let Some(release)=self.release { release(self.token); }
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_renderer_create(ui:*mut PxuiUi, gpu:*const pxui_types::Gpu, target:*const PxuiTargetDesc) -> *mut PxuiRenderer {
    let Some(ui)=(unsafe { get(ui) }) else { return core::ptr::null_mut(); };
    if gpu.is_null() { return core::ptr::null_mut(); }
    let gpu=unsafe { &*gpu };
    if gpu.size < core::mem::size_of::<pxui_types::Gpu>() as u32 { return core::ptr::null_mut(); }
    let mut lease=GpuLease { token:gpu.owner_token, release:gpu.release_owner, active:true };
    if target.is_null() { return core::ptr::null_mut(); }
    let target=unsafe { *target };
    if target.size < core::mem::size_of::<PxuiTargetDesc>() as u32 || target.samples == 0 { return core::ptr::null_mut(); }
    let (format,color) = match target.format { 1=>(TextureFormat::Rgba8Unorm,ColorFormat::Rgba8Unorm), 2=>(TextureFormat::Bgra8Unorm,ColorFormat::Bgra8Unorm), _=>return core::ptr::null_mut() };
    let native=ogpu_core::backend::NativeDevice { size:gpu.size, backend:gpu.backend, instance:gpu.instance, physical_device:gpu.physical_device, device:gpu.device, queue:gpu.queue, queue_family:gpu.queue_family, api_version:gpu.api_version, features:gpu.features, get_instance_proc_addr:gpu.get_instance_proc_addr, pipeline_cache:gpu.pipeline_cache, mtl_device:gpu.mtl_device, wl_display:gpu.wl_display, wl_surface:gpu.wl_surface, metal_layer:gpu.metal_layer, hwnd:gpu.hwnd, owner_token:gpu.owner_token, release_owner:gpu.release_owner };
    #[cfg(target_os = "macos")]
    let driver={ lease.active=false; match ogpu_metal::adopt(native) { Ok(driver)=>driver, Err(_)=>return core::ptr::null_mut() } };
    #[cfg(any(windows,target_os = "linux"))]
    let driver={ lease.active=false; match ogpu_vulkan::adopt(native) { Ok(driver)=>driver, Err(_)=>return core::ptr::null_mut() } };
    let device=match gpu_backend::create_device(driver) { Ok(d)=>d, Err(_)=>return core::ptr::null_mut() };
    let queue=match gpu_backend::create_queue(&device) { Ok(q)=>q, Err(_)=>{ let _=gpu_backend::destroy_device(&device); return core::ptr::null_mut(); } };
    let (mask,color_size)=unsafe { let text=(&mut *ui.engine).text_mut(); (text.atlas_size(TextKind::Mask),text.atlas_size(TextKind::Color)) };
    if (target.mask_width!=0 && target.mask_width!=mask.0) || (target.mask_height!=0 && target.mask_height!=mask.1) || (target.color_width!=0 && target.color_width!=color_size.0) || (target.color_height!=0 && target.color_height!=color_size.1) {
        let _=gpu_backend::destroy_queue(&queue); let _=gpu_backend::destroy_device(&device); return core::ptr::null_mut();
    }
    let painter=match Painter::new(&device,color,DepthFormat::NoDepth,i64::from(target.samples),mask,color_size) {
        Ok(p)=>p,
        Err(_)=>{ let _=gpu_backend::destroy_queue(&queue); let _=gpu_backend::destroy_device(&device); return core::ptr::null_mut(); }
    };
    Box::into_raw(Box::new(PxuiRenderer { painter, queue, device, textures:std::collections::HashMap::new(), engine:ui.engine, error:ui.error, format, samples:i64::from(target.samples), frame_serial:0, completed:0, atlas_initialized:false }))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_renderer_prepare(renderer:*mut PxuiRenderer, commands:*const pxui_types::Commands, frame:u64, completed:u64) -> i32 {
    if renderer.is_null() { return PXUI_ERR_ARGUMENT; }
    let renderer=unsafe { &mut *renderer };
    if commands.is_null() || unsafe { (*commands).size } < core::mem::size_of::<pxui_types::Commands>() as u32 { return renderer.fail(PXUI_ERR_ARGUMENT, "invalid commands descriptor"); }
    let commands=unsafe { *commands };
    // The painter's instance ring and the glyph atlas reuse what frame `frame - FRAMES` used: that frame must be done.
    if frame <= renderer.frame_serial { return renderer.fail(PXUI_ERR_ARGUMENT, "frame serials must increase"); }
    if completed >= frame || completed < renderer.completed || frame - completed > pxui_render::FRAMES as u64 {
        return renderer.fail(PXUI_ERR_ARGUMENT, "completed must not go back and must be at least frame - 3");
    }
    let native=ogpu_core::backend::NativeCommands { size:core::mem::size_of::<ogpu_core::backend::NativeCommands>() as u32, vk_command_buffer:commands.vk_command_buffer, mtl_command_buffer:commands.mtl_command_buffer, frame_serial:frame, inside_pass:0 };
    let commands=match gpu_backend::adopt_commands(&renderer.queue,native) { Ok(c)=>c, Err(e)=>return renderer.fail_gpu(e) };
    if let Err(e)=gpu_backend::retire(&renderer.queue,completed) { return renderer.fail_gpu(e); }
    renderer.completed=completed;
    let ui=unsafe { &mut *renderer.engine };
    let mut failed=None;
    if !renderer.atlas_initialized {
        for (kind,atlas,texel) in [(TextKind::Mask,AtlasKind::Mask,1i64),(TextKind::Color,AtlasKind::Color,4i64)] {
            let (w,h)=ui.text_mut().atlas_size(kind);
            let bytes=ui.text_mut().atlas_pixels(kind);
            if let Err(error)=gpu_backend::write_texture_on(&commands,renderer.painter.atlas(atlas),None,Origin{x:0,y:0,z:0},Extent{width:w as i64,height:h as i64,depth:1},(w as i64)*texel,bytes) {
                failed=Some(error);
                break;
            }
        }
        if failed.is_none() {
            ui.text_mut().drain_uploads(&mut |_,_,_,_,_,_,_| {});
            renderer.atlas_initialized=true;
        }
    }
    if let Some(e)=failed { return renderer.fail_gpu(e); }
    ui.text_mut().drain_uploads(&mut |kind,x,y,w,h,bytes,stride| {
        let atlas=if kind==TextKind::Mask { AtlasKind::Mask } else { AtlasKind::Color };
        let result=gpu_backend::write_texture_on(&commands,renderer.painter.atlas(atlas),None,Origin{x:x as i64,y:y as i64,z:0},Extent{width:w as i64,height:h as i64,depth:1},stride as i64,bytes);
        if let Err(error)=result { if failed.is_none() { failed=Some(error); } }
    });
    if let Some(e)=failed { renderer.atlas_initialized=false; return renderer.fail_gpu(e); }
    renderer.frame_serial=frame;
    renderer.painter.begin_frame();
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_renderer_draw(renderer:*mut PxuiRenderer, pass:*const pxui_types::Pass) -> i32 {
    if renderer.is_null() { return PXUI_ERR_ARGUMENT; }
    let renderer=unsafe { &mut *renderer };
    if pass.is_null() { return renderer.fail(PXUI_ERR_ARGUMENT, "pass is null"); }
    let pass=unsafe { *pass };
    if pass.size < core::mem::size_of::<pxui_types::Pass>() as u32 || pass.width_px==0 || pass.height_px==0 || !pass.scale.is_finite() || pass.scale<=0.0 { return renderer.fail(PXUI_ERR_ARGUMENT, "invalid pass descriptor"); }
    if renderer.frame_serial==0 { return renderer.fail(PXUI_ERR_STATE, "draw needs a prepared frame"); }
    let commands_native=ogpu_core::backend::NativeCommands { size:core::mem::size_of::<ogpu_core::backend::NativeCommands>() as u32, vk_command_buffer:pass.vk_command_buffer, mtl_command_buffer:pass.mtl_command_buffer, frame_serial:renderer.frame_serial, inside_pass:1 };
    let native_pass=ogpu_core::backend::NativePass { size:core::mem::size_of::<ogpu_core::backend::NativePass>() as u32, vk_command_buffer:pass.vk_command_buffer, mtl_render_encoder:pass.mtl_render_encoder, width_px:pass.width_px, height_px:pass.height_px, scale:pass.scale };
    let commands=match gpu_backend::adopt_commands(&renderer.queue,commands_native) { Ok(c)=>c, Err(e)=>return renderer.fail_gpu(e) };
    let encoder=match gpu_backend::adopt_render_encoder(&commands,native_pass,renderer.format,renderer.samples) { Ok(e)=>e, Err(e)=>return renderer.fail_gpu(e) };
    let ui=unsafe { &mut *renderer.engine };
    for (index,v) in ui.output().views {
        if let Err(e)=renderer.painter.set_view(*index,View { offset:v.offset,opacity:v.opacity,clip:v.clip,scale:v.scale,transform:v.transform,pivot:v.pivot }) { return renderer.fail_gpu(e); }
    }
    let mut used_textures=std::collections::HashSet::new();
    for index in 0..ui.output().draw.len() {
        let layer=&ui.output().draw[index];
        // Adopt foreign textures before building `Break` references. Keeping those
        // references alive while inserting another texture would borrow the map
        // immutably across a mutation.
        for b in &layer.breaks {
            if b.image==pxui_core::draw::ATLAS { continue; }
            let Some(image)=layer.images.get(b.image as usize) else { continue; };
            if image.handle.downcast_ref::<Texture>().is_some() { continue; }
            let Some(key)=image::texture_key(image) else { continue; };
            used_textures.insert(key);
            if !renderer.textures.contains_key(&key) {
                let texture=match image::adopt_texture(&renderer.device,image) { Ok(texture)=>texture, Err(())=>return renderer.fail(PXUI_ERR_GPU, "an app image could not be wrapped") };
                renderer.textures.insert(key,texture);
            }
        }
        let mut breaks=Vec::with_capacity(layer.breaks.len());
        for b in &layer.breaks {
            let texture=if b.image==pxui_core::draw::ATLAS { None } else if let Some(image)=layer.images.get(b.image as usize) {
                if let Some(texture)=image.handle.downcast_ref::<Texture>() { Some(texture) }
                else { image::texture_key(image).and_then(|key|renderer.textures.get(&key)) }
            } else { None };
            breaks.push(Break { at_instance:b.at_instance, texture });
        }
        if let Err(e)=renderer.painter.draw_retained(&encoder,[pass.width_px,pass.height_px],pass.scale,layer.id,&layer.instances,&layer.dirty,&breaks) { drop(breaks); return renderer.fail_gpu(e); }
        ui.drawn(index);
    }
    renderer.textures.retain(|key,_|used_textures.contains(key));
    // ends only this copy's view of the pass; the app's encoder must `invalidate_state` before drawing more
    if let Err(e)=gpu_backend::end_render(&encoder) { return renderer.fail_gpu(e); }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_renderer_destroy(renderer:*mut PxuiRenderer) {
    if renderer.is_null() { return; }
    let PxuiRenderer { painter, queue, device, textures, error, frame_serial, .. } = *unsafe { Box::from_raw(renderer) };
    // a painter or texture that fails to go stays a child of the device, which then refuses to go: reported below
    drop(painter);
    drop(textures);
    // Contract (pxui.h): the app has finished every frame it passed to prepare, so everything retires now.
    let result=gpu_backend::retire(&queue,frame_serial).and_then(|()| gpu_backend::destroy_queue(&queue)).and_then(|()| gpu_backend::destroy_device(&device));
    if let Err(e)=result { if let Ok(m)=CString::new(ogpu_core::error::to_string(&e)) { unsafe { *error=m; } } }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_panel(ui: *mut PxuiUi, panel: *const PxuiPanel, label: Str, build: Build, user: *mut c_void) {
    let (Some(ui), false) = (unsafe { get(ui) }, panel.is_null()) else { return; };
    let Some(label) = (unsafe { text(label) }) else { return; };
    let panel = unsafe { *panel };
    if panel.size < core::mem::size_of::<PxuiPanel>() as u32 { return; }
    let mut opts = PanelOpts::default().at(panel.x, panel.y).width(panel.width).padding(panel.padding).window(panel.window != 0);
    if panel.has_height != 0 { opts = opts.height(panel.height); }
    let mut callback = |engine: &mut Ui| if let Some(build) = build { let mut borrowed=PxuiUi { engine: engine as *mut Ui, error:ui.error, returned:ui.returned, a11y_nodes:ui.a11y_nodes, grid_tracks:ui.grid_tracks, grid_updated:ui.grid_updated, owner:false }; build(user,&mut borrowed); };
    unsafe { (&mut *ui.engine).panel(opts, label, &mut callback); }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_label(ui: *mut PxuiUi, label: Str) {
    let Some(ui) = (unsafe { get(ui) }) else { return; };
    if let Some(label) = unsafe { text(label) } { unsafe { (&mut *ui.engine).label(label); } }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_button(ui: *mut PxuiUi, label: Str) -> u8 {
    let Some(ui) = (unsafe { get(ui) }) else { return 0; };
    unsafe { text(label) }.map_or(0, |label| unsafe { (&mut *ui.engine).button(label) as u8 })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_toggle(ui: *mut PxuiUi, label: Str, on: u8) -> u8 {
    let Some(ui) = (unsafe { get(ui) }) else { return on; };
    unsafe { text(label) }.map_or(on, |label| unsafe { (&mut *ui.engine).toggle(label, on != 0) as u8 })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_text_field(ui: *mut PxuiUi, label: Str, value: Str, changed: *mut Str) -> u8 {
    let Some(ui) = (unsafe { get(ui) }) else { return 0; };
    if changed.is_null() { return 0; }
    let (Some(label), Some(value)) = (unsafe { text(label) }, unsafe { text(value) }) else { return 0; };
    let Some(value) = (unsafe { (&mut *ui.engine).text_field(label, value) }) else { return 0; };
    let Ok(value) = CString::new(value) else { return 0; };
    unsafe { (&mut *ui.returned).push(value); }
    let bytes = unsafe { (&*ui.returned).last().unwrap().as_bytes() };
    unsafe { *changed = Str { ptr: bytes.as_ptr(), len: bytes.len() }; }
    1
}
