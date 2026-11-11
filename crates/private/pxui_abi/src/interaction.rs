//! Building blocks for widgets written outside the library: enter/leave, focus, per-box state and `signal`.
use crate::state::{KeyChunk, PxuiUi, fail, get, live_box};
use crate::ui::{button_code, key_code};
use crate::{PXUI_ERR_ARGUMENT, PxuiBox, PxuiSaved, PxuiSignal, Str};
use input::{Event, ScrollPhase};
use pxui_core::ui::{Saved, Signal};
use pxui_types::{Event as FfiEvent, ffi::Slice};

export!(pub unsafe fn pxui_ui_enter(ui: *mut PxuiUi, box_: *const PxuiBox, out: *mut PxuiSaved) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if box_.is_null() || out.is_null() { return unsafe { fail(ui, "null enter argument") }; }
    let engine = unsafe { &mut *state.engine };
    let Some(box_) = live_box(engine, unsafe { *box_ }) else { return unsafe { fail(ui, "stale enter box") }; };
    if !engine.is_current_build_box(&box_) { return unsafe { fail(ui, "enter box is not in the current build") }; }
    let Saved::Depths(parents, seeds) = engine.enter(&box_);
    unsafe { *out = PxuiSaved { size: core::mem::size_of::<PxuiSaved>() as u32, reserved: 0, parents, seeds }; }
    0
});

export!(pub unsafe fn pxui_ui_leave(ui: *mut PxuiUi, saved: *const PxuiSaved) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if saved.is_null() { return unsafe { fail(ui, "null leave state") }; }
    let saved = unsafe { *saved };
    if saved.size < core::mem::size_of::<PxuiSaved>() as u32 || saved.parents > 1 << 20 || saved.seeds > 1 << 20 {
        return unsafe { fail(ui, "invalid leave state") };
    }
    unsafe { (&mut *state.engine).leave(Saved::Depths(saved.parents, saved.seeds)); }
    0
});

export!(pub unsafe fn pxui_ui_focus_box(ui: *mut PxuiUi, box_: *const PxuiBox) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if box_.is_null() { return unsafe { fail(ui, "null focus box") }; }
    let engine = unsafe { &mut *state.engine };
    let Some(box_) = live_box(engine, unsafe { *box_ }) else { return unsafe { fail(ui, "stale focus box") }; };
    engine.focus_box(&box_);
    0
});

export!(pub unsafe fn pxui_ui_focused(ui: *mut PxuiUi, box_: *const PxuiBox, out: *mut u8) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if box_.is_null() || out.is_null() { return unsafe { fail(ui, "null focused argument") }; }
    let engine = unsafe { &*state.engine };
    let Some(box_) = live_box(engine, unsafe { *box_ }) else { return unsafe { fail(ui, "stale focused box") }; };
    unsafe { *out = engine.focused(&box_) as u8; }
    0
});

export!(pub unsafe fn pxui_ui_state(ui: *mut PxuiUi, box_: *const PxuiBox, default: i64, out: *mut i64) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if box_.is_null() || out.is_null() { return unsafe { fail(ui, "null state argument") }; }
    let engine = unsafe { &*state.engine };
    let Some(box_) = live_box(engine, unsafe { *box_ }) else { return unsafe { fail(ui, "stale state box") }; };
    unsafe { *out = engine.state(&box_, default); }
    0
});

export!(pub unsafe fn pxui_ui_set_state(ui: *mut PxuiUi, box_: *const PxuiBox, value: i64) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if box_.is_null() { return unsafe { fail(ui, "null state box") }; }
    let engine = unsafe { &mut *state.engine };
    let Some(box_) = live_box(engine, unsafe { *box_ }) else { return unsafe { fail(ui, "stale state box") }; };
    engine.set_state(&box_, value);
    0
});

/// The bytes of the text `event` carries; a file dialog's paths are joined by NUL.
fn text_len(event: &Event) -> usize {
    match event {
        Event::TextInput(s) | Event::Paste(s) | Event::FileDropped(s) | Event::TextEditing { text: s, .. } | Event::FileDialog { result: Err(s), .. } => s.len(),
        Event::FileDialog { result: Ok(paths), .. } => paths.iter().map(String::len).sum::<usize>() + paths.len().saturating_sub(1),
        _ => 0,
    }
}

