//! Bounded host-owned image descriptors and coalesced requests; no decoding or GPU ownership.

use crate::Image;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

/// Number of image slots in `Images`, and of wanted images it remembers at once.
pub const CAPACITY: usize = 1024;
/// Byte budget for decoded pixels the host keeps resident for these images (the host enforces it; `Images` only holds descriptors).
pub const BYTE_LIMIT: usize = 128 * 1024 * 1024;
/// Number of images one `Request` packet can name.
pub const REQUEST_CAPACITY: usize = 8;

/// A bounded UTF-8 image source path copied into worker requests without heap allocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Path {
    bytes: [u8; 1024],
    len: u16,
}
impl Default for Path {
    fn default() -> Self {
        Self {
            bytes: [0; 1024],
            len: 0,
        }
    }
}
impl Path {
    /// Copies `text` into a fixed-size path; `None` when it is longer than 1024 bytes (an empty path is allowed).
    ///
    /// The path is not checked against the filesystem, and `Images` ignores empty ones.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::images::Path;
    /// assert_eq!(Path::new("CoverArt/a.png").unwrap().as_str(), "CoverArt/a.png");
    /// assert!(Path::new(&"x".repeat(1025)).is_none());
    /// ```
    pub fn new(text: &str) -> Option<Self> {
        if text.len() > 1024 {
            return None;
        }
        let mut path = Self::default();
        path.bytes[..text.len()].copy_from_slice(text.as_bytes());
        path.len = text.len() as u16;
        Some(path)
    }
    /// The path text exactly as given to `Path::new`, borrowed from the path itself.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::images::Path;
    /// assert_eq!(Path::new("é.png").unwrap().as_str(), "é.png");
    /// assert_eq!(Path::default().as_str(), "");
    /// ```
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..self.len as usize])
            .expect("image path was copied from UTF-8")
    }
}

/// One wanted image: what the worker should decode and how large.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Key {
    /// The app's image id (never 0); one cached image per id.
    pub id: usize,
    /// Where the worker reads the source image from.
    pub path: Path,
    /// Longest side, in pixels, the worker should decode to (1 to 1024; a multiple of 32 when `Images` made it).
    pub max_pixels: u32,
}
/// A packet of up to `REQUEST_CAPACITY` wanted images for a worker, stamped with the `Images` epoch it was built in.
#[derive(Clone, Copy, Default, Debug, Eq)]
pub struct Request {
    epoch: u64,
    keys: [Key; REQUEST_CAPACITY],
    count: u8,
}
impl PartialEq for Request {
    fn eq(&self, other: &Self) -> bool {
        self.epoch == other.epoch && self.keys() == other.keys()
    }
}
impl Request {
    /// Empties the packet of its keys; the epoch stays.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::images::Request;
    /// let mut request = Request::default();
    /// assert!(request.insert(1, "a.png", 64));
    /// request.clear();
    /// assert!(request.keys().is_empty());
    /// ```
    pub fn clear(&mut self) {
        self.count = 0;
    }
    /// The `Images::epoch` this packet was built in; 0 for a packet made with `Request::default`.
    ///
    /// A host compares it with the current epoch to drop results that were decoded before an `Images::clear`.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::images::{Images, Request};
    /// let mut images = Images::default();
    /// assert_eq!(images.requests().epoch(), 0);
    /// images.clear();
    /// assert_eq!(images.requests().epoch(), 1);
    /// assert_eq!(Request::default().epoch(), 0);
    /// ```
    pub fn epoch(&self) -> u64 {
        self.epoch
    }
    /// The wanted images in this packet, in the order they were added.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::images::Request;
    /// let mut request = Request::default();
    /// request.insert(7, "a.png", 64);
    /// assert_eq!(request.keys()[0].id, 7);
    /// ```
    pub fn keys(&self) -> &[Key] {
        &self.keys[..usize::from(self.count)]
    }
    /// Adds image `id` read from `path` at up to `max_pixels` to the packet; false when it was not added.
    ///
    /// An `id` already in the packet is not added twice: its `max_pixels` becomes the larger of the two (its first path stays) and the
    /// call is true, even when the packet is full. False for `id` 0, an empty or over-1024-byte `path`, `max_pixels` outside 1 to 1024,
    /// or a full packet (`REQUEST_CAPACITY` keys) and a new `id`.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::images::Request;
    /// let mut request = Request::default();
    /// assert!(request.insert(1, "a.png", 64));
    /// assert!(request.insert(1, "a.png", 128));
    /// assert_eq!((request.keys().len(), request.keys()[0].max_pixels), (1, 128));
    /// assert!(!request.insert(0, "a.png", 64));
    /// ```
    pub fn insert(&mut self, id: usize, path: &str, max_pixels: u32) -> bool {
        if id == 0 || path.is_empty() || !(1..=1024).contains(&max_pixels) {
            return false;
        }
        if let Some(key) = self.keys[..usize::from(self.count)]
            .iter_mut()
            .find(|key| key.id == id)
        {
            key.max_pixels = key.max_pixels.max(max_pixels);
            return true;
        }
        if usize::from(self.count) == REQUEST_CAPACITY {
            return false;
        }
        let Some(path) = Path::new(path) else {
            return false;
        };
        self.keys[usize::from(self.count)] = Key {
            id,
            path,
            max_pixels,
        };
        self.count += 1;
        true
    }
}

