//! The engine-shaped surface through the wrapper: queries, boxes, patches, scroll and text state, carries, view calls
//! and the loud failure of a caller bug.
use crate::Ui;
use crate::opts::BoxOpts;
use pxui_types::{geom::Rect, ui::{Cursor, Drop, UndoKind, CLICKABLE, CLIP, SCROLL}, FrameIn, Mark, Size};
use std::panic::{catch_unwind, AssertUnwindSafe};

fn input() -> FrameIn { FrameIn { width: 320.0, height: 240.0, ..FrameIn::default() } }

#[test]
fn backdrop_saturation_validates_and_updates_without_warm_allocations() {
    use std::cell::Cell;
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new().unwrap();
    let view = Cell::new(None);
    let mut build = |ui: &mut Ui| {
        view.set(Some(ui.view("glass", 1, Size::Px(120.0), Size::Px(40.0), &mut |ui| {
            ui.node("pane", crate::Node { w: Size::Px(120.0), h: Size::Px(40.0), look: crate::ui::Look { fill: pxui_types::geom::color::white(), ..crate::ui::Look::NONE }, ..crate::Node::DEFAULT });
        })));
    };
    ui.frame(&input(), &mut build).unwrap();
    let view = view.get().unwrap();
    ui.view_backdrop_blur(&view, 8.0);
    for time in [1.0, 2.0] { ui.frame(&FrameIn { time, ..input() }, &mut build).unwrap(); }
    for saturation in [0.0, 0.9, 1.0, 1.5] {
        mem_sys::local_reset();
        ui.view_backdrop_saturation(&view, saturation);
        ui.frame(&FrameIn { time: 3.0, ..input() }, &mut build).unwrap();
        assert_eq!(mem_sys::local_alloc_count(), 0, "warm backdrop saturation update");
    }
    for invalid in [-1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(catch_unwind(AssertUnwindSafe(|| ui.view_backdrop_saturation(&view, invalid))).is_err());
        assert!(ui.last_error().contains("saturation"));
    }
}

