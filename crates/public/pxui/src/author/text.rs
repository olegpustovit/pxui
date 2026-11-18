//! Text authoring: `ui.label`, `ui.label_fmt` and `ui.label_digits` show text in a `Label` style (a `Copy` value with const builders),
//! through the label widget body, formatting into a fixed stack buffer where they take `format_args!`.
//!
//! Labels are leaves without a key and return their node (a `Handle`), not a `Response`: they hold no state and take no input, so
//! there is nothing to report; the node is there for the rare tweak a style does not cover. Interactive calls take a key first and
//! return a `Response`.
use core::fmt::{Arguments, Write};
use pxui_types::Label;
use crate::{Handle, Ui};

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
    /// Shows `text` in `style`, a leaf node built through the theme's label body (an app-registered `label` body replaces it).
    ///
    /// A typographic style's node is `style`'s width (`Fit` unless a builder changed it) and as tall as the lines it shows (lines
    /// times the line height, 1.2 times the size); wrapping, line limit, ellipsis and alignment apply inside that width.
    /// `Label::DEFAULT` is the theme's kit label.
    ///
    /// Returns the label's node, valid for this frame, for the rare tweak a style does not cover: a fill (`Ui::set_look`), an accessible
    /// state or value (`Ui::set_role`, `Ui::a11y_value`), or other text laid into the same node (`Ui::text_line_box`). Most callers
    /// drop it. A registered label body builds inside this node.
    /// Allocates: never on a warm frame.
    /// Identity: no key; every label is the n-th `"\0label"` among its siblings. Labels hold no state, so moving one costs nothing.
    /// Panics: when the library refuses the label (outside a frame, a style with a non-finite or non-positive size, an unloaded font). Display text past 64 KiB is clipped at a character boundary, never refused.
    ///
    /// ```
    /// use pxui::{theme::ColorToken, Label, Node};
    /// let (paper, ink) = (pxui::geom::Rgba { r: 250, g: 250, b: 250, a: 255 }, pxui::geom::Rgba { r: 90, g: 90, b: 90, a: 255 });
    /// const DIM_INK: ColorToken = ColorToken(0);
    /// const ROW: Node = Node::row().h_px(44.0).gap(8.0).center();
    /// const TITLE: Label = Label::BODY.size(15.0).bold();
    /// const DIM: Label = Label::BODY.color(DIM_INK);
    /// const CLOCK: Label = Label::MONO;
    /// let (title, duration_text, index, total, m, s) = (String::from("Song"), String::from("3:07"), 4, 12, 3, 7);
    /// let mut ui = pxui::Ui::new(&pxui::UiDesc::DEFAULT).unwrap();
    /// let mut theme = pxui::ThemeHandle::default();
    /// theme.set_color(DIM_INK, if ui.tokens().is_dark() { paper } else { ink });
    /// ui.set_theme(&theme).unwrap();
    /// ui.set_a11y(true);
    /// for _ in 0..2 {
    ///     ui.frame(&pxui::FrameIn::new(320.0, 240.0), &mut |ui| {
    ///         ui.row("song", ROW, |ui| {
    ///             ui.label(&title, TITLE.grow());
    ///             ui.label(&duration_text, DIM);
    ///         });
    ///         ui.label_fmt(format_args!("{} of {}", index + 1, total), DIM);
    ///         ui.label_digits(format_args!("{m}:{s:02}"), CLOCK);
    ///         ui.label("Section", Label::DEFAULT);
    ///         let badge = ui.label("New", Label::CAPTION);
    ///         ui.set_look(&badge, pxui::ui::Look { fill: ink, ..pxui::ui::Look::NONE }, pxui::ui::State::default());
    ///     }).unwrap();
    /// }
    /// assert!(ui.a11y_nodes().iter().any(|n| ui.a11y_name_of(n) == "5 of 12"));
    /// ```
    #[inline]
    #[track_caller]
    pub fn label(&mut self, text: &str, style: Label) -> crate::Handle { self.label_body(text, &style, false) }

    /// Shows formatted text in `style` as `label` does (returning its node), formatting into a 128-byte stack buffer.
    ///
    /// Text past 128 bytes is dropped at a UTF-8 character boundary, so the label never holds a partial character.
    /// Allocates: never; a warm frame with a repeating value allocates nothing.
    /// Identity: as `label`.
    /// Panics: as `label`.
    ///
    /// ```
    /// let ui = pxui::doc::frames(2, &mut |ui| { ui.label_fmt(format_args!("{} songs", 10_000), pxui::Label::DEFAULT); });
    /// assert!(ui.a11y_nodes().iter().any(|n| ui.a11y_name_of(n) == "10000 songs"));
    /// ```
    #[inline]
    #[track_caller]
    pub fn label_fmt(&mut self, args: Arguments, style: Label) -> Handle { self.label_body(Stack::new(args).as_str(), &style, false) }

    /// Shows a number that changes every frame (a clock, a stopwatch, "1:23") in `style`: formatted into a 128-byte stack buffer
    /// and drawn one cached character layout at a time, digits on the advance of `0`, so no new text is shaped once its characters
    /// have been seen.
    ///
    /// The node (returned, as `label` returns its node) is as wide as the characters and one line tall; its accessible value is the text. There is no kerning between
    /// characters and no wrapping; `Label::DEFAULT` is the theme's label here too. A registered `label` body is given the style with
    /// `digits` set.
    /// Allocates: never when warm; the first use of a character in a style shapes it.
    /// Identity: as `label`.
    /// Panics: as `label`.
    ///
    /// ```
    /// // once every digit and the colon have been shown, a changing clock shapes nothing new
    /// let mut ui = pxui::doc::frames(1, &mut |ui| { ui.label_digits(format_args!("0123456789:"), pxui::Label::MONO); });
    /// let layouts = ui.text_layout_count();
    /// ui.frame(&pxui::FrameIn::new(320.0, 240.0), &mut |ui| { ui.label_digits(format_args!("{}:{:02}", 3, 47), pxui::Label::MONO); }).unwrap();
    /// assert_eq!(ui.text_layout_count(), layouts, "no new layout for digits already shown");
    /// ```
    #[inline]
    #[track_caller]
    pub fn label_digits(&mut self, args: Arguments, style: Label) -> Handle { self.label_body(Stack::new(args).as_str(), &style, true) }

    /// The one body of `label`, `label_fmt` and `label_digits`, through the label body.
    #[inline]
    #[track_caller]
    fn label_body(&mut self, text: &str, style: &Label, digits: bool) -> Handle {
        self.label_call(text, style, digits)
    }
}

