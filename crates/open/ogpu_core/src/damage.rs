//! Per-image present damage: what each swapchain image still lacks of the retained canvas.
//!
//! A presenter that keeps the canvas as the source of truth copies into a swapchain image only what changed since
//! that image was last presented: the union of every frame's damage presented since then (on other images), plus
//! this frame's. An image never presented on the current swapchain, a resized or rebuilt (out-of-date) swapchain, or
//! a frame without a damage list takes the whole extent. Fixed capacity, no allocation.
use crate::render_pass::Rect;

/// Most rectangles one image's debt or one present carries; more collapse into their bounding box.
pub const MAX_RECTS: usize = 8;
/// Most swapchain images tracked; an index past it is always copied whole.
pub const MAX_IMAGES: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Debt {
    rects: [Rect; MAX_RECTS],
    len: u8,
    /// Unknown contents: copy the whole extent.
    full: bool,
}

impl Default for Debt {
    /// Owes nothing.
    fn default() -> Debt { Debt { full: false, ..Debt::FULL } }
}

impl Debt {
    const FULL: Debt = Debt { rects: [Rect { x: 0, y: 0, width: 0, height: 0 }; MAX_RECTS], len: 0, full: true };

    fn add(&mut self, rect: Rect) {
        if self.full || self.rects[..self.len as usize].iter().any(|&r| contains(r, rect)) {
            return;
        }
        // Drop what the new rect covers, then append; a full list collapses into one bounding box.
        let mut kept = 0;
        for i in 0..self.len as usize {
            if !contains(rect, self.rects[i]) {
                self.rects[kept] = self.rects[i];
                kept += 1;
            }
        }
        self.len = kept as u8;
        if kept == MAX_RECTS {
            let bounds = self.rects.iter().fold(rect, |a, &b| union(a, b));
            self.rects[0] = bounds;
            self.len = 1;
        } else {
            self.rects[kept] = rect;
            self.len += 1;
        }
    }
}

/// The damage history of one swapchain: per image index, the canvas changes it has not shown yet.
#[derive(Clone, Debug)]
pub struct History {
    images: [Debt; MAX_IMAGES],
    /// The swapchain generation and extent the debts belong to; a change makes every image unknown.
    generation: u64,
    extent: (i64, i64),
    /// The kill switch: every present copies the whole extent and carries no regions.
    force_full: bool,
    /// The last `take`'s copy list, which its result borrows.
    copy: Debt,
}

impl Default for History {
    fn default() -> Self { History::new() }
}

impl History {
    /// Every image unknown.
    pub const fn new() -> History {
        History { images: [Debt::FULL; MAX_IMAGES], generation: u64::MAX, extent: (0, 0), force_full: false, copy: Debt::FULL }
    }

    /// Turns damage-only presentation off (every copy full) or back on; turning it on forgets every image.
    pub fn set_force_full(&mut self, on: bool) {
        if self.force_full && !on {
            self.invalidate();
        }
        self.force_full = on;
    }

    pub fn force_full(&self) -> bool { self.force_full }

    /// Forgets every image: the next present of each copies the whole extent.
    pub fn invalidate(&mut self) {
        self.images = [Debt::FULL; MAX_IMAGES];
    }

    /// Records a present of `image` on swapchain `generation` with `extent` (pixels) whose canvas changed in
    /// `damage` since the previous present (`None`: unknown, everything), and returns what to copy into it: `None`
    /// for the whole extent (unknown contents, no damage list, the kill switch, or most of the image owed), else only
    /// these rects (clamped to the extent; none when the image is already current).
    /// Every other image owes this frame's damage from now on; `image` owes nothing after the copy.
    pub fn take(&mut self, generation: u64, extent: (i64, i64), image: usize, damage: Option<&[Rect]>) -> Option<&[Rect]> {
        if generation != self.generation || extent != self.extent {
            self.generation = generation;
            self.extent = extent;
            self.invalidate();
        }
        let whole = Rect { x: 0, y: 0, width: extent.0, height: extent.1 };
        for debt in &mut self.images {
            match damage {
                Some(rects) if !self.force_full => {
                    for &rect in rects {
                        if let Some(rect) = clamp(rect, whole) {
                            debt.add(rect);
                        }
                    }
                }
                _ => *debt = Debt::FULL,
            }
        }
        let Some(debt) = self.images.get_mut(image) else { return None };
        self.copy = std::mem::take(debt);
        let area: i64 = self.copy.rects[..self.copy.len as usize].iter().map(|r| r.width * r.height).sum();
        // Most of the image: one whole copy is cheaper than many scattered ones.
        if self.copy.full || area * 10 > whole.width * whole.height * 6 {
            return None;
        }
        Some(&self.copy.rects[..self.copy.len as usize])
    }
}

