//! Text authoring: `ui.label_fmt` formats into a fixed stack buffer, `ui.label_with` and `ui.label_fmt_with` show text in a keyed node
//! with an explicit style, and `text_style` names the usual styles.
use core::fmt::{Arguments, Write};
use pxui_types::{Node, PxuiBox, TextStyle};
use super::layout::{id_text, Key};
use crate::Ui;

/// Named text styles for `label_with`: `BODY`, `CAPTION`, `TITLE` and `MONO`. Spread one to change a field:
/// `TextStyle { weight: 700, ..text_style::BODY }`.
pub mod text_style {
    use pxui_types::TextStyle;
    const BASE: TextStyle = TextStyle {
        size: core::mem::size_of::<TextStyle>() as u32, font: 0, weight: 400, size_points: 14.0, line_height: 0.0, wrap: 0, align: 0,
        max_lines: 0, italic: 0, ellipsis: 0, features: 0, tracking: 0.0,
    };
    /// Running text: the default font at 14 points, regular weight.
    pub const BODY: TextStyle = BASE;
    /// Secondary text: 11 points, regular weight.
    pub const CAPTION: TextStyle = TextStyle { size_points: 11.0, ..BASE };
    /// Headings: 20 points, bold.
    pub const TITLE: TextStyle = TextStyle { size_points: 20.0, weight: 700, ..BASE };
    /// Figures and counters: body size in the default font with tabular numbers, so digits keep one width.
    pub const MONO: TextStyle = TextStyle { features: TextStyle::FEATURE_TABULAR_NUMBERS, ..BASE };
}

/// The bytes a formatted label keeps; the rest is dropped at a character boundary.
const CAPACITY: usize = 128;

/// A fixed stack buffer that truncates at a UTF-8 character boundary and never spills to the heap.
struct Stack { bytes: [u8; CAPACITY], len: usize }

impl Write for Stack {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let room = CAPACITY - self.len;
        let mut take = s.len().min(room);
        while !s.is_char_boundary(take) { take -= 1; }
        self.bytes[self.len..self.len + take].copy_from_slice(&s.as_bytes()[..take]);
        self.len += take;
        // a full buffer stops the formatting early: nothing more can be kept
        if take < s.len() { Err(core::fmt::Error) } else { Ok(()) }
    }
}

impl Stack {
    fn new(args: Arguments) -> Stack {
        let mut stack = Stack { bytes: [0; CAPACITY], len: 0 };
        let _ = stack.write_fmt(args);
        stack
    }
    fn as_str(&self) -> &str { core::str::from_utf8(&self.bytes[..self.len]).unwrap_or_default() }
}

impl Ui {
    /// Shows formatted text as `label` does, formatting into a 128-byte stack buffer.
    ///
    /// Text past 128 bytes is dropped at a UTF-8 character boundary, so the label never holds a partial character.
    /// Allocates: never; a warm frame with a repeating value allocates nothing.
    /// Panics: when the library refuses the label (outside a frame).
    ///
    /// ```
    /// let ui = pxui::doc::frames(2, &mut |ui| ui.label_fmt(format_args!("{} songs", 10_000)));
    /// assert!(ui.a11y_nodes().iter().any(|n| n.label == "10000 songs"));
    /// ```
    #[inline]
    #[track_caller]
    pub fn label_fmt(&mut self, args: Arguments) { self.label(Stack::new(args).as_str()) }

    /// Declares a node under `key` showing `text` in `style`, and returns its handle.
    ///
    /// The node is `Node::DEFAULT` (full width, fitting height); style the handle further if needed.
    /// Allocates: as `node` and `text_spans` (never on a warm frame).
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`).
    /// Panics: when the library refuses the node or the text (outside a frame).
    ///
    /// ```
    /// use pxui::text_style;
    /// let mut title = None;
    /// let ui = pxui::doc::frames(2, &mut |ui| { title = Some(ui.label_with("title", "Library", &text_style::TITLE)); });
    /// assert!(ui.rect(&title.unwrap()).h > 0.0);
    /// ```
    #[inline]
    #[track_caller]
    pub fn label_with<K: Into<Key>>(&mut self, key: K, text: &str, style: &TextStyle) -> PxuiBox { self.label_body(key.into(), text, style) }

    /// Declares a keyed node showing formatted text in `style`, formatting into a 128-byte stack buffer as `label_fmt` does.
    ///
    /// Text past 128 bytes is dropped at a UTF-8 character boundary.
    /// Allocates: never on a warm frame.
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`).
    /// Panics: when the library refuses the node or the text (outside a frame).
    ///
    /// ```
    /// use pxui::text_style;
    /// let mut count = None;
    /// let ui = pxui::doc::frames(2, &mut |ui| { count = Some(ui.label_fmt_with(7u64, format_args!("{} songs", 10_000), &text_style::MONO)); });
    /// assert!(ui.rect(&count.unwrap()).h > 0.0);
    /// ```
    #[inline]
    #[track_caller]
    pub fn label_fmt_with<K: Into<Key>>(&mut self, key: K, args: Arguments, style: &TextStyle) -> PxuiBox { self.label_body(key.into(), Stack::new(args).as_str(), style) }

    /// The one body of `label_with` and `label_fmt_with`.
    #[track_caller]
    fn label_body(&mut self, key: Key, text: &str, style: &TextStyle) -> PxuiBox {
        let mut id_label = [0u8; 17];
        let label = match key { Key::Label(label) => label, Key::Id(id) => id_text(id, &mut id_label) };
        let b = self.node(label, Node::DEFAULT);
        let result = self.text_spans(&b, text, style, &[]);
        crate::check_error(self, result);
        b
    }
}

#[cfg(test)]
mod tests {
    use super::{text_style, Stack};
    use crate::FrameIn;

    /// 100 frames of `label_fmt` and `label_fmt_with` with a repeating value allocate nothing once warm.
    #[test]
    fn label_fmt_allocates_nothing_on_warm_frames() {
        mem_sys::set_peer(crate::library_counter);
        let mut ui = crate::Ui::new().unwrap();
        let input = FrameIn { width: 320.0, height: 240.0, ..FrameIn::default() };
        let mut build = |ui: &mut crate::Ui| {
            ui.label_fmt(format_args!("{} songs", 10_000));
            ui.label_fmt_with("count", format_args!("{} songs", 10_000), &text_style::MONO);
        };
        for _ in 0..8 { ui.frame(&input, &mut build).unwrap(); }
        let scope = mem_sys::warm_scope();
        for _ in 0..100 {
            mem_sys::local_reset();
            ui.frame(&input, &mut build).unwrap();
            assert_eq!(mem_sys::local_alloc_count(), 0);
        }
        drop(scope);
        assert!(mem_sys::take_reports().is_empty());
    }

    /// A 200-byte string of 3-byte characters keeps 126 bytes: 42 whole characters, never half of the 43rd.
    #[test]
    fn long_text_truncates_at_a_character_boundary() {
        let long = "\u{20AC}".repeat(66); // 198 bytes
        let s = Stack::new(format_args!("{long}"));
        assert_eq!(s.as_str(), "\u{20AC}".repeat(42));
        let mixed = format!("{}\u{20AC}{}", "a".repeat(127), "b".repeat(70));
        assert_eq!(Stack::new(format_args!("{mixed}")).as_str(), "a".repeat(127));
        let exact = "x".repeat(128);
        assert_eq!(Stack::new(format_args!("{exact}")).as_str(), exact);
    }
}