/// Encodes `value` and appends the text it carries to `text` (the caller reserved room, so pointers into it stay valid).
fn encode(value: &Event, text: &mut String) -> FfiEvent {
    let mut out = FfiEvent::default();
    match value {
        Event::KeyPressed(k) => { out.kind = FfiEvent::KEY_PRESSED; out.code = key_code(*k); }
        Event::KeyReleased(k) => { out.kind = FfiEvent::KEY_RELEASED; out.code = key_code(*k); }
        Event::PointerMoved(p) => { out.kind = FfiEvent::POINTER_MOVED; (out.x, out.y) = *p; }
        Event::PointerPressed(b, p) => { out.kind = FfiEvent::POINTER_PRESSED; out.code = button_code(*b); (out.x, out.y) = *p; }
        Event::PointerReleased(b, p) => { out.kind = FfiEvent::POINTER_RELEASED; out.code = button_code(*b); (out.x, out.y) = *p; }
        Event::PointerCancelled(b) => { out.kind = FfiEvent::POINTER_CANCELLED; out.code = button_code(*b); }
        Event::Scrolled(p) => { out.kind = FfiEvent::SCROLLED; (out.x, out.y) = *p; }
        Event::TextInput(s) => { out.kind = FfiEvent::TEXT_INPUT; text.push_str(s); }
        Event::Paste(s) => { out.kind = FfiEvent::PASTE; text.push_str(s); }
        Event::TextEditing { text: s, start, length } => { out.kind = FfiEvent::TEXT_EDITING; out.x = *start as f64; out.y = *length as f64; text.push_str(s); }
        Event::FileDropped(s) => { out.kind = FfiEvent::FILE_DROPPED; text.push_str(s); }
        Event::FileDragMoved(p) => { out.kind = FfiEvent::FILE_DRAG_MOVED; (out.x, out.y) = *p; }
        Event::FileDragEnded => out.kind = FfiEvent::FILE_DRAG_ENDED,
        Event::Pinched(v) => { out.kind = FfiEvent::PINCHED; out.value = *v; }
        Event::FileDialog { id, result } => {
            out.kind = FfiEvent::FILE_DIALOG;
            out.value = *id as f64;
            match result {
                Ok(paths) => { out.code = 0; for (i, p) in paths.iter().enumerate() { if i != 0 { text.push('\0'); } text.push_str(p); } }
                Err(message) => { out.code = 1; text.push_str(message); }
            }
        }
        Event::DialogOpened(id) => { out.kind = FfiEvent::DIALOG_OPENED; out.code = *id as u32; }
        Event::Resized((w, h)) => { out.kind = FfiEvent::RESIZED; out.x = *w as f64; out.y = *h as f64; }
        Event::FocusLost => out.kind = FfiEvent::FOCUS_LOST,
        Event::FocusGained => out.kind = FfiEvent::FOCUS_GAINED,
        Event::Closed => out.kind = FfiEvent::CLOSED,
        Event::PixelSizeChanged((w, h)) => { out.kind = FfiEvent::PIXEL_SIZE_CHANGED; out.x = *w as f64; out.y = *h as f64; }
        Event::VisibilityChanged(v) => { out.kind = FfiEvent::VISIBILITY_CHANGED; out.value = *v as u8 as f64; }
        Event::TrackpadScrolled { delta, phase, time } => {
            out.kind = FfiEvent::TRACKPAD_SCROLLED;
            (out.x, out.y) = *delta;
            out.code = match phase { ScrollPhase::Touched => 0, ScrollPhase::Moved => 1, ScrollPhase::Lifted => 2, ScrollPhase::Momentum => 3 };
            out.value = *time;
        }
    }
    out
}

/// Encodes `keys` into `chunk`: its events, and their text in one buffer that the events point into.
fn fill(chunk: &mut KeyChunk, keys: &[Event]) {
    chunk.events.clear();
    chunk.text.clear();
    chunk.text.reserve(keys.iter().map(text_len).sum());
    for key in keys {
        let start = chunk.text.len();
        let mut event = encode(key, &mut chunk.text);
        let len = chunk.text.len() - start;
        if len != 0 { event.text = Str { ptr: chunk.text.as_ptr().wrapping_add(start), len }; }
        chunk.events.push(event);
    }
}

