//! Typed views of a box's one state slot: bool, i64 and f32 over `Ui::state` / `Ui::set_state`, no generic.
use pxui_types::PxuiBox;
use crate::Ui;

impl Ui {
    /// Reads `box_`'s state slot as a bool (nonzero is true); a box never set reads false.
    ///
    /// The slot is shared with `state`, `set_state` and the other `local_*` getters: use one type per box.
    /// Allocates: never.
    /// Panics: when the library refuses the call (outside a frame, or a handle it no longer resolves: it does not read as false), like `state`.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let b = ui.node("n", pxui::Node::DEFAULT);
    ///     assert!(!ui.local_bool(&b));
    ///     ui.set_local_bool(&b, true);
    ///     assert!(ui.local_bool(&b));
    /// });
    /// ```
    #[inline]
    #[track_caller]
    pub fn local_bool(&self, box_: &PxuiBox) -> bool { self.state(box_, 0) != 0 }

    /// Stores `value` in `box_`'s state slot as a bool; it persists across frames while the box lives.
    ///
    /// Allocates: never.
    /// Panics: as `local_bool`.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let b = ui.node("n", pxui::Node::DEFAULT);
    ///     ui.set_local_bool(&b, true);
    ///     ui.set_local_bool(&b, false);
    ///     assert!(!ui.local_bool(&b));
    /// });
    /// ```
    #[inline]
    #[track_caller]
    pub fn set_local_bool(&mut self, box_: &PxuiBox, value: bool) { self.set_state(box_, value as i64) }

    /// Reads `box_`'s state slot as an i64; a box never set reads 0.
    ///
    /// Allocates: never.
    /// Panics: as `local_bool`.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let b = ui.node("n", pxui::Node::DEFAULT);
    ///     ui.set_local_i64(&b, -7);
    ///     assert_eq!(ui.local_i64(&b), -7);
    /// });
    /// ```
    #[inline]
    #[track_caller]
    pub fn local_i64(&self, box_: &PxuiBox) -> i64 { self.state(box_, 0) }

    /// Stores `value` in `box_`'s state slot; it persists across frames while the box lives.
    ///
    /// Allocates: never.
    /// Panics: as `local_bool`.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let b = ui.node("n", pxui::Node::DEFAULT);
    ///     ui.set_local_i64(&b, 42);
    ///     assert_eq!(ui.local_i64(&b), 42);
    /// });
    /// ```
    #[inline]
    #[track_caller]
    pub fn set_local_i64(&mut self, box_: &PxuiBox, value: i64) { self.set_state(box_, value) }

    /// Reads `box_`'s state slot as an f32 (stored as its bits); a box never set reads 0.0.
    ///
    /// Allocates: never.
    /// Panics: as `local_bool`.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let b = ui.node("n", pxui::Node::DEFAULT);
    ///     ui.set_local_f32(&b, -1.5);
    ///     assert_eq!(ui.local_f32(&b), -1.5);
    /// });
    /// ```
    #[inline]
    #[track_caller]
    pub fn local_f32(&self, box_: &PxuiBox) -> f32 { f32::from_bits(self.state(box_, 0) as u32) }

    /// Stores `value` in `box_`'s state slot as its bits; it persists across frames while the box lives.
    ///
    /// Allocates: never.
    /// Panics: as `local_bool`.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let b = ui.node("n", pxui::Node::DEFAULT);
    ///     ui.set_local_f32(&b, 0.25);
    ///     assert_eq!(ui.local_f32(&b), 0.25);
    /// });
    /// ```
    #[inline]
    #[track_caller]
    pub fn set_local_f32(&mut self, box_: &PxuiBox, value: f32) { self.set_state(box_, value.to_bits() as i64) }
}

#[cfg(test)]
mod tests {
    use crate::{FrameIn, Node};
    use pxui_types::PxuiBox;

    fn frame(ui: &mut crate::Ui, f: &mut dyn FnMut(&mut crate::Ui)) {
        let input = FrameIn { width: 320.0, height: 240.0, ..FrameIn::default() };
        ui.frame(&input, f).expect("frame");
    }

