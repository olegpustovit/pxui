//! The numbering of keys, buttons and events at the `libpxui` C ABI (`pxui.h`, `pxui_types::Event`), so a Rust host
//! encodes `Event` to the wire and the library decodes it from one definition. The numbers are the ABI contract:
//! `pxui_abi` asserts they equal `pxui_types`.
use crate::{Event, Key, MouseButton, ScrollPhase};

pub const KEY_SPECIAL: u32 = 0x11_0000;
pub const KEY_UNKNOWN: u32 = 0x8000_0000;

/// The special keys in `pxui.h` order: key code `KEY_SPECIAL + n` is `SPECIAL_KEYS[n]`.
pub const SPECIAL_KEYS: [Key; 31] = [
    Key::ArrowUp, Key::ArrowDown, Key::ArrowLeft, Key::ArrowRight, Key::Space, Key::Enter, Key::Escape, Key::Backspace,
    Key::Tab, Key::Shift, Key::Ctrl, Key::Alt, Key::Meta,
    Key::F1, Key::F2, Key::F3, Key::F4, Key::F5, Key::F6, Key::F7, Key::F8, Key::F9, Key::F10, Key::F11, Key::F12,
    Key::Home, Key::End, Key::PageUp, Key::PageDown, Key::Insert, Key::Delete,
];

impl Key {
    /// The key's code on the wire: a character is its scalar value, a special key `KEY_SPECIAL + n`.
    pub fn code(self) -> u32 {
        match self {
            Key::KeyChar(c) => c as u32,
            // ponytail: platform codes outside 0..2^31 are truncated; widen the event if one ever matters
            Key::Unknown(code) => KEY_UNKNOWN | (code as u32 & !KEY_UNKNOWN),
            special => SPECIAL_KEYS.iter().position(|k| *k == special).map_or(KEY_UNKNOWN, |n| KEY_SPECIAL + n as u32),
        }
    }

    pub fn from_code(code: u32) -> Option<Key> {
        if code >= KEY_UNKNOWN { return Some(Key::Unknown(i64::from(code - KEY_UNKNOWN))); }
        if code >= KEY_SPECIAL { return SPECIAL_KEYS.get((code - KEY_SPECIAL) as usize).copied(); }
        char::from_u32(code).filter(|c| *c != '\0').map(Key::KeyChar)
    }
}

impl MouseButton {
    pub fn code(self) -> u32 {
        match self { MouseButton::LeftButton => 0, MouseButton::RightButton => 1, MouseButton::MiddleButton => 2, MouseButton::MouseX1 => 3, MouseButton::MouseX2 => 4 }
    }

    pub fn from_code(code: u32) -> Option<MouseButton> {
        Some(match code { 0 => MouseButton::LeftButton, 1 => MouseButton::RightButton, 2 => MouseButton::MiddleButton, 3 => MouseButton::MouseX1, 4 => MouseButton::MouseX2, _ => return None })
    }
}

impl From<Key> for u32 { fn from(key: Key) -> u32 { key.code() } }
impl From<MouseButton> for u32 { fn from(button: MouseButton) -> u32 { button.code() } }

pub const KEY_PRESSED: u32 = 1;
pub const KEY_RELEASED: u32 = 2;
pub const POINTER_MOVED: u32 = 3;
pub const POINTER_PRESSED: u32 = 4;
pub const POINTER_RELEASED: u32 = 5;
pub const POINTER_CANCELLED: u32 = 6;
pub const SCROLLED: u32 = 7;
pub const TEXT_INPUT: u32 = 8;
pub const PASTE: u32 = 9;
pub const TEXT_EDITING: u32 = 10;
pub const FILE_DROPPED: u32 = 11;
pub const FILE_DRAG_MOVED: u32 = 12;
pub const FILE_DRAG_ENDED: u32 = 13;
pub const PINCHED: u32 = 14;
pub const FILE_DIALOG: u32 = 15;
pub const DIALOG_OPENED: u32 = 16;
pub const RESIZED: u32 = 17;
pub const FOCUS_LOST: u32 = 18;
pub const FOCUS_GAINED: u32 = 19;
pub const CLOSED: u32 = 20;
pub const PIXEL_SIZE_CHANGED: u32 = 21;
pub const VISIBILITY_CHANGED: u32 = 22;
pub const TRACKPAD_SCROLLED: u32 = 23;

