//! Bounded host-owned image descriptors and coalesced requests; no decoding or GPU ownership.
//!
//! Draw code asks `Images::get` for an image by the app's id; what is missing becomes a `Want` that the host drains with
//! `Images::requests`, decodes on a worker and answers with `Images::store`. Everything is bounded: `CAPACITY` cached descriptors
//! (the least recently used makes room), `CAPACITY` waiting wants (extra ones are dropped) and `REQUEST_CAPACITY` wants per packet.
//! App data never panics here: an id, path or size that does not make a `Want` is `None` (`Want::new`) or not recorded (`get`); slots
//! are internal.
//!
//! Memory: an `Images` is a fixed budget made at `Default` and never grown: `CAPACITY` descriptors (72 bytes each, 72 KiB), an id
//! index sized for `CAPACITY` ids (2048 buckets of 16 bytes and their control bytes, about 34 KiB), `CAPACITY` waiting wants (40 bytes
//! each, 40 KiB) and `PATH_ARENA_BYTES` (64 KiB) of their paths: about 210 KiB in all, pinned by
//! `tests::the_memory_budget_is_what_the_module_says`. A `Request` is a fixed 4272-byte value (`REQUEST_PATH_BYTES` of paths); it is
//! filled in place, never allocated.

use crate::Image;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

/// Number of image slots in `Images`, and of wanted images it remembers at once.
pub const CAPACITY: usize = 1024;
/// Byte budget for decoded pixels the host keeps resident for these images (the host enforces it; `Images` only holds descriptors).
pub const BYTE_LIMIT: usize = 128 * 1024 * 1024;
/// Number of images one `Request` packet can name.
pub const REQUEST_CAPACITY: usize = 8;
/// Longest image source path, in bytes, a want can carry.
pub const PATH_LIMIT: usize = 1024;
/// Bytes of source paths one `Request` packet holds, all its wants together (at least one path of `PATH_LIMIT`).
pub const REQUEST_PATH_BYTES: usize = 4096;
/// Bytes of source paths `Images` keeps for its waiting wants, all together; a want whose path does not fit (after compaction) is
/// dropped, as one past `CAPACITY` is.
pub const PATH_ARENA_BYTES: usize = 64 * 1024;

/// One wanted image: which image the worker should decode and how large. Its source path travels beside it in the `Request`.
/// Made only by `Want::new`, so it is always valid. 16 bytes, `Copy`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Want {
    id: usize,
    max_pixels: u32,
}
impl Want {
    const NONE: Want = Want { id: 0, max_pixels: 0 };
    /// The want of image `id` decoded to at most `max_pixels` on its longest side; `None` for `id` 0 or `max_pixels` outside 1 to 1024.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::images::Want;
    /// let want = Want::new(7, 256).unwrap();
    /// assert_eq!((want.id(), want.max_pixels()), (7, 256));
    /// assert!(Want::new(0, 256).is_none() && Want::new(7, 0).is_none() && Want::new(7, 2048).is_none());
    /// ```
    pub fn new(id: usize, max_pixels: u32) -> Option<Want> {
        (id != 0 && (1..=1024).contains(&max_pixels)).then_some(Want { id, max_pixels })
    }
    /// The app's image id (never 0); one cached image per id.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui::images::Want::new(7, 64).unwrap().id(), 7);
    /// ```
    pub fn id(&self) -> usize {
        self.id
    }
    /// The longest side, in pixels, the worker should decode to: 1 to 1024, a multiple of 32 when `Images` asked for it.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui::images::Want::new(7, 64).unwrap().max_pixels(), 64);
    /// ```
    pub fn max_pixels(&self) -> u32 {
        self.max_pixels
    }
}