/// One `drm_mode_rect` of a DRM/KMS plane's `FB_DAMAGE_CLIPS` property blob: framebuffer pixels, `x2`/`y2` exclusive.
/// Design hook for a KMS host (docs/vulkan-linux.md): each scanout buffer owes its debt like a swapchain image.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DrmClip {
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
}

/// Fills `out` with the damage clips for an atomic commit of a `extent` framebuffer from what `History::take`
/// returned (`None`: the whole buffer) and returns how many it wrote. Zero clips means nothing changed; the KMS
/// property then stays unset, which the kernel reads as the whole plane.
/// Allocates: never.
pub fn drm_clips(copy: Option<&[Rect]>, extent: (i64, i64), out: &mut [DrmClip; MAX_RECTS]) -> usize {
    let whole = Rect { x: 0, y: 0, width: extent.0, height: extent.1 };
    let fit = |v: i64| v.clamp(0, i64::from(i32::MAX)) as i32;
    let mut count = 0;
    for rect in copy.unwrap_or(std::slice::from_ref(&whole)).iter().filter_map(|&r| clamp(r, whole)).take(MAX_RECTS) {
        out[count] = DrmClip { x1: fit(rect.x), y1: fit(rect.y), x2: fit(rect.x + rect.width), y2: fit(rect.y + rect.height) };
        count += 1;
    }
    count
}

/// `rect` inside `bounds`, or `None` when nothing of it is.
pub fn clamp(rect: Rect, bounds: Rect) -> Option<Rect> {
    let (x0, y0) = (rect.x.max(bounds.x), rect.y.max(bounds.y));
    let (x1, y1) = ((rect.x.saturating_add(rect.width)).min(bounds.x + bounds.width), (rect.y.saturating_add(rect.height)).min(bounds.y + bounds.height));
    (x1 > x0 && y1 > y0).then_some(Rect { x: x0, y: y0, width: x1 - x0, height: y1 - y0 })
}

fn contains(a: Rect, b: Rect) -> bool {
    a.x <= b.x && a.y <= b.y && a.x + a.width >= b.x + b.width && a.y + a.height >= b.y + b.height
}

