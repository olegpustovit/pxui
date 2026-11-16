//! Text authoring: `ui.label_fmt` formats into a fixed stack buffer, `ui.label_with` and `ui.label_fmt_with` show text in a keyed node
//! with an explicit style and `LabelOpts`, and `text_style` names the usual styles.
use core::fmt::{Arguments, Write};
use pxui_types::{a11y::Role, geom::Rgba as Color, Look, Node, PxuiBox, Size, TextStyle};
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

/// What a label's `TextStyle` does not carry: the text colour and the node's width.
///
/// Wrapping (`wrap`), `max_lines`, `ellipsis`, `align` and `line_height` are `TextStyle` fields; the label's height follows from
/// them (lines shown times the line height). `LabelOpts::default()` is the theme's foreground at full width.
#[derive(Clone, Copy, Debug)]
pub struct LabelOpts {
    /// The text colour; `None` is the theme's foreground.
    pub color: Option<Color>,
    /// The node's width: `Size::Grow` (the default) fills the parent and wraps there, `Size::Fit` is the text's own width,
    /// `Size::Px` is fixed.
    pub width: Size,
}

impl Default for LabelOpts {
    fn default() -> Self { LabelOpts { color: None, width: Size::Grow } }
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

    /// Declares a node under `key` showing `text` in `style` and `opts`, and returns its handle.
    ///
    /// The node is `Node::DEFAULT` with `opts.width` and role `Label`; its height is the lines shown times
    /// `style.line_height` (`1.2` times the size when that is 0), so a wrapped or `max_lines`-limited label takes the room it
    /// needs. `style.wrap`, `style.max_lines`, `style.ellipsis` and `style.align` apply inside the node's width; text cut by
    /// `max_lines` or a width too small for a `WRAP_NONE` line ends in an ellipsis when `style.ellipsis` is on.
    /// Allocates: as `node` and `text_spans` (never on a warm frame).
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`).
    /// Panics: when the library refuses the node or the text (outside a frame), or `style` holds a non-finite value.
    ///
    /// ```
    /// use pxui::{text_style, LabelOpts, TextStyle};
    /// let style = TextStyle { wrap: TextStyle::WRAP_WORD, max_lines: 2, ellipsis: TextStyle::ELLIPSIS_END, ..text_style::BODY };
    /// let mut note = None;
    /// let ui = pxui::doc::frames(3, &mut |ui| {
    ///     note = Some(ui.label_with("note", "a long note that needs more than one line to show", &style, &LabelOpts::default()));
    /// });
    /// assert!(ui.rect(&note.unwrap()).h > 0.0);
    /// ```
    #[inline]
    #[track_caller]
    pub fn label_with<K: Into<Key>>(&mut self, key: K, text: &str, style: &TextStyle, opts: &LabelOpts) -> PxuiBox { self.label_body(key.into(), text, style, opts) }

    /// Declares a keyed node showing formatted text in `style`, formatting into a 128-byte stack buffer as `label_fmt` does.
    ///
    /// Text past 128 bytes is dropped at a UTF-8 character boundary.
    /// Allocates: never on a warm frame.
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`).
    /// Panics: when the library refuses the node or the text (outside a frame).
    ///
    /// ```
    /// use pxui::{text_style, LabelOpts};
    /// let mut count = None;
    /// let ui = pxui::doc::frames(2, &mut |ui| { count = Some(ui.label_fmt_with(7u64, format_args!("{} songs", 10_000), &text_style::MONO, &LabelOpts::default())); });
    /// assert!(ui.rect(&count.unwrap()).h > 0.0);
    /// ```
    #[inline]
    #[track_caller]
    pub fn label_fmt_with<K: Into<Key>>(&mut self, key: K, args: Arguments, style: &TextStyle, opts: &LabelOpts) -> PxuiBox { self.label_body(key.into(), Stack::new(args).as_str(), style, opts) }

