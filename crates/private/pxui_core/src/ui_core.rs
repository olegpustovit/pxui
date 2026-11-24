//! Scale-only animations update retained transform tables without rebuilding/rewriting instances.
//! Preserve independent axes, fixed fractional anchors, manual/nested composition and
//! hit/accessibility geometry. Animated bounds invalidate accessibility walks. Sampled curves
//! retain uneven positions/overshoot in fixed arrays and finish by duration, never by progress
//! reaching one; stack keyframe descriptors remain bounded. Compaction scratch must cover existing
//! fresh views becoming retained together within the actual retained-length budget.
//!
//! Reuse empty text/name/value buffers across virtual-slot identities and compact retained draw
//! runs within a budget proportional to live runs. Text nodes keep original accessibility labels
//! and optional span storage across rebuild/reuse; plain text/patch_text clears active spans. Host
//! reduced motion overrides scoped themes, snaps active keys and restores authored flags when
//! disabled. Reuse routed signal/modifier storage without dropping ordered key/text/IME events;
//! owned key-vector/text payload reads remain distinct allocation paths.
//!
//! Each frame declares keyed nodes. A node is a retained record with a stable `Handle` (slot index and
//! generation): its layout inputs, its laid-out rectangle, its retained input state, and its `Look`
//! and text as plain data the engine paints with no closure. A node lives until the build that owns
//! it stops declaring it. Input uses prior-frame rectangles; layout follows construction and painting
//! packs instance lists. Canvas closures live in the frame arena and run after layout. Between builds
//! the `patch_*` functions rewrite one node's instances in place (layer 1 of specification/pxui.md).
use crate::theme;
mod text_identity;
mod reveal;
use reveal::Geometry as ListGeometry;
use text_identity::lay_out_node;
pub use crate::geom::Rect;
use pxui_types::damage::DamageList;
/// An input point (`f64` window points) as the engine's `f32` point.
#[inline]
pub(crate) fn p32(p: (f64, f64)) -> (f32, f32) { (p.0 as f32, p.1 as f32) }
use input::{Event, Key, MouseButton, Platform, ScrollPhase};
use input::frame as rframe;
use crate::geom::Rgba;
use crate::draw::{Break, Instance};
use crate::layout::MAX_TRACKS;
pub use crate::draw::Fit;
use crate::arena::{self, Arena};
pub use crate::geom::Edges;
use std::any::Any;
use pxui_text::{Metrics, Style, Text, TextSpan};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use pxui_types::ffi::PxuiFrameReasons;
mod string_pool;
mod touch;
pub mod gesture;
mod reorder;
mod promotion;
use string_pool::{EmptyStrings, Field as StringField};

impl StringField {
    fn get(self, slot: &Slot) -> &String {
        match self {Self::Text=>&slot.text,Self::Name=>&slot.a11y_name,Self::Value=>&slot.a11y_value}
    }
    fn get_mut(self, slot: &mut Slot) -> &mut String {
        match self {Self::Text=>&mut slot.text,Self::Name=>&mut slot.a11y_name,Self::Value=>&mut slot.a11y_value}
    }
}

pub use pxui_types::{Flags, Size};
pub use crate::opts::{BoxOpts, HitFn};
pub use pxui_types::shape::{clip_of, Shapes};
pub use pxui_types::ui::{
    ellipsis, text_top_for, display, hash_range, int_key, key_of, shown, Cursor, Keyframe, Keyframes, ListResponse, Overlay, PickerResponse, Repeat, SelectMode, SelectionList, Target, TreeResponse, UndoKind,
    DRAG_THRESHOLD, ROOT_KEY,
};
/// What a carry holds: a `kind` and a `value`, both strings (the engine's own storage; the wrapper lends them as `PayloadRef`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Payload {
    /// What sort of thing is carried; a drop target accepts by comparing it.
    pub kind: String,
    /// The thing itself, in the app's own encoding.
    pub value: String,
}

/// What a drop target sees of a carry this frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Drop {
    /// the pointer is over the box while a payload is held
    Hover(Payload),
    /// the payload was released over the box, this frame only
    Dropped(Payload),
}
pub use pxui_types::node::{Axis, Bar, Dir, Mark, Node};
use pxui_types::node::Transition;
use crate::bodies::{in_body, BodyTable, TipConfig};
/// Sentinel for no box in parent, child and sibling indices.
pub const NONE: usize = usize::MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Origin {
    FromPointer(i64),
    FromKey,
}

// ---------------------------------------------------------------- keys

// ------------------------------------------------------ retained cache

/// Open addressing over integer keys. Deletion closes its probe cluster so that a scrolled list
/// can replace every row without accumulating tombstones and rehashing the table.
struct SlotTable {
    /// (key, slot) side by side: a lookup touches one cache line. Key 0 is empty.
    entries: Vec<(i64, usize)>,
    live: usize,
}

impl SlotTable {
    fn create() -> SlotTable {
        SlotTable { entries: vec![(0, 0); 256], live: 0 }
    }

    fn index(&self, key: i64) -> usize {
        let mask = self.entries.len() - 1;
        let mut position = (key as usize) & mask;
        loop {
            let found = self.entries[position].0;
            if found == key || found == 0 {
                return position;
            }
            position = (position + 1) & mask;
        }
    }

    fn find(&self, key: i64) -> Option<usize> {
        if key == 0 || key == 1 { return None; }
        let (found, slot) = self.entries[self.index(key)];
        (found == key).then_some(slot)
    }

    fn add(&mut self, key: i64, slot: usize) {
        if 2 * (self.live + 1) > self.entries.len() {
            let capacity = 2 * self.entries.len();
            let entries = std::mem::replace(&mut self.entries, vec![(0, 0); capacity]);
            for &(k, s) in &entries {
                if k != 0 {
                    let position = self.index(k);
                    self.entries[position] = (k, s);
                }
            }
        }
        let position = self.index(key);
        debug_assert_eq!(self.entries[position].0, 0, "only new slots are inserted");
        self.entries[position] = (key, slot);
        self.live += 1;
    }

    fn remove(&mut self, key: i64) {
        let mut hole = self.index(key);
        if self.entries[hole].0 != key { return; }
        let mask = self.entries.len() - 1;
        let mut next = (hole + 1) & mask;
        while self.entries[next].0 != 0 {
            let home = self.entries[next].0 as usize & mask;
            let distance = |at: usize| at.wrapping_sub(home) & mask;
            if distance(hole) < distance(next) {
                self.entries[hole] = self.entries[next];
                hole = next;
            }
            next = (next + 1) & mask;
        }
        self.entries[hole] = (0, 0);
        self.live -= 1;
    }
}

// ------------------------------------------------------------- boxes

const HIT_FLAGS: Flags = Flags::CLICKABLE.or(Flags::FOCUSABLE).or(Flags::SCROLL).or(Flags::SCROLL_X).or(Flags::BLOCKING).or(Flags::CAPTURE_UNCLAIMED);

pub use crate::theme::{Bodies, BodyId, Curve, Font, Look, Part, Ring, Shadow, State, States, Theme, Tokens};
pub use crate::a11y::{A11yAction, A11yTree, Role};
use crate::a11y::{actions as a11y_actions, state as a11y_state, Action as A11yKind, NO_PARENT};
pub use crate::layout::{place_anchored, Align, Along, Anchor, Cell, Direction, Grid, Justify, Placement, Side, Track};


/// One box of this frame's build: its place in the tree, its hit shape and its canvases. What
/// outlives the frame is in the node's slot.
struct BoxRec {
    key: i64,
    slot: usize,
    parent: usize,
    first: usize,
    last: usize,
    next: usize,
    hit: Option<HitFn>,
    hit_rect: Option<Rect>,
    /// a hit shape without a closure (`Ui::hit_part`, `Ui::hit_outset`)
    shape: HitShape,
    paint: arena::List,
    over: arena::List,
    /// the view this box holds (`NO_VIEW`), and whether its build was skipped this frame
    view: u32,
    kept: bool,
    /// the `grow_line` pass that froze it at its min or max (`Ui::grow_pass`)
    frozen: u32,
    /// built in a virtual list's row past the viewport while the prepare queue runs: text the cache has not seen goes to
    /// the queue instead of shaping (`Ui::ahead`)
    ahead: bool,
}

/// A box's hit shape without a closure: its own rectangle, a rectangle relative to it (a body's part, `Ui::hit_part`) or its own
/// grown by an outset (`Ui::hit_outset`).
#[derive(Clone, Copy)]
enum HitShape {
    Own,
    Part(Rect),
    Outset(Edges),
}

impl BoxRec {
    fn blank() -> BoxRec {
        BoxRec { key: 0, slot: 0, parent: NONE, first: NONE, last: NONE, next: NONE, hit: None, hit_rect: None, shape: HitShape::Own, paint: arena::EMPTY, over: arena::EMPTY, view: NO_VIEW, kept: false, frozen: 0, ahead: false }
    }
}

/// No running transition.
const NO_TWEEN: u32 = u32::MAX;

/// A node's look, offset/opacity, and the scale of its view's contents.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Pose {
    look: Look,
    dx: f32,
    dy: f32,
    opacity: f32,
    scale: [f32; 2],
    anchor: [f32; 2],
}

/// A running transition of slot `slot` (while it has `generation`) from `from` to `to`, through
/// the keyframes `keys` (an index into `Ui::keyframes`, `NO_KEYS` for none) when it has them.
#[derive(Clone, Copy)]
struct Tween {
    slot: usize,
    generation: u32,
    start: f64,
    curve: Curve,
    from: Pose,
    to: Pose,
    keys: u32,
    /// Only the view table changes; its instance run needs no patch or steady look.
    view_only: bool,
}

const NO_KEYS: u32 = u32::MAX;

/// Seconds after an interaction during which ambient animations match the display rate.
const LOOP_IDLE: f64 = 1.0;

/// Most keyframes one `animate_keys` run holds.
pub const MAX_KEYS: usize = 8;

/// A keyframed run's stops: the pose reached at each time (seconds from the start) and the curve of
/// the segment that ends there.
#[derive(Clone, Copy)]
struct Keys {
    n: usize,
    t: [f32; MAX_KEYS],
    pose: [Pose; MAX_KEYS],
    curve: [Curve; MAX_KEYS],
    repeat: u32,
    autoreverse: bool,
    offset: f32,
    rate: pxui_types::ui::Rate,
}

impl Keys {
    fn cycle(&self) -> f64 { f64::from(self.t[self.n - 1]) * if self.autoreverse { 2.0 } else { 1.0 } }
    fn finished(&self, elapsed: f64) -> bool {
        // Preserve the existing single-run finish precision at authored f32 deadlines.
        if self.repeat == 1 && !self.autoreverse && self.offset == 0.0 { return elapsed as f32 >= self.t[self.n - 1]; }
        self.repeat != 0 && elapsed + f64::from(self.offset) >= self.cycle() * f64::from(self.repeat)
    }
    fn time(&self, elapsed: f64) -> f32 {
        if self.repeat == 1 && !self.autoreverse && self.offset == 0.0 { return elapsed.max(0.0) as f32; }
        let duration = f64::from(self.t[self.n - 1]);
        let age = if self.finished(elapsed) { self.cycle() } else { (elapsed + f64::from(self.offset)).max(0.0) % self.cycle() };
        if self.autoreverse && age > duration { (self.cycle() - age) as f32 } else { age as f32 }
    }
}

/// A transparent colour.
const CLEAR_INK: Rgba = Rgba { r: 0, g: 0, b: 0, a: 0 };

/// No view: a box that holds none, a fresh piece of a layer.
const NO_VIEW: u32 = u32::MAX;
/// The clip inside a view with a table entry: its outer clip is the entry's.
const UNBOUNDED: Rect = Rect::new(-1.0e7, -1.0e7, 2.0e7, 2.0e7);
/// Instances that fill the room left before a kept view's run: they draw nothing.
const EMPTY: Instance = Instance::DEFAULT;
/// Room left before each kept run when a layer is compacted: `SLACK` instances, or an eighth of the
/// run when that is more, so the fresh pieces between big kept runs can grow by as much again before
/// the next compaction (compactions get rarer as layers grow).
const SLACK: usize = 32;

/// A view's entry in the painter's view table (`pxui_render::View`): its instances move by `offset`,
/// their alpha scales by `opacity`, and `clip` (`[x0, y0, x1, y1]`, window points) bounds them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewEntry {
    pub offset: [f32; 2],
    pub opacity: f32,
    pub clip: [f32; 4],
    /// then `pivot + scale * transform * (p - pivot)` (row-major 2x2, window points), as the painter's View
    pub scale: f32,
    pub transform: [f32; 4],
    pub pivot: [f32; 2],
}

impl ViewEntry {
    /// Where the painter shows view point `p` (already offset).
    fn map(&self, p: (f32, f32)) -> (f32, f32) {
        let [a, b, c, d] = self.transform;
        let (x, y) = (p.0 - self.pivot[0], p.1 - self.pivot[1]);
        (self.pivot[0] + self.scale * (a * x + b * y), self.pivot[1] + self.scale * (c * x + d * y))
    }

    /// The view point the painter shows at window point `q` (before the offset); `None` when the
    /// transform has no inverse.
    fn unmap(&self, q: (f32, f32)) -> Option<(f32, f32)> {
        let [a, b, c, d] = self.transform;
        let det = (a * d - b * c) * self.scale;
        if det == 0.0 || !det.is_finite() {
            return None;
        }
        let (x, y) = (q.0 - self.pivot[0], q.1 - self.pivot[1]);
        Some((self.pivot[0] + (d * x - b * y) / det, self.pivot[1] + (a * y - c * x) / det))
    }

    fn identity(&self) -> bool {
        let [a, b, c, d] = self.transform;
        self.scale == 1.0 && a == 1.0 && b == 0.0 && c == 0.0 && d == 1.0
    }

    /// The window rectangle that covers `r` (view points, offset) once transformed.
    fn cover(&self, r: Rect) -> Rect {
        if self.identity() {
            return r;
        }
        let pts = [(r.x, r.y), (r.x + r.w, r.y), (r.x, r.y + r.h), (r.x + r.w, r.y + r.h)].map(|p| self.map(p));
        let (x0, y0) = pts.iter().fold((f32::INFINITY, f32::INFINITY), |m, p| (m.0.min(p.0), m.1.min(p.1)));
        let (x1, y1) = pts.iter().fold((f32::NEG_INFINITY, f32::NEG_INFINITY), |m, p| (m.0.max(p.0), m.1.max(p.1)));
        Rect::new(x0, y0, x1 - x0, y1 - y0)
    }
}

/// A view that asked for its backdrop blurred (`Ui::view_backdrop_blur`): the host draws layer `layer`
/// up to `at_instance`, blurs what is under `rect` (window points) by `sigma` points with
/// `pxui_render::Painter::blur`, draws the result as an image there (its corners rounded by `radii`,
/// the view box look's, and its alpha scaled by the view's composed `opacity`), then draws the rest.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlurOut {
    pub layer: usize,
    pub at_instance: u32,
    pub rect: Rect,
    pub sigma: f32,
    pub saturation: f32,
    pub radii: [f32; 4],
    pub opacity: f32,
}

impl BlurOut {
    /// Source coverage of the three-sigma taps, rounded pixel bounds and downsample blocks.
    pub fn footprint(self, scale: f32) -> Rect {
        let pad = self.sigma * 3.0 + 8.0 / scale;
        Rect::new(self.rect.x - pad, self.rect.y - pad, self.rect.w + 2.0 * pad, self.rect.h + 2.0 * pad)
    }
}

pub fn expand_blur_damage(damage: &mut DamageList, blurs: &[BlurOut], viewport: Rect, scale: f32) {
    // A newly damaged blur may feed another one earlier in this list.
    while !damage.is_full() {
        let mut changed = false;
        for blur in blurs {
            let rect = blur.rect.intersect(viewport);
            if rect.w <= 0.0 || rect.h <= 0.0 || damage.covers(rect) { continue; }
            let footprint = blur.footprint(scale);
            if damage.rects().iter().any(|r| {
                let overlap = r.intersect(footprint);
                overlap.w > 0.0 && overlap.h > 0.0
            }) {
                let before = *damage;
                damage.add(rect, viewport);
                changed |= *damage != before;
            }
        }
        if !changed { break; }
    }
}

/// `Ui::list_measured`'s cache: each row's measured (or estimated) height, the rows' tops (prefix
/// sums, `count + 1`, rebuilt when a height changes) and the frame it was last used.
#[derive(Default)]
struct RowHeights {
    heights: Vec<f32>,
    tops: Vec<f32>,
    dirty: bool,
    frame: i64,
    geometry: ListGeometry,
}

/// A cached subtree (`Ui::view`): when it was built, where its output lives and how it is shown.
struct ViewRec {
    name: String,
    slot: usize,
    /// the enclosing view (0: the root, which rebuilds every frame)
    parent: u32,
    revision: u64,
    epoch: u64,
    /// the `themed` override it was built under (0: none)
    theme: u32,
    /// frames it was last built, kept (build skipped) and reached by the paint
    built: i64,
    kept: i64,
    visited: i64,
    /// the atlas shelves its output samples (its build's glyphs, its patches', kept views inside)
    shelves: pxui_text::Shelves,
    dirty: bool,
    /// its last build put boxes at the root, asked for a frame or ran a hover delay: rebuild it
    escapes: bool,
    patched: bool,
    palette_labels: bool,
    promote: bool,
    palette_paint: bool,
    palette_shape_count: u8,
    viewport_independent: bool,
    /// a clean view rebuilt by `verify_views`: its output must equal the kept one
    verify: bool,
    /// Rebuild descendants together when this view's available size changes.
    reflow: bool,
    /// `run` in layer `layer` holds its output, painted at frame and time `kept_paint`
    valid: bool,
    layer: i64,
    run: (u32, u32),
    kept_paint: (i64, f64),
    /// this frame's pieces of it: scratch layer, first and end piece
    pieces: (usize, usize, usize),
    /// the texture in effect at the run's start, and the breaks inside it
    tex0: Option<Image>,
    breaks: Vec<(u32, Option<Image>)>,
    hits: Vec<HitRec>,
    /// the slots its last build declared, in order (the accessibility walk), and the build's
    /// before (what `prune` frees from)
    nodes: Vec<usize>,
    prev: Vec<usize>,
    hit_parent: i64,
    /// its box at its build (parent coordinates), how far layout has moved it since, and the clip
    /// at its box (parent coordinates)
    built_rect: Rect,
    /// its box's content size at its last build (the `Fit` sides' measure while it is kept)
    fit: (f32, f32),
    delta: (f32, f32),
    box_clip: Rect,
    /// its painter table index (0: none, the table was full) and the view whose entry moves its
    /// instances (itself, or with no index its parent's)
    index: u16,
    xf: u32,
    offset: (f32, f32),
    opacity: f32,
    clip: Option<Rect>,
    /// its 2D transform about `pivot` (from its box's corner, or a window point with `pivot_fixed`)
    /// and its backdrop blur (0: none)
    scale: f32,
    transform: [f32; 4],
    pivot: (f32, f32),
    pivot_fixed: bool,
    /// Resting animated X/Y scale and fractional anchor, composed after the manual transform.
    scale_pose: [f32; 4],
    blur: f32,
    saturation: f32,
    /// when its last build asked to be rebuilt (`Ui::repaint_after`; infinity: never)
    wake: f64,
    /// its nodes' run in the published accessibility tree (nested views inside it), valid while
    /// `a11y_gen` is the tree's, and the shift its rects were taken under
    a11y_span: (u32, u32),
    a11y_gen: u64,
    /// the placement its rects were taken under: shift, scale, transform, pivot, composed affine
    a11y_shift: [f32; 19],
    /// where its span started in the published tree when this walk copied it (`u32::MAX`: emitted)
    a11y_from: u32,
    /// the composed entry and the screen rectangle it shows
    shift: (f32, f32),
    alpha: f32,
    outer: Rect,
    screen: Rect,
    stamp: u64,
    /// the transforms of the views around it composed (`q = pa * p + pb`, window points), applied
    /// after its own: identity unless an enclosing view is scaled or transformed
    pa: [f32; 4],
    pb: [f32; 2],
    /// no transform of its own or around it (composed): hit testing skips the inverse
    flat: bool,
}

const IDENTITY_2X2: [f32; 4] = [1.0, 0.0, 0.0, 1.0];

impl ViewRec {
    fn new() -> ViewRec {
        ViewRec {
            name: String::new(),
            nodes: Vec::new(),
            prev: Vec::new(),
            slot: NONE,
            parent: 0,
            revision: 0,
            epoch: 0,
            theme: 0,
            built: -1,
            kept: -1,
            visited: -1,
            shelves: [0; 8],
            dirty: false,
            escapes: false,
            patched: false,
            palette_labels: false,
            promote: false,
            palette_paint: false,
            palette_shape_count: 0,
            viewport_independent: false,
            verify: false,
            reflow: false,
            valid: false,
            layer: 0,
            run: (0, 0),
            kept_paint: (-1, 0.0),
            pieces: (0, 0, 0),
            tex0: None,
            breaks: Vec::new(),
            hits: Vec::new(),
            hit_parent: 0,
            built_rect: Rect::new(0.0, 0.0, 0.0, 0.0),
            fit: (0.0, 0.0),
            delta: (0.0, 0.0),
            box_clip: UNBOUNDED,
            index: 0,
            xf: 0,
            offset: (0.0, 0.0),
            opacity: 1.0,
            clip: None,
            scale: 1.0,
            transform: [1.0, 0.0, 0.0, 1.0],
            pivot: (0.0, 0.0),
            pivot_fixed: false,
            scale_pose: [1.0, 1.0, 0.5, 0.5],
            blur: 0.0,
            saturation: 1.0,
            wake: f64::INFINITY,
            a11y_span: (0, 0),
            a11y_gen: 0,
            a11y_from: u32::MAX,
            a11y_shift: [0.0; 19],
            shift: (0.0, 0.0),
            alpha: 1.0,
            outer: UNBOUNDED,
            screen: Rect::new(0.0, 0.0, 0.0, 0.0),
            stamp: 0,
            pa: IDENTITY_2X2,
            pb: [0.0; 2],
            flat: true,
        }
    }

    /// No scale or transform of its own.
    fn plain(&self) -> bool {
        let [a, b, c, d] = self.transform;
        self.scale == 1.0 && a == 1.0 && b == 0.0 && c == 0.0 && d == 1.0
    }

    /// No enclosing view transforms it.
    fn parent_plain(&self) -> bool {
        self.pa == IDENTITY_2X2 && self.pb == [0.0; 2]
    }

    /// Its own and its enclosing transforms as one affine `(a, b)`: `q = a * p + b` for `p` in
    /// offset view points.
    fn affine(&self) -> ([f32; 4], [f32; 2]) {
        let e = self.own_entry();
        let [m0, m1, m2, m3] = e.transform.map(|m| m * e.scale);
        // own: q = m p + (I - m) pivot
        let (b0, b1) = (e.pivot[0] - (m0 * e.pivot[0] + m1 * e.pivot[1]), e.pivot[1] - (m2 * e.pivot[0] + m3 * e.pivot[1]));
        let [p0, p1, p2, p3] = self.pa;
        let a = [p0 * m0 + p1 * m2, p0 * m1 + p1 * m3, p2 * m0 + p3 * m2, p2 * m1 + p3 * m3];
        (a, [p0 * b0 + p1 * b1 + self.pb[0], p2 * b0 + p3 * b1 + self.pb[1]])
    }

    /// Where the painter shows offset view point `p`.
    fn map(&self, p: (f32, f32)) -> (f32, f32) {
        let q = self.own_entry().map(p);
        if self.parent_plain() {
            return q;
        }
        let [a, b, c, d] = self.pa;
        (a * q.0 + b * q.1 + self.pb[0], c * q.0 + d * q.1 + self.pb[1])
    }

    /// The offset view point the painter shows at window point `q`; `None` without an inverse.
    fn unmap(&self, q: (f32, f32)) -> Option<(f32, f32)> {
        let q = if self.parent_plain() {
            q
        } else {
            let [a, b, c, d] = self.pa;
            let det = a * d - b * c;
            if det == 0.0 || !det.is_finite() {
                return None;
            }
            let (x, y) = (q.0 - self.pb[0], q.1 - self.pb[1]);
            ((d * x - b * y) / det, (a * y - c * x) / det)
        };
        if self.plain() { Some(q) } else { self.own_entry().unmap(q) }
    }

    /// The window rectangle covering offset view rectangle `r` once transformed.
    fn cover(&self, r: Rect) -> Rect {
        if self.parent_plain() {
            return if self.plain() { r } else { self.own_entry().cover(r) };
        }
        let pts = [(r.x, r.y), (r.x + r.w, r.y), (r.x, r.y + r.h), (r.x + r.w, r.y + r.h)].map(|p| self.map(p));
        let (x0, y0) = pts.iter().fold((f32::INFINITY, f32::INFINITY), |m, p| (m.0.min(p.0), m.1.min(p.1)));
        let (x1, y1) = pts.iter().fold((f32::NEG_INFINITY, f32::NEG_INFINITY), |m, p| (m.0.max(p.0), m.1.max(p.1)));
        Rect::new(x0, y0, x1 - x0, y1 - y0)
    }

    /// The painter's entry: its own transform, or inside a transformed view the composed affine
    /// about the origin, the offset carrying its translation (`a * (p + o) = a * p + b`).
    fn entry(&self) -> ViewEntry {
        let e = self.own_entry();
        if self.parent_plain() {
            return e;
        }
        let (a, b) = self.affine();
        let det = a[0] * a[3] - a[1] * a[2];
        if det == 0.0 || !det.is_finite() {
            return ViewEntry { opacity: 0.0, ..e };
        }
        let (x, y) = ((a[3] * b[0] - a[1] * b[1]) / det, (a[0] * b[1] - a[2] * b[0]) / det);
        ViewEntry { offset: [e.offset[0] + x, e.offset[1] + y], scale: 1.0, transform: a, pivot: [0.0, 0.0], ..e }
    }

    /// Its entry with its own transform alone.
    fn own_entry(&self) -> ViewEntry {
        let c = clip_of(self.outer);
        let b = self.built_rect;
        let pivot = if self.pivot_fixed { [self.pivot.0, self.pivot.1] } else { [b.x + self.shift.0 + self.pivot.0, b.y + self.shift.1 + self.pivot.1] };
        ViewEntry {
            offset: [self.shift.0, self.shift.1],
            opacity: self.alpha,
            clip: [c[0] as f32, c[1] as f32, c[2] as f32, c[3] as f32],
            scale: self.scale,
            transform: self.transform,
            pivot,
        }
    }
}

/// Color sources in a kept canvas. Indices are relative to the view's run after placement.
#[derive(Clone, Copy, Default)]
struct PaletteShape {
    at: u32,
    end: u32,
    fill: Rgba,
    stroke: Rgba,
}

// Large canvases rebuild. Small views can recycle between kinds without growing storage in a warm frame.
const PALETTE_SHAPES: usize = 16;

/// A run of a layer: fresh instances `a..b` of the frame's scratch layer, or a kept view's run;
/// `at` is where it lands in the retained layer.
#[derive(Clone, Copy)]
struct Piece {
    view: u32,
    a: u32,
    b: u32,
    at: u32,
}

/// A node handle: its index in this frame's build (`NONE` for `box_of_key`), key, retained slot and
/// the slot's generation. The slot and generation are stable while the node lives; a handle to a
/// node that is gone is stale, and the engine checks it before touching the slot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
// C layout: the wrapper's handle (`PxuiBox`) byte for byte
#[repr(C)]
pub struct UiBox {
    pub index: usize,
    pub box_key: i64,
    pub box_slot: usize,
    pub generation: u32,
    /// Serial of the `Ui` that made the handle (the ABI's `PxuiBox.ui`): nonzero, so a zeroed handle names no `Ui`. The ABI layer
    /// compares it with the engine's own (`Ui::serial`) where a caller's handle enters; the engine's own handles carry its serial.
    pub ui: u32,
}

pub type Handle = UiBox;

pub fn key(b: &UiBox) -> i64 {
    b.box_key
}

/// The spans of one styled node: a range of `Ui::span_arena`.
#[derive(Clone, Copy, Default)]
struct SpanRange { start: u32, len: u32 }
struct NodeText { style:Style, spans:SpanRange, line_box:bool }
impl NodeText {
    fn spans<'a>(&self, arena: &'a [TextSpan]) -> &'a [TextSpan] { &arena[self.spans.start as usize..][..self.spans.len as usize] }

    /// The top of the em box when the text does not use CSS line boxes.
    fn top(&self,y:f32,h:f32,metrics:Metrics)->f32 {
        let line=if self.style.line_height>0.0 { self.style.line_height } else { self.style.size*1.2 };
        let block=self.style.size+metrics.lines.saturating_sub(1) as f32*line;
        y+((h-block)/2.0).floor()
    }
    /// A line box's first baseline: its lines centred in `h` from `y`, `metrics.baseline` (the rounded primary-face ascent plus the
    /// floored half-leading, from `pxui_text`) below the first line's top.
    fn line_box_baseline(&self,y:f32,h:f32,metrics:Metrics)->f32 {
        y+(h-self.style.line_height*metrics.lines.max(1) as f32)/2.0+metrics.baseline
    }
    fn style(&self,node:&Node,rtl:bool)->Style {
        let mut style=self.style;
        if node.flags.any(Flags::FIT_TEXT) && style.ellipsis==pxui_text::Ellipsis::None { style.ellipsis=pxui_text::Ellipsis::End; }
        if rtl && style.align==pxui_text::Align::Left { style.align=pxui_text::Align::Right; }
        style
    }
}

/// The bit of `theme_id` set inside a `disabled` scope (the low byte numbers `themed` overrides).
const DISABLED_ID: u32 = 0x100;

#[derive(Clone, Copy, PartialEq, Eq)]
enum NodeTemplate {
    Custom,
    View,
    PositionedLabel,
}

/// A template stays reusable until an author or the engine mutates any of its node fields.
struct SlotNode {
    value: Node,
    template: NodeTemplate,
}

impl SlotNode {
    fn prepare(&mut self, template: NodeTemplate) -> &mut Node {
        if self.template != template {
            self.value = Node::DEFAULT;
            if template == NodeTemplate::PositionedLabel {
                self.value.role = Role::Label;
            }
            self.template = template;
        }
        &mut self.value
    }
}

impl std::ops::Deref for SlotNode {
    type Target = Node;
    fn deref(&self) -> &Node { &self.value }
}

impl std::ops::DerefMut for SlotNode {
    fn deref_mut(&mut self) -> &mut Node {
        self.template = NodeTemplate::Custom;
        &mut self.value
    }
}

struct Slot {
    key: i64,
    new_identity: bool,
    /// bumped when the slot is freed: handles to the node it held are stale from then on
    generation: u32,
    touched: i64,
    /// the frame and the key of the last duplicate of this key declared in it (`unique_key`)
    dup: (i64, i64),
    node: SlotNode,
    text: String,
    /// Last layout key and text fingerprint survive builds and virtual-slot recycling.
    text_layout: std::cell::Cell<Option<(u64, u64)>>,
    rich: Option<NodeText>,
    label_color: Option<pxui_types::theme::ColorToken>,
    label_root_ink: bool,
    /// where the node's look was painted: the layer's key, the instance range, the clip, and the
    /// frame
    range_layer: i64,
    range: (u32, u32),
    /// empty instances after `range` a patch may grow into (left by a patch that grew it)
    room: u32,
    clip: Rect,
    clip_radius: f32,
    painted: i64,
    look_needs_snapshot: bool,
    /// the view that declares the node, and the view the node holds (`NO_VIEW`)
    owner: u32,
    view: u32,
    /// the virtual list (box key; 0: none) and row this node is the box of, for the text cache's list ownership
    row_of: (i64, u32),
    /// whether this node's children are a virtual list's rows (`row_of`): only its paint scopes the text cache per child
    holds_rows: bool,
    /// the frame in which the app asked this node about a carried payload (`Ui::drop_target_ref`): the node takes the pointer's hit
    /// that frame whatever its flags, so a drop reaches any node
    drop_zone: i64,
    /// a reorderable list's row: assistive technology is offered Move up and Move down on it
    movable: bool,
    rx: f32,
    ry: f32,
    rw: f32,
    rh: f32,
    state_value: Option<i64>,
    reveal: usize,
    text_value: Option<String>,
    labels: Vec<Rc<str>>,
    label_count: usize,
    press_time: f64,
    press_x: f32,
    press_y: f32,
    /// consecutive left presses within 0.35 s and 5 points
    press_count: i64,
    /// the look and state last painted while some theme animates (what a transition starts from),
    /// the running transition (`NO_TWEEN`), and where `animate` left the node: offset and opacity
    seen: Look,
    seen_state: State,
    tween: u32,
    pose: (f32, f32, f32),
    /// the slot of the node it was declared under (`NONE` for the root); its accessible name and
    /// value (the text when the name is empty) and its text selection (anchor, caret in chars)
    parent_slot: usize,
    a11y_name: String,
    a11y_value: String,
    a11y_selection: (u32, u32),
    /// a numeric value (now, min, max, step), over the text one
    a11y_number: Option<[f64; 4]>,
}

impl Slot {
    fn fresh() -> Slot {
        Slot {
            key: 0,
            new_identity: false,
            generation: 0,
            touched: -1,
            dup: (-1, 0),
            node: SlotNode { value: Node::DEFAULT, template: NodeTemplate::Custom },
            text: String::new(),
            text_layout: std::cell::Cell::new(None),
            rich:None,
            label_color: None,
            label_root_ink: false,
            range_layer: 0,
            range: (0, 0),
            room: 0,
            clip: Rect::new(0.0, 0.0, 0.0, 0.0),
            clip_radius: 0.0,
            painted: -1,
            look_needs_snapshot: false,
            owner: 0,
            view: NO_VIEW,
            row_of: (0, 0),
            holds_rows: false,
            drop_zone: -1,
            movable: false,
            rx: 0.0,
            ry: 0.0,
            rw: 0.0,
            rh: 0.0,
            state_value: None,
            reveal: usize::MAX,
            text_value: None,
            labels: Vec::new(),
            label_count: 0,
            press_time: f64::NEG_INFINITY,
            press_x: 0.0,
            press_y: 0.0,
            press_count: 0,
            seen: Look::NONE,
            seen_state: State::default(),
            tween: NO_TWEEN,
            pose: (0.0, 0.0, 1.0),
            parent_slot: NONE,
            a11y_name: String::new(),
            a11y_value: String::new(),
            a11y_number: None,
            a11y_selection: (0, 0),
        }
    }
}

/// A hit rectangle in the coordinates of view `view`'s instances: its entry moves and clips it.
#[derive(Clone, Copy)]
struct HitRec {
    key: i64,
    parent: i64,
    flags: Flags,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    view: u32,
}

/// One axis of a scroll box in motion: the overscroll (input travel `raw`, shown as `visual`), the
/// last input and frame times, and a trackpad's coast or its bounce off an edge.
#[derive(Clone, Copy)]
struct Glide {
    raw: f64,
    visual: f64,
    event_time: f64,
    frame_time: f64,
    /// 0 neither, 1 trackpad coast, 2 bounce, 3 finger coast
    mode: i64,
    velocity: f64,
    start: f64,
    from: f64,
}

impl Glide {
    const REST: Glide = Glide { raw: 0.0, visual: 0.0, event_time: f64::NEG_INFINITY, frame_time: 0.0, mode: 0, velocity: 0.0, start: 0.0, from: 0.0 };
    fn moving(&self) -> bool {
        self.mode != 0 || (self.visual != 0.0 && self.event_time != f64::INFINITY)
    }
}

struct ExpectedSize {
    slot: usize,
    w: f32,
    h: f32,
    grid: Option<GridFit>,
}

struct GridFit {
    extent: f32,
    across: f32,
    per: usize,
    padding: Edges,
    sideways: bool,
}

fn grid_space(w: f32, h: f32, padding: Edges, sideways: bool) -> (f32, f32) {
    let (w, h) = (w - padding.l - padding.r, h - padding.t - padding.b);
    if sideways { (w, h) } else { (h, w) }
}

/// What a press, a drag, a scroll or a key did to a box this frame.
#[derive(Clone)]
pub struct Accumulator {
    pub pressed: bool,
    pub subtree_press: Option<i64>,
    pub released: bool,
    pub clicked: bool,
    pub double_clicked: bool,
    pub clicks: i64,
    pub moved: bool,
    pub drag_x: f32,
    pub drag_y: f32,
    pub press_point: (f32, f32),
    pub release_point: (f32, f32),
    pub button: Option<MouseButton>,
    pub scroll_x: f32,
    pub scroll_y_steps: f32,
    /// what moves a scroll box: an unphased wheel's steps, the fingers' points on a trackpad, and
    /// whether they touched or lifted this frame
    pub scroll_wheel: f32,
    /// an unphased wheel's sideways steps (a `SCROLL_X` box)
    pub scroll_wheel_x: f32,
    pub scroll_drag: f32,
    /// the fingers' sideways points (a `SCROLL_X` box)
    pub scroll_drag_x: f32,
    /// a value an assistive technology set (`A11yAction` SetValue)
    pub a11y_value: Option<f64>,
    /// a selection an assistive technology set (`A11yAction` Select): anchor and caret in chars
    pub a11y_select: Option<(u32, u32)>,
    /// a row move an assistive technology asked for (`A11yAction` MoveUp -1, MoveDown 1)
    pub a11y_move: i8,
    pub scroll_touch: bool,
    pub scroll_lift: bool,
    pub(crate) scroll_velocity: (f64, f64),
    pub(crate) scroll_finger: bool,
    /// product of this frame's pinch factors
    pub pinch: f64,
    pub keys: Vec<(Event, Vec<Key>)>,
    pub press_keys: Vec<Key>,
}

/// A host texture with its size in pixels; PXUI only binds it to the instances that sample it.
#[derive(Clone)]
pub struct Image {
    pub handle: Rc<dyn Any>,
    pub width: i64,
    pub height: i64,
}

/// One paint layer: the base UI, then a layer per floating root (`key` is its box key; 0 for the
/// base). Image instances sample `images[break.image]` from each break on. A layer is retained: a
/// frame rewrites only the instances that changed, and a kept view's run stays where it is.
/// `dirty` gathers the `[start, end)` instance ranges written since the host last called
/// `Ui::drawn` for this layer; `id` names the layer's storage (stable while it lives, below 64), the
/// painter's retained layer index.
#[derive(Default)]
pub struct Layer {
    pub key: i64,
    pub id: u32,
    /// what the layer is (`LayerRole::Base` for layer 0 and an app's `to_front` root): a host may present a popup, modal, tooltip or
    /// drag layer in a surface of its own instead of the window's
    pub role: LayerRole,
    /// where a floating layer is placed, in window points: the rectangle its root was anchored to (`Node::anchor`), else the root's
    /// own rectangle; empty for the base
    pub anchor: Rect,
    pub instances: Vec<Instance>,
    pub breaks: Vec<Break>,
    pub images: Vec<Image>,
    pub dirty: Vec<(u32, u32)>,
    pub bounds: Vec<[f32; 4]>,
    pub runs: Vec<CullRun>,
    pieces: Vec<Piece>,
    live: bool,
}

impl Layer {
    fn new(key: i64, id: u32) -> Layer {
        Layer { key, id, role: LayerRole::Base, anchor: Rect::default(), instances: Vec::with_capacity(256), breaks: Vec::new(), images: Vec::new(), dirty: Vec::new(), bounds: Vec::new(), runs: Vec::new(), pieces: Vec::new(), live: false }
    }
}

/// A contiguous paint piece sharing a renderer view. Bounds are in that view's points.
#[derive(Clone, Copy)]
pub struct CullRun {
    pub from: u32,
    pub to: u32,
    pub view: u16,
    pub bounds: [f32; 4],
}

pub const EMPTY_BOUNDS: [f32; 4] = [f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY];

fn union_bounds(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])]
}

fn fresh_runs(out: &mut Vec<CullRun>, layer: &Layer, from: u32, to: u32) {
    let mut run = None::<CullRun>;
    for at in from..to {
        let bounds = layer.bounds[at as usize];
        if bounds == EMPTY_BOUNDS { continue; }
        let view = layer.instances[at as usize].view;
        if let Some(current) = &mut run && current.view == view {
            current.to = at + 1;
            current.bounds = union_bounds(current.bounds, bounds);
        } else if let Some(previous) = run.replace(CullRun { from: at, to: at + 1, view, bounds }) {
            out.push(previous);
        }
    }
    out.extend(run);
}

fn kept_runs(out: &mut Vec<CullRun>, old: &[CullRun], from: u32, to: u32, at: u32) {
    let first = old.partition_point(|run| run.to <= from);
    for run in old[first..].iter().take_while(|run| run.from < to) {
        out.push(CullRun {
            from: at + (run.from.max(from) - from),
            to: at + (run.to.min(to) - from),
            ..*run
        });
    }
}

/// What a paint layer holds: the window's content, or a floating root the engine raised over it. Every popup, modal, tooltip and drag
/// root is a layer of its own, so a host that can (a compositor) presents it in a child surface that may overflow the window; one
/// that cannot draws the layers in order into the window, which looks the same. Bounded by `pxui_types::MAX_LAYERS`: the roots from
/// the last layer's on share it, as `LayerRole::Merged`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LayerRole {
    /// the window's content, and a root an app raised with `Ui::to_front`
    #[default]
    Base,
    /// a popup or menu (`Ui::popup`, `Ui::anchored`, a menu or completion list): dismissed by a press outside it
    Popup,
    /// a modal sheet (`Ui::modal`, a dialog): it takes the input while open
    Modal,
    /// a tooltip near the pointer
    Tooltip,
    /// what a drag carries, under the pointer
    Drag,
    /// the last layer when more floating roots than `pxui_types::MAX_LAYERS` allows paint in a frame: every root from the one that opened
    /// it on, in order, whatever their roles; its anchor is the union of theirs. A host presents it in the window.
    Merged,
}

/// Add `[a, b)` to sorted, disjoint `ranges` (`draw::add_range`).
#[inline]
fn add_range(ranges: &mut Vec<(u32, u32)>, a: u32, b: u32) {
    crate::draw::add_range(ranges, a, b, DIRTY_RANGES);
}

/// Dirty ranges a layer keeps before the closest two merge.
const DIRTY_RANGES: usize = 64;

/// What the frame asks of the window system; the host applies it after the frame.
#[derive(Default)]
pub struct PlatformOut {
    /// the pointer shape a hovered or captured control asked for
    pub cursor: Option<Cursor>,
    /// the focused text's input area and the caret's offset in it, for the input method
    pub ime: Option<((i64, i64, i64, i64), i64)>,
    /// the ground the UI is painted on, on the frame it changed (the macOS title bar)
    pub background: Option<Rgba>,
    /// text to put on the clipboard
    pub clipboard: Option<String>,
    /// the UI wants the clipboard's text: the host sends `Event::Paste` with the next frame
    pub paste: bool,
    /// when the UI needs its next frame with no input (`Ui::repaint_at`)
    pub repaint_at: Option<f64>,
}

/// The completed frame: its paint layers, the platform requests, what changed on screen since the
/// output before (`damage`, window points; empty when nothing did) and the view table entries the
/// layers' instances name (the host sets each with `pxui_render::Painter::set_view`).
pub struct FrameOut<'a> {
    pub draw: &'a [Layer],
    pub platform: &'a PlatformOut,
    pub damage: &'a DamageList,
    pub views: &'a [(u16, ViewEntry)],
    pub blurs: &'a [BlurOut],
    pub promoted: &'a [pxui_types::ffi::PxuiPromoted],
    /// the accessibility tree (empty unless `Ui::set_a11y(true)`): its `revision` changes only when
    /// its content did
    pub a11y: &'a A11yTree,
}

// -------------------------------------------------------------- text

fn lay_out_spans(t:&mut Text,style:&Style,width:f32,text:&str,spans:&[TextSpan])->Option<(u64,Metrics)> {
    let key=pxui_text::content_key(style,width,text,spans);
    t.layout_spans(key,text,style,width,spans).map(|metrics|(key,metrics))
}

/// `points` rounded half away from zero to whole points, at least 1: the integer kit's view of a fractional size.
pub(crate) fn round_points(points: f32) -> i64 {
    (points.round() as i64).max(1)
}

fn plain_style(look:&Look,size:f32,flags:Flags)->Style {
    Style {font:look.font.family,size,weight:look.font.weight,
        tracking:look.font.tracking*size,
        ellipsis:if flags.any(Flags::FIT_TEXT) {pxui_text::Ellipsis::End} else {pxui_text::Ellipsis::None},
        ..Style::default()}
}

/// Kit text: the UI face (font 0) at `size` points with `tracking` points after every glyph.
pub fn kit_style(size: i64, tracking: f32) -> Style {
    Style { size: size as f32, tracking, ..Style::default() }
}

/// The top of text `size` points tall centred (floored) in a row at `y` of height `h`: `text_top_for` with a fractional size.
pub(crate) fn text_top_at(size: f32, y: f32, h: f32) -> f32 {
    y + ((h - size) / 2.0).floor()
}

/// `font` at the integer kit's whole points, for a body of `body` whole points.
pub(crate) fn font_px(font: &Font, body: i64) -> i64 {
    round_points(font.points(body as f32))
}

/// Lay `text` out (a cache hit when unchanged) and return its key and metrics.
pub(crate) fn lay_out(t: &mut Text, style: &Style, width: f32, text: &str) -> (u64, Metrics) {
    let key = pxui_text::content_key(style, width, text, &[]);
    (key, t.layout(key, text, style, width))
}

/// The x of the caret before byte `index` of single-line kit text, in points from its start.
fn caret_x(t: &mut Text, size: i64, text: &str, index: usize) -> f32 {
    let (key, _) = lay_out(t, &kit_style(size, 0.0), 0.0, text);
    t.caret(key, index.min(text.len())).x
}

// ----------------------------------------------------------------- Ui

type Palette = [Rgba; pxui_types::theme::ColorToken::LIMIT];

#[repr(C)]
pub struct Ui {
    /// see `Ui::serial`
    ui_serial: u32,
    /// `themed` overrides in effect (`themes[..theme_depth]`, storage kept), the distinct themes views
    /// were built under (an id is an index) and the id in effect
    // boxed: an entry stays where it is while deeper ones are pushed, so a pointer to the theme in effect (`theme`) stays valid
    // for as long as its scope lasts (the library's handles cache it)
    themes: Vec<Box<(Theme, Option<Palette>)>>,
    theme_depth: usize,
    theme_ids: Vec<(Theme, Option<Palette>)>,
    /// the root theme as `set_theme` was given it, kept only when its looks name colour tokens (`theme` holds them resolved), so a
    /// colour table change resolves it again
    theme_raw: Option<Box<Theme>>,
    repaint_palette: Option<(u64, u64)>,
    theme_id: u32,
    /// this `Ui`'s number among the process's, for `theme_key`: from one process-wide counter, per process by design (an atomic `u64`
    /// taken once per `Ui`, so thread-safe and never wrapping; it holds no other state)
    serial: u64,
    /// running transitions, the frame's time, and whether a theme in effect animates
    tweens: Vec<Tween>,
    /// keyframe stops of running `animate_keys` runs, and the free ones (reused: no allocation warm)
    keyframes: Vec<Keys>,
    free_keyframes: Vec<u32>,
    now: f64,
    motion: bool,
    environment:pxui_types::Environment,
    keyboard: pxui_types::Keyboard,
    system_appearance:u32,
    system_reduced_motion:bool,
    /// the app's colour tokens, the light table then the dark one (`set_colors`): a painter copies one whole; unset entries are transparent
    colors: [Palette; 2],
    theme_colors: Option<Palette>,
    theme_reduced_motion:bool,
    font_size: i64,
    /// The body size exactly as created or set (`font_size` is it rounded, for the integer kit and the 1.x C getters).
    font_points: f32,
    platform: Platform,
    /// the text engine; `Paint` holds it while a frame paints
    /// boxed: it moves into each frame's `Paint` and back, a pointer instead of its 2.3 KiB
    text: Option<Box<Text>>,
    table: SlotTable,
    slots: Vec<Slot>,
    empty_strings: EmptyStrings,
    /// The spans of every styled node, one range each (`NodeText::spans`): one arena for the `Ui`, reused across frames. A node's spans are
    /// overwritten in place when their count is unchanged, else appended; freed ranges are dead space that `store_spans` compacts away
    /// (into `span_spare`, kept at the arena's capacity) once the arena is full and at least half dead, so a warm frame allocates nothing.
    span_arena: Vec<TextSpan>,
    span_spare: Vec<TextSpan>,
    /// the spans the nodes' ranges cover
    span_live: usize,
    free: Vec<usize>,
    /// slots whose scroll may be in motion (`apply_scroll` adds, `prune` drops the settled), and
    /// `prune`'s scratch
    moving: Vec<usize>,
    gone: Vec<usize>,
    scroll_y: Vec<f32>,
    /// per slot and axis (down, sideways), for overlay scrollers (`Environment::scrollers`): the offset its thumb last saw (NaN
    /// before its first paint) and when it last moved
    thumbs: Vec<[(f32, f64); 2]>,
    /// overlay thumbs painted, by slot, and the time each hides: the view showing one repaints then; and the painter's scratch
    thumb_hides: Vec<(usize, f64)>,
    thumb_shown: Vec<(usize, f64)>,
    // per-frame boxes
    count: usize,
    boxes: Vec<BoxRec>,
    l_x: Vec<f32>,
    l_y: Vec<f32>,
    l_w: Vec<f32>,
    l_h: Vec<f32>,
    l_content: Vec<f32>,
    l_content_w: Vec<f32>,
    /// the horizontal scroll of each retained slot (`SCROLL_X`)
    scroll_xs: Vec<f32>,
    /// each slot's scroll motion down (0) and sideways (1), beside the slots (scans of the slots
    /// stay small)
    glides: [Vec<Glide>; 2],
    gestures: Vec<gesture::GSlot>,
    /// slots whose swipe or back spring is running
    gsettle: Vec<usize>,
    /// boxes the last layout ran over; row flow direction
    arranged: usize,
    dir: Direction,
    /// the published accessibility tree, the next one being built (published when it differs), the
    /// slot-to-tree-index map of the walk, and the actions a bridge handed in for the next frame
    a11y: A11yTree,
    a11y_next: A11yTree,
    a11y_map: Vec<u32>,
    a11y_pending: Vec<A11yAction>,
    /// something the tree shows may have changed outside the root's build (a view built, moved or
    /// was patched, an action came in), and the focus and capture the last walk saw
    a11y_dirty: bool,
    /// the published tree's generation (views' spans in it are valid while theirs matches), a
    /// change no span can carry (a view moved), and the nodes the last walk computed (not copied)
    a11y_gen: u64,
    a11y_full: bool,
    a11y_written: usize,
    pub(crate) a11y_on: bool,
    /// where a carried payload would land in the last reorderable list built (its insertion index) and whether it was dropped there
    pub(crate) list_drop: Option<(usize, bool)>,
    /// the row move the last reorderable list built reported (`ui_core::reorder`), the drag in progress and the rows animating
    pub(crate) list_moved: Option<(usize, usize)>,
    reorder: reorder::Reorder,
    /// the handle (`Part::HANDLE`) the building reorderable row made, how many reorderable rows are building, and whether the next
    /// section list's rows move (a selection list's)
    row_handle: Option<Handle>,
    movable_rows: u32,
    pub(crate) row_moves: bool,
    /// sortable headings' composed accessible names (`ui_lists`), bounded
    pub(crate) sort_names: crate::ui_lists::SortNames,
    timing: bool,
    a11y_seen: (i64, Option<i64>),
    /// how long the last frame's layout passes took
    layout_time: std::time::Duration,
    phases: [std::time::Duration; PHASES],
    phase_at: Option<std::time::Instant>,
    phase_counter: Option<fn() -> Option<u64>>,
    phase_counts: Option<[u64; PHASES]>,
    counter_at: Option<u64>,
    /// `order_stack`'s scratch: (z, build position, box)
    z_order: Vec<(i16, u32, usize)>,
    // build state
    parents: Vec<usize>,
    /// the frame's floating roots and their layer roles, kept until its paint: raised by the engine (painted last, in call order)
    /// and by `to_front` (order, root, role)
    overlays: Vec<(usize, LayerRole)>,
    foreground: Vec<(i64, usize, LayerRole)>,
    seeds: Vec<i64>,
    building: bool,
    /// the rows being built are past the viewport (`BoxRec::ahead`)
    ahead: bool,
    /// the rows being built are outside the viewport, so culled: a kept view among them stays kept without a run (`view_entry`)
    out_of_view: bool,
    /// the frame a theme swap transitions in (`set_theme`)
    swap_frame: i64,
    frame_number: i64,
    first_build: i64,
    /// the last frame that made any node a drop zone (`drop_target_ref` while a payload is carried): the hit pass reads a node's
    /// slot for its zone only in such a frame
    drop_zones: i64,
    density: i64,
    kit_row_height: i64,
    /// The kit row exactly (`kit_row_height` is it rounded): the theme's row in a frame, `font_points + 11` (at least 24) between.
    row_points: f32,
    closed_section: usize,
    kit_window: bool,
    // input
    pointer: (f32, f32),
    hot: i64,
    hover_rest: Option<(HoverId, (f32, f32), f64, i64)>,
    active: Option<i64>,
    active_button: MouseButton,
    active_press: (f32, f32),
    active_keys: Vec<Key>,
    focus: i64,
    keyboard_focus: bool,
    composition: String,
    composition_selection: (i64, i64),
    previous_keys: Vec<Key>,
    route_keys: Vec<Key>,
    frame_modifiers: u32,
    pub(crate) edit_focus: i64,
    /// the box a key press takes as its focus while no node holds it (`focus_default`), stated by the last build
    default_focus: i64,
    pub(crate) edit_owner: i64,
    pub(crate) text_input_options: pxui_types::TextTraits,
    pub edit_value: String,
    pub(crate) edit_work: String,
    /// the text `search_field`, `value_field` and `text_area_submit` return borrowed: valid until the next call on the Ui
    pub(crate) returned: String,
    /// The box of the text input widget (text field, line field, search field, text area, value field) built last.
    pub(crate) input_box: Option<Handle>,
    /// What each text area keeps between frames (bounded, see `AreaCacheEntry`).
    pub(crate) area_cache: Vec<crate::ui_text::AreaCacheEntry>,
    /// A text table cell's text, as its callback gave `item_text` (kept between frames), and whether it gave one.
    pub(crate) text_cell: String,
    pub(crate) text_cell_set: bool,
    pub(crate) edit_caret: usize,
    pub(crate) edit_anchor: usize,
    pub(crate) edit_scroll_x: f32,
    window_ground: Option<Rgba>,
    gesture_key: i64,
    gesture_time: f64,
    /// the fingers' release velocity, down and sideways
    gesture_velocity: (f64, f64),
    touches: touch::Touches,
    pub edit_undo: Vec<(String, usize, usize)>,
    pub(crate) text_undo_held: i64,
    /// What the host last charged to its memory ledger for this UI; PXUI never reads it.
    pub charged: Bytes,
    pub(crate) text_rows_seen: Vec<(Option<usize>, Rc<str>, Rc<Vec<(usize, usize, usize)>>)>,
    pub(crate) text_panes: usize,
    pub(crate) text_colors_seen: Vec<(Tokens, String, Rc<Vec<(i64, i64, Rgba)>>)>,
    pub(crate) edit_redo: Vec<(String, usize, usize)>,
    pub(crate) edit_group: i64,
    pub(crate) edit_shelf: Vec<ShelfEntry>,
    /// the picker's rows, kept between frames so `picker_filled` reuses their strings
    pub(crate) picker_rows: pxui_types::ui::PickRows,
    /// the table's column stops, kept between frames
    pub(crate) table_stops: Vec<f32>,
    /// the bodies apps registered for the widget kinds (`register_toggle_body` and siblings), by kind; empty until one is.
    /// Boxed: a Vec per kind inline grows `Ui` with every kind and moves the fields placed after it, and a hot field moved
    /// past an aarch64 load's offset range costs an address instruction on every node (B2a's four kinds: +510 a Layout frame)
    pub(crate) bodies: Box<BodyTable>,
    /// the key of the widget whose body is building: `part` derives its keys from it
    pub(crate) part_owner: i64,
    /// the last duplicate key taken: (frame, the key duplicated, its first slot, the duplicate taken), for `unique_key_repeated`
    last_dup: (i64, i64, usize, i64),
    pub(crate) edit_unit: Option<(usize, usize, i64)>,
    pub(crate) edit_column: f32,
    pub(crate) scrub_origin: Option<(i64, String)>,
    payload: Option<(Payload, Origin)>,
    dropped: Option<(Payload, i64)>,
    spare_payload: Option<Payload>,
    carry_fraction: [f64; 2],
    carry_latch: Option<i64>,
    passed_undo: Option<UndoKind>,
    frame_events: Vec<Event>,
    /// one flag a `frame_events` entry: a focused widget handled that key, so `key_pressed` no longer reports it
    taken: Vec<std::cell::Cell<bool>>,
    input_frame: Option<rframe::Frame>,
    routed_events: Vec<(i64, Event, (f64, f64))>,
    cancelled: Vec<MouseButton>,
    modal_key: Option<i64>,
    modal_extra: Vec<i64>,
    dismissed_popups: Vec<i64>,
    modal_in_frame: bool,
    /// The modal this frame's input was routed to (the last frame's top one): a popup under it keeps its signals.
    routed_modal: Option<i64>,
    view_w: f32,
    view_h: f32,
    height_changed: bool,
    full_damage: bool,
    modal_heights: HashMap<i64, (f32, i64)>,
    signals: HashMap<i64, Accumulator>,
    spare_signals: Vec<Accumulator>,
    spare_modifiers: Vec<Vec<Key>>,
    /// Recycled accumulators' warmed key lists: a node taking keys reuses one, whichever accumulator the pool gave it.
    spare_key_lists: Vec<Vec<(Event, Vec<Key>)>>,
    hits: Vec<HitRec>,
    hit_count: usize,
    chain_key: i64,
    chain: Vec<i64>,
    label: String,
    /// the frame's layers are `draw[..draw_len]`; the rest keep their storage for later frames
    draw: Vec<Layer>,
    draw_len: usize,
    /// the frame before's layers, painted over by the next frame
    spare: Vec<Layer>,
    draw_revision: i64,
    /// the frame number of the last paint: node ranges painted then index `draw`
    painted: i64,
    /// the scratch layer a patch paints one node into
    patch_layers: Vec<Layer>,
    /// nodes of kept views patched while building: rewritten after the paint
    repatch: Vec<usize>,
    /// the boxes a press is under (kept: a press allocates nothing)
    press_chain: Vec<i64>,
    /// views by id (0: the root); free ids and painter table indices; the table index's view
    views: Vec<ViewRec>,
    palette_shapes: Vec<[PaletteShape; PALETTE_SHAPES]>,
    free_views: Vec<u32>,
    free_indices: Vec<u16>,
    index_owner: Vec<u32>,
    /// the view being built, and the view whose entry the hits being recorded follow
    view_cur: u32,
    paint_xf: u32,
    /// bumped by what changes every view's output (theme, font size, density, window size)
    epoch: u64,
    verify: bool,
    stamp: u64,
    /// hot, focus, keyboard focus and carrying as the last frame left them
    seen: (i64, i64, bool, bool),
    marks: Vec<i64>,
    damage: DamageList,
    view_out: Vec<(u16, ViewEntry)>,
    blur_out: Vec<BlurOut>,
    view_prev: Vec<(u16, ViewEntry)>,
    promotion: promotion::Promotion,
    /// (slot, scratch layer) of every node the frame painted, and the compaction buffer
    painted_nodes: Vec<(usize, usize)>,
    compact: Vec<Instance>,
    compact_bounds: Vec<[f32; 4]>,
    cull_runs: Vec<CullRun>,
    /// the kept runs a compaction moves (from, to, where to), applied together
    moves: Vec<(u32, u32, u32)>,
    temp_breaks: Vec<Break>,
    temp_images: Vec<Image>,
    /// a grid's rows (sizes, then tops), lent to `grid_tracks` and handed back
    grid_rows: std::cell::Cell<Vec<f32>>,
    /// this walk copied a kept view's run (nested views' spans follow, `a11y_relocate`)
    a11y_copied: bool,
    /// layout found kept output stale: `frame` builds again (`settle`), with this quiet input
    settle: bool,
    quiet: Option<rframe::Frame>,
    /// `list_measured`'s heights per list, and the rows this frame built (list, row, slot)
    list_heights: HashMap<i64, RowHeights>,
    list_rows: Vec<(i64, usize, usize)>,
    /// sizes a build read before layout (slot, width, height; NaN: not read): layout disagreeing
    /// settles the frame
    expected: std::cell::Cell<Vec<ExpectedSize>>,
    /// Recent Bezier samples (parameters, seconds, progress): retargeting alternates the old and new
    /// times, so both samples stay available to the other nodes in the change.
    curve_memo: std::cell::Cell<[([u32; 6], f32); 2]>,
    /// `grow_line`'s pass stamp (`BoxRec::frozen`)
    grow_pass: u32,
    /// the text atlas's retirements and misses seen
    atlas_events: (u64, u64),
    /// layer tails shifted by a growing patch, and layers compacted
    shifts: u64,
    compactions: u64,
    /// canvas closures and formatted labels of the frame being built
    arena: Arena,
    painting: bool,
    frame_requested: bool,
    /// Frame time of the last interaction that affected a control or its input state.
    input_at: f64,
    /// Sample time and phase shared by the visible ambient animations.
    ambient_at: f64,
    ambient_epoch: f64,
    ambient_wake: f64,
    ambient_hz: u8,
    repaint_hz: u8,
    /// Inside `step`: no node was painted this frame, so every running transition patches its node.
    stepping: bool,
    /// `damage` as the last frame left it: a difference at the next paint is between-frame damage not drawn yet.
    frame_damage: DamageList,
    /// views built (not kept) this frame
    rebuilt: usize,
    /// the earliest `repaint_after` the root asked for this frame
    wake: f64,
    out: PlatformOut,
    destroyed: bool,
    /// the shown rows of the trees built with a revision, kept between builds (`Ui::tree`), one a live tree
    pub(crate) tree_rows: Vec<crate::ui_widgets::TreeRows>,
    /// nodes the trees' row derivation walked (`Ui::tree_nodes_walked`)
    pub(crate) tree_walked: u64,
    /// kept tree rows checked against the nodes where they were read (`Ui::tree_rows_checked`)
    pub(crate) tree_checked: u64,
    // Append scratch here to keep earlier hot fields within their existing load offsets.
    tooltip_lines: Vec<core::ops::Range<usize>>,
    pub(crate) tooltip_wait_key: String,
    pub(crate) tooltip_description: String,
    pub(crate) inspector_chip: String,
    trace_frame_reasons: AtomicBool,
    frame_reasons: PxuiFrameReasons,
    /// last, and `repr(C)` to keep it so: the 57 KB theme inline anywhere earlier puts every hot field past one aarch64
    /// load's reach (32 KB scaled, 4 KB for bytes), an address instruction per access (measured: see git log)
    theme: Theme,
}

/// Bytes a UI holds: the glyph atlases' CPU mirrors, the text layouts' glyph lists, and the text undo
/// stacks and shelf.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Bytes {
    pub atlas: i64,
    pub glyph_lists: i64,
    pub text_undo: i64,
}

/// One text left behind: its key, value and its undo and redo stacks.
pub type ShelfEntry = (i64, String, Vec<(String, usize, usize)>, Vec<(String, usize, usize)>);



fn rgba(c: Rgba) -> [u8; 4] {
    [c.r, c.g, c.b, c.a]
}

fn palette_color(colors: &Palette, accent: Rgba, ink: Rgba) -> Rgba {
    let Some(token) = ink.as_token() else { return ink; };
    token_ink(ink, token_entry(colors, accent, token.0))
}

impl Ui {
    /// `platform` is the host's modifier conventions: the text keys and every Command check follow
    /// it. `text` holds the fonts the host loaded: font 0 is the kit face (`pxui_text::PRAGMASEVKA`
    /// in the default theme); `font_size` is the logical size of kit text.
    /// Serial of this `Ui` (nonzero), one count for the process: a handle carries it (`UiBox.ui`), so another `Ui`'s handle, or a zeroed
    /// one, is told apart in one compare. Per process by design: a counter nothing else reads, bounded by `u32` (it skips 0 on wrap).
    #[inline(always)]
    pub fn serial(&self) -> u32 { self.ui_serial }

    pub fn create(platform: Platform, font_points: Option<f32>, text: Text) -> Ui {
        let font_points = font_points.unwrap_or(theme::FONT_SIZE as f32);
        let font_size = round_points(font_points);
        if !(font_points.is_finite() && font_points > 0.0) {
            panic!("Ui.create: font_size must be positive");
        }
        static SERIAL: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
        let serial = loop {
            let next = SERIAL.fetch_add(1, core::sync::atomic::Ordering::Relaxed).wrapping_add(1);
            if next != 0 { break next; }
        };
        Ui {
            ui_serial: serial,
            theme: Theme::default(),
            themes: Vec::new(),
            theme_depth: 0,
            theme_ids: Vec::new(),
            theme_raw: None,
            repaint_palette: None,
            theme_id: 0,
            serial: { static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1); NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed) },
            tweens: Vec::new(),
            keyframes: Vec::new(),
            free_keyframes: Vec::new(),
            now: 0.0,
            motion: false,
            environment:pxui_types::Environment::default(),
            keyboard: pxui_types::Keyboard::default(),
            system_appearance:0,
            system_reduced_motion:false,
            colors: [[Rgba::default(); pxui_types::theme::ColorToken::LIMIT]; 2],
            theme_colors: None,
            theme_reduced_motion:false,
            font_size,
            font_points,
            platform,
            text: Some(Box::new(text)),
            table: SlotTable::create(),
            slots: Vec::new(),
            empty_strings: EmptyStrings::default(),
            span_arena: Vec::new(),
            span_spare: Vec::new(),
            span_live: 0,
            free: Vec::new(),
            moving: Vec::new(),
            gone: Vec::new(),
            scroll_y: Vec::new(),
            thumbs: Vec::new(),
            thumb_hides: Vec::with_capacity(8),
            thumb_shown: Vec::with_capacity(8),
            count: 0,
            boxes: Vec::new(),
            l_x: Vec::new(),
            l_y: Vec::new(),
            l_w: Vec::new(),
            l_h: Vec::new(),
            l_content: Vec::new(),
            l_content_w: Vec::new(),
            scroll_xs: Vec::new(),
            glides: [Vec::new(), Vec::new()],
            gestures: Vec::new(),
            gsettle: Vec::new(),
            arranged: 0,
            dir: Direction::Ltr,
            z_order: Vec::new(),
            layout_time: std::time::Duration::ZERO,
            phases: [std::time::Duration::ZERO; PHASES],
            phase_at: None,
            phase_counter: None,
            phase_counts: None,
            counter_at: None,
            a11y: A11yTree::new(),
            a11y_next: A11yTree::new(),
            a11y_map: Vec::new(),
            a11y_pending: Vec::new(),
            a11y_dirty: true,
            a11y_gen: 1,
            a11y_full: true,
            a11y_written: 0,
            a11y_on: false,
            list_drop: None,
            list_moved: None,
            reorder: reorder::Reorder::default(),
            row_handle: None,
            movable_rows: 0,
            row_moves: false,
            sort_names: crate::ui_lists::SortNames::default(),
            timing: false,
            a11y_seen: (0, None),
            parents: Vec::new(),
            overlays: Vec::new(),
            foreground: Vec::new(),
            seeds: Vec::new(),
            building: false,
            ahead: false,
            out_of_view: false,
            swap_frame: -1,
            frame_number: 0,
            first_build: 1,
            drop_zones: -1,
            density: 1,
            kit_row_height: 24,
            row_points: 24.0,
            closed_section: NONE,
            kit_window: false,
            pointer: (f32::NAN, f32::NAN),
            hot: 0,
            hover_rest: None,
            active: None,
            active_button: MouseButton::LeftButton,
            active_press: (0.0, 0.0),
            active_keys: Vec::new(),
            focus: 0,
            keyboard_focus: false,
            composition: String::new(),
            composition_selection: (0, 0),
            previous_keys: Vec::new(),
            route_keys: Vec::new(),
            frame_modifiers: 0,
            edit_focus: 0,
            default_focus: 0,
            edit_owner: 0,
            text_input_options: pxui_types::TextTraits::default(),
            edit_value: String::new(),
            edit_work: String::new(),
            returned: String::new(),
            input_box: None,
            area_cache: Vec::new(),
            text_cell: String::new(),
            text_cell_set: false,
            edit_caret: 0,
            edit_anchor: 0,
            edit_scroll_x: 0.0,
            window_ground: None,
            gesture_key: 0,
            gesture_time: f64::NAN,
            gesture_velocity: (0.0, 0.0),
            touches: touch::Touches::EMPTY,
            edit_undo: Vec::new(),
            text_undo_held: 0,
            charged: Bytes::default(),
            text_rows_seen: Vec::new(),
            text_panes: 8,
            text_colors_seen: Vec::new(),
            edit_redo: Vec::new(),
            edit_group: -1,
            edit_shelf: Vec::new(),
            picker_rows: pxui_types::ui::PickRows::new(),
            table_stops: Vec::new(),
            bodies: Box::default(),
            part_owner: 0,
            last_dup: (-1, 0, 0, 0),
            edit_unit: None,
            edit_column: -1.0,
            scrub_origin: None,
            payload: None,
            dropped: None,
            spare_payload: None,
            carry_fraction: [0.0; 2],
            carry_latch: None,
            passed_undo: None,
            frame_events: Vec::with_capacity(input::EVENT_RESERVE),
            taken: Vec::with_capacity(input::EVENT_RESERVE),
            input_frame: None,
            routed_events: Vec::with_capacity(input::EVENT_RESERVE),
            cancelled: Vec::new(),
            modal_key: None,
            modal_extra: Vec::new(),
            dismissed_popups: Vec::new(),
            modal_in_frame: false,
            routed_modal: None,
            view_w: 0.0,
            view_h: 0.0,
            height_changed: false,
            full_damage: false,
            modal_heights: HashMap::new(),
            signals: HashMap::new(),
            spare_signals: Vec::new(),
            spare_modifiers: Vec::new(),
            spare_key_lists: Vec::new(),
            hits: Vec::new(),
            hit_count: 0,
            chain_key: 0,
            chain: Vec::new(),
            label: String::new(),
            draw: Vec::new(),
            draw_len: 0,
            spare: Vec::new(),
            draw_revision: 0,
            painted: -1,
            // A tween can finish after the UI has warmed without ever using the patch painter.
            patch_layers: vec![Layer { pieces: Vec::with_capacity(1), ..Layer::new(0, 0) }],
            repatch: Vec::new(),
            press_chain: Vec::new(),
            views: vec![ViewRec::new()],
            palette_shapes: vec![[PaletteShape::default(); PALETTE_SHAPES]],
            free_views: Vec::new(),
            free_indices: Vec::new(),
            index_owner: vec![0],
            view_cur: 0,
            paint_xf: 0,
            epoch: 0,
            verify: false,
            stamp: 0,
            seen: (0, 0, false, false),
            marks: Vec::new(),
            damage: DamageList::default(),
            view_out: Vec::new(),
            blur_out: Vec::new(),
            view_prev: Vec::new(),
            promotion: promotion::Promotion::default(),
            painted_nodes: Vec::new(),
            compact: Vec::new(),
            compact_bounds: Vec::new(),
            cull_runs: Vec::new(),
            moves: Vec::new(),
            temp_breaks: Vec::new(),
            temp_images: Vec::new(),
            atlas_events: (0, 0),
            grow_pass: 0,
            settle: false,
            quiet: None,
            list_heights: HashMap::new(),
            list_rows: Vec::new(),
            expected: std::cell::Cell::new(Vec::new()),
            curve_memo: std::cell::Cell::new([([0; 6], 1.0); 2]), // zero-duration curve at time zero
            a11y_copied: false,
            grid_rows: std::cell::Cell::new(Vec::new()),
            shifts: 0,
            compactions: 0,
            arena: Arena::default(),
            painting: false,
            frame_requested: false,
            input_at: f64::NEG_INFINITY,
            trace_frame_reasons: AtomicBool::new(std::env::var_os("PXUI_FRAME_REASONS").is_some_and(|value| value == "1")),
            frame_reasons: PxuiFrameReasons::default(),
            ambient_at: f64::NEG_INFINITY,
            ambient_epoch: f64::NEG_INFINITY,
            ambient_wake: f64::INFINITY,
            ambient_hz: 0,
            repaint_hz: 0,
            stepping: false,
            frame_damage: DamageList::default(),
            rebuilt: 0,
            wake: f64::INFINITY,
            out: PlatformOut::default(),
            destroyed: false,
            tree_rows: Vec::new(),
            tree_walked: 0,
            tree_checked: 0,
            tooltip_lines: Vec::new(),
            tooltip_wait_key: String::new(),
            tooltip_description: String::new(),
            inspector_chip: String::new(),
        }
    }

    pub fn platform(&self) -> Platform {
        self.platform
    }

    /// Release what this UI holds. The handle must not be used afterwards.
    pub fn destroy(&mut self) {
        if !self.destroyed {
            self.destroyed = true;
            self.draw.clear();
            self.spare.clear();
            self.draw_len = 0;
            self.boxes.clear();
            self.arena.reset();
            for slot in &mut self.slots { slot.labels.clear(); }
            self.count = 0;
        }
    }

    /// The theme in effect: the UI's own, or the innermost `themed` override while building in one.
    pub fn theme(&self) -> &Theme {
        // depth 0 wraps to no index: the UI's own (no bounds panic, so a library call that only reads the theme cannot unwind)
        self.themes.get(self.theme_depth.wrapping_sub(1)).map_or(&self.theme, |entry| &entry.0)
    }

    /// Names the theme `theme()` returns, process-wide: it differs between `Ui`s and changes whenever the theme might (a swap, a
    /// `themed` override). A cache of the theme keyed by this is never stale; it may be refilled more often than needed.
    // ponytail: 24 bits of `Ui` number and 32 of epoch; a process that makes 16 million `Ui`s or changes one's theme 4 billion times
    // wraps into a repeat, so widen the key (a second output) then.
    pub fn theme_key(&self) -> u64 { self.serial << 40 | (self.epoch & 0x7fff_ffff) << 9 | (self.theme_id & 0x1ff) as u64 }

    /// The theme in effect's tokens, by value: what a canvas closure captures.
    pub fn tokens(&self) -> Tokens {
        self.theme().tokens
    }

    /// Swap the UI's theme (light, dark, high contrast, an accent: each is a `Theme` value). A change
    /// rebuilds every view once (for the new geometry); every node shown before then moves from the
    /// look it showed to the new one over the new theme's `motion.normal`, through the patch path
    /// (no further rebuild); a theme that does not move (`reduced_motion`, or no durations) snaps.
    /// Between frames, or inside one for what is built after it.
    pub fn set_theme(&mut self, theme: Theme) {
        let names_tokens = theme.names_color_tokens();
        self.set_theme_with_token_colors(&theme, names_tokens);
    }

    /// Install a theme whose named colours were checked during parsing.
    #[doc(hidden)]
    pub fn set_theme_with_token_colors(&mut self, theme: &Theme, names_tokens: bool) {
        self.set_theme_with_palette(theme, names_tokens, None, true);
    }

    #[doc(hidden)]
    pub fn set_theme_with_palette(&mut self, theme: &Theme, names_tokens: bool, palette: Option<&Palette>, animate_swap: bool) {
        debug_assert_eq!(names_tokens, theme.names_color_tokens());
        let epoch = self.epoch;
        let can_reuse = !animate_swap && !self.building
            && (!self.system_reduced_motion || theme.tokens.reduced_motion)
            && self.views[1..].iter().any(|view| view.palette_labels || view.palette_paint)
            && self.theme.same_geometry(theme);
        if self.theme_colors.as_ref() != palette {
            self.theme_colors = palette.copied();
            self.epoch += 1;
            self.full_damage = true;
        }
        self.theme_reduced_motion = theme.tokens.reduced_motion;
        if names_tokens || (self.system_reduced_motion && !theme.tokens.reduced_motion) {
            self.install_theme_with_overrides(theme, names_tokens, animate_swap);
        } else {
            self.theme_raw = None;
            self.install_theme(theme, animate_swap);
        }
        if self.epoch != epoch {
            let source = self.repaint_palette.filter(|&(_, target)| target == epoch).map_or(epoch, |(source, _)| source);
            self.repaint_palette = can_reuse.then_some((source, self.epoch));
        }
    }

    /// Re-resolve eligible output before any view can keep it. A parent cannot keep a stale child.
    fn recolor_views(&mut self, epoch: u64) {
        for v in 1..self.views.len() {
            let r = &self.views[v];
            if r.slot == NONE || !(r.palette_labels || r.palette_paint) || r.epoch != epoch || r.theme != 0 || r.dirty || r.reflow {
                continue;
            }
            let reusable = !r.patched && (!r.valid || (self.shown(v as u32) && r.nodes.iter().all(|&slot| {
                let s = &self.slots[slot];
                let n = &s.node;
                if s.key == self.focus || self.tween_of(slot).is_some() { return false; }
                if n.template == NodeTemplate::PositionedLabel {
                    return s.label_root_ink && s.rich.as_ref().is_some_and(|rich| rich.spans.len == 0)
                        && s.pose.2 == 1.0 && s.painted == r.built && s.range_layer == r.layer
                        && s.range.0 >= r.run.0 && s.range.1 <= r.run.1;
                }
                let ink = &n.look;
                r.palette_paint && s.text.is_empty() && s.rich.is_none() && n.bar.is_none()
                    && matches!(n.mark, Mark::None) && !n.flags.any(Flags::KEEP_ROOM)
                    && (ink.fill.a | ink.border.a | ink.text.a) == 0 && ink.gradient.is_none()
                    && ink.shadows.iter().all(|shadow| shadow.color.a == 0)
                    && ink.ring.color.a == 0
            })));
            if !reusable {
                self.views[v].palette_labels = false;
                self.views[v].palette_paint = false;
            }
        }
        for v in 1..self.views.len() {
            let r = &self.views[v];
            if r.slot == NONE || ((r.palette_labels || r.palette_paint) && r.epoch == epoch && r.theme == 0 && !r.dirty && !r.reflow) {
                continue;
            }
            let mut parent = r.parent as usize;
            while parent != 0 && self.views[parent].palette_paint {
                self.views[parent].palette_paint = false;
                parent = self.views[parent].parent as usize;
            }
        }
        let colors = *self.palette();
        let accent = self.theme.tokens.accent;
        let label_ink = self.theme.label.idle.text;
        let resolve = |ink: Rgba| palette_color(&colors, accent, ink);
        let mut changed = false;
        for index in 1..self.views.len() {
            let view = &self.views[index];
            if view.slot == NONE || !(view.palette_labels || view.palette_paint) || view.epoch != epoch || view.theme != 0 || view.dirty || view.reflow {
                continue;
            }
            if !view.valid {
                // A view without a run can only be kept while outside the viewport.
                self.views[index].epoch = self.epoch;
                continue;
            }
            let Some(layer) = self.draw[..self.draw_len].iter().position(|layer| layer.key == view.layer) else { continue };
            if self.draw[layer].instances.get(view.run.0 as usize..view.run.1 as usize).is_none() { continue; }
            let layer = &mut self.draw[layer];
            let mut dirty = false;
            for &slot in &view.nodes {
                let s = &mut self.slots[slot];
                if s.node.template != NodeTemplate::PositionedLabel { continue; }
                let ink = resolve(s.label_color.map_or(label_ink, Rgba::token));
                if ink == s.node.look.text { continue; }
                s.node.value.look.text = ink;
                s.seen.text = ink;
                let (start, end) = s.range;
                for instance in &mut layer.instances[start as usize..end as usize] {
                    match instance.kind {
                        crate::draw::kind::MASK => instance.color = rgba(ink),
                        crate::draw::kind::IMAGE => instance.color[3] = ink.a,
                        _ => {}
                    }
                }
                dirty |= start < end;
            }
            for shape in &self.palette_shapes[index][..view.palette_shape_count as usize] {
                let start = (view.run.0 + shape.at) as usize;
                let end = (view.run.0 + shape.end) as usize;
                let fill = shape.fill.as_token().map(|_| rgba(resolve(shape.fill)));
                let stroke = shape.stroke.as_token().map(|_| rgba(resolve(shape.stroke)));
                for instance in &mut layer.instances[start..end] {
                    if let Some(color) = fill { instance.color = color; }
                    if let Some(color) = stroke { instance.color2 = color; }
                }
                dirty = true;
            }
            if dirty {
                add_range(&mut layer.dirty, view.run.0, view.run.1);
                changed = true;
            }
            self.views[index].epoch = self.epoch;
        }
        if changed { self.draw_revision += 1; }
    }

    #[inline(never)]
    fn install_theme_with_overrides(&mut self, theme: &Theme, names_tokens: bool, animate_swap: bool) {
        let mut theme = *theme;
        self.theme_raw = (names_tokens && self.theme_colors.is_none()).then(|| Box::new(theme));
        if names_tokens {
            let palette = self.theme_colors.as_ref().unwrap_or(&self.colors[theme.tokens.is_dark() as usize]);
            resolve_theme(&mut theme, palette);
        }
        theme.tokens.reduced_motion |= self.system_reduced_motion;
        self.install_theme(&theme, animate_swap);
    }

    fn install_theme(&mut self, theme: &Theme, animate_swap: bool) {
        if *theme != self.theme {
            self.epoch += 1;
            self.full_damage = true;
            if theme.tokens.reduced_motion && !self.theme.tokens.reduced_motion { self.snap_motion(); }
            // Paint already saved the declared look unless the node changed again or motion was off.
            self.swap_frame = -1;
            if animate_swap && theme.tokens.moves() && theme.tokens.motion.normal > 0.0 {
                self.swap_frame = self.frame_number + if self.building { 0 } else { 1 };
                for s in &mut self.slots {
                    if s.key != 0 && (s.touched > s.painted || s.look_needs_snapshot) {
                        s.seen = s.node.look;
                        s.look_needs_snapshot = false;
                    }
                }
            }
            self.theme = *theme;
        }
    }

    /// Host facts stay separate from authored themes; turning the OS preference off restores them.
    pub fn set_system_preferences(&mut self,appearance:u32,reduced_motion:bool) {
        if (appearance, reduced_motion) != (self.system_appearance, self.system_reduced_motion) {
            let previous = self.epoch;
            self.epoch += 1;
            if reduced_motion == self.system_reduced_motion {
                if let Some((_, target)) = self.repaint_palette.as_mut() && *target == previous {
                    *target = self.epoch;
                }
                for view in &mut self.views[1..] {
                    if (view.palette_labels || view.palette_paint) && view.epoch == previous {
                        view.epoch = self.epoch;
                    }
                }
            }
        }
        if reduced_motion && !self.system_reduced_motion { self.snap_motion(); }
        (self.system_appearance,self.system_reduced_motion)=(appearance,reduced_motion);
        self.theme.tokens.reduced_motion=self.theme_reduced_motion||reduced_motion;
    }

    /// Installs the app's colour tokens: entry `i` of `light`/`dark` is token `i`; the rest read transparent. An entry that itself
    /// names a token (`Rgba::token`) is stored as clear, so a resolved colour never names one and resolving it again changes nothing.
    /// False (nothing changes) when the lengths differ or pass `ColorToken::LIMIT`. A changed table rebuilds every view once and
    /// re-resolves the root theme's token looks.
    pub fn set_colors(&mut self, light: &[Rgba], dark: &[Rgba]) -> bool {
        if light.len() != dark.len() || light.len() > pxui_types::theme::ColorToken::LIMIT { return false; }
        let mut colors = [[Rgba::default(); pxui_types::theme::ColorToken::LIMIT]; 2];
        colors[0][..light.len()].copy_from_slice(light);
        colors[1][..dark.len()].copy_from_slice(dark);
        for c in colors.iter_mut().flatten() { if c.as_token().is_some() { *c = Rgba::default(); } }
        if colors != self.colors {
            self.colors = colors;
            self.epoch += 1;
            if let Some(raw) = self.theme_raw.as_deref() {
                let mut theme = *raw;
                let palette = &self.colors[theme.tokens.is_dark() as usize];
                resolve_theme(&mut theme, palette);
                self.theme = theme;
            }
        }
        true
    }

    #[inline] fn theme_palette(&self) -> Option<&Palette> {
        self.themes.get(self.theme_depth.wrapping_sub(1))
            .and_then(|entry| entry.1.as_ref())
            .or(self.theme_colors.as_ref())
    }

    #[inline] fn palette(&self) -> &Palette {
        self.theme_palette().unwrap_or_else(|| &self.colors[self.theme().tokens.is_dark() as usize])
    }

    /// Colour token for the theme in effect. The legacy table selects a side from the theme's appearance.
    #[inline] pub fn color(&self, token: u16) -> Rgba {
        token_entry(self.palette(), self.theme().tokens.accent, token)
    }

    /// `look` with every colour that names a token (`Rgba::token`) replaced by `color(token)`, the theme in effect's entry; `None` when
    /// it names none (most looks), so a caller copies nothing. Called only where a look is applied (declared, set, patched, animated
    /// to, its ink read by a built-in body), never on an interpolated look: a transition's in-between colour that happens to read as a token is
    /// painted as the clear colour it is.
    pub fn resolve_look(&self, look: &Look) -> Option<Look> {
        if !names_token(look) { return None; }
        let mut out = *look;
        resolve_with(&mut out, self.palette(), self.theme().tokens.accent);
        Some(out)
    }

    /// `color` resolved as a look's colour is (`resolve_look`): a token's entry for the theme in effect, else `color` itself.
    pub fn resolve_color(&self, color: Rgba) -> Rgba {
        match color.as_token() { Some(token) => token_ink(color, self.color(token.0)), None => color }
    }

    pub fn set_environment(&mut self, value:pxui_types::Environment) {
        if self.environment!=value { self.environment=value; self.epoch+=1; }
    }
    pub fn environment(&self)->pxui_types::Environment { self.environment }
    /// The environment the engine holds, by reference: what a frame entry compares a new one against.
    #[inline] pub fn environment_ref(&self)->&pxui_types::Environment { &self.environment }
    pub fn set_keyboard(&mut self, value: pxui_types::Keyboard) {
        if self.keyboard != value { self.keyboard = value; self.epoch += 1; }
    }
    pub fn keyboard(&self) -> pxui_types::Keyboard { self.keyboard }
    /// The keyboard geometry the engine holds, by reference: what a frame entry compares a new one against.
    #[inline] pub fn keyboard_ref(&self) -> &pxui_types::Keyboard { &self.keyboard }
    /// The box of the text input widget built last (`text_field`, `line_field`, `search_field`, `text_area`, `value_field`), or none before one.
    pub fn input_box(&self) -> Option<Handle> { self.input_box }
    /// What happened to that input this frame (its signal and focus), the box named `name` for a screen reader first when one is
    /// given; none before a text input was built or once its box has gone. One call where a caller would read the box, name it,
    /// and read its signal and focus.
    pub fn input_response(&mut self, name: Option<&str>) -> Option<crate::ui_kit::Response> {
        let b = self.input_box.filter(|b| self.live(b))?;
        if let Some(name) = name { self.a11y_name(&b, name); }
        // `Response::of(&self.signal(&b), ..)` reading only the parts a response keeps, not the whole signal
        let key = b.box_key;
        let held = key != 0 && self.active == Some(key);
        let hovered = key != 0 && self.hot == key && !self.touches.using_touch;
        let (clicked, released, pressed, button) = match self.signals.get(&key).filter(|_| key != 0) { Some(v) => (v.clicked, v.released, v.pressed, v.button), None => (false, false, false, None) };
        let button = button.or(if held { Some(self.active_button) } else { None });
        Some(crate::ui_kit::Response { clicked, hovered, held, focused: self.focus == key, activated: clicked && !released, secondary: pressed && button == Some(input::MouseButton::RightButton), handle: b })
    }
    pub fn text_input_options(&self) -> pxui_types::TextTraits {
        if self.text_input_focused() {self.text_input_options} else {pxui_types::TextTraits::default()}
    }
    /// Safe content bounds, reduced by a full-width docked keyboard's current presentation frame.
    pub fn content_rect(&self) -> Rect {
        let [top, right, bottom, left] = self.environment.safe_area;
        let (w, h) = (self.view_w.max(0.0), self.view_h.max(0.0));
        let (x, y) = (left.min(w), top.min(h));
        let right = (w-right).max(x);
        let mut bottom = (h-bottom).max(y);
        let [kx, ky, kw, kh] = self.keyboard.frame;
        if self.keyboard.flags & pxui_types::Keyboard::VALID != 0 && kw > 0.0 && kh > 0.0
            && kx <= x && kx+kw >= right && ky+kh >= bottom {
            bottom = bottom.min(ky).max(y);
        }
        Rect::new(x, y, right-x, bottom-y)
    }
    pub fn system_appearance(&self)->u32 { self.system_appearance }

    /// Build `f` under `theme` (a scoped override: what `f` builds reads `theme()` as `theme`). A view
    /// built inside rebuilds when the override it was built under changes.
    pub fn themed(&mut self, theme: &Theme, f: &mut dyn FnMut(&mut Ui)) {
        self.themed_with_palette(theme, None, f);
    }

    #[doc(hidden)]
    pub fn themed_with_palette(&mut self, theme: &Theme, palette: Option<&Palette>, f: &mut dyn FnMut(&mut Ui)) {
        let mut reduced;
        let theme=if self.system_reduced_motion && !theme.tokens.reduced_motion {
            reduced=*theme;
            reduced.tokens.reduced_motion=true;
            &reduced
        } else { theme };
        self.views[self.view_cur as usize].palette_paint = false;
        let palette = palette.or_else(|| self.theme_palette()).copied();
        let saved = self.theme_id;
        // ids by search over the distinct themes and palettes seen, at most 32 (specification/pxui.md, designed limits)
        self.theme_id = saved & DISABLED_ID | match self.theme_ids.iter().position(|(t, p)| t == theme && *p == palette) {
            Some(i) => i as u32 + 1,
            None => {
                if self.theme_ids.len() >= 32 {
                    self.theme_ids.pop();
                    self.epoch += 1;
                }
                self.theme_ids.push((*theme, palette));
                self.theme_ids.len() as u32
            }
        };
        self.motion |= theme.tokens.moves();
        if self.theme_depth == self.themes.len() {
            self.themes.push(Box::new((*theme, palette)));
        } else {
            *self.themes[self.theme_depth] = (*theme, palette);
        }
        if theme.names_color_tokens() {
            let (resolved, palette) = &mut *self.themes[self.theme_depth];
            resolve_theme(resolved, palette.as_ref().unwrap_or(&self.colors[theme.tokens.is_dark() as usize]));
        }
        self.theme_depth += 1;
        f(self);
        self.theme_depth -= 1;
        self.theme_id = saved;
    }

    /// Build `f` with every interactive node it declares disabled (no press, no focus, accessibility `disabled`) when `disabled`;
    /// scopes nest by OR. The flag is part of the view cache's theme id, so a view built inside rebuilds when it flips; nothing allocates.
    /// The nodes `f` declared are marked when it returns, so a widget inside sees the scope (`scope_disabled`) while it builds.
    pub fn disabled(&mut self, disabled: bool, f: &mut dyn FnMut(&mut Ui)) {
        let (saved, start) = (self.theme_id, self.count);
        if disabled { self.theme_id |= DISABLED_ID; }
        f(self);
        self.theme_id = saved;
        if !disabled { return; }
        for index in start..self.count {
            let n = &mut self.slots[self.boxes[index].slot].node;
            n.flags.0 &= !(Flags::CLICKABLE.0 | Flags::FOCUSABLE.0);
            n.state.disabled = true;
        }
    }

    /// Whether a `disabled` scope is open.
    pub(crate) fn scope_disabled(&self) -> bool {
        self.theme_id & DISABLED_ID != 0
    }

    /// The body size exactly as created or set, fractions kept.
    pub fn font_size(&self) -> f32 {
        self.font_points
    }

    /// The body size rounded to whole points: what the integer kit measures and the 1.x C getter reports.
    pub fn font_px(&self) -> i64 {
        self.font_size
    }

    /// The kit text size from the next frame on, and the row height with it (`max 24 (size + 11)`).
    /// Call it between frames, never inside `frame`.
    pub fn set_font_size(&mut self, points: f32) {
        if !(points.is_finite() && points > 0.0) {
            panic!("Ui.set_font_size: size must be positive");
        }
        if points != self.font_points {
            self.epoch += 1;
        }
        let size = round_points(points);
        self.font_points = points;
        self.font_size = size;
        self.kit_row_height = 24.max(size + 11);
        self.row_points = (points + 11.0).max(24.0);
    }

    /// The most recently completed frame.
    #[inline]
    pub fn output(&self) -> FrameOut<'_> {
        FrameOut { draw: &self.draw[..self.draw_len], platform: &self.out, damage: &self.damage, views: &self.view_out, blurs: &self.blur_out, promoted: &self.promotion.current, a11y: &self.a11y }
    }

    /// The host drew layer `index` of the output with its `dirty` ranges: they start over empty, and
    /// so does the damage (between frames it accumulates over every patch until a draw consumes it).
    pub fn drawn(&mut self, index: usize) {
        self.damage = DamageList::default();
        if let Some(layer) = self.draw[..self.draw_len].get_mut(index) {
            layer.dirty.clear();
        }
    }

    /// Nodes declared by the last build (a kept view counts as its one box): what a view or a
    /// virtual list saves.
    pub fn built_nodes(&self) -> usize {
        self.count
    }

    /// Layer tails shifted by a patch that grew past its node's room, and layers compacted, since
    /// the `Ui` was made: both rewrite a layer from that point on.
    pub fn layer_rewrites(&self) -> (u64, u64) {
        (self.shifts, self.compactions)
    }

    /// Views (`Ui::view`) the last frame built rather than kept; the root is not counted.
    pub fn rebuilt_views(&self) -> usize {
        self.rebuilt
    }

    /// The views whose label (`Ui::view`'s) starts with `prefix` the last frame built rather than kept.
    pub fn rebuilt_named(&self, prefix: &str) -> usize {
        self.views.iter().skip(1).filter(|r| r.built >= self.first_build && r.name.starts_with(prefix)).count()
    }

    /// The view being built is clean and runs again only to be checked (`verify_views`): a build
    /// that reads state its revision leaves out (a pan the view's offset shows) reads it as of the
    /// kept build.
    pub fn verifying(&self) -> bool {
        self.building && self.view_cur != 0 && self.views[self.view_cur as usize].verify
    }

    /// Declare what a node built this frame is to a screen reader, and the states its widget knows
    /// (`a11y::state` bits).
    pub fn set_role(&mut self, b: &Handle, role: Role, states: u32) {
        if self.live(b) {
            let n = &mut self.slots[b.box_slot].node;
            (n.role, n.a11y) = (role, states);
        }
    }

    /// Gives `b` `role` when its builder left it `Role::Unknown` (its states as they are) and the accessible `name` when it has none:
    /// behaviour's part of a node a body built, which never overwrites what the body set.
    pub fn default_role(&mut self, b: &Handle, role: Role, name: &str) {
        if !self.live(b) { return; }
        let n = &mut self.slots[b.box_slot].node;
        if n.role == Role::Unknown { n.role = role; }
        if self.slots[b.box_slot].a11y_name.is_empty() { self.a11y_name(b, name); }
    }

    // Recycled view containers can hold the buffer a newly visible text node needs.
    fn cleared_strings(&mut self, slot: usize) {
        if !self.empty_strings.active() {return;}
        for field in StringField::ALL {
            self.empty_strings.empty(field,slot,field.get(&self.slots[slot]).capacity());
        }
    }

    fn reuse_empty_string(&mut self, slot: usize, len: usize, field: StringField) {
        let capacity=field.get(&self.slots[slot]).capacity();
        if len == 0 { self.empty_strings.empty(field,slot,capacity); return; }
        let new_identity = self.slots[slot].new_identity;
        let current = field.get(&self.slots[slot]);
        if capacity >= len && (!new_identity || !current.is_empty() || capacity == len) {
            return;
        }
        if self.empty_strings.rebuild(field) {
            for (slot, value) in self.slots.iter().enumerate() {
                let buffer = field.get(value);
                if buffer.is_empty() { self.empty_strings.empty(field, slot, buffer.capacity()); }
            }
        }
        let slots = &self.slots;
        let empty_capacity = |slot: usize| { let buffer = field.get(&slots[slot]); buffer.is_empty().then_some(buffer.capacity()) };
        if let Some(donor) = self.empty_strings.donor(field,slot,capacity,len,&empty_capacity) {
            let buffer = std::mem::take(field.get_mut(&mut self.slots[donor]));
            let mut old = std::mem::replace(field.get_mut(&mut self.slots[slot]), buffer);
            old.clear();
            *field.get_mut(&mut self.slots[donor]) = old;
            self.empty_strings.empty(field,donor,capacity);
        }
    }

    /// The node's accessible name (its text by default), for a node declared this frame.
    pub fn a11y_name(&mut self, b: &Handle, name: &str) {
        if self.live(b) {
            self.reuse_empty_string(b.box_slot, name.len(), StringField::Name);
            let s = &mut self.slots[b.box_slot].a11y_name;
            s.clear();
            s.push_str(name);
        }
    }

    /// The node's accessible value (a field's text, a readout) and its selection (anchor and caret
    /// in chars).
    pub fn a11y_value(&mut self, b: &Handle, value: &str, selection: (u32, u32)) {
        if self.live(b) {
            self.reuse_empty_string(b.box_slot, value.len(), StringField::Value);
            let s = &mut self.slots[b.box_slot];
            s.a11y_value.clear();
            s.a11y_value.push_str(value);
            s.a11y_selection = selection;
        }
    }

    /// The node's accessible value as a number in `min..=max` moved by `step` (`a11y::Value::Number`:
    /// a slider, a determinate progress bar); it wins over a text value, which then says the number in words.
    pub fn a11y_number(&mut self, b: &Handle, now: f64, min: f64, max: f64, step: f64) {
        if self.live(b) {
            self.slots[b.box_slot].a11y_number = Some([now, min, max, step]);
        }
    }

    /// Actions from an assistive technology (`pxui_atspi::Bridge::pump`), applied at the start of
    /// the next frame as input: Press clicks (not a node shown disabled), Focus focuses, Scroll scrolls by `payload` points,
    /// SetValue hands `payload` to the widget (`Signal::a11y_value`), Expand and Collapse click a
    /// disclosure whose state differs, Select focuses a text field and sets its selection
    /// (`Signal::a11y_select`). Ids of nodes that are gone are dropped.
    pub fn inject_a11y(&mut self, actions: &[A11yAction]) {
        self.a11y_pending.extend_from_slice(actions);
    }

    /// Discards requests for a surface that is no longer registered, retaining the queue's storage.
    pub fn clear_a11y_actions(&mut self) { self.a11y_pending.clear(); }

    /// Requests waiting for the next frame; used by the ABI's per-Ui queue budget before accepting a whole batch.
    pub fn pending_a11y_actions(&self) -> usize { self.a11y_pending.len() }

    fn apply_a11y(&mut self) {
        let mut pending = std::mem::take(&mut self.a11y_pending);
        for a in &pending {
            let (slot, generation) = ((a.id & 0xffff_ffff) as usize, (a.id >> 32) as u32);
            let Some(s) = self.slots.get(slot).filter(|s| s.generation == generation && s.key != 0) else { continue };
            let (key, expanded, disabled) = (s.key, s.node.a11y & a11y_state::EXPANDED != 0, s.node.state.disabled);
            match a.action {
                // a node shown disabled takes no press, as it takes none from the pointer or the keyboard
                A11yKind::Press | A11yKind::Expand | A11yKind::Collapse if disabled => {}
                A11yKind::Press => self.accumulator(key).clicked = true,
                A11yKind::Focus => {
                    let b = UiBox { index: NONE, box_key: key, box_slot: slot, generation, ui: self.ui_serial };
                    self.focus_box(&b);
                }
                A11yKind::Scroll => self.scroll_y[slot] = (self.scroll_y[slot] + a.payload as f32).clamp(0.0, f32::MAX),
                A11yKind::SetValue => self.accumulator(key).a11y_value = Some(a.payload),
                // a selection is made in a focused field: focus it, then the widget applies the range
                A11yKind::Select => {
                    let b = UiBox { index: NONE, box_key: key, box_slot: slot, generation, ui: self.ui_serial };
                    self.focus_box(&b);
                    self.accumulator(key).a11y_select = Some((a.payload.max(0.0) as u32, a.end.max(0.0) as u32));
                }
                A11yKind::Expand | A11yKind::Collapse => {
                    if expanded != (a.action == A11yKind::Expand) {
                        self.accumulator(key).clicked = true;
                    }
                }
                A11yKind::MoveUp | A11yKind::MoveDown => self.accumulator(key).a11y_move = if a.action == A11yKind::MoveUp { -1 } else { 1 },
            }
            self.mark_key(key);
        }
        self.a11y_dirty |= !pending.is_empty();
        pending.clear();
        self.a11y_pending = pending;
    }

    /// Build the accessibility tree (`FrameOut::a11y`) from this frame on. Off by default, so a UI
    /// nobody reads pays nothing for it; a host turns it on while an assistive technology listens
    /// (the AT-SPI bridge connected and a client is present). Off publishes an empty tree.
    pub fn set_a11y(&mut self, on: bool) {
        if on != self.a11y_on {
            self.a11y_on = on;
            (self.a11y_dirty, self.a11y_full) = (true, true);
            if !on {
                (self.a11y, self.a11y_next) = (A11yTree::new(), A11yTree::new());
            }
        }
    }

    /// Walk the views' declared nodes (a kept view's from its last build) into the next tree and
    /// publish it when it differs: an unchanged frame leaves the published tree and its revision.
    // A frame that rebuilds anything (or whose root declares roles) copies kept views' runs, walks
    // rebuilt views' nodes and relocates the views nested in copied runs (O(views x depth)); it
    // compares the next tree with the published one (a memcmp, O(tree)) to decide publication:
    // specification/pxui.md, designed limits.
    fn emit_a11y(&mut self) {
        if !self.a11y_on {
            return;
        }
        // nothing a kept view shows changed and the root declared no role: the tree stands
        let seen = (self.focus, self.active);
        if !self.a11y_dirty && seen == self.a11y_seen && self.views[0].nodes.iter().all(|s| self.slots[*s].node.role == Role::Unknown) {
            return;
        }
        (self.a11y_dirty, self.a11y_seen) = (false, seen);
        let mut t = std::mem::replace(&mut self.a11y_next, A11yTree::new());
        let mut map = std::mem::take(&mut self.a11y_map);
        t.begin();
        if map.len() < self.slots.len() {
            map.resize(self.slots.len(), NO_PARENT);
        }
        self.a11y_written = 0;
        self.emit_view(&mut t, &mut map, 0);
        self.a11y_relocate();
        if t.nodes != self.a11y.nodes || t.strings != self.a11y.strings {
            t.revision = self.a11y.revision;
            t.end();
            std::mem::swap(&mut t, &mut self.a11y);
        }
        // the spans recorded this walk hold in the published tree (it equals the walk's when unchanged)
        (self.a11y_gen, self.a11y_full) = (self.a11y_gen + 1, false);
        // the two trees take turns: each keeps the larger one's capacity (a no-op once equal), so a frame no larger than any before
        // allocates nothing, whichever tree it is walked into
        let (nodes, strings) = (self.a11y.nodes.capacity().max(t.nodes.capacity()), self.a11y.strings.capacity().max(t.strings.capacity()));
        for tree in [&mut self.a11y, &mut t] {
            tree.nodes.reserve(nodes - tree.nodes.len());
            tree.strings.reserve(strings - tree.strings.len());
        }
        self.a11y_next = t;
        self.a11y_map = map;
    }

    /// Nodes the last accessibility walk computed; a kept view's nodes are copied from the published
    /// tree, not counted.
    pub fn a11y_written(&self) -> usize {
        self.a11y_written
    }

    fn invalidate_a11y_view(&mut self,mut view:u32) {
        self.a11y_dirty=true;
        loop {
            self.views[view as usize].a11y_gen=0;
            if view==0 {break;}
            view=self.views[view as usize].parent;
        }
    }

    fn emit_view(&mut self, t: &mut A11yTree, map: &mut [u32], v: u32) {
        let next_gen = self.a11y_gen + 1;
        let r = &self.views[v as usize];
        // what places its rects: the entry of the view that moves it (offset, transform, composed)
        let xf = &self.views[r.xf as usize];
        // its own transform and the composed one around it (`compose` keeps both)
        let [t0, t1, t2, t3] = xf.transform;
        let [a0, a1, a2, a3] = xf.pa;
        let shift = [xf.shift.0, xf.shift.1, xf.scale, t0, t1, t2, t3, xf.pivot.0, xf.pivot.1, a0, a1, a2, a3, xf.pb[0], xf.pb[1], xf.outer.x, xf.outer.y, xf.outer.w, xf.outer.h];
        if v != 0 && r.built != self.frame_number && r.a11y_gen == self.a11y_gen && r.a11y_shift == shift && !self.a11y_full {
            // kept and unmoved: its run of the published tree, nested views included, as it was (the
            // nested views' spans follow after the walk, `a11y_relocate`)
            let (s0, s1) = r.a11y_span;
            let base = t.nodes.len() as u32;
            t.copy_span(&self.a11y, s0..s1, map[r.slot]);
            self.a11y_copied = true;
            let r = &mut self.views[v as usize];
            (r.a11y_span, r.a11y_gen, r.a11y_from) = ((base, base + (s1 - s0)), next_gen, s0);
            return;
        }
        let start = t.nodes.len() as u32;
        self.emit_nodes(t, map, v);
        let end = t.nodes.len() as u32;
        let r = &mut self.views[v as usize];
        (r.a11y_span, r.a11y_gen, r.a11y_shift, r.a11y_from) = ((start, end), next_gen, shift, u32::MAX);
    }

    /// After a walk: a view nested in a kept view whose run was copied takes its span in the new
    /// tree. One pass over the views, each walking up to the copied view around it.
    fn a11y_relocate(&mut self) {
        const MOVED: u32 = u32::MAX - 1;
        if !std::mem::take(&mut self.a11y_copied) {
            return;
        }
        let next_gen = self.a11y_gen + 1;
        for w in 1..self.views.len() {
            if self.views[w].a11y_gen != self.a11y_gen {
                continue;
            }
            let mut p = self.views[w].parent;
            while p != 0 {
                // up past views relocated with the copy around them (`MOVED`) to the copied one
                let q = &self.views[p as usize];
                if q.a11y_gen == next_gen && q.a11y_from != MOVED {
                    let (s0, base) = (q.a11y_from, q.a11y_span.0);
                    let r = &mut self.views[w];
                    if s0 != u32::MAX && r.a11y_span.0 >= s0 {
                        (r.a11y_span, r.a11y_gen, r.a11y_from) = ((r.a11y_span.0 - s0 + base, r.a11y_span.1 - s0 + base), next_gen, MOVED);
                    }
                    break;
                }
                p = self.views[p as usize].parent;
            }
        }
    }

    fn emit_nodes(&mut self, t: &mut A11yTree, map: &mut [u32], v: u32) {
        for k in 0..self.views[v as usize].nodes.len() {
            let slot = self.views[v as usize].nodes[k];
            let s = &self.slots[slot];
            let parent = if s.parent_slot == NONE { NO_PARENT } else { map[s.parent_slot] };
            map[slot] = parent;
            if s.node.role != Role::Unknown {
                let n = &s.node;
                // where it shows: offset and transformed as `rect` reports it
                let xf = &self.views[self.views[s.owner as usize].xf as usize];
                let rect = xf.cover(Rect::new(s.rx + xf.shift.0, s.ry + xf.shift.1, s.rw, s.rh));
                let local = Rect::new(s.rx, s.ry, s.rw, s.rh).intersect(s.clip);
                let shown = xf.cover(Rect::new(local.x + xf.shift.0, local.y + xf.shift.1, local.w, local.h)).intersect(xf.outer);
                let mut states = n.a11y;
                let mut acts = 0;
                if !n.state.disabled {
                    states |= a11y_state::ENABLED;
                }
                if local.w > 0.0 && local.h > 0.0 && shown.w > 0.0 && shown.h > 0.0 {
                    states |= a11y_state::VISIBLE;
                }
                // a node focusable to assistive technology alone (a radio group's options past its one Tab stop) offers Focus too
                if n.flags.any(Flags::FOCUSABLE) {
                    states |= a11y_state::FOCUSABLE;
                }
                if states & a11y_state::FOCUSABLE != 0 {
                    acts |= a11y_actions::FOCUS;
                }
                if s.key == self.focus {
                    states |= a11y_state::FOCUSED;
                }
                if self.active == Some(s.key) {
                    states |= a11y_state::PRESSED;
                }
                if n.flags.any(Flags::CLICKABLE) {
                    acts |= a11y_actions::PRESS;
                }
                if n.flags.any(Flags::SCROLL | Flags::SCROLL_X) {
                    acts |= a11y_actions::SCROLL;
                }
                if n.a11y & a11y_state::EDITABLE != 0 || n.role == Role::Slider {
                    acts |= a11y_actions::SET_VALUE;
                }
                if n.a11y & a11y_state::EXPANDABLE != 0 {
                    acts |= a11y_actions::EXPAND | a11y_actions::COLLAPSE;
                }
                if s.movable {
                    acts |= a11y_actions::MOVE_UP | a11y_actions::MOVE_DOWN;
                }
                let i = t.push(parent, crate::a11y::node_id(slot, s.generation), n.role, rect);
                t.set_name(i, if s.a11y_name.is_empty() { &s.text } else { &s.a11y_name });
                if let Some([now, min, max, step]) = s.a11y_number {
                    t.set_value_number(i, now, min, max, step, &s.a11y_value);
                } else if !s.a11y_value.is_empty() || n.a11y & a11y_state::EDITABLE != 0 {
                    t.set_value(i, &s.a11y_value);
                    t.set_selection(i, s.a11y_selection.0, s.a11y_selection.1);
                }
                t.set_states(i, states);
                t.set_actions(i, acts);
                map[slot] = i;
                self.a11y_written += 1;
            }
            let held = self.slots[slot].view;
            if held != NO_VIEW && held != v {
                self.emit_view(t, map, held);
            }
        }
    }

    /// Time the frame's phases from the next frame on (`phase_times`, `layout_time`): a clock read
    /// per phase, off by default.
    pub fn set_timing(&mut self, on: bool) {
        self.timing = on;
        self.phases = [std::time::Duration::ZERO; PHASES];
    }

    /// The last frame's time in each of its phases, in `PHASE_NAMES` order, while `set_timing` is
    /// on: begin (input routing), build, intrinsic, arrange, paint, a11y, finish.
    pub fn phase_times(&self) -> [std::time::Duration; PHASES] {
        self.phases
    }

    /// Optional monotonic counter sampled at frame phase boundaries. The caller chooses the unit.
    pub fn set_phase_counter(&mut self, counter: Option<fn() -> Option<u64>>) {
        self.phase_counter = counter;
        self.phase_counts = None;
        self.counter_at = None;
    }

    /// Counter deltas in PHASE_NAMES order, summed over every settling pass of the last frame.
    /// None before a timed frame or if any counter sample failed or regressed.
    pub fn phase_counts(&self) -> Option<[u64; PHASES]> {
        self.phase_counts
    }

    fn phase(&mut self, i: usize) {
        if let Some(at) = self.phase_at {
            let now = std::time::Instant::now();
            self.phases[i] = now - at;
            self.phase_at = Some(now);
        }
        if let Some(at) = self.counter_at {
            let now = self.phase_counter.and_then(|clock| clock());
            if let (Some(elapsed), Some(phases)) = (
                now.and_then(|now| now.checked_sub(at)),
                self.phase_counts.as_mut(),
            ) {
                phases[i] = phases[i].saturating_add(elapsed);
            } else {
                self.phase_counts = None;
            }
            self.counter_at = now;
        }
    }

    /// How long the last frame's layout (`intrinsic` and `arrange`) took, while `set_timing` is on.
    pub fn layout_time(&self) -> std::time::Duration {
        self.layout_time
    }

    /// Nodes the last frame's layout ran over: a kept view's are not among them.
    pub fn arranged_nodes(&self) -> usize {
        self.arranged
    }

    /// The row flow direction (`Rtl` mirrors flow children); every view rebuilds when it changes.
    pub fn set_dir(&mut self, dir: Direction) {
        if dir != self.dir {
            self.dir = dir;
            self.epoch += 1;
        }
    }

    pub fn dir(&self) -> Direction {
        self.dir
    }

    /// Re-run every clean view's build and panic, naming the view, when its output differs from the
    /// kept one (a revision the app forgot to bump). For tests: it builds every view every frame.
    pub fn verify_views(&mut self, on: bool) {
        self.verify = on;
    }

    /// Ask the host for the clipboard's text; it arrives as `Event::Paste` with the next frame.
    pub(crate) fn request_paste(&mut self) {
        self.out.paste = true;
    }

    pub(crate) fn copy(&mut self, text: &str) {
        self.out.clipboard = Some(text.to_owned());
    }

    /// Changes exactly when a frame paints layers different from the frame before's.
    pub fn draw_revision(&self) -> i64 {
        self.draw_revision
    }

    /// The text engine: the host loads fonts into it and feeds its painter from `drain_uploads`
    /// (the glyph atlases the frame's glyph instances sample).
    pub fn text_mut(&mut self) -> &mut Text {
        self.text.as_mut().expect("Ui: text is out while painting")
    }

    /// How many keys the retained cache holds (live slots).
    pub fn retained_slots(&self) -> usize {
        self.slots.iter().filter(|s| s.key != 0).count()
    }

    /// Cumulative text layout misses. None while the painter temporarily owns the text service.
    pub fn text_layout_count(&self) -> Option<u64> { self.text.as_ref().map(|t| t.layout_count()) }

    /// The kit row height exactly, fractions kept.
    pub fn row_height(&self) -> f32 {
        self.row_points
    }

    /// The kit row height in whole points, as the integer kit lays it out.
    pub fn row_px(&self) -> i64 {
        self.kit_row_height
    }

    /// The window's width in points: responsive layout is `if ui.width() < 600.0 { .. }`.
    pub fn width(&self) -> f32 {
        self.view_w
    }

    /// Integer rendering density shared with Paint (host scale rounded to at least one).
    pub fn density(&self) -> i64 { self.density }

    pub fn view_size(&self) -> (f32, f32) {
        (self.view_w, self.view_h)
    }

    pub fn text_line_height(&self) -> i64 {
        self.font_size + (self.font_size + 1) / 2
    }

    /// The heap this UI holds for the host to account for; zero once destroyed.
    pub fn bytes(&self) -> Bytes {
        if self.destroyed {
            Bytes::default()
        } else {
            let (all, lists) = self.text.as_ref().map_or((0, 0), |t| (t.bytes(), t.glyph_list_bytes()));
            Bytes { atlas: (all - lists) as i64, glyph_lists: lists as i64, text_undo: self.text_undo_held }
        }
    }

    /// The width of kit text in points, for layout during the build (`Paint::text_width` while
    /// painting).
    pub fn text_width(&mut self, size: Option<i64>, text: &str) -> f32 {
        let size = size.unwrap_or(self.font_size);
        lay_out(self.text_mut(), &kit_style(size, 0.0), 0.0, text).1.w
    }

    /// Lay `text` out in any style (wrap, ellipsis, line limit, alignment, font) within `width`
    /// points (0: unbounded), as `Paint::text_styled` draws it.
    pub fn text_metrics(&mut self, style: &Style, width: f32, text: &str) -> Metrics {
        lay_out(self.text_mut(), style, width, text).1
    }

    /// The x of the caret before byte `index` of kit text, in points from the text's start.
    pub fn caret_x(&mut self, size: Option<i64>, text: &str, index: usize) -> f32 {
        let size = size.unwrap_or(self.font_size);
        caret_x(self.text_mut(), size, text, index)
    }

    /// Measure the same ASCII-uppercase label style as `Paint::cap`, reusing the frame's text buffer.
    pub fn cap_width(&mut self, text: &str) -> f32 {
        let mut buffer = std::mem::take(&mut self.label);
        buffer.clear();
        buffer.extend(text.chars().map(|c| c.to_ascii_uppercase()));
        let size = 8.max(self.font_size - 2);
        let width = lay_out(self.text_mut(), &kit_style(size, 0.08 * size as f32), 0.0, &buffer).1.w;
        self.label = buffer;
        width
    }

    // ----------------------------------------------------- retained slots

    fn slot_of(&mut self, key: i64, found: Option<usize>) -> usize {
        if let Some(slot) = found {
            return slot;
        }
        let slot = match self.free.pop() {
            Some(slot) => slot,
            None => {
                self.slots.push(Slot::fresh());
                self.empty_strings.reserve(self.slots.capacity());
                self.scroll_y.push(0.0);
                self.thumbs.push([(f32::NAN, f64::NEG_INFINITY); 2]);
                self.scroll_xs.push(0.0);
                self.glides[0].push(Glide::REST);
                self.glides[1].push(Glide::REST);
                self.gestures.push(gesture::GSlot::REST);
                // the free list can take every slot: freeing never grows it (amortised with `slots`)
                self.free.reserve(self.slots.capacity().saturating_sub(self.free.len()));
                self.slots.len() - 1
            }
        };
        let s = &mut self.slots[slot];
        // Virtual rows reuse text and accessibility storage after their old identity is retired.
        let generation = s.generation;
        let text_layout = s.text_layout.get();
        let mut text = std::mem::take(&mut s.text);
        let mut a11y_name = std::mem::take(&mut s.a11y_name);
        let mut a11y_value = std::mem::take(&mut s.a11y_value);
        text.clear();
        a11y_name.clear();
        a11y_value.clear();
        *s = Slot::fresh();
        s.generation = generation;
        s.text_layout.set(text_layout);
        s.text = text;
        s.a11y_name = a11y_name;
        s.a11y_value = a11y_value;
        s.key = key;
        s.touched = self.frame_number;
        self.scroll_y[slot] = 0.0;
        self.thumbs[slot] = [(f32::NAN, f64::NEG_INFINITY); 2];
        self.scroll_xs[slot] = 0.0;
        (self.glides[0][slot], self.glides[1][slot]) = (Glide::REST, Glide::REST);
        self.gestures[slot] = gesture::GSlot::REST;
        self.table.add(key, slot);
        slot
    }

    /// `v` or a view around it kept its last output this frame (its nodes live on undeclared).
    fn kept_now(&self, mut v: u32) -> bool {
        while v != 0 {
            let r = &self.views[v as usize];
            if r.kept == self.frame_number {
                return true;
            }
            v = r.parent;
        }
        false
    }

    /// Free the nodes this frame did not declare, unless a kept view holds them: the nodes the
    /// views built this frame (the root among them) declared in their build before and not now,
    /// and with a view among those, every node it held. O(views + nodes built), not O(nodes).
    fn prune(&mut self) {
        for index in 0..self.count {
            let s = &mut self.slots[self.boxes[index].slot];
            s.labels.truncate(s.label_count);
        }
        // a scroll in motion moves its view's layout every frame
        let mut moving = std::mem::take(&mut self.moving);
        moving.retain(|slot| self.slots[*slot].key != 0 && (self.glides[0][*slot].moving() || self.glides[1][*slot].moving()));
        let settling = std::mem::take(&mut self.gsettle);
        for slot in moving.iter().chain(&settling) {
            self.mark_view(self.slots[*slot].owner);
        }
        self.gsettle = settling;
        self.moving = moving;
        let mut gone = std::mem::take(&mut self.gone);
        // room for every rebuilt view's next `prev` too, so the frame after a view's first build
        // does not grow the list
        let room = self.views.iter().filter(|r| r.built == self.frame_number).map(|r| r.nodes.len().max(r.prev.len())).sum::<usize>();
        gone.reserve(room.saturating_sub(gone.len()));
        for view in &self.views {
            if view.built == self.frame_number && view.prev != view.nodes {
                gone.extend_from_slice(&view.prev);
            }
        }
        while let Some(slot) = gone.pop() {
            let s = &self.slots[slot];
            if s.key == 0 || s.touched == self.frame_number || self.kept_now(s.owner) {
                continue;
            }
            let view = std::mem::replace(&mut self.slots[slot].view, NO_VIEW);
            if view != NO_VIEW {
                let r = &self.views[view as usize];
                gone.extend_from_slice(&r.nodes);
                gone.extend_from_slice(&r.prev);
                self.free_view(view);
            }
            self.free_slot(slot);
        }
        self.gone = gone;
    }

    fn free_slot(&mut self, slot: usize) {
        let key = self.slots[slot].key;
        self.table.remove(key);
        self.clear_rich(slot);
        let s = &mut self.slots[slot];
        s.key = 0;
        s.generation = s.generation.wrapping_add(1);
        s.text_value = None;
        s.labels.clear();
        // Freed slots can donate their retained strings before their next identity is assigned.
        s.text.clear();
        s.a11y_name.clear();
        s.a11y_value.clear();
        self.cleared_strings(slot);
        self.free.push(slot);
        if self.focus == key {
            self.focus = 0;
            self.composition.clear();
            self.edit_focus = 0;
        }
        if self.active == Some(key) {
            self.cancelled.insert(0, self.active_button);
            self.active = None;
        }
        // the box that holds a carry's pointer is gone: the carry goes with it
        if let Some((_, Origin::FromPointer(owner))) = &self.payload {
            if *owner == key {
                self.retire_payload();
            }
        }
        if self.hot == key {
            self.hot = 0;
        }
    }

    // -------------------------------------------------------------- input

    pub(crate) fn command_modifiers(&self, keys: &[Key]) -> bool {
        input::command_held(self.platform, keys)
    }

    /// Command moves and deletes by lines on Apple only; elsewhere Ctrl is the word modifier.
    pub(crate) fn line_modifiers(&self, keys: &[Key]) -> bool {
        self.platform == Platform::Apple && self.command_modifiers(keys)
    }

    /// The selection a screen reader set on box `key` this frame (anchor, caret in chars).
    pub(crate) fn a11y_select_of(&self, key: i64) -> Option<(u32, u32)> {
        self.signals.get(&key).and_then(|s| s.a11y_select)
    }

    fn accumulator(&mut self, key: i64) -> &mut Accumulator {
        if !self.signals.contains_key(&key) {
            let (keys, mut press_keys) = self.spare_signals.pop().map_or_else(
                || (Vec::new(), Vec::new()), |value| (value.keys, value.press_keys));
            press_keys.clear();
            if self.active == Some(key) { press_keys.extend_from_slice(&self.active_keys); }
            self.signals.insert(
                key,
                Accumulator {
                    pressed: false,
                    subtree_press: None,
                    released: false,
                    clicked: false,
                    double_clicked: false,
                    clicks: 0,
                    moved: false,
                    drag_x: 0.0,
                    drag_y: 0.0,
                    press_point: (0.0, 0.0),
                    release_point: (0.0, 0.0),
                    button: None,
                    scroll_x: 0.0,
                    scroll_y_steps: 0.0,
                    pinch: 1.0,
                    keys,
                    scroll_wheel: 0.0,
                    scroll_wheel_x: 0.0,
                    scroll_drag: 0.0,
                    scroll_drag_x: 0.0,
                    a11y_value: None,
                    a11y_select: None,
                    a11y_move: 0,
                    scroll_touch: false,
                    scroll_lift: false,
                    scroll_velocity: (0.0, 0.0),
                    scroll_finger: false,
                    press_keys,
                },
            );
        }
        self.signals.get_mut(&key).unwrap()
    }

    fn hit_index(&self, key: i64) -> Option<usize> {
        (0..self.hit_count).rev().find(|i| self.hits[*i].key == key)
    }

    /// Calls `f` for `key` and each ancestor in the hit list, innermost first.
    fn hit_ancestors(&self, key: i64, f: &mut dyn FnMut(i64)) {
        let mut key = key;
        while key != 0 {
            f(key);
            match self.hit_index(key) {
                Some(index) => key = self.hits[index].parent,
                None => break,
            }
        }
    }

    /// `key` is `ancestor` or under it in the hit list. The chain of a key is walked once (each
    /// step scans the list) and kept until the list is rebuilt.
    fn hit_within(&mut self, key: i64, ancestor: i64) -> bool {
        if key == ancestor {
            return true;
        }
        if key == 0 {
            return false;
        }
        if self.chain_key != key {
            // the kept list, refilled: a hover moving between nodes allocates nothing
            let mut chain = std::mem::take(&mut self.chain);
            chain.clear();
            self.hit_ancestors(key, &mut |k| chain.push(k));
            self.chain_key = key;
            self.chain = chain;
        }
        self.chain.contains(&ancestor)
    }

    fn hit_contains(&self, index: usize, point: (f32, f32)) -> bool {
        let h = &self.hits[index];
        let v = &self.views[h.view as usize];
        if !v.outer.contains(point) {
            return false;
        }
        let local = if v.flat { Some(point) } else { v.unmap(point) };
        if !local.is_some_and(|p| (Rect::new(h.x, h.y, h.w, h.h)).contains((p.0 - v.shift.0, p.1 - v.shift.1))) { return false; }
        let mut dismissed = false;
        if !self.dismissed_popups.is_empty() {
            self.hit_ancestors(h.key, &mut |key| dismissed |= self.dismissed_popups.contains(&key));
        }
        !dismissed
    }

    /// Topmost hit entry under `point`; 0 when none.
    fn topmost(&self, point: (f32, f32)) -> i64 {
        for index in (0..self.hit_count).rev() {
            if self.hits[index].flags.any(HIT_FLAGS) && self.hit_contains(index, point) {
                return self.hits[index].key;
            }
        }
        0
    }

    /// The root key whose box or child is topmost under the point in PXUI's previous hit tree.
    pub fn root_at(&self, point: (f32, f32), roots: &[i64]) -> Option<i64> {
        let mut found = None;
        self.hit_ancestors(self.topmost(point), &mut |key| {
            if found.is_none() && roots.contains(&key) {
                found = Some(key);
            }
        });
        found
    }

    /// Last root key pressed in `frame`, using PXUI's previous hit tree.
    pub fn last_press_within(&self, frame: &rframe::Frame, roots: &[i64]) -> Option<i64> {
        let mut last = None;
        for event in &frame.input.events {
            let point = match event {
                Event::PointerPressed(_, point) => Some(*point),
                Event::Touch(t) if t.phase == input::TouchPhase::Began => Some(t.point),
                _ => None,
            };
            if let Some(point) = point {
                if let Some(root) = self.root_at(p32(point), roots) {
                    last = Some(root);
                }
            }
        }
        last
    }

    fn flags_of_key(&self, key: i64) -> Flags {
        match self.hit_index(key) {
            Some(index) => self.hits[index].flags,
            None => Flags::NONE,
        }
    }

    /// The scroll box the wheel goes to at `point`: the topmost one, unless a blocking box lies
    /// over it that is not its own content (a field inside a scrolling panel lets the panel
    /// scroll; a menu over it does not).
    fn scroll_target(&mut self, point: (f32, f32)) -> i64 {
        self.scroll_target_flags(point, Flags::SCROLL | Flags::SCROLL_X)
    }

    fn scroll_target_flags(&mut self, point: (f32, f32), wanted: Flags) -> i64 {
        let mut blocker = 0;
        let mut index = self.hit_count as i64 - 1;
        while index >= 0 {
            let i = index as usize;
            if self.hit_contains(i, point) {
                let flags = self.hits[i].flags;
                let key = self.hits[i].key;
                if flags.any(wanted) {
                    return if blocker == 0 || self.hit_within(blocker, key) { key } else { 0 };
                } else if blocker == 0 && flags.any(Flags::BLOCKING) {
                    blocker = key;
                }
            }
            index -= 1;
        }
        0
    }

    fn eligible(&mut self, index: usize) -> bool {
        if !self.hits[index].flags.any(Flags::FOCUSABLE) {
            return false;
        }
        match self.modal_key {
            None => true,
            Some(modal) => {
                let key = self.hits[index].key;
                if self.hit_within(key, modal) {
                    return true;
                }
                let extra = self.modal_extra.clone();
                extra.iter().any(|e| self.hit_within(key, *e))
            }
        }
    }

    fn traverse_focus(&mut self, shift: bool) {
        let n = self.hit_count as i64;
        if n == 0 {
            return;
        }
        let current = self.hit_index(self.focus);
        let direction: i64 = if shift { -1 } else { 1 };
        let start: i64 = match current {
            Some(c) => c as i64,
            None => if direction < 0 { 0 } else { n - 1 },
        };
        let mut distance = 1;
        while distance <= n {
            let index = ((start + direction * distance + n) % n) as usize;
            if self.eligible(index) {
                self.focus = self.hits[index].key;
                self.keyboard_focus = true;
                self.composition.clear();
                self.edit_focus = 0;
                return;
            }
            distance += 1;
        }
    }

    fn modal_target(&mut self, target: i64) -> i64 {
        match self.modal_key {
            Some(modal) => {
                let extra = self.modal_extra.clone();
                if !(self.hit_within(target, modal) || extra.iter().any(|e| self.hit_within(target, *e))) {
                    modal
                } else {
                    target
                }
            }
            None => target,
        }
    }

    fn pointer_target(&mut self, point: (f32, f32)) -> i64 {
        match self.active {
            Some(target) => target,
            None => {
                let top = self.topmost(point);
                self.modal_target(top)
            }
        }
    }

    fn lift(&mut self) {
        if self.gesture_key != 0 {
            let key = self.gesture_key;
            let velocity = self.gesture_velocity;
            let value = self.accumulator(key);
            value.scroll_lift = true;
            value.scroll_velocity = velocity;
        }
        self.gesture_key = 0;
    }

    fn route(&mut self, frame: &rframe::Frame) {
        for (_, mut value) in self.signals.drain() {
            for (_, modifiers) in value.keys.drain(..) { self.spare_modifiers.push(modifiers); }
            if value.keys.capacity() > 0 { self.spare_key_lists.push(std::mem::take(&mut value.keys)); }
            self.spare_signals.push(value);
        }
        self.out.cursor = None;
        self.frame_events.clone_from(&frame.input.events);
        // stated per build: this frame's build names it again
        let default_focus = std::mem::take(&mut self.default_focus);
        self.taken.clear();
        self.taken.resize(self.frame_events.len(), std::cell::Cell::new(false));
        keep_frame(&mut self.input_frame, frame);
        self.routed_events.clear();
        self.cancelled.clear();
        if let Some((payload, _)) = self.dropped.take() { self.spare_payload = Some(payload); }
        // a carry lives with the press that holds it: capture taken away since (a popup, a host
        // dismissal) takes the payload too, so no later release puts it
        if let Some((_, Origin::FromPointer(owner))) = &self.payload {
            if self.active != Some(*owner) {
                self.retire_payload();
            }
        }
        let mut released_carry = false;
        self.passed_undo = None;
        self.modal_in_frame = self.modal_key.is_some();
        self.routed_modal = self.modal_key;
        let mut modifiers = std::mem::take(&mut self.route_keys);
        input::keys_before_into(&self.previous_keys, &frame.input.keys, &frame.input.events, &mut modifiers);
        self.frame_modifiers = modifiers.iter().fold(0, |mask, &key| mask | modifier_bit(key));
        self.previous_keys.clone_from(&frame.input.keys);
        self.view_w = frame.input.size.0 as f32;
        self.view_h = frame.input.size.1 as f32;
        // a trackpad's motion also arrives as wheel steps: in such a frame those only feed
        // `signal.scroll`, and the gesture moves the scroll box
        let phased = frame.input.events.iter().any(|e| match e {
            Event::TrackpadScrolled { delta: (dx, dy), .. } => *dx != 0.0 || *dy != 0.0,
            _ => false,
        });
        self.touches.clock = frame.input.t;
        for (event_index, original) in frame.input.events.iter().enumerate() {
            let touch_owner = self.touch_owner(original);
            let actions = self.touch_events(original);
            for (step, event) in std::iter::once(original).chain(actions.iter().flatten()).enumerate() {
                input::update_keys(&mut modifiers, event);
                let command = self.command_modifiers(&modifiers);
                if let Event::KeyPressed(Key::Tab) = event {
                    if !command && !self.flags_of_key(self.focus).any(Flags::KEEP_TAB) {
                        self.traverse_focus(modifiers.contains(&Key::Shift));
                    }
                }
                let (owner, delta): (i64, (f64, f64)) = match event {
                    Event::PointerMoved(point) => {
                        let owner = self.pointer_target(p32(*point));
                        let delta = if self.pointer.0.is_finite() {
                            (point.0 - self.pointer.0 as f64, point.1 - self.pointer.1 as f64)
                        } else {
                            (0.0, 0.0)
                        };
                        (owner, delta)
                    }
                    Event::Touch(t)=>(touch_owner.unwrap_or_else(|| self.pointer_target(p32(t.point))),(0.0,0.0)),
                    Event::PointerPressed(_, point) | Event::PointerReleased(_, point) => {
                        (self.pointer_target(p32(*point)), (0.0, 0.0))
                    }
                    Event::Scrolled(_) | Event::Pinched(_) => {
                        let target = match self.active {
                            Some(target) => target,
                            None => {
                                let scroll = self.scroll_target(self.pointer);
                                let fallback = if scroll != 0 { scroll } else { self.topmost(self.pointer) };
                                self.modal_target(fallback)
                            }
                        };
                        (target, (0.0, 0.0))
                    }
                    Event::KeyPressed(Key::Tab) if !command => {
                        (if self.focus == 0 { ROOT_KEY } else { self.focus }, (0.0, 0.0))
                    }
                    Event::KeyPressed(_) | Event::KeyReleased(_) | Event::TextInput(_) | Event::TextEditing { .. } | Event::Paste(_) => {
                        (self.focus, (0.0, 0.0))
                    }
                    _ => (0, (0.0, 0.0)),
                };
                if step == 0 { self.routed_events.push((owner, event.clone(), delta)); }
                else { self.frame_events.push(event.clone()); self.taken.push(std::cell::Cell::new(false)); }
                match event {
                    Event::PointerMoved(point) => {
                        let (px, py) = self.pointer;
                        self.pointer = p32(*point);
                        if let Some(active) = self.active {
                            let (x, y) = self.pointer;
                            let value = self.accumulator(active);
                            if px.is_finite() {
                                value.drag_x += x - px;
                                value.drag_y += y - py;
                            }
                            value.moved = true;
                        }
                    }
                    // a carry started from a key: a left press is the put, on what is under it, and
                    // nothing else sees the press
                    Event::PointerPressed(MouseButton::LeftButton, point)
                        if matches!(self.payload, Some((_, Origin::FromKey))) =>
                    {
                        self.pointer = p32(*point);
                        if let Some((payload, _)) = self.payload.take() {
                            self.dropped = Some((payload, self.topmost(p32(*point))));
                        }
                        if let Some(last) = self.routed_events.last_mut() {
                            last.0 = -1;
                        }
                    }
                    Event::PointerPressed(button, point) => {
                        self.pointer = p32(*point);
                        let target = if owner == 0 || self.flags_of_key(owner).any(Flags::CLICKABLE) {
                            owner
                        } else {
                            let mut found = None;
                            self.hit_ancestors(owner, &mut |key| {
                                if found.is_none() && self.flags_of_key(key).any(Flags::CAPTURE_UNCLAIMED) {
                                    found = Some(key);
                                }
                            });
                            found.unwrap_or(owner)
                        };
                        let mut chain = std::mem::take(&mut self.press_chain);
                        chain.clear();
                        self.hit_ancestors(target, &mut |key| chain.push(key));
                        for &key in &chain {
                            self.accumulator(key).subtree_press = Some(event_index as i64);
                        }
                        self.press_chain = chain;
                        let flags = self.flags_of_key(target);
                        if *button == MouseButton::LeftButton {
                            // the nearest focusable box at or around the press takes the focus unless
                            // it is a Tab stop only (buttons, as on macOS); a list takes it from its rows
                            let mut holder = 0;
                            if !flags.any(Flags::KEEP_FOCUS) {
                                self.hit_ancestors(target, &mut |key| {
                                    if holder == 0 && self.flags_of_key(key).any(Flags::FOCUSABLE) {
                                        holder = key;
                                    }
                                });
                            }
                            let focus = if flags.any(Flags::KEEP_FOCUS) {
                                self.focus
                            } else if holder != 0 && !self.flags_of_key(holder).any(Flags::TAB_ONLY) {
                                holder
                            } else {
                                0
                            };
                            if focus != self.focus {
                                self.composition.clear();
                                self.edit_focus = 0;
                            }
                            self.focus = focus;
                            self.keyboard_focus = false;
                        }
                        if (target == 0 || flags.any(Flags::CLICKABLE | Flags::CAPTURE_UNCLAIMED)) && self.active.is_none() {
                            self.active = Some(target);
                            self.active_button = *button;
                            self.active_press = self.pointer;
                            self.active_keys.clone_from(&modifiers);
                            let pointer = self.pointer;
                            let value = self.accumulator(target);
                            value.pressed = true;
                            value.press_point = pointer;
                            value.press_keys.clone_from(&modifiers);
                            value.button = Some(*button);
                            if let Some(slot) = self.table.find(target).filter(|s| *s < self.slots.len()) {
                                if *button == MouseButton::LeftButton {
                                    let (x, y) = pointer;
                                    let s = &self.slots[slot];
                                    let dx = x - s.press_x;
                                    let dy = y - s.press_y;
                                    // the second press within 0.35 s and 5 points is a double click,
                                    // the third a triple; the fourth starts over
                                    let count = if frame.input.t >= s.press_time
                                        && frame.input.t - s.press_time <= 0.35
                                        && dx * dx + dy * dy <= 25.0
                                        && s.press_count < 3
                                    {
                                        s.press_count + 1
                                    } else {
                                        1
                                    };
                                    let s = &mut self.slots[slot];
                                    s.press_count = count;
                                    s.press_time = frame.input.t;
                                    s.press_x = x;
                                    s.press_y = y;
                                    let value = self.accumulator(target);
                                    value.clicks = count;
                                    value.double_clicked = count == 2;
                                }
                            }
                        }
                    }
                    Event::PointerReleased(button, point) => {
                        self.pointer = p32(*point);
                        if let Some(active) = self.active {
                            if *button == self.active_button {
                                let carried = matches!(&self.payload, Some((_, Origin::FromPointer(owner))) if *owner == active && *button == MouseButton::LeftButton);
                                released_carry |= carried;
                                let pointer = self.pointer;
                                let active_press = self.active_press;
                                let active_keys = std::mem::take(&mut self.active_keys);
                                let index = self.hit_index(active);
                                let clicked = !carried && self.carry_latch != Some(active) && *button == MouseButton::LeftButton
                                    && index.is_some_and(|i| self.hit_contains(i, pointer) && self.hit_contains(i, active_press));
                                let value = self.accumulator(active);
                                value.released = true;
                                value.release_point = pointer;
                                value.press_point = active_press;
                                value.press_keys.clone_from(&active_keys);
                                value.button = Some(*button);
                                if clicked {
                                    value.clicked = true;
                                }
                                self.active_keys = active_keys;
                                self.active = None;
                            }
                        }
                    }
                    Event::Scrolled((horizontal, vertical)) => {
                        let scroll = self.scroll_target(self.pointer);
                        let target = self.modal_target(scroll);
                        if target != 0 {
                            let value = self.accumulator(target);
                            value.scroll_x += *horizontal as f32;
                            value.scroll_y_steps += *vertical as f32;
                            if !phased {
                                value.scroll_wheel += *vertical as f32;
                                value.scroll_wheel_x += *horizontal as f32;
                            }
                        }
                    }
                    // The fingers on a trackpad: the gesture stays with the box it began over, and the
                    // velocity it is released with is its last motion over the time since the one
                    // before. The system's own momentum is not used: the box coasts by itself.
                    Event::TrackpadScrolled { phase: ScrollPhase::Momentum, .. } => {}
                    Event::TrackpadScrolled { phase: ScrollPhase::Lifted, time, .. } => {
                        // fingers that rested before they lifted leave the content where it is
                        if time - self.gesture_time > 0.1 {
                            self.gesture_velocity = (0.0, 0.0);
                        }
                        self.lift();
                    }
                    Event::TrackpadScrolled { delta: (horizontal, vertical), phase, time } => {
                        if *phase == ScrollPhase::Touched || self.gesture_key == 0 {
                            let scroll = self.scroll_target(self.pointer);
                            self.gesture_key = self.modal_target(scroll);
                            self.gesture_time = f64::NAN;
                            self.gesture_velocity = (0.0, 0.0);
                        }
                        let dt = time - self.gesture_time;
                        self.gesture_velocity = if dt > 0.0 { (-vertical / dt, -horizontal / dt) } else { (0.0, 0.0) };
                        self.gesture_time = *time;
                        if self.gesture_key != 0 {
                            let key = self.gesture_key;
                            let value = self.accumulator(key);
                            if *phase == ScrollPhase::Touched {
                                value.scroll_touch = true;
                                value.scroll_lift = false;
                                value.scroll_velocity = (0.0, 0.0);
                                value.scroll_finger = false;
                            }
                            value.scroll_drag += *vertical as f32;
                            value.scroll_drag_x += *horizontal as f32;
                        }
                    }
                    // a trackpad pinch goes where the wheel goes
                    Event::Pinched(scale) => {
                        let scroll = self.scroll_target(self.pointer);
                        let target = self.modal_target(scroll);
                        if target != 0 {
                            self.accumulator(target).pinch *= *scale;
                        }
                    }
                    // Cancellation ends capture without a release: no click, no commit.
                    Event::PointerCancelled(button) => {
                        if *button == self.active_button {
                            self.active = None;
                        }
                        if let Some((_, Origin::FromPointer(_))) = &self.payload {
                            if *button == MouseButton::LeftButton {
                                self.retire_payload();
                                released_carry = false;
                            }
                        }
                    }
                    Event::FocusLost => {
                        self.gesture_velocity = (0.0, 0.0);
                        self.lift();
                        self.retire_payload();
                        released_carry = false;
                        self.active = None;
                        self.focus = 0;
                        self.hot = 0;
                        self.composition.clear();
                        self.edit_focus = 0;
                        self.keyboard_focus = false;
                    }
                    Event::KeyPressed(_) | Event::KeyReleased(_) | Event::TextInput(_) | Event::TextEditing { .. } | Event::Paste(_) => {
                        // the default key target's nearest focusable box, unless a modal leaves it out
                        let mut holder = 0;
                        if default_focus != 0 && matches!(event, Event::KeyPressed(_)) {
                            self.hit_ancestors(default_focus, &mut |key| if holder == 0 && self.flags_of_key(key).any(Flags::FOCUSABLE) { holder = key; });
                            if !self.hit_index(holder).is_some_and(|index| self.eligible(index)) { holder = 0; }
                        }
                        // it takes the focus while no node holds it; while a widget other than a text editor holds it, the press
                        // falls through to it too, which acts only on a press the focused widget left untaken
                        // shortcut: "left untaken" holds when the focused widget builds before the default target (a sidebar before
                        // its content); a target built first sees the press first. Upgrade if a window builds its default target first.
                        if self.focus == 0 { self.focus = holder; }
                        let fall = if holder != self.focus && self.focus != self.edit_focus { holder } else { 0 };
                        for target in [self.focus, fall] {
                            if target == 0 { continue; }
                            // Empty release buffers must not displace warmed press capacity.
                            let reusable = self.spare_modifiers.iter().enumerate()
                                .filter(|(_, keys)| keys.capacity() >= modifiers.len())
                                .min_by_key(|(_, keys)| keys.capacity()).map(|(index, _)| index);
                            let mut mods = reusable.map(|index| self.spare_modifiers.swap_remove(index))
                                .unwrap_or_else(|| self.spare_modifiers.pop().unwrap_or_default());
                            mods.clear();
                            mods.extend_from_slice(&modifiers);
                            // a focus taking its first key this frame takes a warmed key list (recycled accumulators hand theirs to
                            // `spare_key_lists`, so whichever accumulator a node gets, a key it takes reuses one)
                            let warmed = if self.signals.get(&target).is_none_or(|a| a.keys.capacity() == 0) { self.spare_key_lists.pop() } else { None };
                            let value = self.accumulator(target);
                            if let Some(keys) = warmed { value.keys = keys; }
                            value.keys.push((event.clone(), mods));
                        }
                        if self.focus != 0 {
                            let focus = self.focus;
                            // a focused control (not a text editor) presses on Enter and Space: those keys are the control's
                            let control = step == 0 && focus != self.edit_focus && self.flags_of_key(focus).any(Flags::CLICKABLE);
                            let value = self.accumulator(focus);
                            if let Event::KeyPressed(Key::Enter) | Event::KeyPressed(Key::Space) | Event::KeyPressed(Key::KeyChar(' ')) = event {
                                if !command {
                                    value.clicked = true;
                                    if control { self.taken[event_index].set(true); }
                                }
                            }
                            match event {
                                Event::TextEditing { text, start, length } => {
                                    self.composition.clone_from(text);
                                    self.composition_selection = (*start, *length);
                                }
                                Event::TextInput(_) => self.composition.clear(),
                                Event::KeyPressed(Key::Escape) if self.flags_of_key(self.focus).any(Flags::TAB_ONLY) => {
                                    self.focus = 0;
                                    self.keyboard_focus = false;
                                    self.edit_focus = 0;
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        self.gesture_tick(frame.input.t);
        let (mouse_x, mouse_y) = p32(frame.input.pointer);
        self.route_keys = modifiers;
        if !self.pointer.0.is_finite() {
            self.pointer = (mouse_x, mouse_y);
        }
        self.hot = if frame.input.events.iter().any(|e| matches!(e, Event::FocusLost)) {
            0
        } else {
            self.topmost(self.pointer)
        };
        if self.active.is_none() {
            self.carry_latch = None;
        }
        // released: what the carry was put on is what was hot at the release
        if released_carry {
            if let Some((payload, _)) = self.payload.take() {
                self.dropped = Some((payload, self.hot));
            }
        }
    }

    pub fn wants_pointer(&self) -> bool {
        self.hot != 0 || self.active.is_some_and(|a| a != 0)
    }

    /// Cursor requested by a hovered or captured PXUI control; the last request of a frame wins.
    pub fn request_cursor(&mut self, shape: Cursor) {
        self.out.cursor = Some(shape);
    }

    /// A text editor holds the keyboard focus: the focus is the box whose edit was loaded since the focus came to it.
    pub fn text_input_focused(&self) -> bool {
        self.focus != 0 && self.focus == self.edit_focus
    }

    /// A focused widget handled a press of `key` this frame: the first press of it not taken yet is no longer reported by
    /// `key_pressed` (app shortcuts see only the keys no widget handled).
    pub(crate) fn take_key(&self, key: Key) {
        if let Some(i) = self.frame_events.iter().enumerate().position(|(i, e)| matches!(e, Event::KeyPressed(k) if *k == key) && !self.taken[i].get()) {
            self.taken[i].set(true);
        }
    }

    pub fn passed_undo(&self) -> Option<UndoKind> {
        self.passed_undo
    }

    pub(crate) fn set_passed_undo(&mut self, kind: UndoKind) {
        self.passed_undo = Some(kind);
    }

    /// Raw held keys, including modifiers while a widget or popup owns keyboard input.
    pub fn key_down(&self, key: Key) -> bool {
        self.input_frame.as_ref().is_some_and(|frame| frame.input.keys.contains(&key))
    }

    /// The key was pressed in this frame's events (a dialog's Enter); with `modifier`, only while that modifier was held at the
    /// event, including same-frame releases.
    pub fn key_pressed(&self, key: Key, modifier: Option<Key>) -> bool {
        let modifiers = match modifier {
            None => 16,
            Some(modifier) => {
                let bit = modifier_bit(modifier);
                if bit == 0 { return false; }
                bit
            }
        };
        self.key_pressed_mods(key, modifiers)
    }

    /// Modifier mask 0 requires no held modifiers; 16 accepts a press with any modifiers.
    pub fn key_pressed_mods(&self, key: Key, modifiers: u32) -> bool {
        let free = |i: usize| !self.taken[i].get();
        if modifiers == 16 { return self.frame_events.iter().enumerate().any(|(i, e)| matches!(e, Event::KeyPressed(k) if *k == key) && free(i)); }
        let mut held = self.frame_modifiers;
        for (i, event) in self.frame_events.iter().enumerate() {
            match event {
                Event::KeyPressed(k) => {
                    held |= modifier_bit(*k);
                    if *k == key && free(i) && (if modifiers == 0 { held == 0 } else { held & modifiers == modifiers }) { return true; }
                }
                Event::KeyReleased(k) => held &= !modifier_bit(*k),
                Event::FocusLost => held = 0,
                _ => {}
            }
        }
        false
    }

    pub fn unfocus(&mut self) {
        self.focus = 0;
        self.composition.clear();
        self.edit_focus = 0;
        self.keyboard_focus = false;
    }

    /// When the host closes a popup after accepting its result, release focus and capture. Its
    /// current frame remains consumed; next frame is unblocked.
    pub fn dismiss_popup(&mut self) {
        self.cancel_touch_for_popup(None, &[]);
        if let Some(key) = self.modal_key {
            self.dismissed_popups.push(key);
            self.dismissed_popups.extend_from_slice(&self.modal_extra);
        }
        self.modal_key = None;
        self.modal_extra.clear();
        self.unfocus();
        if self.active.is_some() {
            self.cancelled.insert(0, self.active_button);
            self.active = None;
        }
    }

    pub(crate) fn composition(&self) -> &str {
        &self.composition
    }

    pub(crate) fn pointer(&self) -> (f32, f32) {
        self.pointer
    }

    pub(crate) fn frame_events(&self) -> &[Event] {
        &self.frame_events
    }

    pub(crate) fn input_frame(&self) -> Option<&rframe::Frame> {
        self.input_frame.as_ref()
    }

    pub(crate) fn active(&self) -> Option<i64> {
        self.active
    }

    pub(crate) fn focus_key(&self) -> i64 {
        self.focus
    }

    /// Event ownership is recorded before release/cancellation clears capture. A viewport passes
    /// its exact root key: child widgets retain their events.
    pub fn input(&self, owner: Option<i64>) -> rframe::Frame {
        let mut out=rframe::Frame {input:input::at_time(0.0),pixel_scale:(1.0,1.0),fps:0.0,mouse_delta:(0.0,0.0)};
        self.input_into(owner,&mut out);
        out
    }

    pub fn input_into(&self,owner:Option<i64>,out:&mut rframe::Frame) {
        let owner = owner.unwrap_or(0);
        let frame = match &self.input_frame {
            Some(frame) => frame,
            None => panic!("Ui.input: no frame has completed"),
        };
        let accepts = |target: i64| !self.modal_in_frame && (target == 0 || target == owner);
        let accepts_pointer = |target: i64| accepts(target) && (owner == 0 || target == owner);
        let mut events=std::mem::take(&mut out.input.events);
        events.clear();
        let (mut dx, mut dy) = (0.0, 0.0);
        let mut complete = true;
        for (target, event, (mx, my)) in &self.routed_events {
            match event {
                Event::FocusLost | Event::PointerCancelled(_) => events.push(event.clone()),
                _ => {
                    let ok = match event {
                        Event::PointerPressed(..) | Event::PointerReleased(..) | Event::PointerMoved(_) | Event::Scrolled(_) | Event::Touch(_) => {
                            accepts_pointer(*target)
                        }
                        _ => accepts(*target),
                    };
                    if ok {
                        events.push(event.clone());
                        dx += mx;
                        dy += my;
                    } else if let Event::PointerReleased(button, _) = event {
                        events.push(Event::PointerCancelled(*button));
                        complete = false;
                    } else if let Event::Touch(t) = event {
                        if matches!(t.phase, input::TouchPhase::Ended | input::TouchPhase::Cancelled) {
                            events.push(Event::Touch(input::Touch { phase: input::TouchPhase::Cancelled, ..*t }));
                        }
                        complete = false;
                    } else {
                        complete = false;
                    }
                }
            }
        }
        let held = self.active.is_some_and(|a| accepts_pointer(a));
        let pointer = held || (self.active.is_none() && frame.input.buttons.is_empty() && accepts_pointer(self.hot));
        for button in &self.cancelled {events.push(Event::PointerCancelled(*button));}
        if self.modal_in_frame {for button in &frame.input.buttons {events.push(Event::PointerCancelled(*button));}}
        out.input.events = events;
        out.input.keys.clear();
        if !self.modal_in_frame && self.focus==0 {out.input.keys.extend_from_slice(&frame.input.keys);}
        out.input.buttons.clear();
        if held {out.input.buttons.extend_from_slice(&frame.input.buttons);}
        let i=&frame.input;
        (out.input.t,out.input.dt,out.input.frame,out.input.tick,out.input.size,out.input.pointer)=(i.t,i.dt,i.frame,i.tick,i.size,i.pointer);
        (out.pixel_scale,out.fps)=(frame.pixel_scale,frame.fps);
        out.mouse_delta = if !pointer {
            (0.0, 0.0)
        } else if complete {
            frame.mouse_delta
        } else {
            (dx, dy)
        };
    }

    /// When the last frame's UI needs another frame with no input: at once for a gesture, a
    /// running transition, a scroll in motion or a requested frame; at the end of a running hover
    /// delay or the earliest `repaint_after`; `None` when time alone changes nothing. Keyboard
    /// focus alone asks for nothing (no caret blinks).
    pub fn repaint_at(&self) -> Option<f64> {
        if self.frame_requested {
            Some(f64::NEG_INFINITY)
        } else {
            // a `repaint_after` made between frames
            Some(self.out.repaint_at.unwrap_or(f64::INFINITY).min(self.wake).min(self.touches.wake)).filter(|t| t.is_finite())
        }
    }

    /// The highest active ambient rate in Hz, or zero for display-rate and inactive frames.
    pub fn repaint_hz(&self) -> u32 {
        if self.frame_requested { 0 } else { u32::from(self.repaint_hz) }
    }

    /// The frame gate: build, lay out and paint only for input or a due repaint. A host that gets
    /// `false` keeps the last frame and waits for input or `repaint_at`.
    pub fn wants_frame(&self, events: &[Event], now: f64) -> bool {
        !events.is_empty() || self.repaint_at().is_some_and(|at| now >= at)
    }

    /// Ask for a frame: between frames, the next one; while building, the one after this (an
    /// animation or timer the app drives calls it every frame it runs).
    pub fn request_frame(&mut self) {
        self.frame_requested = true;
        self.escape();
    }

    /// The view being built did what a skipped build would lose (a box at the root, a frame request,
    /// a hover delay): it rebuilds next frame too.
    pub(crate) fn escape(&mut self) {
        if self.building && self.view_cur != 0 {
            self.views[self.view_cur as usize].escapes = true;
        }
    }

    /// Ask for a frame `seconds` after this frame's time (a timer: a toast's expiry, a delayed
    /// reveal), building the view that asks again then; `repaint_at_time` takes the frame clock's
    /// time. A request stands until its time (a view's until the view rebuilds); the earliest is
    /// `repaint_at`.
    pub fn repaint_after(&mut self, seconds: f64) {
        self.repaint_at_time(self.now + seconds.max(0.0));
    }

    /// The time of the frame being built (the last frame's between frames), in the frame clock's
    /// seconds.
    pub fn now(&self) -> f64 {
        self.now
    }

    /// `repaint_after` at the frame clock's `time`.
    pub fn repaint_at_time(&mut self, time: f64) {
        if self.building && self.view_cur != 0 {
            let r = &mut self.views[self.view_cur as usize];
            r.wake = r.wake.min(time);
        } else {
            self.wake = self.wake.min(time);
        }
    }

    /// Cumulative scheduling diagnostics. Disabled counters stay at zero.
    pub fn frame_reasons(&self) -> PxuiFrameReasons {
        self.frame_reasons
    }

    fn advance_ambient(&mut self, now: f64) {
        if !self.ambient_wake.is_finite() || self.now - self.input_at < LOOP_IDLE || now < self.now {
            self.ambient_at = now;
            self.ambient_epoch = now;
        } else if now >= self.ambient_wake {
            // Drop missed samples but keep their phase across late callbacks and builds.
            let hz = f64::from(self.ambient_hz);
            let tick = ((now - self.ambient_epoch) * hz).ceil();
            let at = self.ambient_epoch + tick / hz;
            let sample = if at <= now { at } else { self.ambient_epoch + (tick - 1.0) / hz };
            self.ambient_at = sample.max(self.ambient_wake).min(now);
        }
    }

    fn next_repaint(&mut self, now: f64) -> Option<f64> {
        use pxui_types::ui::Rate;
        let trace = self.trace_frame_reasons.load(Ordering::Relaxed);
        let requested = self.frame_requested || self.keyboard.flags & pxui_types::Keyboard::ANIMATING != 0;
        let interacting = self.active.is_some() || !self.moving.is_empty() || !self.gsettle.is_empty();
        if trace {
            if requested { self.frame_reasons.requested += 1; }
            else if interacting { self.frame_reasons.interaction += 1; }
        }
        let mut display = requested || interacting;
        let mut hz = 0;
        let promoted = !self.promotion.current.is_empty();
        for tween in &self.tweens {
            if !self.tween_visible(tween) || (promoted && self.tween_promoted(tween)) { continue; }
            let keys = self.keyframes.get(tween.keys as usize);
            match keys.map_or(Rate::Display, |keys| keys.rate) {
                Rate::Display => {
                    if trace && !display {
                        match keys {
                            None => {
                                self.frame_reasons.transition += 1;
                                self.frame_reasons.transition_key = self.slots[tween.slot].key;
                                self.frame_reasons.transition_curve = tween.curve.into();
                            }
                            Some(keys) if keys.repeat != 0 => {
                                self.frame_reasons.finite_keys += 1;
                                let elapsed = keys.time(now - tween.start);
                                let segment = keys.t[..keys.n].iter().position(|&end| elapsed < end).unwrap_or(keys.n - 1);
                                let start = if segment == 0 { 0.0 } else { keys.t[segment - 1] };
                                self.frame_reasons.transition_key = self.slots[tween.slot].key;
                                self.frame_reasons.transition_curve = keys.curve[segment].with_duration(keys.t[segment] - start).into();
                            }
                            Some(_) => self.frame_reasons.active_loop += 1,
                        }
                    }
                    display = true;
                }
                Rate::Ambient(rate) => hz = hz.max(rate),
            }
        }
        if hz != self.ambient_hz {
            self.ambient_epoch = self.ambient_at;
            self.ambient_hz = hz;
        }
        self.ambient_wake = if hz == 0 {
            f64::INFINITY
        } else {
            let hz = f64::from(hz);
            let tick = ((self.ambient_at - self.ambient_epoch) * hz).round();
            self.ambient_epoch + (tick + 1.0) / hz
        };
        let hold_display = hz != 0 && now - self.input_at < LOOP_IDLE;
        self.repaint_hz = if display || hold_display { 0 } else { hz };
        if display { return Some(now); }
        if hold_display {
            if trace { self.frame_reasons.active_loop += 1; }
            return Some(now);
        }
        if trace && hz != 0 { self.frame_reasons.idle_loop += 1; }
        let wake = self.views.iter().skip(1).fold(self.wake.min(self.touches.wake).min(self.ambient_wake), |w, r| w.min(r.wake));
        let hover = match &self.hover_rest {
            Some((_, _, start, seen)) if *seen == self.frame_number && now - start < HOVER_SECONDS => start + HOVER_SECONDS,
            _ => f64::INFINITY,
        };
        Some(wake.min(hover)).filter(|t| t.is_finite())
    }
}

/// What a hover wait is keyed by: the C export's text (copied when it starts) or a resolved key (copied as a number).
pub(crate) enum HoverId { Text(String), Key(i64) }

impl HoverId {
    /// Whether this is the wait asked for by `text` (the C export's), else by the resolved `key`.
    fn is(&self, text: Option<&str>, key: i64) -> bool {
        match (self, text) { (HoverId::Text(a), Some(b)) => a == b, (HoverId::Key(a), None) => *a == key, _ => false }
    }
}

#[inline]
fn modifier_bit(key: Key) -> u32 {
    match key {
        Key::Shift => 1,
        Key::Ctrl => 2,
        Key::Alt => 4,
        Key::Meta => 8,
        _ => 0,
    }
}

/// `label` cut to `limit` points wide with an ellipsis, on a character boundary. `width` measures
/// text as the sum of its characters, so one pass from the start measures each character once and
/// stops at the first that does not fit: the cost follows what is shown, not the label's length.
pub const PHASES: usize = 7;
pub const PHASE_NAMES: [&str; PHASES] = ["begin", "build", "intrinsic", "arrange", "paint", "a11y", "finish"];

pub(crate) const HOVER_SECONDS: f64 = 0.380;

// ------------------------------------------------------------- boxes

/// What `enter`, `scope` and `enter_root` change; `leave` puts it back. Each only pushes (a root
/// pushes the root box), so the depths restore it with no allocation.
pub enum Saved {
    Depths(usize, usize),
}

#[derive(Clone, Debug, Default)]
pub struct Signal {
    /// topmost box under the pointer, or captured
    pub hovered: bool,
    /// a press on this box began this frame
    pub pressed: bool,
    /// Ordinal of the last press on this box or any hit descendant this frame.
    pub subtree_press: Option<i64>,
    /// holds pointer capture after this frame's events
    pub held: bool,
    /// capture ended this frame
    pub released: bool,
    /// left press and release both inside the hit rect
    pub clicked: bool,
    pub double_clicked: bool,
    /// on the frame of a left press: 1, or 2 and 3 for the second and third press within 0.35 s
    /// and 5 points; 0 otherwise
    pub clicks: i64,
    /// captured and the pointer moved this frame
    pub dragging: bool,
    /// captured pointer motion this frame
    pub drag: (f32, f32),
    /// latest pointer, screen space
    pub pointer: (f32, f32),
    /// where the current or last press began
    pub press_point: (f32, f32),
    pub release_point: (f32, f32),
    pub button: Option<MouseButton>,
    /// wheel steps routed to this box
    pub scroll: (f32, f32),
    /// product of the trackpad pinch factors routed to this box this frame
    pub pinch: f64,
    /// ordered key/text events while focused
    pub keys: Vec<Event>,
    /// a value an assistive technology set this frame: the widget applies it as one edit
    pub a11y_value: Option<f64>,
    /// a selection (anchor, caret in chars) an assistive technology set this frame on a focused text field
    pub a11y_select: Option<(u32, u32)>,
}

/// The kinds of node keyed by their place among their scope's nodes of the same kind (D25).
#[derive(Clone, Copy)]
pub(crate) enum Positional { Container = 1, Label = 2, Image = 3 }

/// The first key of the n-th `kind` node under `seed`, in a hash domain of its own: authored labels and ids hash their bytes from
/// the seed (`key_of`) and a resolved label is an existing box key, while this mixes the seed and the kind (a splitmix64 finaliser
/// over a domain constant), so no authored key is derived through it. Repeats take `#dup` keys as any repeated key does.
#[inline]
pub(crate) fn positional_key(seed: i64, kind: Positional) -> i64 {
    let mut z = (seed as u64) ^ 0x7078_7569_706f_7300 ^ (kind as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    let key = (z ^ (z >> 31)) as i64;
    if key == 0 || key == 1 { key + 2 } else { key }
}

impl Ui {
    /// The frame's number (a retained row's key changes with it while the row stays live).
    pub(crate) fn frame_no(&self) -> i64 { self.frame_number }
    /// The rows being built are outside their list's viewport (culled): a kept view among them keeps its build (`view_entry`).
    pub(crate) fn out_of_view(&self) -> bool { self.out_of_view }

    pub(crate) fn require_building(&self) {
        if !self.building {
            panic!("Ui: boxes must be built inside Ui.frame");
        }
    }

    pub(crate) fn current_parent(&self) -> usize {
        self.parents.last().copied().unwrap_or(0)
    }

    pub fn current_seed(&self) -> i64 {
        self.seeds.last().copied().unwrap_or(0)
    }

    /// The seed a lookup resolves an authored key under: the active scope's during a build, `ROOT_KEY` outside one.
    pub fn lookup_seed(&self) -> i64 {
        self.seeds.last().copied().unwrap_or(ROOT_KEY)
    }

    /// Two boxes with one key in a frame get distinct, order-stable keys: the n-th takes the key's
    /// n-th `#dup` hash. The first key's slot remembers the last duplicate taken this frame, so the
    /// n-th hashes on from there without looking it up again: two lookups and one hash, whatever n.
    /// With the key, its slot when it has one.
    #[inline(always)]
    pub(crate) fn unique_key(&mut self, first: i64) -> (i64, Option<usize>) {
        self.unique_key_from(first, first, None)
    }

    /// `unique_key` for a widget whose siblings repeat its key (layout containers, labels and images key by position): when
    /// `first` is the key duplicated last this frame, the next duplicate is hashed on from the one taken, with no lookup of `first`.
    pub(crate) fn unique_key_repeated(&mut self, first: i64) -> (i64, Option<usize>) {
        let last = self.last_dup;
        if last.0 == self.frame_number && last.1 == first { self.unique_key_from(first, hash_range(last.3, "#dup", 0, 4), Some(last.2)) } else { self.unique_key_from(first, first, None) }
    }

    /// `unique_key` from candidate `key`, the slot of `first` (`head`) when it is known to be taken.
    #[inline(always)]
    fn unique_key_from(&mut self, first: i64, mut key: i64, mut head: Option<usize>) -> (i64, Option<usize>) {
        assert!(first != 0 && first != 1, "Ui: keys 0 and 1 are reserved; derive integer keys with int_key");
        loop {
            match self.table.find(key) {
                Some(slot) if self.slots[slot].touched == self.frame_number => {
                    let dup = self.slots[slot].dup;
                    if head.is_none() {
                        head = Some(slot);
                        if dup.0 == self.frame_number {
                            // the last duplicate taken this frame is taken: hash on from it
                            key = hash_range(dup.1, "#dup", 0, 4);
                            continue;
                        }
                    }
                    key = hash_range(key, "#dup", 0, 4);
                }
                found => {
                    if let Some(head) = head {
                        self.slots[head].dup = (self.frame_number, key);
                        self.last_dup = (self.frame_number, first, head, key);
                    }
                    return (key, found);
                }
            }
        }
    }

    fn append_box(&mut self, key: i64, found: Option<usize>, parent: usize) -> usize {
        let index = self.count;
        if index == self.boxes.len() {
            self.boxes.push(BoxRec::blank());
            self.l_x.push(0.0);
            self.l_y.push(0.0);
            self.l_w.push(0.0);
            self.l_h.push(0.0);
            self.l_content.push(0.0);
            self.l_content_w.push(0.0);
        }
        self.count = index + 1;
        let slot = self.slot_of(key, found);
        self.slots[slot].new_identity = found.is_none();
        self.slots[slot].touched = self.frame_number;
        self.slots[slot].label_count = 0;
        self.slots[slot].owner = self.view_cur;
        self.slots[slot].parent_slot = if parent == NONE { NONE } else { self.boxes[parent].slot };
        self.views[self.view_cur as usize].nodes.push(slot);
        let b = &mut self.boxes[index];
        b.key = key;
        b.slot = slot;
        b.view = NO_VIEW;
        b.kept = false;
        b.ahead = self.ahead;
        b.parent = parent;
        b.first = NONE;
        b.last = NONE;
        b.next = NONE;
        if parent != NONE {
            let last = self.boxes[parent].last;
            if last == NONE {
                self.boxes[parent].first = index;
            } else {
                self.boxes[last].next = index;
            }
            self.boxes[parent].last = index;
        }
        index
    }

    /// Declare build box `index`: its node record takes `node`, and its text, hit shape and canvases
    /// start empty. A patch made since the node's last build ends here.
    fn set_box(&mut self, index: usize, node: &Node) {
        let slot = self.reset_box(index);
        let s = &mut self.slots[slot];
        *s.node = *node;
        self.clear_box_text(slot);
        if let Some(look) = self.resolve_look(&node.look) { self.slots[slot].node.look = look; }
    }

    fn reset_box(&mut self, index: usize) -> usize {
        let b = &mut self.boxes[index];
        b.hit = None;
        b.hit_rect = None;
        b.shape = HitShape::Own;
        b.paint = arena::EMPTY;
        b.over = arena::EMPTY;
        let s = &mut self.slots[b.slot];
        s.a11y_name.clear();
        s.a11y_value.clear();
        s.a11y_selection = (0, 0);
        s.a11y_number = None;
        b.slot
    }

    fn clear_box_text(&mut self, slot: usize) {
        self.slots[slot].text.clear();
        self.clear_rich(slot);
        self.cleared_strings(slot);
    }

    fn template_box(&mut self, key: &str) -> Handle {
        self.require_building();
        let (key, found) = self.unique_key(key_of(self.current_seed(), key));
        let index = self.append_box(key, found, self.current_parent());
        let slot = self.reset_box(index);
        UiBox { index, box_key: key, box_slot: slot, generation: self.slots[slot].generation, ui: self.ui_serial }
    }

    pub(crate) fn positioned_label(&mut self, key: &str, rect: Rect, color: Rgba, token: Option<pxui_types::theme::ColorToken>, text: &str, style: &Style) -> Handle {
        let b = self.template_box(key);
        let slot = b.box_slot;
        self.slots[slot].label_color = token;
        self.slots[slot].label_root_ink = self.theme_id == 0;
        let color = self.resolve_color(color);
        let n = self.slots[slot].node.prepare(NodeTemplate::PositionedLabel);
        (n.w, n.h, n.at, n.look.text) = (Size::Px(rect.w), Size::Px(rect.h), Some((rect.x, rect.y)).into(), color);
        if self.empty_strings.active() {
            for field in [StringField::Name, StringField::Value] {
                let capacity = field.get(&self.slots[slot]).capacity();
                if capacity != 0 { self.empty_strings.empty(field, slot, capacity); }
            }
        }
        self.write_text(slot, text);
        self.clear_rich(slot);
        self.slots[slot].rich = Some(NodeText { style: *style, spans: SpanRange::default(), line_box: true });
        b
    }

    /// The layout inputs and look of build box `index`.
    fn nd(&self, index: usize) -> &Node {
        &self.slots[self.boxes[index].slot].node
    }

    /// Declare a node under the current parent by its key.
    #[inline]
    pub fn node_keyed(&mut self, key: i64, node: Node) -> Handle {
        self.node_keyed_ref(key, &node)
    }

    /// `node_keyed` of a node the caller keeps: it is copied once, into the box.
    pub fn node_keyed_ref(&mut self, key: i64, node: &Node) -> Handle {
        self.require_building();
        let unique = self.unique_key(key);
        self.node_unique(unique, node)
    }

    /// Writes the handle directly into the ABI caller's output after its building-handle check.
    pub fn write_node_in_frame(&mut self, key: i64, node: &Node, out: &mut core::mem::MaybeUninit<UiBox>) {
        debug_assert!(self.building);
        let unique = self.unique_key(key);
        out.write(self.node_unique(unique, node));
    }

    /// A keyless layout container (`row`/`column`/`stack`): the n-th container of the current scope, keyed in the engine's positional
    /// namespace (`positional_key`), so containers keep their keys while labels, images or keyed siblings come and go.
    pub fn layout_node(&mut self, node: &Node) -> Handle {
        self.require_building();
        let unique = self.unique_key_repeated(positional_key(self.current_seed(), Positional::Container));
        self.node_unique(unique, node)
    }

    /// `node_keyed_ref` of a key `unique_key` already resolved this frame (with its slot): a widget that derives another node's key
    /// from its own resolves it first, so the derived one is unique too.
    #[inline(always)]
    pub(crate) fn node_unique(&mut self, (key, found): (i64, Option<usize>), node: &Node) -> Handle {
        let parent = self.current_parent();
        let index = self.append_box(key, found, parent);
        self.set_box(index, node);
        let slot = self.boxes[index].slot;
        UiBox { index, box_key: key, box_slot: slot, generation: self.slots[slot].generation, ui: self.ui_serial }
    }

    /// Declare a node under the current parent by `key` (see `pxui_types::ui::Key`): a label as `key_of`, an id as its NUL-and-hex
    /// label, both under the parent's seed.
    #[inline]
    pub fn node(&mut self, key: pxui_types::ui::Key, node: Node) -> Handle {
        let mut id_label = [0u8; 17];
        self.node_ref(key.label(&mut id_label), &node)
    }

    /// A node keyed by a label the caller holds (the C ABI's `pxui_ui_node`, the widgets' own labels); it is copied once, into the box.
    /// Inline: a key and a call; libpxui's partitioning otherwise keeps it out of line where the static build folds it in.
    #[inline]
    pub fn node_ref(&mut self, label: &str, node: &Node) -> Handle {
        self.require_building();
        let key = key_of(self.current_seed(), label);
        self.node_keyed_ref(key, node)
    }

    pub fn box_keyed(&mut self, key: i64, opts: BoxOpts) -> UiBox {
        let BoxOpts { flags, w, h, max_h, axis, padding, at, scroll_step, hit, hit_rect } = opts;
        let b = self.node_keyed(key, Node { flags, w, h, max_h, axis, padding: Edges::all(padding), at: at.into(), scroll_step, ..Node::DEFAULT });
        self.boxes[b.index].hit = hit;
        self.boxes[b.index].hit_rect = hit_rect;
        b
    }

    /// Create a box under the current parent. `at` positions it outside the flow at an offset from
    /// the parent's origin. `max_h` caps a `Fit` height; overflowing `scroll` boxes reserve a
    /// 10-point scrollbar gutter. `hit` maps the laid-out rectangle to the rectangle that receives
    /// input (the default is the whole box).
    pub fn box_(&mut self, label: &str, opts: BoxOpts) -> UiBox {
        self.require_building();
        let key = key_of(self.current_seed(), label);
        self.box_keyed(key, opts)
    }

    /// `box_` of a label formatted into the frame arena: keys hash the text, none keeps it.
    pub fn box_fmt(&mut self, label: std::fmt::Arguments, opts: BoxOpts) -> UiBox {
        match label.as_str() {
            Some(text) => self.box_(text, opts),
            None => {
                self.require_building();
                let range = self.arena.fmt(label);
                let key = key_of(self.current_seed(), self.arena.str(range));
                self.box_keyed(key, opts)
            }
        }
    }

    pub fn set_at(&mut self, b: &UiBox, at: (f32, f32)) {
        self.require_building();
        self.slots[b.box_slot].node.at = Some(at).into();
    }

    /// Give a node declared this frame the look `look`, picked for `state` (what `node` takes, set once
    /// its signal is known).
    pub fn set_look(&mut self, b: &Handle, mut look: Look, state: State) {
        if self.live(b) {
            if names_token(&look) {
                resolve_with(&mut look, self.palette(), self.theme().tokens.accent);
            }
            let s = &mut self.slots[b.box_slot];
            // a node's own transition (declared, or the theme's an earlier call this frame picked) stays: paint runs it on a change of
            // state
            if s.node.transition.is_some() {
                if s.touched <= s.painted { s.look_needs_snapshot = true; }
                (s.node.look, s.node.state) = (look, state);
                return;
            }
            let s = &self.slots[b.box_slot];
            // a node shown last frame in another state moves to this look over the theme's transition
            let transition = if self.motion && s.seen_state != state && self.range_valid(b.box_slot) {
                Some(self.theme().tokens.transition(s.seen_state, state)).filter(|c| !c.instant()).and_then(Transition::of)
            } else {
                None
            };
            let s = &mut self.slots[b.box_slot];
            if s.touched <= s.painted { s.look_needs_snapshot = true; }
            (s.node.look, s.node.state, s.node.transition) = (look, state, transition.into());
        }
    }

    /// Gives a node declared this frame its own transition, as `Node::transition` declares it (`set_node_layout` carries one).
    pub fn declare_transition(&mut self, b: &Handle, transition: Transition) {
        if self.live(b) {
            let s = &mut self.slots[b.box_slot];
            s.node.transition = Some(transition).into();
        }
    }

    /// The record of a node declared this frame, to finish declaring it once its signal is known.
    pub fn node_mut(&mut self, b: &Handle) -> &mut Node {
        assert!(self.live(b), "Ui.node_mut: a stale handle");
        let s = &mut self.slots[b.box_slot];
        if s.touched <= s.painted { s.look_needs_snapshot = true; }
        &mut s.node
    }

    /// `node_mut` of a handle this frame's build has just made, as a default body holds it: no liveness check.
    pub(crate) fn node_new(&mut self, b: &Handle) -> &mut Node {
        let s = &mut self.slots[b.box_slot];
        debug_assert!(s.touched > s.painted);
        &mut s.node
    }

    /// `set_look` with the look `states` picks for `state`.
    pub fn style(&mut self, b: &Handle, states: &States, state: State) {
        self.set_look(b, *states.pick(state), state);
    }

    /// What the pointer and keyboard do to the node now: hovered, pressed (it holds the capture),
    /// focused. A widget adds `checked` and `disabled`.
    pub fn node_state(&self, b: &Handle) -> State {
        self.key_state(if self.live(b) { b.box_key } else { 0 })
    }

    /// `node_state` of the node keyed `key` as the last frame left it: a kit part's state before its body builds it.
    pub(crate) fn key_state(&self, key: i64) -> State {
        State { hovered: key != 0 && self.hot == key && !self.touches.using_touch, pressed: key != 0 && self.active == Some(key), focused: key != 0 && self.focus == key, ..State::default() }
    }

    /// The handle names a slot this `Ui` has: the most a call that only indexes the slot (scroll, state, text) needs of it.
    pub fn has_slot(&self, b: &Handle) -> bool {
        b.box_slot < self.slots.len()
    }

    /// The handle still names the node it was made for (that node has not gone since).
    pub fn live(&self, b: &Handle) -> bool {
        self.slots.get(b.box_slot).is_some_and(|s| s.generation == b.generation && s.key == b.box_key && s.key != 0)
    }

    /// The handle is this frame's build record `index` holds, by its key: where `within` attaches children (its slot is not read), so a
    /// handle of an earlier frame, a forged index or a record another node holds now is refused. One bounds check and one key compare.
    #[inline]
    pub fn is_build_record(&self, b: &Handle) -> bool {
        b.index < self.count && self.boxes.get(b.index).is_some_and(|record| record.key == b.box_key)
    }

    /// The handle points at this frame's build record as well as a live retained slot.
    pub fn is_current_build_box(&self, b: &Handle) -> bool {
        self.building && self.live(b) && self.boxes.get(b.index).is_some_and(|record| record.key == b.box_key && record.slot == b.box_slot)
    }

    /// Set the node's text (painted in its look's text colour and font).
    pub fn text(&mut self, b: &Handle, text: &str) {
        if self.live(b) {
            self.write_text(b.box_slot, text);
            self.clear_rich(b.box_slot);
        }
    }

    fn clear_rich(&mut self, slot: usize) {
        if let Some(rich) = self.slots[slot].rich.take() { self.span_live -= rich.spans.len as usize; }
    }

    fn write_text(&mut self, slot: usize, text: &str) {
        self.reuse_empty_string(slot, text.len(), StringField::Text);
        let stored = &mut self.slots[slot].text;
        if stored != text {
            stored.clear();
            stored.push_str(text);
        }
    }

    /// Explicit styled text on a node; its accessible name remains the original text.
    pub fn text_spans(&mut self,b:&Handle,text:&str,style:&Style,spans:&[TextSpan])->bool {
        self.write_styled_text(b, text, style, spans, false)
    }

    fn write_styled_text(&mut self, b: &Handle, text: &str, style: &Style, spans: &[TextSpan], line_box: bool) -> bool {
        if !self.live(b) || !pxui_text::valid_spans(text,spans) { return false; }
        let slot = b.box_slot;
        self.store_styled_text(slot, text, style, spans, line_box);
        true
    }

    fn store_styled_text(&mut self, slot: usize, text: &str, style: &Style, spans: &[TextSpan], line_box: bool) {
        self.write_text(slot, text);
        let spans = self.store_spans(slot, spans);
        self.slots[slot].rich = Some(NodeText { style: *style, spans, line_box });
    }

    /// Puts `spans` in the arena as `slot`'s: over its old range when the count is the same, else at the end (the old range is dead
    /// space). A full arena that is at least half dead is compacted first, so the arena grows only while it is mostly live.
    fn store_spans(&mut self, slot: usize, spans: &[TextSpan]) -> SpanRange {
        let old = self.slots[slot].rich.as_mut().map_or_else(SpanRange::default, |rich| core::mem::take(&mut rich.spans));
        if old.len as usize == spans.len() {
            self.span_arena[old.start as usize..][..spans.len()].copy_from_slice(spans);
            return old;
        }
        self.span_live = self.span_live - old.len as usize + spans.len();
        if spans.is_empty() { return SpanRange::default(); }
        let len = self.span_arena.len();
        if len + spans.len() > self.span_arena.capacity() && len - (self.span_live - spans.len()) >= len / 2 { self.compact_spans(); }
        let start = self.span_arena.len();
        self.span_arena.extend_from_slice(spans);
        // the spare keeps room for every span the arena can hold, so a compaction never allocates
        self.span_spare.reserve(self.span_arena.capacity());
        SpanRange { start: start as u32, len: spans.len() as u32 }
    }

    /// Packs the live ranges to the front of a fresh arena (the spare, swapped in).
    fn compact_spans(&mut self) {
        let mut packed = core::mem::take(&mut self.span_spare);
        packed.clear();
        for slot in &mut self.slots {
            if let Some(rich) = slot.rich.as_mut().filter(|rich| rich.spans.len != 0) {
                let at = packed.len();
                packed.extend_from_slice(&self.span_arena[rich.spans.start as usize..][..rich.spans.len as usize]);
                rich.spans.start = at as u32;
            }
        }
        self.span_spare = core::mem::replace(&mut self.span_arena, packed);
        self.span_spare.clear();
    }

    /// Explicit CSS line boxes: primary-face rounded metrics set the first baseline.
    pub fn text_line_box(&mut self,b:&Handle,text:&str,style:&Style,spans:&[TextSpan])->bool {
        if !style.line_height.is_finite() || style.line_height<=0.0 || self.text.as_ref().and_then(|t|t.font_metrics(style.font)).is_none() { return false; }
        self.write_styled_text(b, text, style, spans, true)
    }

    /// `text_line_box` with no spans, of `text` in capitals when `caps` (through the reused label buffer: no allocation once grown).
    pub(crate) fn text_line_box_caps(&mut self, b: &Handle, text: &str, style: &Style, caps: bool) -> bool {
        if !caps { return self.text_line_box(b, text, style, &[]); }
        let mut buffer = core::mem::take(&mut self.label);
        buffer.clear();
        buffer.extend(text.chars().map(|c| c.to_ascii_uppercase()));
        let done = self.text_line_box(b, &buffer, style, &[]);
        self.label = buffer;
        done
    }

    /// Start building children of `b`; give the result to `leave`.
    pub fn enter(&mut self, b: &UiBox) -> Saved {
        self.require_building();
        let saved = Saved::Depths(self.parents.len(), self.seeds.len());
        self.parents.push(b.index);
        self.seeds.push(b.box_key);
        saved
    }

    /// The node being built into (the innermost one entered): a table cell's, for the cell's builder.
    pub fn entered(&self) -> Handle {
        let index = self.parents.last().copied().unwrap_or(0);
        let slot = self.boxes[index].slot;
        UiBox { index, box_key: self.current_seed(), box_slot: slot, generation: self.slots[slot].generation, ui: self.ui_serial }
    }

    pub fn leave(&mut self, saved: Saved) {
        let Saved::Depths(parents, seeds) = saved;
        self.parents.truncate(parents);
        self.seeds.truncate(seeds);
    }

    /// Build children of `b`.
    pub fn within(&mut self, b: &UiBox, f: &mut dyn FnMut(&mut Ui)) {
        let saved = self.enter(b);
        f(self);
        self.leave(saved);
    }

    /// Mix a label into the keys of boxes built inside, without a box.
    pub fn scope(&mut self, label: &str, f: &mut dyn FnMut(&mut Ui)) {
        self.require_building();
        let saved = Saved::Depths(self.parents.len(), self.seeds.len());
        let seed = key_of(self.current_seed(), label);
        self.seeds.push(seed);
        f(self);
        self.leave(saved);
    }

    /// Build at the root under the root's own seed, wherever the caller is in the tree: what is
    /// built is laid out in the window, clipped by no pane.
    pub(crate) fn enter_root(&mut self) -> Saved {
        self.escape();
        let saved = Saved::Depths(self.parents.len(), self.seeds.len());
        self.parents.push(0);
        self.seeds.push(ROOT_KEY);
        saved
    }

    pub fn box_of_key(&self, key: i64) -> Option<UiBox> {
        self.table.find(key).map(|slot| UiBox { index: NONE, box_key: key, box_slot: slot, generation: self.slots[slot].generation, ui: self.ui_serial })
    }

    /// Resolve a published accessibility identity without exposing its slot encoding to hosts.
    pub fn box_of_a11y(&self, id: u64) -> Option<UiBox> {
        let (slot, generation) = ((id & 0xffff_ffff) as usize, (id >> 32) as u32);
        let s = self.slots.get(slot).filter(|s| s.generation == generation && s.key != 0)?;
        Some(UiBox { index: NONE, box_key: s.key, box_slot: slot, generation, ui: self.ui_serial })
    }

    /// The box's screen rectangle from the previous frame (empty for a stale handle).
    /// The build read slot `slot`'s size as `w` x `h` (NaN: that side not read) before layout: if
    /// layout lays it out otherwise, `Ui::frame` builds again.
    pub(crate) fn expect(&self, slot: usize, w: f32, h: f32) {
        self.record_size(ExpectedSize { slot, w, h, grid: None });
    }

    fn record_size(&self, size: ExpectedSize) {
        let mut e = self.expected.take();
        e.push(size);
        self.expected.set(e);
    }

    pub fn rect(&self, b: &UiBox) -> Rect {
        if !self.live(b) {
            return Rect::new(0.0, 0.0, 0.0, 0.0);
        }
        let s = &self.slots[b.box_slot];
        let v = &self.views[self.views[s.owner as usize].xf as usize];
        v.cover(Rect::new(s.rx + v.shift.0, s.ry + v.shift.1, s.rw, s.rh))
    }

    /// What input did to the node this frame; a stale handle has no signal.
    pub fn signal_owned(&self, b: &UiBox) -> Signal {
        let mut signal = self.signal(b);
        signal.keys.extend(self.key_events_ref(b).iter().map(|(event, _)| event.clone()));
        signal
    }

    /// Signal scalars without cloning the owned key/text events.
    pub fn signal(&self, b: &UiBox) -> Signal {
        let key = if self.live(b) { b.box_key } else { 0 };
        let held = key != 0 && self.active == Some(key);
        let slot = &self.slots[b.box_slot];
        match self.signals.get(&key).filter(|_| key != 0) {
            None => Signal {
                hovered: key != 0 && self.hot == key && !self.touches.using_touch,
                pressed: false,
                subtree_press: None,
                held,
                released: false,
                clicked: false,
                double_clicked: false,
                clicks: 0,
                dragging: false,
                drag: (0.0, 0.0),
                pointer: self.pointer,
                press_point: if held { self.active_press } else { (slot.press_x, slot.press_y) },
                release_point: self.pointer,
                button: if held { Some(self.active_button) } else { None },
                scroll: (0.0, 0.0),
                pinch: 1.0,
                keys: Vec::new(),
                a11y_value: None,
                a11y_select: None,
            },
            Some(value) => Signal {
                hovered: self.hot == key && !self.touches.using_touch,
                pressed: value.pressed,
                subtree_press: value.subtree_press,
                held,
                released: value.released,
                clicked: value.clicked,
                double_clicked: value.double_clicked,
                clicks: value.clicks,
                dragging: value.moved && (held || value.released),
                drag: (value.drag_x, value.drag_y),
                pointer: self.pointer,
                press_point: if value.pressed || value.released {
                    value.press_point
                } else if held {
                    self.active_press
                } else {
                    (slot.press_x, slot.press_y)
                },
                release_point: value.release_point,
                button: match value.button {
                    Some(b) => Some(b),
                    None => if held { Some(self.active_button) } else { None },
                },
                scroll: (value.scroll_x, value.scroll_y_steps),
                pinch: value.pinch,
                keys: Vec::new(),
                a11y_value: value.a11y_value,
                a11y_select: value.a11y_select,
            },
        }
    }

    pub(crate) fn cancel_press(&mut self, b: &UiBox) {
        if self.active == Some(b.box_key) {
            self.cancelled.insert(0, self.active_button);
            self.active = None;
        }
    }

    /// Key/text events of the focused box with the modifiers held at each.
    pub fn key_events(&self, b: &UiBox) -> Vec<(Event, Vec<Key>)> {
        self.key_events_ref(b).to_vec()
    }

    pub fn key_events_ref(&self, b: &UiBox) -> &[(Event, Vec<Key>)] {
        let key = if self.live(b) { b.box_key } else { 0 };
        self.signals.get(&key).filter(|_| key != 0).map_or(&[], |value| &value.keys)
    }

    /// The key events routed to the node keyed `key` this frame (the focused one), read without its handle.
    pub(crate) fn key_events_of(&self, key: i64) -> &[(Event, Vec<Key>)] {
        self.signals.get(&key).filter(|_| key != 0).map_or(&[], |value| &value.keys)
    }

    pub(crate) fn with_key_events(&mut self, b: &UiBox, f: &mut dyn FnMut(&mut Ui, &[(Event, Vec<Key>)])) {
        let key = if self.live(b) { b.box_key } else { 0 };
        let Some(signal) = self.signals.get_mut(&key).filter(|_| key != 0) else { f(self, &[]); return; };
        let keys = std::mem::take(&mut signal.keys);
        f(self, &keys);
        if let Some(signal) = self.signals.get_mut(&key) { signal.keys = keys; }
    }

    /// Held keys at this box's captured press, retained through release.
    pub fn press_keys(&self, b: &UiBox) -> Vec<Key> {
        match self.signals.get(&b.box_key) {
            None => if self.active == Some(b.box_key) { self.active_keys.clone() } else { Vec::new() },
            Some(value) => value.press_keys.clone(),
        }
    }

    /// Whether Shift and the platform's command modifier were held at this box's captured press (no copy of the keys).
    pub(crate) fn press_mods(&self, b: &UiBox) -> (bool, bool) {
        let keys: &[Key] = match self.signals.get(&b.box_key) {
            Some(value) => &value.press_keys,
            None if self.active == Some(b.box_key) => &self.active_keys,
            None => &[],
        };
        (keys.contains(&Key::Shift), self.command_modifiers(keys))
    }

    pub(crate) fn press_shift(&self, b: &UiBox) -> bool {
        match self.signals.get(&b.box_key) {
            Some(value) => value.press_keys.contains(&Key::Shift),
            None => self.active == Some(b.box_key) && self.active_keys.contains(&Key::Shift),
        }
    }

    /// The released right-click in this box or a descendant, through shared capture.
    pub fn context_at(&mut self, b: &UiBox) -> Option<(f32, f32)> {
        let targets: Vec<(i64, (f32, f32))> = self
            .signals
            .iter()
            .filter(|(_, v)| {
                v.released && v.button == Some(MouseButton::RightButton) && {
                    let (px, py) = v.press_point;
                    let (rx, ry) = v.release_point;
                    (px - rx).hypot(py - ry) < 4.0
                }
            })
            .map(|(k, v)| (*k, v.release_point))
            .collect();
        for (target, point) in targets {
            if self.hit_within(target, b.box_key) {
                return Some(point);
            }
        }
        None
    }

    pub fn focused(&self, b: &UiBox) -> bool {
        self.focus == b.box_key
    }

    pub fn focus_box(&mut self, b: &UiBox) {
        if self.focus != b.box_key {
            self.composition.clear();
            self.edit_focus = 0;
        }
        self.focus = b.box_key;
    }

    /// Names the box a key press takes as its focus while no node holds it (a window's default key target): its nearest
    /// focusable box (itself or an ancestor; a list's scroll box names the list), unless a modal leaves that out. Stated each
    /// build: a build that names none leaves no default. While a widget other than a text editor holds the focus, a press it
    /// leaves untaken falls through to that box.
    pub fn focus_default(&mut self, b: &UiBox) {
        self.default_focus = b.box_key;
    }

    /// The box itself is the topmost under the pointer (none of its children are).
    pub fn hovered_exactly(&self, b: &UiBox) -> bool {
        b.box_key != 0 && self.hot == b.box_key && !self.touches.using_touch
    }

    /// The box or a hit descendant owns hover in the shared hit tree.
    pub fn hovered_within(&mut self, b: &UiBox) -> bool {
        let hot = self.hot;
        self.hit_within(hot, b.box_key)
    }

    // ------------------------------------------------------------- carry

    /// A box holds the pointer capture this frame (a press or drag in progress).
    pub fn captured(&self) -> bool {
        self.active.is_some()
    }

    /// The number of the frame being built (a revision part that rebuilds a view every frame).
    pub fn frame_index(&self) -> u64 {
        self.frame_number as u64
    }

    pub fn carrying(&self) -> Option<Payload> { self.carrying_ref().cloned() }

    pub fn carrying_ref(&self) -> Option<&Payload> { self.payload.as_ref().map(|(p, _)| p) }

    fn retire_payload(&mut self) {
        if let Some((payload, _)) = self.payload.take() { self.spare_payload = Some(payload); }
    }

    fn set_payload(&mut self, kind: &str, value: &str, origin: Origin) {
        self.retire_payload();
        self.carry_fraction = [0.0; 2];
        let mut payload = self.spare_payload.take().unwrap_or_else(|| Payload { kind: String::new(), value: String::new() });
        payload.kind.clear(); payload.kind.push_str(kind);
        payload.value.clear(); payload.value.push_str(value);
        self.payload = Some((payload, origin));
    }

    /// Hold a payload. With `from`, the box that owns the pressed left button: it is held once the
    /// pointer has moved 4 points from the press, so a widget calls it every frame while pressed.
    /// Without `from` (a key) it is held at once, with no capture.
    pub fn carry(&mut self, from: Option<&UiBox>, kind: &str, value: &str) {
        match from {
            None => {
                if let Some((held, Origin::FromKey)) = &self.payload {
                    if held.kind == kind && held.value == value {
                        return;
                    }
                }
                self.set_payload(kind, value, Origin::FromKey);
            }
            Some(b) => {
                if self.payload.is_none()
                    && self.active == Some(b.box_key)
                    && self.carry_latch != self.active
                    && self.active_button == MouseButton::LeftButton
                {
                    let (px, py) = self.active_press;
                    let (x, y) = self.pointer;
                    if (x - px).hypot(y - py) >= DRAG_THRESHOLD {
                        self.set_payload(kind, value, Origin::FromPointer(b.box_key));
                    }
                }
            }
        }
    }

    /// Move pointer capture to a live hit ancestor so virtualized source rows can disappear.
    /// A scroll container follows the pointer at its edges; key carries and unrelated/stale targets
    /// leave the current capture untouched.
    pub fn transfer_carry(&mut self, to: &UiBox) -> bool {
        if !self.live(to) { return false; }
        let Some((_, Origin::FromPointer(owner))) = &self.payload else { return false; };
        let owner = *owner;
        if self.active != Some(owner) || !self.hit_within(owner, to.box_key) { return false; }
        if owner != to.box_key { self.carry_fraction = [0.0; 2]; }
        self.active = Some(to.box_key);
        if let Some((_, origin)) = &mut self.payload { *origin = Origin::FromPointer(to.box_key); }
        true
    }

    /// Forget the payload: the host cancelled (Escape) or finished a carry it started.
    /// Retires the carried payload; the frame's Escape press that cancels it is spent (`key_pressed` no longer reports it).
    pub fn cancel_carry(&mut self) {
        if self.payload.is_some() {
            self.carry_latch = self.active;
            self.take_key(Key::Escape);
        }
        self.retire_payload();
    }

    pub fn drop_target(&mut self, b: &UiBox) -> Option<Drop> {
        self.drop_target_ref(b).map(|(dropped, payload)| if dropped { Drop::Dropped(payload.clone()) } else { Drop::Hover(payload.clone()) })
    }

    /// Whether a payload is carried or was dropped this frame: without one no box is a drop target (`drop_target_ref` answers `None`
    /// before it looks at the box).
    #[inline]
    pub fn drop_pending(&self) -> bool { self.dropped.is_some() || self.payload.is_some() }

    /// A drop remains readable by the target and its ancestors throughout this frame.
    /// Asking makes `b` a drop zone for this frame's hit tree, so a plain node (no input flags) takes the drop too.
    pub fn drop_target_ref(&mut self, b: &UiBox) -> Option<(bool, &Payload)> {
        if !self.drop_pending() { return None; }
        if self.live(b) { (self.slots[b.box_slot].drop_zone, self.drop_zones) = (self.frame_number, self.frame_number); }
        let hot = self.dropped.as_ref().map_or(self.hot, |(_, hot)| *hot);
        if hot == 0 || !self.hit_within(hot, b.box_key) { return None; }
        if let Some((payload, _)) = &self.dropped { Some((true, payload)) }
        else { self.carrying_ref().map(|payload| (false, payload)) }
    }

    pub fn scroll_offset(&self, b: &UiBox) -> f32 {
        self.scroll_y[b.box_slot]
    }

    /// Current painted scroll position, including elastic edge movement.
    pub fn scroll_position(&self, b: &UiBox) -> f32 {
        self.scroll_y[b.box_slot] + self.glides[0][b.box_slot].visual as f32
    }

    /// The horizontal scroll of a `SCROLL_X` box.
    pub fn scroll_offset_x(&self, b: &UiBox) -> f32 {
        self.scroll_xs[b.box_slot]
    }

    pub fn set_scroll_offset_x(&mut self, b: &UiBox, value: f32) {
        self.scroll_xs[b.box_slot] = value;
        self.glides[1][b.box_slot] = Glide::REST;
        if self.active == Some(b.box_key) { self.carry_fraction[1] = 0.0; }
    }

    pub fn set_scroll_offset(&mut self, b: &UiBox, value: f32) {
        self.scroll_y[b.box_slot] = value;
        self.glides[0][b.box_slot] = Glide::REST;
        if self.active == Some(b.box_key) { self.carry_fraction[0] = 0.0; }
    }

    /// Retained per-box scalar state for custom widgets.
    /// The value `set_state` stored in `b`'s slot, else `default`; a gone node's handle (its slot reused or freed) reads `default`.
    pub fn state(&self, b: &UiBox, default: i64) -> i64 {
        if !self.live(b) { return default; }
        self.slots[b.box_slot].state_value.unwrap_or(default)
    }

    /// Stores `value` in `b`'s slot while the node lives; a gone node's handle stores nothing (it never reaches the node that took
    /// its slot).
    pub fn set_state(&mut self, b: &UiBox, value: i64) {
        if self.live(b) { self.slots[b.box_slot].state_value = Some(value); }
    }

    /// Stable paint text follows its existing retained box key and is discarded with that slot.
    pub fn cached_label(&mut self, b: &UiBox, index: usize, text: &str) -> Rc<str> {
        let slot = &mut self.slots[b.box_slot];
        slot.label_count = slot.label_count.max(index + 1);
        if index >= slot.labels.len() {
            slot.labels.resize_with(index + 1, || Rc::from(""));
        }
        if slot.labels[index].as_ref() != text { slot.labels[index] = Rc::from(text); }
        slot.labels[index].clone()
    }

    /// `row` takes presses where `part` of its widget sat last frame, relative to the row's own rectangle: the whole row until the
    /// part has been laid out. No closure.
    pub(crate) fn hit_part(&mut self, row: &UiBox, part: Part) {
        let Some(p) = self.part_box(row.box_key, part) else { return };
        let (a, b) = (&self.slots[row.box_slot], &self.slots[p.box_slot]);
        if a.rw > 0.0 && b.rw > 0.0 && b.rh > 0.0 {
            self.boxes[row.index].shape = HitShape::Part(Rect::new(b.rx - a.rx, b.ry - a.ry, b.rw, b.rh));
        }
    }

    /// The box `part` of the widget keyed `owner` has in this frame's build, if the body built it (a canvas needs the build record,
    /// which `part_box` does not give). It was built just now, so the search from the end of the build is short.
    pub(crate) fn part_built(&self, owner: i64, part: Part) -> Option<UiBox> {
        let key = crate::ui_kit::part_key(owner, part);
        let index = self.boxes[..self.count].iter().rposition(|b| b.key == key)?;
        let slot = self.boxes[index].slot;
        Some(UiBox { index, box_key: key, box_slot: slot, generation: self.slots[slot].generation, ui: self.ui_serial })
    }

    /// `b` takes presses on its own rectangle grown by `outset` on each side (a negative side shrinks it), from this frame's
    /// layout on: a band wider than a thin line, with no part node and no closure. A body declares it on the box it was given.
    pub fn hit_outset(&mut self, b: &UiBox, outset: Edges) {
        self.boxes[b.index].shape = HitShape::Outset(outset);
    }

    pub fn text_state(&self, b: &UiBox) -> Option<String> {
        self.slots[b.box_slot].text_value.clone()
    }

    pub fn set_text_state(&mut self, b: &UiBox, value: Option<String>) {
        self.slots[b.box_slot].text_value = value;
    }

    /// Free-form drawing with the node's final laid-out rectangle, after its look and before its
    /// children. The closure lives in the frame arena: it runs once after layout and is dropped when
    /// painting ends.
    pub fn canvas<F: Fn(&mut Paint, Rect) + 'static>(&mut self, b: &UiBox, f: F) {
        assert!(b.index < self.count && self.boxes[b.index].slot == b.box_slot, "Ui.canvas: the node was not built this frame");
        self.views[self.slots[b.box_slot].owner as usize].palette_labels = false;
        self.arena.add(&mut self.boxes[b.index].paint, f);
    }

    /// A canvas that draws after the node's children, outside its content clip.
    pub fn canvas_over<F: Fn(&mut Paint, Rect) + 'static>(&mut self, b: &UiBox, f: F) {
        assert!(b.index < self.count && self.boxes[b.index].slot == b.box_slot, "Ui.canvas_over: the node was not built this frame");
        self.views[self.slots[b.box_slot].owner as usize].palette_labels = false;
        self.arena.add(&mut self.boxes[b.index].over, f);
    }

    /// `canvas` under its earlier name.
    pub fn draw<F: Fn(&mut Paint, Rect) + 'static>(&mut self, b: &UiBox, f: F) {
        self.canvas(b, f);
    }

    /// `canvas_over` under its earlier name.
    pub fn draw_over<F: Fn(&mut Paint, Rect) + 'static>(&mut self, b: &UiBox, f: F) {
        self.canvas_over(b, f);
    }

    /// Raise a root box and its children above ordinary boxes, below modal popups. Higher `order`
    /// values paint later; equal values retain their order of calls.
    pub fn to_front(&mut self, order: i64, b: &UiBox) {
        self.require_building();
        if self.boxes[b.index].parent != 0 {
            panic!("Ui.to_front: expected a root box");
        }
        self.escape();
        self.foreground.push((order, b.index, LayerRole::Base));
    }



    pub(crate) fn set_box_h(&mut self, b: &UiBox, h: Size) {
        self.slots[b.box_slot].node.h = h;
    }

    /// The last child built under the current parent (`NONE` when it has none yet).
    pub(crate) fn last_child_of_parent(&self) -> usize {
        self.boxes[self.current_parent()].last
    }

    pub(crate) fn box_w_px(&self, index: usize) -> Option<f32> {
        match self.nd(index).w {
            Size::Px(w) => Some(w),
            _ => None,
        }
    }

    pub(crate) fn node_at(&self, index: usize) -> &Node {
        self.nd(index)
    }

    /// The font of the look a live box was last declared with (a body's part as last laid out).
    pub(crate) fn kept_font(&self, b: &UiBox) -> pxui_types::theme::Font {
        self.slots[b.box_slot].node.look.font
    }

    pub(crate) fn box_parent(&self, index: usize) -> usize {
        self.boxes[index].parent
    }

    pub(crate) fn box_count(&self) -> usize {
        self.count
    }

    pub(crate) fn push_overlay(&mut self, index: usize, role: LayerRole) {
        self.escape();
        self.overlays.push((index, role));
    }


    pub(crate) fn slot_rect(&self, slot: usize) -> Rect {
        let s = &self.slots[slot];
        Rect::new(s.rx, s.ry, s.rw, s.rh)
    }

    pub(crate) fn slot_for(&self, key: i64) -> Option<usize> {
        self.table.find(key)
    }

    pub(crate) fn slot_state(&self, slot: usize) -> Option<i64> {
        self.slots[slot].state_value
    }

    // ------------------------------------------------------------- layout

    fn flow(&self, index: usize) -> bool {
        let n = self.nd(index);
        n.at.is_none() && n.anchor.is_none()
    }

    /// The first flow child at or after `child` in its parent's list.
    fn next_flow(&self, mut child: usize) -> usize {
        while child != NONE && !self.flow(child) {
            child = self.boxes[child].next;
        }
        child
    }

    /// Bottom-up: fixed and fit sizes. Children follow their parent in build order, so a reverse
    /// sweep sees every child first.
    fn intrinsic(&mut self) {
        for index in (0..self.count).rev() {
            if self.nd(index).axis == Axis::Stack {
                self.order_stack(index);
            }
            let n = &self.slots[self.boxes[index].slot].node;
            if self.boxes[index].first == NONE && self.boxes[index].view == NO_VIEW && n.grid.is_none()
                && let (Size::Px(w), Size::Px(h)) = (n.w, n.h)
            {
                self.l_content[index] = (0.0 + n.padding.t) + n.padding.b;
                self.l_content_w[index] = (0.0 + n.padding.l) + n.padding.r;
                self.l_w[index] = clamp(w, n.min_w, n.max_w);
                self.l_h[index] = clamp(h, n.min_h, n.max_h);
                continue;
            }
            let (content_w, content_h) = match n.grid.get() {
                Some(grid) => {
                    let (cols, nc, rows) = self.grid_tracks(index, (n.gap, n.row_gap), &grid, None);
                    let extent = (span(&cols[..nc], n.gap), span(&rows, n.row_gap));
                    self.grid_rows.set(rows);
                    extent
                }
                None => {
                    // a wrapping box's lines need its main size: declared, or for a size only layout
                    // resolves (Grow, Pct, Rel) the one it was laid out at last (`rewrap` settles the
                    // frame when that changed)
                    let slot = self.boxes[index].slot;
                    let last = (self.slots[slot].rw, self.slots[slot].rh);
                    let limit = match (n.wrap, n.axis == Axis::Row, n.w, n.h) {
                        (true, true, Size::Px(w), _) => w - n.padding.l - n.padding.r,
                        (true, false, _, Size::Px(h)) => h - n.padding.t - n.padding.b,
                        (true, true, Size::Grow | Size::Pct(_) | Size::Rel(_), _) if last.0 > 0.0 => last.0 - n.padding.l - n.padding.r,
                        (true, false, _, Size::Grow | Size::Pct(_) | Size::Rel(_)) if last.1 > 0.0 => last.1 - n.padding.t - n.padding.b,
                        _ => f32::INFINITY,
                    };
                    self.flow_extent(index, n, limit)
                }
            };
            let pad = n.padding;
            let (mut cw, mut ch) = (content_w + pad.l + pad.r, content_h + pad.t + pad.b);
            let slot=self.boxes[index].slot;
            // measured without its text (`BoxRec::ahead`): the view holding it builds again rather than keep the size
            let mut unmeasured=false;
            let rich=self.slots[slot].rich.as_ref();
            let line_box=rich.is_some_and(|r|r.line_box);
            let pixel=n.flags.any(Flags::FIT_TEXT_PIXEL);
            // a Fit width is the text's: any text, not only pixel-fitted or line-box text (else it collapsed to 0)
            let has_text=rich.is_some() || !self.slots[slot].text.is_empty();
            if (line_box && (matches!(n.w,Size::Fit) || matches!(n.h,Size::Fit))) || ((pixel || has_text) && matches!(n.w,Size::Fit)) {
                let style=rich.map_or_else(||plain_style(&n.look,n.look.font.points(self.font_points),n.flags),|r|r.style(n,self.dir==Direction::Rtl));
                let inset=n.inset;
                let stored=&self.slots[slot];
                let value=if rich.is_none() && n.look.font.caps {
                    self.label.clear(); self.label.extend(stored.text.chars().map(|c|c.to_ascii_uppercase()));
                    self.label.as_str()
                } else {stored.text.as_str()};
                let text=self.text.as_mut().expect("Ui: the text engine is lent out");
                let spans=rich.map_or(&[][..], |r|r.spans(&self.span_arena));
                let ahead=self.boxes[index].ahead;
                let deferred=text.queue_counts().0;
                let mut natural=if matches!(n.w,Size::Fit) {
                    lay_out_node(text,&stored.text_layout,n.flags,&style,0.0,value,spans,ahead).unwrap().1.w+inset.l+inset.r
                } else {0.0};
                if pixel { natural=(natural*self.density as f32).ceil()/self.density as f32; }
                if matches!(n.w,Size::Fit) {cw=cw.max(natural);}
                if line_box && matches!(n.h,Size::Fit) {
                    let width=match n.w {
                        Size::Px(w)=>clamp(w,n.min_w,n.max_w),
                        _ if stored.rw>0.0=>stored.rw,
                        Size::Fit=>clamp(natural,n.min_w,n.max_w),
                        _=>self.view_w,
                    };
                    // an ahead row's first layout guesses the width (the window's): nothing to prepare until it is placed
                    let guess=ahead && text.queue_running() && !matches!(n.w,Size::Px(_)|Size::Fit) && stored.rw<=0.0;
                    let m=if guess {Metrics::default()} else {lay_out_node(text,&stored.text_layout,n.flags,&style,(width-inset.l-inset.r).max(0.0),value,spans,ahead).unwrap().1};
                    ch=ch.max(style.line_height*m.lines.max(1) as f32+inset.t+inset.b);
                    unmeasured=guess || text.queue_counts().0!=deferred;
                    self.expect(slot,width,f32::NAN);
                } else {
                    unmeasured=text.queue_counts().0!=deferred;
                }
            }
            // a view's box: kept, its content is the size its last build measured (its children are
            // not declared); built, that measure is what it keeps
            let v = self.boxes[index].view;
            let kept = v != NO_VIEW && self.boxes[index].kept;
            if kept {
                (cw, ch) = self.views[v as usize].fit;
            }
            let resolve = |kind: &Size, content: f32| match kind {
                Size::Px(value) => *value,
                Size::Fit => content,
                _ => 0.0,
            };
            let (mut w, mut h) = (resolve(&n.w, cw), resolve(&n.h, ch));
            if n.aspect > 0.0 {
                match (n.w, n.h) {
                    (Size::Px(_), Size::Fit) => h = w / n.aspect,
                    (Size::Fit, Size::Px(_)) => w = h * n.aspect,
                    _ => {}
                }
            }
            let (w, h) = (clamp(w, n.min_w, n.max_w), clamp(h, n.min_h, n.max_h));
            (self.l_content[index], self.l_content_w[index]) = (ch, cw);
            (self.l_w[index], self.l_h[index]) = (w, h);
            if v != NO_VIEW && !kept {
                self.views[v as usize].fit = (cw, ch);
            }
            if unmeasured {
                self.mark_view(self.slots[slot].owner);
            }
        }
    }

    /// The extent of a row, column or stack's flow children (margins and gaps in, padding out). A
    /// wrapping flow breaks its lines at `limit`, its inner main size: its `Px` one when measuring
    /// (else one line), the resolved one in `rewrap`.
    fn flow_extent(&self, index: usize, n: &Node, limit: f32) -> (f32, f32) {
        let row = n.axis == Axis::Row;
        let stack = n.axis == Axis::Stack;
        let (mut along, mut across, mut line_main, mut line_cross, mut count) = (0.0f32, 0.0f32, 0.0f32, 0.0f32, 0);
        let mut child = self.next_flow(self.boxes[index].first);
        while child != NONE {
            let m = self.nd(child).margin;
            let (ow, oh) = (self.l_w[child] + m.l + m.r, self.l_h[child] + m.t + m.b);
            child = self.next_flow(self.boxes[child].next);
            if stack {
                (along, across) = (along.max(ow), across.max(oh));
                continue;
            }
            let (cm, cc) = if row { (ow, oh) } else { (oh, ow) };
            if count > 0 && line_main + n.gap + cm > limit {
                along = along.max(line_main);
                across += line_cross + n.row_gap;
                (line_main, line_cross, count) = (0.0, 0.0, 0);
            }
            line_main += if count > 0 { n.gap } else { 0.0 } + cm;
            line_cross = line_cross.max(cc);
            count += 1;
        }
        if stack {
            return (along, across);
        }
        along = along.max(line_main);
        across += line_cross;
        if row { (along, across) } else { (across, along) }
    }

    /// A wrapping `Fit` box whose main size its parent resolved (`Grow`, `Pct`, `Rel`, stretch):
    /// its lines measured again at that size give its cross size, before the parent places it.
    /// Called once per such child by its parent's pass, so layout stays O(nodes).
    /// The parent's own Fit size, scroll extent and grid row were measured with the lines at the
    /// size it was laid out at last frame (one line the first time): a different cross size now
    /// settles the frame (`Ui::frame` builds again and measures with this size).
    fn rewrap(&mut self, c: usize) {
        if !self.nd(c).wrap {
            return;
        }
        let n = *self.nd(c);
        let row = n.axis == Axis::Row;
        let (main, cross) = if row { (n.w, n.h) } else { (n.h, n.w) };
        if n.grid.is_some() || n.axis == Axis::Stack || matches!(main, Size::Px(_) | Size::Fit) || !matches!(cross, Size::Fit) {
            return;
        }
        let pad = n.padding;
        let limit = if row { self.l_w[c] - pad.l - pad.r } else { self.l_h[c] - pad.t - pad.b };
        let (w, h) = self.flow_extent(c, &n, limit);
        let measured = if row { self.l_h[c] } else { self.l_w[c] };
        let now = if row { clamp(h + pad.t + pad.b, n.min_h, n.max_h) } else { clamp(w + pad.l + pad.r, n.min_w, n.max_w) };
        if (now - measured).abs() > 0.01 {
            self.settle = true;
        }
        if row {
            self.l_content[c] = h + pad.t + pad.b;
            self.set_main(false, c, self.l_content[c]);
        } else {
            self.l_content_w[c] = w + pad.l + pad.r;
            self.set_main(true, c, self.l_content_w[c]);
        }
    }

    /// A stack's children in `z` order (stable): paint and hit order follow the list. Sorted in a
    /// scratch vector kept between frames (no allocation once warm), O(k log k).
    fn order_stack(&mut self, index: usize) {
        let mut order = std::mem::take(&mut self.z_order);
        order.clear();
        let mut child = self.boxes[index].first;
        while child != NONE {
            order.push((self.nd(child).z, order.len() as u32, child));
            child = self.boxes[child].next;
        }
        if order.iter().any(|o| o.0 != 0) {
            order.sort_unstable();
            for k in 0..order.len() {
                self.boxes[order[k].2].next = order.get(k + 1).map_or(NONE, |o| o.2);
            }
            self.boxes[index].first = order[0].2;
            self.boxes[index].last = order[order.len() - 1].2;
        }
        self.z_order = order;
    }

    /// The grid's column and row sizes: a content-sized track takes its largest single-span child,
    /// then what spanning children lack (`spread`; a fractional track measures as content, `room`
    /// None, but takes no span's share); with `room` (inner w, h) the
    /// fractional tracks share what the others leave.
    fn grid_tracks(&self, index: usize, (gap, row_gap): (f32, f32), g: &Grid, room: Option<(f32, f32)>) -> ([f32; MAX_TRACKS], usize, Vec<f32>) {
        let nc = g.cols().len().clamp(1, MAX_TRACKS);
        // rows: as many as the children reach, in a scratch the caller hands back (`grid_rows`)
        let mut rowc = self.grid_rows.take();
        rowc.clear();
        rowc.resize(g.rows().len(), 0.0);
        let mut colc = [0.0f32; MAX_TRACKS];
        let mut cursor = (0, 0);
        let mut spans = false;
        let mut child = self.next_flow(self.boxes[index].first);
        while child != NONE {
            let cn = self.nd(child);
            let (c, r, cs, rs) = next_cell(cn.cell, &mut cursor, nc);
            if r + rs > rowc.len() {
                rowc.resize(r + rs, 0.0);
            }
            if cs == 1 {
                colc[c] = colc[c].max(self.l_w[child] + cn.margin.l + cn.margin.r);
            }
            if rs == 1 {
                rowc[r] = rowc[r].max(self.l_h[child] + cn.margin.t + cn.margin.b);
            }
            spans |= cs > 1 || rs > 1;
            child = self.next_flow(self.boxes[child].next);
        }
        // a second walk: what a spanning child needs past its tracks goes to their Fit ones
        if spans {
            (cursor, child) = ((0, 0), self.next_flow(self.boxes[index].first));
            while child != NONE {
                let cn = self.nd(child);
                let (c, r, cs, rs) = next_cell(cn.cell, &mut cursor, nc);
                if cs > 1 {
                    spread(g.cols(), &mut colc[..nc], c, cs, self.l_w[child] + cn.margin.l + cn.margin.r, gap);
                }
                if rs > 1 {
                    spread(g.rows(), &mut rowc, r, rs, self.l_h[child] + cn.margin.t + cn.margin.b, row_gap);
                }
                child = self.next_flow(self.boxes[child].next);
            }
        }
        // content sizes become track sizes in place
        let size = |tracks: &[Track], sizes: &mut [f32], room: Option<f32>, gap: f32| {
            let (mut fixed, mut fr) = (0.0, 0.0);
            for i in 0..sizes.len() {
                match (tracks.get(i).copied().unwrap_or(Track::Fit), room) {
                    (Track::Px(v), _) => sizes[i] = v,
                    (Track::Fr(f), Some(_)) => (sizes[i], fr) = (0.0, fr + f),
                    _ => {}
                }
                fixed += sizes[i];
            }
            if let Some(room) = room {
                let free = (room - fixed - gap * sizes.len().saturating_sub(1) as f32).max(0.0);
                for i in 0..sizes.len() {
                    if let Some(Track::Fr(f)) = tracks.get(i) {
                        sizes[i] = if fr > 0.0 { free * f / fr } else { 0.0 };
                    }
                }
            }
        };
        size(g.cols(), &mut colc[..nc], room.map(|r| r.0), gap);
        size(g.rows(), &mut rowc, room.map(|r| r.1), row_gap);
        (colc, nc, rowc)
    }

    /// The offset of `slot`'s scroll on axis `ax` (0 down, 1 sideways).
    fn offset_mut(&mut self, ax: usize, slot: usize) -> &mut f32 {
        if ax == 0 { &mut self.scroll_y[slot] } else { &mut self.scroll_xs[slot] }
    }

    fn pull(&mut self, ax: usize, slot: usize, max_scroll: f64, extent: f64, past: f64, delta: f64) {
        let old = self.glides[ax][slot].raw;
        let next = *self.offset_mut(ax, slot) as f64 + old - delta;
        let bounded = 0.0f64.max(max_scroll.min(next));
        let over = next - bounded;
        let stretch = max_scroll > 0.0 || (ax == 0 && self.gestures[slot].kinds & gesture::REFRESH != 0);
        let raw = if !stretch || over == 0.0 { 0.0 } else { old + (over - old) * past };
        *self.offset_mut(ax, slot) = bounded as f32;
        self.glides[ax][slot].raw = raw;
        self.glides[ax][slot].visual = scroll_stretch(raw, extent);
    }

    /// One scroll box, once its height is laid out (`arrange`): a height that comes from the
    /// parent (Grow, Pct, Rel) stretches and bounces as a fixed one does.
    pub(crate) fn apply_scroll(&mut self, consume: bool, time: f64, index: usize) {
        let key = self.boxes[index].key;
        let (wheel, drag, touch, lift) = self.signals.get(&key).map_or((0.0, 0.0, false, false), |v| (v.scroll_wheel, v.scroll_drag, v.scroll_touch, v.scroll_lift));
        let max_scroll = 0.0f64.max(self.l_content[index] as f64 - self.l_h[index] as f64);
        self.glide(0, time, index, max_scroll, (wheel, drag, touch, lift));
        if consume {
            if let Some(value) = self.signals.get_mut(&key) {
                value.scroll_wheel = 0.0;
                value.scroll_drag = 0.0;
                value.scroll_touch = false;
                value.scroll_lift = false;
            }
        }
    }

    /// A `SCROLL_X` box sideways, as `apply_scroll` down; right to left the offset runs from the
    /// right edge, so the wheel and the fingers move the content mirrored (it follows them).
    pub(crate) fn apply_scroll_x(&mut self, consume: bool, time: f64, index: usize) {
        let key = self.boxes[index].key;
        let (wheel, drag, touch, lift) = self.signals.get(&key).map_or((0.0, 0.0, false, false), |v| (v.scroll_wheel_x, v.scroll_drag_x, v.scroll_touch, v.scroll_lift));
        let max_scroll = 0.0f64.max(self.l_content_w[index] as f64 - self.l_w[index] as f64);
        let sign = if self.dir == Direction::Rtl { -1.0 } else { 1.0 };
        self.glide(1, time, index, max_scroll, (wheel * sign, drag * sign, touch, lift));
        if consume {
            if let Some(value) = self.signals.get_mut(&key) {
                (value.scroll_wheel_x, value.scroll_drag_x, value.scroll_touch, value.scroll_lift) = (0.0, 0.0, false, false);
            }
        }
    }

    /// A captured carry scrolls its container at the visible edges, once per input time.
    fn scroll_carry(&mut self, ax: usize, time: f64, index: usize, max_scroll: f64) {
        let key = self.boxes[index].key;
        if max_scroll <= 0.0 || self.active != Some(key)
            || !matches!(&self.payload, Some((_, Origin::FromPointer(owner))) if *owner == key)
            || !self.hit_within(self.hot, key) { return; }
        let slot = self.boxes[index].slot;
        let s = &self.slots[slot];
        let rect = self.rect(&Handle { index, box_key: key, box_slot: slot, generation: s.generation, ui: self.ui_serial });
        let (point, start, extent) = if ax == 0 { (self.pointer.1, rect.y, rect.h) } else { (self.pointer.0, rect.x, rect.w) };
        if extent <= 0.0 { return; }
        let edge = (extent * 0.25).min(32.0);
        let position = point - start;
        let velocity = if position < edge { -(1.0 - position / edge).clamp(0.0, 1.0) }
            else if position > extent - edge { ((position - extent + edge) / edge).clamp(0.0, 1.0) } else { 0.0 };
        if velocity == 0.0 { return; }
        let dt = (time - self.glides[ax][slot].frame_time).clamp(0.0, 0.05);
        let sign = if ax == 1 && self.dir == Direction::Rtl { -1.0 } else { 1.0 };
        let fraction = self.carry_fraction[ax];
        let offset = self.offset_mut(ax, slot);
        let desired = (*offset as f64 + velocity as f64 * sign * 720.0 * dt + fraction).clamp(0.0, max_scroll);
        *offset = desired as f32;
        // Keep subpixel travel at high frame rates instead of rounding every small step away.
        self.carry_fraction[ax] = desired - *offset as f64;
        self.glides[ax][slot] = Glide { frame_time: time, ..Glide::REST };
    }

    /// The scroll motion of axis `ax`: wheel steps and finger travel pull (stretching past an
    /// edge), fingers lifted in motion coast and bounce, a stretch left alone decays.
    fn glide(&mut self, ax: usize, time: f64, index: usize, max_scroll: f64, (wheel, drag, touch, lift): (f32, f32, bool, bool)) {
        let slot = self.boxes[index].slot;
        // Virtual rows are selected before arrange: a smaller extent must bound the offset now.
        let offset = *self.offset_mut(ax, slot);
        let bounded = 0.0f64.max(max_scroll.min(offset as f64)) as f32;
        if offset != bounded {
            *self.offset_mut(ax, slot) = bounded;
            self.glides[ax][slot] = Glide { frame_time: time, ..Glide::REST };
        }
        let extent = if ax == 0 { self.l_h[index] } else { self.l_w[index] } as f64;
        self.scroll_carry(ax, time, index, max_scroll);
        // at rest with no input: nothing moves (most scroll boxes, most frames)
        if !touch && !lift && wheel == 0.0 && drag == 0.0 && !self.glides[ax][slot].moving() {
            self.glides[ax][slot].frame_time = time;
            return;
        }
        let (velocity, finger) = self.signals.get(&self.boxes[index].key)
            .map_or(((0.0, 0.0), false), |v| (v.scroll_velocity, v.scroll_finger));
        let release = if ax == 0 { velocity.0 } else { velocity.1 };
        let minimum = if finger { SCROLL_FINGER_MIN_VELOCITY } else { SCROLL_MIN_VELOCITY };
        let release = if ax == 1 && self.dir == Direction::Rtl { -release } else { release };
        // any input catches a coast or a bounce where it is
        if touch || wheel != 0.0 || drag != 0.0 {
            self.glides[ax][slot].mode = 0;
        }
        if touch {
            self.glides[ax][slot].event_time = f64::INFINITY;
        }
        if drag != 0.0 {
            self.pull(ax, slot, max_scroll, extent, 1.0, drag as f64);
            // held by the fingers: it springs back when they lift
            self.glides[ax][slot].event_time = f64::INFINITY;
        } else if wheel != 0.0 {
            let step = self.nd(index).scroll_step;
            self.pull(ax, slot, max_scroll, extent, SCROLL_WHEEL_PAST, wheel as f64 * step as f64);
            self.glides[ax][slot].event_time = time;
        }
        if lift {
            if self.glides[ax][slot].visual != 0.0 {
                self.glides[ax][slot].event_time = f64::NEG_INFINITY;
                let g = &self.gestures[slot];
                if ax == 0 && g.kinds & gesture::REFRESH != 0 && !g.refreshing && -self.glides[0][slot].visual >= f64::from(g.threshold_or_default()) {
                    self.gestures[slot].refreshing = true;
                    // read by the next frame: layout runs after this frame's build
                    self.gpulse(slot, gesture::REFRESH_FIRED, self.frame_number + 1);
                }
            } else if max_scroll > 0.0 && release.abs() >= minimum {
                let from = *self.offset_mut(ax, slot) as f64;
                let g = &mut self.glides[ax][slot];
                (g.mode, g.velocity, g.start, g.from) = (if finger { 3 } else { 1 }, release, time, from);
            }
        }
        if wheel == 0.0 && drag == 0.0 {
            let g = self.glides[ax][slot];
            if g.mode == 1 || g.mode == 3 {
                let tau = if g.mode == 3 { SCROLL_FINGER_TAU } else { SCROLL_COAST_TAU };
                // v(t) = v0 e^(-t/tau), so y(t) = y0 + v0 tau (1 - e^(-t/tau))
                let at = |t: f64| g.from + g.velocity * tau * (1.0 - (-t / tau).exp());
                let t = 0.0f64.max(time - g.start);
                let y = at(t);
                if y < 0.0 || y > max_scroll {
                    // the coast is monotonic: bisect for when it met the edge and bounce from then
                    let edge = if y < 0.0 { 0.0 } else { max_scroll };
                    let mut early = 0.0;
                    let mut late = t;
                    for _ in 0..24 {
                        let mid = (early + late) / 2.0;
                        if (at(mid) - edge) * (y - edge) > 0.0 { late = mid } else { early = mid }
                    }
                    let arrival = g.velocity * (-late / tau).exp();
                    *self.offset_mut(ax, slot) = edge as f32;
                    let g = &mut self.glides[ax][slot];
                    g.mode = 2;
                    g.start += late;
                    g.velocity = SCROLL_BOUNCE_LAUNCH * arrival / (SCROLL_BOUNCE_B - SCROLL_BOUNCE_A);
                } else {
                    // on whole device pixels, so text is sharp where the coast ends
                    let density = self.density as f64;
                    *self.offset_mut(ax, slot) = ((y * density).round() / density) as f32;
                    let minimum = if g.mode == 3 { SCROLL_FINGER_MIN_VELOCITY } else { SCROLL_MIN_VELOCITY };
                    if (g.velocity * (-t / tau).exp()).abs() < minimum {
                        self.glides[ax][slot].mode = 0;
                    }
                }
            }
            let hold = if ax == 0 { self.gestures[slot].hold() } else { 0.0 };
            let g = &mut self.glides[ax][slot];
            if g.mode == 2 {
                let t = 0.0f64.max(time - g.start);
                let visual = g.velocity * ((-SCROLL_BOUNCE_A * t).exp() - (-SCROLL_BOUNCE_B * t).exp());
                let settled = t > 1.0 / SCROLL_BOUNCE_A && visual.abs() < 0.25;
                if settled {
                    g.mode = 0;
                }
                g.visual = if settled { 0.0 } else { visual };
                g.raw = scroll_travel(g.visual, extent);
            } else if g.visual != 0.0 && time - g.event_time > 0.08 {
                let dt = 0.0f64.max(time - g.frame_time);
                let visual = hold + (g.visual - hold) * (-dt / SCROLL_SPRING_TAU).exp();
                g.visual = if (visual - hold).abs() < 0.25 { hold } else { visual };
                // resting at a refresh hold is a stationary stretch: no more frames
                if hold != 0.0 && g.visual == hold {
                    g.event_time = f64::INFINITY;
                }
                g.raw = scroll_travel(g.visual, extent);
            }
        }
        self.glides[ax][slot].frame_time = time;
        if self.glides[ax][slot].moving() && !self.moving.contains(&slot) {
            self.moving.push(slot);
        }
    }

    /// Top-down: relative sizes, grow and shrink shares, alignment, scrolling, scroll gutters and
    /// positions. A kept view's nodes are not in the build, so their layout is not run again.
    fn arrange(&mut self, time: f64) {
        self.l_x[0] = 0.0;
        self.l_y[0] = 0.0;
        for index in 0..self.count {
            // A leaf's position and size are already resolved by its parent.
            if self.boxes[index].first == NONE && !self.nd(index).flags.any(Flags::SCROLL | Flags::SCROLL_X) {
                continue;
            }
            let node = self.nd(index);
            let (pad, flags, grid) = (node.padding, node.flags, node.grid.get());
            let n = FlowLayout { axis: node.axis, justify: node.justify, align: node.align,
                wrap: node.wrap, gap: node.gap, row_gap: node.row_gap };
            let scrolls = flags.any(Flags::SCROLL);
            let max_scroll = 0.0f32.max(self.l_content[index] - self.l_h[index]);
            // overlay scrollers (the system's style) float their thumb over the content: no gutter
            let overlay = self.environment.scrollers == pxui_types::Environment::OVERLAY_SCROLLERS;
            let gutter = if scrolls && max_scroll > 0.0 && !flags.any(Flags::OVER_THUMB) && !overlay { 10.0 } else { 0.0 };
            let slot = self.boxes[index].slot;
            if scrolls {
                self.apply_scroll(false, time, index);
                self.scroll_y[slot] = 0.0f32.max(max_scroll.min(self.scroll_y[slot]));
            }
            if flags.any(Flags::SCROLL_X) {
                self.apply_scroll_x(false, time, index);
                let max_x = 0.0f32.max(self.l_content_w[index] - self.l_w[index]);
                self.scroll_xs[slot] = 0.0f32.max(max_x.min(self.scroll_xs[slot]));
            }
            let inner_w = self.l_w[index] - pad.l - pad.r - gutter;
            let inner_h = self.l_h[index] - pad.t - pad.b;
            let relative = |kind: Size, inner: f32| match kind {
                Size::Pct(fraction) => Some(inner * fraction),
                Size::Rel(f) => Some(f(inner)),
                _ => None,
            };
            let mut next = self.boxes[index].first;
            while next != NONE {
                let child = &self.slots[self.boxes[next].slot].node;
                if child.template != NodeTemplate::PositionedLabel {
                    let w = relative(child.w, inner_w).map(|w| clamp(w, child.min_w, child.max_w));
                    let h = relative(child.h, inner_h).map(|h| clamp(h, child.min_h, child.max_h));
                    if let Some(w) = w { self.l_w[next] = w; }
                    if let Some(h) = h { self.l_h[next] = h; }
                }
                next = self.boxes[next].next;
            }
            let (bake_x, bake_y) = self.baked(index);
            let (origin_x, origin_y) = (self.l_x[index] + bake_x, self.l_y[index] + bake_y);
            let inner_at = (origin_x + pad.l, origin_y + pad.t);
            match (grid, n.axis) {
                (Some(grid), _) => self.arrange_grid(index, &n, &grid, (inner_w, inner_h), inner_at),
                (None, Axis::Stack) => self.arrange_stack(index, &n, (inner_w, inner_h), inner_at),
                (None, _) => self.arrange_flow(index, &n, (inner_w, inner_h), inner_at),
            }
            let scroll_y = if scrolls { self.scroll_y[slot] + self.glides[0][slot].visual as f32 } else { 0.0 };
            let scroll_x = if flags.any(Flags::SCROLL_X) { self.scroll_xs[slot] + self.glides[1][slot].visual as f32 } else { 0.0 };
            let rtl = self.dir == Direction::Rtl;
            let (root_w, root_h) = (self.l_w[0], self.l_h[0]);
            let last = self.boxes[index].last;
            let mut stuck = false;
            next = self.boxes[index].first;
            while next != NONE {
                let child = next;
                let c = &self.slots[self.boxes[child].slot].node;
                if c.template == NodeTemplate::PositionedLabel {
                    let (at_x, at_y) = c.at.get().unwrap_or_default();
                    self.l_x[child] = origin_x + at_x;
                    self.l_y[child] = origin_y + at_y;
                    next = self.boxes[child].next;
                    continue;
                }
                let c = Placed { flags: c.flags, margin_t: c.margin.t, anchor: c.anchor.get(), w_grow: c.w.is_grow(), h_grow: c.h.is_grow(), at: c.at.get() };
                if self.flow(child) {
                    self.l_y[child] -= scroll_y;
                    self.l_x[child] -= scroll_x;
                    if rtl {
                        self.l_x[child] = 2.0 * origin_x + self.l_w[index] - self.l_x[child] - self.l_w[child];
                    }
                    if c.flags.any(Flags::STICKY) && scrolls {
                        self.l_y[child] = self.l_y[child].max(inner_at.1 + c.margin_t);
                        stuck = true;
                    }
                } else if let Some(a) = c.anchor {
                    let r = place_anchored(a.rect, (self.l_w[child], self.l_h[child]), a.placement, Rect::new(0.0, 0.0, root_w, root_h));
                    (self.l_x[child], self.l_y[child]) = (r.x, r.y);
                } else {
                    // an `at` child's Grow size fills the parent across its axis (along it, none)
                    if n.axis == Axis::Row && c.h_grow {
                        self.set_main(false, child, inner_h);
                    } else if n.axis != Axis::Row && c.w_grow {
                        self.set_main(true, child, inner_w);
                    }
                    let (at_x, at_y) = c.at.unwrap_or((0.0, 0.0));
                    self.l_x[child] = origin_x + at_x;
                    self.l_y[child] = origin_y + at_y;
                }
                next = self.boxes[child].next;
            }
            // stuck headers paint (and take input) over the rows scrolling under them
            if stuck {
                let mut child = self.boxes[index].first;
                while child != NONE {
                    let next = self.boxes[child].next;
                    if self.nd(child).flags.any(Flags::STICKY) && child != self.boxes[index].last {
                        self.unlink(index, child);
                        let end = self.boxes[index].last;
                        self.boxes[end].next = child;
                        self.boxes[index].last = child;
                    }
                    if child == last {
                        break;
                    }
                    child = next;
                }
            }
        }
        self.arranged = self.count;
    }

    /// Take `child` out of `index`'s child list.
    fn unlink(&mut self, index: usize, child: usize) {
        let mut previous = NONE;
        let mut at = self.boxes[index].first;
        while at != child {
            previous = at;
            at = self.boxes[at].next;
        }
        let next = self.boxes[child].next;
        if previous == NONE {
            self.boxes[index].first = next;
        } else {
            self.boxes[previous].next = next;
        }
        if self.boxes[index].last == child {
            self.boxes[index].last = previous;
        }
        self.boxes[child].next = NONE;
    }

    fn main(&self, row: bool, c: usize) -> f32 {
        if row { self.l_w[c] } else { self.l_h[c] }
    }

    fn set_main(&mut self, row: bool, c: usize, v: f32) {
        let n = self.nd(c);
        if row {
            self.l_w[c] = clamp(v, n.min_w, n.max_w);
        } else {
            self.l_h[c] = clamp(v, n.min_h, n.max_h);
        }
    }

    fn set_cross(&mut self, row: bool, c: usize, v: f32) {
        self.set_main(!row, c, v);
    }

    /// The distance from a node's top to its first text baseline. A node without text takes its
    /// first in-flow child's, when that child's top is known before the node is arranged (a column
    /// justified to the start, or a row aligning that child to the start or stretching it; not
    /// scrolled, no grid); otherwise, or with no child, its bottom edge.
    fn baseline(&mut self, c: usize) -> f32 {
        let slot = self.boxes[c].slot;
        let h = self.l_h[c];
        if self.slots[slot].text.is_empty() && !self.slots[slot].rich.as_ref().is_some_and(|r|r.line_box) {
            let n = *self.nd(c);
            let f = self.next_flow(self.boxes[c].first);
            if f == NONE || n.grid.is_some() || n.flags.any(Flags::SCROLL | Flags::SCROLL_X) {
                return h;
            }
            let fa = self.nd(f).align_self.get().unwrap_or(n.align);
            let top_known = match n.axis {
                Axis::Column => n.justify == Justify::Start,
                Axis::Row => !n.wrap && matches!(fa, Align::Start | Align::Stretch),
                Axis::Stack => false,
            };
            return if top_known { n.padding.t + self.nd(f).margin.t + self.baseline(f) } else { h };
        }
        let (look, inset) = (self.slots[slot].node.look, self.slots[slot].node.inset);
        let size = look.font.points(self.font_points);
        let stored=&self.slots[slot];
        let flags=stored.node.flags;
        let pixel=flags.any(Flags::FIT_TEXT_PIXEL);
        let rich=stored.rich.as_ref();
        let style=if pixel {plain_style(&look,size,flags)} else {Style {font:look.font.family,size:size as f32,weight:look.font.weight,tracking:look.font.tracking*size as f32,..Style::default()}};
        let value=if pixel && rich.is_none() && look.font.caps {
            self.label.clear();self.label.extend(stored.text.chars().map(|c|c.to_ascii_uppercase()));self.label.as_str()
        } else {stored.text.as_str()};
        let text=self.text.as_mut().expect("Ui: the text engine is lent out");
        let ahead=self.boxes[c].ahead;
        let m=if let Some(rich)=rich {lay_out_node(text,&stored.text_layout,flags,&rich.style(&stored.node,self.dir==Direction::Rtl),self.l_w[c]-inset.l-inset.r,value,rich.spans(&self.span_arena),ahead).unwrap().1}
            else {lay_out_node(text,&stored.text_layout,flags,&style,0.0,value,&[],ahead).unwrap().1};
        if let Some(rich)=rich.filter(|r|r.line_box) { return rich.line_box_baseline(inset.t,h-inset.t-inset.b,m); }
        let top=rich.map_or_else(||text_top_at(size,inset.t,h-inset.t-inset.b),|r|r.top(inset.t,h-inset.t-inset.b,m));
        top+m.baseline
    }

    /// A row or column: lines (one unless it wraps), each sharing its room by grow and shrink
    /// weights, placed along by `justify` and across by `align`.
    fn arrange_flow(&mut self, index: usize, n: &FlowLayout, inner: (f32, f32), at: (f32, f32)) {
        let row = n.axis == Axis::Row;
        let (main_room, cross_room) = if row { inner } else { (inner.1, inner.0) };
        let main_margins = |m: Edges| if row { (m.l, m.r) } else { (m.t, m.b) };
        let cross_margins = |m: Edges| if row { (m.t, m.b) } else { (m.l, m.r) };
        let mut start = self.next_flow(self.boxes[index].first);
        // One ordinary child needs neither a weight pass nor a baseline pass.
        if !n.wrap && n.justify == Justify::Start && start != NONE
            && self.next_flow(self.boxes[start].next) == NONE
        {
            let c = self.nd(start);
            let (main, cross) = if row { (c.w, c.h) } else { (c.h, c.w) };
            let align = c.align_self.get().unwrap_or(n.align);
            if matches!(align, Align::Start | Align::Stretch) && !c.wrap && c.aspect == 0.0
                && c.shrink == 0.0 && (!main.is_grow() || c.grow == 1.0)
            {
                let (m, (ms, me), (cs, ce)) = (c.margin, main_margins(c.margin), cross_margins(c.margin));
                if main.is_grow() { self.set_main(row, start, (main_room - (ms + me)).max(0.0)); }
                if cross.is_grow() || (align == Align::Stretch && matches!(cross, Size::Fit)) {
                    self.set_cross(row, start, cross_room - cs - ce);
                }
                (self.l_x[start], self.l_y[start]) = (at.0 + m.l, at.1 + m.t);
                return;
            }
        }
        let mut cross_at = 0.0;
        while start != NONE {
            // the line: its children, their bases and the room they take
            let (mut used, mut count, mut end) = (0.0f32, 0usize, start);
            while end != NONE {
                let (cross_size, main_size, margin, align, aspect, wrap) = {
                    let c = self.nd(end);
                    (if row {c.h} else {c.w}, if row {c.w} else {c.h}, c.margin, c.align_self.get().unwrap_or(n.align), c.aspect, c.wrap)
                };
                // a one-line flow knows its children's stretched cross sizes now, and an aspect
                // child's Fit main size follows from them
                let (cs, ce) = cross_margins(margin);
                if !n.wrap && (cross_size.is_grow() || (align == Align::Stretch && matches!(cross_size, Size::Fit))) {
                    self.set_cross(row, end, cross_room - cs - ce);
                }
                if aspect > 0.0 && matches!(main_size, Size::Fit) {
                    let cross = self.main(!row, end);
                    self.set_main(row, end, if row { cross * aspect } else { cross / aspect });
                }
                // in a column a wrapping row's width is known now; in a row, after the shares
                if !row && wrap {
                    self.rewrap(end);
                }
                // a Grow child starts from nothing (its minimum holds in `grow_line`)
                let basis = if main_size.is_grow() { 0.0 } else { self.main(row, end) };
                let (ms, me) = main_margins(margin);
                let outer = basis + ms + me;
                if n.wrap && count > 0 && used + n.gap + outer > main_room {
                    break;
                }
                used += if count > 0 { n.gap } else { 0.0 } + outer;
                count += 1;
                self.set_main(row, end, basis);
                end = self.next_flow(self.boxes[end].next);
            }
            let free = main_room - used;
            self.grow_line(row, start, end, free);
            if free < 0.0 {
                self.shrink_line(row, start, end, -free);
            }
            // across: stretched, aspect-derived, the line's extent and its baseline
            let (mut line_cross, mut base_max) = (0.0f32, 0.0f32);
            let mut c = start;
            while c != end {
                let (wrap, margin, a, fit, aspect) = {
                    let cn = self.nd(c);
                    (cn.wrap,cn.margin,cn.align_self.get().unwrap_or(n.align),matches!(if row {cn.h} else {cn.w},Size::Fit),cn.aspect)
                };
                if row && wrap {
                    self.rewrap(c);
                }
                let (cs, ce) = cross_margins(margin);
                if aspect > 0.0 && fit {
                    let main = self.main(row, c);
                    self.set_cross(row, c, if row { main / aspect } else { main * aspect });
                }
                line_cross = line_cross.max(self.main(!row, c) + cs + ce);
                if a == Align::Baseline {
                    base_max = base_max.max(cs + self.baseline(c));
                }
                c = self.next_flow(self.boxes[c].next);
            }
            if !n.wrap {
                line_cross = cross_room;
            }
            let mut used = 0.0;
            c = start;
            while c != end {
                let (ms, me) = main_margins(self.nd(c).margin);
                used += self.main(row, c) + ms + me;
                c = self.next_flow(self.boxes[c].next);
            }
            let rem = main_room - used - n.gap * count.saturating_sub(1) as f32;
            let k = count as f32;
            let (mut pos, between) = match n.justify {
                Justify::Start => (0.0, n.gap),
                Justify::Center => (rem / 2.0, n.gap),
                Justify::End => (rem, n.gap),
                Justify::SpaceBetween if rem > 0.0 && count > 1 => (0.0, n.gap + rem / (k - 1.0)),
                Justify::SpaceAround if rem > 0.0 => (rem / k / 2.0, n.gap + rem / k),
                Justify::SpaceEvenly if rem > 0.0 => (rem / (k + 1.0), n.gap + rem / (k + 1.0)),
                _ => (0.0, n.gap),
            };
            c = start;
            while c != end {
                let (margin,a,cross_size) = {
                    let cn = self.nd(c);
                    (cn.margin,cn.align_self.get().unwrap_or(n.align),if row {cn.h} else {cn.w})
                };
                let (ms, me) = main_margins(margin);
                let (cs, ce) = cross_margins(margin);
                if n.wrap && (cross_size.is_grow() || (a == Align::Stretch && matches!(cross_size, Size::Fit))) {
                    self.set_cross(row, c, line_cross - cs - ce);
                }
                let outer = self.main(!row, c) + cs + ce;
                let off = match a {
                    Align::Center => (line_cross - outer) / 2.0,
                    Align::End => line_cross - outer,
                    Align::Baseline => base_max - cs - self.baseline(c),
                    _ => 0.0,
                } + cs;
                let main_at = pos + ms;
                if row {
                    (self.l_x[c], self.l_y[c]) = (at.0 + main_at, at.1 + cross_at + off);
                } else {
                    (self.l_x[c], self.l_y[c]) = (at.0 + cross_at + off, at.1 + main_at);
                }
                pos = main_at + self.main(row, c) + me + between;
                c = self.next_flow(self.boxes[c].next);
            }
            cross_at += line_cross + n.row_gap;
            start = end;
        }
    }

    /// Share `free` points among the line's `Grow` children by their `grow` weights; a child held
    /// at its minimum or maximum leaves the rest to the others. With nothing free they keep their
    /// minimum.
    // Each pass freezes every child its share would push past its min or max, so passes are at
    // most the clamped children plus one: O(children x clamped), linear when nothing clamps.
    fn grow_line(&mut self, row: bool, start: usize, end: usize, mut free: f32) {
        self.grow_pass = self.grow_pass.wrapping_add(1).max(1);
        let pass = self.grow_pass;
        loop {
            let (mut total, mut c) = (0.0, start);
            while c != end {
                let n = self.nd(c);
                if (if row { n.w.is_grow() } else { n.h.is_grow() }) && n.grow > 0.0 && self.boxes[c].frozen != pass {
                    total += n.grow;
                }
                c = self.next_flow(self.boxes[c].next);
            }
            if total <= 0.0 {
                return;
            }
            let unit = free.max(0.0) / total;
            let mut capped = false;
            c = start;
            while c != end {
                let (grows, weight, min, max) = {
                    let n = self.nd(c);
                    (if row {n.w.is_grow()} else {n.h.is_grow()},n.grow,if row {n.min_w} else {n.min_h},if row {n.max_w} else {n.max_h})
                };
                if grows && weight > 0.0 && self.boxes[c].frozen != pass {
                    let v = unit * weight;
                    if v < min || v > max {
                        let held = clamp(v, min, max);
                        free -= held;
                        self.set_main(row, c, held);
                        self.boxes[c].frozen = pass;
                        capped = true;
                    }
                }
                c = self.next_flow(self.boxes[c].next);
            }
            if !capped {
                c = start;
                while c != end {
                    let n = self.nd(c);
                    if (if row { n.w.is_grow() } else { n.h.is_grow() }) && n.grow > 0.0 && self.boxes[c].frozen != pass {
                        let g = n.grow;
                        self.set_main(row, c, unit * g);
                    }
                    c = self.next_flow(self.boxes[c].next);
                }
                return;
            }
        }
    }

    /// Take `over` points from the line's children by `shrink` times their size, none below its
    /// minimum.
    fn shrink_line(&mut self, row: bool, start: usize, end: usize, mut over: f32) {
        let min = |ui: &Ui, c: usize| if row { ui.nd(c).min_w } else { ui.nd(c).min_h };
        loop {
            let mut total = 0.0;
            let mut c = start;
            while c != end {
                if self.nd(c).shrink > 0.0 && self.main(row, c) > min(self, c) {
                    total += self.nd(c).shrink * self.main(row, c);
                }
                c = self.next_flow(self.boxes[c].next);
            }
            if total <= 0.0 || over <= 0.0 {
                return;
            }
            let ratio = over / total;
            let mut capped = false;
            c = start;
            while c != end {
                let (s, v, lo) = (self.nd(c).shrink, self.main(row, c), min(self, c));
                if s > 0.0 && v > lo && v - ratio * s * v < lo {
                    over -= v - lo;
                    self.set_main(row, c, lo);
                    capped = true;
                }
                c = self.next_flow(self.boxes[c].next);
            }
            if !capped {
                c = start;
                while c != end {
                    let (s, v, lo) = (self.nd(c).shrink, self.main(row, c), min(self, c));
                    if s > 0.0 && v > lo {
                        self.set_main(row, c, v - ratio * s * v);
                    }
                    c = self.next_flow(self.boxes[c].next);
                }
                return;
            }
        }
    }

    /// A stack: every flow child at the inner origin, `Grow` sizes filling it, placed across by
    /// `justify` and down by `align`.
    fn arrange_stack(&mut self, index: usize, n: &FlowLayout, inner: (f32, f32), at: (f32, f32)) {
        let mut c = self.next_flow(self.boxes[index].first);
        while c != NONE {
            let (w_grow,h_grow,h_fit,aspect,wrap,m,align) = {
                let cn = self.nd(c);
                (cn.w.is_grow(),cn.h.is_grow(),matches!(cn.h,Size::Fit),cn.aspect,cn.wrap,cn.margin,cn.align_self.get().unwrap_or(n.align))
            };
            if w_grow {
                self.set_main(true, c, inner.0 - m.l - m.r);
            }
            if h_grow {
                self.set_main(false, c, inner.1 - m.t - m.b);
            }
            if aspect > 0.0 && h_fit {
                self.set_main(false, c, self.l_w[c] / aspect);
            }
            if wrap {
                self.rewrap(c);
            }
            let ow = self.l_w[c] + m.l + m.r;
            let oh = self.l_h[c] + m.t + m.b;
            let x = match n.justify {
                Justify::Center => (inner.0 - ow) / 2.0,
                Justify::End => inner.0 - ow,
                _ => 0.0,
            };
            let y = match align {
                Align::Center => (inner.1 - oh) / 2.0,
                Align::End => inner.1 - oh,
                _ => 0.0,
            };
            (self.l_x[c], self.l_y[c]) = (at.0 + x + m.l, at.1 + y + m.t);
            c = self.next_flow(self.boxes[c].next);
        }
    }

    /// A grid: tracks sized by `grid_tracks`, each child in its cell (a `Grow` size fills the cell;
    /// `align` places it down).
    fn arrange_grid(&mut self, index: usize, n: &FlowLayout, g: &Grid, inner: (f32, f32), at: (f32, f32)) {
        let (cols, nc, mut rows) = self.grid_tracks(index, (n.gap, n.row_gap), g, Some(inner));
        let mut col_x = [0.0f32; MAX_TRACKS + 1];
        for i in 0..nc {
            col_x[i + 1] = col_x[i] + cols[i] + n.gap;
        }
        // the row tops follow the row sizes in the scratch
        let nr = rows.len();
        rows.push(0.0);
        for i in 0..nr {
            let y = rows[nr + i] + rows[i] + n.row_gap;
            rows.push(y);
        }
        let row_y = |r: usize| rows[nr + r];
        let mut cursor = (0, 0);
        let mut c = self.next_flow(self.boxes[index].first);
        while c != NONE {
            let (m,cell,a,w_grow,h_grow,h_fit,aspect) = {
                let cn = self.nd(c);
                (cn.margin,cn.cell,cn.align_self.get().unwrap_or(n.align),cn.w.is_grow(),cn.h.is_grow(),matches!(cn.h,Size::Fit),cn.aspect)
            };
            let (col, row, cs, rs) = next_cell(cell, &mut cursor, nc);
            let span_w = col_x[col + cs] - col_x[col] - n.gap;
            let span_h = row_y(row + rs) - row_y(row) - n.row_gap;
            if w_grow {
                self.set_main(true, c, span_w - m.l - m.r);
            }
            if h_grow || (a == Align::Stretch && h_fit) {
                self.set_main(false, c, span_h - m.t - m.b);
            }
            if aspect > 0.0 && h_fit {
                self.set_main(false, c, self.l_w[c] / aspect);
            }
            let oh = self.l_h[c] + m.t + m.b;
            let y = match a {
                Align::Center => (span_h - oh) / 2.0,
                Align::End => span_h - oh,
                _ => 0.0,
            };
            (self.l_x[c], self.l_y[c]) = (at.0 + col_x[col] + m.l, at.1 + row_y(row) + y + m.t);
            c = self.next_flow(self.boxes[c].next);
        }
        self.grid_rows.set(rows);
    }

    fn screen(&self, index: usize) -> Rect {
        Rect::new(self.l_x[index], self.l_y[index], self.l_w[index], self.l_h[index])
    }

    pub(crate) fn set_l_h(&mut self, b: &UiBox, h: f32) {
        self.l_h[b.index] = h;
    }

    pub(crate) fn set_l_content(&mut self, b: &UiBox, v: f32) {
        self.l_content[b.index] = v;
    }
}

/// `v` held to `[min, max]`; the minimum wins.
/// `kept` becomes a copy of `f` in the storage it already has (a frame with input allocated twice).
fn keep_frame(kept: &mut Option<rframe::Frame>, f: &rframe::Frame) {
    let k = kept.get_or_insert_with(|| {
        let mut input=input::at_time(0.0);
        input.events.reserve(input::EVENT_RESERVE);
        rframe::Frame { input, pixel_scale:(1.0,1.0),fps:0.0,mouse_delta:(0.0,0.0) }
    });
    let rframe::Frame { input, pixel_scale, fps, mouse_delta } = f;
    let input::Frame { t, dt, frame, tick, size, pointer, buttons, keys, events } = input;
    let i = &mut k.input;
    (i.t, i.dt, i.frame, i.tick, i.size, i.pointer) = (*t, *dt, *frame, *tick, *size, *pointer);
    i.buttons.clone_from(buttons);
    i.keys.clone_from(keys);
    i.events.clone_from(events);
    (k.pixel_scale, k.fps, k.mouse_delta) = (*pixel_scale, *fps, *mouse_delta);
}

/// The container properties used to place its children.
struct FlowLayout {
    axis: Axis,
    justify: Justify,
    align: Align,
    wrap: bool,
    gap: f32,
    row_gap: f32,
}

/// The child properties needed to place a box.
struct Placed {
    flags: Flags,
    margin_t: f32,
    anchor: Option<Anchor>,
    w_grow: bool,
    h_grow: bool,
    at: Option<(f32, f32)>,
}

fn clamp(v: f32, min: f32, max: f32) -> f32 {
    v.min(max).max(min)
}

/// Tracks' total extent with `gap` between them.
fn span(tracks: &[f32], gap: f32) -> f32 {
    tracks.iter().sum::<f32>() + gap * tracks.len().saturating_sub(1) as f32
}

/// A child spanning `count` tracks from `at` needs `need` points: what its tracks (fixed and the
/// content ones so far, with the gaps) lack is shared equally by its `Fit` tracks. A span over an
/// `Fr` track sizes nothing (as in CSS: the fractions take the room).
fn spread(tracks: &[Track], content: &mut [f32], at: usize, count: usize, need: f32, gap: f32) {
    let (mut have, mut fits) = (gap * (count - 1) as f32, 0);
    for i in at..at + count {
        match tracks.get(i).copied().unwrap_or(Track::Fit) {
            Track::Px(v) => have += v,
            Track::Fr(_) => return,
            Track::Fit => (have, fits) = (have + content[i], fits + 1),
        }
    }
    if fits > 0 && need > have {
        let each = (need - have) / fits as f32;
        for i in at..at + count {
            if matches!(tracks.get(i).copied().unwrap_or(Track::Fit), Track::Fit) {
                content[i] += each;
            }
        }
    }
}

/// A grid child's (column, row, column span, row span): its own cell, or the next free one along
/// the rows from `cursor` (auto placement; explicit cells are not avoided).
fn next_cell(c: Cell, cursor: &mut (usize, usize), cols: usize) -> (usize, usize, usize, usize) {
    let cs = (c.col_span.max(1) as usize).min(cols);
    let (col, row) = if c.col == Cell::AUTO {
        if cursor.0 + cs > cols {
            *cursor = (0, cursor.1 + 1);
        }
        let at = *cursor;
        cursor.0 += cs;
        at
    } else {
        ((c.col as usize).min(cols - 1), c.row as usize)
    };
    (col, row, cs.min(cols - col), c.row_span.max(1) as usize)
}

// Scrolling as NSScrollView does it, the constants fitted to a recording of one. Past an edge
// `travel` points of input show as `stretch travel height`; left alone the overscroll decays with
// tau 0.084 s. A trackpad's fingers move the content point for point and hold a stretch until
// they lift; lifted in motion, the box coasts on the release velocity decaying with tau 0.26 s,
// and reaching an edge it overshoots as the overdamped spring. Every motion is a closed form of
// the time since it began, so the frame rate does not change it.
const SCROLL_STIFFNESS: f64 = 0.55;
const SCROLL_WHEEL_PAST: f64 = 0.05 / 0.55;
const SCROLL_SPRING_TAU: f64 = 0.084;
const SCROLL_COAST_TAU: f64 = 0.26;
// v(t) = v0 * 0.998^(1000t), UIScrollView's normal millisecond decay.
const SCROLL_FINGER_TAU: f64 = 0.499499833166455;
const SCROLL_BOUNCE_A: f64 = 9.25;
const SCROLL_BOUNCE_B: f64 = 19.0;
const SCROLL_BOUNCE_LAUNCH: f64 = 0.39;
const SCROLL_MIN_VELOCITY: f64 = 20.0;
const SCROLL_FINGER_MIN_VELOCITY: f64 = 5.0;

fn scroll_stretch(travel: f64, height: f64) -> f64 {
    if height <= 0.0 {
        0.0
    } else {
        SCROLL_STIFFNESS * travel / (1.0 + SCROLL_STIFFNESS * travel.abs() / height)
    }
}

/// the travel that shows `overscroll`: the inverse of `scroll_stretch`
fn scroll_travel(overscroll: f64, height: f64) -> f64 {
    if height <= 0.0 || overscroll == 0.0 {
        0.0
    } else {
        let shown = overscroll.abs().min(height * 0.999);
        (shown / (SCROLL_STIFFNESS * (1.0 - shown / height))).copysign(overscroll)
    }
}

// ----------------------------------------------------------- painting


/// The painter's handle for one frame. It owns the layers being filled, the glyph atlas and the
/// layout arrays painters read while painting; `Ui::frame` hands them back afterwards.
pub struct Paint {
    layers: Vec<Layer>,
    layer: usize,
    /// the view table index instances take
    view: u16,
    pub clip_rect: Rect,
    /// the corner radius of `clip_rect` (a rounded `CLIP` node's children)
    pub clip_radius: f32,
    /// The callback uses symbolic shape colors so the view can be recolored after its build.
    pub capture_palette: bool,
    pub theme: Tokens,
    pub font_size: i64,
    pub row_height: i64,
    /// device pixels per point
    pub density: i64,
    /// the `Ui`'s colour tokens resolved for `theme` (`Ui::color`), transparent past the table
    pub colors: [Rgba; pxui_types::theme::ColorToken::LIMIT],
    /// the root theme's label look, which a `Label`'s unset fields resolve from (`Paint::label`)
    pub label_look: Look,
    pub text: Box<Text>,
    // Two recent ppem/density pairs share the Apple shader's packed half values.
    glyph_light_memo: [([u32; 2], [u32; 2]); 2],
    /// the focused text's input area and caret offset
    pub ime: Option<((i64, i64, i64, i64), i64)>,
    label: String,
    /// the focused text field's value and the IME composition this frame's edit canvas paints, lent by `Ui`
    pub(crate) edit_text: String,
    pub(crate) composition: String,
    /// the horizontal scroll of the focused text; painters adjust it
    pub edit_scroll_x: f32,
    pub(crate) composition_selection: (i64, i64),
    /// the content height of each box of this frame, by box index
    pub l_content: Vec<f32>,
    /// the scroll offset of each retained slot
    pub scroll_y: Vec<f32>,
    /// the content width and horizontal scroll offset, as `l_content` and `scroll_y`
    pub l_content_w: Vec<f32>,
    pub scroll_x: Vec<f32>,
    /// right to left: node text sits its start inset from the right, scroll thumbs mirror
    pub rtl: bool,
    /// overlay scrollers: a thumb shows only while its offset moves and `Environment::SCROLLER_HOLD` after (`Ui::thumbs`, lent);
    /// the frame's time, and the thumbs painted with the time each hides
    overlay: bool,
    now: f64,
    thumbs: Vec<[(f32, f64); 2]>,
    shown: Vec<(usize, f64)>,
    /// the node being painted is in a list row past the viewport and out of view: text the cache has not seen goes to the
    /// prepare queue (`Text::defer`) and draws nothing
    ahead: bool,
}

impl Paint {
    /// Colour token `token` resolved for this painter's theme; transparent past the table.
    #[inline] pub fn color(&self, token: u16) -> Rgba { token_entry(&self.colors, self.theme.accent, token) }

    /// `text` in `label` at `at`, laid out within `width` (0: unbounded), its unset fields resolved as a label body resolves them
    /// against this painter's theme (`label_look`, its rows and colour tokens); the label's box fields (width, pad) do not apply.
    /// `None`, drawing nothing, when the style does not resolve.
    pub fn label(&mut self, at: (f32, f32), label: &pxui_types::text::LabelFields, width: f32, text: &str) -> Option<Metrics> {
        let resolved = crate::ui_kit::resolve_label(label, &self.label_look, self.font_size as f32, self.row_height as f32, label.color.map(|t| self.color(t.0)))?;
        if !resolved.caps { return Some(self.text_styled(at, &resolved.style, width, Some(resolved.color), text)); }
        let mut buffer = core::mem::take(&mut self.label);
        buffer.clear();
        buffer.extend(text.chars().map(|c| c.to_ascii_uppercase()));
        let m = self.text_styled(at, &resolved.style, width, Some(resolved.color), &buffer);
        self.label = buffer;
        Some(m)
    }

    /// `text` in `label` one cached character layout at a time, digits on the advance of `0`, resolved as `label`; the width drawn.
    pub fn label_chars(&mut self, at: (f32, f32), label: &pxui_types::text::LabelFields, text: &str) -> Option<f32> {
        let resolved = crate::ui_kit::resolve_label(label, &self.label_look, self.font_size as f32, self.row_height as f32, label.color.map(|t| self.color(t.0)))?;
        Some(self.chars(at, &resolved.style, Some(resolved.color), resolved.caps, text))
    }

    /// `text` laid out in `label` within `width` (0: unbounded) with `spans`, as `label` resolves and draws it (capitals included),
    /// without drawing: the painter's text service measures what this painter paints. `None` when the style or spans do not resolve.
    pub fn label_metrics(&mut self, label: &pxui_types::text::LabelFields, width: f32, text: &str, spans: &[TextSpan]) -> Option<Metrics> {
        let resolved = crate::ui_kit::resolve_label(label, &self.label_look, self.font_size as f32, self.row_height as f32, label.color.map(|t| self.color(t.0)))?;
        let mut buffer = core::mem::take(&mut self.label);
        buffer.clear();
        if resolved.caps { buffer.extend(text.chars().map(|c| c.to_ascii_uppercase())); }
        let value = if resolved.caps { buffer.as_str() } else { text };
        let laid = lay_out_spans(&mut self.text, &resolved.style, width, value, spans).map(|(_, m)| m);
        self.label = buffer;
        laid
    }

    /// `text` in `label` with `spans` at `at`, laid out within `width`, resolved as `label` resolves it; the metrics drawn.
    pub fn label_spans(&mut self, at: (f32, f32), label: &pxui_types::text::LabelFields, width: f32, text: &str, spans: &[TextSpan]) -> Option<Metrics> {
        let resolved = crate::ui_kit::resolve_label(label, &self.label_look, self.font_size as f32, self.row_height as f32, label.color.map(|t| self.color(t.0)))?;
        if !resolved.caps { return self.text_spans(at, &resolved.style, width, Some(resolved.color), text, spans); }
        let mut buffer = core::mem::take(&mut self.label);
        buffer.clear();
        buffer.extend(text.chars().map(|c| c.to_ascii_uppercase()));
        let m = self.text_spans(at, &resolved.style, width, Some(resolved.color), &buffer, spans);
        self.label = buffer;
        m
    }

    /// Draws `text` one character at a time from one-character layouts in `style` (capitals when `caps`), digits on the advance of
    /// `0`: a number that changes every frame shapes nothing after the first use of its characters. Returns the width drawn.
    pub fn chars(&mut self, at: (f32, f32), style: &Style, color: Option<Rgba>, caps: bool, text: &str) -> f32 {
        let zero = self.text_metrics(style, 0.0, "0").w;
        let (mut x, mut utf8) = (at.0, [0u8; 4]);
        for ch in text.chars() {
            let ch = if caps { ch.to_ascii_uppercase() } else { ch };
            let advance = self.text_styled((x, at.1), style, 0.0, color, ch.encode_utf8(&mut utf8)).w;
            x += if ch.is_ascii_digit() { zero } else { advance };
        }
        x - at.0
    }

    /// Start the next layer (a floating root's, or the base with key 0), reusing a layer vector a previous frame left.
    fn next_layer(&mut self, key: i64, role: LayerRole, anchor: Rect) {
        self.layer = self.layer.wrapping_add(1);
        if self.layer == self.layers.len() {
            self.layers.push(Layer::new(key, 0));
        }
        let layer = &mut self.layers[self.layer];
        (layer.key, layer.role, layer.anchor) = (key, role, anchor);
        layer.instances.clear();
        layer.breaks.clear();
        layer.images.clear();
        layer.pieces.clear();
        layer.pieces.push(Piece { view: NO_VIEW, a: 0, b: 0, at: 0 });
    }

    /// End the current fresh piece and start the next where the instances stand.
    fn cut(&mut self) {
        let layer = &mut self.layers[self.layer];
        let n = layer.instances.len() as u32;
        layer.pieces.last_mut().expect("a layer starts with a piece").b = n;
        layer.pieces.push(Piece { view: NO_VIEW, a: n, b: n, at: 0 });
    }

    /// Kept view `v`'s run goes here.
    fn keep(&mut self, v: u32) {
        self.cut();
        let layer = &mut self.layers[self.layer];
        let n = layer.instances.len() as u32;
        let last = layer.pieces.len() - 1;
        layer.pieces[last].view = v;
        layer.pieces.push(Piece { view: NO_VIEW, a: n, b: n, at: 0 });
    }

    /// The text `look` would lay out for a node `w` points wide, deferred to the prepare queue when unseen (`Text::defer`):
    /// rich text at its inner width, plain text at its natural width, in capitals when its font says so (as `look`). Fitted
    /// plain text's cut depends on its natural layout: only the natural one is requested. Shapes nothing, draws nothing.
    fn defer_text(&mut self, w: f32, node: &Node, text: &str, rich: Option<&NodeText>, arena: &[TextSpan], memo: &std::cell::Cell<Option<(u64, u64)>>) {
        if let Some(rich) = rich {
            let style = rich.style(node, self.rtl);
            let spans = rich.spans(arena);
            let width = w - node.inset.l - node.inset.r;
            // not placed yet (its natural width is still on its way): nothing to request at this width
            if width <= 0.0 {
                return;
            }
            let fingerprint = memo.get().filter(|(key, _)| node.flags.any(Flags::FIT_TEXT_PIXEL) && self.text.has_text(*key, text))
                .map_or_else(|| pxui_text::text_hash(text), |(_, fingerprint)| fingerprint);
            self.text.defer(pxui_text::layout_key(&style, width, fingerprint, spans), text, &style, width, spans);
            return;
        }
        if text.is_empty() {
            return;
        }
        let look = &node.look;
        let size = look.font.points(self.font_size as f32);
        let style = if node.flags.any(Flags::FIT_TEXT_PIXEL) { plain_style(look, size, node.flags) }
            else { Style { font: look.font.family, size, weight: look.font.weight, tracking: look.font.tracking * size, ..Style::default() } };
        let mut buffer = std::mem::take(&mut self.label);
        buffer.clear();
        if look.font.caps { buffer.extend(text.chars().map(|c| c.to_ascii_uppercase())); }
        let value = if look.font.caps { buffer.as_str() } else { text };
        self.text.defer(pxui_text::content_key(&style, 0.0, value, &[]), value, &style, 0.0, &[]);
        self.label = buffer;
    }

    /// A node's look, text and mark in `r` (its ring too when `focused`): what the engine paints for it
    /// with no closure. Square, solid looks paint as hard rectangles on the pixel grid; rounded,
    /// gradient and shadowed ones as distance fields. `steady` (a running transition) emits the same
    /// instances whatever the colours, keeping only shadows that can show during the run.
    fn look(&mut self, r: Rect, node: &Node, look: &Look, text: &str, rich:Option<&NodeText>, arena: &[TextSpan], memo:&std::cell::Cell<Option<(u64,u64)>>, focused: bool, steady: Option<[bool; 2]>) {
        let background = steady.is_some() || node.flags.any(Flags::KEEP_ROOM) || look.gradient.is_some()
            || (look.fill.a | look.border.a | look.shadows[0].color.a | look.shadows[1].color.a) != 0;
        if !background && text.is_empty() && rich.is_none() && matches!(node.mark, Mark::None) && (!focused || look.ring.color.a == 0) {
            return;
        }
        let placed = r;
        let Rect { x, y, w, h } = placed;
        let o = look.opacity.clamp(0.0, 1.0);
        let fade = |c: Rgba| if o < 1.0 { Rgba { a: (c.a as f32 * o).round() as u8, ..c } } else { c };
        let corners = [x, y, x + w, y + h];
        let radii = look.radius;
        let area = w > 0.0 && h > 0.0;
        let shown = |c: Rgba| c.a > 0 || steady.is_some();
        let shadow_shown = |i: usize, s: &Shadow| s.color.a > 0 || steady.is_some_and(|shadows| shadows[i]);
        let round = steady.is_some() || look.gradient.is_some() || radii.iter().any(|r| *r > 0.0) || look.shadows.iter().any(|s| shown(s.color));
        let shadow = |s: &Shadow| Instance::shadow(corners, radii, rgba(fade(s.color)), s.offset, s.blur / 2.0, s.spread, s.inner);
        let bordered = steady.is_some() || (look.border_w > 0.0 && look.border.a > 0);
        if area && background {
            for (i, s) in look.shadows.iter().enumerate() {
                if !s.inner && shadow_shown(i, s) {
                    self.push(shadow(s));
                }
            }
            let inner = look.shadows.iter().enumerate().any(|(i, s)| s.inner && shadow_shown(i, s));
            if round {
                let mut shape = Instance::rect(corners, rgba(fade(look.fill))).radii(radii);
                if let Some(to) = look.gradient.get() {
                    let (dx, dy) = (look.angle.to_radians().sin(), -look.angle.to_radians().cos());
                    shape = shape.linear([0.5 - dx / 2.0, 0.5 - dy / 2.0], [0.5 + dx / 2.0, 0.5 + dy / 2.0], rgba(fade(to)));
                }
                let edge = Instance::rect(corners, [0; 4]).radii(radii).border(look.border_w, rgba(fade(look.border))).sides(look.sides);
                if bordered && !inner {
                    shape = Instance { width: edge.width, color2: edge.color2, flags: shape.flags & 0x0f | edge.flags & 0xf0, ..shape };
                }
                if shown(look.fill) || look.gradient.is_some() || (bordered && !inner) {
                    self.push(shape);
                }
                for (i, s) in look.shadows.iter().enumerate() {
                    if s.inner && shadow_shown(i, s) {
                        self.push(shadow(s));
                    }
                }
                if bordered && inner {
                    self.push(edge);
                }
            } else {
                let room = node.flags.any(Flags::KEEP_ROOM);
                if look.fill.a > 0 || room {
                    self.fill(x, y, w, h, fade(look.fill));
                }
                if look.dashed && (bordered || room) {
                    self.dashed_rect(x + 0.5, y + 0.5, w - 1.0, h - 1.0, 1.0, fade(look.border));
                } else if bordered || (room && look.border_w > 0.0) {
                    self.bands(placed, look.border_w, look.sides, fade(look.border));
                }
            }
        }
        if let Some(rich)=rich {
            let width=w-node.inset.l-node.inset.r;
            let style=rich.style(node,self.rtl);
            let spans=rich.spans(arena);
            // Explicit line boxes with a fixed width also reuse their last painted layout.
            let memo_flags=if rich.line_box && spans.is_empty() && matches!(node.w,Size::Px(_)) { node.flags | Flags::FIT_TEXT_PIXEL } else { node.flags };
            if let Some((key,metrics))=lay_out_node(&mut self.text,memo,memo_flags,&style,width,text,spans,self.ahead) {
                let below=self.text.ascent(style.font)*style.size;
                let (top,height)=(y+node.inset.t,h-node.inset.t-node.inset.b);
                let top=if rich.line_box { rich.line_box_baseline(top,height,metrics)-below } else { rich.top(top,height,metrics) };
                let at=(x+if self.rtl { node.inset.r } else { node.inset.l },top);
                self.text_laid(at,below,&style,Some(look.text),key,metrics,o,!spans.is_empty());
            }
        } else if !text.is_empty() && node.flags.any(Flags::FIT_TEXT_PIXEL) {
            let size=look.font.points(self.font_size as f32);
            let style=plain_style(look,size,node.flags);
            let inset=node.inset;
            let mut buffer=std::mem::take(&mut self.label);
            buffer.clear();
            if look.font.caps {buffer.extend(text.chars().map(|c|c.to_ascii_uppercase()));}
            let value=if look.font.caps {buffer.as_str()} else {text};
            let (mut key,mut metrics)=lay_out_node(&mut self.text,memo,node.flags,&style,0.0,value,&[],self.ahead).unwrap();
            let limit=w-inset.l-inset.r;
            if node.flags.any(Flags::FIT_TEXT) && metrics.w>limit {
                (key,metrics)=lay_out_node(&mut self.text,memo,node.flags,&style,limit,value,&[],self.ahead).unwrap();
            }
            let at=(if self.rtl {x+w-inset.l-metrics.w} else {x+inset.l},text_top_at(size,y+inset.t,h-inset.t-inset.b));
            let below=self.text.ascent(style.font)*style.size;
            self.text_laid(at,below,&style,Some(fade(look.text)),key,metrics,1.0,false);
            self.label=buffer;
        } else if !text.is_empty() {
            let size = look.font.points(self.font_size as f32);
            let inset = node.inset;
            let at = (x + inset.l, text_top_at(size, y + inset.t, h - inset.t - inset.b));
            let style = Style { font: look.font.family, size, weight: look.font.weight, tracking: look.font.tracking * size, ..Style::default() };
            let ink = Some(fade(look.text));
            let mut buffer = std::mem::take(&mut self.label);
            buffer.clear();
            let mut fitted = false;
            if node.flags.any(Flags::FIT_TEXT) {
                let limit = w - inset.l - inset.r;
                let plain = Style { tracking: 0.0, ..style };
                if lay_out(&mut self.text, &plain, 0.0, text).1.w > limit {
                    // Keep the legacy character-width fit, but reuse paint storage for the cut.
                    let dots = lay_out(&mut self.text, &plain, 0.0, "\u{2026}").1.w;
                    let mut cut = 0;
                    let mut used = 0.0;
                    for (index, character) in text.char_indices() {
                        let next = index + character.len_utf8();
                        used += lay_out(&mut self.text, &plain, 0.0, &text[index..next]).1.w;
                        if used > limit {
                            buffer.push_str(&text[..cut]);
                            buffer.push('\u{2026}');
                            fitted = true;
                            break;
                        }
                        if used + dots <= limit { cut = next; }
                    }
                }
            }
            if look.font.caps {
                if fitted { buffer.make_ascii_uppercase(); }
                else { buffer.extend(text.chars().map(|c| c.to_ascii_uppercase())); }
            }
            let text = if fitted || look.font.caps { buffer.as_str() } else { text };
            if !(self.ahead && self.text.defer(pxui_text::content_key(&style, 0.0, text, &[]), text, &style, 0.0, &[])) {
                // right to left the start inset is at the right: the text ends there
                let (key, m) = lay_out(&mut self.text, &style, 0.0, text);
                let at = if self.rtl { (x + w - inset.l - m.w, at.1) } else { at };
                let below=self.text.ascent(style.font)*style.size;
                self.text_laid(at, below, &style, ink, key, m,1.0,false);
            }
            self.label = buffer;
        }
        match node.mark {
            Mark::Chevron(direction) => self.chevron((x + w / 2.0, y + h / 2.0), direction, 1.0, fade(look.text)),
            Mark::Line { width, rising } => {
                let half = width / 2.0;
                let (left, top, right, bottom) = (x + half, y + half, x + w - half, y + h - half);
                let (a, b) = if rising { ((left, bottom), (right, top)) } else { ((left, top), (right, bottom)) };
                self.line_w(a, b, width, fade(look.text));
            }
            Mark::None => {}
        }
        let ring = look.ring;
        if focused && area && (steady.is_some() || (ring.width > 0.0 && ring.color.a > 0)) {
            if round {
                self.push(Instance::rect(corners, [0; 4]).radii(radii).border(ring.width, rgba(fade(ring.color))).sides(ring.sides));
            } else {
                self.bands(placed, ring.width, ring.sides, fade(ring.color));
            }
        }
    }

    /// A scroll node's thumb (`Node::bar`) for box `index` at `r`, where its content and scroll put it.
    /// Whether an overlay thumb on `axis` (0 down, 1 sideways) shows at `offset` (it moved within `SCROLLER_HOLD`), noting the move
    /// and the time it hides.
    fn thumb_shows(&mut self, slot: usize, axis: usize, offset: f32) -> bool {
        if !self.overlay {
            return true;
        }
        let seen = &mut self.thumbs[slot][axis];
        // the offset a node is first painted at (a list restored mid-way, a swipe row at rest) is not a scroll
        if seen.0.is_nan() {
            seen.0 = offset;
        } else if (offset - seen.0).abs() > 0.01 {
            *seen = (offset, self.now);
        }
        let hide = seen.1 + pxui_types::Environment::SCROLLER_HOLD;
        if self.now >= hide {
            return false;
        }
        self.shown.push((slot, hide));
        true
    }

    fn bar(&mut self, r: Rect, index: usize, slot: usize, bar: &Bar) {
        if !self.thumb_shows(slot, 0, self.scroll_y[slot]) {
            return;
        }
        let content = self.l_content[index];
        let (w, gap) = (bar.width, bar.gap);
        let thumb = if bar.whole {
            // a panel's thumb lands on whole points, as its integer layout did
            let (x, y, width, height) = (r.x as i64, r.y as i64, r.w as i64, r.h as i64);
            let (content, margin) = (content as i64, bar.margin as i64);
            let maximum = 0.max(content - height);
            (maximum > 0).then(|| {
                let track_h = 1.max(height - 2 * margin);
                let thumb_h = track_h.min(20.max(track_h * height / 1.max(content)));
                let thumb_y = y + margin + self.scroll_y[slot] as i64 * (track_h - thumb_h) / maximum;
                let x = if self.rtl { (x as f32) + gap } else { (x + width) as f32 - gap - w };
                Rect::new(x, thumb_y as f32, w, thumb_h as f32)
            })
        } else {
            (content > r.h).then(|| {
                let track_h = 1.0f32.max(r.h - 2.0 * bar.margin);
                let thumb_h = track_h.min(20.0f32.max(track_h * r.h / content));
                let thumb_y = r.y + bar.margin + self.scroll_y[slot] * (track_h - thumb_h) / (content - r.h);
                Rect::new(if self.rtl { r.x + gap } else { r.x + r.w - gap - w }, thumb_y, w, thumb_h)
            })
        };
        if let Some(t) = thumb {
            if bar.radius > 0.0 {
                self.push(Instance::rect([t.x, t.y, t.x + t.w, t.y + t.h], rgba(bar.color)).radii([bar.radius; 4]));
            } else {
                self.fill(t.x, t.y, t.w, t.h, bar.color);
            }
        }
    }

    /// A `SCROLL_X` node's thumb along its bottom edge, as `bar` along the right.
    fn bar_x(&mut self, r: Rect, index: usize, slot: usize, bar: &Bar) {
        if !self.thumb_shows(slot, 1, self.scroll_x[slot]) {
            return;
        }
        let content = self.l_content_w[index];
        if content <= r.w {
            return;
        }
        let track_w = 1.0f32.max(r.w - 2.0 * bar.margin);
        let thumb_w = track_w.min(20.0f32.max(track_w * r.w / content));
        let along = bar.margin + self.scroll_x[slot] * (track_w - thumb_w) / (content - r.w);
        let x = if self.rtl { r.x + r.w - along - thumb_w } else { r.x + along };
        let t = Rect::new(x, r.y + r.h - bar.gap - bar.width, thumb_w, bar.width);
        if bar.radius > 0.0 {
            self.push(Instance::rect([t.x, t.y, t.x + t.w, t.y + t.h], rgba(bar.color)).radii([bar.radius; 4]));
        } else {
            self.fill(t.x, t.y, t.w, t.h, bar.color);
        }
    }

    /// Bind `image` for the instances pushed next: the layer breaks to its texture.
    fn bind(&mut self, image: &Image) {
        let layer = &mut self.layers[self.layer];
        let index = match layer.images.iter().position(|other| Rc::ptr_eq(&other.handle, &image.handle)) {
            Some(index) => index,
            None => {
                layer.images.push(image.clone());
                layer.images.len() - 1
            }
        } as u32;
        if layer.breaks.last().map(|b| b.image) != Some(index) {
            layer.breaks.push(Break { at_instance: layer.instances.len() as u32, image: index });
        }
    }

    /// Draw a host-owned image for this frame; the layer breaks to its texture.
    pub fn image(&mut self, x: f32, y: f32, w: f32, h: f32, image: &Image) {
        self.image_fit(x, y, w, h, image, Fit::Fill, Rgba::WHITE);
    }

    /// An image placed in the box as CSS `object-fit` places it, its texels multiplied by `tint`
    /// (white keeps it; see `Instance::image`), filtered linearly when scaled (`flags::image::SMOOTH`).
    pub fn image_fit(&mut self, x: f32, y: f32, w: f32, h: f32, image: &Image, fit: Fit, tint: Rgba) {
        if w > 0.0 && h > 0.0 {
            self.bind(image);
            let (rect, uv) = crate::draw::fit(fit, [image.width as f32, image.height as f32], [x, y, x + w, y + h], [0.0, 0.0, 1.0, 1.0]);
            self.push(Instance { flags: crate::draw::flags::image::SMOOTH, ..Instance::image(rect, uv, rgba(tint)) });
        }
    }

    /// An image as a nine-slice frame (CSS `border-image`): corners `inset` texels in (`[left, top,
    /// right, bottom]`) stay 1:1, edges and centre stretch over the box.
    pub fn image_nine(&mut self, x: f32, y: f32, w: f32, h: f32, image: &Image, inset: [f32; 4], tint: Rgba) {
        if w > 0.0 && h > 0.0 {
            self.bind(image);
            for i in crate::draw::nine_slice([x, y, x + w, y + h], [0.0, 0.0, 1.0, 1.0], [image.width as f32, image.height as f32], inset, rgba(tint)) {
                self.push(Instance { flags: crate::draw::flags::image::SMOOTH, ..i });
            }
        }
    }

    /// One glyph of font `font` (an icon font loaded into the UI's `Text`) at `size` points, its top-left
    /// at `at`, in `color`: its advance.
    pub fn icon(&mut self, at: (f32, f32), font: u16, size: f32, codepoint: char, color: Rgba) -> f32 {
        let mut buffer = [0u8; 4];
        let style = Style { font, size, ..Style::default() };
        self.text_em(at, &style, 0.0, Some(color), codepoint.encode_utf8(&mut buffer)).w
    }

    /// Kit text with its top-left corner at `at`. Without `size` it uses the UI's own font; with
    /// `size` the kit face at that size. `tracking` adds points after every glyph.
    pub fn text_with(&mut self, at: (f32, f32), size: Option<i64>, tracking: f32, color: Option<Rgba>, text: &str) {
        if !text.is_empty() {
            let style = kit_style(size.unwrap_or(self.font_size), tracking);
            self.text_em(at, &style, 0.0, color, text);
        }
    }

    /// Text in any style (wrap, ellipsis, line limit, alignment, font) within `width` points (0:
    /// unbounded), its layout's top-left corner at `at`: the first baseline sits `Metrics::baseline` below `at`,
    /// on a device pixel, whatever the line height. Mask glyphs take `color` (the theme's foreground by default);
    /// colour glyphs keep their own colours under its alpha.
    pub fn text_styled(&mut self, at: (f32, f32), style: &Style, width: f32, color: Option<Rgba>, text: &str) -> Metrics {
        let (key, m) = lay_out(&mut self.text, style, width, text);
        self.text_laid(at, m.baseline, style, color, key, m,1.0,false)
    }

    /// `text_styled` placed by its em box: the first baseline sits the font's ascent below `at` (kit text and icons).
    fn text_em(&mut self, at: (f32, f32), style: &Style, width: f32, color: Option<Rgba>, text: &str) -> Metrics {
        let (key, m) = lay_out(&mut self.text, style, width, text);
        let below = self.text.ascent(style.font) * style.size;
        self.text_laid(at, below, style, color, key, m,1.0,false)
    }

    /// `text_styled` with styled byte ranges; `None` when the spans are invalid.
    pub fn text_spans(&mut self,at:(f32,f32),style:&Style,width:f32,color:Option<Rgba>,text:&str,spans:&[TextSpan])->Option<Metrics> {
        let (key,metrics)=lay_out_spans(&mut self.text,style,width,text,spans)?;
        Some(self.text_laid(at,metrics.baseline,style,color,key,metrics,1.0,!spans.is_empty()))
    }

    /// Draws a layout already made (`lay_out`'s key and metrics) with its first baseline `below` points under `at.1`, on a device pixel:
    /// the font's ascent for text placed by its em box (`text_em`, node text), `Metrics::baseline` for text placed by its layout top.
    fn text_laid(&mut self, at: (f32, f32), below: f32, style: &Style, color: Option<Rgba>, key: u64, m: Metrics, opacity:f32, decorated:bool) -> Metrics {
        let d = self.density as f32;
        let snap = |v: f32| (v * d).round() / d;
        let baseline = snap(at.1 + below);
        let left = snap(at.0);
        let origin = (left, if decorated { baseline - self.text.glyph_baseline(key) } else { 0.0 });
        let (clip, view) = (clip_of(self.clip_rect), self.view);
        let Paint { text: t, layers, layer, glyph_light_memo, .. } = self;
        let layer = &mut layers[*layer];
        let fade=move |mut c:[u8;4]| { if opacity<1.0 { c[3]=(c[3] as f32*opacity).round() as u8; } c };
        let color = fade(rgba(color.unwrap_or(self.theme.foreground)));
        let decoration=|deco:pxui_text::Decoration,layer:&mut Layer| {
            let r=deco.rect;
            layer.instances.push(Instance { clip,view,..Instance::rect([origin.0+r.x,origin.1+r.y,origin.0+r.x+r.w,origin.1+r.y+r.h],fade(deco.color)) });
        };
        if decorated { t.decorations(key,false,&mut |deco|decoration(deco,layer)); }
        let (mask_flags, light) = {
            let ppem = style.size * d;
            let key = [ppem.to_bits(), d.to_bits()];
            let [older, recent] = *glyph_light_memo;
            let extra = if recent.0 == key { recent.1 } else if older.0 == key { older.1 } else {
                let extra = Instance::light_extra(ppem, d);
                *glyph_light_memo = [recent, (key, extra)];
                extra
            };
            (crate::draw::flags::mask::LIGHT, extra)
        };
        let glyph_paint = pxui_text::GlyphPaint { clip, view, color, opacity, mask_flags, light };
        t.draw_at_baseline(key, (left, baseline), self.density, glyph_paint, &mut layer.instances, &mut layer.breaks);
        if decorated { t.decorations(key,true,&mut |deco|decoration(deco,layer)); }
        m
    }

    pub fn text(&mut self, at: (f32, f32), text: &str) {
        self.text_with(at, None, 0.0, None, text);
    }

    pub fn text_c(&mut self, at: (f32, f32), color: Rgba, text: &str) {
        self.text_with(at, None, 0.0, Some(color), text);
    }

    pub fn text_s(&mut self, at: (f32, f32), size: i64, color: Rgba, text: &str) {
        self.text_with(at, Some(size), 0.0, Some(color), text);
    }

    pub fn text_width(&mut self, size: Option<i64>, text: &str) -> f32 {
        let size = size.unwrap_or(self.font_size);
        lay_out(&mut self.text, &kit_style(size, 0.0), 0.0, text).1.w
    }

    /// See `Ui::text_metrics`.
    pub fn text_metrics(&mut self, style: &Style, width: f32, text: &str) -> Metrics {
        lay_out(&mut self.text, style, width, text).1
    }

    /// The x of the caret before byte `index` of kit text, in points from the text's start.
    pub fn caret_x(&mut self, size: Option<i64>, text: &str, index: usize) -> f32 {
        let size = size.unwrap_or(self.font_size);
        caret_x(&mut self.text, size, text, index)
    }

    /// The highlight of bytes `a..b` of single-line kit text: one span per visual run (two or
    /// more across a direction change), as `(x, w)` in points from the text's start.
    pub fn selection(&mut self, size: Option<i64>, text: &str, a: usize, b: usize, f: &mut dyn FnMut(f32, f32)) {
        let size = size.unwrap_or(self.font_size);
        let (key, _) = lay_out(&mut self.text, &kit_style(size, 0.0), 0.0, text);
        self.text.selection(key, a, b, &mut |r| f(r.x, r.w));
    }

    pub(crate) fn selection_fill(&mut self, size: Option<i64>, text: &str, range: (usize,usize), at: (f32,f32), height:f32, color:Rgba) {
        let (key, _) = lay_out(&mut self.text, &kit_style(size.unwrap_or(self.font_size),0.0),0.0,text);
        let (clip,clip_radius,view)=self.stamp();
        let density=self.density as f32;
        let snap=|value:f32|(value*density-0.5).ceil()/density;
        let (top,bottom)=(snap(at.1),snap(at.1+height));
        let instances=&mut self.layers[self.layer].instances;
        self.text.selection(key,range.0,range.1,&mut |r| {
            let (left,right)=(snap(at.0+r.x),snap(at.0+r.x+r.w));
            if right>left && bottom>top {
                instances.push(Instance {clip,clip_radius,view,..Instance::rect([left,top,right,bottom],rgba(color))});
            }
        });
    }

    /// Where the top of kit text of `size` goes to sit centred in a row at `y` of height `h`.
    pub fn text_top(&self, size: Option<i64>, y: f32, h: f32) -> f32 {
        text_top_for(self.font_size, size, y, h)
    }

    /// The label style: upper case, two points under the body, 0.08 em of tracking.
    pub fn label_size(&self) -> i64 {
        8.max(self.font_size - 2)
    }

    pub fn cap_tracking(&self) -> f32 {
        0.08 * self.label_size() as f32
    }

    /// A label: upper case at `label_size` with 0.08 em of tracking, ink-2 by default.
    pub fn cap(&mut self, at: (f32, f32), color: Option<Rgba>, label: &str) {
        let color = color.unwrap_or(self.theme.ink_2());
        let size = self.label_size();
        let tracking = self.cap_tracking();
        let mut buffer = std::mem::take(&mut self.label);
        buffer.clear();
        buffer.extend(label.chars().map(|c| c.to_ascii_uppercase()));
        self.text_with(at, Some(size), tracking, Some(color), &buffer);
        self.label = buffer;
    }

    /// The tracking follows every letter, the last too (CSS letter-spacing): text after a label
    /// starts one step later, and a right-aligned label ends one step early.
    pub fn cap_width(&mut self, label: &str) -> f32 {
        let mut buffer = std::mem::take(&mut self.label);
        buffer.clear();
        buffer.extend(label.chars().map(|c| c.to_ascii_uppercase()));
        let style = kit_style(self.label_size(), self.cap_tracking());
        let width = lay_out(&mut self.text, &style, 0.0, &buffer).1.w;
        self.label = buffer;
        width
    }

    /// The ratio of a splitter being dragged, `first / second`, as an accent label with no box.
    pub fn ratio(&mut self, at: (f32, f32), first: i64, second: i64) {
        let accent = self.theme.accent;
        self.cap(at, Some(accent), &format!("{} / {}", first, second));
    }

    /// The focused text's input area, for the input method; the first one painted wins.
    pub fn input_region(&mut self, cursor: f32, x: f32, y: f32, w: f32, h: f32) {
        if self.ime.is_none() {
            self.ime = Some(((x as i64, y as i64, w as i64, h as i64), 0.max(cursor.round() as i64)));
        }
    }
}

impl Paint {
    /// What `push` stamps on an instance: its clip, clip radius (f16 bits) and view table index.
    pub fn stamp(&self) -> ([i16; 4], u16, u16) {
        (clip_of(self.clip_rect), if self.clip_radius > 0.0 { crate::draw::f16(self.clip_radius) } else { 0 }, self.view)
    }

    /// The current layer's instances. The C ABI hands their spare capacity out as a `Span` the caller writes shapes into,
    /// and commits what was written with `set_len`.
    pub fn instances_mut(&mut self) -> &mut Vec<Instance> {
        &mut self.layers[self.layer].instances
    }
}

impl Shapes for Paint {
    /// Append `instance` under the current clip and view.
    fn push(&mut self, instance: Instance) {
        let (clip, clip_radius, view) = self.stamp();
        self.layers[self.layer].instances.push(Instance { clip, clip_radius, view, ..instance });
    }
    fn density(&self) -> i64 {
        self.density
    }
    fn theme(&self) -> Tokens {
        self.theme
    }
    fn clip_rect(&self) -> Rect {
        self.clip_rect
    }
    fn set_clip_rect(&mut self, clip: Rect) {
        self.clip_rect = clip;
    }
}

impl Ui {
    fn record_hit(&mut self, index: usize, parent: i64, rect: Rect) {
        let Rect { x, y, w, h } = rect;
        // a drop zone that is no input node holds the pointer's hit as a plain blocking area, so a drop reaches it
        let flags = self.nd(index).flags;
        let zone = self.drop_zones == self.frame_number && self.slots[self.boxes[index].slot].drop_zone == self.frame_number && !flags.any(HIT_FLAGS);
        let rec = HitRec { key: self.boxes[index].key, parent, flags: if zone { flags.or(Flags::BLOCKING) } else { flags }, x, y, w, h, view: self.paint_xf };
        self.push_hit(rec);
    }

    fn push_hit(&mut self, rec: HitRec) {
        let position = self.hit_count;
        if position >= self.hits.len() {
            self.hits.push(rec);
        } else {
            self.hits[position] = rec;
        }
        self.chain_key = 0;
        self.hit_count = position + 1;
    }

    fn run_painters(&mut self, first: u32, slot: usize, rect: Rect, clip_rect: Rect, paint: &mut Paint) {
        if first == arena::END { return; }
        paint.clip_rect = clip_rect;
        let view = self.slots[slot].owner as usize;
        let capture = self.views[view].palette_paint;
        paint.capture_palette = capture;
        let start = paint.layers[paint.layer].instances.len();
        self.arena.run(first, paint, rect);
        paint.capture_palette = false;
        if !capture { return; }
        self.record_palette_shapes(paint, view, start);
    }

    #[cold]
    fn record_palette_shapes(&mut self, paint: &mut Paint, view: usize, start: usize) {
        let instances = &mut paint.layers[paint.layer].instances;
        for (at, instance) in instances.iter_mut().enumerate().skip(start) {
            let [r, g, b, a] = instance.color;
            let fill = Rgba { r, g, b, a };
            let [r, g, b, a] = instance.color2;
            let stroke = Rgba { r, g, b, a };
            if fill.as_token().is_some() || stroke.as_token().is_some() {
                instance.color = rgba(palette_color(&paint.colors, paint.theme.accent, fill));
                instance.color2 = rgba(palette_color(&paint.colors, paint.theme.accent, stroke));
                let v = &mut self.views[view];
                if !v.palette_paint { continue; }
                let at = at as u32;
                let count = v.palette_shape_count as usize;
                let shapes = &mut self.palette_shapes[view];
                if let Some(previous) = shapes[..count].last_mut()
                    && previous.end == at && at > start as u32 && previous.fill == fill && previous.stroke == stroke
                {
                    previous.end += 1;
                } else if count < PALETTE_SHAPES {
                    shapes[count] = PaletteShape { at, end: at + 1, fill, stroke };
                    v.palette_shape_count += 1;
                } else {
                    v.palette_paint = false;
                    v.palette_shape_count = 0;
                }
            }
        }
    }

    /// The node marks its own focus (`FOCUS_MARK`) and holds it: its look's ring shows.
    fn marks_focus(&self, slot: usize) -> bool {
        let s = &self.slots[slot];
        s.key != 0 && s.key == self.focus && s.node.flags.any(Flags::FOCUS_MARK)
    }

    /// Paint the node's look and text, recording the instance range a patch rewrites.
    #[inline(never)]
    fn paint_node(&mut self, paint: &mut Paint, index: usize, r: Rect, clip_rect: Rect) {
        paint.clip_rect = clip_rect;
        let slot = self.boxes[index].slot;
        let start = paint.layers[paint.layer].instances.len() as u32;
        let s = &self.slots[slot];
        // an ahead row's node out of view (a view row paints wherever it is) leaves unseen text to the prepare queue
        paint.ahead = self.boxes[index].ahead && { let seen = r.intersect(clip_rect); seen.w <= 0.0 || seen.h <= 0.0 };
        let deferred = paint.ahead.then(|| paint.text.queue_counts().0);
        if s.node.template == NodeTemplate::PositionedLabel
            && s.tween == NO_TWEEN && s.pose == (0.0, 0.0, 1.0)
            && self.swap_frame != self.frame_number
            && let Some(rich) = s.rich.as_ref().filter(|rich| rich.line_box && rich.spans.len == 0)
        {
            let style = rich.style(&s.node, paint.rtl);
            if let Some((key, metrics)) = lay_out_node(
                &mut paint.text, &s.text_layout, Flags::FIT_TEXT_PIXEL, &style, r.w, &s.text, &[], paint.ahead,
            ) {
                let below = paint.text.ascent(style.font) * style.size;
                let top = rich.line_box_baseline(r.y, r.h, metrics) - below;
                paint.text_laid((r.x, top), below, &style, Some(s.node.look.text), key, metrics, 1.0, false);
            }
        } else if s.node.template != NodeTemplate::View || s.tween != NO_TWEEN
            || self.swap_frame == self.frame_number || !s.text.is_empty() || s.rich.is_some()
        {
            self.paint_node_look(paint, slot, r);
        }
        // its kept output would lack that text: the view builds again
        if deferred.is_some_and(|before| paint.text.queue_counts().0 != before) { self.mark_view(self.slots[slot].owner); }
        paint.ahead = false;
        let s = &mut self.slots[slot];
        if self.motion {
            (s.seen, s.seen_state) = (s.node.look, s.node.state);
        } else {
            s.look_needs_snapshot = true;
        }
        // scratch positions until `settle` places them
        s.range = (start, paint.layers[paint.layer].instances.len() as u32);
        s.room = 0;
        s.clip = clip_rect;
        s.clip_radius = paint.clip_radius;
        s.painted = self.frame_number;
        self.painted_nodes.push((slot, paint.layer));
    }

    #[inline(never)]
    fn paint_node_look(&mut self, paint: &mut Paint, slot: usize, r: Rect) {
        // a theme swap moves every node shown before it from the look it showed
        let s = &self.slots[slot];
        let swap = self.swap_frame == self.frame_number && s.painted >= 0 && s.seen != s.node.look;
        let swap = swap.then(|| Curve::Bezier { duration: self.theme.tokens.motion.normal, points: self.theme.tokens.motion.ease });
        // a node's transition (its own, or the theme's pick), out of line: most nodes carry none
        let ran = self.slots[slot].node.transition.is_some() && self.declared_transition(slot);
        if let Some(curve) = swap.filter(|_| !ran && !self.system_reduced_motion) {
            self.tween_look(slot, curve);
        }
        // A declaration wins over an animated look.
        let animated = self.tween_of(slot).map(|t| {
            self.tweens[t].to.look = self.slots[slot].node.look;
            self.pose_at(slot, self.verify_time(slot))
        });
        let focused = self.marks_focus(slot);
        let s = &self.slots[slot];
        let (look, dx, dy, opacity, steady) = match &animated {
            Some((p, steady)) => (&p.look, p.dx, p.dy, p.opacity, *steady),
            None => (&s.node.look, s.pose.0, s.pose.1, s.pose.2, None),
        };
        let (dx, dy, opacity) = if s.view != NO_VIEW && self.views[s.view as usize].index != 0 {
            (0.0, 0.0, 1.0)
        } else { (dx, dy, opacity) };
        let faded;
        let look = if opacity == 1.0 { look } else {
            faded = Look { opacity: look.opacity * opacity, ..*look };
            &faded
        };
        if s.node.template != NodeTemplate::View || animated.is_some() || !s.text.is_empty() || s.rich.is_some() {
            paint.look(Rect::new(r.x + dx, r.y + dy, r.w, r.h), &s.node, look, &s.text, s.rich.as_ref(), &self.span_arena, &s.text_layout, focused, steady);
        }
    }

    #[inline(never)]
    fn declared_transition(&mut self, slot: usize) -> bool {
        let transition = core::mem::take(&mut self.slots[slot].node.transition);
        transition.get().is_some_and(|t| self.node_transition(slot, t))
    }

    /// Runs a node's transition (its own, or the theme's pick) when its state changed since it was last painted and motion is not
    /// reduced, and tracks the node's painted look and state while no theme moves (paint does while one does). Out of line so the
    /// paint of a node without one keeps its shape (abi_bench).
    #[inline(never)]
    fn node_transition(&mut self, slot: usize, transition: Transition) -> bool {
        let s = &self.slots[slot];
        let run = s.painted >= 0 && s.seen_state != s.node.state && !self.theme.tokens.reduced_motion && !self.system_reduced_motion;
        if run { self.tween_look(slot, transition.curve()); }
        if !self.motion {
            let s = &mut self.slots[slot];
            (s.seen, s.seen_state) = (s.node.look, s.node.state);
        }
        run
    }

    /// Starts the node's look moving from what it shows (its running tween's pose, or the look last painted) to its declared look.
    fn tween_look(&mut self, slot: usize, curve: Curve) {
        let from = match self.tween_of(slot) {
            Some(_) => self.pose(slot).0,
            None => Pose { look: self.slots[slot].seen, ..self.rest(slot) },
        };
        let to = Pose { look: self.slots[slot].node.look, ..self.rest(slot) };
        self.start_tween(slot, curve, from, to);
    }

    fn visit(&mut self, paint: &mut Paint, index: usize, clip_rect: Rect, parent_hit: i64) {
        if self.boxes[index].parent == 0 {
            let floating = self.foreground.iter().map(|f| (f.1, f.2)).chain(self.overlays.iter().copied()).find(|f| f.0 == index);
            if let Some((_, role)) = floating {
                let anchor = self.nd(index).anchor.get().map_or_else(|| self.screen(index), |a| a.rect);
                if paint.layer + 1 < pxui_types::MAX_LAYERS {
                    paint.next_layer(self.boxes[index].key, role, anchor);
                } else {
                    // past the renderer's layer limit the remaining floating roots paint into the last layer: `Merged`, anchored at
                    // the union of its roots' anchors
                    let last = &mut paint.layers[paint.layer];
                    (last.role, last.anchor) = (LayerRole::Merged, union(Some(last.anchor), anchor));
                }
            }
        }
        let screen_rect = self.screen(index);
        let slot = self.boxes[index].slot;
        let Rect { x, y, w, h } = screen_rect;
        {
            let s = &mut self.slots[slot];
            s.rx = x;
            s.ry = y;
            s.rw = w;
            s.rh = h;
        }
        let b = &self.boxes[index];
        let flags = self.slots[slot].node.flags;
        let drop_hit = self.drop_zones == self.frame_number && self.slots[slot].drop_zone == self.frame_number;
        let interactive = flags.any(HIT_FLAGS) || self.gestures[slot].kinds != 0 || drop_hit;
        let focus_ring = self.keyboard_focus && self.focus == b.key && !flags.any(Flags::FOCUS_MARK);
        if b.first == NONE && b.view == NO_VIEW && !interactive && !focus_ring
            && b.hit_rect.is_none() && b.hit.is_none() && matches!(b.shape, HitShape::Own)
            && b.paint == arena::EMPTY && b.over == arena::EMPTY && self.slots[slot].node.bar.is_none()
        {
            let clipped = screen_rect.intersect(clip_rect);
            if clipped.w > 0.0 && clipped.h > 0.0 {
                self.paint_node(paint, index, screen_rect, clip_rect);
            } else if (w > 0.0 && h > 0.0) || flags.any(Flags::CLIP) {
                self.slots[slot].clip = clip_rect;
                if self.boxes[index].ahead {
                    self.defer_culled(paint, index);
                }
            }
            return;
        }
        let shape = b.shape;
        let hit = self.boxes[index].hit_rect.unwrap_or_else(|| match (&self.boxes[index].hit, shape) {
            (Some(f), _) => f(screen_rect),
            (None, HitShape::Part(p)) => Rect::new(x + p.x, y + p.y, p.w, p.h),
            (None, HitShape::Outset(e)) => Rect::new(x - e.l, y - e.t, w + e.l + e.r, h + e.t + e.b),
            (None, HitShape::Own) => screen_rect,
        });
        let clipped_hit = hit.intersect(clip_rect);
        let Rect { w: hw, h: hh, .. } = clipped_hit;
        let visible = match (self.boxes[index].hit_rect, &self.boxes[index].hit, shape) {
            (None, None, HitShape::Own) => hw > 0.0 && hh > 0.0,
            _ => {
                let Rect { w: vw, h: vh, .. } = screen_rect.intersect(clip_rect);
                vw > 0.0 && vh > 0.0
            }
        };
        // a view box is painted wherever it is, so its output can be kept and scrolled into view; a virtual list's row box that holds
        // its row's view is culled as any row (past the viewport its text goes to the prepare queue, its view keeps its build)
        let view = self.boxes[index].view;
        let visible = visible || (view != NO_VIEW && w > 0.0 && h > 0.0 && self.slots[slot].row_of.0 == 0);
        // a gesture node is findable by touch arbitration without being interactive itself
        let has_hit = interactive && hw > 0.0 && hh > 0.0;
        if has_hit {
            self.record_hit(index, parent_hit, clipped_hit);
        }
        // a box with no extent of its own (a `Fit` box whose children are all placed with `at`) is
        // not out of view: its children show unless it clips them
        let shown = visible;
        if shown || ((w <= 0.0 || h <= 0.0) && !flags.any(Flags::CLIP)) {
            if shown {
                self.paint_node(paint, index, screen_rect, clip_rect);
                self.run_painters(self.boxes[index].paint.0, slot, screen_rect, clip_rect, paint);
            }
            let child_clip = if flags.any(Flags::CLIP) {
                let p = self.nd(index).padding;
                clip_rect.intersect(Rect::new(x + p.l, y + p.t, 0.0f32.max(w - p.l - p.r), 0.0f32.max(h - p.t - p.b)))
            } else {
                clip_rect
            };
            let parent_hit = if has_hit { self.boxes[index].key } else { parent_hit };
            // a `ROUND_CLIP` node's corner radius rounds its children's clip; a square clip inside a
            // rounded one drops the rounding
            // one radius (the largest corner's) for the clip: the instance's `clip_radius` is its one
            // f16 in the 64-byte budget (specification/pxui.md, the designed limit); a cached view
            // inside takes its outer clip square from the view table
            let outer_radius = paint.clip_radius;
            if flags.any(Flags::CLIP) {
                let r = self.nd(index).look.radius;
                paint.clip_radius = if flags.any(Flags::ROUND_CLIP) { r[0].max(r[1]).max(r[2]).max(r[3]) } else { 0.0 };
            }
            if view != NO_VIEW {
                self.visit_view(paint, index, view, screen_rect, child_clip, parent_hit);
            } else {
                let mut child = self.boxes[index].first;
                if child != NONE && self.slots[slot].holds_rows {
                    // a virtual list's rows: the text each lays out belongs to its row (`Text::list_row`)
                    while child != NONE {
                        let (list, row) = self.slots[self.boxes[child].slot].row_of;
                        if list != 0 { paint.text.list_row(list as u64, row as usize); } else { paint.text.list_end(); }
                        self.visit(paint, child, child_clip, parent_hit);
                        child = self.boxes[child].next;
                    }
                    paint.text.list_end();
                }
                while child != NONE {
                    self.visit(paint, child, child_clip, parent_hit);
                    child = self.boxes[child].next;
                }
            }
            paint.clip_radius = outer_radius;
            if shown {
                self.run_painters(self.boxes[index].over.0, slot, screen_rect, clip_rect, paint);
                if let Some(bar) = self.nd(index).bar.get() {
                    paint.clip_rect = clip_rect;
                    if flags.any(Flags::SCROLL) || !flags.any(Flags::SCROLL_X) {
                        paint.bar(screen_rect, index, slot, &bar);
                    }
                    if flags.any(Flags::SCROLL_X) {
                        paint.bar_x(screen_rect, index, slot, &bar);
                    }
                }
            }
            if shown && focus_ring {
                // a node that marks its own focus draws its look's ring; any other gets the theme's
                paint.clip_rect = clip_rect;
                let ring = self.theme.ring;
                let r = Rect::new(self.l_x[index], self.l_y[index], 0.0f32.max(self.l_w[index]), 0.0f32.max(self.l_h[index]));
                if ring.width > 0.0 && ring.color.a > 0 {
                    paint.bands(r, ring.width, ring.sides, ring.color);
                }
            }
        } else {
            // Culled: keep retained rectangles current for `rect` queries.
            self.retain_rects(index, clip_rect);
            if self.boxes[index].ahead {
                self.defer_culled(paint, index);
            }
        }
    }

    /// A culled box of a list row past the viewport and its subtree: the text each would draw, unseen, goes to the prepare
    /// queue (`Paint::defer_text`), ready when the row scrolls into view. Its view, if any, builds again once it has arrived.
    fn defer_culled(&mut self, paint: &mut Paint, index: usize) {
        let slot = self.boxes[index].slot;
        let before = paint.text.queue_counts().0;
        let s = &self.slots[slot];
        paint.defer_text(s.rw, &s.node, &s.text, s.rich.as_ref(), &self.span_arena, &s.text_layout);
        if paint.text.queue_counts().0 != before {
            self.mark_view(self.slots[slot].owner);
        }
        let mut child = self.boxes[index].first;
        while child != NONE {
            self.defer_culled(paint, child);
            child = self.boxes[child].next;
        }
    }

    fn retain_rects(&mut self, index: usize, clip: Rect) {
        let rect = self.screen(index);
        let slot = self.boxes[index].slot;
        let s = &mut self.slots[slot];
        (s.rx, s.ry, s.rw, s.rh, s.clip) = (rect.x, rect.y, rect.w, rect.h, clip);
        let n = self.nd(index);
        let child_clip = if n.flags.any(Flags::CLIP) {
            let p = n.padding;
            clip.intersect(Rect::new(rect.x + p.l, rect.y + p.t, (rect.w - p.l - p.r).max(0.0), (rect.h - p.t - p.b).max(0.0)))
        } else { clip };
        let v = self.boxes[index].view;
        if v != NO_VIEW {
            let r = &mut self.views[v as usize];
            r.box_clip = child_clip;
            if self.boxes[index].kept {
                r.delta = (rect.x - r.built_rect.x, rect.y - r.built_rect.y);
                let resized = rect.w != r.built_rect.w || rect.h != r.built_rect.h;
                r.reflow |= resized;
                if resized || (r.index == 0 && r.delta != (0.0, 0.0)) {
                    self.mark_view(v);
                    self.frame_requested = true;
                    self.settle = true;
                }
            } else {
                r.built_rect = rect;
                r.delta = (0.0, 0.0);
            }
        }
        let mut child = self.boxes[index].first;
        while child != NONE {
            self.retain_rects(child, child_clip);
            child = self.boxes[child].next;
        }
    }

    fn paint_all(&mut self, frame: &rframe::Frame) {
        assert!(!self.painting, "Ui: painting re-entered");
        self.painting = true;
        self.hit_count = 0;
        self.chain_key = 0;
        self.paint_xf = 0;
        let clip = Rect::new(0.0, 0.0, frame.input.size.0 as f32, frame.input.size.1 as f32);
        // a frame reports its own damage, plus what changed between frames (patches, steps) and was not drawn yet:
        // the build re-declares a patched look, so its own comparison would not see that the screen still lacks it
        let between = if self.damage != self.frame_damage { self.damage } else { DamageList::default() };
        self.damage = if self.full_damage { DamageList::full(clip) } else { between };
        let scratch = std::mem::take(&mut self.spare);
        let mut paint = self.paint_begin(scratch, clip);
        self.painted = self.frame_number;
        self.views[0].visited = self.painted;
        self.visit(&mut paint, 0, clip, 0);
        self.out.ime = paint.ime;
        self.edit_scroll_x = paint.edit_scroll_x;
        let len = paint.layer + 1;
        let mut layers = self.paint_end(paint);
        if self.verify {
            self.verify_runs(&layers);
        }
        let mut changed = self.refresh_views();
        changed |= self.place_all(&mut layers, len);
        self.settle(&layers);
        self.collect_blurs();
        if changed {
            self.draw_revision += 1;
        }
        self.spare = layers;
        self.painting = false;
    }

    /// A painter over `layers` (its first layer cleared) with the text engine and the layout
    /// arrays lent to it; `paint_end` takes them back.
    fn paint_begin(&mut self, layers: Vec<Layer>, clip: Rect) -> Paint {
        let mut paint = Paint {
            layers,
            // the base layer is the one after none
            layer: usize::MAX,
            view: 0,
            clip_rect: clip,
            clip_radius: 0.0,
            capture_palette: false,
            theme: self.theme.tokens,
            font_size: self.font_size,
            row_height: self.kit_row_height,
            density: self.density,
            colors: self.theme_colors.unwrap_or(self.colors[self.theme.tokens.is_dark() as usize]),
            label_look: self.theme.label.idle,
            text: self.text.take().expect("Ui: painting re-entered"),
            glyph_light_memo: [([0; 2], [0; 2]); 2],
            ime: None,
            label: std::mem::take(&mut self.label),
            edit_text: std::mem::take(&mut self.edit_work),
            composition: std::mem::take(&mut self.composition),
            edit_scroll_x: self.edit_scroll_x,
            composition_selection: self.composition_selection,
            l_content: std::mem::take(&mut self.l_content),
            scroll_y: std::mem::take(&mut self.scroll_y),
            l_content_w: std::mem::take(&mut self.l_content_w),
            scroll_x: std::mem::take(&mut self.scroll_xs),
            rtl: self.dir == Direction::Rtl,
            overlay: self.environment.scrollers == pxui_types::Environment::OVERLAY_SCROLLERS,
            now: self.now,
            thumbs: std::mem::take(&mut self.thumbs),
            shown: std::mem::take(&mut self.thumb_shown),
            ahead: false,
        };
        paint.next_layer(0, LayerRole::Base, Rect::default());
        paint
    }

    fn paint_end(&mut self, paint: Paint) -> Vec<Layer> {
        let Paint { layers, text, label, edit_text, composition, l_content, scroll_y, l_content_w, scroll_x, thumbs, mut shown, .. } = paint;
        self.thumbs = thumbs;
        // a shown overlay thumb's view repaints when it hides (`hide_thumbs`), the frame waking then
        for &(slot, hide) in &shown {
            match self.thumb_hides.iter_mut().find(|entry| entry.0 == slot) {
                Some(entry) => entry.1 = hide,
                None => self.thumb_hides.push((slot, hide)),
            }
            self.wake = self.wake.min(hide);
        }
        shown.clear();
        self.thumb_shown = shown;
        self.text = Some(text);
        self.label = label;
        self.edit_work = edit_text;
        self.composition = composition;
        self.l_content = l_content;
        self.scroll_y = scroll_y;
        self.l_content_w = l_content_w;
        self.scroll_xs = scroll_x;
        layers
    }

    // ----------------------------------------------------- direct patching

    /// Repaint one node's look and text over its range of the last frame's instances: in place when
    /// the count is unchanged, else the layer's later instances, node ranges and breaks shift.
    /// While building nothing is rewritten: the frame's paint emits the node anyway.
    fn repaint_node(&mut self, slot: usize) -> bool {
        if self.building {
            // the record holds the patch: a view built this frame paints it; a kept view's node is
            // rewritten after the paint (whether it is on screen is known then: a view kept inside
            // a view kept this frame is not yet). Either view's next check (`verify_views`) is off.
            let owner = self.slots[slot].owner as usize;
            self.views[owner].patched = true;
            if !self.repatch.contains(&slot) {
                self.repatch.push(slot);
            }
            return true;
        }
        if !self.range_valid(slot) {
            // not on screen: the record holds the patch
            return true;
        }
        let scratch = std::mem::take(&mut self.patch_layers);
        let mut paint = self.paint_begin(scratch, self.slots[slot].clip);
        let changed = self.repaint_with(slot, &mut paint);
        self.patch_layers = self.paint_end(paint);
        changed
    }

    /// Repaint a node with a valid instance range through an animation step's shared scratch painter.
    fn repaint_with(&mut self, slot: usize, paint: &mut Paint) -> bool {
        let (key, (start, end)) = (self.slots[slot].range_layer, self.slots[slot].range);
        let Some(layer) = self.draw[..self.draw_len].iter().position(|l| l.key == key) else { return true };
        let (start, end) = (start as usize, end as usize);
        let owner = self.slots[slot].owner;
        paint.layer = usize::MAX;
        paint.next_layer(0, LayerRole::Base, Rect::default());
        paint.clip_rect = self.slots[slot].clip;
        paint.clip_radius = self.slots[slot].clip_radius;
        paint.view = self.views[self.views[owner as usize].xf as usize].index;
        let (pose, steady) = self.pose(slot);
        let focused = self.marks_focus(slot);
        let s = &self.slots[slot];
        let v = s.view;
        let pose = if v != NO_VIEW && self.views[v as usize].index != 0 {
            Pose { dx: 0.0, dy: 0.0, opacity: 1.0, ..pose }
        } else { pose };
        let look = Look { opacity: pose.look.opacity * pose.opacity, ..pose.look };
        let outer = (!s.text.is_empty() || s.rich.is_some()).then(|| {
            let outer = paint.text.marks();
            paint.text.set_marks([0; 8]);
            outer
        });
        paint.look(Rect::new(s.rx + pose.dx, s.ry + pose.dy, s.rw, s.rh), &s.node, &look, &s.text, s.rich.as_ref(), &self.span_arena, &s.text_layout, focused, steady);
        if let Some(outer) = outer {
            let shelves = paint.text.marks();
            paint.text.set_marks(outer);
            // the patched instances sit in the runs of its view and the views around it
            let mut v = owner;
            while v != 0 {
                let r = &mut self.views[v as usize];
                r.shelves = std::array::from_fn(|k| r.shelves[k] | shelves[k]);
                v = r.parent;
            }
        }
        let fresh = &paint.layers[0].instances;
        // a colour glyph or an image in the patch, or a break inside the node: the layer's breaks are
        // rewritten around the node now (`patch_breaks`)
        let rebreak = fresh.iter().any(|i| i.kind == crate::draw::kind::IMAGE)
            || !paint.layers[0].breaks.is_empty()
            || self.draw[layer].breaks.iter().any(|b| start <= b.at_instance as usize && (b.at_instance as usize) < end);
        let mut target = std::mem::take(&mut self.draw[layer]);
        let room = self.slots[slot].room as usize;
        let (old_len, old_end) = (target.instances.len(), end + room);
        let mut old_breaks = std::mem::take(&mut self.temp_breaks);
        old_breaks.clear();
        if rebreak {
            old_breaks.extend_from_slice(&target.breaks);
        }
        if fresh.len() <= end - start + room {
            // in place, growing into the room after it; a shorter patch leaves empty instances in the
            // rest of the range
            let grown = end.max(start + fresh.len());
            let s = &mut self.slots[slot];
            (s.range.1, s.room) = (grown as u32, (room - (grown - end)) as u32);
            let end = grown;
            for k in start..end {
                let new = fresh.get(k - start).copied().unwrap_or(EMPTY);
                if target.instances[k] != new {
                    let old = target.instances[k];
                    self.damage_change(&old, &new);
                    target.instances[k] = new;
                    target.bounds[k] = cull_bounds(&new);
                    add_range(&mut target.dirty, k as u32, k as u32 + 1);
                }
            }
        } else {
            // past its room: the layer's tail shifts by the growth plus as much room again (one
            // shift per doubling, so a node that keeps growing costs O(1) amortised per instance)
            let (end, extra) = (end + room, fresh.len());
            for k in start..end {
                let old = target.instances[k];
                self.damage_instance(&old);
            }
            for new in fresh {
                self.damage_instance(new);
            }
            target.instances.splice(start..end, fresh.iter().copied().chain(std::iter::repeat_n(EMPTY, extra)));
            target.bounds.reserve(target.instances.capacity().saturating_sub(target.bounds.len()));
            target.bounds.splice(start..end, fresh.iter().map(cull_bounds).chain(std::iter::repeat_n(EMPTY_BOUNDS, extra)));
            let delta = (fresh.len() + extra - (end - start)) as u32;
            self.shifts += 1;
            add_range(&mut target.dirty, start as u32, target.instances.len() as u32);
            for b in &mut target.breaks {
                if b.at_instance as usize >= end {
                    b.at_instance += delta;
                }
            }
            for (index, s) in self.slots.iter_mut().enumerate() {
                if index != slot && s.key != 0 && s.range_layer == key && s.range.0 as usize >= end {
                    s.range = (s.range.0 + delta, s.range.1 + delta);
                }
            }
            for r in &mut self.views[1..] {
                if r.valid && r.layer == key {
                    if r.run.0 as usize >= end {
                        r.run.0 += delta;
                    }
                    if r.run.1 as usize >= end {
                        r.run.1 += delta;
                    }
                }
            }
            let s = &mut self.slots[slot];
            (s.range.1, s.room) = ((start + fresh.len()) as u32, extra as u32);
        }
        if rebreak {
            let delta = target.instances.len() as i64 - old_len as i64;
            patch_breaks(&mut target, &old_breaks, &paint.layers[0], start, old_end, start + fresh.len(), delta);
            if owner != 0 {
                // its kept record of its breaks is stale: it rebuilds next frame (this one is right)
                self.views[owner as usize].dirty = true;
                self.frame_requested = true;
            }
        }
        let node_end = (start + fresh.len()) as u32;
        if !fresh.is_empty() || target.instances.len() != old_len {
            let at = target.runs.partition_point(|run| run.to <= start as u32);
            if target.instances.len() == old_len
                && target.runs.get(at).is_some_and(|run| run.from <= start as u32 && node_end <= run.to && run.view == paint.view)
            {
                let run = &mut target.runs[at];
                for &bounds in &target.bounds[start..node_end as usize] {
                    run.bounds = union_bounds(run.bounds, bounds);
                }
            } else {
                let mut runs = std::mem::take(&mut target.runs);
                runs.clear();
                fresh_runs(&mut runs, &target, 0, target.instances.len() as u32);
                target.runs = runs;
            }
        }
        self.temp_breaks = old_breaks;
        self.draw[layer] = target;
        self.views[owner as usize].patched = true;
        self.invalidate_a11y_view(owner);
        self.draw_revision += 1;
        self.a11y_dirty = true;
        true
    }

    /// Between builds: set a node's text and rewrite only its instances, with no build or layout.
    /// The host sees `draw_revision` change, draws, and drains new glyphs from `text_mut`. The patch
    /// holds until the node's owner next declares it; then the build's value wins. `false` for a
    /// stale handle, or when the patch has to wait for the next frame.
    pub fn patch_text(&mut self, b: &Handle, text: &str) -> bool {
        if !self.live(b) {
            return false;
        }
        self.text(b, text);
        self.repaint_node(b.box_slot)
    }

    /// `patch_text` for the look's fill colour.
    pub fn patch_fill(&mut self, b: &Handle, fill: Rgba) -> bool {
        self.patch_look(b, &Look { fill, ..self.slots.get(b.box_slot).map_or(Look::NONE, |s| s.node.look) })
    }

    /// `patch_text` for the whole look.
    pub fn patch_look(&mut self, b: &Handle, look: &Look) -> bool {
        if !self.live(b) {
            return false;
        }
        let look = self.resolve_look(look).unwrap_or(*look);
        let s = &mut self.slots[b.box_slot];
        s.node.look = look;
        s.look_needs_snapshot = true;
        self.repaint_node(b.box_slot)
    }

    /// `patch_text` for the node's rectangle: its look and text move or resize. Its children, its
    /// canvases and its hit area stay until the next frame.
    pub fn patch_rect(&mut self, b: &Handle, rect: Rect) -> bool {
        if !self.live(b) {
            return false;
        }
        let s = &mut self.slots[b.box_slot];
        (s.rx, s.ry, s.rw, s.rh) = (rect.x, rect.y, rect.w, rect.h);
        self.repaint_node(b.box_slot)
    }

    /// `patch_text` for the node's mark (`Mark::Line`'s direction).
    pub fn patch_mark(&mut self, b: &Handle, mark: Mark) -> bool {
        if !self.live(b) {
            return false;
        }
        self.slots[b.box_slot].node.mark = mark;
        self.repaint_node(b.box_slot)
    }

    /// The node's rectangle as its view lays it out, what `patch_rect` takes (`rect` is where the
    /// window shows it); empty for a stale handle.
    pub fn local_rect(&self, b: &Handle) -> Rect {
        if self.live(b) { self.slot_rect(b.box_slot) } else { Rect::new(0.0, 0.0, 0.0, 0.0) }
    }

    /// `patch_rect` by an offset.
    pub fn patch_offset(&mut self, b: &Handle, dx: f32, dy: f32) -> bool {
        if !self.live(b) {
            return false;
        }
        let r = self.slot_rect(b.box_slot);
        self.patch_rect(b, Rect::new(r.x + dx, r.y + dy, r.w, r.h))
    }

    // -------------------------------------------------------------- frame

    /// The payload in flight follows the pointer as a small label above everything. Idle frames
    /// (nothing carried) build and paint nothing.
    fn carry_ghost(&mut self) {
        if let Some((_, Origin::FromPointer(owner))) = &self.payload {
            let owner = *owner;
            // A source/container can disappear during this build; leave no painted ghost behind.
            let present = self.table.find(owner).is_some_and(|slot| self.slots[slot].touched == self.frame_number || self.kept_now(self.slots[slot].owner));
            if self.active != Some(owner) || !present { self.cancel_carry(); return; }
        }
        if !self.pointer.0.is_finite() { return; }
        // The ghost copies text into frame storage; keep ownership here instead of cloning each frame.
        let Some((payload, origin)) = self.payload.take() else { return; };
        let (px, py) = self.pointer;
        let width = self.text_width(None, &payload.value) + 12.0;
        let saved = self.enter_root();
        // kit overlays [07]: a white 20-point chip with the payload in body-size ink, and under it
        // the sheet's tip with the key that drops it
        let ghost = self.box_("ui-carry-ghost", BoxOpts::default().w(Size::Px(width)).h(Size::Px(20.0)).at((px + 14.0, py + 14.0)));
        let small = 8.max(self.font_size - 2);
        let tip_text = "drop to put";
        let tip_key = "esc";
        let tw1 = self.text_width(Some(small), tip_text);
        let tw2 = self.text_width(Some(small), tip_key);
        let tip_w = 2.0 + 12.0 + tw1 + 6.0 + tw2;
        let tip = self.box_("ui-carry-tip", BoxOpts::default().w(Size::Px(tip_w)).h(Size::Px(20.0)).at((px + 14.0, py + 14.0 + 20.0 + 8.0)));
        let t = self.theme();
        let (ghost_look, tip_look, key_look) = (t.ghost.idle, t.tooltip.idle, t.key.idle);
        self.set_look(&ghost, ghost_look, State::default());
        self.slots[ghost.box_slot].node.inset.l = 6.0;
        self.text(&ghost, &payload.value);
        self.set_look(&tip, tip_look, State::default());
        self.slots[tip.box_slot].node.inset.l = 7.0;
        self.text(&tip, tip_text);
        let inner = self.enter(&tip);
        let k = self.node_keyed(int_key(tip.box_key, 0), Node { w: Size::Px(tw2.max(1.0)), h: Size::Px(20.0), at: Some((7.0 + tw1 + 6.0, 0.0)).into(), look: key_look, ..Node::DEFAULT });
        self.text(&k, tip_key);
        self.leave(inner);
        // Tooltip and ghost layers precede existing foreground entries at maximum priority.
        self.foreground.push((i64::MAX, tip.index, LayerRole::Drag));
        self.foreground.push((i64::MAX, ghost.index, LayerRole::Drag));
        self.leave(saved);
        self.payload = Some((payload, origin));
    }

    fn clear_frame_callbacks(&mut self) {
        for index in 0..self.count {
            let b = &mut self.boxes[index];
            b.hit = None;
            b.hit_rect = None;
            b.shape = HitShape::Own;
            b.paint = arena::EMPTY;
            b.over = arena::EMPTY;
        }
        self.arena.reset();
    }

    /// Advance the running transitions to `t` without building, laying out or painting: each
    /// animated node is patched in place and a view-level transition moves its view entry, so the
    /// cost follows the animated nodes, not the window. Returns false, changing nothing, when the
    /// next frame needs a build instead: before the first frame, while input, a gesture, a frame
    /// request, a timer, a hover delay or a settling layout is due by `t`, or when time runs backwards.
    /// A host with no events and nothing new to show calls this in place of `frame` while
    /// `repaint_at` comes only from transitions; damage and `draw_revision` report what changed.
    pub fn step(&mut self, t: f64) -> bool {
        let trace = self.trace_frame_reasons.load(Ordering::Relaxed);
        if self.destroyed || self.building || self.painting || self.painted < 0 || !t.is_finite() || t < self.now {
            if trace { self.frame_reasons.step_state += 1; }
            return false;
        }
        if self.settle || !self.repatch.is_empty() || self.frame_requested {
            if trace { self.frame_reasons.step_build += 1; }
            return false;
        }
        if self.keyboard.flags & pxui_types::Keyboard::ANIMATING != 0 || self.active.is_some()
            || !self.moving.is_empty() || !self.gsettle.is_empty() {
            if trace { self.frame_reasons.step_interaction += 1; }
            return false;
        }
        if self.views.iter().any(|v| v.dirty) {
            if trace { self.frame_reasons.step_build += 1; }
            return false;
        }
        let wake = self.views.iter().skip(1).fold(self.wake.min(self.touches.wake), |w, r| w.min(r.wake));
        let hover = matches!(&self.hover_rest, Some((_, _, start, seen)) if *seen == self.frame_number && t >= start + HOVER_SECONDS);
        if wake <= t || hover {
            if trace { self.frame_reasons.step_deadline += 1; }
            return false;
        }
        if trace { self.frame_reasons.stepped += 1; }
        self.advance_ambient(t);
        self.now = t;
        self.stepping = true;
        self.step_tweens();
        self.stepping = false;
        if self.refresh_views() {
            self.draw_revision += 1;
        }
        self.collect_blurs();
        if self.promotion.enabled { self.update_promotions(); }
        self.out.repaint_at = self.next_repaint(t);
        true
    }

    /// Route the frame's ordered events, run the builder, lay out, and paint. Tab/Shift-Tab
    /// traverse visible focusable boxes in presentation order, restricted to an open popup.
    /// Enter/Space activate controls.
    /// One frame: build, lay out, paint. When layout finds kept output stale (a kept view resized,
    /// moved without a table entry or in another layer, a virtual grid's columns or a scroll gutter
    /// changed), the frame settles: it builds again at once with the same input minus its events (no
    /// click or key is seen twice), at most twice, so no stale frame is shown.
    #[inline]
    pub fn frame(&mut self, frame: &rframe::Frame, f: &mut dyn FnMut(&mut Ui)) -> FrameOut<'_> {
        self.build_frame(frame, f);
        self.output()
    }

    fn build_frame(&mut self, frame: &rframe::Frame, f: &mut dyn FnMut(&mut Ui)) {
        self.phase_counts = self.phase_counter.map(|_| [0; PHASES]);
        self.first_build = self.frame_number + 1;
        self.begin_frame(frame);
        f(self);
        self.end_frame(frame);
        for _ in 0..2 {
            if !std::mem::take(&mut self.settle) {
                break;
            }
            let mut quiet = self.quiet.take().unwrap_or_else(|| frame.clone());
            quiet.input.t = frame.input.t;
            quiet.input.dt = 0.0;
            (quiet.input.frame, quiet.input.tick, quiet.input.size, quiet.input.pointer) = (frame.input.frame, frame.input.tick, frame.input.size, frame.input.pointer);
            quiet.input.buttons.clone_from(&frame.input.buttons);
            quiet.input.keys.clone_from(&frame.input.keys);
            quiet.input.events.clear();
            (quiet.pixel_scale, quiet.fps, quiet.mouse_delta) = (frame.pixel_scale, frame.fps, (0.0, 0.0));
            // a copy or paste the first build asked for is this frame's: the quiet build sees no key to ask again
            let (paste, clipboard) = (self.out.paste, self.out.clipboard.take());
            self.begin_frame(&quiet);
            f(self);
            self.end_frame(&quiet);
            self.out.paste |= paste;
            if self.out.clipboard.is_none() { self.out.clipboard = clipboard; }
            self.quiet = Some(quiet);
        }
        self.settle = false;
        self.full_damage = false;
    }

    /// Leaves a frame whose build panicked (the panic goes on to the app): the next `frame` starts clean, and every kept view is
    /// built again, as one the panic stopped part way has nodes that are not its own.
    pub fn abandon_frame(&mut self) {
        self.building = false;
        self.clear_frame_callbacks();
        self.settle = false;
        self.view_cur = 0;
        self.theme_depth = 0;
        self.parents.clear();
        self.seeds.clear();
        self.epoch += 1;
    }

    pub fn begin_frame(&mut self, frame: &rframe::Frame) {
        if self.destroyed {
            panic!("Ui.frame: the UI was destroyed");
        }
        if self.building {
            panic!("Ui.frame: frames cannot nest");
        }
        if !frame.input.t.is_finite() {
            panic!("Ui.frame: time must be finite");
        }
        self.phase_at = self.timing.then(std::time::Instant::now);
        self.counter_at = self.phase_counter.and_then(|clock| clock());
        // text the prepare queue delivered since the last frame: this frame draws it
        if let Some(text) = &mut self.text { text.install_queued(); }
        if self.counter_at.is_none() { self.phase_counts = None; }
        let (scale_x, scale_y) = frame.pixel_scale;
        let density = 1.max(scale_x.max(scale_y).round() as i64);
        let size = (frame.input.size.0 as f32, frame.input.size.1 as f32);
        self.height_changed = size.1 != self.view_h;
        if density != self.density || size != (self.view_w, self.view_h) {
            let previous = self.epoch;
            self.epoch += 1;
            self.full_damage = true;
            if density == self.density {
                for view in &mut self.views[1..] {
                    if view.viewport_independent && view.epoch == previous {
                        view.epoch = self.epoch;
                    }
                }
            }
        }
        self.density = density;
        if let Some((source, target)) = self.repaint_palette.take() && target == self.epoch {
            self.recolor_views(source);
        }
        // outside a panel the kit row is the theme's row (a panel sets its own while it builds)
        self.row_points = self.theme.row_h * self.theme.tokens.density;
        self.kit_row_height = self.row_points.round() as i64;
        self.frame_number += 1;
        // a request made between frames is this frame; one made while building is the next
        self.frame_requested = false;
        self.out.clipboard = None;
        self.out.paste = false;
        self.text_input_options = pxui_types::TextTraits::default();
        // a variant a host forgot to clear does not reach this frame's widgets
        self.set_variant(0, 0);
        self.out.background = None;
        // the window's title bar shows the ground the UI is painted on
        if self.window_ground != Some(self.theme.tokens.panel) {
            self.window_ground = Some(self.theme.tokens.panel);
            self.out.background = Some(self.theme.tokens.panel);
        }
        let previous_input = (self.hot, self.active, self.focus, self.keyboard_focus);
        let accessible_input = !self.a11y_pending.is_empty();
        self.route(frame);
        self.dismissed_popups.clear();
        self.apply_a11y();
        if accessible_input || (!frame.input.events.is_empty()
            && (previous_input != (self.hot, self.active, self.focus, self.keyboard_focus)
                || !self.signals.is_empty()
                || self.routed_events.iter().any(|(owner, event, _)| match event {
                    Event::PointerMoved(_) => false,
                    Event::FocusGained | Event::FocusLost => true,
                    _ => *owner != 0,
                })))
        {
            self.input_at = frame.input.t;
        }
        self.mark_input();
        // overlay thumbs whose hold is over: their views repaint without them
        let mut i = 0;
        while i < self.thumb_hides.len() {
            let (slot, hide) = self.thumb_hides[i];
            if hide <= frame.input.t {
                self.thumb_hides.swap_remove(i);
                if let Some(owner) = self.slots.get(slot).map(|s| s.owner) {
                    self.mark_view(owner);
                }
            } else {
                i += 1;
            }
        }
        // views whose `repaint_after` is due rebuild now (the root rebuilds anyway)
        if self.wake <= frame.input.t {
            self.wake = f64::INFINITY;
        }
        for v in 1..self.views.len() {
            if self.views[v].wake <= frame.input.t {
                self.views[v].wake = f64::INFINITY;
                self.mark_view(v as u32);
            }
        }
        self.views[0].built = self.frame_number;
        self.modal_key = None;
        self.modal_extra.clear();
        self.count = 0;
        self.rebuilt = 0;
        self.closed_section = NONE;
        (self.theme_depth, self.theme_id) = (0, 0);
        self.advance_ambient(frame.input.t);
        (self.now, self.motion) = (frame.input.t, self.theme.tokens.moves());
        self.building = true;
        self.parents.clear();
        self.seeds.clear();
        let root = &mut self.views[0];
        std::mem::swap(&mut root.nodes, &mut root.prev);
        root.nodes.clear();
        let root = self.append_box(ROOT_KEY, self.table.find(ROOT_KEY), NONE);
        let (w, h) = (frame.input.size.0 as f32, frame.input.size.1 as f32);
        self.set_box(root, &Node { w: Size::Px(w), h: Size::Px(h), at: Some((0.0, 0.0)).into(), scroll_step: 0.0, ..Node::DEFAULT });
        self.parents.push(root);
        self.seeds.push(ROOT_KEY);
        self.phase(0);
    }

    pub fn end_frame(&mut self, frame: &rframe::Frame) {
        self.carry_ghost();
        self.building = false;
        let root = &mut self.views[0];
        root.prev.reserve(root.nodes.len().saturating_sub(root.prev.len()));
        self.parents.clear();
        self.seeds.clear();
        // Move popups to the end of the root's children, oldest first.
        self.foreground.sort_by(|a, b| a.0.cmp(&b.0));
        for position in 0..self.foreground.len() + self.overlays.len() {
            let index = if position < self.foreground.len() {
                self.foreground[position].1
            } else {
                self.overlays[position - self.foreground.len()].0
            };
            if self.boxes[index].parent == 0 && self.boxes[0].last != index {
                let mut previous = NONE;
                let mut child = self.boxes[0].first;
                while child != NONE {
                    if child == index {
                        if previous == NONE {
                            self.boxes[0].first = self.boxes[index].next;
                        } else {
                            self.boxes[previous].next = self.boxes[index].next;
                        }
                        break;
                    }
                    previous = child;
                    child = self.boxes[child].next;
                }
                let last = self.boxes[0].last;
                self.boxes[last].next = index;
                self.boxes[index].next = NONE;
                self.boxes[0].last = index;
            }
        }
        self.phase(1);
        self.intrinsic();
        self.phase(2);
        self.arrange(frame.input.t);
        self.phase(3);
        self.layout_time = self.phases[2] + self.phases[3];
        self.paint_all(frame);
        // a size the build read before layout (last frame's) that layout changed: build again
        let mut expected = self.expected.take();
        for ExpectedSize { slot, w, h, grid } in &expected {
            let (slot, w, h) = (*slot, *w, *h);
            let s = &self.slots[slot];
            if s.key != 0 && ((!w.is_nan() && (s.rw - w).abs() > 0.01) || (!h.is_nan() && (s.rh - h).abs() > 0.01)) {
                if let Some(grid) = grid {
                    let (room, across) = grid_space(s.rw, s.rh, grid.padding, grid.sideways);
                    let offset = if grid.sideways { self.scroll_xs[slot] } else { self.scroll_y[slot] };
                    let glide = &self.glides[usize::from(grid.sideways)][slot];
                    if s.node.padding == grid.padding && offset == 0.0 && glide.visual == 0.0 && !glide.moving()
                        && room >= grid.extent && ((across / grid.across).floor() as usize).max(1) == grid.per {
                        continue;
                    }
                }
                self.mark_view(s.owner);
                self.settle = true;
            }
        }
        expected.clear();
        self.expected.set(expected);
        // measured rows of `list_measured`: a new height goes into the cache and the frame settles
        for k in 0..self.list_rows.len() {
            let (key, i, slot) = self.list_rows[k];
            let h = self.slots[slot].rh;
            if let Some(cache) = self.list_heights.get_mut(&key) {
                if i < cache.heights.len() && (cache.heights[i] - h).abs() > 0.01 && self.slots[slot].key != 0 {
                    (cache.heights[i], cache.dirty) = (h, true);
                    self.settle = true;
                }
            }
        }
        self.list_rows.clear();
        self.finish_reveals();
        let frame_number = self.frame_number;
        self.list_heights.retain(|key, c| frame_number - c.frame < 600 || self.table.find(*key).is_some());
        self.phase(4);
        self.emit_a11y();
        self.phase(5);
        let mut repatch = std::mem::take(&mut self.repatch);
        for slot in repatch.drain(..) {
            // a node moving through a tween is painted by its step, next
            if self.tween_of(slot).is_none() {
                self.repaint_node(slot);
            }
        }
        self.repatch = repatch;
        self.step_tweens();
        // the atlas does not rewrite a shelf a kept view's instances sample: it retires it, and every
        // view rebuilds next frame (the texels stay for the frames in flight); a glyph without room
        // shows next frame
        let mut pin = [0u64; 8];
        for r in &self.views[1..] {
            if r.valid {
                for k in 0..8 {
                    pin[k] |= r.shelves[k];
                }
            }
        }
        let text = self.text_mut();
        text.end_frame();
        text.pin(pin);
        let atlas = (text.retirements(), text.misses());
        if atlas != self.atlas_events {
            self.epoch += (atlas.0 != self.atlas_events.0) as u64;
            self.atlas_events = atlas;
            self.frame_requested = true;
        }
        self.foreground.clear();
        self.overlays.clear();
        self.prune();
        if self.focus != 0 && self.table.find(self.focus).is_none() {
            self.focus = 0;
            self.edit_focus = 0;
        }
        // the build moved the focus: views already kept this frame show it next frame
        if (self.focus, self.keyboard_focus) != (self.seen.1, self.seen.2) {
            self.mark_key(self.seen.1);
            self.mark_key(self.focus);
            (self.seen.1, self.seen.2) = (self.focus, self.keyboard_focus);
        }
        if self.promotion.enabled { self.update_promotions(); }
        self.out.repaint_at = self.next_repaint(frame.input.t);
        self.frame_damage = self.damage;
        self.frame_requested = false;
        // Layout, hit rectangles and painting are published data now. The callbacks can capture
        // whole document versions, so keep them only while building.
        self.clear_frame_callbacks();
        self.phase(6);
    }

    // ------------------------------------------------------ layout helpers

    /// The hover wait of the C text-keyed export: a new text copies it (its 1.x behaviour).
    pub fn hover_delay(&mut self, key: &str) -> bool { self.hover_wait(Some(key), 0) }

    /// The hover wait on a resolved key (a box key or an authored key resolved under its seed): resets copy no text.
    pub fn hover_delay_key(&mut self, key: i64) -> bool { self.hover_wait(None, key) }

    pub(crate) fn hover_wait(&mut self, text: Option<&str>, key: i64) -> bool {
        self.escape();
        let time = match &self.input_frame {
            Some(frame) => frame.input.t,
            None => 0.0,
        };
        if self.touches.using_touch || self.active.is_some() || self.modal_in_frame || self.frame_events.iter().any(|e| matches!(e, Event::FocusLost)) {
            self.hover_rest = None;
            return false;
        }
        match &self.hover_rest {
            Some((previous, pointer, start, seen))
                if previous.is(text, key)
                    && *pointer == self.pointer
                    && (*seen == self.frame_number || *seen == self.frame_number - 1)
                    && time >= *start =>
            {
                let start = *start;
                self.hover_rest.as_mut().unwrap().3 = self.frame_number;
                time - start >= HOVER_SECONDS
            }
            _ => {
                self.hover_rest = Some((text.map_or(HoverId::Key(key), |t| HoverId::Text(t.to_owned())), self.pointer, time, self.frame_number));
                false
            }
        }
    }

    /// The tooltip once the hover wait of `wait` (text) or `key` (see `hover_wait`) has passed: a 20-point sheet with a line-2 edge
    /// and 6 points of padding, text at the label size and long text wrapping to further 14-point lines.
    pub(crate) fn tooltip_sheet(&mut self, wait: Option<&str>, key: i64, text: &str) {
        if !self.hover_wait(wait, key) {
            return;
        }
        let size = 8.max(self.font_size - 2);
        let char_width = 1.0f32.max(self.text_width(Some(size), "0"));
        let width = 400.0f32.min(40.0f32.max(self.view_w - 16.0));
        let columns = 1.max(((width - 14.0) / char_width) as i64) as usize;
        let mut lines = std::mem::take(&mut self.tooltip_lines);
        let (mut line, mut word_start) = (0..0, 0);
        for word in text.split(' ') {
            let end = word_start + word.len();
            if line.is_empty() {
                line = word_start..end;
            } else if end - line.start <= columns {
                line.end = end;
            } else {
                lines.push(std::mem::replace(&mut line, word_start..end));
            }
            word_start = end + 1;
        }
        lines.push(line);
        let mut widest: f32 = 0.0;
        for line in &lines {
            widest = widest.max(self.text_width(Some(size), &text[line.clone()]));
        }
        let width = width.min(14.0 + widest);
        let height = 20.0 + 14.0 * (lines.len() as f32 - 1.0);
        // 12 right of the pointer and 20 below it, or 8 above it when that does not fit
        let (px, py) = self.pointer;
        let placement = Placement { side: Side::Below, align: Along::Start, gap: 0.0, margin: 8.0, flip: true };
        let Rect { x, y, .. } = place_anchored(Rect::new(px + 12.0, py - 8.0, 0.0, 28.0), (width, height), placement, Rect::new(0.0, 0.0, self.view_w, self.view_h));
        let saved = self.enter_root();
        let tip = self.box_("ui-tooltip", BoxOpts::default().w(Size::Px(width)).h(Size::Px(height)).at((x, y)));
        self.set_role(&tip, Role::Tooltip, 0);
        self.a11y_name(&tip, text);
        self.overlays.push((tip.index, LayerRole::Tooltip));
        let config = TipConfig { tip, width, height };
        in_body!(self, tip.box_key, tip, tooltip, default_tip(&config, text, &lines));
        self.leave(saved);
        lines.clear();
        if lines.capacity() > self.tooltip_lines.capacity() { self.tooltip_lines = lines; }
    }

    /// A popup placed by `anchor` (window points, e.g. `ui.rect(&field)` or a point's empty rect):
    /// on the placement's side, flipped to the other when it does not fit there, kept inside the
    /// window. Its size may be `Fit`: placement runs in layout, after the size is known. Built at the
    /// root wherever it is called, and painted over the rest.
    pub(crate) fn anchored_label(&mut self, label: &str, anchor: Rect, placement: Placement, w: Size, h: Size, f: &mut dyn FnMut(&mut Ui)) -> Handle {
        let saved = self.enter_root_parent();
        let b = self.node_ref(label, &Node { w, h, anchor: Some(Anchor::new(anchor, placement)).into(), ..Node::DEFAULT });
        self.overlays.push((b.index, LayerRole::Popup));
        self.within(&b, f);
        self.leave(saved);
        b
    }

    /// A sheet keyed by `key` (hashed under the parent's seed) placed by `anchor` at `overlay`'s placement, `overlay`'s width and
    /// height (each its content's when unset).
    pub fn anchored(&mut self, key: &str, anchor: Rect, overlay: &Overlay, f: &mut dyn FnMut(&mut Ui)) -> Handle {
        let size = |v: Option<f32>| v.map_or(Size::Fit, Size::Px);
        self.anchored_label(key, anchor, overlay.place, size(overlay.w), size(overlay.h), f)
    }

    pub fn col(&mut self, padding: f32, label: &str, f: &mut dyn FnMut(&mut Ui)) {
        let b = self.box_(label, BoxOpts::default().axis(Axis::Column).padding(padding));
        self.within(&b, f);
    }

    pub(crate) fn input_frame_time(&self) -> Option<f64> {
        self.input_frame.as_ref().map(|f| f.input.t)
    }
}

// ---------------------------------------------------------- transitions

impl Ui {
    fn snap_motion(&mut self) {
        self.swap_frame=-1;
        for i in 0..self.tweens.len() {
            self.free_keys(i);
            let t=&mut self.tweens[i];
            t.from=t.to;
            t.curve=Curve::Bezier { duration:0.0,points:[0.0,0.0,1.0,1.0] };
        }
    }

    /// The running transition of `slot`, if it is still that node's.
    fn tween_of(&self, slot: usize) -> Option<usize> {
        let s = &self.slots[slot];
        let t = s.tween as usize;
        (s.tween != NO_TWEEN && t < self.tweens.len() && self.tweens[t].slot == slot && self.tweens[t].generation == s.generation).then_some(t)
    }

    /// Where the node rests: its declared look, offset and opacity.
    fn rest(&self, slot: usize) -> Pose {
        let s = &self.slots[slot];
        let [x, y, ax, ay] = if s.view == NO_VIEW { [1.0, 1.0, 0.5, 0.5] } else { self.views[s.view as usize].scale_pose };
        Pose { look: s.node.look, dx: s.pose.0, dy: s.pose.1, opacity: s.pose.2, scale: [x, y], anchor: [ax, ay] }
    }

    /// What the node shows now, and a running transition's shadows that need steady instance room.
    fn pose(&self, slot: usize) -> (Pose, Option<[bool; 2]>) {
        self.pose_at(slot, self.now)
    }

    /// `pose` at time `at`.
    fn pose_at(&self, slot: usize, at: f64) -> (Pose, Option<[bool; 2]>) {
        match self.tween_of(slot) {
            None => (self.rest(slot), None),
            Some(i) => {
                let t = &self.tweens[i];
                let (from, to, p) = self.tween_segment(i, at);
                let f = |a: f32, b: f32| a + (b - a) * p;
                let pose = Pose { look: from.look.lerp(&to.look, p), dx: f(from.dx, to.dx), dy: f(from.dy, to.dy), opacity: f(from.opacity, to.opacity), scale: [f(from.scale[0], to.scale[0]), f(from.scale[1], to.scale[1])], anchor: to.anchor };
                // Invisible endpoint shadows stay invisible; keyframes retain room for every stop.
                let shadows = std::array::from_fn(|i| t.keys != NO_KEYS || t.from.look.shadows[i].color.a > 0 || t.to.look.shadows[i].color.a > 0);
                let body = |look: &Look| look.fill != Rgba::CLEAR || look.gradient.is_some() || look.border != Rgba::CLEAR || look.ring.color != Rgba::CLEAR || look.shadows.iter().any(|s| s.color.a > 0);
                let steady = !t.view_only && (t.keys != NO_KEYS || body(&t.from.look) || body(&t.to.look));
                (pose, steady.then_some(shadows))
            }
        }
    }

    /// A view's transform uses the same segment as its painted look.
    fn tween_segment(&self, i: usize, at: f64) -> (&Pose, &Pose, f32) {
        let t = &self.tweens[i];
        let elapsed = at - t.start;
        match self.keyframes.get(t.keys as usize) {
            None => (&t.from, &t.to, self.curve_at(&t.curve, elapsed as f32)),
            Some(k) => {
                let elapsed = k.time(self.keyframe_age(t, k, at));
                let j = k.t[..k.n].iter().position(|&end| elapsed < end).unwrap_or(k.n - 1);
                let (from, t0) = if j == 0 { (&t.from, 0.0) } else { (&k.pose[j - 1], k.t[j - 1]) };
                let curve = k.curve[j].with_duration(k.t[j] - t0);
                (from, &k.pose[j], self.curve_at(&curve, elapsed - t0))
            }
        }
    }

    fn keyframe_age(&self, tween: &Tween, keys: &Keys, at: f64) -> f64 {
        let sample = match keys.rate {
            pxui_types::ui::Rate::Display => at,
            _ if self.tween_promoted(tween) => at,
            pxui_types::ui::Rate::Ambient(_) => at.min(self.ambient_at).max(tween.start),
        };
        sample - tween.start
    }

    /// Share a Bezier bisection between poses at the same time on the same curve.
    fn curve_at(&self, curve: &Curve, t: f32) -> f32 {
        let Curve::Bezier { duration, points: [x1, y1, x2, y2] } = *curve else { return curve.at(t) };
        let key = [duration, x1, y1, x2, y2, t].map(f32::to_bits);
        let memo = self.curve_memo.get();
        for (last, p) in memo {
            if last == key { return p; }
        }
        let p = curve.at(t);
        self.curve_memo.set([memo[1], (key, p)]);
        p
    }

    fn start_tween(&mut self, slot: usize, curve: Curve, from: Pose, to: Pose) {
        let v = self.slots[slot].view;
        let view_only = v != NO_VIEW && self.views[v as usize].index != 0 && from.look == to.look;
        let tween = Tween { slot, generation: self.slots[slot].generation, start: self.now, curve, from, to, keys: NO_KEYS, view_only };
        match self.tween_of(slot) {
            Some(i) => {
                self.free_keys(i);
                self.tweens[i] = tween;
            }
            None => {
                self.slots[slot].tween = self.tweens.len() as u32;
                self.tweens.push(tween);
            }
        }
    }

    fn tween_visible(&self, tween: &Tween) -> bool {
        let slot = &self.slots[tween.slot];
        // A kept child's instances stay live without being painted again.
        if slot.view == NO_VIEW && !self.range_valid(tween.slot) { return false; }
        let mut v = if slot.view == NO_VIEW { slot.owner } else { slot.view };
        if v == 0 { return true; }
        let view = &self.views[v as usize];
        if !self.shown(v) { return false; }
        if view.screen.w <= 0.0 || view.screen.h <= 0.0 {
            // A view can move into its clip on its own, even when its current pose is outside.
            let moving = slot.view != NO_VIEW && tween.keys == NO_KEYS
                && (tween.from.dx != tween.to.dx || tween.from.dy != tween.to.dy
                    || tween.from.scale != tween.to.scale);
            if !moving || view.outer.w <= 0.0 || view.outer.h <= 0.0 { return false; }
        }
        while v != 0 {
            if self.views[v as usize].opacity == 0.0 { return false; }
            v = self.views[v as usize].parent;
        }
        true
    }

    /// After a paint: finished transitions come to rest, and nodes the paint did not reach (in a kept
    /// view) are rewritten in place through the patch path, so a transition never rebuilds a view.
    fn step_tweens(&mut self) {
        let mut paint = None;
        let mut i = 0;
        while i < self.tweens.len() {
            // the fields it reads, not the whole tween (two poses and a curve) per tween per frame
            let t = &self.tweens[i];
            let (slot, view_only) = (t.slot, t.view_only);
            let s = &self.slots[slot];
            if s.key == 0 || s.generation != t.generation || s.tween != i as u32 {
                self.drop_tween(i);
                continue;
            }
            let done = self.keyframes.get(t.keys as usize).map_or_else(|| t.curve.finished((self.now - t.start) as f32), |k| k.finished(self.now - t.start));
            if done {
                let to = &self.tweens[i].to;
                let s = &mut self.slots[slot];
                (s.node.look, s.pose) = (to.look, (to.dx, to.dy, to.opacity));
                s.look_needs_snapshot = true;
                if s.view != NO_VIEW { self.views[s.view as usize].scale_pose = [to.scale[0], to.scale[1], to.anchor[0], to.anchor[1]]; }
                self.drop_tween(i);
            } else {
                i += 1;
            }
            // a node of a view verified this frame was painted as of the kept build (`verify_time`):
            // it shows now what a fresh paint would, which is no patch for the next check to skip
            let owner = self.slots[slot].owner as usize;
            let verified = !self.stepping && owner != 0 && self.views[owner].verify && self.views[owner].built == self.frame_number;
            if !view_only && (done || verified || self.stepping || self.slots[slot].painted != self.frame_number) {
                let patched = self.views[owner].patched;
                if self.range_valid(slot) {
                    let paint = paint.get_or_insert_with(|| {
                        let scratch = std::mem::take(&mut self.patch_layers);
                        self.paint_begin(scratch, self.slots[slot].clip)
                    });
                    self.repaint_with(slot, paint);
                }
                if verified {
                    self.views[owner].patched = patched;
                }
            }
        }
        if let Some(paint) = paint { self.patch_layers = self.paint_end(paint); }
    }

    /// Give tween `i`'s keyframes back.
    fn free_keys(&mut self, i: usize) {
        let k = std::mem::replace(&mut self.tweens[i].keys, NO_KEYS);
        if k != NO_KEYS {
            self.free_keyframes.push(k);
        }
    }

    fn drop_tween(&mut self, i: usize) {
        self.free_keys(i);
        let slot = self.tweens[i].slot;
        if self.slots[slot].tween == i as u32 {
            self.slots[slot].tween = NO_TWEEN;
        }
        self.tweens.swap_remove(i);
        if let Some(moved) = self.tweens.get(i) {
            if self.slots[moved.slot].tween == self.tweens.len() as u32 {
                self.slots[moved.slot].tween = i as u32;
            }
        }
    }

    /// Animate a node to `target` along `curve` from where it shows now: its look, its offset from
    /// where layout puts it, its opacity, or its view's X/Y scale. Driven by the patch/table path (the node's view does not
    /// rebuild); the frame gate asks for frames until it rests. An animated look holds until the
    /// node's next declaration (as `patch_look`); offset and opacity hold until animated again. The
    /// theme's `reduced_motion` makes it a jump. Scale composes after the manual transform about a
    /// fractional anchor, fixed for the segment; it requires a view with its own painter entry.
    /// `false` for a stale handle or a scale target on an ordinary node/flat view.
    // Offset and opacity move the node's own look and text, not its children (a view's offset and
    // opacity move a group), and hit testing stays where layout put it. Scale moves the whole view
    // through its table entry, including nested hit/accessible rectangles, without rewriting instances.
    pub fn animate(&mut self, b: &Handle, target: Target, curve: Curve) -> bool {
        if !self.live(b) {
            return false;
        }
        if matches!(target, Target::Scale { .. }) && !self.view_id(b).is_some_and(|v| self.views[v as usize].index != 0) { return false; }
        let slot = b.box_slot;
        let curve = if self.theme().tokens.reduced_motion { Curve::Bezier { duration: 0.0, points: [0.0, 0.0, 1.0, 1.0] } } else { curve };
        let from = self.pose(slot).0;
        let mut to = match self.tween_of(slot) {
            Some(i) => self.tweens[i].to,
            None => self.rest(slot),
        };
        match target {
            Target::Look(look) => to.look = self.resolve_look(&look).unwrap_or(look),
            Target::Offset(dx, dy) => (to.dx, to.dy) = (dx, dy),
            Target::Opacity(o) => to.opacity = o.clamp(0.0, 1.0),
            Target::Scale { x, y, anchor } => { to.scale = [x, y]; to.anchor = [anchor.0, anchor.1]; },
        }
        self.start_tween(slot, curve, from, to);
        if !self.building {
            self.frame_requested = true;
        }
        true
    }

    /// Animate a node through up to `MAX_KEYS` keyframes from where it shows now: each `Keyframe { time,
    /// target, curve }` reaches `target` `time` seconds after the start (times ascending), its segment shaped by
    /// `curve` (Bezier/table/steps use the segment's duration; a spring that has not settled by the next stop
    /// jumps to it). A target sets its channel and keeps the others from the stop before. Driven as
    /// `animate` is (patch path, no rebuild, no allocation once warm; reduced motion jumps to the
    /// last stop); `animate` is the one-key case. `play` says how often the run repeats (a count includes the reverse leg), whether
    /// each cycle plays back and how many seconds into the first cycle it starts. Hidden runs keep their timeline but do not schedule
    /// frames. Retarget to stop. `false` for a stale handle.
    pub fn animate_keys(&mut self, b: &Handle, keys: &[Keyframe], play: &Keyframes) -> bool {
        let repeat = match play.cycles() { Repeat::Count(n) => n as u32, Repeat::Forever => 0 };
        let (autoreverse, offset) = (play.reverses(), play.start());
        assert!(!keys.is_empty() && keys.len() <= MAX_KEYS, "Ui.animate_keys: 1 to {MAX_KEYS} keyframes, not {}", keys.len());
        assert!(keys.windows(2).all(|w| w[0].time() <= w[1].time()) && keys[0].time() >= 0.0, "Ui.animate_keys: keyframe times ascend from 0");
        assert!(repeat <= u16::MAX as u32 && offset.is_finite() && offset >= 0.0);
        if !self.live(b) {
            return false;
        }
        if keys.iter().any(|key| matches!(key.target(), Target::Scale { .. })) && !self.view_id(b).is_some_and(|v| self.views[v as usize].index != 0) { return false; }
        let slot = b.box_slot;
        let from = self.pose(slot).0;
        let mut k = Keys { n: 0, t: [0.0; MAX_KEYS], pose: [from; MAX_KEYS], curve: [Curve::Bezier { duration: 0.0, points: [0.0, 0.0, 1.0, 1.0] }; MAX_KEYS], repeat, autoreverse, offset, rate: play.repaint_rate() };
        let mut at = from;
        for key in keys {
            let (t, curve) = (key.time(), key.segment());
            match key.target() {
                Target::Look(look) => at.look = self.resolve_look(&look).unwrap_or(look),
                Target::Offset(dx, dy) => (at.dx, at.dy) = (dx, dy),
                Target::Opacity(o) => at.opacity = o.clamp(0.0, 1.0),
                Target::Scale { x, y, anchor } => { at.scale = [x, y]; at.anchor = [anchor.0, anchor.1]; },
            }
            // Channels at the same timestamp are one pose, so they interpolate together.
            let j = if k.n > 0 && k.t[k.n - 1] == t { k.n - 1 } else { k.n += 1; k.n - 1 };
            (k.t[j], k.pose[j], k.curve[j]) = (t, at, curve);
        }
        let last = if self.theme().tokens.reduced_motion { 0.0 } else { k.t[k.n - 1] };
        if autoreverse { at = if k.t[0] == 0.0 { k.pose[0] } else { from }; }
        self.start_tween(slot, Curve::Bezier { duration: last, points: [0.0, 0.0, 1.0, 1.0] }, from, at);
        let tween = self.tween_of(slot).expect("the run just started");
        self.tweens[tween].view_only &= keys.iter().all(|key| !matches!(key.target(), Target::Look(_)));
        if last > 0.0 {
            let id = match self.free_keyframes.pop() {
                Some(id) => {
                    self.keyframes[id as usize] = k;
                    id
                }
                None => {
                    self.keyframes.push(k);
                    // Returning a completed animation must not allocate on a warm frame.
                    self.free_keyframes.reserve(self.keyframes.len().saturating_sub(self.free_keyframes.len()));
                    self.keyframes.len() as u32 - 1
                }
            };
            let i = self.tween_of(slot).expect("the run just started");
            self.tweens[i].keys = id;
        }
        if !self.building {
            self.frame_requested = true;
        }
        true
    }

    /// Transitions running now.
    pub fn transitions(&self) -> usize {
        self.tweens.len()
    }
}

// ------------------------------------------------------ views (layer 2)

/// Rows built past each edge of a virtual list's visible rows.
const OVERSCAN: usize = 4;
/// The nodes a new view record has room for, built and kept.
const VIEW_NODES: usize = 32;

/// IEEE half bits as f32.
fn half(h: u16) -> f32 {
    let magnitude = u32::from(h & 0x7fff);
    let value = match magnitude {
        0..=0x03ff => magnitude as f32 / 16_777_216.0,
        0x7c00.. => f32::INFINITY,
        _ => f32::from_bits((magnitude << 13) + (112 << 23)),
    };
    if h & 0x8000 != 0 { -value } else { value }
}

/// A patch rewrote instances `start..old_end` of `target` (its tail moved by `delta`) with `fresh`'s
/// first `node_end - start`: the breaks before it stay, the node's are `fresh`'s (from the atlas on,
/// as its paint assumed), the texture in effect after the old node is restored after the new one,
/// and the later breaks follow the tail.
fn patch_breaks(target: &mut Layer, old: &[Break], fresh: &Layer, start: usize, old_end: usize, node_end: usize, delta: i64) {
    use crate::draw::ATLAS;
    let active = |upto: &dyn Fn(usize) -> bool| old.iter().filter(|b| upto(b.at_instance as usize)).last().map_or(ATLAS, |b| b.image);
    let (before, after) = (active(&|at| at < start), active(&|at| at <= old_end));
    target.breaks.clear();
    target.breaks.extend(old.iter().filter(|b| (b.at_instance as usize) < start).copied());
    let push = |breaks: &mut Vec<Break>, at: usize, image: u32| match breaks.last_mut() {
        Some(last) if last.at_instance as usize == at => last.image = image,
        Some(last) if last.image == image => {}
        None if image == ATLAS => {}
        _ => breaks.push(Break { at_instance: at as u32, image }),
    };
    if before != ATLAS {
        push(&mut target.breaks, start, ATLAS);
    }
    for b in &fresh.breaks {
        let image = if b.image == ATLAS {
            ATLAS
        } else {
            let t = &fresh.images[b.image as usize];
            match target.images.iter().position(|i| Rc::ptr_eq(&i.handle, &t.handle)) {
                Some(k) => k as u32,
                None => {
                    target.images.push(t.clone());
                    target.images.len() as u32 - 1
                }
            }
        };
        push(&mut target.breaks, start + b.at_instance as usize, image);
    }
    push(&mut target.breaks, node_end, after);
    for b in old.iter().filter(|b| b.at_instance as usize > old_end) {
        push(&mut target.breaks, (b.at_instance as i64 + delta) as usize, b.image);
    }
}

/// What an instance can cover, in its own view's points (an empty instance covers nothing).
fn bounds(i: &Instance) -> Option<Rect> {
    if *i == EMPTY { return None; }
    let [x0, y0, x1, y1] = i.clip.map(|v| v as f32);
    let r = geometry_bounds(i).intersect(Rect::new(x0, y0, x1 - x0, y1 - y0));
    (r.w > 0.0 && r.h > 0.0 && r.x.is_finite() && r.y.is_finite()).then_some(r)
}

/// Instance coverage before clipping. The renderer adds the device-pixel AA margin after mapping it.
pub fn cull_bounds(i: &Instance) -> [f32; 4] {
    use crate::draw::{flags, kind};
    if *i == EMPTY { return EMPTY_BOUNDS; }
    let [x0, y0, x1, y1] = match i.kind {
        kind::LINE | kind::CURVE | kind::TRIANGLE | kind::ARC | kind::SHADOW => {
            let r = geometry_bounds(i);
            [r.x, r.y, r.x + r.w, r.y + r.h]
        }
        _ => i.rect,
    };
    let pad = match i.kind {
        kind::MASK if i.flags & flags::mask::LIGHT != 0 => 1.0 / half((i.extra[0] >> 16) as u16).max(1.0),
        // These shaders can write to the edge of the padded vertex quad.
        kind::GRID | kind::SHADOW => 1.0,
        _ => 0.0,
    };
    let edges = [x0 - pad, y0 - pad, x1 + pad, y1 + pad];
    if edges.iter().all(|v| v.is_finite()) { edges } else { [f32::NEG_INFINITY, f32::NEG_INFINITY, f32::INFINITY, f32::INFINITY] }
}

fn geometry_bounds(i: &Instance) -> Rect {
    use crate::draw::kind;
    let [a, b, c, d] = i.rect;
    let span = |x0: f32, y0: f32, x1: f32, y1: f32, grow: f32| Rect::new(x0 - grow, y0 - grow, x1 - x0 + 2.0 * grow, y1 - y0 + 2.0 * grow);
    match i.kind {
        kind::LINE => span(a.min(c), b.min(d), a.max(c), b.max(d), half(i.width)),
        kind::CURVE | kind::TRIANGLE => {
            let (cx, cy) = (f32::from_bits(i.extra[0]), f32::from_bits(i.extra[1]));
            span(a.min(c).min(cx), b.min(d).min(cy), a.max(c).max(cx), b.max(d).max(cy), half(i.width))
        }
        kind::ARC => span(a - c, b - c, a + c, b + c, half(i.width)),
        kind::SHADOW => {
            let (dx, dy) = (half(i.extra[0] as u16), half((i.extra[0] >> 16) as u16));
            let grow = 3.0 * half(i.extra[1] as u16) + half((i.extra[1] >> 16) as u16).abs();
            span(a.min(a + dx), b.min(b + dy), c.max(c + dx), d.max(d + dy), grow)
        }
        _ => Rect::new(a, b, c - a, d - b),
    }
}

fn union(a: Option<Rect>, b: Rect) -> Rect {
    match a {
        None => b,
        Some(a) => {
            let (x0, y0) = (a.x.min(b.x), a.y.min(b.y));
            Rect::new(x0, y0, (a.x + a.w).max(b.x + b.w) - x0, (a.y + a.h).max(b.y + b.h) - y0)
        }
    }
}

impl Ui {
    /// A cached subtree: `f` builds its children, and is skipped entirely while `revision` is the
    /// one it was last built with and nothing else dirtied it; its nodes, their layout, hit areas
    /// and instance run stay as they were. Its size comes from outside (`Px`, `Pct`, `Rel`, `Grow`)
    /// or, on a `Fit` side, from its content: kept, it is laid out at the content size its last
    /// build measured; rebuilt, it is measured in that frame's layout with the views around it
    /// (which a rebuild always rebuilds), and kept views it moves follow through their table entries
    /// (see `specification/pxui.md`, content-sized views). Besides the revision it rebuilds when the theme, the font size, the
    /// density or the window size change; when input reaches one of its nodes (press, release, drag,
    /// wheel, keys); when the hovered or focused node enters or leaves it, or carrying starts or
    /// stops over it; every frame it holds the keyboard focus or a scroll in motion; when its
    /// laid-out size changes (shown stale for that one frame); and on the frame after a build that
    /// put boxes at the root (popups, tooltips, `to_front`), asked for a frame or ran a hover delay.
    /// Pointer motion inside one node rebuilds nothing: a view that draws from the pointer's
    /// position bumps its own revision. A rebuild of the view rebuilds the views around it; the
    /// views inside it may still be kept. Returns the view's box, for `view_offset` and friends.
    pub fn view(&mut self, label: &str, revision: u64, w: Size, h: Size, f: &mut dyn FnMut(&mut Ui)) -> Handle {
        self.view_entry(label, revision, w, h, true, f)
    }

    /// `view` that takes no painter table entry of its own: its instances ride its parent's entry
    /// exactly as the parent's own would (many small views that never move apart from their parent,
    /// one per item of a list, say: each entry is 64 bytes the painter writes every frame). `view_offset` and the other entry
    /// changes on it are baked by a rebuild.
    pub fn view_flat(&mut self, label: &str, revision: u64, w: Size, h: Size, f: &mut dyn FnMut(&mut Ui)) -> Handle {
        self.view_entry(label, revision, w, h, false, f)
    }

    /// The authoring view (the wrapper's `Ui::view_of`): a `view` keyed by `key` that fills its parent, so its size comes from the
    /// enclosing node. On the main axis of a row or column whose size there is `Fit` (where filling leaves nothing to fill) it takes
    /// its content's size instead, as a content-sized view. `revision` is the hash of what `f` reads; the view keeps its build while it
    /// is unchanged. It always has its own painter table entry.
    pub fn view_of(&mut self, key: &str, revision: &u64, f: &mut dyn FnMut(&mut Ui)) -> Handle {
        let parent = self.nd(self.current_parent());
        let along = |axis: Axis, size: Size| if parent.axis == axis && matches!(size, Size::Fit) { Size::Fit } else { Size::Grow };
        let (w, h) = (along(Axis::Row, parent.w), along(Axis::Column, parent.h));
        self.view_entry(key, *revision, w, h, true, f)
    }

    /// The current view's positioned labels declare constant content and use palette tokens for color.
    pub fn retain_palette_labels(&mut self) {
        if self.building && self.view_cur != 0 {
            self.views[self.view_cur as usize].palette_labels = true;
        }
    }

    /// The current view's paint uses explicit color sources and palette-independent geometry.
    pub fn retain_palette_paint(&mut self) {
        if self.building && self.view_cur != 0 && self.theme_id == 0 {
            self.views[self.view_cur as usize].palette_paint = true;
        }
    }

    /// The current view's build only uses its local extent and its revision.
    pub fn retain_view_on_resize(&mut self) {
        if self.building && self.view_cur != 0 {
            self.views[self.view_cur as usize].viewport_independent = true;
        }
    }

    pub(crate) fn view_entry(&mut self, label: &str, revision: u64, w: Size, h: Size, entry: bool, f: &mut dyn FnMut(&mut Ui)) -> Handle {
        let b = self.template_box(label);
        let n = self.slots[b.box_slot].node.prepare(NodeTemplate::View);
        (n.w, n.h) = (w, h);
        self.clear_box_text(b.box_slot);
        self.view_in(b, label, revision, entry, false, f)
    }

    /// `view_entry` on the box `b` the caller declared this frame (a list's row box): the box holds the view (`label` names its record);
    /// with `entered` the caller has entered `b` already, so `f` builds in it as it is.
    pub(crate) fn view_in(&mut self, b: Handle, label: &str, revision: u64, entry: bool, entered: bool, f: &mut dyn FnMut(&mut Ui)) -> Handle {
        let parent = self.view_cur;
        let v = match self.slots[b.box_slot].view {
            NO_VIEW => {
                // Reopen the same view kind with its warmed node/hit buffers.
                let matching = self.free_views.iter().rposition(|&v| self.views[v as usize].name == label);
                // No label match: take the smallest record, so a newcomer does not strip a named view's big
                // warmed buffers that its own label (a layout flipped away and back) reopens later.
                let reused = matching.or_else(|| {
                    (0..self.free_views.len()).min_by_key(|&i| {
                        let r = &self.views[self.free_views[i] as usize];
                        r.nodes.capacity() + r.prev.capacity()
                    })
                }).map(|i| self.free_views.swap_remove(i));
                let v = reused.unwrap_or_else(|| {
                    // Recycled records change labels as adaptive subtrees enter and leave.
                    // room for a list row's nodes: a recycled record seldom grows when a view of another size takes it
                    self.views.push(ViewRec { name: String::with_capacity(label.len().max(32)), nodes: Vec::with_capacity(VIEW_NODES), prev: Vec::with_capacity(VIEW_NODES), hits: Vec::with_capacity(VIEW_NODES / 4), ..ViewRec::new() });
                    self.palette_shapes.push([PaletteShape::default(); PALETTE_SHAPES]);
                    self.views.len() as u32 - 1
                });
                // a table index of its own (unless flat): a freed one, else the next (the painter's
                // table grows to the 16-bit instance field; past that a view bakes its offset)
                let index = if !entry {
                    0
                } else {
                    self.free_indices.pop().unwrap_or_else(|| {
                        let next = self.index_owner.len();
                        if next > u16::MAX as usize {
                            return 0;
                        }
                        self.index_owner.push(0);
                        next as u16
                    })
                };
                let r = &mut self.views[v as usize];
                r.name.clear();
                r.name.push_str(label);
                (r.slot, r.index) = (b.box_slot, index);
                if index != 0 {
                    self.index_owner[index as usize] = v;
                }
                self.slots[b.box_slot].view = v;
                v
            }
            v => v,
        };
        self.boxes[b.index].view = v;
        let xf = if self.views[v as usize].index != 0 { v } else { self.views[parent as usize].xf };
        let parent_reflow = self.views[parent as usize].reflow;
        let r = &mut self.views[v as usize];
        (r.parent, r.xf) = (parent, xf);
        r.reflow |= parent_reflow;
        // a view culled since its build has no run: kept while it stays out of view (a wrong guess rebuilds it on the settling
        // build, as a kept view with no run in its layer)
        let shown = r.valid || (self.out_of_view && r.index != 0);
        let clean = !r.reflow && r.built >= 0 && shown && !r.dirty && !r.escapes && r.revision == revision && r.epoch == self.epoch && r.theme == self.theme_id;
        if clean && !self.verify {
            r.kept = self.frame_number;
            self.boxes[b.index].kept = true;
            return b;
        }
        r.verify = clean;
        r.escapes = false;
        r.palette_labels = false;
        r.palette_paint = false;
        r.palette_shape_count = 0;
        r.viewport_independent = false;
        self.rebuilt += 1;
        r.wake = f64::INFINITY;
        std::mem::swap(&mut r.nodes, &mut r.prev);
        r.nodes.clear();
        self.a11y_dirty = true;
        self.view_cur = v;
        if entered {
            f(self);
        } else {
            let saved = self.enter(&b);
            f(self);
            self.leave(saved);
        }
        self.view_cur = parent;
        let r = &mut self.views[v as usize];
        (r.revision, r.epoch, r.theme, r.built) = (revision, self.epoch, self.theme_id, self.frame_number);
        // a verifying run keeps what was patched since the kept build (it is not checked against it)
        r.patched &= r.verify;
        if r.palette_labels {
            r.palette_labels = r.nodes.iter().all(|&s| {
                self.slots[s].node.template == NodeTemplate::PositionedLabel && self.slots[s].label_root_ink
            }) && self.boxes[b.index + 1..self.count].iter().all(|b| b.paint == arena::EMPTY && b.over == arena::EMPTY && b.view == NO_VIEW);
        }
        r.dirty = r.escapes;
        r.reflow = false;
        // the next build fills `prev`: room now, so a view's second build does not grow it
        r.prev.reserve(r.nodes.len().saturating_sub(r.prev.len()));
        if r.escapes && parent != 0 {
            self.views[parent as usize].escapes = true;
        }
        b
    }

    /// A scrolling list of `count` rows `row_h` points high, filling its parent (`Grow` both ways),
    /// that builds only the rows in view and `OVERSCAN` more past each edge: `f(ui, i)` builds row
    /// `i`'s content inside a row box keyed by `i`, so a row's state and handles stay while it is
    /// built. Returns the list's scroll box.
    // Rows are chosen from the scroll position before layout: a fling past the overscan shows empty
    // rows for a frame. Rows of differing heights: `list_measured`.
    pub fn list(&mut self, label: &str, count: usize, row_h: f32, f: &mut dyn FnMut(&mut Ui, usize)) -> Handle {
        self.list_keyed(label, count, row_h, None, f)
    }

    /// `list` with row `i`'s box keyed by the item id `ids(i)` when `ids` is given (as `section_list`'s `row_key`), else by its index.
    pub fn list_keyed(&mut self, label: &str, count: usize, row_h: f32, ids: Option<&dyn Fn(usize) -> i64>, f: &mut dyn FnMut(&mut Ui, usize)) -> Handle {
        let padding = self.list_padding(key_of(self.current_seed(), label));
        assert!(row_h > 0.0, "Ui.list {label}: rows need a positive height");
        let body = self.node_ref(label, &Node { flags: Flags::SCROLL | Flags::CLIP, w: Size::Grow, h: Size::Grow, scroll_step: row_h, role: Role::List, ..Node::DEFAULT });
        let seen = self.slots[body.box_slot].rh;
        let height = if seen > 0.0 { seen } else { self.view_h };
        // the wheel and the trackpad move the list before its rows are chosen (as `table` does)
        self.set_l_h(&body, height);
        self.set_l_content(&body, count as f32 * row_h + padding.t + padding.b);
        if let Some(t) = self.input_frame_time() {
            self.apply_scroll(true, t, body.index);
        }
        let top = (self.scroll_offset(&body) / row_h).floor().max(0.0) as usize;
        let first = top.saturating_sub(OVERSCAN).min(count);
        let last = (top + (height / row_h).ceil() as usize + 1 + OVERSCAN).min(count);
        let saved = self.enter(&body);
        let extent = self.node_keyed(int_key(body.box_key, -1), Node { h: Size::Px(count as f32 * row_h), ..Node::DEFAULT });
        let inner = self.enter(&extent);
        self.slots[extent.box_slot].holds_rows = true;
        self.text_mut().list_window(body.box_key as u64, first, last);
        let outer = (self.ahead, self.out_of_view);
        let shown = top..top + (height / row_h).ceil() as usize + 1;
        for i in first..last {
            self.ahead_row(outer, &shown, false, i);
            let row = self.list_row(body.box_key, i, ids, Node { h: Size::Px(row_h), at: Some((0.0, i as f32 * row_h)).into(), role: Role::ListItem, ..Node::DEFAULT });
            self.slots[row.box_slot].row_of = (body.box_key, i as u32);
            let saved = self.enter(&row);
            f(self, i);
            self.leave(saved);
        }
        (self.ahead, self.out_of_view) = outer;
        self.leave(inner);
        self.leave(saved);
        self.remember_list(&body, ListGeometry::rows(&body, count, row_h, 0.0, 0.0, 0.0, padding));
        body
    }

    /// A scrolling lead-in, sticky heading and virtual rows (fixed, or measured with `SectionList::measured`) in one region keyed by
    /// `label`; `f(ui, index, y)` builds the part `index` (`usize::MAX` the lead-in, `usize::MAX - 1` the heading, else row `index`) at its
    /// logical content Y. With `row_key`, row `i`'s box is keyed by the item id `key(i)` (`Key::Id` under the rows), so a row's state
    /// follows its item when the items reorder; without it, by its index. With `trailer`, the space after the last row is built too
    /// (`usize::MAX - 2`); without it (the 1.x C contract: every other index is a row) it stays empty.
    pub fn section_list(&mut self,label:&str,section:&pxui_types::ui::SectionList,row_key:Option<&dyn Fn(usize)->i64>,trailer:bool,f:&mut dyn FnMut(&mut Ui,usize,f32))->Handle {
        (self.list_drop,self.list_moved)=(None,None);
        if section.measured { return self.section_rows_measured(label,section,row_key,trailer,f); }
        let y=|index:usize| match index { usize::MAX=>0.0, i if i==usize::MAX-1=>section.leading_height, i if i==usize::MAX-2=>section.leading_height+section.header_height+section.count as f32*section.row_height, i=>section.leading_height+section.header_height+i as f32*section.row_height };
        self.section_rows_fixed(label,section,row_key,trailer,&mut |ui,index| f(ui,index,y(index)))
    }

    /// On open and window resize, start with the declared room of a resting list in a fixed-height pane.
    /// Layout still checks the guess: a column may gain more children after the list's callback returns.
    fn section_height(&self, body: &Handle) -> f32 {
        let slot = body.box_slot;
        let seen = &self.slots[slot];
        let fallback = if seen.rh > 0.0 || seen.rw > 0.0 { seen.rh } else { self.view_h };
        if !(self.height_changed || seen.new_identity)
            || self.scroll_y[slot] != 0.0 || self.glides[0][slot].visual != 0.0 || self.glides[0][slot].moving()
            || self.signals.get(&seen.key).is_some_and(|s| s.scroll_wheel != 0.0 || s.scroll_drag != 0.0 || s.scroll_touch || s.scroll_lift)
            || self.payload.is_some()
        {
            return fallback;
        }
        let mut index = body.index;
        let mut inset = 0.0;
        while index != 0 {
            let node = self.nd(index);
            if let Size::Px(h) = node.h {
                return (clamp(h, node.min_h, node.max_h) - inset).max(0.0);
            }
            let parent = self.boxes[index].parent;
            let pane = self.nd(parent);
            if !node.h.is_grow() || node.grow <= 0.0 || !self.flow(index)
                || node.min_h != 0.0 || node.max_h != f32::INFINITY
                || pane.wrap || pane.grid.is_some() || pane.flags.any(Flags::SCROLL)
            {
                break;
            }
            if pane.axis == Axis::Column {
                let mut sibling = self.next_flow(self.boxes[parent].first);
                while sibling != index {
                    let other = self.nd(sibling);
                    let Size::Px(height) = other.h else { return fallback; };
                    inset += clamp(height, other.min_h, other.max_h) + other.margin.t + other.margin.b + pane.gap;
                    sibling = self.next_flow(self.boxes[sibling].next);
                }
            }
            inset += node.margin.t + node.margin.b + pane.padding.t + pane.padding.b;
            index = parent;
        }
        fallback
    }

    fn section_rows_fixed(&mut self,label:&str,section:&pxui_types::ui::SectionList,row_key:Option<&dyn Fn(usize)->i64>,trailer:bool,f:&mut dyn FnMut(&mut Ui,usize))->Handle {
        let padding = self.list_padding(key_of(self.current_seed(), label));
        let rows_h=section.count as f32*section.row_height;
        let before=section.leading_height+section.header_height;
        let total=before+rows_h+section.trailing_height;
        assert!(section.row_height>0.0 && section.row_height.is_finite() && total.is_finite()
            && [section.leading_height,section.header_height,section.trailing_height].iter().all(|v|v.is_finite() && *v>=0.0),"Ui.section_list: invalid section geometry");
        let flags=Flags::SCROLL|Flags::CLIP|if section.overlay_scrollbar {Flags::OVER_THUMB} else {Flags::NONE};
        let body=self.node_ref(label, &Node {flags,w:Size::Grow,h:Size::Grow,scroll_step:section.row_height,role:Role::List,..Node::DEFAULT});
        let height = self.section_height(&body);
        // rows that all fit at rest keep fitting while the room still holds them (the grid's exemption, one column wide)
        let grid=(height-padding.t-padding.b>=total).then_some(GridFit{extent:total,across:f32::INFINITY,per:1,padding,sideways:false});
        self.record_size(ExpectedSize{slot:body.box_slot,w:f32::NAN,h:height,grid});
        self.set_l_h(&body,height);
        self.set_l_content(&body,total+padding.t+padding.b);
        if let Some(time)=self.input_frame_time() {self.apply_scroll(true,time,body.index);}
        let offset=self.scroll_position(&body);
        let top=((offset-before)/section.row_height).floor().max(0.0) as usize;
        let bottom=((offset+height-before)/section.row_height).ceil().max(0.0) as usize;
        let first=top.saturating_sub(OVERSCAN).min(section.count);
        let last=bottom.saturating_add(OVERSCAN+1).min(section.count);
        let saved=self.enter(&body);
        for (key,h,flags,index) in [("leading",section.leading_height,Flags::NONE,usize::MAX),("header",section.header_height,Flags::STICKY|Flags::BLOCKING,usize::MAX-1)] {
            if h>0.0 {
                let part=self.node_ref(key, &Node {flags,w:Size::Grow,h:Size::Px(h),..Node::DEFAULT});
                let inner=self.enter(&part); f(self,index); self.leave(inner);
            }
        }
        let extent=self.node("rows".into(),Node {w:Size::Grow,h:Size::Px(rows_h),..Node::DEFAULT});
        let inner=self.enter(&extent);
        self.slots[extent.box_slot].holds_rows = true;
        self.text_mut().list_window(body.box_key as u64,first,last);
        let (outer,mut line)=((self.ahead,self.out_of_view),None);
        let mut moves=(section.reorder|core::mem::take(&mut self.row_moves)).then(|| self.rows_move_begin(&body,&extent,rows_h,section.count));
        let lifted=self.lifted_row(&body,section.count);
        for index in (first..last).chain(lifted.filter(|i| !(first..last).contains(i))) {
            // the lifted row follows the pointer: in view wherever its index is
            self.ahead_row(outer,&(top..bottom),false,index);
            self.out_of_view&=Some(index)!=lifted;
            let at=index as f32*section.row_height;
            let node=Node {w:Size::Grow,h:Size::Px(section.row_height),at:Some((0.0,at)).into(),role:Role::ListItem,..Node::DEFAULT};
            let row=self.list_row(body.box_key,index,row_key,node);
            self.slots[row.box_slot].row_of = (body.box_key, index as u32);
            self.build_row_part(&row,moves.as_mut(),section.reorder,index,at,section.row_height,&mut |ui| f(ui,index));
            if section.reorder { line=line.or(self.row_insertion(&row,index,at,section.row_height)); }
        }
        (self.ahead,self.out_of_view)=outer;
        if let Some(y)=line { self.insertion_line(&extent,y); }
        if let Some(m)=moves.as_mut() { self.rows_move_end(m,&body); self.list_moved=m.moved; }
        self.leave(inner);
        if section.trailing_height>0.0 {
            let t=self.node("trailing".into(),Node {w:Size::Grow,h:Size::Px(section.trailing_height),..Node::DEFAULT});
            if trailer {let inner=self.enter(&t); f(self,usize::MAX-2); self.leave(inner);}
        }
        self.leave(saved);
        self.remember_list(&body,ListGeometry::rows(&body,section.count,section.row_height,before,section.header_height,section.trailing_height,padding));
        body
    }

    /// Where a payload carried over the reorderable list row `row` (index `index`, `top` and `h` its place in the rows) would land: the
    /// insertion index before it (pointer in its upper half) or after it, kept in `list_drop` with whether it was dropped; the
    /// insertion line's Y in the rows while it is only carried. Allocates: never.
    pub(crate) fn row_insertion(&mut self, row: &Handle, index: usize, top: f32, h: f32) -> Option<f32> {
        let (after, dropped) = self.drop_side(row, index)?;
        (!dropped).then_some(if after { top + h } else { top })
    }

    /// A payload carried over the list row `row` (index `index`): whether it would land after the row (the pointer in its lower half)
    /// and whether it was dropped; the insertion index goes to `list_drop`.
    pub(crate) fn drop_side(&mut self, row: &Handle, index: usize) -> Option<(bool, bool)> {
        let dropped = self.drop_target_ref(row)?.0;
        let r = self.rect(row);
        let after = self.pointer.1 >= r.y + r.h / 2.0;
        self.list_drop = Some((index + usize::from(after), dropped));
        Some((after, dropped))
    }

    /// The insertion index a carried payload was dropped at in the reorderable list built last (`section_list`), this frame.
    pub fn list_dropped(&self) -> Option<usize> { self.list_drop.filter(|d| d.1).map(|d| d.0) }

    /// The row move `(from, to)` the reorderable list built last (`section_list`) reported this frame.
    pub fn list_moved(&self) -> Option<(usize, usize)> { self.list_moved }

    /// The 2-point insertion line at `y` in the rows `extent`, in the theme's accent, drawn over the rows.
    fn insertion_line(&mut self, extent: &Handle, y: f32) {
        let look = Look { fill: self.theme().tokens.accent, ..Look::NONE };
        self.node_keyed(int_key(extent.box_key, -2), Node { w: Size::Grow, h: Size::Px(2.0), at: Some((0.0, (y - 1.0).max(0.0))).into(), look, ..Node::DEFAULT });
    }

    /// Row (or grid line) `index` of a virtual list builds past the viewport rows `shown` while the prepare queue runs (or
    /// inside such a row, `outer.0`): its nodes' unseen text goes to the queue instead of shaping (`BoxRec::ahead`), so it is
    /// ready when the row scrolls into view; `below`: only rows below the viewport (measured rows, whose heights do not move the
    /// rows in view). Outside `shown` (or inside such a row, `outer.1`) it is culled, so a kept view in it keeps its build
    /// (`out_of_view`).
    fn ahead_row(&mut self, outer: (bool, bool), shown: &core::ops::Range<usize>, below: bool, index: usize) {
        let past = if below { index >= shown.end } else { !shown.contains(&index) };
        self.ahead = outer.0 || (past && self.text.as_ref().is_some_and(|t| t.queue_running()));
        self.out_of_view = outer.1 || !shown.contains(&index);
    }

    /// Attaches this `Ui` to the prepare queue's `worker` (shared with any number of `Ui`s; `None` detaches): from then on its
    /// virtual lists queue the unseen text of the rows they build past the viewport, and `prewarm_text` queues. Allocates: a
    /// fork of this `Ui`'s fonts and the lane's fixed buffers, once per attach; detaching does not wait for the worker. `false`:
    /// the worker serves `pxui_text::MAX_LANES` `Ui`s already (this one is left detached).
    pub fn set_text_worker(&mut self, worker: Option<&pxui_text::Worker>) -> bool {
        self.text_mut().attach(worker)
    }

    /// Text a coming frame will draw, prepared now on the attached worker (`set_text_worker`) and installed before a later
    /// frame builds: `text` in `style` within `width` points (0: unbounded), the layout a node shows when it lays out the same
    /// text, style and width. Width 0 is the natural layout, which a node drawn at a
    /// width of at least the natural one shows too (`Text::reusable_layout`). Nothing when no worker is attached, the text is cached or already requested; a full lane drops
    /// the request. `false` (nothing done) for a negative or non-finite width, or text over `pxui_text::QUEUE_BYTES`.
    /// Allocates: never (the lane's fonts refork after a font change, at the end of the frame).
    pub fn prewarm_text(&mut self, text: &str, style: &Style, width: f32) -> bool {
        if !width.is_finite() || width < 0.0 || text.len() > pxui_text::QUEUE_BYTES {
            return false;
        }
        if !text.is_empty() {
            self.text_mut().defer(pxui_text::content_key(style, width, text, &[]), text, style, width, &[]);
        }
        true
    }

    /// Row `index`'s box of the section list `list`: keyed by its item id when the list has `key`, else by its index.
    fn list_row(&mut self,list:i64,index:usize,key:Option<&dyn Fn(usize)->i64>,node:Node)->Handle {
        match key {
            Some(key)=>self.node(pxui_types::ui::Key::Id(key(index)),node),
            None=>self.node_keyed(int_key(list,index as i64),node),
        }
    }

    fn section_rows_measured(&mut self,label:&str,section:&pxui_types::ui::SectionList,row_key:Option<&dyn Fn(usize)->i64>,trailer:bool,f:&mut dyn FnMut(&mut Ui,usize,f32))->Handle {
        let key=key_of(self.current_seed(),label);
        let padding=self.list_padding(key);
        let mut cache=self.list_heights.remove(&key).unwrap_or_default();
        if cache.heights.len()!=section.count {cache.heights.resize(section.count,section.row_height);cache.dirty=true;}
        if cache.dirty || cache.tops.len()!=section.count+1 {
            cache.tops.clear();cache.tops.push(0.0);
            for index in 0..section.count {cache.tops.push(cache.tops[index]+cache.heights[index]);}
            cache.dirty=false;
        }
        cache.frame=self.frame_number;
        let rows_h=cache.tops[section.count];
        let before=section.leading_height+section.header_height;
        let total=before+rows_h+section.trailing_height;
        assert!(section.row_height>0.0 && section.row_height.is_finite() && total.is_finite()
            && [section.leading_height,section.header_height,section.trailing_height].iter().all(|v|v.is_finite() && *v>=0.0),"Ui.section_list: invalid section geometry");
        let flags=Flags::SCROLL|Flags::CLIP|if section.overlay_scrollbar {Flags::OVER_THUMB} else {Flags::NONE};
        let body=self.node_ref(label, &Node {flags,w:Size::Grow,h:Size::Grow,scroll_step:section.row_height,role:Role::List,..Node::DEFAULT});
        let height = self.section_height(&body);
        self.expect(body.box_slot,f32::NAN,height);
        self.set_l_h(&body,height);
        self.set_l_content(&body,total+padding.t+padding.b);
        if let Some(time)=self.input_frame_time() {self.apply_scroll(true,time,body.index);}
        let offset=self.scroll_position(&body);
        let top=cache.tops.partition_point(|y| *y<=offset-before).saturating_sub(1).min(section.count);
        let bottom=cache.tops.partition_point(|y| *y<offset+height-before).min(section.count);
        let first=top.saturating_sub(OVERSCAN).min(section.count);
        let last=bottom.saturating_add(OVERSCAN+1).min(section.count);
        let saved=self.enter(&body);
        for (key,h,flags,index) in [("leading",section.leading_height,Flags::NONE,usize::MAX),("header",section.header_height,Flags::STICKY|Flags::BLOCKING,usize::MAX-1)] {
            if h>0.0 {
                let part=self.node_ref(key, &Node {flags,w:Size::Grow,h:Size::Px(h),..Node::DEFAULT});
                let inner=self.enter(&part); f(self,index,if index==usize::MAX {0.0} else {section.leading_height}); self.leave(inner);
            }
        }
        let extent=self.node("rows".into(),Node {w:Size::Grow,h:Size::Px(rows_h),..Node::DEFAULT});
        let inner=self.enter(&extent);
        self.slots[extent.box_slot].holds_rows = true;
        self.text_mut().list_window(body.box_key as u64,first,last);
        let (outer,mut line)=((self.ahead,self.out_of_view),None);
        let mut moves=(section.reorder|core::mem::take(&mut self.row_moves)).then(|| self.rows_move_begin(&body,&extent,rows_h,section.count));
        let lifted=self.lifted_row(&body,section.count);
        for index in (first..last).chain(lifted.filter(|i| !(first..last).contains(i))) {
            self.ahead_row(outer,&(top..bottom),true,index);
            self.out_of_view&=Some(index)!=lifted;
            let at=cache.tops[index];
            let node=Node {w:Size::Grow,h:Size::Fit,at:Some((0.0,at)).into(),role:Role::ListItem,..Node::DEFAULT};
            let row=self.list_row(body.box_key,index,row_key,node);
            self.slots[row.box_slot].row_of = (body.box_key, index as u32);
            self.list_rows.push((body.box_key,index,row.box_slot));
            self.build_row_part(&row,moves.as_mut(),section.reorder,index,at,cache.heights[index],&mut |ui| f(ui,index,before+at));
            if section.reorder { line=line.or(self.row_insertion(&row,index,at,cache.heights[index])); }
        }
        (self.ahead,self.out_of_view)=outer;
        if let Some(y)=line { self.insertion_line(&extent,y); }
        if let Some(m)=moves.as_mut() { self.rows_move_end(m,&body); self.list_moved=m.moved; }
        self.leave(inner);
        if section.trailing_height>0.0 {
            let t=self.node("trailing".into(),Node {w:Size::Grow,h:Size::Px(section.trailing_height),..Node::DEFAULT});
            if trailer {let inner=self.enter(&t); f(self,usize::MAX-2,before+rows_h); self.leave(inner);}
        }
        self.leave(saved);
        cache.geometry=ListGeometry::rows(&body,section.count,0.0,before,section.header_height,section.trailing_height,padding);
        self.list_heights.insert(body.box_key,cache);
        body
    }

    /// `list` with rows of their own (`Fit`) heights: each row is measured when it is built and its
    /// height cached by index (rows not yet built count `estimate_h`); the rows' tops are prefix sums
    /// of the cache, so the scroll position finds its row by binary search. A measured height that
    /// differs from the cached one settles the frame (`Ui::frame` builds again), so rows never
    /// overlap or gap. No allocation once warm (the cache grows with `count`).
    pub fn list_measured(&mut self, label: &str, count: usize, estimate_h: f32, f: &mut dyn FnMut(&mut Ui, usize)) -> Handle {
        let padding = self.list_padding(key_of(self.current_seed(), label));
        assert!(estimate_h > 0.0, "Ui.list_measured {label}: the estimate needs a positive height");
        let body = self.node_ref(label, &Node { flags: Flags::SCROLL | Flags::CLIP, w: Size::Grow, h: Size::Grow, scroll_step: estimate_h, role: Role::List, ..Node::DEFAULT });
        let mut cache = self.list_heights.remove(&body.box_key).unwrap_or_default();
        if cache.heights.len() != count {
            cache.heights.resize(count, estimate_h);
            cache.dirty = true;
        }
        if cache.dirty || cache.tops.len() != count + 1 {
            cache.tops.clear();
            cache.tops.push(0.0);
            for k in 0..count {
                let top = cache.tops[k] + cache.heights[k];
                cache.tops.push(top);
            }
            cache.dirty = false;
        }
        cache.frame = self.frame_number;
        let seen = self.slots[body.box_slot].rh;
        // a list never laid out guesses the window's height; one laid out to no height stays so (no rebuild every frame)
        let height = if seen > 0.0 || self.slots[body.box_slot].rw > 0.0 { seen } else { self.view_h };
        self.expect(body.box_slot, f32::NAN, height);
        self.set_l_h(&body, height);
        self.set_l_content(&body, cache.tops[count] + padding.t + padding.b);
        if let Some(t) = self.input_frame_time() {
            self.apply_scroll(true, t, body.index);
        }
        let offset = self.scroll_offset(&body);
        // the row at the offset, and the one past the bottom edge
        let top = cache.tops.partition_point(|y| *y <= offset).saturating_sub(1).min(count);
        let bottom = cache.tops.partition_point(|y| *y < offset + height).min(count);
        let (first, last) = (top.saturating_sub(OVERSCAN), (bottom + OVERSCAN).min(count));
        let saved = self.enter(&body);
        let extent = self.node_keyed(int_key(body.box_key, -1), Node { h: Size::Px(cache.tops[count]), ..Node::DEFAULT });
        let inner = self.enter(&extent);
        self.slots[extent.box_slot].holds_rows = true;
        self.text_mut().list_window(body.box_key as u64, first, last);
        let outer = (self.ahead, self.out_of_view);
        for i in first..last {
            self.ahead_row(outer, &(top..bottom), true, i);
            let row = self.node_keyed(int_key(body.box_key, i as i64), Node { h: Size::Fit, at: Some((0.0, cache.tops[i])).into(), role: Role::ListItem, ..Node::DEFAULT });
            self.slots[row.box_slot].row_of = (body.box_key, i as u32);
            self.list_rows.push((body.box_key, i, row.box_slot));
            let saved = self.enter(&row);
            f(self, i);
            self.leave(saved);
        }
        (self.ahead, self.out_of_view) = outer;
        self.leave(inner);
        self.leave(saved);
        cache.geometry = ListGeometry::rows(&body, count, 0.0, 0.0, 0.0, 0.0, padding);
        self.list_heights.insert(body.box_key, cache);
        body
    }

    /// A scrolling grid of `count` cells `cell_w` by `cell_h`, filling its parent (`Grow` both ways):
    /// as many columns as its width holds, in rows scrolling down, building only the rows in view
    /// and `OVERSCAN` more past each edge. `f(ui, i)` builds cell `i` inside a cell box keyed by `i`
    /// (its state stays while it is built); right to left the first cell is at the right. Returns the
    /// grid's scroll box.
    // Cells of one size; the columns come from last frame's width (the window's on the first): a
    // width layout changes settles the frame (`Ui::frame` builds again), so no old wrap shows.
    pub fn grid_list(&mut self, label: &str, count: usize, cell_w: f32, cell_h: f32, f: &mut dyn FnMut(&mut Ui, usize)) -> Handle {
        self.cells(label, count, (cell_w, cell_h), false, f)
    }

    /// `grid_list` turned: as many rows as its height holds, in columns scrolling sideways (`SCROLL_X`,
    /// from the right edge right to left).
    pub fn grid_list_x(&mut self, label: &str, count: usize, cell_w: f32, cell_h: f32, f: &mut dyn FnMut(&mut Ui, usize)) -> Handle {
        self.cells(label, count, (cell_w, cell_h), true, f)
    }

    fn cells(&mut self, label: &str, count: usize, cell: (f32, f32), sideways: bool, f: &mut dyn FnMut(&mut Ui, usize)) -> Handle {
        let padding = self.list_padding(key_of(self.current_seed(), label));
        assert!(cell.0 > 0.0 && cell.1 > 0.0, "Ui.grid_list {label}: cells need a positive size");
        let flags = Flags::CLIP | if sideways { Flags::SCROLL_X } else { Flags::SCROLL };
        // `along` is the scrolling axis, `across` the one the cells wrap in
        let (along, across) = if sideways { cell } else { (cell.1, cell.0) };
        let body = self.node_ref(label, &Node { flags, w: Size::Grow, h: Size::Grow, scroll_step: along, role: Role::List, ..Node::DEFAULT });
        let seen = (self.slots[body.box_slot].rw, self.slots[body.box_slot].rh);
        let (w, h) = (if seen.0 > 0.0 { seen.0 } else { self.view_w }, if seen.1 > 0.0 { seen.1 } else { self.view_h });
        let (room, room_across) = grid_space(w, h, padding, sideways);
        let per = ((room_across / across).floor() as usize).max(1);
        let lines = count.div_ceil(per);
        let extent = lines as f32 * along;
        // the wheel and the trackpad move the grid before its lines are chosen (as `list` does)
        (self.l_w[body.index], self.l_h[body.index]) = (w, h);
        if sideways {
            self.l_content_w[body.index] = extent + padding.l + padding.r;
        } else {
            self.l_content[body.index] = extent + padding.t + padding.b;
        }
        let offset = match (self.input_frame_time(), sideways) {
            (Some(t), true) => {
                self.apply_scroll_x(true, t, body.index);
                self.scroll_offset_x(&body)
            }
            (Some(t), false) => {
                self.apply_scroll(true, t, body.index);
                self.scroll_offset(&body)
            }
            (None, true) => self.scroll_offset_x(&body),
            (None, false) => self.scroll_offset(&body),
        };
        // When every cell fits at rest, only wrapping or newly scrollable content needs a new build.
        let glide = &self.glides[usize::from(sideways)][body.box_slot];
        let grid = (offset == 0.0 && glide.visual == 0.0 && !glide.moving() && room >= extent)
            .then_some(GridFit { extent, across, per, padding, sideways });
        self.record_size(ExpectedSize { slot: body.box_slot, w, h, grid });
        let top = (offset / along).floor().max(0.0) as usize;
        let first = top.saturating_sub(OVERSCAN).min(lines);
        let last = (top + (room / along).ceil() as usize + 1 + OVERSCAN).min(lines);
        let rtl = self.dir == Direction::Rtl;
        let saved = self.enter(&body);
        let (ew, eh) = if sideways { (extent, per as f32 * across) } else { (per as f32 * across, extent) };
        let holder = self.node_keyed(int_key(body.box_key, -1), Node { w: Size::Px(ew), h: Size::Px(eh), ..Node::DEFAULT });
        let inner = self.enter(&holder);
        self.slots[holder.box_slot].holds_rows = true;
        self.text_mut().list_window(body.box_key as u64, first * per, (last * per).min(count));
        let outer = (self.ahead, self.out_of_view);
        let shown = top..top + (room / along).ceil() as usize + 1;
        for i in first * per..(last * per).min(count) {
            let (line, k) = (i / per, i % per);
            self.ahead_row(outer, &shown, false, line);
            // the holder is a flow child, mirrored right to left; cells are `at` children, physical
            let (mut x, y) = if sideways { (line as f32 * cell.0, k as f32 * cell.1) } else { (k as f32 * cell.0, line as f32 * cell.1) };
            if rtl {
                x = ew - x - cell.0;
            }
            let c = self.node_keyed(int_key(body.box_key, i as i64), Node { w: Size::Px(cell.0), h: Size::Px(cell.1), at: Some((x, y)).into(), role: Role::ListItem, ..Node::DEFAULT });
            self.slots[c.box_slot].row_of = (body.box_key, i as u32);
            let saved = self.enter(&c);
            f(self, i);
            self.leave(saved);
        }
        (self.ahead, self.out_of_view) = outer;
        self.leave(inner);
        self.leave(saved);
        let mut geometry = ListGeometry::rows(&body, count, along, 0.0, 0.0, 0.0, padding);
        geometry.across = across;
        geometry.horizontal = sideways;
        self.remember_list(&body, geometry);
        body
    }

    /// The view `b` holds, if it is a live view.
    fn view_id(&self, b: &Handle) -> Option<u32> {
        if self.live(b) { Some(self.slots[b.box_slot].view).filter(|v| *v != NO_VIEW) } else { None }
    }

    /// Move a view's output by `(dx, dy)` points from where layout put it: one table entry changes,
    /// nothing is rebuilt or rewritten. Hit testing and `rect` follow it.
    pub fn view_offset(&mut self, b: &Handle, dx: f32, dy: f32) {
        if let Some(v) = self.view_id(b) {
            if self.views[v as usize].offset != (dx, dy) {
                self.views[v as usize].offset = (dx, dy);
                self.view_changed(v);
            }
        }
    }

    /// Scale a view's alpha by `opacity` (on each instance; overlaps inside it show through).
    pub fn view_opacity(&mut self, b: &Handle, opacity: f32) {
        if let Some(v) = self.view_id(b) {
            if self.views[v as usize].opacity != opacity {
                self.views[v as usize].opacity = opacity;
                self.view_changed(v);
            }
        }
    }

    /// Scale a view's output by `scale` about its box's centre (`view_transform` with no rotation).
    pub fn view_scale(&mut self, b: &Handle, scale: f32) {
        if let Some(v) = self.view_id(b) {
            let r = self.views[v as usize].built_rect;
            self.view_transform(b, scale, [1.0, 0.0, 0.0, 1.0], (r.w / 2.0, r.h / 2.0));
        }
    }

    /// Map a view's output through `pivot + scale * transform * (p - pivot)` (`transform` a row-major
    /// 2x2: rotation, skew, flip; `pivot` from its box's top-left corner): one table entry, no rebuild.
    /// Views nested in it inherit it (their entries compose it with their own, as offset, opacity
    /// and clip compose); hit testing goes through the inverse, and `rect` and the damage report
    /// the window rectangle covering the transformed box.
    pub fn view_transform(&mut self, b: &Handle, scale: f32, transform: [f32; 4], pivot: (f32, f32)) {
        if let Some(v) = self.view_id(b) {
            let r = &mut self.views[v as usize];
            if (r.scale,r.transform,r.pivot,r.pivot_fixed)!=(scale,transform,pivot,false) {
                (r.scale, r.transform, r.pivot, r.pivot_fixed) = (scale, transform, pivot, false);
                self.view_changed(v);
            }
        }
    }

    /// `view_transform` about the window point `pivot`, which stays put when the view moves: about
    /// `(0, 0)` with the move in `view_offset`, a scale of one leaves every point exact (the painter
    /// maps `pivot + scale * (p - pivot)` in floats). Unchanged values change nothing.
    pub fn view_transform_about(&mut self, b: &Handle, scale: f32, transform: [f32; 4], pivot: (f32, f32)) {
        if let Some(v) = self.view_id(b) {
            let r = &mut self.views[v as usize];
            if (r.scale, r.transform, r.pivot, r.pivot_fixed) != (scale, transform, pivot, true) {
                (r.scale, r.transform, r.pivot, r.pivot_fixed) = (scale, transform, pivot, true);
                self.view_changed(v);
            }
        }
    }

    /// Ask the host to blur what is under a view by `sigma` points before drawing it (CSS
    /// `backdrop-filter: blur`); 0 stops it. Reported in `FrameOut::blurs`, with the view box's look
    /// radius (`node_mut(&view).look.radius`) rounding the blurred image.
    pub fn view_backdrop_blur(&mut self, b: &Handle, sigma: f32) {
        if let Some(v) = self.view_id(b) {
            self.views[v as usize].blur = sigma.max(0.0);
            self.view_changed(v);
        }
    }

    /// Saturation of the blurred backdrop: 0 is grayscale, 1 preserves colour (the default).
    pub fn view_backdrop_saturation(&mut self, b: &Handle, saturation: f32) {
        if !saturation.is_finite() || saturation < 0.0 { return; }
        if let Some(v) = self.view_id(b) {
            self.views[v as usize].saturation = saturation;
            self.view_changed(v);
        }
    }

    /// The blur records of the shown views, where their runs now sit. Inline, as `refresh_views`: libpxui's partitioning otherwise
    /// keeps both out of `end_frame` where the static build folds them in (abi_bench dynamic, every frame).
    #[inline(always)]
    fn collect_blurs(&mut self) {
        let mut count = 0;
        let viewport = Rect::new(0.0, 0.0, self.view_w, self.view_h);
        for r in &self.views[1..] {
            if r.blur > 0.0 && r.valid && r.visited == self.painted {
                if let Some(layer) = self.draw[..self.draw_len].iter().position(|l| l.key == r.layer) {
                    let rect = r.cover(r.screen);
                    let radii = self.slots[r.slot].node.look.radius;
                    let blur = BlurOut { layer, at_instance: r.run.0, rect, sigma: r.blur, saturation: r.saturation, radii, opacity: r.alpha };
                    if let Some(old) = self.blur_out.get_mut(count) {
                        if *old != blur {
                            self.damage.add(old.rect, viewport);
                            self.damage.add(rect, viewport);
                            *old = blur;
                        }
                    } else {
                        self.damage.add(rect, viewport);
                        self.blur_out.push(blur);
                    }
                    count += 1;
                }
            }
        }
        for old in &self.blur_out[count..] {
            self.damage.add(old.rect, viewport);
        }
        self.blur_out.truncate(count);
        expand_blur_damage(&mut self.damage, &self.blur_out, viewport, self.density as f32);
    }

    /// Clip a view's output to `clip` (window points), besides its parents' clips; `None` lifts it.
    pub fn view_clip(&mut self, b: &Handle, clip: Option<Rect>) {
        if let Some(v) = self.view_id(b) {
            if self.views[v as usize].clip != clip {
                self.views[v as usize].clip = clip;
                self.view_changed(v);
            }
        }
    }

    /// A view's entry changed: between frames the table and the damage update now; a view with no
    /// table index bakes its offset into layout, so it rebuilds next frame.
    fn view_changed(&mut self, v: u32) {
        (self.a11y_dirty, self.a11y_full) = (true, true);
        if self.views[v as usize].index == 0 {
            // past 65,535 live views (the instance's 16-bit index) the offset is baked by a rebuild
            // (opacity and clip are not applied): specification/pxui.md, designed limits
            self.mark_view(v);
            self.views[v as usize].dirty = true;
            self.frame_requested = true;
        } else if !self.building && !self.painting && self.painted >= 0 {
            if self.refresh_views() {
                self.draw_revision += 1;
            }
            self.collect_blurs();
            if self.promotion.enabled { self.update_promotions(); }
        }
    }

    /// Mark `v` and the views around it to rebuild at their next declaration; one already kept this
    /// frame rebuilds next frame.
    /// Builds the frame again before it is drawn: a widget's state changed after part of it was built from the old one (a click on a
    /// row that changes rows built before it), so `b`'s view and the frame settle once more.
    pub(crate) fn build_again(&mut self, b: &Handle) {
        if self.live(b) {
            self.mark_view(self.slots[b.box_slot].owner);
            // the view building `b` keeps the mark past its own build, so the settling build rebuilds it
            self.escape();
            self.settle = true;
        }
    }

    fn mark_view(&mut self, mut v: u32) {
        while v != 0 && v != NO_VIEW {
            let r = &mut self.views[v as usize];
            r.dirty = true;
            if r.kept == self.frame_number {
                self.frame_requested = true;
            }
            v = r.parent;
        }
    }

    fn mark_key(&mut self, key: i64) {
        if key != 0 {
            if let Some(slot) = self.table.find(key) {
                self.mark_view(self.slots[slot].owner);
            }
        }
    }

    /// Dirty the views whose look follows what input changed since the last frame left it.
    fn mark_input(&mut self) {
        let mut marks = std::mem::take(&mut self.marks);
        marks.clear();
        // a drag-free node held and moved (no press, release or key) dirties nothing
        let drag_free = |ui: &Ui, key: i64| ui.active == Some(key) && ui.table.find(key).and_then(|slot| ui.slots.get(slot)).is_some_and(|s| s.node.flags.any(Flags::DRAG_FREE));
        marks.extend(self.signals.iter().filter(|(k, a)| !(drag_free(self, **k) && !a.pressed && !a.released && !a.clicked && a.keys.is_empty())).map(|(k, _)| *k));
        let (hot, focus, keyboard, carrying) = self.seen;
        let mut now = (if self.touches.using_touch { 0 } else { self.hot }, self.focus, self.keyboard_focus, self.payload.is_some());
        // while a drag-free node holds the capture the pointer's hover waits: the views it passed
        // over follow it when the drag ends
        if self.active.is_some_and(|k| drag_free(self, k)) {
            now.0 = hot;
        }
        if now.3 != carrying {
            marks.extend([hot, now.0]);
        } else if now.0 != hot {
            for key in [hot, now.0] {
                if !self.table.find(key).is_some_and(|slot| self.slots[slot].node.flags.any(Flags::HOVER_FREE)) {
                    marks.push(key);
                }
            }
        }
        if now.1 != focus || now.2 != keyboard {
            marks.push(focus);
        }
        // the focused view paints the caret and the input method's area every frame
        marks.push(now.1);
        // the view holding the pointer capture follows the gesture (and its keys) every frame
        marks.extend(self.active.filter(|k| !drag_free(self, *k)));
        for key in &marks {
            self.mark_key(*key);
        }
        self.marks = marks;
        self.seen = now;
    }

    fn free_view(&mut self, v: u32) {
        let r = &mut self.views[v as usize];
        if r.index != 0 {
            self.free_indices.push(r.index);
        }
        let (name, mut breaks, mut hits, mut nodes, mut prev) = (std::mem::take(&mut r.name), std::mem::take(&mut r.breaks), std::mem::take(&mut r.hits), std::mem::take(&mut r.nodes), std::mem::take(&mut r.prev));
        breaks.clear();
        hits.clear();
        nodes.clear();
        prev.clear();
        *r = ViewRec { name, breaks, hits, nodes, prev, outer: Rect::new(0.0, 0.0, 0.0, 0.0), ..ViewRec::new() };
        self.free_views.push(v);
        self.text_mut().view_freed(v);
    }

    /// The view's output is in the latest paint: its box was reached, or it sits in a kept view
    /// that was.
    fn shown(&self, v: u32) -> bool {
        if self.views[v as usize].visited == self.painted {
            return true;
        }
        let mut x = self.views[v as usize].parent;
        while x != 0 {
            let r = &self.views[x as usize];
            if r.visited == self.painted {
                return r.kept == self.painted;
            }
            x = r.parent;
        }
        false
    }

    /// Compose view `v`'s entry from its parent's.
    fn compose(&mut self, v: u32) {
        let stamp = self.stamp;
        if v == 0 || self.views[v as usize].stamp == stamp {
            return;
        }
        let parent = self.views[v as usize].parent;
        self.compose(parent);
        let pxf = self.views[parent as usize].xf;
        let p = &self.views[pxf as usize];
        let (pshift, palpha, pouter) = (p.shift, p.alpha, p.outer);
        // the enclosing transforms: none at the root, else the parent entry's composed with its own
        let (pa, pb) = if pxf == 0 || (p.plain() && p.parent_plain()) { (IDENTITY_2X2, [0.0; 2]) } else { p.affine() };
        let c = self.views[v as usize].box_clip;
        let box_clip = p.cover(Rect::new(c.x + pshift.0, c.y + pshift.1, c.w, c.h));
        let slot = self.views[v as usize].slot;
        let (dx, dy, opacity, scale) = if let Some(i) = self.tween_of(slot) {
            let (from, to, p) = self.tween_segment(i, self.now);
            let tween = |a, b| a + (b - a) * p;
            (tween(from.dx, to.dx), tween(from.dy, to.dy), tween(from.opacity, to.opacity),
                [tween(from.scale[0], to.scale[0]), tween(from.scale[1], to.scale[1]), to.anchor[0], to.anchor[1]])
        } else {
            let (dx, dy, opacity) = self.slots[slot].pose;
            (dx, dy, opacity, self.views[v as usize].scale_pose)
        };
        let r = &mut self.views[v as usize];
        r.stamp = stamp;
        if r.index == 0 {
            (r.shift, r.alpha, r.outer) = (pshift, palpha, pouter);
            // its instances ride its parent's entry: the parent's own transform is not around it
            (r.pa, r.pb) = (IDENTITY_2X2, [0.0; 2]);
        } else {
            r.shift = (pshift.0 + r.offset.0 + r.delta.0 + dx, pshift.1 + r.offset.1 + r.delta.1 + dy);
            r.alpha = palpha * r.opacity * opacity;
            (r.pa, r.pb) = (pa, pb);
            if scale[0] != 1.0 || scale[1] != 1.0 {
                let anchor = r.own_entry().map((r.built_rect.x + r.shift.0 + r.built_rect.w * scale[2], r.built_rect.y + r.shift.1 + r.built_rect.h * scale[3]));
                let shift = [(1.0 - scale[0]) * anchor.0, (1.0 - scale[1]) * anchor.1];
                r.pa = [pa[0] * scale[0], pa[1] * scale[1], pa[2] * scale[0], pa[3] * scale[1]];
                r.pb = [pb[0] + pa[0] * shift[0] + pa[1] * shift[1], pb[1] + pa[2] * shift[0] + pa[3] * shift[1]];
            }
            r.outer = box_clip.intersect(pouter).intersect(r.clip.unwrap_or(UNBOUNDED));
        }
        r.flat = r.plain() && r.parent_plain();
        let b = r.built_rect;
        r.screen = Rect::new(b.x + r.shift.0, b.y + r.shift.1, b.w, b.h).intersect(r.outer);
    }

    /// Recompose every shown view's entry into `view_out`; the damage gets each moved view's old
    /// and new rectangles. Whether any entry changed.
    #[inline(always)]
    fn refresh_views(&mut self) -> bool {
        self.stamp += 1;
        let mut out = std::mem::take(&mut self.view_prev);
        out.clear();
        let promoted = !self.promotion.current.is_empty();
        for v in 1..self.views.len() as u32 {
            if self.views[v as usize].index == 0 || self.views[v as usize].slot == NONE {
                continue;
            }
            let shown = self.shown(v);
            let r = &self.views[v as usize];
            let old_entry = r.entry();
            let old_cover = (shown && !self.full_damage && (!promoted || !self.promoted_view(r.index))).then(|| r.cover(r.screen));
            // Culled views still place rect/accessibility queries, without retaining draw runs.
            self.compose(v);
            let r = &self.views[v as usize];
            let (index, entry) = (r.index, r.entry());
            if entry != old_entry {
                if let Some(old_cover) = old_cover {
                    let cover = if r.flat { r.screen } else { r.cover(r.screen) };
                    self.add_damage(old_cover);
                    self.add_damage(cover);
                }
                self.invalidate_a11y_view(v);
            }
            if shown {
                out.push((index, entry));
            }
        }
        let changed = out != self.view_out;
        self.a11y_dirty |= changed;
        self.view_prev = std::mem::replace(&mut self.view_out, out);
        // the next refresh writes into `view_prev`: room now, not on the second frame
        self.view_prev.reserve(self.view_out.len().saturating_sub(self.view_prev.len()));
        changed
    }

    /// Colour/UV changes cover the same geometry: add those bounds only once.
    fn damage_change(&mut self, old: &Instance, new: &Instance) {
        if self.full_damage { return; }
        self.damage_instance(old);
        if old.rect != new.rect || old.clip != new.clip || old.kind != new.kind || old.view != new.view
            || old.width != new.width || old.extra != new.extra {
            self.damage_instance(new);
        }
    }

    /// Add an instance's cover to the damage, moved and clipped as its view shows it.
    fn damage_instance(&mut self, i: &Instance) {
        if self.full_damage { return; }
        let v = &self.views[self.index_owner[i.view as usize] as usize];
        // Every instance is clipped to this window-space rectangle after its view transform.
        if self.damage.covers(v.outer) { return; }
        if let Some(r) = bounds(i) {
            let r = Rect::new(r.x + v.shift.0, r.y + v.shift.1, r.w, r.h);
            let r = v.cover(r).intersect(v.outer);
            self.add_damage(r);
        }
    }

    fn add_damage(&mut self, r: Rect) {
        let viewport = Rect::new(0.0, 0.0, self.view_w, self.view_h);
        self.damage.add(r, viewport);
        if !self.painting && !self.blur_out.is_empty() {
            expand_blur_damage(&mut self.damage, &self.blur_out, viewport, self.density as f32);
        }
    }

    /// Paint a view box's children: a kept view adds its run and its hit areas, a fresh one paints
    /// inside its own clip (its outer clip is its table entry's) and records both.
    fn visit_view(&mut self, paint: &mut Paint, index: usize, v: u32, rect: Rect, clip: Rect, parent_hit: i64) {
        let layer = paint.layers[paint.layer].key;
        let painted = self.painted;
        let r = &mut self.views[v as usize];
        r.visited = painted;
        r.box_clip = clip;
        if self.boxes[index].kept {
            r.delta = (rect.x - r.built_rect.x, rect.y - r.built_rect.y);
            let resized = rect.w != r.built_rect.w || rect.h != r.built_rect.h;
            let moved = r.index == 0 && r.delta != (0.0, 0.0);
            let here = r.valid && r.layer == layer;
            if here {
                paint.keep(v);
                // an enclosing view being rebuilt keeps these instances in its run too (the root's
                // marks are not kept)
                if r.parent != 0 {
                    paint.text.add_marks(&r.shelves);
                }
            }
            let hit_parent = r.hit_parent;
            r.reflow |= resized;
            if resized || moved || !here {
                // Reach a dirty nested view through every enclosing cache on the settling build.
                self.mark_view(v);
                self.frame_requested = true;
                self.settle = true;
            }
            for k in 0..self.views[v as usize].hits.len() {
                let mut h = self.views[v as usize].hits[k];
                if h.parent == hit_parent {
                    h.parent = parent_hit;
                }
                self.push_hit(h);
            }
            return;
        }
        if r.verify && r.built_rect != rect {
            r.verify = false;
        }
        r.built_rect = rect;
        r.delta = (0.0, 0.0);
        let (table, xf) = (r.index, r.xf);
        // the text this build paints belongs to the view while it is not painted again
        let owner = paint.text.paint_view(v);
        let saved = (paint.view, self.paint_xf);
        if table != 0 {
            paint.view = table;
        }
        self.paint_xf = xf;
        let child_clip = if table != 0 { UNBOUNDED } else { clip };
        paint.cut();
        let outer = paint.text.marks();
        paint.text.set_marks([0; 8]);
        let (first, hit_start) = (paint.layers[paint.layer].pieces.len() - 1, self.hit_count);
        let mut child = self.boxes[index].first;
        while child != NONE {
            self.visit(paint, child, child_clip, parent_hit);
            child = self.boxes[child].next;
        }
        paint.cut();
        let shelves = paint.text.marks();
        paint.text.set_marks(outer);
        paint.text.add_marks(&shelves);
        paint.text.paint_view_end(owner);
        let r = &mut self.views[v as usize];
        r.shelves = shelves;
        r.pieces = (paint.layer, first, paint.layers[paint.layer].pieces.len() - 1);
        r.hit_parent = parent_hit;
        r.hits.clear();
        r.hits.extend_from_slice(&self.hits[hit_start..self.hit_count]);
        (paint.view, self.paint_xf) = saved;
    }

    /// Children of a view box with no table index are laid out at its offset.
    fn baked(&self, index: usize) -> (f32, f32) {
        match self.boxes[index].view {
            NO_VIEW => (0.0, 0.0),
            v => {
                let r = &self.views[v as usize];
                if r.index == 0 { r.offset } else { (0.0, 0.0) }
            }
        }
    }

    /// The node's instance range indexes the current output: its view's run is shown and was
    /// painted by the build that painted the node.
    fn range_valid(&self, slot: usize) -> bool {
        let s = &self.slots[slot];
        if s.owner == 0 {
            return s.painted == self.painted;
        }
        let v = &self.views[s.owner as usize];
        v.valid && s.painted == v.built && self.shown(s.owner)
    }

    /// Re-run each verified view's build check: its fresh instances (empty ones aside) equal the
    /// run it kept.
    fn verify_runs(&self, scratch: &[Layer]) {
        // a view's kept output holds the patches of the views inside it, which a fresh build does not
        let within = |mut k: usize, v: usize| {
            while k != 0 {
                k = self.views[k].parent as usize;
                if k == v {
                    return true;
                }
            }
            false
        };
        for (v, r) in self.views.iter().enumerate().skip(1) {
            if !(r.verify && r.valid && !r.patched && r.built == self.painted && r.visited == self.painted) {
                continue;
            }
            if self.views.iter().enumerate().any(|(k, q)| q.patched && within(k, v)) {
                continue;
            }
            // a view inside it fading a node (a list row's hover wash) moves its output between builds, as a patch does
            if self.tweens.iter().any(|t| { let k = self.slots[t.slot].owner as usize; k != v && within(k, v) }) {
                continue;
            }
            let Some(old) = self.draw[..self.draw_len].iter().find(|l| l.key == r.layer) else { continue };
            let layer = &scratch[r.pieces.0];
            let pieces = &layer.pieces[r.pieces.1..r.pieces.2];
            if pieces.iter().any(|p| p.view != NO_VIEW) {
                continue;
            }
            let new = pieces.iter().flat_map(|p| layer.instances[p.a as usize..p.b as usize].iter());
            let old = old.instances[r.run.0 as usize..r.run.1 as usize].iter();
            let same = old.filter(|i| **i != EMPTY).eq(new.filter(|i| **i != EMPTY));
            assert!(same, "Ui.verify_views: view {:?} built different output under an unchanged revision: its revision misses an input", r.name);
        }
    }

    /// When a node of a view being checked (`verify_views`) paints its running transition: as its
    /// kept build painted it, so the check compares like with like (`step_tweens` brings it to now
    /// after). A transition shows a look declared unlike the kept build's only in later frames, so
    /// that look is named here. Any other node paints now.
    fn verify_time(&self, slot: usize) -> f64 {
        let s = &self.slots[slot];
        let r = &self.views[s.owner as usize];
        if !(r.verify && r.valid && !r.patched && s.painted == r.kept_paint.0) || self.tween_of(slot).is_none() {
            return self.now;
        }
        // `seen` is the look the kept build declared (painting keeps it only while the theme moves)
        assert!(!self.motion || s.node.look == s.seen, "Ui.verify_views: view {:?} built different output under an unchanged revision: its revision misses an input", r.name);
        r.kept_paint.1
    }

    /// Merge the frame's scratch layers into the retained ones: fresh pieces land where they fall,
    /// kept runs stay. Gathers dirty ranges and damage; whether anything changed.
    fn place_all(&mut self, scratch: &mut [Layer], len: usize) -> bool {
        let mut changed = len != self.draw_len;
        for layer in &mut self.draw {
            layer.live = false;
        }
        for i in 0..len {
            let key = scratch[i].key;
            let j = match (i..self.draw.len()).find(|j| self.draw[*j].key == key) {
                Some(j) => j,
                None => {
                    // a new layer: storage no later layer of this frame wants, emptied
                    changed = true;
                    let j = match (i..self.draw.len()).find(|j| !scratch[i + 1..len].iter().any(|s| s.key == self.draw[*j].key)) {
                        Some(j) => j,
                        None => {
                            self.draw.push(Layer::new(i64::MIN, self.draw.len() as u32));
                            self.draw.len() - 1
                        }
                    };
                    let old = std::mem::take(&mut self.draw[j].instances);
                    for k in 0..old.len() {
                        self.damage_instance(&old[k]);
                    }
                    let d = &mut self.draw[j];
                    d.instances = old;
                    d.instances.clear();
                    d.bounds.clear();
                    d.runs.clear();
                    d.key = key;
                    d.breaks.clear();
                    d.images.clear();
                    d.dirty.clear();
                    j
                }
            };
            if i != j {
                changed = true;
                self.draw.swap(i, j);
            }
            (self.draw[i].role, self.draw[i].anchor) = (scratch[i].role, scratch[i].anchor);
            self.draw[i].live = true;
            changed |= self.place(&mut scratch[i], i);
        }
        // layers of the last frame that are gone
        for j in len..self.draw.len() {
            if self.draw[j].live || self.draw[j].instances.is_empty() {
                continue;
            }
            let old = std::mem::take(&mut self.draw[j].instances);
            for k in 0..old.len() {
                self.damage_instance(&old[k]);
            }
            self.draw[j].instances = old;
            self.draw[j].instances.clear();
            self.draw[j].bounds.clear();
            self.draw[j].runs.clear();
            self.draw[j].key = i64::MIN;
        }
        self.draw_len = len;
        changed
    }

    /// One layer: the fast path writes fresh pieces in place and fills the room before each kept run
    /// with empty instances; when a piece does not fit (or kept runs changed order) the layer is
    /// compacted with slack before every kept run.
    fn place(&mut self, s: &mut Layer, at: usize) -> bool {
        let n = s.instances.len() as u32;
        if let Some(last) = s.pieces.last_mut() {
            last.b = n;
        }
        let mut d = std::mem::take(&mut self.draw[at]);
        let old_len = d.instances.len();
        let mut changed = false;
        let mut kept = 0usize;
        let mut packed = s.instances.len();
        for piece in &s.pieces {
            if piece.view != NO_VIEW {
                let (a, b) = self.views[piece.view as usize].run;
                if a < b {
                    kept += 1;
                    let len = (b - a) as usize;
                    packed += len + SLACK.max(len / 8);
                }
            }
        }
        let budget = packed.saturating_mul(2).max(256);
        if kept != 0 || self.views.len() > 1 {
            // Existing fresh views may all be kept next frame (a fresh UI's second frame too); reserve their run padding now.
            let reserve = packed.saturating_add(SLACK.saturating_mul(self.views.len().saturating_sub(kept)))
                .saturating_add(s.instances.len() / 8).saturating_mul(2).max(256);
            d.instances.reserve(reserve.saturating_sub(d.instances.len()));
            d.bounds.reserve(d.instances.capacity().saturating_sub(d.bounds.len()));
            self.compact.reserve(reserve.saturating_sub(self.compact.len()));
            self.compact_bounds.reserve(reserve.saturating_sub(self.compact_bounds.len()));
            self.moves.reserve(self.views.len().saturating_sub(self.moves.len()));
            d.dirty.reserve(s.pieces.len().saturating_sub(d.dirty.len()));
        }
        let mut runs = std::mem::take(&mut self.cull_runs);
        runs.clear();
        runs.reserve(d.runs.len().max(s.pieces.len()).max(self.views.len() * 2));
        if self.full_damage && kept == 0 && s.pieces.iter().all(|p| p.view == NO_VIEW) {
            for p in &mut s.pieces {
                p.at = p.a;
            }
            changed = d.instances != s.instances;
            if changed {
                d.instances.clear();
                d.instances.extend_from_slice(&s.instances);
                d.bounds.clear();
                d.bounds.reserve(d.instances.capacity());
                d.bounds.extend(s.instances.iter().map(cull_bounds));
                d.dirty.clear();
                add_range(&mut d.dirty, 0, n);
                for p in &s.pieces { fresh_runs(&mut runs, &d, p.a, p.b); }
                self.finish_cull_runs(&mut d, runs);
            } else {
                self.cull_runs = runs;
            }
            changed |= self.rebuild_breaks(s, &mut d);
            self.draw[at] = d;
            return changed;
        }
        // where each fresh piece must end: the fresh pieces before a non-empty kept run end against it, so the room a compaction
        // left before a run stays ahead of them (a rebuilt view starting there does not take that room into its run)
        let mut limit = u32::MAX;
        for p in s.pieces.iter_mut().rev() {
            if p.view == NO_VIEW {
                p.at = limit;
                if limit != u32::MAX { limit = limit.saturating_sub(p.b - p.a); }
            } else {
                let (a, b) = self.views[p.view as usize].run;
                if a < b {
                    limit = a;
                }
            }
        }
        let mut w = 0u32;
        // Decide whether all pieces fit before touching the old layer. Compaction needs the
        // original instances for damage, and a late failure would copy the fresh prefix twice.
        let mut fits = old_len <= budget;
        for p in &mut s.pieces {
            if !fits { break; }
            if p.view == NO_VIEW {
                let len = p.b - p.a;
                let at = if p.at == u32::MAX { w } else { p.at.saturating_sub(len).max(w) };
                let end = at as usize + len as usize;
                fits = end <= p.at as usize && end <= budget;
                p.at = at;
                w = end as u32;
            } else {
                let (a, b) = self.views[p.view as usize].run;
                if a == b {
                    p.at = w;
                } else {
                    fits = a >= w;
                    p.at = a;
                    w = b;
                }
            }
        }
        if fits {
            w = 0;
            for p in &s.pieces {
                let at = p.at;
                if p.view == NO_VIEW {
                    let len = p.b - p.a;
                    let end = (at + len) as usize;
                    if end > d.instances.len() {
                        d.instances.resize(end, EMPTY);
                        d.bounds.reserve(d.instances.capacity().saturating_sub(d.bounds.len()));
                        d.bounds.resize(end, EMPTY_BOUNDS);
                    }
                    changed |= self.clear_piece_gap(&mut d, w, at);
                    if self.full_damage {
                        let fresh = &s.instances[p.a as usize..p.b as usize];
                        let current = &mut d.instances[at as usize..end];
                        if end > old_len || current != fresh {
                            current.copy_from_slice(fresh);
                            for (bounds, instance) in d.bounds[at as usize..end].iter_mut().zip(fresh) {
                                *bounds = cull_bounds(instance);
                            }
                            add_range(&mut d.dirty, at, end as u32);
                            changed = true;
                        }
                    } else {
                        for (x, new) in s.instances[p.a as usize..p.b as usize].iter().enumerate() {
                            let i = at as usize + x;
                            if i >= old_len || d.instances[i] != *new {
                                if i < old_len { self.damage_change(&d.instances[i], new); }
                                else { self.damage_instance(new); }
                                d.instances[i] = *new;
                                d.bounds[i] = cull_bounds(new);
                                add_range(&mut d.dirty, i as u32, i as u32 + 1);
                                changed = true;
                            }
                        }
                    }
                    w = end as u32;
                } else {
                    let (a, b) = self.views[p.view as usize].run;
                    if a != b {
                        changed |= self.clear_piece_gap(&mut d, w, a);
                        w = b;
                    }
                }
            }
            if self.full_damage {
                changed |= d.instances.len() > w as usize;
            } else {
                for i in w as usize..d.instances.len() {
                    self.damage_instance(&d.instances[i]);
                    changed = true;
                }
            }
            d.instances.truncate(w as usize);
            d.bounds.truncate(w as usize);
            // a range the host never drew may reach past the shorter layer: the painter refuses it
            d.dirty.retain_mut(|r| { r.1 = r.1.min(w); r.0 < r.1 });
            // Exact instance bytes still have valid runs, even if a view moved on screen.
            if changed {
                for p in &s.pieces {
                    if p.view == NO_VIEW {
                        fresh_runs(&mut runs, &d, p.at, p.at + p.b - p.a);
                    } else {
                        let (a, b) = self.views[p.view as usize].run;
                        if a != b { kept_runs(&mut runs, &d.runs, a, b, a); }
                    }
                }
            }
        } else {
            changed = true;
            self.compact_layer(s, &mut d, old_len, &mut runs);
        }
        if changed { self.finish_cull_runs(&mut d, runs); }
        else { self.cull_runs = runs; }
        changed |= self.rebuild_breaks(s, &mut d);
        self.draw[at] = d;
        changed
    }

    fn finish_cull_runs(&mut self, layer: &mut Layer, mut runs: Vec<CullRun>) {
        std::mem::swap(&mut layer.runs, &mut runs);
        runs.reserve(layer.runs.capacity().saturating_sub(runs.len()));
        self.cull_runs = runs;
    }

    fn clear_piece_gap(&mut self, layer: &mut Layer, start: u32, end: u32) -> bool {
        let gap = &mut layer.instances[start as usize..end as usize];
        if self.full_damage {
            if gap.iter().all(|i| *i == EMPTY) { return false; }
            gap.fill(EMPTY);
            layer.bounds[start as usize..end as usize].fill(EMPTY_BOUNDS);
            add_range(&mut layer.dirty, start, end);
            return true;
        }
        let mut changed = false;
        for (offset, instance) in gap.iter_mut().enumerate() {
            if *instance != EMPTY {
                self.damage_instance(instance);
                *instance = EMPTY;
                let i = start + offset as u32;
                layer.bounds[i as usize] = EMPTY_BOUNDS;
                add_range(&mut layer.dirty, i, i + 1);
                changed = true;
            }
        }
        changed
    }

    fn compact_layer(&mut self, s: &mut Layer, d: &mut Layer, old_len: usize, runs: &mut Vec<CullRun>) {
        self.compactions += 1;
        let mut out = std::mem::take(&mut self.compact);
        let mut bounds = std::mem::take(&mut self.compact_bounds);
        out.clear();
        bounds.clear();
        self.moves.clear();
        let mut fresh = 0usize;
        for k in 0..s.pieces.len() {
            let p = s.pieces[k];
            if p.view == NO_VIEW {
                s.pieces[k].at = out.len() as u32;
                let new = &s.instances[p.a as usize..p.b as usize];
                out.extend_from_slice(new);
                bounds.extend(new.iter().map(cull_bounds));
                fresh += (p.b - p.a) as usize;
                continue;
            }
            let (a, b) = self.views[p.view as usize].run;
            if a == b {
                s.pieces[k].at = out.len() as u32;
                continue;
            }
            // Another fresh batch fits here; total extra padding is at most the scratch instance count.
            out.resize(out.len() + SLACK.max((b - a) as usize / 8) + fresh, EMPTY);
            bounds.resize(out.len(), EMPTY_BOUNDS);
            fresh = 0;
            let to = out.len() as u32;
            out.extend_from_slice(&d.instances[a as usize..b as usize]);
            bounds.extend_from_slice(&d.bounds[a as usize..b as usize]);
            s.pieces[k].at = to;
            self.moves.push((a, b, to));
        }
        // Fresh pieces and copied runs must be appended in their final paint order.
        std::mem::swap(&mut d.instances, &mut out);
        std::mem::swap(&mut d.bounds, &mut bounds);
        let mut kept = self.moves.iter();
        for p in &s.pieces {
            if p.view == NO_VIEW {
                fresh_runs(runs, d, p.at, p.at + p.b - p.a);
            } else if self.views[p.view as usize].run.0 != self.views[p.view as usize].run.1 {
                if let Some(&(a, b, to)) = kept.next() { kept_runs(runs, &d.runs, a, b, to); }
            }
        }
        self.moves.sort_unstable_by_key(|m| m.0);
        self.move_runs(d.key);
        if self.full_damage {
            // Placement may have moved any kept run. One upload covers the new layer.
            d.dirty.clear();
            add_range(&mut d.dirty, 0, d.instances.len() as u32);
        } else {
            let mut dirty_start = None;
            for i in 0..d.instances.len().max(old_len) {
                let (old, new) = (out.get(i), d.instances.get(i));
                if old != new {
                    match (old, new) {
                        (Some(old), Some(new)) => self.damage_change(old, new),
                        (Some(old), None) => self.damage_instance(old),
                        (None, Some(new)) => self.damage_instance(new),
                        (None, None) => {}
                    }
                    if new.is_some() { dirty_start.get_or_insert(i as u32); }
                } else if let Some(start) = dirty_start.take() {
                    add_range(&mut d.dirty, start, i as u32);
                }
            }
            if let Some(start) = dirty_start { add_range(&mut d.dirty, start, d.instances.len() as u32); }
        }
        self.compact = out;
        self.compact_bounds = bounds;
        // ranges of skipped draws may reach past a layer that compaction made shorter: the painter refuses them
        let len = d.instances.len() as u32;
        d.dirty.retain_mut(|r| { r.1 = r.1.min(len); r.0 < r.1 });
    }

    /// A kept run `a..b` of layer `key` moved to `to`: the runs inside it and the ranges of nodes
    /// painted into it follow. O(views + the moved views' nodes).
    fn move_runs(&mut self, key: i64) {
        // each by the one kept run that held it before any moved (a run nested in another moves with
        // it, once): the runs are in layer order and do not overlap
        let moves = &self.moves;
        let shift = |(a0, b0): (u32, u32)| {
            let k = moves.partition_point(|m| m.1 < b0);
            moves.get(k).filter(|&&(a, b, _)| a0 >= a && b0 <= b).map(|&(a, _, to)| (a0 - a + to, b0 - a + to))
        };
        // a node painted into a kept run is one its view's last build declared
        for r in &mut self.views[1..] {
            if r.valid && r.built != self.painted && r.layer == key {
                if let Some(run) = shift(r.run) {
                    r.run = run;
                    for &slot in &r.nodes {
                        let s = &mut self.slots[slot];
                        if s.key != 0 && s.painted != self.painted && s.range_layer == key {
                            if let Some(range) = shift(s.range) {
                                s.range = range;
                            }
                        }
                    }
                }
            }
        }
    }

    /// The layer's texture breaks and images from its pieces; whether they changed.
    fn rebuild_breaks(&mut self, s: &Layer, d: &mut Layer) -> bool {
        let kept_breaks = s.pieces.iter().any(|p| p.view != NO_VIEW && {
            let r = &self.views[p.view as usize];
            r.tex0.is_some() || !r.breaks.is_empty()
        });
        if s.breaks.is_empty() && d.breaks.is_empty() && !kept_breaks {
            return false;
        }
        let mut breaks = std::mem::take(&mut self.temp_breaks);
        let mut images = std::mem::take(&mut self.temp_images);
        breaks.clear();
        images.clear();
        let mut current: Option<Image> = None;
        let mut set = |at: u32, tex: &Option<Image>, breaks: &mut Vec<Break>, images: &mut Vec<Image>| {
            let same = match (&current, tex) {
                (None, None) => true,
                (Some(a), Some(b)) => Rc::ptr_eq(&a.handle, &b.handle),
                _ => false,
            };
            if !same {
                let image = match tex {
                    None => crate::draw::ATLAS,
                    Some(t) => match images.iter().position(|i| Rc::ptr_eq(&i.handle, &t.handle)) {
                        Some(k) => k as u32,
                        None => {
                            images.push(t.clone());
                            images.len() as u32 - 1
                        }
                    },
                };
                match breaks.last_mut() {
                    Some(last) if last.at_instance == at => last.image = image,
                    _ => breaks.push(Break { at_instance: at, image }),
                }
                current = tex.clone();
            }
        };
        let tex_of = |image: u32| if image == crate::draw::ATLAS { None } else { Some(s.images[image as usize].clone()) };
        let (mut si, mut stex) = (0, None);
        for p in &s.pieces {
            if p.view == NO_VIEW {
                while si < s.breaks.len() && s.breaks[si].at_instance <= p.a {
                    stex = tex_of(s.breaks[si].image);
                    si += 1;
                }
                set(p.at, &stex, &mut breaks, &mut images);
                while si < s.breaks.len() && s.breaks[si].at_instance < p.b {
                    stex = tex_of(s.breaks[si].image);
                    set(p.at + s.breaks[si].at_instance - p.a, &stex, &mut breaks, &mut images);
                    si += 1;
                }
            } else {
                let r = &self.views[p.view as usize];
                set(p.at, &r.tex0, &mut breaks, &mut images);
                for (at, image) in &r.breaks {
                    set(p.at + at, image, &mut breaks, &mut images);
                }
            }
        }
        // a texture break with no instance after it is dropped by the painter's rule anyway
        let changed = breaks != d.breaks || images.len() != d.images.len() || images.iter().zip(&d.images).any(|(a, b)| !Rc::ptr_eq(&a.handle, &b.handle));
        std::mem::swap(&mut breaks, &mut d.breaks);
        std::mem::swap(&mut images, &mut d.images);
        // the next rebuild fills these: room for this frame's count now, not on the second frame
        breaks.reserve(d.breaks.len().saturating_sub(breaks.len()));
        images.reserve(d.images.len().saturating_sub(images.len()));
        self.temp_breaks = breaks;
        self.temp_images = images;
        changed
    }

    /// After placement: fresh views take their runs (and the breaks inside them), views not shown
    /// lose theirs, and the nodes the frame painted take their places in the retained layers.
    fn settle(&mut self, scratch: &[Layer]) {
        let painted = self.painted;
        for v in 1..self.views.len() {
            let r = &self.views[v];
            if r.slot == NONE {
                continue;
            }
            if r.visited == painted && r.built == painted {
                let (li, first, end) = r.pieces;
                let pieces = &scratch[li].pieces;
                // the run spans its non-empty pieces: an empty one may sit anywhere in the room before a kept child, and taking
                // it in would carry that compaction room inside the run (each compaction and rebuild then grew the layer)
                let span = |p: &Piece| match p.view {
                    NO_VIEW => (p.b > p.a).then_some((p.at, p.at + (p.b - p.a))),
                    k => { let (a, b) = self.views[k as usize].run; (a < b).then_some((a, b)) }
                };
                let (start, stop) = match (pieces[first..end].iter().find_map(span), pieces[first..end].iter().rev().find_map(span)) {
                    (Some((start, _)), Some((_, stop))) => (start, stop),
                    _ => (pieces[first].at, pieces[first].at),
                };
                let key = scratch[li].key;
                let d = &self.draw[li];
                debug_assert_eq!(d.key, key);
                let first_break = d.breaks.partition_point(|b| b.at_instance <= start);
                let texture = |b: &Break| (b.image != crate::draw::ATLAS).then(|| d.images[b.image as usize].clone());
                let tex0 = d.breaks[..first_break].last().and_then(texture);
                let mut breaks = std::mem::take(&mut self.views[v].breaks);
                breaks.clear();
                for b in d.breaks[first_break..].iter().take_while(|b| b.at_instance < stop) {
                    // a break back to the atlas (`None`) is replayed too
                    breaks.push((b.at_instance - start, texture(b)));
                }
                let r = &mut self.views[v];
                // the layer moves with the run: until now `verify_runs` reads the kept run where it was
                (r.layer, r.run, r.valid, r.tex0, r.breaks) = (key, (start, stop), true, tex0, breaks);
                let mut piece = first;
                for shape in &mut self.palette_shapes[v][..r.palette_shape_count as usize] {
                    while piece + 1 < end && pieces[piece].b <= shape.at { piece += 1; }
                    let p = pieces[piece];
                    debug_assert!(p.view == NO_VIEW && (p.a..p.b).contains(&shape.at) && shape.end <= p.b);
                    shape.at = p.at + shape.at - p.a - start;
                    shape.end = p.at + shape.end - p.a - start;
                }
                r.kept_paint = (painted, self.now);
            } else if !self.shown(v as u32) {
                self.views[v].valid = false;
            }
        }
        let mut layer = NONE;
        let mut piece = 0;
        for &(slot, li) in &self.painted_nodes {
            if li != layer {
                layer = li;
                piece = 0;
            }
            let pieces = &scratch[li].pieces;
            let s = &mut self.slots[slot];
            let (a, b) = s.range;
            // Painted nodes and pieces both follow scratch instance order.
            debug_assert!(piece == 0 || pieces[piece - 1].b <= a);
            while piece + 1 < pieces.len() && pieces[piece].b <= a { piece += 1; }
            let p = pieces[piece];
            s.range = (p.at + a - p.a, p.at + b - p.a);
            s.range_layer = scratch[li].key;
        }
        self.painted_nodes.clear();
    }
}

// ---- crate-private access for the kit and text widgets

impl Ui {
    pub(crate) fn kit_window_flag(&self) -> bool {
        self.kit_window
    }

    pub(crate) fn set_kit_window(&mut self, window: bool) {
        self.kit_window = window;
    }

    pub(crate) fn set_kit_row_height(&mut self, height: i64) {
        self.kit_row_height = height;
        self.row_points = height as f32;
    }

    pub(crate) fn closed_section_index(&self) -> usize {
        self.closed_section
    }

    pub(crate) fn set_closed_section(&mut self, index: usize) {
        self.closed_section = index;
    }

    pub(crate) fn set_modal_in_frame(&mut self, value: bool) {
        self.modal_in_frame = value;
    }

    /// Whether the popup `key`, open last frame (it has a slot), lies under the modal this frame's input went to: a sheet over it took
    /// the input, so its signals stay for that sheet.
    pub(crate) fn under_routed_modal(&self, key: i64) -> bool {
        self.routed_modal.is_some_and(|top| top != key) && self.slot_for(key).is_some()
    }

    pub(crate) fn set_modal_key(&mut self, key: Option<i64>) {
        self.escape();
        self.modal_key = key;
    }

    pub(crate) fn push_modal_extra(&mut self, key: i64) {
        self.modal_extra.insert(0, key);
    }

    /// Popups live at the root: the parents are the root alone, the seeds stay.
    pub(crate) fn enter_root_parent(&mut self) -> Saved {
        self.escape();
        let saved = Saved::Depths(self.parents.len(), self.seeds.len());
        self.parents.push(0);
        saved
    }

    fn belongs(&mut self, target: i64, key: i64, related: &[i64]) -> bool {
        self.hit_within(target, key) || related.iter().any(|r| self.hit_within(target, *r))
    }

    /// A popup keeps only the signals, capture and focus that belong to it.
    pub(crate) fn retain_popup_state(&mut self, key: i64, related: &[i64]) {
        self.cancel_touch_for_popup(Some(key), related);
        let mut signals = std::mem::take(&mut self.signals);
        let mut spare_signals = std::mem::take(&mut self.spare_signals);
        let mut spare_modifiers = std::mem::take(&mut self.spare_modifiers);
        let mut spare_key_lists = std::mem::take(&mut self.spare_key_lists);
        for (_, mut value) in signals.extract_if(|target, _| !self.belongs(*target, key, related)) {
            for (_, modifiers) in value.keys.drain(..) { spare_modifiers.push(modifiers); }
            if value.keys.capacity() > 0 { spare_key_lists.push(std::mem::take(&mut value.keys)); }
            spare_signals.push(value);
        }
        self.spare_key_lists = spare_key_lists;
        self.spare_signals = spare_signals;
        self.spare_modifiers = spare_modifiers;
        self.signals = signals;
        if let Some(active) = self.active {
            if !self.belongs(active, key, related) {
                self.cancelled.insert(0, self.active_button);
                self.active = None;
            }
        }
        if self.focus != 0 && !self.belongs(self.focus, key, related) {
            self.unfocus();
        }
    }

    pub(crate) fn remember_modal_height(&mut self, key: i64, slot: usize) {
        if !self.modal_heights.contains_key(&key) && self.modal_heights.len() >= 32 {
            // the least recently seen of at most 32 goes (specification/pxui.md, designed limits)
            let oldest = self.modal_heights.iter().min_by_key(|(_, (_, seen))| *seen).map(|(k, _)| *k);
            if let Some(k) = oldest {
                self.modal_heights.remove(&k);
            }
        }
        let height = self.slots[slot].rh;
        self.modal_heights.insert(key, (height, self.frame_number));
    }

    pub(crate) fn modal_height(&self, key: i64) -> f32 {
        self.modal_heights.get(&key).map(|(h, _)| *h).unwrap_or(0.0)
    }


    pub(crate) fn slot_rw(&self, slot: usize) -> f32 {
        self.slots[slot].rw
    }

    pub(crate) fn box_flags(&self, b: &UiBox) -> Flags {
        self.slots[b.box_slot].node.flags
    }

    pub(crate) fn set_flags_of_box(&mut self, b: &UiBox, flags: Flags) {
        self.slots[b.box_slot].node.flags = flags;
    }

}

/// The built-in tooltip: the tooltip look with clear ink, then each line a 20-point node 14 points below the last, text 7 points in.
pub(crate) fn default_tip(this: &mut Ui, c: &TipConfig, text: &str, lines: &[core::ops::Range<usize>]) {
    let look = this.theme().tooltip.idle;
    this.set_look(&c.tip, Look { text: CLEAR_INK, ..look }, State::default());
    for (i, line) in lines.iter().enumerate() {
        let n = this.node_keyed(int_key(c.tip.box_key, i as i64), Node { w: Size::Px(c.width), h: Size::Px(20.0), at: Some((0.0, 14.0 * i as f32)).into(), look: Look { fill: CLEAR_INK, border: CLEAR_INK, shadows: [Shadow::NONE; 2], ..look }, inset: Edges { l: 7.0, ..Edges::default() }, ..Node::DEFAULT });
        this.text(&n, &text[line.clone()]);
    }
}

/// Whether any colour of `look` names a colour token (`Rgba::token`).
fn names_token(look: &Look) -> bool {
    let named = |c: Rgba| c.as_token().is_some();
    named(look.fill) || matches!(look.gradient, pxui_types::node::OptColor::Some(c) if named(c)) || named(look.border) || named(look.shadows[0].color)
        || named(look.shadows[1].color) || named(look.text) || named(look.ring.color) || named(look.selection) || named(look.caret)
}

/// Token `token`'s colour: its entry of `table`, `accent` for `ColorToken::ACCENT`, transparent past both.
#[inline]
pub(crate) fn token_entry(table: &[Rgba; pxui_types::theme::ColorToken::LIMIT], accent: Rgba, token: u16) -> Rgba {
    if token == pxui_types::theme::ColorToken::ACCENT.0 { accent } else { table.get(token as usize).copied().unwrap_or_default() }
}

/// The token colour `c` (`Rgba::token`) painted as `entry` at the token's opacity (`Rgba::alpha`, D19, final): its green byte.
#[inline]
pub(crate) fn token_ink(c: Rgba, entry: Rgba) -> Rgba {
    Rgba { a: ((u32::from(entry.a) * u32::from(c.g) + 127) / 255) as u8, ..entry }
}

/// Replaces each colour of `look` that names a token by its entry of `table` (`accent` for `ColorToken::ACCENT`, transparent past
/// both) at the token's opacity.
fn resolve_with(look: &mut Look, table: &[Rgba; pxui_types::theme::ColorToken::LIMIT], accent: Rgba) {
    let ink = |c: &mut Rgba| if let Some(token) = c.as_token() { *c = token_ink(*c, token_entry(table, accent, token.0)) };
    let [a, b] = &mut look.shadows;
    for c in [&mut look.fill, &mut look.border, &mut a.color, &mut b.color, &mut look.text, &mut look.ring.color, &mut look.selection, &mut look.caret] { ink(c); }
    if let pxui_types::node::OptColor::Some(c) = &mut look.gradient { ink(c); }
}

impl Theme {
    /// Whether a part's look or the focus ring names a colour token.
    #[doc(hidden)]
    pub fn names_color_tokens(&self) -> bool {
        let mut any = self.ring.color.as_token().is_some();
        self.each_states(&mut |s| any |= [&s.idle, &s.hover, &s.pressed, &s.focus, &s.disabled, &s.checked].into_iter().any(names_token));
        any
    }
}

/// Resolves every token look of `theme` (its parts' states and its focus ring) against its own side of `colors` (light or dark by its
/// tokens).
fn resolve_theme(theme: &mut Theme, table: &Palette) {
    let accent = theme.tokens.accent;
    if let Some(token) = theme.ring.color.as_token() { theme.ring.color = token_ink(theme.ring.color, token_entry(table, accent, token.0)); }
    theme.each_states_mut(&mut |s| for look in [&mut s.idle, &mut s.hover, &mut s.pressed, &mut s.focus, &mut s.disabled, &mut s.checked] { resolve_with(look, table, accent); });
}
