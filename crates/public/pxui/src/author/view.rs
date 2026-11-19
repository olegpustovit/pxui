//! Cached views keyed by their inputs: `ui.view_of(key, &inputs, build)` rebuilds only when `hash_of(&inputs)` changes.
use crate::Handle;
use core::hash::{Hash, Hasher};
use crate::Ui;
use super::Key;

/// FNV-1a 64 over bytes, an integer folded in as one 64-bit word: fixed seed, std only, no allocation, so a hash is the same on every
/// run and platform (std's own hashers promise neither).
struct FixedHasher(u64);

const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

impl FixedHasher {
    /// One FNV-1a step over a whole word: a single integer (a view's revision) costs a multiply, not eight. Each step is a bijection
    /// of the state, so two different integers never hash alike.
    #[inline]
    fn word(&mut self, value: u64) { self.0 = (self.0 ^ value).wrapping_mul(FNV_PRIME); }
}

impl Hasher for FixedHasher {
    #[inline]
    fn finish(&self) -> u64 { self.0 }
    #[inline]
    fn write(&mut self, bytes: &[u8]) { for byte in bytes { self.0 = (self.0 ^ *byte as u64).wrapping_mul(FNV_PRIME); } }
    // Every integer up to 64 bits is one word, sign-extended; a pointer-width one too, so 32 and 64-bit targets agree.
    #[inline]
    fn write_u8(&mut self, value: u8) { self.word(value as u64) }
    #[inline]
    fn write_u16(&mut self, value: u16) { self.word(value as u64) }
    #[inline]
    fn write_u32(&mut self, value: u32) { self.word(value as u64) }
    #[inline]
    fn write_u64(&mut self, value: u64) { self.word(value) }
    #[inline]
    fn write_usize(&mut self, value: usize) { self.word(value as u64) }
    #[inline]
    fn write_i8(&mut self, value: i8) { self.word(value as i64 as u64) }
    #[inline]
    fn write_i16(&mut self, value: i16) { self.word(value as i64 as u64) }
    #[inline]
    fn write_i32(&mut self, value: i32) { self.word(value as i64 as u64) }
    #[inline]
    fn write_i64(&mut self, value: i64) { self.word(value as u64) }
    #[inline]
    fn write_isize(&mut self, value: isize) { self.word(value as i64 as u64) }
}

/// Runs `f` on a fresh hasher and returns its result: the one non-generic body of `hash_of` (inline: a small input's hash is a few
/// multiplies, less than the call).
#[inline]
fn hash_run(f: &mut dyn FnMut(&mut FixedHasher)) -> u64 {
    let mut hasher = FixedHasher(0xcbf2_9ce4_8422_2325);
    f(&mut hasher);
    hasher.finish()
}