#[test]
fn scale_animation_retargets_keyframes_and_accessibility_allocate_nothing_when_warm() {
    use crate::{kit::{AnimationTarget, Keyframe}, ui::{Curve, Look, Node, Role, Target}};
    use std::cell::Cell;
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new().unwrap();
    ui.set_a11y(true);
    let (child, view, builds) = (Cell::new(None), Cell::new(None), Cell::new(0));
    let mut build = |ui: &mut Ui| {
        let v = ui.view("scale", 1, Size::Px(100.0), Size::Px(80.0), &mut |ui| {
            builds.set(builds.get() + 1);
            let b = ui.node("child", Node { w: Size::Px(20.0), h: Size::Px(10.0), role: Role::Button, at: Some((20.0, 10.0)).into(), look: Look { fill: pxui_types::geom::color::rgb(9, 9, 9), ..Look::NONE }, ..Node::DEFAULT });
            ui.a11y_name(&b, "Scaled");
            child.set(Some(b));
        });
        ui.set_at(&v, (100.0, 40.0));
        view.set(Some(v));
    };
    for time in [0.0, 0.1] { ui.frame(&FrameIn { time, ..input() }, &mut build).unwrap(); }
    let (view, child, built) = (view.get().unwrap(), child.get().unwrap(), builds.get());
    let instances = ui.layer(0).instances.to_vec();
    let mut positions = [0.0; 16]; positions[1] = 0.5; positions[2] = 1.0;
    let mut values = positions; values[1] = 1.1;
    let spring = Curve::Table { duration: 0.2, count: 3, positions, values };
    let linear = Curve::Bezier { duration: 0.2, points: [0.0, 0.0, 1.0, 1.0] };
    let instant = Curve::Steps { duration: 0.0, count: 1 };
    let large = Target::Scale { x: 2.0, y: 0.5, anchor: (1.0, 1.0) };
    let normal = Target::Scale { x: 1.0, y: 1.0, anchor: (1.0, 1.0) };
    let shown = |ui: &Ui, expected: Rect| {
        let near = |r: Rect| [r.x - expected.x, r.y - expected.y, r.w - expected.w, r.h - expected.h].iter().all(|d| d.abs() < 0.005);
        assert!(near(ui.rect(&child)), "shown {:?}, expected {expected:?}", ui.rect(&child));
        let node = ui.a11y_nodes().iter().find(|n| ui.a11y_name_of(n) == "Scaled").unwrap();
        assert!(near(node.rect), "accessible {:?}, expected {expected:?}", node.rect);
    };
    for round in 0..3 {
        let base = 0.1 + round as f64 * 2.0;
        mem_sys::local_reset();
        assert!(ui.animate(&view, normal, instant));
        ui.frame(&FrameIn { time: base, ..input() }, &mut build).unwrap();
        assert!(ui.animate(&view, large, spring));
        ui.frame(&FrameIn { time: base + 0.1, ..input() }, &mut build).unwrap();
        shown(&ui, Rect::new(32.0, 88.5, 42.0, 4.5));
        let before = ui.rect(&child);
        assert!(ui.animate(&view, normal, linear));
        ui.frame(&FrameIn { time: base + 0.1, ..input() }, &mut build).unwrap();
        assert_eq!(ui.rect(&child), before, "retargeting cannot jump");
        ui.frame(&FrameIn { time: base + 0.2, ..input() }, &mut build).unwrap();
        shown(&ui, Rect::new(76.0, 69.25, 31.0, 7.25));
        ui.frame(&FrameIn { time: base + 0.3, ..input() }, &mut build).unwrap();
        shown(&ui, Rect::new(120.0, 50.0, 20.0, 10.0));
        let keys = [
            Keyframe { time: 0.2, target: AnimationTarget::Scale { x: 2.0, y: 0.5, anchor: (1.0, 1.0) }, curve: spring.with_duration(99.0) },
            Keyframe { time: 0.4, target: AnimationTarget::Scale { x: 1.0, y: 1.0, anchor: (1.0, 1.0) }, curve: Curve::Steps { duration: 99.0, count: 2 } },
        ];
        assert!(ui.animate_keys(&view, &keys));
        for (dt, expected) in [(0.1, Rect::new(32.0, 88.5, 42.0, 4.5)), (0.3, Rect::new(80.0, 67.5, 30.0, 7.5)), (0.4, Rect::new(120.0, 50.0, 20.0, 10.0))] {
            ui.frame(&FrameIn { time: base + 0.3 + dt, ..input() }, &mut build).unwrap();
            shown(&ui, expected);
        }
        let allocations = mem_sys::local_alloc_count();
        assert!(round == 0 || allocations == 0, "warm scale animations allocated {allocations} times");
        assert_eq!(builds.get(), built);
        assert!(!ui.wants_frame(&[], base + 1.9));
        assert_eq!(ui.layer(0).instances, instances.as_slice(), "scale must only update the transform table");
    }
    ui.set_theme(&crate::ThemeSource { flags: crate::ThemeSource::REDUCED_MOTION, ..crate::ThemeSource::default() }).unwrap();
    assert!(ui.tokens().reduced_motion);
    ui.frame(&FrameIn { time: 7.0, ..input() }, &mut build).unwrap();
    assert!(ui.animate(&view, large, spring));
    ui.frame(&FrameIn { time: 7.0, ..input() }, &mut build).unwrap();
    shown(&ui, Rect::new(40.0, 85.0, 40.0, 5.0));
    assert!(!ui.wants_frame(&[], 7.1));
}

