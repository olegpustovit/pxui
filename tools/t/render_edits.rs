//! Common deterministic edit sequence for the host and the renderer differential checks.
use pxui::theme::{Curve, Look};
use pxui::ui::{Keyframe, Target};
use pxui::{
    Fill, FrameIn, Keyframes, Label, Node, Overlay, Point, Rect, Repeat, Rgba, ThemeHandle, Ui,
    ViewTransform,
};

pub const STAGES: usize = 10;
pub const HEIGHT: u32 = 512;

pub fn width(stage: usize) -> u32 {
    if stage >= 9 { 420 } else { 400 }
}

fn seeds() -> usize {
    if std::env::var_os("RAYS_SLOW").is_some_and(|value| value == "1") {
        5_000
    } else {
        200
    }
}

fn mix(seed: usize) -> u32 {
    let mut value = (seed as u32 + 1).wrapping_mul(0x9e3779b9);
    value ^= value << 13;
    value ^= value >> 17;
    value ^ (value << 5)
}

pub fn edit(ui: &mut Ui, stage: usize, scale: f32, theme: &ThemeHandle) {
    let count = seeds();
    let time = stage as f64 * 0.05;
    let play = Keyframes::DEFAULT.repeat(Repeat::Forever).autoreverse();
    let curve = Curve::Bezier {
        duration: 0.2,
        points: [0.25, 0.1, 0.25, 1.0],
    };
    let mut patch = None;
    if stage == 8 {
        ui.set_theme(theme).unwrap();
    }
    ui.frame(
        &FrameIn::new(width(stage) as f32, HEIGHT as f32)
            .scale(scale)
            .time(time),
        &mut |ui| {
            if stage == 4 {
                let popup = Overlay::DEFAULT
                    .at(Point::new(72.0, 92.0))
                    .w_px(180.0)
                    .h_px(44.0);
                ui.popup("popup", &popup, |ui| {
                    ui.label("Overlay", Label::BODY);
                });
            }
            ui.node(
                "panel",
                Node::DEFAULT
                    .w_px(width(stage) as f32)
                    .h_px(HEIGHT as f32)
                    .at(0.0, 0.0)
                    .look(Look::NONE.fill(ui.tokens().panel)),
            );
            let tiles = ui.node(
                "samples",
                Node::DEFAULT.w_px(350.0).h_px(410.0).at(9.0, 16.0),
            );
            // Leave the seeded grid unchanged for the localized patch in stage two.
            let tile_stage = if stage == 2 { 1 } else { stage };
            ui.canvas(&tiles, move |paint, rect| {
                for seed in 0..count {
                    let n = mix(seed);
                    let variant = if tile_stage > 0 && seed % 3 == tile_stage % 3 {
                        1 + tile_stage / 4
                    } else {
                        0
                    };
                    let value = (n >> (variant * 6)) as u8;
                    let x = rect.x + (seed % 50) as f32 * 7.0 + (n & 1) as f32;
                    let y = rect.y + (seed / 50) as f32 * 4.0;
                    paint.fill(
                        Rect::new(x, y, 3.0 + (value % 3) as f32, 3.0),
                        Fill::new(Rgba::rgba(value, n as u8, (n >> 8) as u8, 240)),
                    );
                }
            });
            let text = ui.node("text", Node::DEFAULT.w_px(96.0).h_px(24.0).at(28.0, 430.0));
            ui.text_line_box(
                &text,
                if stage < 3 { "South" } else { "North" },
                Label::BODY.line_height(20.0),
                &[],
            )
            .unwrap();
            let view = ui.view_of("motion", &(stage == 2), |ui| {
                patch = Some(
                    ui.node(
                        "patch",
                        Node::DEFAULT
                            .w_px(24.0)
                            .h_px(16.0)
                            .look(Look::NONE.fill(Rgba::rgb(233, 110, 80))),
                    ),
                );
            });
            ui.set_node_layout(
                &view,
                &Node::column().w_px(24.0).h_px(16.0).at(111.0, 430.0),
            );
            ui.set_view(
                &view,
                &ViewTransform::DEFAULT.offset(if stage >= 1 { 8.0 } else { 0.0 }, 0.0),
            );
            ui.animate(
                &view,
                Target::Opacity(if stage >= 3 { 0.6 } else { 1.0 }),
                curve,
            );
            let looped = ui.view_of("loop", &1, |ui| {
                ui.node(
                    "shape",
                    Node::DEFAULT
                        .w_px(22.0)
                        .h_px(18.0)
                        .look(Look::NONE.fill(Rgba::rgb(58, 163, 211))),
                );
            });
            ui.set_node_layout(
                &looped,
                &Node::column().w_px(22.0).h_px(18.0).at(170.0, 431.0),
            );
            ui.animate_keys(
                &looped,
                &[Keyframe::scale(0.12, 0.6, 1.0).curve(curve)],
                &play,
            );
            if matches!(stage, 5 | 6) {
                let pane = ui.view_of("glass", &1, |ui| {
                    ui.node(
                        "tint",
                        Node::DEFAULT
                            .w_px(48.0)
                            .h_px(24.0)
                            .look(Look::NONE.fill(Rgba::rgba(0, 0, 0, 8))),
                    );
                });
                ui.set_node_layout(&pane, &Node::column().w_px(48.0).h_px(24.0).at(30.0, 55.0));
                ui.set_view(&pane, &ViewTransform::DEFAULT.blur(3.0));
            }
            ui.node(
                "canary",
                Node::DEFAULT
                    .w_px(11.0)
                    .h_px(11.0)
                    .at(380.0, 480.0)
                    .look(Look::NONE.fill(Rgba::rgb(5, 193, 155))),
            );
        },
    )
    .unwrap();
    if stage == 2 {
        let patch = patch.expect("motion child rebuilt");
        assert!(ui.patch_look(&patch, &Look::NONE.fill(Rgba::rgb(200, 45, 147))));
    }
}

pub fn diff(partial: &[u8], full: &[u8], tolerance: u8, stage: usize) {
    assert_eq!(partial.len(), full.len());
    if let Some((index, (a, b))) = partial
        .iter()
        .zip(full)
        .enumerate()
        .find(|(_, (a, b))| a.abs_diff(**b) > tolerance)
    {
        panic!(
            "partial_equals_full stage {stage}: pixel {} channel {} was {a}, expected {b}",
            index / 4,
            index % 4
        );
    }
}
