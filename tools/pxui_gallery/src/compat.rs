//! The calls whose shape differs between the wrapper and the engine (a generic closure against an `Rc`, keyframe structs against
//! tuples). The A7 bench compiles the pages against the engine with its own version of these in `tools/abi_bench`.
use pxui::kit::{AnimationTarget, Keyframe};
use pxui::{PxuiBox, Ui};
use pxui_types::theme::Curve;

// The pages keep owned rows (the engine takes them); the wrapper borrows, so each call lends them through a stack array.
pub fn table(ui: &mut Ui, at: (f32, f32), w: f32, h: f32, headers: &[String], rows: i64, cell: &mut dyn FnMut(i64, i64) -> String, label: &str) -> Option<i64> {
    let mut lent = [""; 16];
    for (slot, header) in lent.iter_mut().zip(headers) { *slot = header; }
    ui.table(at, w, h, &lent[..headers.len().min(16)], rows, cell, label)
}

pub fn footer(ui: &mut Ui, right: Option<&str>, hints: &[(String, String)]) {
    let mut lent = [("", ""); 8];
    for (slot, (key, text)) in lent.iter_mut().zip(hints) { *slot = (key, text); }
    ui.footer(right, &lent[..hints.len().min(8)])
}

pub fn context_menu(ui: &mut Ui, at: (f32, f32), o: &pxui::MenuOpts, label: &str, items: &[(String, bool)]) -> pxui::MenuResult {
    let mut lent = [("", false); 16];
    for (slot, (text, enabled)) in lent.iter_mut().zip(items) { *slot = (text, *enabled); }
    ui.context_menu(at, o, label, &lent[..items.len().min(16)])
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
