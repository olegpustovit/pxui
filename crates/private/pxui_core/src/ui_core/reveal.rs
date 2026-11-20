//! Reveal uses retained list geometry, including estimates for unvisited measured rows. Keep the
//! request until measurement/layout settles. Warm requests use cached prefixes without allocating;
//! new measurements rebuild those prefixes in their retained storage.
use super::*;

#[derive(Clone, Copy, Default)]
pub(super) struct Geometry {
    slot: usize,
    generation: u32,
    count: usize,
    stride: f32,
    pub(super) across: f32,
    before: f32,
    header: f32,
    after: f32,
    pub(super) horizontal: bool,
    padding: Edges,
}
impl Geometry {
    pub(super) fn rows(body: &Handle, count: usize, stride: f32, before: f32, header: f32, after: f32, padding: Edges) -> Self {
        Self { slot: body.box_slot, generation: body.generation, count, stride, across: 0.0, before, header, after, horizontal: false, padding }
    }
}

impl Ui {
    pub(super) fn list_padding(&self, key: i64) -> Edges {
        self.table.find(key).map_or(Edges::ZERO, |slot| self.slots[slot].node.padding)
    }

    pub(super) fn remember_list(&mut self, body: &Handle, geometry: Geometry) {
        let cache = self.list_heights.entry(body.box_key).or_default();
        cache.geometry = geometry;
        cache.frame = self.frame_number;
    }

    /// Scroll the minimum needed to show an item in a fixed/measured list or grid. Measured rows
    /// use their cached heights, settling new measurements without replaying input. Stale handles
    /// and out-of-range indices do nothing. A request never allocates.
    pub fn reveal(&mut self, list: &Handle, index: usize) {
        if !self.live(list) || index == usize::MAX || !self.slots[list.box_slot].node.flags.any(Flags::SCROLL | Flags::SCROLL_X) { return; }
        self.slots[list.box_slot].reveal = index;
        if self.building { return; }
        if let Some(mut cache) = self.list_heights.remove(&list.box_key) {
            if cache.geometry.slot == list.box_slot && cache.geometry.generation == list.generation {
                self.position_reveal(&mut cache, true);
            }
            self.list_heights.insert(list.box_key, cache);
        } else {
            self.slots[list.box_slot].reveal = usize::MAX;
        }
    }

    pub(super) fn finish_reveals(&mut self) {
        let mut caches = std::mem::take(&mut self.list_heights);
        for cache in caches.values_mut() {
            let g = cache.geometry;
            if self.slots.get(g.slot).is_some_and(|s| s.generation == g.generation && s.key != 0
                && (s.touched == self.frame_number || self.kept_now(s.owner))) {
                if self.slots[g.slot].node.padding != g.padding {
                    self.mark_view(self.slots[g.slot].owner);
                    self.settle = true;
                    self.frame_requested = true;
                }
                self.position_reveal(cache, true);
            }
        }
        self.list_heights = caches;
    }

    fn position_reveal(&mut self, cache: &mut RowHeights, finish: bool) {
        let g = cache.geometry;
        let index = self.slots[g.slot].reveal;
        if index == usize::MAX { return; }
        if index >= g.count { self.slots[g.slot].reveal = usize::MAX; return; }
        let slot = &self.slots[g.slot];
        let p = slot.node.padding;
        let room = if g.horizontal { slot.rw - p.l - p.r } else { slot.rh - p.t - p.b };
        let across = if g.horizontal { slot.rh - p.t - p.b } else { slot.rw - p.l - p.r };
        let per = if g.across > 0.0 { ((across / g.across).floor() as usize).max(1) } else { 1 };
        let measured = g.stride == 0.0;
        let dirty = measured && cache.dirty;
        if dirty {
            cache.tops.clear(); cache.tops.push(0.0);
            for &height in &cache.heights { cache.tops.push(cache.tops.last().unwrap() + height); }
            cache.dirty = false;
        }
        let (top, bottom, total) = if measured {
            let Some(pair) = cache.tops.get(index..=index + 1) else { return; };
            (g.before + pair[0], g.before + pair[1], g.before + cache.tops.last().unwrap() + g.after)
        } else {
            let top = g.before + (index / per) as f32 * g.stride;
            (top, top + g.stride, g.before + g.count.div_ceil(per) as f32 * g.stride + g.after)
        };
        if room <= 0.0 || !total.is_finite() { if finish { self.slots[g.slot].reveal = usize::MAX; } return; }
        let header = g.header.min(room);
        let offset = if g.horizontal { self.scroll_xs[g.slot] } else { self.scroll_y[g.slot] };
        let target = if top < offset + header || bottom - top > room - header { top - header }
            else if bottom > offset + room { bottom - room } else { offset };
        let target = target.clamp(0.0, (total - room).max(0.0));
        let axis = usize::from(g.horizontal);
        let changed = target != offset || self.glides[axis][g.slot].visual != 0.0;
        let body = Handle { index: NONE, box_key: slot.key, box_slot: g.slot, generation: g.generation, ui: self.ui_serial };
        if g.horizontal { self.set_scroll_offset_x(&body, target); } else { self.set_scroll_offset(&body, target); }
        if !self.glides[0][g.slot].moving() && !self.glides[1][g.slot].moving() { self.moving.retain(|slot| *slot != g.slot); }
        if changed || dirty {
            self.mark_view(self.slots[g.slot].owner);
            self.settle |= self.building;
            self.frame_requested = true;
        } else if finish {
            self.slots[g.slot].reveal = usize::MAX;
        }
    }
}
