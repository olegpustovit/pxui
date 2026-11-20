//! Checks of a box's state slots (`Ui::state` / `Ui::set_state` and the text slot): stale handles, frames and warm allocations.
use crate::Handle;
use crate::{FrameIn, Node};

fn frame(ui: &mut crate::Ui, f: &mut dyn FnMut(&mut crate::Ui)) {
    let input = FrameIn::new(320.0, 240.0);
    ui.frame(&input, f).expect("frame");
}

/// Values round-trip across frames, including the i64 extremes and an f32 stored as its bits; unset reads the default.
#[test]
fn round_trips() {
    let mut ui = crate::Ui::new(&crate::UiDesc::DEFAULT).expect("library");
    frame(&mut ui, &mut |ui| {
        let b = ui.node("n", Node::DEFAULT);
        assert_eq!(ui.state(&b, -1), -1);
        for v in [0, 1, -1, i64::MIN, i64::MAX, -123_456_789_012] { ui.set_state(&b, v); assert_eq!(ui.state(&b, 0), v); }
        ui.set_state(&b, 3.5f32.to_bits() as i64);
    });
    frame(&mut ui, &mut |ui| { let b = ui.node("n", Node::DEFAULT); assert_eq!(f32::from_bits(ui.state(&b, 0) as u32), 3.5); });
}

/// A handle the engine no longer resolves reads the default and a set stores nothing: no panic, and the `Ui` stays usable.
#[test]
fn stale_handle_reads_default_and_set_is_a_no_op() {
    let mut ui = crate::Ui::new(&crate::UiDesc::DEFAULT).expect("library");
    let stale = Handle { index: 9999, box_key: 77, box_slot: 9999, generation: 99, ui: 0 };
    frame(&mut ui, &mut |ui| {
        let b = ui.node("n", Node::DEFAULT);
        ui.set_state(&b, 5);
        assert_eq!(ui.state(&stale, 3), 3);
        ui.set_state(&stale, 1);
        assert_eq!(ui.state(&stale, 0), 0);
        assert_eq!(ui.state(&b, 0), 5, "a stale set must not reach a live node");
        ui.set_text_state(&b, Some("kept"));
        ui.set_text_state(&stale, Some("lost"));
        assert_eq!(ui.text_state(&stale), None, "the text slot follows the same rule");
        assert_eq!(ui.text_state(&b), Some("kept"));
    });
    frame(&mut ui, &mut |ui| { let b = ui.node("n", Node::DEFAULT); assert_eq!(ui.state(&b, 0), 5); });
}

/// Warm get/set calls allocate nothing.
#[test]
fn warm_calls_allocate_nothing() {
    let mut ui = crate::Ui::new(&crate::UiDesc::DEFAULT).expect("library");
    let run = |ui: &mut crate::Ui| frame(ui, &mut |ui| {
        let b = ui.node("n", Node::DEFAULT);
        ui.set_state(&b, -3);
        assert_eq!(ui.state(&b, 0), -3);
    });
    for _ in 0..4 { run(&mut ui); }
    mem_sys::local_reset();
    for _ in 0..4 { run(&mut ui); }
    assert_eq!(mem_sys::local_alloc_count(), 0);
}
