//! Fixed-capacity damage accumulation shared by the core and host.

use crate::geom::Rect;

const CAPACITY: usize = 8;

/// A bounded collection of damaged rectangles in logical window points.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DamageList {
    rects: [Rect; CAPACITY],
    len: u8,
    full: bool,
}

impl DamageList {
    /// Returns the populated rectangles, including the viewport for full damage.
    /// Allocates: never.
    /// Panics: never.
    /// ```
    /// assert!(pxui_types::damage::DamageList::default().rects().is_empty());
    /// ```
    pub fn rects(&self) -> &[Rect] {
        &self.rects[..self.len as usize]
    }

    /// Reports whether the whole viewport needs redrawing.
    /// Allocates: never.
    /// Panics: never.
    /// ```
    /// assert!(!pxui_types::damage::DamageList::default().is_full());
    /// ```
    pub fn is_full(&self) -> bool {
        self.full
    }

    /// Marks the whole viewport in window points as damaged.
    /// Allocates: never.
    /// Panics: never.
    /// ```
    /// use pxui_types::{damage::DamageList, geom::Rect};
    /// assert!(DamageList::full(Rect::new(0.0, 0.0, 80.0, 60.0)).is_full());
    /// ```
    pub fn full(viewport: Rect) -> Self {
        let mut list = Self {
            full: true,
            ..Self::default()
        };
        if area(viewport) > 0.0 {
            list.rects[0] = viewport;
            list.len = 1;
        }
        list
    }

    /// Returns the bounding box of the reported rectangles in window points.
    /// Allocates: never.
    /// Panics: never.
    /// ```
    /// assert_eq!(pxui_types::damage::DamageList::default().bounds(), None);
    /// ```
    pub fn bounds(&self) -> Option<Rect> {
        self.rects().iter().copied().reduce(union)
    }

    /// Reports whether a single damage rectangle already covers these window points.
    /// Allocates: never.
    /// Panics: never.
    /// ```
    /// use pxui_types::{damage::DamageList, geom::Rect};
    /// let viewport = Rect::new(0.0, 0.0, 100.0, 100.0);
    /// assert!(DamageList::full(viewport).covers(viewport));
    /// ```
    pub fn covers(&self, rect: Rect) -> bool {
        self.rects().iter().any(|&r| contains(r, rect))
    }

    /// Adds a rectangle clipped to the viewport in window points.
    /// Allocates: never.
    /// Panics: never.
    /// ```
    /// use pxui_types::{damage::DamageList, geom::Rect};
    /// let rect = Rect::new(10.0, 10.0, 5.0, 5.0);
    /// let mut damage = DamageList::default();
    /// damage.add(rect, Rect::new(0.0, 0.0, 100.0, 100.0));
    /// assert_eq!(damage.rects(), &[rect]);
    /// ```
    pub fn add(&mut self, rect: Rect, viewport: Rect) {
        if self.full
            || ![rect.x, rect.y, rect.w, rect.h]
                .iter()
                .all(|v| v.is_finite())
        {
            return;
        }
        let rect = rect.intersect(viewport);
        if area(rect) <= 0.0 || self.covers(rect) {
            return;
        }
        self.insert(rect);
        if self.rects().iter().copied().map(area).sum::<f32>() > area(viewport) * 0.6 {
            *self = Self::full(viewport);
        }
    }

