//! Promotion keeps the original instances available for an in-app capture or a same-frame demotion.
use super::{EMPTY, IDENTITY_2X2, NO_VIEW, Pose, Rect, Ui, ViewRec, bounds, union};
use crate::ui::Handle;
use pxui_types::draw::{flags, kind};
use pxui_types::ffi::{PxuiPromoted, PxuiPromotedKey};
use pxui_types::theme::Curve;
use pxui_types::ui::Rate;

const LIMIT: usize = 16;
const LINEAR: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

#[derive(Default)]
pub(super) struct Promotion {
    pub(super) enabled: bool,
    has_hints: bool,
    pub(super) current: Vec<PxuiPromoted>,
    next: Vec<PxuiPromoted>,
}

impl Ui {
    /// Enable the compositor contract on a host that can consume the promoted output.
    pub fn set_promotion(&mut self, enabled: bool) {
        if self.promotion.enabled == enabled {
            return;
        }
        self.promotion.enabled = enabled;
        if enabled {
            self.promotion.current.reserve(LIMIT);
            self.promotion.next.reserve(LIMIT);
        }
        self.update_promotions();
        self.out.repaint_at = self.next_repaint(self.now);
    }

    /// Mark a retained view for static promotion when its content and placement allow it.
    pub fn promote(&mut self, handle: &Handle) -> bool {
        let Some(v) = self.view_id(handle) else {
            return false;
        };
        self.views[v as usize].promote = true;
        self.promotion.has_hints = true;
        if !self.building && !self.painting {
            self.update_promotions();
        }
        true
    }

    #[inline]
    pub(super) fn promoted_view(&self, index: u16) -> bool {
        self.promotion.current.iter().any(|run| run.view == index)
    }

    pub(super) fn update_promotions(&mut self) {
        if !self.promotion.enabled && self.promotion.current.is_empty() {
            return;
        }
        let mut next = std::mem::take(&mut self.promotion.next);
        next.clear();
        if self.promotion.enabled && self.blur_out.is_empty() {
            let animated = self
                .tweens
                .iter()
                .filter(|tween| tween.view_only)
                .map(|tween| self.slots[tween.slot].view)
                .filter(|&index| index != NO_VIEW);
            let views = if self.promotion.has_hints {
                &self.views[..]
            } else {
                &[]
            };
            let hinted = views
                .iter()
                .enumerate()
                .skip(1)
                .filter(|(_, view)| {
                    view.valid && view.promote && self.tween_of(view.slot).is_none()
                })
                .map(|(index, _)| index as u32);
            for index in animated.chain(hinted) {
                if next.len() == LIMIT {
                    break;
                }
                let view = &self.views[index as usize];
                if !view.valid || !self.shown(index) || view.index == 0 {
                    continue;
                }
                if let Some(run) = self.promotion_candidate(view) {
                    if self.promotion_z_safe(&run) {
                        next.push(run);
                    }
                }
            }
        }
        next.sort_unstable_by_key(|p| (p.layer, p.instance));
        for i in (0..next.len()).rev() {
            let cover = rect(next[i].swept);
            if next[i + 1..]
                .iter()
                .any(|run| intersects(cover, rect(run.swept)))
            {
                next.remove(i);
            }
        }
        let mut current = std::mem::replace(&mut self.promotion.current, next);
        if current != self.promotion.current {
            self.draw_revision += 1;
        }
        for run in &current {
            if !self.promotion.current.contains(run) {
                self.add_damage(rect(run.swept));
            }
        }
        for i in 0..self.promotion.current.len() {
            let run = self.promotion.current[i];
            if !current.contains(&run) {
                self.add_damage(rect(run.swept));
            }
        }
        current.clear();
        self.promotion.next = current;
    }