/// One event as the C struct carries it: `kind`, `code`, `x`, `y`, `value` and UTF-8 `text` (`None`: the pointer was null).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Raw<'a> { pub kind: u32, pub code: u32, pub x: f64, pub y: f64, pub value: f64, pub text: Option<&'a str> }

impl Event {
    /// The event as the wire carries it. `scratch` holds the text a dialog result joins; it must outlive the `Raw`.
    pub fn raw<'a>(&'a self, scratch: &'a mut String) -> Raw<'a> {
        let raw = |kind, code, x, y, value, text| Raw { kind, code, x, y, value, text: Some(text) };
        match self {
            Event::KeyPressed(k) => raw(KEY_PRESSED, k.code(), 0.0, 0.0, 0.0, ""),
            Event::KeyReleased(k) => raw(KEY_RELEASED, k.code(), 0.0, 0.0, 0.0, ""),
            Event::PointerMoved((x, y)) => raw(POINTER_MOVED, 0, *x, *y, 0.0, ""),
            Event::PointerPressed(b, (x, y)) => raw(POINTER_PRESSED, b.code(), *x, *y, 0.0, ""),
            Event::PointerReleased(b, (x, y)) => raw(POINTER_RELEASED, b.code(), *x, *y, 0.0, ""),
            Event::PointerCancelled(b) => raw(POINTER_CANCELLED, b.code(), 0.0, 0.0, 0.0, ""),
            Event::Scrolled((x, y)) => raw(SCROLLED, 0, *x, *y, 0.0, ""),
            Event::TextInput(s) => raw(TEXT_INPUT, 0, 0.0, 0.0, 0.0, s),
            Event::Paste(s) => raw(PASTE, 0, 0.0, 0.0, 0.0, s),
            Event::TextEditing { text, start, length } => raw(TEXT_EDITING, 0, *start as f64, *length as f64, 0.0, text),
            Event::FileDropped(s) => raw(FILE_DROPPED, 0, 0.0, 0.0, 0.0, s),
            Event::FileDragMoved((x, y)) => raw(FILE_DRAG_MOVED, 0, *x, *y, 0.0, ""),
            Event::FileDragEnded => raw(FILE_DRAG_ENDED, 0, 0.0, 0.0, 0.0, ""),
            Event::Pinched(v) => raw(PINCHED, 0, 0.0, 0.0, *v, ""),
            Event::FileDialog { id, result } => match result {
                Ok(paths) => { *scratch = paths.join("\0"); raw(FILE_DIALOG, 0, 0.0, 0.0, *id as f64, scratch) }
                Err(error) => raw(FILE_DIALOG, 1, 0.0, 0.0, *id as f64, error),
            },
            Event::DialogOpened(id) => raw(DIALOG_OPENED, *id as u32, 0.0, 0.0, 0.0, ""),
            Event::Resized((w, h)) => raw(RESIZED, 0, *w as f64, *h as f64, 0.0, ""),
            Event::FocusLost => raw(FOCUS_LOST, 0, 0.0, 0.0, 0.0, ""),
            Event::FocusGained => raw(FOCUS_GAINED, 0, 0.0, 0.0, 0.0, ""),
            Event::Closed => raw(CLOSED, 0, 0.0, 0.0, 0.0, ""),
            Event::PixelSizeChanged((w, h)) => raw(PIXEL_SIZE_CHANGED, 0, *w as f64, *h as f64, 0.0, ""),
            Event::VisibilityChanged(v) => raw(VISIBILITY_CHANGED, 0, 0.0, 0.0, *v as u8 as f64, ""),
            Event::TrackpadScrolled { delta: (x, y), phase, time } => {
                let code = match phase { ScrollPhase::Touched => 0, ScrollPhase::Moved => 1, ScrollPhase::Lifted => 2, ScrollPhase::Momentum => 3 };
                raw(TRACKPAD_SCROLLED, code, *x, *y, *time, "")
            }
        }
    }

