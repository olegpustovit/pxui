//! Cached views keyed by their inputs: `ui.view_of(key, &inputs, build)` rebuilds only when `hash_of(&inputs)` changes.
use core::hash::{Hash, Hasher};
use pxui_types::{PxuiBox, Size};
use crate::Ui;
use super::Key;

/// FNV-1a 64: fixed seed, std only, no allocation, so a hash is the same on every run and platform (std's own hashers promise neither).
struct FixedHasher(u64);

impl Hasher for FixedHasher {
    #[inline]
    fn finish(&self) -> u64 { self.0 }
    #[inline]
    fn write(&mut self, bytes: &[u8]) { for byte in bytes { self.0 = (self.0 ^ *byte as u64).wrapping_mul(0x0000_0100_0000_01b3); } }
    // A pointer-width integer hashes as 64 bits, so 32 and 64-bit targets agree.
    #[inline]
    fn write_usize(&mut self, value: usize) { self.write(&(value as u64).to_le_bytes()) }
    #[inline]
    fn write_isize(&mut self, value: isize) { self.write(&(value as i64).to_le_bytes()) }
}

/// Runs `f` on a fresh hasher and returns its result: the one non-generic body of `hash_of`.
fn hash_run(f: &mut dyn FnMut(&mut FixedHasher)) -> u64 {
    let mut hasher = FixedHasher(0xcbf2_9ce4_8422_2325);
    f(&mut hasher);
    hasher.finish()
}

/// Hashes `value` with a fixed-seed FNV-1a 64 hasher: the same value gives the same number on every run and platform.
///
/// Integers hash as their little-endian bytes (`usize` as 64 bits); everything else as its `Hash` impl feeds the hasher, so the number
/// is stable only while the type's `Hash` impl is. Use it as the `revision` of a view.
/// Allocates: never.
/// Panics: never (unless `T::hash` does).
///
/// ```
/// assert_eq!(pxui::hash_of(&1u32), pxui::hash_of(&1u32));
/// assert_ne!(pxui::hash_of(&1u32), pxui::hash_of(&2u32));
/// assert_eq!(pxui::hash_of(&0u8), 0xaf63_bd4c_8601_b7df);
/// ```
#[inline]
pub fn hash_of<T: Hash + ?Sized>(value: &T) -> u64 { hash_run(&mut |hasher| value.hash(hasher)) }

impl Ui {
    /// Declares a view that fills its parent and runs `f` only when `hash_of(inputs)` differs from its last build; otherwise the retained subtree is reused.
    ///
    /// `inputs` must contain everything `f` reads (data, sizes, flags): a value it reads but `inputs` omits goes stale. Same as `view` with
    /// `Size::Grow` on both axes and `hash_of(inputs)` as the revision.
    /// Allocates: as `view` (never on a warm frame, kept or rebuilt).
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); an id never collides with a
    /// printable label.
    /// Panics: when the library refuses the view (outside a frame), or when `f` panics.
    ///
    /// ```
    /// use std::cell::Cell;
    /// let runs = Cell::new(0);
    /// pxui::doc::frames(3, &mut |ui| { ui.view_of("title", &("Songs", 12u32), |ui| { runs.set(runs.get() + 1); ui.label("Songs", crate::Label::DEFAULT); }); });
    /// assert_eq!(runs.get(), 1);
    /// ```
    #[inline]
    #[track_caller]
    pub fn view_of<K: Into<Key>, T: Hash + ?Sized, F: FnMut(&mut Ui)>(&mut self, key: K, inputs: &T, mut f: F) -> PxuiBox { self.view_keyed(key.into(), hash_of(inputs), &mut f) }

    /// The one body of `view_of`: a view of `revision` declared by `key`, filling its parent.
    #[inline]
    #[track_caller]
    fn view_keyed(&mut self, key: Key, revision: u64, f: &mut dyn FnMut(&mut Ui)) -> PxuiBox {
        let mut id_label = [0u8; 17];
        let label = key.label(&mut id_label);
        self.view(label, revision, Size::Grow, Size::Grow, f)
    }
}

#[cfg(test)]
mod tests {
    use super::{hash_of, FixedHasher};
    use core::hash::Hasher;
    use crate::FrameIn;
    use std::cell::Cell;

    /// The hasher is FNV-1a 64 with its standard seed: these numbers never change between runs, releases or platforms.
    #[test]
    fn hash_of_is_pinned() {
        let fnv = |bytes: &[u8]| { let mut hasher = FixedHasher(0xcbf2_9ce4_8422_2325); hasher.write(bytes); hasher.finish() };
        // the published FNV-1a 64 test vectors
        assert_eq!(fnv(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv(b"foobar"), 0x8594_4171_f739_67e8);
        assert_eq!(hash_of(&0u8), 0xaf63_bd4c_8601_b7df);
        assert_eq!(hash_of(&("a", 1u32)), hash_of(&("a", 1u32)));
        assert_ne!(hash_of(&("a", 1u32)), hash_of(&("a", 2u32)));
        assert_eq!(hash_of(&7usize), hash_of(&7u64));
    }

    /// The body runs once across equal inputs, again when one input changes, and kept frames allocate nothing.
    #[test]
    fn view_of_rebuilds_only_when_inputs_change() {
        let runs = Cell::new(0u32);
        let mut ui = crate::Ui::new().expect("library");
        let input = FrameIn { width: 320.0, height: 240.0, ..FrameIn::default() };
        let frame = |ui: &mut crate::Ui, n: u32| {
            ui.frame(&input, &mut |ui| { ui.view_of("v", &("title", n), |ui| { runs.set(runs.get() + 1); ui.label("title", crate::Label::DEFAULT); }); }).expect("frame");
        };
        for _ in 0..10 { frame(&mut ui, 1); }
        assert_eq!(runs.get(), 1);
        frame(&mut ui, 2);
        assert_eq!(runs.get(), 2);
        for _ in 0..4 { frame(&mut ui, 2); }
        mem_sys::local_reset();
        for _ in 0..4 { frame(&mut ui, 2); }
        assert_eq!(mem_sys::local_alloc_count(), 0, "kept frames allocate nothing");
        assert_eq!(runs.get(), 2);
    }
}
