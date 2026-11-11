use crate::{PxuiUi, get};
use input::{Event, Key, MouseButton, ScrollPhase};
use pxui_core::ui::{Saved, Signal, UiBox};
use pxui_types::{Event as FfiEvent, PxuiBox, Str, ffi::Slice};

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct PxuiSaved { pub size: u32, pub reserved: u32, pub parents: usize, pub seeds: usize }

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PxuiSignal {
    pub size: u32,
    pub flags: u32,
    pub has_subtree_press: u8,
    pub button: u8,
    pub has_a11y_value: u8,
    pub has_a11y_select: u8,
    pub reserved: [u8; 4],
    pub subtree_press: i64,
    pub clicks: i64,
    pub drag: [f32; 2],
    pub pointer: [f32; 2],
    pub press_point: [f32; 2],
    pub release_point: [f32; 2],
    pub scroll: [f32; 2],
    pub pinch: f64,
    pub a11y_value: f64,
    pub a11y_select: [u32; 2],
    pub keys: Slice<FfiEvent>,
    pub text_len: usize,
}

impl Default for PxuiSignal {
    fn default() -> Self {
        Self { size: 0, flags: 0, has_subtree_press: 0, button: u8::MAX, has_a11y_value: 0, has_a11y_select: 0,
            reserved: [0; 4], subtree_press: 0, clicks: 0, drag: [0.0; 2], pointer: [0.0; 2], press_point: [0.0; 2],
            release_point: [0.0; 2], scroll: [0.0; 2], pinch: 1.0, a11y_value: 0.0, a11y_select: [0; 2],
            keys: Slice { ptr: core::ptr::null(), len: 0 }, text_len: 0 }
    }
}

fn core_box(raw: PxuiBox) -> Option<UiBox> {
    Some(UiBox { index: usize::try_from(raw.index).ok()?, box_key: raw.key, box_slot: usize::try_from(raw.slot).ok()?, generation: raw.generation })
}
fn live_box(engine: &pxui_core::ui::Ui, raw: PxuiBox) -> Option<UiBox> {
    let b = core_box(raw)?;
    engine.live(&b).then_some(b)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_enter(ui: *mut PxuiUi, box_: *const PxuiBox, out: *mut PxuiSaved) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return -1; };
    if box_.is_null() || out.is_null() { return unsafe { crate::fail(state as *mut PxuiUi, "null enter argument") }; }
    let engine = unsafe { &mut *state.engine };
    let Some(box_) = live_box(engine, unsafe { *box_ }) else { return unsafe { crate::fail(state as *mut PxuiUi, "stale enter box") }; };
    if !engine.is_current_build_box(&box_) { return unsafe { crate::fail(state as *mut PxuiUi, "enter box is not in the current build") }; }
    let saved = engine.enter(&box_);
    let Saved::Depths(parents, seeds) = saved;
    unsafe { *out = PxuiSaved { size: core::mem::size_of::<PxuiSaved>() as u32, reserved: 0, parents, seeds }; }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_leave(ui: *mut PxuiUi, saved: *const PxuiSaved) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return -1; };
    if saved.is_null() { return unsafe { crate::fail(state as *mut PxuiUi, "null leave state") }; }
    let saved = unsafe { *saved };
    if saved.size < core::mem::size_of::<PxuiSaved>() as u32 || saved.parents > 1 << 20 || saved.seeds > 1 << 20 {
        return unsafe { crate::fail(state as *mut PxuiUi, "invalid leave state") };
    }
    unsafe { (&mut *state.engine).leave(Saved::Depths(saved.parents, saved.seeds)); }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_focus_box(ui: *mut PxuiUi, box_: *const PxuiBox) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return -1; };
    if box_.is_null() { return unsafe { crate::fail(state as *mut PxuiUi, "null focus box") }; }
    let engine = unsafe { &mut *state.engine };
    let Some(box_) = live_box(engine, unsafe { *box_ }) else { return unsafe { crate::fail(state as *mut PxuiUi, "stale focus box") }; };
    engine.focus_box(&box_);
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_focused(ui: *mut PxuiUi, box_: *const PxuiBox, out: *mut u8) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return -1; };
    if box_.is_null() || out.is_null() { return unsafe { crate::fail(state as *mut PxuiUi, "null focused argument") }; }
    let engine = unsafe { &*state.engine };
    let Some(box_) = live_box(engine, unsafe { *box_ }) else { return unsafe { crate::fail(state as *mut PxuiUi, "stale focused box") }; };
    unsafe { *out = engine.focused(&box_) as u8; }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_state(ui: *mut PxuiUi, box_: *const PxuiBox, default: i64, out: *mut i64) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return -1; };
    if box_.is_null() || out.is_null() { return unsafe { crate::fail(state as *mut PxuiUi, "null state argument") }; }
    let engine = unsafe { &*state.engine };
    let Some(box_) = live_box(engine, unsafe { *box_ }) else { return unsafe { crate::fail(state as *mut PxuiUi, "stale state box") }; };
    unsafe { *out = engine.state(&box_, default); }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_set_state(ui: *mut PxuiUi, box_: *const PxuiBox, value: i64) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return -1; };
    if box_.is_null() { return unsafe { crate::fail(state as *mut PxuiUi, "null state box") }; }
    let engine = unsafe { &mut *state.engine };
    let Some(box_) = live_box(engine, unsafe { *box_ }) else { return unsafe { crate::fail(state as *mut PxuiUi, "stale state box") }; };
    engine.set_state(&box_, value);
    0
}

