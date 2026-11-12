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

/// A refused `theme()` (here a `Ui` an engine panic poisoned: `label` outside a frame) panics with the library's message
/// instead of dereferencing the null the library returns. Dynamic build only: a static build has no catch at the export
/// boundary, so the engine panic unwinds to the caller and nothing poisons the `Ui`.
#[cfg(not(feature = "static"))]
#[test]
fn theme_of_a_poisoned_ui_panics_with_the_library_message() {
    let mut ui = Ui::new().expect("library");
    assert!(catch_unwind(AssertUnwindSafe(|| { ui.label("outside a frame"); })).is_err(), "the engine panics outside a frame");
    let payload = catch_unwind(AssertUnwindSafe(|| { let _ = ui.theme(); })).expect_err("theme() on a poisoned Ui must panic, not read null");
    let message = payload.downcast_ref::<String>().expect("formatted message");
    assert!(message.starts_with("pxui: panic: "), "{message}");
    let _ = ui.frame(&input(), &mut |_| {}); // the next frame clears the poison
    let _ = ui.theme();
}