#[cfg(test)]
mod tests {
    use super::{Label, Stack};
    use crate::FrameIn;
    use pxui_types::{geom::Rgba, theme::ColorToken, Node, Size};
    use crate::bodies::BodyConfig;

    /// 100 frames of kit, typographic and digits labels with a repeating value allocate nothing once warm.
    #[test]
    fn label_fmt_allocates_nothing_on_warm_frames() {
        mem_sys::set_peer(crate::library_counter);
        let mut ui = crate::Ui::new(&crate::UiDesc::DEFAULT).unwrap();
        let input = FrameIn::new(320.0, 240.0);
        let mut build = |ui: &mut crate::Ui| {
            ui.label_fmt(format_args!("{} songs", 10_000), Label::DEFAULT);
            ui.label_fmt(format_args!("{} songs", 10_000), Label::MONO);
            ui.label("Songs", Label::TITLE.grow());
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

    /// Display text from user data past the 64 KiB cap is clipped at a character boundary by the wrapper, never a panic: a 100 KiB
    /// title renders with its first 65,535 bytes (the cap falls inside a two-byte character), and its warm frames allocate nothing.
    #[test]
    fn a_title_past_the_display_cap_is_clipped_not_refused() {
        mem_sys::set_peer(crate::library_counter);
        let mut title = String::from("a");
        while title.len() < 100 << 10 { title.push('é'); }
        let mut ui = crate::Ui::new(&crate::UiDesc::DEFAULT).unwrap();
        ui.set_a11y(true);
        let input = FrameIn::new(320.0, 240.0);
        let mut build = |ui: &mut crate::Ui| { ui.label(&title, Label::TITLE); };
        for _ in 0..8 { ui.frame(&input, &mut build).unwrap(); }
        assert!(ui.a11y_nodes().iter().any(|node| ui.a11y_name_of(node) == &title[..65_535]), "the clipped title is the label's name");
        let scope = mem_sys::warm_scope();
        for _ in 0..100 {
            mem_sys::local_reset();
            ui.frame(&input, &mut build).unwrap();
            assert_eq!(mem_sys::local_alloc_count(), 0);
        }
        drop(scope);
        assert!(mem_sys::take_reports().is_empty());
    }

    /// Frames `build` in a 160-point-wide `Ui` with red as colour token 0 and accessibility on, five times, and returns the `Ui` with the last frame's tree.
    fn settle(build: &mut dyn FnMut(&mut crate::Ui)) -> crate::Ui {
        let mut ui = crate::Ui::new(&crate::UiDesc::DEFAULT).unwrap();
        let mut theme = crate::ThemeHandle::default();
        theme.set_color(ColorToken(0), RED);
        ui.set_theme(&theme).unwrap();
        ui.set_a11y(true);
        for _ in 0..5 { ui.frame(&FrameIn::new(160.0, 240.0), build).unwrap(); }
        ui
    }

    const RED: Rgba = Rgba { r: 255, g: 0, b: 0, a: 255 };
    const LONG: &str = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma tau";

    /// `label` inside a column (`Fit` tall, `w` wide), returning the column's handle: its rectangle is the label's.
    fn boxed(ui: &mut crate::Ui, key: &'static str, w: Size, text: &str, style: Label) -> crate::Handle {
        ui.column(key, Node { w, ..Node::DEFAULT }, |ui| { ui.label(text, style); })
    }

    /// `kit::text`: one line, end ellipsis, height one line, however long the text; `settings::paragraph`: word wrap with
    /// the height of every wrapped line; `max_lines` clips the height at that many lines.
    #[test]
    fn labels_wrap_clip_and_ellipsize_to_their_lines() {
        let line = 14.0f32 * 1.2;
        let one = Label::BODY.grow().lines(1).ellipsis();
        let wrapped = Label::BODY.grow().wrap();
        let two = wrapped.lines(2).ellipsis();
        let (mut a, mut b, mut c, mut d) = (None, None, None, None);
        let ui = settle(&mut |ui| {
            a = Some(boxed(ui, "a", Size::Grow, LONG, one));
            b = Some(boxed(ui, "b", Size::Grow, LONG, wrapped));
            c = Some(boxed(ui, "c", Size::Grow, LONG, two));
            d = Some(boxed(ui, "d", Size::Grow, "short", one));
        });
        let (a, b, c, d) = (ui.rect(&a.unwrap()), ui.rect(&b.unwrap()), ui.rect(&c.unwrap()), ui.rect(&d.unwrap()));
        assert_eq!((a.w, a.h, d.h), (160.0, line, line));
        assert!(b.h >= 3.0 * line && (b.h / line).fract() < 1e-3, "word wrap grows the node: {b:?}");
        assert_eq!(c.h, 2.0 * line);
    }

    /// Settled wrapped, clipped and aligned labels, one static and one alternating between two texts, allocate nothing per frame.
    #[test]
    fn wrapped_labels_allocate_nothing_on_warm_frames() {
        mem_sys::set_peer(crate::library_counter);
        let mut ui = crate::Ui::new(&crate::UiDesc::DEFAULT).unwrap();
        let input = FrameIn::new(160.0, 240.0);
        let wrapped = Label::BODY.grow().wrap().lines(3).ellipsis().align(crate::TextAlign::Center);
        let frame = core::cell::Cell::new(0usize);
        let mut build = |ui: &mut crate::Ui| {
            ui.label(LONG, wrapped);
            ui.label(if frame.get() % 2 == 0 { LONG } else { "a shorter note" }, wrapped);
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
        let red = Label::BODY.grow().color(ColorToken(0));
        let ui = settle(&mut |ui| { ui.label(LONG, red.lines(1).ellipsis()); });
        let (lo, hi) = ink(&ui);
        assert!(lo >= 0.0 && hi <= 160.0 && hi > 120.0, "ellipsized line fills but never passes the width: {lo} {hi}");
        let ui = settle(&mut |ui| { ui.label(LONG, red.lines(1)); });
        assert!(ink(&ui).1 > 160.0, "without an ellipsis the line runs on");
        let mut edges = Vec::new();
        for style in [red, red.align(crate::TextAlign::Center), red.align(crate::TextAlign::End)] {
            let ui = settle(&mut |ui| { ui.label("hi", style); });
            edges.push(ink(&ui));
        }
        let mid = |(lo, hi): (f32, f32)| (lo + hi) / 2.0;
        assert!(edges[0].0 < 2.0 && (mid(edges[1]) - 80.0).abs() < 2.0 && edges[2].1 > 158.0, "{edges:?}");
    }

    /// `Fit` (the presets' width) takes the text's width, `w(Size::Px)` a fixed one and `grow` the parent's; a colour token is the
    /// text colour.
    #[test]
    fn label_width_and_colour_options_apply() {
        let (mut fit, mut fixed, mut grown) = (None, None, None);
        let ui = settle(&mut |ui| {
            fit = Some(ui.row("fit", Node { w: Size::Fit, ..Node::DEFAULT }, |ui| { ui.label("hi", Label::BODY.color(ColorToken(0))); }));
            fixed = Some(ui.row("fixed", Node { w: Size::Fit, ..Node::DEFAULT }, |ui| { ui.label("hi", Label::BODY.w(Size::Px(50.0))); }));
            grown = Some(ui.row("grown", Node::DEFAULT, |ui| { ui.label("hi", Label::BODY.grow()); }));
        });
        let (fit, fixed, grown) = (ui.rect(&fit.unwrap()), ui.rect(&fixed.unwrap()), ui.rect(&grown.unwrap()));
        assert!(fit.w > 0.0 && fit.w < 40.0, "{fit:?}");
        assert_eq!((fixed.w, grown.w), (50.0, 160.0));
        assert!(fixed.h > 0.0);
        assert!(ink(&ui).1 > ink(&ui).0, "the token's red is the text colour");
    }

    /// Two labels with one text under one parent are two nodes in build order, and each keeps its node when a label is added after them.
    #[test]
    fn labels_are_identified_by_position() {
        let (mut first, mut second) = (Vec::new(), Vec::new());
        let mut ui = crate::doc::frames(1, &mut |ui| { ui.label("same", Label::BODY); ui.label("same", Label::BODY); });
        for n in 0..2 {
            ui.frame(&FrameIn::new(160.0, 240.0), &mut |ui| {
                ui.label("same", Label::BODY);
                ui.label("same", Label::BODY);
                if n == 1 { ui.label("more", Label::BODY); }
            }).unwrap();
            let ids: Vec<u64> = ui.a11y_nodes().iter().filter(|a| ui.a11y_name_of(a) == "same").map(|a| a.id).collect();
            if n == 0 { first = ids; } else { second = ids; }
        }
        assert_eq!(first.len(), 2);
        assert_ne!(first[0], first[1]);
        assert_eq!(first, second);
    }

    /// A stopwatch (every frame a new value) and a 120 fps clock (frame counter as m:ss.ff) shape nothing and allocate nothing once warm.
    #[test]
    fn a_label_that_changes_every_frame_makes_no_layout_and_no_allocation() {
        mem_sys::set_peer(crate::library_counter);
        let mut ui = crate::Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        let mut layouts = 0;
        for n in 0..400u32 {
            let t = n * 7 + 11;
            let (a, b, c) = if n % 2 == 0 { (t / 6000, t / 100 % 60, t % 100) } else { (t / 120 / 60, t / 120 % 60, t % 120 * 100 / 120) };
            mem_sys::local_reset();
            ui.frame(&FrameIn::new(200.0, 100.0), &mut |ui: &mut crate::Ui| { ui.label_digits(format_args!("{a}:{b:02}.{c:02}"), Label::BODY); }).unwrap();
            if n == 100 { layouts = ui.text_layout_count(); }
            if n > 100 {
                assert_eq!(ui.text_layout_count(), layouts, "frame {n}");
                assert_eq!(mem_sys::local_alloc_count(), 0, "frame {n}");
            }
        }
    }

    /// A digits label is as wide as its characters with every digit on the advance of `0`, so "1:11" and "8:88" are one width.
    #[test]
    fn digits_take_one_width_whatever_the_digits() {
        let width = |value: u32| {
            let mut b = None;
            let ui = settle(&mut |ui| b = Some(ui.row("clock", Node { w: Size::Fit, ..Node::DEFAULT }, |ui| { ui.label_digits(format_args!("{}:{}", value, value * 11), Label::BODY); })));
            ui.rect(&b.unwrap()).w
        };
        assert!(width(1) > 0.0);
        assert_eq!(width(1), width(8));
    }

    thread_local! {
        static SEEN: core::cell::RefCell<Vec<(String, u32, u8, f32, u16)>> = const { core::cell::RefCell::new(Vec::new()) };
        static GROWN: core::cell::RefCell<Vec<(u32, f32, u8, u8)>> = const { core::cell::RefCell::new(Vec::new()) };
    }

    /// A label body that records its config and fills the label's node with one 30-point-tall child of its own, no text.
    fn boxed_label(ui: &mut crate::Ui, c: &crate::bodies::LabelConfig) {
        assert!(c.size as usize >= core::mem::size_of::<crate::bodies::LabelConfig>(), "this library carries the style and the box");
        SEEN.with(|s| s.borrow_mut().push((c.text_of(&c.text).to_owned(), c.style.set, c.style.digits, c.style.size_points, c.style.color)));
        GROWN.with(|g| g.borrow_mut().push((c.label_box.set, c.label_box.height, c.label_box.role, c.label_box.whole_pixels)));
        ui.node("mark".into(), Node { h: Size::Px(30.0), ..Node::DEFAULT });
    }

    /// With a registered label body every label goes through it with its style as given: the theme's label with nothing set, a
    /// preset with every field but the colour set (and the colour token when `color` sets it), a digits one with `digits`; what the
    /// body builds is what shows (no text of the built-in body).
    #[test]
    fn a_registered_label_body_builds_every_label() {
        let mut ui = crate::Ui::new(&crate::UiDesc::DEFAULT).unwrap();
        let id = ui.register_label_body("boxed", boxed_label).expect("registered");
        let mut theme = crate::ThemeHandle::parse("label.body = boxed").unwrap();
        theme.set_color(ColorToken(0), RED);
        ui.set_theme(&theme).unwrap();
        assert_eq!(ui.theme().body.label, id);
        let (mut kit, mut styled, mut digits, mut boxed) = (None, None, None, None);
        for _ in 0..2 {
            SEEN.with(|s| s.borrow_mut().clear());
            GROWN.with(|g| g.borrow_mut().clear());
            ui.frame(&FrameIn::new(160.0, 240.0), &mut |ui| {
                kit = Some(ui.column("kit", Node::DEFAULT, |ui| { ui.label("Kit", Label::DEFAULT); }));
                styled = Some(ui.column("styled", Node::DEFAULT, |ui| { ui.label("Styled", Label::TITLE.color(ColorToken(0))); }));
                digits = Some(ui.column("digits", Node::DEFAULT, |ui| { ui.label_digits(format_args!("{}:{:02}", 1, 5), Label::MONO); }));
                boxed = Some(ui.column("boxed", Node::DEFAULT, |ui| { ui.label("Boxed", Label::BODY.h_px(44.0).whole_pixels().role(crate::ui::Role::Paragraph)); }));
            }).unwrap();
        }
        let seen = SEEN.with(|s| s.borrow().clone());
        use pxui_types::ffi::PxuiLabelStyle as S;
        assert_eq!(seen, vec![("Kit".into(), 0, 0, 0.0, 0), ("Styled".into(), S::SET_ALL, 0, 20.0, 0), ("1:05".into(), S::SET_ALL & !S::SET_COLOR, 1, 14.0, 0),
            ("Boxed".into(), S::SET_ALL & !S::SET_COLOR, 0, 14.0, 0)]);
        let grown = GROWN.with(|g| g.borrow().clone());
        use pxui_types::ffi::PxuiLabelBox as B;
        assert_eq!(grown[3], (B::SET_ALL, 44.0, crate::ui::Role::Paragraph as u8, 1), "the body is given the box height, role and whole pixels");
        assert!(grown[..3].iter().all(|g| *g == (0, 0.0, 0, 0)));
        for b in [kit, styled, digits, boxed] { assert_eq!(ui.rect(&b.unwrap()).h, 30.0, "the body's node height"); }
        assert!(ui.layers().flat_map(|l| l.instances.iter()).all(|i| i.color != [255, 0, 0, 255]), "the built-in body drew no text");
    }

    /// `Label::DEFAULT` is the theme's label: a builder on it changes only its own field (bold keeps the theme's size, capitals and
    /// row), and theme labels are keyed by position like every label, so two with one text are two nodes.
    #[test]
    fn default_is_the_theme_label_and_builders_set_one_field() {
        let mut rows = Vec::new();
        let ui = settle(&mut |ui| {
            rows.clear();
            for (key, style) in [("plain", Label::DEFAULT), ("bold", Label::DEFAULT.bold()), ("again", Label::DEFAULT)] {
                rows.push(ui.column(key, Node::DEFAULT, |ui| { ui.node("before".into(), Node::DEFAULT); ui.label("Same", style); }));
            }
        });
        let rects: Vec<_> = rows.iter().map(|b| ui.rect(b)).collect();
        assert!(rects.iter().all(|r| r.h == 24.0), "one theme row each, no gap after a sibling: {rects:?}");
        let ids: Vec<u64> = ui.a11y_nodes().iter().filter(|a| ui.a11y_name_of(a) == "Same").map(|a| a.id).collect();
        assert_eq!(ids.len(), 3);
        assert!(ids[0] != ids[1] && ids[1] != ids[2] && ids[0] != ids[2], "{ids:?}");
    }

    /// SwiftUI-style text: `h` gives the node its box with the line centred in it, `whole_pixels` rounds its fitted width up to whole
    /// pixels, `role` is what accessibility hears (`Unknown` for a caption it skips), and `label` returns the node it built.
    #[test]
    fn box_height_whole_pixels_and_role_reach_the_node() {
        use crate::ui::Role;
        const HEADING: Label = Label::BODY.size(13.0).line_height(18.2).lines(1).ellipsis();
        let (mut boxed, mut plain, mut lined, mut hidden) = (None, None, None, None);
        let ui = settle(&mut |ui| {
            boxed = Some(ui.row("boxed", Node { w: Size::Fit, ..Node::DEFAULT }, |ui| { ui.label("Heading", HEADING.h_px(30.0).whole_pixels().role(Role::Paragraph)); }));
            plain = Some(ui.label("Heading", HEADING));
            lined = Some(ui.label("Heading", HEADING.line_height(30.0)));
            hidden = Some(ui.label("Caption", HEADING.role(Role::Unknown)));
        });
        let (boxed, plain, lined) = (ui.rect(&boxed.unwrap()), ui.rect(&plain.unwrap()), ui.rect(&lined.unwrap()));
        assert_eq!(boxed.h, 30.0, "the box height");
        assert!(plain.w.fract() != 0.0, "the fixture's width is fractional: {plain:?}");
        assert_eq!(boxed.w, plain.w.ceil(), "rounded up to whole pixels at 1x");
        assert_eq!(lined.h, 30.0);
        let heading = ui.a11y_nodes().iter().find(|n| ui.a11y_name_of(n) == "Heading").expect("the heading");
        assert!(heading.is_role(Role::Paragraph) && heading.rect.h == 30.0, "the role and the returned node: {heading:?}");
        let glyphs = |r: crate::geom::Rect| -> Vec<[f32; 4]> {
            ui.layers().flat_map(|l| l.instances.iter()).filter(|i| i.kind == pxui_types::draw::kind::MASK && i.rect[1] >= r.y && i.rect[3] <= r.y + r.h)
                .map(|i| [i.rect[0] - r.x, i.rect[1] - r.y, i.rect[2] - r.x, i.rect[3] - r.y]).collect()
        };
        let (in_box, in_line) = (glyphs(boxed), glyphs(lined));
        assert!(!in_box.is_empty() && in_box.len() == in_line.len(), "{in_box:?} {in_line:?}");
        for (a, b) in in_box.iter().zip(&in_line) { assert!(a[0] == b[0] && (a[1] - b[1]).abs() <= 1.0, "an 18.2 line centred in 30 sits where a 30 line does, within its floored half-leading and pixel snap: {a:?} {b:?}"); }
        assert!(hidden.is_some() && ui.a11y_nodes().iter().all(|n| ui.a11y_name_of(n) != "Caption"), "an Unknown label is skipped");
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