    /// Each type round-trips, across frames, including negative ints and the f32 extremes; unset reads the default.
    #[test]
    fn round_trips() {
        let mut ui = crate::Ui::new().expect("library");
        frame(&mut ui, &mut |ui| {
            let b = ui.node("n", Node::DEFAULT);
            assert!(!ui.local_bool(&b));
            assert_eq!(ui.local_i64(&b), 0);
            assert_eq!(ui.local_f32(&b), 0.0);
            ui.set_local_bool(&b, true);
            assert!(ui.local_bool(&b));
            ui.set_local_bool(&b, false);
            assert!(!ui.local_bool(&b));
            for v in [0, 1, -1, i64::MIN, i64::MAX, -123_456_789_012] { ui.set_local_i64(&b, v); assert_eq!(ui.local_i64(&b), v); }
            for v in [0.0f32, -0.0, 1.5, -2.75, f32::MIN, f32::MAX, f32::MIN_POSITIVE, f32::INFINITY] {
                ui.set_local_f32(&b, v);
                assert_eq!(ui.local_f32(&b).to_bits(), v.to_bits());
            }
            ui.set_local_f32(&b, 3.5);
        });
        frame(&mut ui, &mut |ui| { let b = ui.node("n", Node::DEFAULT); assert_eq!(ui.local_f32(&b), 3.5); });
    }

    /// A handle the engine no longer resolves is refused with a panic by the getter and the setter alike, exactly like `state` / `set_state`:
    /// there is no silent default. (A handle of a box that was not redeclared still resolves by its key and reads its retained value.)
    #[test]
    fn stale_handle_panics_like_state() {
        let mut ui = crate::Ui::new().expect("library");
        let stale = PxuiBox { index: 9999, box_key: 77, box_slot: 9999, generation: 99 };
        let refused = |f: &mut dyn FnMut(&mut crate::Ui)| {
            let mut ui = crate::Ui::new().expect("library");
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| frame(&mut ui, f))).is_err()
        };
        assert!(refused(&mut |ui| { ui.node("n", Node::DEFAULT); ui.state(&stale, 0); }), "state is the reference behavior");
        assert!(refused(&mut |ui| { ui.node("n", Node::DEFAULT); ui.set_state(&stale, 1); }), "so is set_state");
        assert!(refused(&mut |ui| { ui.node("n", Node::DEFAULT); ui.local_bool(&stale); }));
        assert!(refused(&mut |ui| { ui.node("n", Node::DEFAULT); ui.local_i64(&stale); }));
        assert!(refused(&mut |ui| { ui.node("n", Node::DEFAULT); ui.local_f32(&stale); }));
        assert!(refused(&mut |ui| { ui.node("n", Node::DEFAULT); ui.set_local_bool(&stale, true); }));
        assert!(refused(&mut |ui| { ui.node("n", Node::DEFAULT); ui.set_local_i64(&stale, 1); }));
        assert!(refused(&mut |ui| { ui.node("n", Node::DEFAULT); ui.set_local_f32(&stale, 1.0); }));
        frame(&mut ui, &mut |ui| { ui.node("n", Node::DEFAULT); });
    }

    /// Warm get/set calls allocate nothing.
    #[test]
    fn warm_calls_allocate_nothing() {
        let mut ui = crate::Ui::new().expect("library");
        let run = |ui: &mut crate::Ui| frame(ui, &mut |ui| {
            let b = ui.node("n", Node::DEFAULT);
            ui.set_local_bool(&b, true); ui.set_local_i64(&b, -3); ui.set_local_f32(&b, 2.5);
            assert_eq!(ui.local_f32(&b), 2.5); assert_eq!(ui.local_i64(&b), 2.5f32.to_bits() as i64); assert!(ui.local_bool(&b));
        });
        for _ in 0..4 { run(&mut ui); }
        mem_sys::local_reset();
        for _ in 0..4 { run(&mut ui); }
        assert_eq!(mem_sys::local_alloc_count(), 0);
    }
}
