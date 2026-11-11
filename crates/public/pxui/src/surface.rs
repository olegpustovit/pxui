//! The engine-shaped surface through the wrapper: queries, boxes, patches, scroll and text state, carries, view calls
//! and the loud failure of a caller bug.
use crate::Ui;
use pxui_types::{geom::Rect, ui::{BoxOpts, Cursor, Drop, UndoKind, CLICKABLE, CLIP, SCROLL}, FrameIn, Mark, Size};
use std::panic::{catch_unwind, AssertUnwindSafe};

fn input() -> FrameIn { FrameIn { width: 320.0, height: 240.0, ..FrameIn::default() } }

#[test]
fn queries_boxes_patches_and_text_state_have_the_engine_shape() {
    let mut ui = Ui::new().expect("library");
    assert_eq!(ui.font_size(), 13);
    ui.set_font_size(14);
    assert_eq!((ui.font_size(), ui.row_height()), (14, 25));
    ui.set_font_size(13);
    assert_eq!(ui.text_top(None, 10.0, 24.0), 10.0 + ((24.0f32 - 13.0) / 2.0).floor());
    let mut seen = false;
    ui.frame(&input(), |ui| {
        assert!(!ui.captured() && !ui.text_input_focused() && !ui.verifying());
        assert_eq!(ui.passed_undo(), None::<UndoKind>);
        let b = ui.box_("scroller", BoxOpts::default().flags(CLICKABLE | SCROLL | CLIP).w(Size::Px(100.0)).h(Size::Px(60.0)).at((10.0, 20.0)).scroll_step(12.0));
        ui.box_fmt(format_args!("{}-fmt", "fmt"), BoxOpts::default().w(Size::Px(10.0)).h(Size::Px(10.0)));
        assert_eq!(ui.scroll_offset(&b), 0.0);
        ui.set_scroll_offset(&b, 5.0);
        assert_eq!((ui.scroll_offset(&b), ui.scroll_position(&b)), (5.0, 5.0));
        assert_eq!(ui.text_state(&b), None);
        ui.set_text_state(&b, Some("kept".into()));
        assert_eq!(ui.text_state(&b).as_deref(), Some("kept"));
        assert_eq!(ui.cached_label(&b, 0, "label"), "label");
        ui.set_at(&b, (11.0, 21.0));
        let _ = ui.patch_rect(&b, Rect::new(0.0, 0.0, 1.0, 1.0));
        let _ = ui.patch_mark(&b, Mark::None);
        let _ = ui.local_rect(&b);
        assert!(!ui.hovered_exactly(&b) && !ui.hovered_within(&b));
        assert_eq!(ui.context_at(&b), None);
        assert_eq!(ui.drop_target(&b), None::<Drop>);
        assert!(ui.press_keys(&b).is_empty());
        assert_eq!(ui.root_at((1.0, 1.0), &[]), None);
        ui.carry(None, "kind", "value");
        let payload = ui.carrying().expect("a key carry is held at once");
        assert_eq!((payload.kind.as_str(), payload.value.as_str()), ("kind", "value"));
        ui.cancel_carry();
        assert_eq!(ui.carrying(), None);
        ui.request_cursor(Cursor::Text);
        ui.repaint_after(0.5);
        seen = true;
    }).expect("frame");
    assert!(seen);
    assert_eq!(ui.frame_index(), 1);
    assert_eq!(ui.expanded("nothing"), None);
    assert!(!ui.hover_delay("x"));
    assert!(ui.bytes().atlas >= 0 && ui.phase_times().len() == 7);
    assert_eq!(ui.rebuilt_named("no such view"), 0);
}

#[test]
fn a_caller_bug_panics_with_the_librarys_message_at_the_call() {
    let mut ui = Ui::new().expect("library");
    let stale = pxui_types::PxuiBox { index: 0, box_key: 1, box_slot: 99, generation: 7 };
    let err = catch_unwind(AssertUnwindSafe(|| { let _ = ui.scroll_offset(&stale); })).expect_err("a stale box is refused");
    let message = err.downcast_ref::<String>().expect("a formatted message");
    assert!(message.contains("stale box"), "{message}");
    let err = catch_unwind(AssertUnwindSafe(|| ui.set_font_size(0))).expect_err("a font size of zero is refused");
    assert!(err.downcast_ref::<String>().is_some_and(|m| m.contains("font size")));
    assert_eq!(ui.font_size(), 13, "the refused call changed nothing");
}

#[test]
fn kit_extras_build_and_report_through_the_engine_shapes() {
    use pxui_types::{theme::Curve, ui::{HeadAction, LineStep, Target}, Axis};
    let mut ui = Ui::new().expect("library");
    let hints = [("Esc".to_string(), "close".to_string())];
    let actions = [HeadAction { caption: "Run".into(), keycap: "R".into(), active: false, usable: true }];
    for round in 0..3 {
        ui.frame(&input(), |ui| {
            ui.panel("extras", |ui| {
                assert!(!ui.button_styled("Styled", true, false, false, Some(pxui_types::geom::Rgba { r: 9, g: 9, b: 9, a: 255 })));
                let head = ui.inspector_head_for_test(&actions);
                assert_eq!(head.renamed, "Title");
                ui.inspector_body(|ui| {
                    assert!(ui.inspector_section_view("section", round, true, None, "Section", |ui| { ui.label("inside"); }));
                    assert!(!ui.inspector_toggle_value("toggle", (0.0, 0.0), false));
                });
                ui.inspector_bar(&hints, 3);
                let (text, step) = ui.line_field((0.0, 0.0), 120.0, 20.0, false, "find", "line", "abc");
                assert_eq!((text.as_str(), step), ("abc", LineStep::None));
                let (grip, signal) = ui.grip(format_args!("grip-{}", 1), Axis::Row, None);
                assert!(!signal.clicked);
                assert_eq!(ui.field_drag(&grip), None);
                if round == 0 { assert!(ui.animate(&grip, Target::Opacity(0.5), Curve::Spring { omega: 30.0 }), "a new target starts moving"); }
                let mut ran = false;
                let open = ui.popup(None, Some(100.0), true, false, (10.0, 10.0), 100.0, 40.0, "pop", |ui| { ran = true; ui.label("popped"); });
                assert_eq!(open, ran, "the build runs exactly while the popup is open");
            });
        }).expect("frame");
    }
}