    fn promotion_candidate(&self, view: &ViewRec) -> Option<PxuiPromoted> {
        let tween = self.tween_of(view.slot).map(|i| &self.tweens[i]);
        let (motion, tween) = match tween {
            Some(tween) if tween.view_only => {
                let keys = self.keyframes.get(tween.keys as usize)?;
                if keys.repeat != 0 {
                    return None;
                }
                (Some(keys), Some(tween))
            }
            None if view.promote => (None, None),
            _ => return None,
        };
        if view.scale != 1.0 || view.transform != IDENTITY_2X2 {
            return None;
        }
        // The host owns no inherited animation. Moving ancestors remain in the in-app renderer.
        let mut outer = view.parent;
        while outer != 0 {
            let parent = &self.views[outer as usize];
            if !parent.flat || self.tween_of(parent.slot).is_some() {
                return None;
            }
            outer = parent.parent;
        }
        let layer = self.draw[..self.draw_len]
            .iter()
            .position(|layer| layer.key == view.layer)?;
        let (from, to) = view.run;
        let mut solid = self.draw[layer]
            .instances
            .get(from as usize..to as usize)?
            .iter()
            .enumerate()
            .filter(|&(_, item)| *item != EMPTY);
        let (instance, fill) = solid.next()?;
        // One vector fill has the same alpha as the same draw blended in the canvas. A group
        // with overlapping instances would combine their alpha before applying the view opacity.
        if solid.next().is_some()
            || fill.view != view.index
            || fill.kind != kind::RECT
            || fill.flags != flags::rect::AREA
            || fill.width != 0
            || fill.radii != [0; 4]
            || fill.clip_radius != 0
            || fill.color[3] == 0
        {
            return None;
        }
        let [x0, y0, x1, y1] = fill.rect;
        let [cx0, cy0, cx1, cy1] = fill.clip.map(f32::from);
        if x0 < cx0 || y0 < cy0 || x1 > cx1 || y1 > cy1 {
            return None;
        }
        let parent = &self.views[self.views[view.parent as usize].xf as usize];
        let origin = (
            parent.shift.0 + view.offset.0 + view.delta.0,
            parent.shift.1 + view.offset.1 + view.delta.1,
        );
        let viewport = Rect::new(0.0, 0.0, self.view_w, self.view_h);
        let [cx0, cy0, cx1, cy1] = view.entry().clip;
        let clip = Rect::new(cx0, cy0, cx1 - cx0, cy1 - cy0).intersect(viewport);
        let make_key = |time, pose: &Pose, curve| {
            let b = view.built_rect;
            let [ax, ay] = [b.x + pose.anchor[0] * b.w, b.y + pose.anchor[1] * b.h];
            let [sx, sy] = pose.scale;
            PxuiPromotedKey {
                time,
                curve,
                rect: [
                    origin.0 + pose.dx + ax + (x0 - ax) * sx,
                    origin.1 + pose.dy + ay + (y0 - ay) * sy,
                    (x1 - x0) * sx,
                    (y1 - y0) * sy,
                ],
                opacity: parent.alpha * view.opacity * pose.opacity,
            }
        };
        let mut run = PxuiPromoted {
            layer: layer as u32,
            instance: from + instance as u32,
            view: view.index,
            content_revision: (view.built as u64) << 32 | u64::from(u32::from_ne_bytes(fill.color)),
            color: fill.color,
            clip: array(clip),
            ..PxuiPromoted::default()
        };
        if let (Some(keys), Some(tween)) = (motion, tween) {
            let initial = if keys.t[0] == 0.0 {
                &keys.pose[0]
            } else {
                &tween.from
            };
            run.keys[0] = make_key(0.0, initial, LINEAR);
            run.count = 1;
            run.start = tween.start;
            run.offset = keys.offset;
            run.autoreverse = keys.autoreverse.into();
            run.hz = match keys.rate {
                Rate::Display => 0,
                Rate::Ambient(hz) => u16::from(hz),
            };
            let mut previous = initial;
            for i in 0..keys.n {
                if keys.t[i] == 0.0 {
                    continue;
                }
                let Curve::Bezier { points, .. } = keys.curve[i] else {
                    return None;
                };
                let key = &keys.pose[i];
                if !(0.0..=1.0).contains(&points[1])
                    || !(0.0..=1.0).contains(&points[3])
                    || (key.anchor != previous.anchor && previous.scale != [1.0; 2])
                {
                    return None;
                }
                run.keys[usize::from(run.count)] = make_key(keys.t[i], key, points);
                run.count += 1;
                previous = key;
            }
        } else {
            run.keys[0] = make_key(0.0, &self.rest(view.slot), LINEAR);
            run.count = 1;
        }
        let mut swept = None;
        for key in &run.keys[..usize::from(run.count)] {
            let bounds = rect(key.rect);
            if !key.rect.iter().all(|v| v.is_finite()) || bounds.w <= 0.0 || bounds.h <= 0.0 {
                return None;
            }
            swept = Some(union(swept, bounds));
        }
        let first = run.keys[0].rect;
        let density = self.density as f32;
        let pixel_aligned = |point| {
            let pixels: f32 = point * density;
            (pixels - pixels.round()).abs() < 1.0e-4
        };
        let keys = &run.keys[..usize::from(run.count)];
        let x_fixed = pixel_aligned(first[0])
            && pixel_aligned(first[0] + first[2])
            && keys
                .iter()
                .all(|key| key.rect[0] == first[0] && key.rect[2] == first[2]);
        let top_fixed = keys.iter().all(|key| key.rect[1] == first[1]);
        let bottom = first[1] + first[3];
        let bottom_fixed = keys
            .iter()
            .all(|key| (key.rect[1] + key.rect[3] - bottom).abs() < 1.0e-4);
        // The qualified coverage is a straight fractional edge along one axis at 1x and 2x.
        // shortcut: qualify general transforms and multi-instance runs before admitting them.
        let separated_edges = keys.iter().all(|key| key.rect[3] * density >= 1.0);
        let clip_aligned = [clip.x, clip.y, clip.x + clip.w, clip.y + clip.h]
            .into_iter()
            .all(pixel_aligned);
        if !matches!(self.density, 1 | 2)
            || !x_fixed
            || !(top_fixed || bottom_fixed)
            || !separated_edges
            || !clip_aligned
        {
            return None;
        }
        let swept = swept?.intersect(clip);
        if swept.w <= 0.0 || swept.h <= 0.0 {
            return None;
        }
        run.swept = array(swept);
        Some(run)
    }