/// Hashes `value` with a fixed-seed FNV-1a 64 hasher: the same value gives the same number on every run and platform.
///
/// Bytes (a `str`'s, a slice's) hash one FNV-1a step each; an integer of up to 64 bits one step of its whole value as a 64-bit word
/// (signed ones sign-extended, `usize` as 64 bits), so two different integers never hash alike. Anything else hashes as its `Hash`
/// impl feeds the hasher, so the number is stable only while the type's `Hash` impl is. The numbers are for comparing within a run;
/// nothing should persist them. Use it as the `revision` of a view.
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
    /// Declares a view that fills its parent and runs `f` only when `hash_of(inputs)` differs from its last build; otherwise the retained
    /// subtree is reused.
    ///
    /// `inputs` must contain everything `f` reads (data, sizes, flags): a value it reads but `inputs` omits goes stale. A hand-kept
    /// revision number is an input like any other (`&revision`). The view's size comes from the enclosing node: it fills it, except on
    /// the main axis of a row or column whose size there is `Fit`, where it takes its content's size (a content-sized view). The
    /// returned handle is the view's, valid this frame (`set_view`, `animate_keys`, `reveal` take it).
    /// Allocates: never on a warm frame, kept or rebuilt; a new identity takes a slot the first time.
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); an id never collides with a
    /// printable label.
    /// Panics: when the library refuses the view (outside a frame), or when `f` panics. A key or identity label over 256 bytes panics (a programmer error).
    ///
    /// ```
    /// use std::cell::Cell;
    /// let runs = Cell::new(0);
    /// let mut ui = pxui::Ui::new(&pxui::UiDesc::DEFAULT).unwrap();
    /// ui.verify_views(false); // a debug build otherwise rebuilds every kept view to verify it
    /// for _ in 0..3 {
    ///     ui.frame(&pxui::FrameIn::new(320.0, 240.0), &mut |ui| { ui.view_of("title", &("Songs", 12u32), |ui| { runs.set(runs.get() + 1); ui.label("Songs", pxui::Label::DEFAULT); }); }).unwrap();
    /// }
    /// assert_eq!(runs.get(), 1);
    /// ```
    #[inline]
    #[track_caller]
    pub fn view_of<K: Into<Key>, T: Hash + ?Sized, F: FnMut(&mut Ui)>(&mut self, key: K, inputs: &T, mut f: F) -> Handle { self.view_key(key.into(), hash_of(inputs), &mut f) }
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
        // an integer is one step of its whole 64-bit word (signed ones sign-extended)
        assert_eq!(hash_of(&0x0102_0304_0506_0708u64), (0xcbf2_9ce4_8422_2325u64 ^ 0x0102_0304_0506_0708).wrapping_mul(0x0000_0100_0000_01b3));
        assert_eq!(hash_of(&-1i8), hash_of(&u64::MAX));
        assert_eq!(hash_of(&5u16), hash_of(&5u32));
    }

    /// The body runs once across equal inputs, again when one input changes, and kept frames allocate nothing.
    #[test]
    fn view_of_rebuilds_only_when_inputs_change() {
        let runs = Cell::new(0u32);
        let mut ui = crate::Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        ui.verify_views(false); // the test counts rebuilds; verification rebuilds every kept view
        let input = FrameIn::new(320.0, 240.0);
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

    /// A view fills its enclosing node, and on the main axis of a `Fit` column it takes its content's size, kept or rebuilt; the kept
    /// frames allocate nothing.
    #[test]
    fn view_of_fills_its_parent_and_fits_a_fit_column() {
        use crate::{Node, ui::Size};
        let (fixed, fitted) = (Cell::new(None), Cell::new(None));
        let mut ui = crate::Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        let input = FrameIn::new(320.0, 240.0);
        let mut build = |ui: &mut crate::Ui| {
            ui.column(Node { w: Size::Px(120.0), h: Size::Px(40.0), ..Node::DEFAULT }, |ui| {
                fixed.set(Some(ui.view_of("v", &1u8, |ui| { ui.node("a", Node { h: Size::Px(10.0), ..Node::DEFAULT }); })));
            });
            ui.column(Node { w: Size::Px(80.0), ..Node::DEFAULT }, |ui| {
                fitted.set(Some(ui.view_of("v", &1u8, |ui| { ui.node("a", Node { h: Size::Px(17.0), ..Node::DEFAULT }); })));
            });
        };
        for _ in 0..4 { ui.frame(&input, &mut build).expect("frame"); }
        let (f, g) = (ui.rect(&fixed.get().unwrap()), ui.rect(&fitted.get().unwrap()));
        assert_eq!((f.w, f.h), (120.0, 40.0), "fills a fixed parent");
        assert_eq!((g.w, g.h), (80.0, 17.0), "takes its content's height in a Fit column, the parent's width across");
        mem_sys::local_reset();
        for _ in 0..4 { ui.frame(&input, &mut build).expect("frame"); }
        assert_eq!(mem_sys::local_alloc_count(), 0, "kept frames allocate nothing");
        let g = ui.rect(&fitted.get().unwrap());
        assert_eq!((g.w, g.h), (80.0, 17.0), "kept, it keeps its measured height");
    }
}