    fn insert(&mut self, mut rect: Rect) {
        let mut i = 0;
        while i < self.len as usize {
            let old = self.rects[i];
            if !touches(old, rect) { i += 1; continue; }
            if contains(old, rect) {
                return;
            }
            if contains(rect, old) {
                self.remove(i);
                continue;
            }
            let (extra, joined) = merge_cost(old, rect);
            if extra <= (area(joined) - extra) * 0.25 {
                self.remove(i);
                rect = joined;
                i = 0;
                continue;
            }
            i += 1;
        }

        if self.len as usize == CAPACITY {
            let candidates: [Rect; CAPACITY + 1] = std::array::from_fn(|i| self.rects.get(i).copied().unwrap_or(rect));
            let edges = candidates.map(|r| (r.x, r.y, r.x + r.w, r.y + r.h, area(r)));
            let mut best = (f32::INFINITY, 0, CAPACITY);
            for (i, &(ax, ay, ar, ab, area_a)) in edges[..CAPACITY].iter().enumerate() {
                for (j, &(bx, by, br, bb, area_b)) in edges.iter().enumerate().skip(i + 1) {
                    let joined = (ar.max(br) - ax.min(bx)) * (ab.max(bb) - ay.min(by));
                    let sum = area_a + area_b;
                    if joined - sum >= best.0 { continue; }
                    let overlap = (ar.min(br) - ax.max(bx)).max(0.) * (ab.min(bb) - ay.max(by)).max(0.);
                    let extra = joined - (sum - overlap);
                    if extra < best.0 { best = (extra, i, j); }
                }
            }
            let (cost, i, j) = best;
            let joined = if cost < f32::INFINITY { union(candidates[i], candidates[j]) } else { rect };
            if j < CAPACITY {
                self.remove(j);
            }
            self.remove(i);
            self.insert(joined);
            if j == CAPACITY {
                return;
            }
            // Merging two existing rectangles freed a slot for the incoming one.
            self.insert(rect);
            return;
        }

        self.rects[self.len as usize] = rect;
        self.len += 1;
    }

    fn remove(&mut self, index: usize) {
        let end = self.len as usize;
        self.rects.copy_within(index + 1..end, index);
        self.len -= 1;
        self.rects[self.len as usize] = Rect::default();
    }
}

fn area(r: Rect) -> f32 {
    r.w.max(0.0) * r.h.max(0.0)
}

fn contains(a: Rect, b: Rect) -> bool {
    a.x <= b.x && a.y <= b.y && a.x + a.w >= b.x + b.w && a.y + a.h >= b.y + b.h
}

fn touches(a: Rect, b: Rect) -> bool {
    a.x <= b.x + b.w && b.x <= a.x + a.w && a.y <= b.y + b.h && b.y <= a.y + a.h
}

fn union(a: Rect, b: Rect) -> Rect {
    let (x, y) = (a.x.min(b.x), a.y.min(b.y));
    Rect::new(
        x,
        y,
        (a.x + a.w).max(b.x + b.w) - x,
        (a.y + a.h).max(b.y + b.h) - y,
    )
}