#[test]
fn sampled_and_stepped_animations_keep_overshoot_and_allocate_nothing_when_warm() {
    use crate::{kit::{AnimationTarget, Keyframe}, ui::{Curve, Look, Node, Target}};
    use std::cell::Cell;
    mem_sys::set_peer(crate::library_counter);
    let table = Curve::Table {
        duration: 0.4, count: 4,
        positions: [0.0, 0.25, 0.5, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        values: [0.0, 1.2, 0.9, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    };
    let instant = Curve::Steps { duration: 0.0, count: 1 };
    let steps = Curve::Steps { duration: 0.2, count: 2 };
    let chip = Cell::new(None);
    let builds = Cell::new(0);
    let mut ui = Ui::new().expect("library");
    let mut build = |ui: &mut Ui| {
        ui.view("animation", 1, Size::Px(250.0), Size::Px(50.0), &mut |ui| {
            builds.set(builds.get() + 1);
            chip.set(Some(ui.node("chip", Node { w: Size::Px(40.0), h: Size::Px(20.0), look: Look { fill: pxui_types::geom::color::rgb(200, 40, 40), ..Look::NONE }, ..Node::DEFAULT })));
        });
    };
    for time in [0.0, 0.1] { ui.frame(&FrameIn { time, ..input() }, &mut build).unwrap(); }
    let chip = chip.get().unwrap();
    let built = builds.get();
    let x = |ui: &Ui| ui.layer(0).instances.iter().find(|i| i.color[0] == 200).unwrap().rect[0];
    for round in 0..3 {
        let base = 0.1 + round as f64 * 2.0;
        mem_sys::local_reset();
        assert!(ui.animate(&chip, Target::Offset(0.0, 0.0), instant));
        ui.frame(&FrameIn { time: base, ..input() }, &mut build).unwrap();
        assert!(ui.animate(&chip, Target::Offset(100.0, 0.0), table));
        for (dt, expected) in [(0.1, 120.0), (0.2, 90.0), (0.3, 95.0), (0.4, 100.0)] {
            ui.frame(&FrameIn { time: base + dt, ..input() }, &mut build).unwrap();
            assert!((x(&ui) - expected).abs() < 0.001, "table {dt}: {}", x(&ui));
            assert_eq!(ui.repaint_at().is_some(), dt < 0.4);
        }
        assert!(ui.animate(&chip, Target::Offset(0.0, 0.0), steps));
        for (dt, expected) in [(0.02, 100.0), (0.1, 50.0), (0.15, 50.0), (0.2, 0.0)] {
            ui.frame(&FrameIn { time: base + 0.4 + dt, ..input() }, &mut build).unwrap();
            assert!((x(&ui) - expected).abs() < 0.001, "steps {dt}: {}", x(&ui));
        }
        let keys = [
            Keyframe { time: 0.4, target: AnimationTarget::Offset(100.0, 0.0), curve: table.with_duration(99.0) },
            Keyframe { time: 0.6, target: AnimationTarget::Offset(0.0, 0.0), curve: steps.with_duration(99.0) },
        ];
        assert!(ui.animate_keys(&chip, &keys));
        for (dt, expected) in [(0.1, 120.0), (0.2, 90.0), (0.4, 100.0), (0.5, 50.0), (0.6, 0.0)] {
            ui.frame(&FrameIn { time: base + 0.6 + dt, ..input() }, &mut build).unwrap();
            assert!((x(&ui) - expected).abs() < 0.001, "keys {dt}: {}", x(&ui));
        }
        let allocations = mem_sys::local_alloc_count();
        assert!(round == 0 || allocations == 0, "warm animation allocated {allocations} times");
        assert_eq!(builds.get(), built);
        assert!(!ui.wants_frame(&[], base + 1.9));
    }
}

#[test]
fn queries_boxes_patches_and_text_state_have_the_engine_shape() {
    let mut ui = Ui::new().expect("library");
    assert_eq!(ui.font_size(), 13);
    ui.set_font_size(14);
    assert_eq!((ui.font_size(), ui.row_height()), (14, 25));
    ui.set_font_size(13);
    assert_eq!(ui.text_top(None, 10.0, 24.0), 10.0 + ((24.0f32 - 13.0) / 2.0).floor());
    let mut seen = false;
    ui.frame(&input(), &mut |ui: &mut Ui| {
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
        ui.frame(&input(), &mut |ui: &mut Ui| {
            ui.panel(pxui_types::ui::PanelOpts::default(), "extras", &mut |ui: &mut Ui| {
                assert!(!ui.button_styled("Styled", true, false, false, Some(pxui_types::geom::Rgba { r: 9, g: 9, b: 9, a: 255 })));
                let head = ui.inspector_head_for_test(&actions);
                assert_eq!(head.renamed, "Title");
                ui.inspector_body(&mut |ui: &mut Ui| {
                    assert!(ui.inspector_section_view("section", round, true, None, "Section", &mut |ui: &mut Ui| { ui.label("inside"); }));
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
                let open = ui.popup(None, Some(100.0), true, false, (10.0, 10.0), 100.0, 40.0, "pop", &mut |ui: &mut Ui| { ran = true; ui.label("popped"); });
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
    ui.frame(&input(), &mut |ui: &mut Ui| {
        let outer = ui.tokens().accent;
        ui.themed(&theme, &mut |ui: &mut Ui| inside = Some(ui.tokens().accent));
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
    use crate::opts::ValueFieldOpts;
    use pxui_types::ui::TextEdit;
    use std::rc::Rc;
    let mut ui = Ui::new().expect("library");
    let opts = ValueFieldOpts {
        fraction: Some(0.5), left: true, kept_drag: true, trail: Some(("px".into(), pxui_types::geom::Rgba { r: 1, g: 2, b: 3, a: 255 })),
        slide: Some(Rc::new(|f| format!("{f:.2}"))), scrub: Some(Rc::new(|origin, dx, _| format!("{origin}{dx}"))), ..ValueFieldOpts::default()
    };
    for _ in 0..2 {
        ui.frame(&input(), &mut |ui: &mut Ui| {
            ui.panel(pxui_types::ui::PanelOpts::default(), "fields", &mut |ui: &mut Ui| {
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
        let _ = ui.frame(&input(), &mut |ui: &mut Ui| ui.remember(&TextEdit { text: "é".into(), caret: 1, anchor: 0 }, false));
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

#[test]
fn a_stale_field_patches_nothing_and_whole_source_files_round_trip() {
    use crate::opts::ValueFieldOpts;
    use pxui_types::ui::TextEdit;
    let mut ui = Ui::new().expect("library");
    let stale = pxui_types::PxuiBox { index: 0, box_key: 1, box_slot: 99, generation: 7 };
    // the engine answers a stale handle with false, so does the one export that replaced the wrapper's several calls
    assert!(!ui.patch_field(&stale, &ValueFieldOpts::default(), "1.5"));
    assert!(!ui.patch_text(&stale, "x"));
    // text far over the old 1 MiB limit crosses whole in both directions, and a refused call is a loud one
    let source = "(graph g)\n".repeat(300_000);
    assert!(source.len() > 1 << 21);
    ui.frame(&input(), &mut |ui: &mut Ui| {
        let edit: TextEdit = ui.load_text_edit(9, &source);
        assert_eq!(edit.text.len(), source.len(), "the edit holds the whole text");
        let (text, _) = ui.text_area_submit((0.0, 0.0), 100.0, 100.0, &Default::default(), "area", &source);
        assert_eq!(text.len(), source.len(), "the area returns the whole text");
    }).expect("frame");
}


#[test]
fn tabular_digits_reach_shaping_and_keep_warm_layouts_allocation_free() {
    const FONT: &[u8]=include_bytes!("../../../../assets/fonts/Inter-Regular.ttf");
    mem_sys::set_peer(crate::library_counter);
    let mut text=crate::Text::new(&[FONT]).expect("shaper");
    let plain=pxui_types::TextStyle { size_points:20.0,..Default::default() };
    let tabular=pxui_types::TextStyle { features:pxui_types::TextStyle::FEATURE_TABULAR_NUMBERS,..plain };
    let proportional=text.measure(1,"1111",&plain,0.0).unwrap();
    let tabular_one=text.measure(1,"1111",&tabular,0.0).expect("tabular feature is accepted");
    let tabular_eight=text.measure(2,"8888",&tabular,0.0).unwrap();
    assert!((proportional.width-tabular_one.width).abs()>1.0,"font fixture must exercise a real substitution");
    assert!((tabular_one.width-tabular_eight.width).abs()<1e-5,"equal digit counts have equal advances");
    assert_eq!(text.measure(1,"1111",&plain,0.0).unwrap(),proportional,"feature changes invalidate the caller-keyed cache");
    text.measure(1,"1111",&tabular,0.0).unwrap();
    for _ in 0..4 {
        mem_sys::local_reset();
        assert_eq!(text.measure(1,"1111",&tabular,0.0).unwrap(),tabular_one);
        assert_eq!(text.measure(2,"8888",&tabular,0.0).unwrap(),tabular_eight);
        assert_eq!(mem_sys::local_alloc_count(),0,"warm feature-bearing layouts allocate nothing across the library boundary");
    }
}


#[test]
fn canvas_feature_styles_have_distinct_warm_cache_entries() {
    use pxui_types::TextStyle;
    const FONT: &[u8]=include_bytes!("../../../../assets/fonts/Inter-Regular.ttf");
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::create(crate::Platform::Apple,None,&[FONT]);
    let plain=TextStyle { size_points:20.0,..Default::default() };
    let tabular=TextStyle { features:TextStyle::FEATURE_TABULAR_NUMBERS,..plain };
    let a=ui.text_service().measure(1,"1111",&plain,0.0).unwrap();
    let b=ui.text_service().measure(2,"1111",&tabular,0.0).unwrap();
    assert!((a.width-b.width).abs()>1.0);
    for features in [2,3,u32::MAX] {
        assert_eq!(ui.text_service().measure(1,"1111",&TextStyle { features,..plain },0.0),Err(pxui_types::PXUI_ERR_ARGUMENT));
    }
    for frame in 0..7 {
        mem_sys::local_reset();
        ui.frame(&FrameIn { time:frame as f64/60.0,..input() },&mut |ui| {
            let bounds=ui.node("numbers",crate::Node { w:Size::Px(240.0),h:Size::Px(80.0),..crate::Node::DEFAULT });
            ui.canvas(&bounds,move |paint,rect| {
                assert_eq!(paint.text_styled((rect.x,rect.y),&plain,0.0,None,"1111"),a);
                assert_eq!(paint.text_styled((rect.x,rect.y+30.0),&tabular,0.0,None,"1111"),b);
            });
        }).unwrap();
        if frame>=3 { assert_eq!(mem_sys::local_alloc_count(),0,"mixed feature styles must keep separate warm layout/glyph caches"); }
    }
}


#[test]
fn inline_spans_on_nodes_preserve_labels_opacity_and_zero_allocation_rebuilds() {
    use crate::{TextSpan,TextStyle};
    use pxui_types::geom::color;
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new().unwrap();
    ui.set_a11y(true);
    let text="ab CD ef";
    let style=TextStyle { size_points:20.0,line_height:24.0,..Default::default() };
    let span=TextSpan { start:3,end:5,flags:TextSpan::COLOR|TextSpan::BACKGROUND|TextSpan::UNDERLINE,
        color:color::rgba(255,0,0,255),background:color::rgba(0,0,255,64),underline:color::rgba(0,255,0,255),..Default::default() };
    let mut shaper=crate::Text::new(&[crate::text::pragmasevka()]).unwrap();
    let measured=shaper.measure_spans(1,text,&style,200.0,&[span]).unwrap();
    assert_eq!(ui.text_service().measure_spans(1,text,&style,200.0,&[span]).unwrap(),measured);
    for frame in 0..10 {
        let plain=frame%3==0;
        let opacity=if frame%2==0 { 0.5 } else { 1.0 };
        mem_sys::local_reset();
        ui.frame(&FrameIn { time:frame as f64/60.0,..input() },&mut |ui| {
            let b=ui.node("label",crate::Node { role:crate::ui::Role::Label,w:Size::Px(200.0),h:Size::Px(40.0),look:crate::ui::Look { text:color::rgba(30,30,30,255),opacity,..crate::ui::Look::NONE },..crate::Node::DEFAULT });
            if plain { ui.text(&b,text); } else { ui.text_spans(&b,text,&style,&[span]).unwrap(); }
        }).unwrap();
        if frame>=6 { assert_eq!(mem_sys::local_alloc_count(),0,"warm rich/plain rebuilds retain their span storage and glyph caches"); }
        let alpha=if opacity==0.5 { 128 } else { 255 };
        let has_red=ui.layers().any(|l|l.instances.iter().any(|i|i.color==[255,0,0,alpha]));
        assert_eq!(has_red,!plain,"plain text clears previous span colours");
        if !plain {
            assert!(ui.layers().any(|l|l.instances.iter().any(|i|i.color==[0,255,0,alpha])));
            let background=if opacity==0.5 { 32 } else { 64 };
            assert!(ui.layers().any(|l|l.instances.iter().any(|i|i.color==[0,0,255,background])));
        }
        assert!(ui.a11y_nodes().iter().any(|node|ui.a11y_name_of(node)==text),"styled nodes expose the complete original label");
    }
}

#[test]
fn inline_canvas_styles_keep_independent_caches_and_reject_invalid_ranges() {
    use crate::{TextSpan,TextStyle};
    use pxui_types::geom::color;
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new().unwrap();
    let style=TextStyle { size_points:20.0,..Default::default() };
    let red=TextSpan { start:0,end:2,flags:TextSpan::COLOR|TextSpan::UNDERLINE,color:color::rgba(255,0,0,255),underline:color::rgba(255,0,0,255),..Default::default() };
    let green=TextSpan { color:color::rgba(0,255,0,255),underline:color::rgba(0,255,0,255),..red };
    for bad in [TextSpan { start:1,end:2,..red },TextSpan { end:99,..red },TextSpan { flags:8,..red }] {
        assert!(ui.text_service().measure_spans(1,"é!",&style,0.0,&[bad]).is_err());
    }
    assert!(ui.text_service().measure_spans(1,"abcdefghij",&style,0.0,&[red;9]).is_err());
    for frame in 0..7 {
        mem_sys::local_reset();
        ui.frame(&FrameIn { time:frame as f64/60.0,..input() },&mut |ui| {
            let b=ui.node("canvas",crate::Node { w:Size::Px(200.0),h:Size::Px(80.0),..crate::Node::DEFAULT });
            ui.canvas(&b,move |paint,rect| {
                let a=paint.text_spans((rect.x,rect.y),&style,0.0,None,"ab cd",&[red]).unwrap();
                let b=paint.text_spans((rect.x,rect.y+30.0),&style,0.0,None,"ab cd",&[green]).unwrap();
                assert_eq!(a,b,"decoration colour changes preserve text geometry");
            });
        }).unwrap();
        if frame>=3 { assert_eq!(mem_sys::local_alloc_count(),0,"warm mixed spans keep separate shape/glyph caches"); }
    }
}


#[test]
fn inline_node_fractional_font_alignment_matches_canvas_text() {
    use crate::{TextSpan,TextStyle};
    use pxui_types::geom::color;
    let mut ui=Ui::new().unwrap();
    let style=TextStyle { size_points:20.25,..Default::default() };
    let span=TextSpan { start:0,end:2,flags:TextSpan::COLOR,color:color::rgba(255,0,0,255),..Default::default() };
    ui.frame(&input(),&mut |ui| {
        let b=ui.node("fractional",crate::Node { w:Size::Px(200.0),h:Size::Px(40.0),look:crate::ui::Look { text:color::rgba(0,0,0,255),..crate::ui::Look::NONE },..crate::Node::DEFAULT });
        ui.text_spans(&b,"ab cd",&style,&[span]).unwrap();
        ui.canvas(&b,move |paint,rect| {
            let top=rect.y+((rect.h-style.size_points)/2.0).floor();
            paint.text_spans((rect.x,top),&style,200.0,Some(color::rgba(0,0,0,255)),"ab cd",&[span]).unwrap();
        });
    }).unwrap();
    let instances:Vec<_>=ui.layers().flat_map(|layer|layer.instances.iter().copied()).collect();
    assert_eq!(instances.len(),8);
    for (node,canvas) in instances[..4].iter().zip(&instances[4..]) {
        assert_eq!(node.rect,canvas.rect,"fractional font points must not be rounded before node alignment");
        assert_eq!(node.aux,canvas.aux);
        assert_eq!(node.color,canvas.color);
    }
}


#[test]
fn inline_node_wrapped_alignment_matches_the_complete_canvas_block() {
    use crate::{TextSpan,TextStyle};
    use pxui_types::geom::color;
    let mut ui=Ui::new().unwrap();
    let style=TextStyle { size_points:20.0,line_height:24.0,wrap:TextStyle::WRAP_WORD,..Default::default() };
    let span=TextSpan { start:3,end:5,flags:TextSpan::COLOR,color:color::rgba(255,0,0,255),..Default::default() };
    let metrics=ui.text_service().measure_spans(1,"aa bb cc",&style,45.0,&[span]).unwrap();
    assert_eq!(metrics.lines,3);
    ui.frame(&input(),&mut |ui| {
        let b=ui.node("wrapped",crate::Node { w:Size::Px(45.0),h:Size::Px(80.0),look:crate::ui::Look { text:color::rgba(0,0,0,255),..crate::ui::Look::NONE },..crate::Node::DEFAULT });
        ui.text_spans(&b,"aa bb cc",&style,&[span]).unwrap();
        ui.canvas(&b,move |paint,rect| {
            let block=style.size_points+(metrics.lines-1) as f32*style.line_height;
            let top=rect.y+((rect.h-block)/2.0).floor();
            paint.text_spans((rect.x,top),&style,45.0,Some(color::rgba(0,0,0,255)),"aa bb cc",&[span]).unwrap();
        });
    }).unwrap();
    let instances:Vec<_>=ui.layers().flat_map(|layer|layer.instances.iter().copied()).collect();
    assert_eq!(instances.len(),12);
    for (node,canvas) in instances[..6].iter().zip(&instances[6..]) {
        assert_eq!(node.rect,canvas.rect,"node alignment includes every wrapped line");
        assert_eq!(node.aux,canvas.aux);
        assert_eq!(node.color,canvas.color);
    }
}
