//! The two calls whose shape differs between the wrapper and the engine (a generic closure against an `Rc`, keyframe structs against
//! tuples). The A7 bench compiles the pages against the engine with its own version of these in `tools/abi_bench`.
use pxui::kit::{AnimationTarget, Keyframe};
use pxui::{PxuiBox, Ui};
use pxui_types::theme::Curve;

pub fn table(ui: &mut Ui, at: (f32, f32), w: f32, h: f32, headers: &[String], rows: i64, cell: &mut dyn FnMut(i64, i64) -> String, label: &str) -> Option<i64> {
    ui.table(at, w, h, headers, rows, cell, label)
}

pub fn bounce(ui: &mut Ui, c: &PxuiBox) {
    let ease = Curve::Bezier { duration: 0.0, points: [0.25, 0.1, 0.25, 1.0] };
    let keys = [
        Keyframe { time: 0.2, target: AnimationTarget::Offset(120.0, 0.0), curve: ease },
        Keyframe { time: 0.35, target: AnimationTarget::Opacity(0.3), curve: ease },
        Keyframe { time: 0.6, target: AnimationTarget::Offset(0.0, 0.0), curve: Curve::Spring { omega: 18.0 } },
        Keyframe { time: 0.7, target: AnimationTarget::Opacity(1.0), curve: ease },
    ];
    let _ = ui.animate_keys(c, &keys);
}