fn merge_cost(a: Rect, b: Rect) -> (f32, Rect) {
    let joined = union(a, b);
    (
        area(joined) - (area(a) + area(b) - area(a.intersect(b))),
        joined,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIEWPORT: Rect = Rect::new(0.0, 0.0, 200.0, 100.0);

    impl DamageList {
        fn insert_reference(&mut self, mut rect: Rect) {
            let mut i = 0;
            while i < self.len as usize {
                let old = self.rects[i];
                if contains(old, rect) {
                    return;
                }
                if contains(rect, old) {
                    self.remove(i);
                    continue;
                }
                if touches(old, rect) {
                    let (extra, joined) = merge_cost(old, rect);
                    if extra <= (area(joined) - extra) * 0.25 {
                        self.remove(i);
                        rect = joined;
                        i = 0;
                        continue;
                    }
                }
                i += 1;
            }

            if self.len as usize == CAPACITY {
                let mut best = (f32::INFINITY, 0, CAPACITY, rect);
                for (i, &a) in self.rects().iter().enumerate() {
                    for (j, b) in self
                        .rects()
                        .iter()
                        .copied()
                        .chain(std::iter::once(rect))
                        .enumerate()
                        .skip(i + 1)
                    {
                        let (extra, joined) = merge_cost(a, b);
                        if extra < best.0 {
                            best = (extra, i, j, joined);
                        }
                    }
                }
                let (_, i, j, joined) = best;
                if j < CAPACITY {
                    self.remove(j);
                }
                self.remove(i);
                self.insert_reference(joined);
                if j == CAPACITY {
                    return;
                }
                // Merging two existing rectangles freed a slot for the incoming one.
                self.insert_reference(rect);
                return;
            }

            self.rects[self.len as usize] = rect;
            self.len += 1;
        }

    }

    #[test]
    fn insertion_preserves_exact_rectangle_order_while_scrolling_and_overflowing() {
        let mut actual = DamageList::default();
        let mut reference = actual;
        let mut insert = |r| {
            actual.insert(r);
            reference.insert_reference(r);
            assert_eq!(actual, reference);
        };
        for row in 0..36 {
            for col in 0..64 {
                let x = 8.5 * col as f32;
                let y = 17.0 * row as f32 + (col % 3) as f32;
                for dx in [0., -8.5] {
                    insert(Rect::new(x + dx, y, 6.5, 9.0 + (col % 4) as f32));
                }
            }
        }
        let mut seed = 7u32;
        for _ in 0..4096 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            insert(Rect::new((seed & 255) as f32 * 7.3, (seed >> 24) as f32 * 3.7,
                ((seed >> 8) % 97 + 1) as f32, ((seed >> 16) % 59 + 1) as f32));
        }
    }

    #[test]
    fn damage_list_contains_merges_and_escalates() {
        let mut list = DamageList::default();
        let a = Rect::new(1.0, 2.0, 10.0, 10.0);
        list.add(a, VIEWPORT);
        list.add(Rect::new(2.0, 3.0, 4.0, 4.0), VIEWPORT);
        assert_eq!(list.rects(), &[a]);
        list.add(Rect::new(0.0, 2.0, 12.0, 10.0), VIEWPORT);
        list.add(Rect::new(12.0, 2.0, 2.0, 10.0), VIEWPORT);
        list.add(Rect::new(14.0, 12.0, 10.0, 10.0), VIEWPORT);
        assert_eq!(
            list.rects(),
            &[
                Rect::new(0.0, 2.0, 14.0, 10.0),
                Rect::new(14.0, 12.0, 10.0, 10.0)
            ]
        );
        list.add(Rect::new(0.0, 0.0, 120.0, 100.0), VIEWPORT);
        assert!(!list.is_full());
        list.add(Rect::new(120.0, 0.0, 1.0, 1.0), VIEWPORT);
        assert_eq!(list, DamageList::full(VIEWPORT));
    }

    #[test]
    fn damage_list_overflow_merges_the_cheapest_pair() {
        let mut list = DamageList::default();
        for x in [0, 20, 40, 60, 80, 100, 120, 140, 23] {
            list.add(Rect::new(x as f32, 10.0, 2.0, 2.0), VIEWPORT);
        }
        assert_eq!(list.rects().len(), CAPACITY);
        assert!(list.rects().contains(&Rect::new(20.0, 10.0, 5.0, 2.0)));
    }

    #[test]
    fn damage_list_fuzz_preserves_coverage_and_is_deterministic() {
        let mut seed = 1u32;
        let mut next = || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            seed >> 16
        };
        for _ in 0..512 {
            let mut a = DamageList::default();
            let mut b = a;
            let mut input = [Rect::default(); 32];
            for rect in &mut input {
                *rect = Rect::new(
                    (next() % 220) as f32 - 10.0,
                    (next() % 120) as f32 - 10.0,
                    (next() % 20 + 1) as f32,
                    (next() % 20 + 1) as f32,
                );
            }
            for (i, &rect) in input.iter().enumerate() {
                a.add(rect, VIEWPORT);
                b.add(rect, VIEWPORT);
                assert_eq!(a, b);
                for rect in &input[..=i] {
                    let clipped = rect.intersect(VIEWPORT);
                    assert!(
                        area(clipped) == 0.0 || a.covers(clipped),
                        "{clipped:?} missing from {a:?}"
                    );
                }
            }
        }
    }
}