    /// The event a wire record encodes, or `None` for an unknown kind, code or a missing payload.
    pub fn from_raw(e: &Raw) -> Option<Event> {
        let s = e.text;
        Some(match e.kind {
            KEY_PRESSED => Event::KeyPressed(Key::from_code(e.code)?),
            KEY_RELEASED => Event::KeyReleased(Key::from_code(e.code)?),
            POINTER_MOVED => Event::PointerMoved((e.x, e.y)),
            POINTER_PRESSED => Event::PointerPressed(MouseButton::from_code(e.code)?, (e.x, e.y)),
            POINTER_RELEASED => Event::PointerReleased(MouseButton::from_code(e.code)?, (e.x, e.y)),
            POINTER_CANCELLED => Event::PointerCancelled(MouseButton::from_code(e.code)?),
            SCROLLED => Event::Scrolled((e.x, e.y)),
            TEXT_INPUT => Event::TextInput(s?.to_owned()),
            PASTE => Event::Paste(s?.to_owned()),
            TEXT_EDITING => Event::TextEditing { text: s?.to_owned(), start: e.x as i64, length: e.y as i64 },
            FILE_DROPPED => Event::FileDropped(s?.to_owned()),
            FILE_DRAG_MOVED => Event::FileDragMoved((e.x, e.y)),
            FILE_DRAG_ENDED => Event::FileDragEnded,
            PINCHED => Event::Pinched(e.value),
            FILE_DIALOG => match e.code {
                0 => Event::FileDialog { id: e.value as i64, result: Ok(s?.split('\0').map(str::to_owned).collect()) },
                1 => Event::FileDialog { id: e.value as i64, result: Err(s?.to_owned()) },
                _ => return None,
            },
            DIALOG_OPENED => Event::DialogOpened(e.code as i64),
            RESIZED => Event::Resized((e.x as i64, e.y as i64)),
            FOCUS_LOST => Event::FocusLost,
            FOCUS_GAINED => Event::FocusGained,
            CLOSED => Event::Closed,
            PIXEL_SIZE_CHANGED => Event::PixelSizeChanged((e.x as i64, e.y as i64)),
            VISIBILITY_CHANGED => Event::VisibilityChanged(e.value != 0.0),
            TRACKPAD_SCROLLED => Event::TrackpadScrolled {
                delta: (e.x, e.y), time: e.value,
                phase: match e.code { 0 => ScrollPhase::Touched, 1 => ScrollPhase::Moved, 2 => ScrollPhase::Lifted, 3 => ScrollPhase::Momentum, _ => return None },
            },
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_codes_round_trip_and_never_collide() {
        let mut keys = vec![Key::KeyChar('a'), Key::KeyChar('\u{100}'), Key::KeyChar('\u{10ffff}'), Key::Unknown(7), Key::Unknown(0x7fff_ffff)];
        keys.extend(SPECIAL_KEYS);
        let mut codes = std::collections::HashSet::new();
        for k in keys {
            let code = k.code();
            assert_eq!(Key::from_code(code), Some(k), "{k:?} -> {code:#x}");
            assert!(codes.insert(code), "{k:?} collides");
        }
        assert_eq!(Key::from_code(0), None);
        assert_eq!(Key::from_code(0xd800), None);
        assert_eq!(Key::from_code(KEY_SPECIAL + SPECIAL_KEYS.len() as u32), None);
        assert_ne!(Key::KeyChar('\u{100}').code(), Key::ArrowUp.code());
    }

    #[test]
    fn events_round_trip() {
        let events = [
            Event::KeyPressed(Key::Enter), Event::KeyReleased(Key::KeyChar('x')), Event::PointerMoved((1.5, 2.5)),
            Event::PointerPressed(MouseButton::RightButton, (3.0, 4.0)), Event::PointerReleased(MouseButton::MouseX2, (5.0, 6.0)),
            Event::PointerCancelled(MouseButton::LeftButton), Event::Scrolled((0.0, -1.0)), Event::TextInput("hé".into()), Event::Paste("p".into()),
            Event::TextEditing { text: "ime".into(), start: 1, length: 2 }, Event::FileDropped("/a".into()), Event::FileDragMoved((1.0, 2.0)),
            Event::FileDragEnded, Event::Pinched(1.25), Event::FileDialog { id: 7, result: Ok(vec!["/a".into(), "/b".into()]) },
            Event::FileDialog { id: 8, result: Err("no".into()) }, Event::DialogOpened(9), Event::Resized((10, 20)), Event::FocusLost,
            Event::FocusGained, Event::Closed, Event::PixelSizeChanged((30, 40)), Event::VisibilityChanged(true),
            Event::TrackpadScrolled { delta: (1.0, 2.0), phase: ScrollPhase::Momentum, time: 3.0 },
        ];
        for event in events {
            let mut scratch = String::new();
            assert_eq!(Event::from_raw(&event.raw(&mut scratch)), Some(event.clone()), "{event:?}");
        }
    }
}