#[derive(Default)]
struct EncodedEvent { event: FfiEvent, text: Vec<u8> }
fn key_code(key: Key) -> u32 {
    match key {
        Key::KeyChar(c) => c as u32,
        Key::ArrowUp => 0x100, Key::ArrowDown => 0x101, Key::ArrowLeft => 0x102, Key::ArrowRight => 0x103,
        Key::Space => 0x104, Key::Enter => 0x105, Key::Escape => 0x106, Key::Backspace => 0x107,
        Key::Tab => 0x108, Key::Shift => 0x109, Key::Ctrl => 0x10a, Key::Alt => 0x10b, Key::Meta => 0x10c,
        Key::F1 => 0x120, Key::F2 => 0x121, Key::F3 => 0x122, Key::F4 => 0x123, Key::F5 => 0x124, Key::F6 => 0x125,
        Key::F7 => 0x126, Key::F8 => 0x127, Key::F9 => 0x128, Key::F10 => 0x129, Key::F11 => 0x12a, Key::F12 => 0x12b,
        Key::Home => 0x130, Key::End => 0x131, Key::PageUp => 0x132, Key::PageDown => 0x133, Key::Insert => 0x134,
        Key::Delete => 0x135, Key::Unknown(code) => code as u32,
    }
}
fn button_code(button: MouseButton) -> u32 { match button { MouseButton::LeftButton => 0, MouseButton::RightButton => 1, MouseButton::MiddleButton => 2, MouseButton::MouseX1 => 3, MouseButton::MouseX2 => 4 } }
fn event(value: &Event) -> EncodedEvent {
    let mut out = EncodedEvent::default();
    match value {
        Event::KeyPressed(k) => { out.event.kind = FfiEvent::KEY_PRESSED; out.event.code = key_code(*k); }
        Event::KeyReleased(k) => { out.event.kind = FfiEvent::KEY_RELEASED; out.event.code = key_code(*k); }
        Event::PointerMoved(p) => { out.event.kind = FfiEvent::POINTER_MOVED; (out.event.x,out.event.y)=*p; }
        Event::PointerPressed(b,p) => { out.event.kind = FfiEvent::POINTER_PRESSED; out.event.code=button_code(*b); (out.event.x,out.event.y)=*p; }
        Event::PointerReleased(b,p) => { out.event.kind = FfiEvent::POINTER_RELEASED; out.event.code=button_code(*b); (out.event.x,out.event.y)=*p; }
        Event::PointerCancelled(b) => { out.event.kind=FfiEvent::POINTER_CANCELLED; out.event.code=button_code(*b); }
        Event::Scrolled(p) => { out.event.kind=FfiEvent::SCROLLED; (out.event.x,out.event.y)=*p; }
        Event::TextInput(s) => { out.event.kind=FfiEvent::TEXT_INPUT; out.text.extend_from_slice(s.as_bytes()); }
        Event::Paste(s) => { out.event.kind=FfiEvent::PASTE; out.text.extend_from_slice(s.as_bytes()); }
        Event::TextEditing{text,start,length} => { out.event.kind=FfiEvent::TEXT_EDITING; out.event.x=*start as f64; out.event.y=*length as f64; out.text.extend_from_slice(text.as_bytes()); }
        Event::FileDropped(s) => { out.event.kind=FfiEvent::FILE_DROPPED; out.text.extend_from_slice(s.as_bytes()); }
        Event::FileDragMoved(p) => { out.event.kind=FfiEvent::FILE_DRAG_MOVED; (out.event.x,out.event.y)=*p; }
        Event::FileDragEnded => out.event.kind=FfiEvent::FILE_DRAG_ENDED,
        Event::Pinched(v) => { out.event.kind=FfiEvent::PINCHED; out.event.value=*v; }
        Event::FileDialog{id,result} => { out.event.kind=FfiEvent::FILE_DIALOG; out.event.value=*id as f64; match result { Ok(paths)=>{out.event.code=0;for (i,p) in paths.iter().enumerate(){if i!=0{out.text.push(0)}out.text.extend_from_slice(p.as_bytes())}},Err(message)=>{out.event.code=1;out.text.extend_from_slice(message.as_bytes())} } }
        Event::DialogOpened(id) => { out.event.kind=FfiEvent::DIALOG_OPENED; out.event.code=*id as u32; }
        Event::Resized((w,h)) => { out.event.kind=FfiEvent::RESIZED; out.event.x=*w as f64; out.event.y=*h as f64; }
        Event::FocusLost => out.event.kind=FfiEvent::FOCUS_LOST,
        Event::FocusGained => out.event.kind=FfiEvent::FOCUS_GAINED,
        Event::Closed => out.event.kind=FfiEvent::CLOSED,
        Event::PixelSizeChanged((w,h)) => { out.event.kind=FfiEvent::PIXEL_SIZE_CHANGED; out.event.x=*w as f64; out.event.y=*h as f64; }
        Event::VisibilityChanged(v) => { out.event.kind=FfiEvent::VISIBILITY_CHANGED; out.event.value=*v as u8 as f64; }
        Event::TrackpadScrolled{delta,phase,time} => { out.event.kind=FfiEvent::TRACKPAD_SCROLLED; (out.event.x,out.event.y)=*delta; out.event.code=match phase{ScrollPhase::Touched=>0,ScrollPhase::Moved=>1,ScrollPhase::Lifted=>2,ScrollPhase::Momentum=>3}; out.event.value=*time; }
    }
    out
}