/// A packet of up to `REQUEST_CAPACITY` wanted images and their source paths (`REQUEST_PATH_BYTES` in all) for a worker, stamped with
/// the `Images` epoch it was built in. A fixed-size value: fill one in place (`Images::requests`) and hand it to the worker.
#[derive(Clone, Debug)]
pub struct Request {
    epoch: u64,
    wants: [Want; REQUEST_CAPACITY],
    /// Each want's path as `(start, len)` in `bytes`.
    paths: [(u16, u16); REQUEST_CAPACITY],
    bytes: [u8; REQUEST_PATH_BYTES],
    count: u8,
    used: u16,
}
impl Default for Request {
    fn default() -> Self {
        Request { epoch: 0, wants: [Want::NONE; REQUEST_CAPACITY], paths: [(0, 0); REQUEST_CAPACITY], bytes: [0; REQUEST_PATH_BYTES], count: 0, used: 0 }
    }
}
impl PartialEq for Request {
    fn eq(&self, other: &Self) -> bool {
        self.epoch == other.epoch && self.wants() == other.wants() && (0..self.wants().len()).all(|i| self.path(i) == other.path(i))
    }
}
impl Eq for Request {}
impl Request {
    /// Empties the packet of its wants and paths; the epoch stays.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::images::{Request, Want};
    /// let mut request = Request::default();
    /// assert!(request.insert(Want::new(1, 64).unwrap(), "a.png"));
    /// request.clear();
    /// assert!(request.wants().is_empty());
    /// ```
    pub fn clear(&mut self) {
        self.count = 0;
        self.used = 0;
    }
    /// The `Images::epoch` this packet was built in; 0 for a packet made with `Request::default`.
    ///
    /// A host compares it with the current epoch to drop results that were decoded before an `Images::clear`.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::images::{Images, Request};
    /// let (mut images, mut request) = (Images::default(), Request::default());
    /// images.clear();
    /// images.requests(None, &mut request);
    /// assert_eq!(request.epoch(), 1);
    /// assert_eq!(Request::default().epoch(), 0);
    /// ```
    pub fn epoch(&self) -> u64 {
        self.epoch
    }
    /// The wanted images in this packet, in the order they were added; `path(i)` is where `wants()[i]` is read from.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::images::{Request, Want};
    /// let mut request = Request::default();
    /// request.insert(Want::new(7, 64).unwrap(), "a.png");
    /// assert_eq!(request.wants()[0].id(), 7);
    /// ```
    pub fn wants(&self) -> &[Want] {
        &self.wants[..usize::from(self.count)]
    }
    /// The source path of the `index`-th want, exactly as given to `insert`; empty past the last want.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::images::{Request, Want};
    /// let mut request = Request::default();
    /// request.insert(Want::new(7, 64).unwrap(), "é.png");
    /// assert_eq!((request.path(0), request.path(1)), ("é.png", ""));
    /// ```
    pub fn path(&self, index: usize) -> &str {
        if index >= usize::from(self.count) { return ""; }
        let (start, len) = self.paths[index];
        // copied from a `&str` whole
        std::str::from_utf8(&self.bytes[usize::from(start)..usize::from(start) + usize::from(len)]).unwrap_or_default()
    }
    /// Adds `want`, read from `path`, to the packet; false when it was not added.
    ///
    /// An id already in the packet is not added twice: its `max_pixels` becomes the larger of the two (its first path stays) and the
    /// call is true, even when the packet is full. False for an empty `path` or one longer than `PATH_LIMIT` bytes, and for a new id
    /// when the packet holds `REQUEST_CAPACITY` wants or its path does not fit the `REQUEST_PATH_BYTES` left.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::images::{Request, Want};
    /// let mut request = Request::default();
    /// assert!(request.insert(Want::new(1, 64).unwrap(), "a.png"));
    /// assert!(request.insert(Want::new(1, 128).unwrap(), "b.png"));
    /// assert_eq!((request.wants().len(), request.wants()[0].max_pixels(), request.path(0)), (1, 128, "a.png"));
    /// assert!(!request.insert(Want::new(2, 64).unwrap(), ""));
    /// ```
    pub fn insert(&mut self, want: Want, path: &str) -> bool {
        if path.is_empty() || path.len() > PATH_LIMIT {
            return false;
        }
        if let Some(wanted) = self.wants[..usize::from(self.count)].iter_mut().find(|wanted| wanted.id == want.id) {
            wanted.max_pixels = wanted.max_pixels.max(want.max_pixels);
            return true;
        }
        let start = usize::from(self.used);
        if usize::from(self.count) == REQUEST_CAPACITY || start + path.len() > REQUEST_PATH_BYTES {
            return false;
        }
        self.bytes[start..start + path.len()].copy_from_slice(path.as_bytes());
        self.wants[usize::from(self.count)] = want;
        self.paths[usize::from(self.count)] = (start as u16, path.len() as u16);
        self.count += 1;
        self.used = (start + path.len()) as u16;
        true
    }
}

