//! Select each paint-order range once across a union of device scissors.

use pxui_core::ui_core::{Layer, EMPTY_BOUNDS};

use crate::View;

fn intersects(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0] <= b[2] && a[2] >= b[0] && a[1] <= b[3] && a[3] >= b[1]
}

/// Move the screen scissor into the view's AABB. Inverse mapping four corners also
/// bounds rotations and shears, so local instance checks only need four comparisons.
fn in_view(rect: [f32; 4], view: &View, pixel: f32) -> [f32; 4] {
    let [ox, oy] = view.offset;
    let [a, b, c, d] = view.transform.map(|v| v * view.scale);
    let det = a * d - b * c;
    if det == 0.0 || !det.is_finite() {
        return [f32::NEG_INFINITY, f32::NEG_INFINITY, f32::INFINITY, f32::INFINITY];
    }
    let [px, py] = view.pivot;
    let x = [rect[0] - px, rect[2] - px];
    let y = [rect[1] - py, rect[3] - py];
    let [a, b, c, d] = [d / det, -b / det, -c / det, a / det];
    // The shader measures AA width using the inverse transform's device-pixel derivatives.
    // This also covers grid and shadow vertices when their shader padding expands under shrink.
    let aa = (pixel * (a.abs() + b.abs()).max(c.abs() + d.abs()))
        .max(det.abs().sqrt().max(1e-3).recip() - 1.0);
    let axis = |u: f32, v: f32, pivot: f32| {
        let [x0, x1] = if u >= 0.0 { x } else { [x[1], x[0]] };
        let [y0, y1] = if v >= 0.0 { y } else { [y[1], y[0]] };
        [u * x0 + v * y0 + pivot - aa, u * x1 + v * y1 + pivot + aa]
    };
    let [x0, x1] = axis(a, b, px - ox);
    let [y0, y1] = axis(c, d, py - oy);
    [x0, y0, x1, y1]
}

pub fn ranges(scissors: &[[u32; 4]], layers: &[Layer], views: &[View], scale: f32, out: &mut Vec<(u32, u32, u8)>) {
    let pixel = 1.0 / scale;
    let mut rects = [([0.0; 4], [0.0; 4]); 8];
    let rects = &mut rects[..scissors.len()];
    for ((rect, padded), &[x, y, w, h]) in rects.iter_mut().zip(scissors) {
        *rect = [x, y, x + w, y + h].map(|v| v as f32 * pixel);
        *padded = [rect[0] - pixel, rect[1] - pixel, rect[2] + pixel, rect[3] + pixel];
    }
    let mut offset = 0;
    let mut view_index = None;
    let mut local = [([0.0; 4], 0); 8];
    let mut local_len = 0;
    out.clear();
    for layer in layers {
        let len = layer.bounds.len() as u32;
        let first = out.len();
        let mut selected = 0;
        // Keep this layer's ranges separate until its 50% fallback has been decided.
        for run in &layer.runs {
            if view_index != Some(run.view) {
                let view = &views[run.view as usize];
                view_index = Some(run.view);
                local_len = 0;
                let translation = view.scale == 1.0 && view.transform == View::IDENTITY.transform;
                let [ox, oy] = view.offset;
                for (i, &(rect, padded)) in rects.iter().enumerate() {
                    if intersects(view.clip, padded) {
                        let local_rect = if translation {
                            [padded[0] - ox, padded[1] - oy, padded[2] - ox, padded[3] - oy]
                        } else { in_view(rect, view, pixel) };
                        local[local_len] = (local_rect, 1 << i);
                        local_len += 1;
                    }
                }
            }
            let local = &local[..local_len];
            let mask = |bounds, candidates| local.iter().fold(0, |mask, &(rect, bit)| {
                if bit & candidates != 0 && intersects(bounds, rect) { mask | bit } else { mask }
            });
            let run_mask = mask(run.bounds, u8::MAX);
            if run_mask == 0 { continue; }
            if run.to - run.from <= 64 {
                push_layer(out, first, offset + run.from, offset + run.to, run_mask);
                selected += run.to - run.from;
            } else {
                for at in run.from..run.to {
                    let bounds = layer.bounds[at as usize];
                    if bounds == EMPTY_BOUNDS { continue; }
                    let damage = mask(bounds, run_mask);
                    if damage != 0 {
                        push_layer(out, first, offset + at, offset + at + 1, damage);
                        selected += 1;
                        if selected > len / 2 { break; }
                    }
                }
            }
            if selected > len / 2 {
                out.truncate(first);
                out.push((offset, offset + len, u8::MAX));
                break;
            }
        }
        if first > 0 && out.len() > first && out[first].0 - out[first - 1].1 < 16 {
            let (_, to, damage) = out.remove(first);
            out[first - 1].1 = to;
            out[first - 1].2 |= damage;
        }
        offset += len;
    }
}

fn push_layer(out: &mut Vec<(u32, u32, u8)>, first: usize, from: u32, to: u32, damage: u8) {
    if out.len() > first && let Some(last) = out.last_mut() && from - last.1 < 16 {
        last.1 = to;
        last.2 |= damage;
    } else {
        out.push((from, to, damage));
    }
}