    /// The one body of `label_with` and `label_fmt_with`.
    #[track_caller]
    fn label_body(&mut self, key: Key, text: &str, style: &TextStyle, opts: &LabelOpts) -> PxuiBox {
        let mut id_label = [0u8; 17];
        let label = match key { Key::Label(label) => label, Key::Id(id) => id_text(id, &mut id_label) };
        let color = opts.color.unwrap_or(self.tokens().foreground);
        let b = self.node(label, Node { w: opts.width, role: Role::Label, look: Look { text: color, ..Look::NONE }, ..Node::DEFAULT });
        // a line box needs a positive line height; the engine's own default is 1.2 times the size
        let style = TextStyle { line_height: if style.line_height > 0.0 { style.line_height } else { style.size_points * 1.2 }, ..*style };
        let result = self.text_line_box(&b, text, &style, &[]);
        crate::check_error(self, result);
        b
    }
}

#[cfg(test)]
mod tests {
    use super::{text_style, LabelOpts, Stack};
    use crate::{FrameIn, TextStyle};

    /// 100 frames of `label_fmt` and `label_fmt_with` with a repeating value allocate nothing once warm.
    #[test]
    fn label_fmt_allocates_nothing_on_warm_frames() {
        mem_sys::set_peer(crate::library_counter);
        let mut ui = crate::Ui::new().unwrap();
        let input = FrameIn { width: 320.0, height: 240.0, ..FrameIn::default() };
        let mut build = |ui: &mut crate::Ui| {
            ui.label_fmt(format_args!("{} songs", 10_000));
            ui.label_fmt_with("count", format_args!("{} songs", 10_000), &text_style::MONO, &LabelOpts::default());
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

    /// Frames `build` in a 160-point-wide `Ui`, five times, and returns the `Ui` with the last frame's tree.
    fn settle(build: &mut dyn FnMut(&mut crate::Ui)) -> crate::Ui {
        let mut ui = crate::Ui::new().unwrap();
        for _ in 0..5 { ui.frame(&FrameIn { width: 160.0, height: 240.0, ..FrameIn::default() }, build).unwrap(); }
        ui
    }

    const LONG: &str = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma tau";

    /// `kit::text`: one line, end ellipsis, height one line, however long the text; `settings::paragraph`: word wrap with
    /// the height of every wrapped line; `max_lines` clips the height at that many lines.
    #[test]
    fn labels_wrap_clip_and_ellipsize_to_their_lines() {
        let line = TextStyle { size_points: 14.0, line_height: 20.0, ..text_style::BODY };
        let one = TextStyle { max_lines: 1, ellipsis: TextStyle::ELLIPSIS_END, ..line };
        let wrapped = TextStyle { wrap: TextStyle::WRAP_WORD, ..line };
        let two = TextStyle { max_lines: 2, ellipsis: TextStyle::ELLIPSIS_END, ..wrapped };
        let opts = LabelOpts::default();
        let (mut a, mut b, mut c, mut d) = (None, None, None, None);
        let ui = settle(&mut |ui| {
            a = Some(ui.label_with("a", LONG, &one, &opts));
            b = Some(ui.label_with("b", LONG, &wrapped, &opts));
            c = Some(ui.label_with("c", LONG, &two, &opts));
            d = Some(ui.label_with("d", "short", &one, &opts));
        });
        let (a, b, c, d) = (ui.rect(&a.unwrap()), ui.rect(&b.unwrap()), ui.rect(&c.unwrap()), ui.rect(&d.unwrap()));
        assert_eq!((a.w, a.h, d.h), (160.0, 20.0, 20.0));
        assert!(b.h >= 60.0 && b.h % 20.0 == 0.0, "word wrap grows the node: {b:?}");
        assert_eq!(c.h, 40.0);
    }

    /// Settled wrapped, clipped and aligned labels, one static and one alternating between two texts, allocate nothing per frame.
    #[test]
    fn wrapped_labels_allocate_nothing_on_warm_frames() {
        mem_sys::set_peer(crate::library_counter);
        let mut ui = crate::Ui::new().unwrap();
        let input = FrameIn { width: 160.0, height: 240.0, ..FrameIn::default() };
        let wrapped = TextStyle { wrap: TextStyle::WRAP_WORD, max_lines: 3, ellipsis: TextStyle::ELLIPSIS_END, align: TextStyle::ALIGN_CENTER, ..text_style::BODY };
        let frame = core::cell::Cell::new(0usize);
        let mut build = |ui: &mut crate::Ui| {
            ui.label_with("static", LONG, &wrapped, &LabelOpts::default());
            ui.label_with("dynamic", if frame.get() % 2 == 0 { LONG } else { "a shorter note" }, &wrapped, &LabelOpts::default());
        };
        for _ in 0..8 { ui.frame(&input, &mut build).unwrap(); frame.set(frame.get() + 1); }
        let scope = mem_sys::warm_scope();
        for _ in 0..100 {
            mem_sys::local_reset();
            ui.frame(&input, &mut build).unwrap();
            frame.set(frame.get() + 1);
            assert_eq!(mem_sys::local_alloc_count(), 0);
        }
        drop(scope);
        assert!(mem_sys::take_reports().is_empty());
    }

    /// The horizontal ink extent of the red text a label drew, in points: its glyph quads' left and right edges.
    fn ink(ui: &crate::Ui) -> (f32, f32) {
        let red = [255, 0, 0, 255];
        ui.layers().flat_map(|l| l.instances.iter()).filter(|i| i.color == red && i.rect[2] > i.rect[0])
            .fold((f32::MAX, f32::MIN), |(lo, hi), i| (lo.min(i.rect[0]), hi.max(i.rect[2])))
    }

    /// `kit::text`'s end ellipsis keeps one line inside the width, `align` places a short line at the start, centre or end.
    #[test]
    fn ellipsis_keeps_ink_inside_the_width_and_align_places_it() {
        let red = Some(pxui_types::geom::Rgba { r: 255, g: 0, b: 0, a: 255 });
        let opts = LabelOpts { color: red, ..LabelOpts::default() };
        let cut = TextStyle { max_lines: 1, ellipsis: TextStyle::ELLIPSIS_END, ..text_style::BODY };
        let ui = settle(&mut |ui| { ui.label_with("cut", LONG, &cut, &opts); });
        let (lo, hi) = ink(&ui);
        assert!(lo >= 0.0 && hi <= 160.0 && hi > 120.0, "ellipsized line fills but never passes the width: {lo} {hi}");
        let ui = settle(&mut |ui| { ui.label_with("clip", LONG, &TextStyle { ellipsis: TextStyle::ELLIPSIS_NONE, max_lines: 1, ..text_style::BODY }, &opts); });
        assert!(ink(&ui).1 > 160.0, "without an ellipsis the line runs on");
        let mut edges = Vec::new();
        for align in [TextStyle::ALIGN_LEFT, TextStyle::ALIGN_CENTER, TextStyle::ALIGN_RIGHT] {
            let ui = settle(&mut |ui| { ui.label_with("hi", "hi", &TextStyle { align, ..text_style::BODY }, &opts); });
            edges.push(ink(&ui));
        }
        let mid = |(lo, hi): (f32, f32)| (lo + hi) / 2.0;
        assert!(edges[0].0 < 2.0 && (mid(edges[1]) - 80.0).abs() < 2.0 && edges[2].1 > 158.0, "{edges:?}");
    }

    /// `Size::Fit` takes the text's width and `Size::Px` a fixed one; a colour is the text colour.
    #[test]
    fn label_width_and_colour_options_apply() {
        let red = pxui_types::geom::Rgba { r: 255, g: 0, b: 0, a: 255 };
        let (mut fit, mut fixed) = (None, None);
        let ui = settle(&mut |ui| {
            fit = Some(ui.label_with("fit", "hi", &text_style::BODY, &LabelOpts { width: pxui_types::Size::Fit, color: Some(red) }));
            fixed = Some(ui.label_with("fixed", "hi", &text_style::BODY, &LabelOpts { width: pxui_types::Size::Px(50.0), ..LabelOpts::default() }));
        });
        let (fit, fixed) = (ui.rect(&fit.unwrap()), ui.rect(&fixed.unwrap()));
        assert!(fit.w > 0.0 && fit.w < 40.0, "{fit:?}");
        assert_eq!(fixed.w, 50.0);
        assert!(fixed.h > 0.0);
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