struct Pending {
    want: Want,
    node: Option<i64>,
    /// The source path as `(start, len)` in `Queue::bytes`.
    path: (u32, u16),
}
/// The waiting wants and their source paths in one arena. Paths are appended in want order and wants leave in order, so the arena is
/// compacted in place (no allocation) when a new path does not fit.
struct Queue {
    pending: Vec<Pending>,
    bytes: Vec<u8>,
}
impl Queue {
    fn path(&self, (start, len): (u32, u16)) -> &str {
        std::str::from_utf8(&self.bytes[start as usize..start as usize + usize::from(len)]).unwrap_or_default()
    }
    fn clear(&mut self) {
        self.pending.clear();
        self.bytes.clear();
    }
    /// Room for `len` more path bytes, compacting the arena first when needed; false when even a compacted arena lacks it.
    fn room(&mut self, len: usize) -> bool {
        if self.bytes.len() + len <= PATH_ARENA_BYTES { return true; }
        let mut at = 0usize;
        for wanted in &mut self.pending {
            let (start, len) = (wanted.path.0 as usize, usize::from(wanted.path.1));
            self.bytes.copy_within(start..start + len, at);
            wanted.path.0 = at as u32;
            at += len;
        }
        self.bytes.truncate(at);
        at + len <= PATH_ARENA_BYTES
    }
}
struct Entry {
    id: usize,
    image: Option<Image>,
    quality: u32,
    used: Cell<u64>,
}
/// The app's table of image descriptors by id: up to `CAPACITY` host-decoded images, plus the decodes still wanted.
///
/// It holds `pxui::Image` descriptors only; the host decodes, uploads and destroys the pixels. Draw code asks with `get`, which returns
/// what is cached and queues what is missing; the host drains `requests` and answers with `store`. When `store` needs room it drops the
/// least recently used descriptor and names it, so the host can release that image's pixels.
pub struct Images {
    entries: Vec<Entry>,
    by_id: HashMap<usize, usize>,
    clock: Cell<u64>,
    queue: RefCell<Queue>,
    revision: u64,
    epoch: u64,
    density: f32,
    max_pixels: u32,
}
impl Default for Images {
    fn default() -> Self {
        Self {
            entries: (0..CAPACITY)
                .map(|_| Entry {
                    id: 0,
                    image: None,
                    quality: 0,
                    used: Cell::new(0),
                })
                .collect(),
            by_id: HashMap::with_capacity(CAPACITY),
            clock: Cell::new(0),
            queue: RefCell::new(Queue { pending: Vec::with_capacity(CAPACITY), bytes: Vec::with_capacity(PATH_ARENA_BYTES) }),
            revision: 0,
            epoch: 0,
            density: 1.0,
            max_pixels: 512,
        }
    }
}
impl Images {
    /// Sets the screen density and the largest decode size `get` will ask for; true when either value changed.
    ///
    /// `density` (device pixels per point) is clamped to 1 to 4, a non-finite one becomes 1; `max_pixels` is clamped to 32 to 1024.
    /// A change bumps the revision without ejecting cached descriptors. When `get` finds a successful decode below its updated
    /// size target, it queues another request and returns the existing descriptor while the upgrade is pending. A cached failure
    /// stays cached until removed.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let mut images = pxui::images::Images::default();
    /// assert!(images.configure(2.0, 384));
    /// assert!(!images.configure(2.0, 384));
    /// assert!(images.configure(f32::NAN, 384));
    /// ```
    pub fn configure(&mut self, density: f32, max_pixels: u32) -> bool {
        let density = if density.is_finite() {
            density.clamp(1.0, 4.0)
        } else {
            1.0
        };
        let max_pixels = max_pixels.clamp(32, 1024);
        if self.density != density || self.max_pixels != max_pixels {
            self.density = density;
            self.max_pixels = max_pixels;
            self.revision = self.revision.wrapping_add(1);
            return true;
        }
        false
    }
    /// A counter that changes whenever what `get` returns may have changed: `configure` with new values, `store`, `remove`, `clear`.
    ///
    /// Compare it with the last value seen to know when a cached view must rebuild. It wraps at `u64::MAX`.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let mut images = pxui::images::Images::default();
    /// let before = images.revision();
    /// images.clear();
    /// assert_ne!(images.revision(), before);
    /// ```
    pub fn revision(&self) -> u64 {
        self.revision
    }
    /// A counter that `clear` advances; each `Request` carries the value it was built in.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let mut images = pxui::images::Images::default();
    /// assert_eq!(images.epoch(), 0);
    /// images.clear();
    /// assert_eq!(images.epoch(), 1);
    /// ```
    pub fn epoch(&self) -> u64 {
        self.epoch
    }
    /// Forgets every wanted image not yet delivered; cached images stay.
    ///
    /// A later `get` of a missing image queues it again.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let images = pxui::images::Images::default();
    /// images.get(1, "a.png", 100.0, None);
    /// images.cancel_requests();
    /// let mut request = pxui::images::Request::default();
    /// images.requests(None, &mut request);
    /// assert!(request.wants().is_empty());
    /// ```
    pub fn cancel_requests(&self) {
        self.queue.borrow_mut().clear();
    }
    /// Fills `out` with the first wanted images that fit it (`REQUEST_CAPACITY` wants, `REQUEST_PATH_BYTES` of paths), in the order
    /// they were first asked for, stamped with the current epoch; with `ui`, only those worth decoding now, judged from its last
    /// laid-out frame.
    ///
    /// Asking does not remove them: they stay wanted until `store` delivers the id or `cancel_requests` / `clear` runs, so a full queue
    /// is served over several calls. Wants for one id from several nodes merge into the largest size. With `ui`, a want made for a node
    /// (`get` with `Some(node)`) counts only while that node is on screen or within two of its heights above or below the view, and one
    /// whose node is not in the tree is dropped for good, so cells that were only provisional leave no work; wants made without a node
    /// always count. Call it after `Ui::frame`, so geometry is final.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::images::{Images, Request};
    /// let images = Images::default();
    /// let ui = pxui::doc::frames(2, &mut |ui| {
    ///     let node = ui.node("cover", pxui::Node::DEFAULT);
    ///     images.get(1, "a.png", 50.0, Some(&node));
    /// });
    /// images.get(2, "b.png", 100.0, None);
    /// let mut request = Request::default();
    /// images.requests(None, &mut request);
    /// assert_eq!((request.wants()[1].id(), request.wants()[1].max_pixels(), request.path(1)), (2, 128, "b.png"));
    /// images.requests(Some(&ui), &mut request);
    /// assert!(request.wants().len() <= 2);
    /// ```
    pub fn requests(&self, ui: Option<&crate::Ui>, out: &mut Request) {
        let mut queue = self.queue.borrow_mut();
        if let Some(ui) = ui {
            queue.pending.retain(|wanted| wanted.node.is_none_or(|key| ui.box_of_key(crate::Key::Resolved(key)).is_some()));
        }
        out.clear();
        out.epoch = self.epoch;
        for wanted in queue.pending.iter() {
            if let (Some(ui), Some(key)) = (ui, wanted.node) {
                let Some(node) = ui.box_of_key(crate::Key::Resolved(key)) else {
                    continue;
                };
                let rect = ui.rect(&node);
                let crate::ViewSize { width, height } = ui.view_size();
                let margin = rect.h * 2.0;
                if rect.w <= 0.0
                    || rect.h <= 0.0
                    || rect.x >= width
                    || rect.y >= height + margin
                    || rect.x + rect.w <= 0.0
                    || rect.y + rect.h <= -margin
                {
                    continue;
                }
            }
            out.insert(wanted.want, queue.path(wanted.path));
        }
    }
    /// The slot holding image `id`, or `None` when it is not cached (a failed decode stored as `None` still has a slot).
    fn index(&self, id: usize) -> Option<usize> {
        self.by_id.get(&id).copied()
    }
    /// The `max_pixels` image `id` was delivered at, or `None` when it is not cached.
    ///
    /// It is the size that was asked for, not the decoded image's dimensions; a failed decode keeps its quality too.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::images::{Images, Want};
    /// let mut images = Images::default();
    /// images.store(Want::new(5, 256).unwrap(), None);
    /// assert_eq!(images.quality(5), Some(256));
    /// assert_eq!(images.quality(6), None);
    /// ```
    pub fn quality(&self, id: usize) -> Option<u32> {
        self.index(id).map(|index| self.entries[index].quality)
    }
    /// The cached image `id`, if any, and a request for a better one when the cached one is smaller than `points` needs.
    ///
    /// `points` is the logical size the image is drawn at; the wanted decode size is `points` times the configured density, rounded up
    /// to a multiple of 32 and clamped to 32 up to the `configure` maximum. `node` is the widget that shows the image, so `requests`
    /// with a `Ui` can skip it while off screen; one id wanted from several nodes keeps one want per node. A cached image is returned
    /// whatever its quality (a placeholder until the better one arrives); `None` when `id` was never delivered or its decode failed.
    /// The want is recorded when `id` and the size make a `Want` (`Want::new`) and `path` is not empty or past `PATH_LIMIT` bytes; up to
    /// `CAPACITY` wants (and `PATH_ARENA_BYTES` of their paths) wait at once, extra ones are dropped. Takes `&self`, so draw code can call it.
    /// Allocates: never.
    /// Identity: `id` is the app's own number for the image; two paths with one `id` share one slot and the first delivered wins.
    /// Panics: never.
    ///
    /// ```
    /// let images = pxui::images::Images::default();
    /// assert!(images.get(1, "a.png", 100.0, None).is_none());
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let node = ui.node("cover", pxui::Node::DEFAULT);
    ///     assert!(images.get(2, "b.png", 50.0, Some(&node)).is_none());
    /// });
    /// let mut request = pxui::images::Request::default();
    /// images.requests(None, &mut request);
    /// assert_eq!((request.wants()[0].id(), request.path(0), request.wants().len()), (1, "a.png", 2));
    /// ```
    pub fn get(&self, id: usize, path: &str, points: f32, node: Option<&crate::Handle>) -> Option<Image> {
        let node = node.map(|node| node.box_key);
        let quality = ((points.max(1.0) * self.density).ceil() as u32)
            .div_ceil(32)
            .saturating_mul(32)
            .clamp(32, self.max_pixels);
        let image = if let Some(index) = self.index(id) {
            let entry = &self.entries[index];
            let tick = self.clock.get().saturating_add(1);
            self.clock.set(tick);
            entry.used.set(tick);
            if entry.image.is_none() || entry.quality >= quality {
                return entry.image;
            }
            entry.image
        } else {
            None
        };
        let mut queue = self.queue.borrow_mut();
        if let Some(wanted) = queue
            .pending
            .iter_mut()
            .find(|wanted| wanted.want.id == id && wanted.node == node)
        {
            wanted.want.max_pixels = wanted.want.max_pixels.max(quality);
        } else if queue.pending.len() < CAPACITY
            && !path.is_empty()
            && path.len() <= PATH_LIMIT
            && let Some(want) = Want::new(id, quality)
            && queue.room(path.len())
        {
            let start = queue.bytes.len() as u32;
            queue.bytes.extend_from_slice(path.as_bytes());
            queue.pending.push(Pending { want, node, path: (start, path.len() as u16) });
        }
        image
    }
    /// The slot to fill next: the first empty slot, else the least recently used one.
    fn oldest(&self) -> usize {
        self.entries
            .iter()
            .position(|entry| entry.id == 0)
            .unwrap_or_else(|| {
                self.entries
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, entry)| entry.used.get())
                    .map_or(0, |(index, _)| index)
            })
    }
    /// The id of the least recently used cached image that holds pixels (a failed decode holds none), or `None` when none does.
    ///
    /// A host over its pixel budget removes the descriptor and retires the native texture after all sampling frames have completed. Each `get` that finds an id cached, and each `store`,
    /// makes that id the most recently used.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{images::{Images, Want}, Image};
    /// let mut images = Images::default();
    /// let pixels = Some({ let mut record = Image::default(); record.width = 8; record.height = 4; record });
    /// images.store(Want::new(1, 64).unwrap(), pixels);
    /// images.store(Want::new(2, 64).unwrap(), pixels);
    /// images.store(Want::new(3, 64).unwrap(), None);
    /// images.get(1, "a.png", 32.0, None);
    /// assert_eq!(images.oldest_image(), Some(2));
    /// ```
    pub fn oldest_image(&self) -> Option<usize> {
        self.entries
            .iter()
            .filter(|entry| entry.id != 0 && entry.image.is_some())
            .min_by_key(|entry| entry.used.get())
            .map(|entry| entry.id)
    }
    /// Stores the worker's result for `want` (`None` records a failed decode, so it is not retried), replacing what was cached for
    /// `want.id()`; returns the id of another image it dropped to make room, if any.
    ///
    /// With `CAPACITY` ids cached, the least recently used descriptor is dropped. The host still owns its texture and can retire it
    /// only after all sampling frames have completed. Pending wants for the id are
    /// cleared, except when `image` is present and a want asked for more pixels than `want.max_pixels()`. A replaced image is only
    /// forgotten here: the host still owns and destroys it. Moves `revision` on.
    /// Allocates: never.
    /// Identity: `want.id()` becomes the id `get` finds the image by.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::images::{Images, Request};
    /// let (mut images, mut request) = (Images::default(), Request::default());
    /// images.get(1, "a.png", 100.0, None);
    /// images.requests(None, &mut request);
    /// assert_eq!(images.store(request.wants()[0], None), None);
    /// images.requests(None, &mut request);
    /// assert!(request.wants().is_empty());
    /// assert_eq!(images.quality(1), Some(128));
    /// ```
    pub fn store(&mut self, want: Want, image: Option<Image>) -> Option<usize> {
        self.queue.borrow_mut().pending.retain(|wanted| {
            wanted.want.id != want.id || (image.is_some() && wanted.want.max_pixels > want.max_pixels)
        });
        let index = self.index(want.id).unwrap_or_else(|| self.oldest());
        let dropped = self.entries[index].id;
        self.remove_at(index);
        let tick = self.clock.get().saturating_add(1);
        self.clock.set(tick);
        self.entries[index] = Entry {
            id: want.id,
            image,
            quality: want.max_pixels,
            used: Cell::new(tick),
        };
        self.by_id.insert(want.id, index);
        (dropped != 0 && dropped != want.id).then_some(dropped)
    }
    /// Forgets every cached image and every wanted one, and starts a new epoch.
    ///
    /// The host still owns the pixels it handed over. Moves both `revision` and `epoch` on.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::images::{Images, Want};
    /// let mut images = Images::default();
    /// images.store(Want::new(1, 64).unwrap(), None);
    /// images.clear();
    /// assert_eq!(images.quality(1), None);
    /// ```
    pub fn clear(&mut self) {
        self.by_id.clear();
        for entry in &mut self.entries {
            entry.id = 0;
            entry.image = None;
        }
        self.revision = self.revision.wrapping_add(1);
        self.epoch = self.epoch.wrapping_add(1);
        self.cancel_requests();
    }
    /// Forgets image `id`, so `get` asks for it again; false when it was not cached. Pending wants are untouched.
    ///
    /// The host still owns and destroys the image. Moves `revision` on when it removed one.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::images::{Images, Want};
    /// let mut images = Images::default();
    /// images.store(Want::new(1, 64).unwrap(), None);
    /// assert!(images.remove(1));
    /// assert!(!images.remove(1));
    /// assert_eq!(images.quality(1), None);
    /// ```
    pub fn remove(&mut self, id: usize) -> bool {
        let Some(index) = self.index(id) else { return false };
        self.remove_at(index);
        true
    }
    /// Empties slot `index` (an empty slot stays empty) and moves `revision` on.
    fn remove_at(&mut self, index: usize) {
        self.by_id.remove(&self.entries[index].id);
        self.entries[index].id = 0;
        self.entries[index].image = None;
        self.revision = self.revision.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn packet(images: &Images, ui: Option<&crate::Ui>) -> Request {
        let mut request = Request::default();
        images.requests(ui, &mut request);
        request
    }
    #[test]
    fn image_paths_preserve_bounded_utf8_without_allocating() {
        let boundary = "é".repeat(512);
        let overflow = format!("{boundary}x");
        let want = Want::new(1, 64).unwrap();
        let mut request = Request::default();
        for _ in 0..8 {
            mem_sys::local_reset();
            request.clear();
            assert!(request.insert(want, &boundary));
            assert_eq!(request.path(0), boundary);
            assert!(!request.insert(Want::new(2, 64).unwrap(), &overflow));
            assert_eq!(mem_sys::local_alloc_count(), 0);
        }
    }
    /// Paths of waiting wants live in one bounded arena: wants that leave leave holes, and a new path that does not fit compacts the
    /// arena in place (paths still read back right) instead of growing it; a path that does not fit even then drops the want.
    #[test]
    fn the_path_arena_compacts_in_place_and_never_grows() {
        let images = Images::default();
        let path = "p".repeat(PATH_LIMIT);
        let fill = PATH_ARENA_BYTES / PATH_LIMIT;
        for id in 1..=fill { images.get(id, &path, 64.0, None); }
        assert_eq!(images.queue.borrow().bytes.len(), PATH_ARENA_BYTES);
        assert!(images.get(999, "late.png", 64.0, None).is_none());
        assert_eq!(images.queue.borrow().pending.len(), fill, "a full arena drops the new want");
        // deliver every other one: holes in the arena
        let mut images = images;
        for id in (1..=fill).step_by(2) { images.store(Want::new(id, 64).unwrap(), None); }
        mem_sys::local_reset();
        images.get(999, "late.png", 64.0, None);
        assert_eq!(mem_sys::local_alloc_count(), 0, "compaction moves bytes, it does not allocate");
        let queue = images.queue.borrow();
        assert_eq!(queue.bytes.capacity(), PATH_ARENA_BYTES);
        assert_eq!(queue.path(queue.pending.last().unwrap().path), "late.png");
        assert!(queue.pending[..queue.pending.len() - 1].iter().all(|wanted| queue.path(wanted.path) == path));
    }
    #[test]
    fn the_memory_budget_is_what_the_module_says() {
        use core::mem::size_of;
        assert_eq!((size_of::<Entry>(), size_of::<Pending>(), size_of::<Want>(), size_of::<Request>()), (72, 40, 16, 4272));
        let images = Images::default();
        assert!(images.by_id.capacity() >= CAPACITY && images.by_id.capacity() < 2 * CAPACITY, "one table for CAPACITY ids, never regrown");
        let queue = images.queue.borrow();
        assert_eq!((queue.pending.capacity(), queue.bytes.capacity()), (CAPACITY, PATH_ARENA_BYTES));
        let total = CAPACITY * size_of::<Entry>() + 2048 * (2 * size_of::<usize>() + 1) + CAPACITY * size_of::<Pending>() + PATH_ARENA_BYTES;
        assert!(total > 205 * 1024 && total < 215 * 1024, "about 210 KiB: {total}");
    }
    #[test]
    fn requests_deduplicate_and_cached_or_missing_images_allocate_nothing() {
        let mut images = Images::default();
        let path = "CoverArt/cover.png";
        images.store(Want::new(1, 512).unwrap(), None);
        mem_sys::local_reset();
        for _ in 0..1000 {
            images.cancel_requests();
            assert!(images.get(1, path, 184.0, None).is_none());
            assert!(
                packet(&images, None).wants().is_empty(),
                "failed images are cached too"
            );
            assert!(images.get(2, "CoverArt/pending.png", 30.0, None).is_none());
            assert!(images.get(2, "CoverArt/pending.png", 184.0, None).is_none());
            assert_eq!(packet(&images, None).wants().len(), 1);
            assert_eq!(packet(&images, None).wants()[0].max_pixels(), 192);
        }
        assert_eq!(mem_sys::local_alloc_count(), 0);
        images.clear();
        assert_eq!(images.oldest(), 0);
        assert!(!Request::default().insert(Want::new(1, 512).unwrap(), &"x".repeat(1025)));
    }
    #[test]
    fn only_final_visible_image_nodes_request_decodes_and_cached_nodes_keep_their_requests() {
        let images = Images::default();
        let mut ui = crate::Ui::new(&crate::UiDesc::DEFAULT).unwrap();
        for frame in 0..2 {
            ui.frame(
                &crate::FrameIn::new(200.0, 100.0).time(frame as f64),
                &mut |ui| {
                    for (id, y) in [
                        (1, 0.0),
                        (2, if frame == 0 { 300.0 } else { 50.0 }),
                        (3, 130.0),
                    ] {
                        let node = ui.node(
                            match id {
                                1 => "visible",
                                2 => "later",
                                _ => "prefetch",
                            },
                            crate::ui::Node::DEFAULT.w(crate::ui::Size::Px(30.0)).h(crate::ui::Size::Px(30.0)).at(0.0, y),
                        );
                        if frame == 0 {
                            images.get(id, "cover.png", 30.0, Some(&node));
                        }
                    }
                },
            )
            .unwrap();
            let packet = packet(&images, Some(&ui));
            assert_eq!(packet.wants().len(), if frame == 0 { 2 } else { 3 });
            assert_eq!(packet.wants()[0].id, 1);
        }
        mem_sys::local_reset();
        for _ in 0..1000 {
            assert_eq!(packet(&images, Some(&ui)).wants().len(), 3);
        }
        assert_eq!(mem_sys::local_alloc_count(), 0);
        ui.frame(
            &crate::FrameIn::new(200.0, 100.0).time(2.0),
            &mut |_| {},
        )
        .unwrap();
        assert!(
            packet(&images, Some(&ui)).wants().is_empty(),
            "retired provisional cells leave no pending work"
        );
    }
    #[test]
    fn retained_views_keep_pending_covers_until_completion_instead_of_starving_other_panels() {
        let mut images = Images::default();
        for id in 1..=12 {
            images.get(id, "cover.png", 184.0, None);
        }
        assert_eq!(packet(&images, None).wants().len(), REQUEST_CAPACITY);
        // A cached album view makes no calls on this frame; a live player still asks for its cover.
        images.get(100, "player.png", 30.0, None);
        assert_eq!(packet(&images, None).wants()[0].id(), 1);
        for id in 1..=8 {
            let want = packet(&images, None).wants()[0];
            assert_eq!(want.id(), id);
            images.store(want, None);
        }
        assert_eq!(
            packet(&images, None).wants()[0].id(),
            9,
            "visible requests beyond the first packet survive until delivery"
        );
        for id in 9..=12 {
            images.get(id, "cover.png", 184.0, None);
        }
        images.get(100, "player.png", 30.0, None);
        assert_eq!(packet(&images, None).wants().len(), 5);
        assert_eq!(packet(&images, None).wants()[4].id(), 100);
        images.cancel_requests();
        assert!(packet(&images, None).wants().is_empty());
    }
    #[test]
    fn visible_galleries_larger_than_sixty_four_keep_all_descriptors_without_warm_allocations() {
        let mut images = Images::default();
        let path = "cover.png";
        for index in 0..768 {
            images.store(
                Want::new(index + 1, 512).unwrap(),
                Some({ let mut record = Image::default(); record.width = 8; record.height = 4; record }),
            );
        }
        mem_sys::local_reset();
        for _ in 0..10 {
            for id in 1..=768 {
                assert!(images.get(id, path, 184.0, None).is_some());
            }
        }
        assert!(packet(&images, None).wants().is_empty());
        assert_eq!(mem_sys::local_alloc_count(), 0);
    }
    #[test]
    fn an_id_stored_again_keeps_one_slot_and_a_full_table_drops_and_names_the_least_recently_used() {
        let mut images = Images::default();
        let pixels = Some({ let mut record = Image::default(); record.width = 8; record.height = 4; record });
        for id in 1..=CAPACITY {
            assert_eq!(images.store(Want::new(id, 64).unwrap(), pixels), None, "free slots drop nothing");
        }
        // storing a cached id again replaces it in place: no second slot, nothing else dropped
        assert_eq!(images.store(Want::new(7, 128).unwrap(), pixels), None);
        assert_eq!(images.by_id.len(), CAPACITY);
        assert_eq!(images.entries.iter().filter(|entry| entry.id == 7).count(), 1);
        assert_eq!(images.quality(7), Some(128));
        // id 1 is now the least recently used; a get of it makes 2 the oldest instead
        assert!(images.get(1, "cover.png", 32.0, None).is_some());
        assert_eq!(images.oldest_image(), Some(2));
        assert_eq!(images.store(Want::new(CAPACITY + 1, 64).unwrap(), None), Some(2), "the dropped id is named");
        assert_eq!((images.quality(2), images.quality(CAPACITY + 1)), (None, Some(64)));
        // removing by id clears exactly that id's mapping
        assert!(images.remove(7));
        assert!(!images.remove(7));
        assert_eq!((images.quality(7), images.quality(8)), (None, Some(64)));
    }
    #[test]
    fn cached_quality_does_not_retry_small_sources() {
        let mut images = Images::default();
        images.configure(2.0, 384);
        images.store(
            Want::new(1, 384).unwrap(),
            Some({ let mut record = Image::default(); record.width = 8; record.height = 4; record }),
        );
        assert!(images.get(1, "tiny.png", 184.0, None).is_some());
        assert!(
            packet(&images, None).wants().is_empty(),
            "record requested quality, not source dimensions"
        );
    }
}
