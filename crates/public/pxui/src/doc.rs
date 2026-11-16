//! Helpers that keep doc examples short.
use crate::{FrameIn, Ui};

/// Runs `n` frames of `build` on a fresh headless `Ui` (320 x 240 points, accessibility on) and returns the `Ui`.
///
/// The returned `Ui` keeps the last frame's tree, so an example can inspect it with `rect`, `a11y_nodes` and the like.
/// Allocates: creating the `Ui` and each frame's first build, like any frame.
/// Panics: when the library cannot create a `Ui` (an ABI of another major version), or a frame fails.
///
/// ```
/// let ui = pxui::doc::frames(2, &mut |ui| ui.label("hello"));
/// assert_eq!(ui.frame_index(), 2);
/// ```
pub fn frames(n: usize, build: &mut dyn FnMut(&mut Ui)) -> Ui {
    let mut ui = Ui::new().expect("pxui: the library refused to create a Ui");
    ui.set_a11y(true);
    for _ in 0..n {
        ui.frame(&FrameIn { width: 320.0, height: 240.0, ..FrameIn::default() }, build).expect("pxui: a doc frame failed");
    }
    ui
}