struct Pending {
    key: Key,
    node: Option<i64>,
}
struct Entry {
    id: usize,
    image: Option<Image>,
    quality: u32,
    used: Cell<u64>,
}
/// The app's table of image descriptors by id: up to `CAPACITY` host-decoded images, plus the decodes still wanted.
///
/// It holds `pxui::Image` descriptors only; the host decodes, uploads and destroys the pixels. Draw code asks with `get` (or `get_at`),
/// which returns what is cached and queues what is missing; the host drains `requests` or `visible_requests` and answers with `insert`.
pub struct Images {
    entries: Vec<Entry>,
    by_id: HashMap<usize, usize>,
    clock: Cell<u64>,
    pending: RefCell<Vec<Pending>>,
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
            pending: RefCell::new(Vec::with_capacity(CAPACITY)),
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
    /// A change bumps `revision` but drops nothing: cached images stay returned, and `get` asks again only when its size now needs more
    /// pixels than the cached image was decoded at.
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
    /// A counter that changes whenever what `get` returns may have changed: `configure` with new values, `insert`, `remove`, `clear`.
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
    /// The use tick of slot `index`: larger is more recent, 0 when never used.
    ///
    /// Each `get` or `get_at` that finds its id cached, and each `insert`, gives that slot the next tick; `oldest` picks the smallest.
    /// Allocates: never.
    /// Panics: when `index` is `CAPACITY` or more.
    ///
    /// ```
    /// use pxui::images::Images;
    /// let mut images = Images::default();
    /// assert_eq!(images.last_used(0), 0);
    /// images.insert(0, Default::default(), None);
    /// assert!(images.last_used(0) > 0);
    /// ```
    pub fn last_used(&self, index: usize) -> u64 {
        self.entries[index].used.get()
    }
    /// Forgets every wanted image not yet delivered; cached images stay.
    ///
    /// A later `get` of a missing image queues it again.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let images = pxui::images::Images::default();
    /// images.get(1, "a.png", 100.0);
    /// images.cancel_requests();
    /// assert!(images.requests().keys().is_empty());
    /// ```
    pub fn cancel_requests(&self) {
        self.pending.borrow_mut().clear();
    }
    /// The first `REQUEST_CAPACITY` wanted images, in the order they were first asked for, with no check of where they are on screen.
    ///
    /// Asking does not remove them: they stay wanted until `insert` delivers the id or `cancel_requests` / `clear` runs, so a
    /// full queue is served over several calls. Wants for one id from several `get_at` nodes merge into the largest size.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let images = pxui::images::Images::default();
    /// images.get(1, "a.png", 100.0);
    /// let request = images.requests();
    /// assert_eq!((request.keys()[0].id, request.keys()[0].max_pixels), (1, 128));
    /// ```
    pub fn requests(&self) -> Request {
        self.packet(None)
    }
    /// Like `requests`, but only the wants worth decoding now, judged from `ui`'s last laid-out frame.
    ///
    /// A want made with `get_at` counts only while its node is on screen or within two of its heights above or below the view; one
    /// whose node is not in the tree is dropped for good, so cells that were only provisional leave no work. Wants made with `get`
    /// always count. Call it after `Ui::frame`, so geometry is final.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::images::Images;
    /// let images = Images::default();
    /// let ui = pxui::doc::frames(2, &mut |ui| {
    ///     let node = ui.node("cover".into(), pxui::Node::DEFAULT);
    ///     images.get_at(1, "a.png", 50.0, node.box_key);
    /// });
    /// assert!(images.visible_requests(&ui).keys().len() <= 1);
    /// ```
    pub fn visible_requests(&self, ui: &crate::Ui) -> Request {
        self.packet(Some(ui))
    }
    fn packet(&self, ui: Option<&crate::Ui>) -> Request {
        let mut pending = self.pending.borrow_mut();
        if let Some(ui) = ui {
            pending.retain(|wanted| wanted.node.is_none_or(|key| ui.box_of_key(key).is_some()));
        }
        let mut request = Request {
            epoch: self.epoch,
            ..Default::default()
        };
        for wanted in pending.iter() {
            if let (Some(ui), Some(key)) = (ui, wanted.node) {
                let Some(node) = ui.box_of_key(key) else {
                    continue;
                };
                let rect = ui.rect(&node);
                let (width, height) = ui.view_size();
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
            request.insert(
                wanted.key.id,
                wanted.key.path.as_str(),
                wanted.key.max_pixels,
            );
        }
        request
    }
    /// The slot holding image `id`, or `None` when it is not cached (a failed decode stored as `None` still has a slot).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::images::Images;
    /// let mut images = Images::default();
    /// assert_eq!(images.index(5), None);
    /// let key = pxui::images::Key { id: 5, ..Default::default() };
    /// images.insert(3, key, None);
    /// assert_eq!(images.index(5), Some(3));
    /// ```
    pub fn index(&self, id: usize) -> Option<usize> {
        self.by_id.get(&id).copied()
    }
    /// The `max_pixels` image `id` was delivered at, or `None` when it is not cached.
    ///
    /// It is the size that was asked for, not the decoded image's dimensions; a failed decode keeps its quality too.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::images::{Images, Key};
    /// let mut images = Images::default();
    /// images.insert(0, Key { id: 5, max_pixels: 256, ..Default::default() }, None);
    /// assert_eq!(images.quality(5), Some(256));
    /// assert_eq!(images.quality(6), None);
    /// ```
    pub fn quality(&self, id: usize) -> Option<u32> {
        self.index(id).map(|index| self.entries[index].quality)
    }
    /// The cached image `id`, if any, and a request for a better one when the cached one is smaller than `points` needs.
    ///
    /// `points` is the logical size the image is drawn at; the wanted decode size is `points` times the configured density,
    /// rounded up to a multiple of 32 and clamped to 32 up to the `configure` maximum. A cached image is returned whatever its
    /// quality (a placeholder until the better one arrives); `None` when `id` was never delivered or its decode failed. The want is
    /// recorded when `id` is not 0 and `path` is not empty or longer than 1024 bytes; up to `CAPACITY` wants wait at once,
    /// extra ones are dropped. Takes `&self`, so draw code can call it.
    /// Allocates: never.
    /// Identity: `id` is the app's own number for the image; two paths with one `id` share one slot and the first delivered wins.
    /// Panics: never.
    ///
    /// ```
    /// let images = pxui::images::Images::default();
    /// assert!(images.get(1, "a.png", 100.0).is_none());
    /// assert_eq!(images.requests().keys()[0].id, 1);
    /// ```
    pub fn get(&self, id: usize, path: &str, points: f32) -> Option<Image> {
        self.lookup(id, path, points, None)
    }
    /// Like `get`, for an image drawn in layout node `node` (a `PxuiBox::box_key`), so `visible_requests` can skip it while off screen.
    ///
    /// One id wanted from several nodes keeps one want per node; the cached image and the size rule are those of `get`.
    /// Allocates: never.
    /// Identity: `id` as in `get`; `node` is the box key of the widget that shows the image.
    /// Panics: never.
    ///
    /// ```
    /// let images = pxui::images::Images::default();
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let node = ui.node("cover".into(), pxui::Node::DEFAULT);
    ///     assert!(images.get_at(1, "a.png", 50.0, node.box_key).is_none());
    /// });
    /// assert_eq!(images.requests().keys().len(), 1);
    /// ```
    pub fn get_at(&self, id: usize, path: &str, points: f32, node: i64) -> Option<Image> {
        self.lookup(id, path, points, Some(node))
    }
    fn lookup(&self, id: usize, path: &str, points: f32, node: Option<i64>) -> Option<Image> {
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
        let mut pending = self.pending.borrow_mut();
        if let Some(key) = pending
            .iter_mut()
            .find(|wanted| wanted.key.id == id && wanted.node == node)
        {
            key.key.max_pixels = key.key.max_pixels.max(quality);
        } else if id != 0
            && !path.is_empty()
            && pending.len() < CAPACITY
            && let Some(path) = Path::new(path)
        {
            pending.push(Pending {
                key: Key {
                    id,
                    path,
                    max_pixels: quality,
                },
                node,
            });
        }
        image
    }
    /// The slot to fill next: the first empty slot, else the least recently used one (smallest `last_used`).
    ///
    /// It evicts nothing; `insert` into the slot replaces what is there.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let mut images = pxui::images::Images::default();
    /// assert_eq!(images.oldest(), 0);
    /// images.insert(0, pxui::images::Key { id: 1, ..Default::default() }, None);
    /// assert_eq!(images.oldest(), 1);
    /// ```
    pub fn oldest(&self) -> usize {
        self.entries
            .iter()
            .position(|entry| entry.id == 0)
            .unwrap_or_else(|| {
                self.entries
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, entry)| entry.used.get())
                    .unwrap()
                    .0
            })
    }
    /// Stores the worker's result for `key` in slot `index` (`None` records a failed decode so it is not retried), replacing that slot.
    ///
    /// Pending wants for `key.id` are cleared, except when `image` is present and a want asked for more pixels than `key.max_pixels`.
    /// The slot's previous image is only forgotten here: the host still owns and destroys it. Moves `revision` on. An id maps to
    /// one slot: when `key.id` is already cached in another slot, that slot is emptied (as `remove` does), so `index(key.id)` is `index`.
    /// Allocates: never.
    /// Identity: `key.id` becomes the id `get` finds the image by.
    /// Panics: when `index` is `CAPACITY` or more.
    ///
    /// ```
    /// use pxui::images::{Images, Key};
    /// let mut images = Images::default();
    /// images.get(1, "a.png", 100.0);
    /// let key = images.requests().keys()[0];
    /// images.insert(images.oldest(), key, None);
    /// assert!(images.requests().keys().is_empty());
    /// assert_eq!(images.quality(1), Some(128));
    /// ```
    pub fn insert(&mut self, index: usize, key: Key, image: Option<Image>) {
        let mut pending = self.pending.borrow_mut();
        pending.retain(|wanted| {
            wanted.key.id != key.id || (image.is_some() && wanted.key.max_pixels > key.max_pixels)
        });
        drop(pending);
        self.remove(index);
        if let Some(other) = self.by_id.get(&key.id).copied() {
            self.remove(other);
        }
        let tick = self.clock.get().saturating_add(1);
        self.clock.set(tick);
        self.entries[index] = Entry {
            id: key.id,
            image,
            quality: key.max_pixels,
            used: Cell::new(tick),
        };
        self.by_id.insert(key.id, index);
        self.revision = self.revision.wrapping_add(1);
    }
    /// Forgets every cached image and every wanted one, and starts a new epoch.
    ///
    /// The host still owns the pixels it handed over. Moves both `revision` and `epoch` on.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let mut images = pxui::images::Images::default();
    /// images.insert(0, pxui::images::Key { id: 1, ..Default::default() }, None);
    /// images.clear();
    /// assert_eq!(images.index(1), None);
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
    /// Empties slot `index`, so its id is no longer cached and `get` asks for it again; pending wants are untouched.
    ///
    /// The host still owns and destroys the image. Moves `revision` on. Removing an empty slot is harmless.
    /// Allocates: never.
    /// Panics: when `index` is `CAPACITY` or more.
    ///
    /// ```
    /// let mut images = pxui::images::Images::default();
    /// images.insert(0, pxui::images::Key { id: 1, ..Default::default() }, None);
    /// images.remove(0);
    /// assert_eq!(images.index(1), None);
    /// ```
    pub fn remove(&mut self, index: usize) {
        self.by_id.remove(&self.entries[index].id);
        self.entries[index].id = 0;
        self.entries[index].image = None;
        self.revision = self.revision.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn image_paths_preserve_bounded_utf8_without_allocating() {
        let boundary = "é".repeat(512);
        let overflow = format!("{boundary}x");
        for _ in 0..8 {
            mem_sys::local_reset();
            assert_eq!(Path::new(&boundary).unwrap().as_str(), boundary);
            assert!(Path::new(&overflow).is_none());
            assert_eq!(mem_sys::local_alloc_count(), 0);
        }
    }
    #[test]
    fn requests_deduplicate_and_cached_or_missing_images_allocate_nothing() {
        let mut images = Images::default();
        let path = Path::new("CoverArt/cover.png").unwrap();
        images.insert(
            0,
            Key {
                id: 1,
                path,
                max_pixels: 512,
            },
            None,
        );
        mem_sys::local_reset();
        for _ in 0..1000 {
            images.cancel_requests();
            assert!(images.get(1, path.as_str(), 184.0).is_none());
            assert!(
                images.requests().keys().is_empty(),
                "failed images are cached too"
            );
            assert!(images.get(2, "CoverArt/pending.png", 30.0).is_none());
            assert!(images.get(2, "CoverArt/pending.png", 184.0).is_none());
            assert_eq!(images.requests().keys().len(), 1);
            assert_eq!(images.requests().keys()[0].max_pixels, 192);
        }
        assert_eq!(mem_sys::local_alloc_count(), 0);
        images.clear();
        assert_eq!(images.oldest(), 0);
        assert!(!Request::default().insert(1, &"x".repeat(1025), 512));
    }
    #[test]
    fn only_final_visible_image_nodes_request_decodes_and_cached_nodes_keep_their_requests() {
        let images = Images::default();
        let mut ui = crate::Ui::new().unwrap();
        for frame in 0..2 {
            ui.frame(
                &crate::FrameIn {
                    width: 200.0,
                    height: 100.0,
                    time: frame as f64,
                    ..Default::default()
                },
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
                            }.into(),
                            crate::ui::Node {
                                w: crate::ui::Size::Px(30.0),
                                h: crate::ui::Size::Px(30.0),
                                at: Some((0.0, y)).into(),
                                ..crate::ui::Node::DEFAULT
                            },
                        );
                        if frame == 0 {
                            images.get_at(id, "cover.png", 30.0, node.box_key);
                        }
                    }
                },
            )
            .unwrap();
            let packet = images.visible_requests(&ui);
            assert_eq!(packet.keys().len(), if frame == 0 { 2 } else { 3 });
            assert_eq!(packet.keys()[0].id, 1);
        }
        mem_sys::local_reset();
        for _ in 0..1000 {
            assert_eq!(images.visible_requests(&ui).keys().len(), 3);
        }
        assert_eq!(mem_sys::local_alloc_count(), 0);
        ui.frame(
            &crate::FrameIn {
                width: 200.0,
                height: 100.0,
                time: 2.0,
                ..Default::default()
            },
            &mut |_| {},
        )
        .unwrap();
        assert!(
            images.visible_requests(&ui).keys().is_empty(),
            "retired provisional cells leave no pending work"
        );
    }
    #[test]
    fn retained_views_keep_pending_covers_until_completion_instead_of_starving_other_panels() {
        let mut images = Images::default();
        for id in 1..=12 {
            images.get(id, "cover.png", 184.0);
        }
        assert_eq!(images.requests().keys().len(), REQUEST_CAPACITY);
        // A cached album view makes no calls on this frame; a live player still asks for its cover.
        images.get(100, "player.png", 30.0);
        assert_eq!(images.requests().keys()[0].id, 1);
        for id in 1..=8 {
            let key = images.requests().keys()[0];
            assert_eq!(key.id, id);
            images.insert(id - 1, key, None);
        }
        assert_eq!(
            images.requests().keys()[0].id,
            9,
            "visible requests beyond the first packet survive until delivery"
        );
        for id in 9..=12 {
            images.get(id, "cover.png", 184.0);
        }
        images.get(100, "player.png", 30.0);
        assert_eq!(images.requests().keys().len(), 5);
        assert_eq!(images.requests().keys()[4].id, 100);
        images.cancel_requests();
        assert!(images.requests().keys().is_empty());
    }
    #[test]
    fn visible_galleries_larger_than_sixty_four_keep_all_descriptors_without_warm_allocations() {
        let mut images = Images::default();
        let path = Path::new("cover.png").unwrap();
        for index in 0..768 {
            images.insert(
                index,
                Key {
                    id: index + 1,
                    path,
                    max_pixels: 512,
                },
                Some(Image {
                    width: 8,
                    height: 4,
                    ..Default::default()
                }),
            );
        }
        mem_sys::local_reset();
        for _ in 0..10 {
            for id in 1..=768 {
                assert!(images.get(id, path.as_str(), 184.0).is_some());
            }
        }
        assert!(images.requests().keys().is_empty());
        assert_eq!(mem_sys::local_alloc_count(), 0);
    }
    #[test]
    fn an_id_inserted_into_a_second_slot_leaves_its_first_slot_and_removing_that_slot_keeps_the_mapping() {
        let mut images = Images::default();
        let key = Key {
            id: 7,
            max_pixels: 64,
            ..Default::default()
        };
        images.insert(0, key, None);
        images.insert(3, key, None);
        assert_eq!(images.index(7), Some(3));
        images.remove(0);
        assert_eq!(images.index(7), Some(3), "removing the old slot must not drop the new mapping");
        assert_eq!(images.quality(7), Some(64));
        images.insert(3, Key { id: 8, ..key }, None);
        assert_eq!((images.index(7), images.index(8)), (None, Some(3)));
    }
    #[test]
    fn cached_quality_does_not_retry_small_sources() {
        let mut images = Images::default();
        images.configure(2.0, 384);
        let key = Key {
            id: 1,
            path: Path::new("tiny.png").unwrap(),
            max_pixels: 384,
        };
        images.insert(
            0,
            key,
            Some(Image {
                width: 8,
                height: 4,
                ..Default::default()
            }),
        );
        assert!(images.get(1, "tiny.png", 184.0).is_some());
        assert!(
            images.requests().keys().is_empty(),
            "record requested quality, not source dimensions"
        );
    }
}