/// The bounding box of `a` and `b`.
pub fn union(a: Rect, b: Rect) -> Rect {
    let (x, y) = (a.x.min(b.x), a.y.min(b.y));
    Rect { x, y, width: (a.x + a.width).max(b.x + b.width) - x, height: (a.y + a.height).max(b.y + b.height) - y }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: i64, y: i64, width: i64, height: i64) -> Rect { Rect { x, y, width, height } }

    /// Three images in rotation: each copies the damage of every frame presented since it was last shown.
    #[test]
    fn per_image_damage_history() {
        let mut h = History::new();
        let ext = (100, 100);
        // Unknown images copy whole, whatever the frame's damage.
        assert_eq!(h.take(1, ext, 0, Some(&[r(0, 0, 4, 4)])), None);
        assert_eq!(h.take(1, ext, 1, Some(&[r(10, 10, 4, 4)])), None);
        assert_eq!(h.take(1, ext, 2, Some(&[r(20, 20, 4, 4)])), None);
        // Image 0 last showed frame 0: it owes frames 1, 2 and this frame 3.
        assert_eq!(h.take(1, ext, 0, Some(&[r(30, 30, 4, 4)])), Some(&[r(10, 10, 4, 4), r(20, 20, 4, 4), r(30, 30, 4, 4)][..]));
        // Image 1 owes frames 2, 3 and 4 (an off-extent part is clipped away).
        assert_eq!(h.take(1, ext, 1, Some(&[r(98, 98, 8, 8)])), Some(&[r(20, 20, 4, 4), r(30, 30, 4, 4), r(98, 98, 2, 2)][..]));
        // A contained rect adds nothing; an idle frame (no damage) leaves the image's own debt.
        assert_eq!(h.take(1, ext, 2, Some(&[r(31, 31, 2, 2)])), Some(&[r(30, 30, 4, 4), r(98, 98, 2, 2)][..]));
        assert_eq!(h.take(1, ext, 0, Some(&[])), Some(&[r(98, 98, 2, 2), r(31, 31, 2, 2)][..]));
        assert_eq!(h.take(1, ext, 1, Some(&[])), Some(&[r(31, 31, 2, 2)][..]));
        assert_eq!(h.take(1, ext, 2, Some(&[])), Some(&[][..]));
        // A frame without a damage list owes everything to every image.
        assert_eq!(h.take(1, ext, 0, None), None);
        assert_eq!(h.take(1, ext, 1, Some(&[])), None);
        // A new swapchain generation (rebuilt, out of date) or a resized extent forgets every image.
        for i in 0..3 { h.take(1, ext, i, Some(&[])); }
        assert_eq!(h.take(1, ext, 2, Some(&[r(1, 1, 1, 1)])), Some(&[r(1, 1, 1, 1)][..]));
        assert_eq!(h.take(2, ext, 2, Some(&[r(1, 1, 1, 1)])), None);
        for i in 0..3 { h.take(2, ext, i, Some(&[])); }
        assert_eq!(h.take(2, (100, 90), 1, Some(&[])), None);
        // An index past the tracked images is always whole.
        assert_eq!(h.take(2, (100, 90), MAX_IMAGES, Some(&[])), None);
    }

    #[test]
    fn overflow_collapses_and_large_debt_copies_whole() {
        let mut h = History::new();
        let ext = (1000, 1000);
        for i in 0..2 { h.take(1, ext, i, Some(&[])); }
        // Nine scattered rects on image 0 overflow image 1's eight slots into one bounding box.
        let rects: Vec<Rect> = (0..9).map(|i| r(i * 20, i * 20, 5, 5)).collect();
        h.take(1, ext, 0, Some(&rects));
        assert_eq!(h.take(1, ext, 1, Some(&[])), Some(&[r(0, 0, 165, 165)][..]));
        // Over 60% of the image owed: one whole copy.
        h.take(1, ext, 0, Some(&[r(0, 0, 1000, 700)]));
        assert_eq!(h.take(1, ext, 1, Some(&[])), None);
    }

    #[test]
    fn drm_damage_clips() {
        let mut out = [DrmClip::default(); MAX_RECTS];
        assert_eq!(drm_clips(None, (640, 480), &mut out), 1);
        assert_eq!(out[0], DrmClip { x1: 0, y1: 0, x2: 640, y2: 480 });
        let n = drm_clips(Some(&[r(10, 20, 30, 40), r(630, 470, 50, 50), r(700, 0, 5, 5)]), (640, 480), &mut out);
        assert_eq!(&out[..n], &[DrmClip { x1: 10, y1: 20, x2: 40, y2: 60 }, DrmClip { x1: 630, y1: 470, x2: 640, y2: 480 }]);
        assert_eq!(drm_clips(Some(&[]), (640, 480), &mut out), 0);
    }

    #[test]
    fn kill_switch_copies_whole() {
        let mut h = History::new();
        let ext = (100, 100);
        h.set_force_full(true);
        for i in 0..3 { assert_eq!(h.take(1, ext, i, Some(&[r(0, 0, 1, 1)])), None); }
        assert_eq!(h.take(1, ext, 0, Some(&[r(0, 0, 1, 1)])), None);
        // Back on: every image is unknown again.
        h.set_force_full(false);
        assert_eq!(h.take(1, ext, 1, Some(&[r(0, 0, 1, 1)])), None);
        assert_eq!(h.take(1, ext, 1, Some(&[r(0, 0, 1, 1)])), Some(&[r(0, 0, 1, 1)][..]));
    }
}