impl Ui {
    fn inspector_head_for_test(&mut self, actions: &[pxui_types::ui::HeadAction]) -> pxui_types::ui::Head {
        self.inspector_header("head", Some("kind"), None, Some("1"), Some(std::rc::Rc::new(|name: &str| !name.is_empty())), actions, Some("reset"), true, "Title", "detail")
    }
}

#[test]
fn a_theme_built_from_key_values_scopes_over_a_subtree_and_refuses_bad_keys() {
    use crate::Theme;
    use pxui_types::theme::default;
    let mut theme = Theme::from_tokens(default());
    theme.set("tokens.accent", "#ff0000").expect("a token");
    theme.set("button.hover.fill", "#00ff00").expect("a look key");
    let err = theme.set("button.nonsense", "1").expect_err("an unknown key is refused");
    assert!(err.contains("unknown key"), "{err}");
    assert!(theme.set("tokens.accent", "red").is_err(), "a malformed value is refused");
    assert!(theme.load("/no/such/file.theme").is_err());
    let mut ui = Ui::new().expect("library");
    let mut inside = None;
    ui.frame(&input(), |ui| {
        let outer = ui.tokens().accent;
        ui.themed(&theme, |ui| inside = Some(ui.tokens().accent));
        assert_eq!(ui.tokens().accent, outer, "the Ui's own theme is back");
    }).expect("frame");
    assert_eq!(inside, Some(pxui_types::geom::Rgba { r: 255, g: 0, b: 0, a: 255 }));
}

#[test]
fn canvas_text_goes_through_the_uis_own_text_service() {
    let mut ui = Ui::new().expect("library");
    let style = pxui_types::TextStyle::default();
    let mut text = ui.text_service();
    let metrics = text.measure(1, "abc", &style, 0.0).expect("measure");
    assert!(metrics.width > 0.0 && metrics.lines == 1);
    let glyphs = text.quads(1, "abc", &style, 0.0, (10.0, 20.0), 1.0).expect("quads");
    assert_eq!(glyphs.len(), 3, "one quad per glyph");
    assert!(glyphs.iter().all(|g| g.rect[0] >= 10.0));
    let (w, h, pixels) = text.rasterize_rgba(2, "abc", &style, 0.0, 1.0, [255, 255, 255, 255]).expect("rasterize");
    assert_eq!(pixels.len(), (w * h * 4) as usize);
    let mut bad = style;
    bad.size_points = f32::NAN;
    assert!(text.measure(3, "abc", &bad, 0.0).is_err(), "a NaN size is refused");
}

#[test]
fn value_fields_take_their_options_and_text_edits_are_kept() {
    use pxui_types::ui::{TextEdit, ValueFieldOpts};
    use std::rc::Rc;
    let mut ui = Ui::new().expect("library");
    let opts = ValueFieldOpts {
        fraction: Some(0.5), left: true, kept_drag: true, trail: Some(("px".into(), pxui_types::geom::Rgba { r: 1, g: 2, b: 3, a: 255 })),
        slide: Some(Rc::new(|f| format!("{f:.2}"))), scrub: Some(Rc::new(|origin, dx, _| format!("{origin}{dx}"))), ..ValueFieldOpts::default()
    };
    for _ in 0..2 {
        ui.frame(&input(), |ui| {
            ui.panel("fields", |ui| {
                let (text, editing) = ui.value_field((10.0, 10.0), 120.0, 20.0, &opts, &|t: &str| t.parse::<f32>().is_ok(), "amount", "0.25");
                assert_eq!((text.as_str(), editing), ("0.25", false));
                let edit = ui.load_text_edit(7, "abc");
                assert_eq!((edit.text.as_str(), edit.caret, edit.anchor), ("abc", 3, 3));
                ui.remember(&TextEdit { text: "abc".into(), caret: 1, anchor: 1 }, false);
                assert!(ui.text_undo_bytes() >= 3, "the snapshot is held");
            });
        }).expect("frame");
    }
    let err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = ui.frame(&input(), |ui| ui.remember(&TextEdit { text: "é".into(), caret: 1, anchor: 0 }, false));
    })).expect_err("a caret inside a character is a caller bug");
    assert!(err.downcast_ref::<String>().is_some_and(|m| m.contains("caret and anchor")));
}

#[test]
fn a_shaper_of_its_own_measures_and_rasterizes_off_the_ui() {
    let mut text = crate::Text::new(&[crate::text::pragmasevka()]).expect("a shaper");
    let style = pxui_types::TextStyle::default();
    let metrics = text.measure(1, "abc", &style, 0.0).expect("measure");
    assert!(metrics.width > 0.0);
    // the same text measures the same in the Ui's own service: one font, two homes
    let mut ui = Ui::new().expect("library");
    assert_eq!(ui.text_service().measure(1, "abc", &style, 0.0).expect("service").width, metrics.width);
    let (w, h, pixels) = text.rasterize_rgba(2, "abc", &style, 0.0, 1.0, [255, 255, 255, 255]).expect("rasterize");
    assert_eq!(pixels.len(), (w * h * 4) as usize);
    assert!(pixels.chunks(4).any(|p| p[3] != 0), "something was inked");
}