fn copy_signal(signal: &Signal, output: &mut PxuiSignal) {
    output.size = core::mem::size_of::<PxuiSignal>() as u32;
    output.flags = signal.hovered as u32 | (signal.pressed as u32)<<1 | (signal.held as u32)<<2 | (signal.released as u32)<<3 | (signal.clicked as u32)<<4 | (signal.double_clicked as u32)<<5 | (signal.dragging as u32)<<6;
    output.has_subtree_press = signal.subtree_press.is_some() as u8; output.subtree_press=signal.subtree_press.unwrap_or(0);
    output.clicks=signal.clicks; output.drag=[signal.drag.0,signal.drag.1]; output.pointer=[signal.pointer.0,signal.pointer.1];
    output.press_point=[signal.press_point.0,signal.press_point.1]; output.release_point=[signal.release_point.0,signal.release_point.1];
    output.button=signal.button.map_or(u8::MAX,|b|button_code(b) as u8); output.scroll=[signal.scroll.0,signal.scroll.1]; output.pinch=signal.pinch;
    output.has_a11y_value=signal.a11y_value.is_some() as u8; output.a11y_value=signal.a11y_value.unwrap_or(0.0);
    output.has_a11y_select=signal.a11y_select.is_some() as u8;
    let selection=signal.a11y_select.unwrap_or((0,0)); output.a11y_select=[selection.0,selection.1];
}

/// Returns 1 when the caller-provided event/text buffers need to be enlarged; output lengths are set either way.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_signal(ui: *mut PxuiUi, box_: *const PxuiBox, out: *mut PxuiSignal, events: *mut FfiEvent, event_capacity: usize, text: *mut u8, text_capacity: usize) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return -1; };
    if box_.is_null() || out.is_null() || (event_capacity != 0 && events.is_null()) || (text_capacity != 0 && text.is_null()) {
        return unsafe { crate::fail(state as *mut PxuiUi, "invalid signal buffers") };
    }
    let engine = unsafe { &*state.engine };
    let Some(box_) = live_box(engine, unsafe { *box_ }) else { return unsafe { crate::fail(state as *mut PxuiUi, "stale signal box") }; };
    let signal = engine.signal(&box_);
    let encoded: Vec<_> = signal.keys.iter().map(event).collect();
    let text_len = encoded.iter().try_fold(0usize, |n,e| n.checked_add(e.text.len()));
    let Some(text_len) = text_len else { return unsafe { crate::fail(state as *mut PxuiUi, "signal text length overflow") }; };
    let output = unsafe { &mut *out };
    *output = PxuiSignal::default(); copy_signal(&signal,output);
    output.keys.len=encoded.len(); output.text_len=text_len;
    if event_capacity < encoded.len() || text_capacity < text_len { return 1; }
    let (mut event_at,mut text_at)=(0usize,0usize);
    for item in encoded {
        let mut item_event=item.event;
        if !item.text.is_empty() {
            unsafe { core::ptr::copy_nonoverlapping(item.text.as_ptr(),text.add(text_at),item.text.len()); }
            item_event.text=Str{ptr:unsafe{text.add(text_at)},len:item.text.len()}; text_at+=item.text.len();
        }
        unsafe { events.add(event_at).write(item_event); }
        event_at+=1;
    }
    output.keys.ptr=events; 0
}

#[cfg(test)]
mod tests {
    use super::{event, Event, FfiEvent, ScrollPhase};

    #[test]
    fn signal_event_encoding_preserves_edit_and_trackpad_fields() {
        let edit = event(&Event::TextEditing { text: "abc".into(), start: 2, length: 1 });
        assert_eq!((edit.event.kind, edit.event.x, edit.event.y), (FfiEvent::TEXT_EDITING, 2.0, 1.0));
        assert_eq!(edit.text, b"abc");
        let scroll = event(&Event::TrackpadScrolled { delta: (3.0, -4.0), phase: ScrollPhase::Momentum, time: 5.0 });
        assert_eq!((scroll.event.kind, scroll.event.code, scroll.event.x, scroll.event.y, scroll.event.value), (FfiEvent::TRACKPAD_SCROLLED, 3, 3.0, -4.0, 5.0));
    }
}
