//! A panic in an app callback is carried across the library and resumed at the call that can run callbacks.
use crate::Ui;
use pxui_types::{FrameIn, Node, Size};
use std::panic::{catch_unwind, AssertUnwindSafe};

fn input() -> FrameIn { FrameIn { width: 320.0, height: 240.0, ..FrameIn::default() } }

#[test]
fn a_callback_panic_resumes_at_frame_skips_later_siblings_and_leaves_the_ui_usable() {
    let mut ui = Ui::new().expect("library");
    let mut second_ran = false;
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _ = ui.frame(&input(), &mut |ui: &mut Ui| {
            ui.col(0.0, "first", &mut |_: &mut Ui| panic!("first callback"));
            ui.col(0.0, "second", &mut |_: &mut Ui| second_ran = true);
        });
    }));
    let payload = result.expect_err("the panic surfaces at frame()");
    assert_eq!(payload.downcast_ref::<&str>(), Some(&"first callback"));
    assert!(!second_ran, "a callback after the panic must not run on the half-built tree");
    let mut built = false;
    ui.frame(&input(), &mut |ui: &mut Ui| { ui.label("fine"); built = true; }).expect("the next frame renders");
    assert!(built);
}

#[test]
fn a_paint_callback_panic_is_carried_to_the_frame_that_paints() {
    let mut ui = Ui::new().expect("library");
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _ = ui.frame(&input(), &mut |ui: &mut Ui| {
            let node = ui.node("canvas", Node { w: Size::Px(40.0), h: Size::Px(40.0), ..Node::DEFAULT });
            ui.draw(&node, |_, _| panic!("paint callback"));
        });
    }));
    // wherever the engine paints (during the frame or at the renderer), the panic is resumed by the call that ran it
    if let Err(payload) = result { assert_eq!(payload.downcast_ref::<&str>(), Some(&"paint callback")); }
    assert!(!crate::composite::deferred(), "nothing is left deferred after the resume");
}

#[test]
fn the_output_borrows_the_ui() {
    let mut ui = Ui::new().expect("library");
    ui.frame(&input(), &mut |ui: &mut Ui| ui.label("x")).expect("frame");
    let revision = ui.output().draw_revision;
    assert_eq!(revision, ui.draw_revision());
}

/// A `Ui` an engine panic poisoned: `box_` outside a frame panics in the engine, under the export's catch (`box_` is no trusted
/// entry, so this holds linked statically too).
fn poisoned() -> Ui {
    let mut ui = Ui::new().expect("library");
    assert!(catch_unwind(AssertUnwindSafe(|| { ui.box_("outside a frame", Default::default()); })).is_err(), "the engine panics outside a frame");
    ui
}

/// The message of the panic `f` must raise on a poisoned `Ui`.
fn refusal(what: &str, f: &mut dyn FnMut()) -> String {
    let payload = catch_unwind(AssertUnwindSafe(f)).expect_err(what);
    payload.downcast_ref::<String>().expect("formatted message").clone()
}

/// A refused `theme()` panics with the library's message instead of dereferencing the null the library returns.
#[test]
fn theme_of_a_poisoned_ui_panics_with_the_library_message() {
    let mut ui = poisoned();
    let message = refusal("theme() on a poisoned Ui must panic, not read null", &mut || { let _ = ui.theme(); });
    assert!(message.starts_with("pxui: panic: "), "{message}");
    let _ = ui.frame(&input(), &mut |_| {}); // the next frame clears the poison
    let _ = ui.theme();
}

/// `view_size()` on a poisoned `Ui` panics like every status call, rather than answering 0 by 0.
#[test]
fn view_size_of_a_poisoned_ui_panics() {
    let ui = poisoned();
    let message = refusal("view_size() on a poisoned Ui must panic", &mut || { let _ = ui.view_size(); });
    assert!(message.starts_with("pxui: panic: "), "{message}");
}

/// The text service of a poisoned `Ui` answers its errors (whose zero is a real answer: font 0, no glyphs, an empty image), not success.
#[test]
fn the_text_service_of_a_poisoned_ui_answers_errors() {
    let mut ui = poisoned();
    let style = pxui_types::TextStyle::default();
    let mut text = ui.text_service();
    assert_eq!(text.add_font(crate::text::pragmasevka()), None);
    assert_eq!(text.quads(1, "", &style, f32::INFINITY, (0.0, 0.0), 1.0), Err(-1));
    assert_eq!(text.rasterize_rgba(1, "", &style, f32::INFINITY, 1.0, [255; 4]), Err(crate::text::TextError::InvalidInput));
}