    fn promotion_z_safe(&self, run: &PxuiPromoted) -> bool {
        let swept = rect(run.swept);
        let pad = 2.0 / self.density.max(1) as f32;
        for (i, layer) in self.draw[..self.draw_len]
            .iter()
            .enumerate()
            .skip(run.layer as usize)
        {
            let from = if i == run.layer as usize {
                run.instance as usize + 1
            } else {
                0
            };
            for item in &layer.instances[from..] {
                let Some(bounds) = bounds(item) else { continue };
                let view = &self.views[self.index_owner[item.view as usize] as usize];
                let cover = view.cover(Rect::new(
                    bounds.x + view.shift.0,
                    bounds.y + view.shift.1,
                    bounds.w,
                    bounds.h,
                ));
                if intersects(
                    swept,
                    Rect::new(
                        cover.x - pad,
                        cover.y - pad,
                        cover.w + 2.0 * pad,
                        cover.h + 2.0 * pad,
                    ),
                ) {
                    return false;
                }
            }
        }
        true
    }
}

fn rect([x, y, w, h]: [f32; 4]) -> Rect {
    Rect::new(x, y, w, h)
}

fn array(rect: Rect) -> [f32; 4] {
    [rect.x, rect.y, rect.w, rect.h]
}

fn intersects(a: Rect, b: Rect) -> bool {
    let overlap = a.intersect(b);
    overlap.w > 0.0 && overlap.h > 0.0
}