fn copy_signal(signal: &Signal, output: &mut PxuiSignal) {
    output.flags = signal.hovered as u32 | (signal.pressed as u32) << 1 | (signal.held as u32) << 2 | (signal.released as u32) << 3
        | (signal.clicked as u32) << 4 | (signal.double_clicked as u32) << 5 | (signal.dragging as u32) << 6;
    output.has_subtree_press = signal.subtree_press.is_some() as u8;
    output.subtree_press = signal.subtree_press.unwrap_or(0);
    output.clicks = signal.clicks;
    output.drag = [signal.drag.0, signal.drag.1];
    output.pointer = [signal.pointer.0, signal.pointer.1];
    output.press_point = [signal.press_point.0, signal.press_point.1];
    output.release_point = [signal.release_point.0, signal.release_point.1];
    output.button = signal.button.map_or(u8::MAX, |b| button_code(b) as u8);
    output.scroll = [signal.scroll.0, signal.scroll.1];
    output.pinch = signal.pinch;
    output.has_a11y_value = signal.a11y_value.is_some() as u8;
    output.a11y_value = signal.a11y_value.unwrap_or(0.0);
    output.has_a11y_select = signal.a11y_select.is_some() as u8;
    let selection = signal.a11y_select.unwrap_or((0, 0));
    output.a11y_select = [selection.0, selection.1];
}

/// Fills `output` from `signal`; its key events (and their text) are kept in the `Ui`'s chunk buffers until the next frame.
pub(crate) fn write_signal(state: &PxuiUi, signal: &Signal, output: &mut PxuiSignal) {
    *output = PxuiSignal::default();
    copy_signal(signal, output);
    if !signal.keys.is_empty() {
        let shared = state.shared();
        if shared.chunks_used == shared.chunks.len() { shared.chunks.push(KeyChunk::default()); }
        let chunk = &mut shared.chunks[shared.chunks_used];
        shared.chunks_used += 1;
        fill(chunk, &signal.keys);
        output.keys = Slice { ptr: chunk.events.as_ptr(), len: chunk.events.len() };
    }
}

// One call, scalars in `out`; `out.keys` (and the text its events point at) is borrowed from the `Ui` until the next frame.
// A box with no key events costs no allocation.
export!(pub unsafe fn pxui_ui_signal(ui: *mut PxuiUi, box_: *const PxuiBox, out: *mut PxuiSignal) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if box_.is_null() || out.is_null() { return unsafe { fail(ui, "invalid signal arguments") }; }
    let engine = unsafe { &*state.engine };
    let Some(box_) = live_box(engine, unsafe { *box_ }) else { return unsafe { crate::state::fail_code(ui, "stale signal box", crate::PXUI_ERR_STALE) }; };
    let signal = engine.signal(&box_);
    unsafe { write_signal(state, &signal, &mut *out); }
    0
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_event_encoding_preserves_edit_trackpad_and_file_dialog_fields() {
        let keys = [
            Event::TextEditing { text: "abc".into(), start: 2, length: 1 },
            Event::TrackpadScrolled { delta: (3.0, -4.0), phase: ScrollPhase::Momentum, time: 5.0 },
            Event::FileDialog { id: 7, result: Ok(vec!["a".into(), "bc".into()]) },
            Event::TextInput("é".into()),
        ];
        let mut chunk = KeyChunk::default();
        fill(&mut chunk, &keys);
        let e = &chunk.events;
        assert_eq!((e[0].kind, e[0].x, e[0].y), (FfiEvent::TEXT_EDITING, 2.0, 1.0));
        assert_eq!((e[1].kind, e[1].code, e[1].x, e[1].y, e[1].value), (FfiEvent::TRACKPAD_SCROLLED, 3, 3.0, -4.0, 5.0));
        let text = |i: usize| unsafe { core::str::from_utf8(core::slice::from_raw_parts(e[i].text.ptr, e[i].text.len)).unwrap() };
        assert_eq!((text(0), text(2), text(3)), ("abc", "a\0bc", "é"));
        assert_eq!(e[1].text.len, 0);
    }
}
