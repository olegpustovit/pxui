#[derive(Clone, Copy)]
pub(super) enum Field { Text, Name, Value }
impl Field {
    pub(super) const ALL: [Self;3] = [Self::Text,Self::Name,Self::Value];
}

/// Capacity buckets: one per capacity below `LAST`, and `LAST` for every larger one.
const LAST: usize = 255;
const NONE: u32 = u32::MAX;

/// One field's empty buffers by capacity: a doubly linked list per bucket through per-slot links, and a bit per non-empty bucket,
/// so adding, moving and taking an entry are O(1) and the best fit is the first set bit at or past the wanted length.
#[derive(Default)]
struct Buckets {
    head: Vec<u32>,
    bits: [u64; 4],
    /// Per slot: its list neighbours and the capacity it is listed under (0: not listed).
    prev: Vec<u32>,
    next: Vec<u32>,
    listed: Vec<usize>,
}
impl Buckets {
    fn clear(&mut self) {
        self.head.fill(NONE);
        self.bits = [0; 4];
        self.listed.fill(0);
    }
    fn link(&mut self, slot: usize, capacity: usize) {
        let b = capacity.min(LAST);
        let first = self.head[b];
        (self.prev[slot], self.next[slot], self.listed[slot]) = (NONE, first, capacity);
        if first != NONE { self.prev[first as usize] = slot as u32; }
        self.head[b] = slot as u32;
        self.bits[b / 64] |= 1 << (b % 64);
    }
    fn unlink(&mut self, slot: usize) {
        let b = self.listed[slot].min(LAST);
        let (prev, next) = (self.prev[slot], self.next[slot]);
        if prev == NONE { self.head[b] = next; } else { self.next[prev as usize] = next; }
        if next != NONE { self.prev[next as usize] = prev; }
        if self.head[b] == NONE { self.bits[b / 64] &= !(1 << (b % 64)); }
        self.listed[slot] = 0;
    }
    /// The first non-empty bucket at or past `from`.
    fn bucket_from(&self, from: usize) -> Option<usize> {
        let mut word = from / 64;
        let mut bits = self.bits.get(word)? & (!0u64 << (from % 64));
        loop {
            if bits != 0 { return Some(word * 64 + bits.trailing_zeros() as usize); }
            word += 1;
            bits = *self.bits.get(word)?;
        }
    }
}

/// Empty text/name/value buffers by capacity, built on a field's first donor query and kept current after: every buffer that turns
/// empty is added (clears, retirement, empty writes, donor swaps); a write leaves its entry, which a donor query drops when it meets it.
#[derive(Default)]
pub(super) struct EmptyStrings {
    fields: [Buckets;3],
    active: u8,
}
impl EmptyStrings {
    pub(super) fn reserve(&mut self, slots: usize) {
        for f in &mut self.fields {
            f.head.resize(LAST + 1, NONE);
            if f.listed.len() < slots {
                f.prev.resize(slots, NONE);
                f.next.resize(slots, NONE);
                f.listed.resize(slots, 0);
            }
        }
    }
    /// Slot `slot`'s buffer is empty and holds `capacity` (an active field only; a zero capacity is never a donor).
    pub(super) fn empty(&mut self, field: Field, slot: usize, capacity: usize) {
        if self.active & (1 << field as usize) == 0 || capacity==0 { return; }
        let f = &mut self.fields[field as usize];
        match f.listed[slot] {
            0 => {}
            listed if listed == capacity => return,
            _ => f.unlink(slot),
        }
        f.link(slot, capacity);
    }
    pub(super) fn active(&self) -> bool {self.active!=0}
    /// Starts the field's index (cleared: the caller adds every empty buffer); false once it is built.
    pub(super) fn rebuild(&mut self, field: Field) -> bool {
        let bit=1 << field as usize;
        if self.active & bit != 0 {return false;}
        self.active|=bit;
        self.fields[field as usize].clear();
        true
    }
    /// The slot whose empty buffer fits `len` best, for slot `own` (its buffer holds `capacity`), taken from the index; `empty_capacity`
    /// reads a slot's buffer capacity when that buffer is empty now (`None`: written since it was indexed, dropped as met).
    pub(super) fn donor(&mut self, field: Field, own: usize, capacity: usize, len: usize, empty_capacity: &dyn Fn(usize) -> Option<usize>) -> Option<usize> {
        let f = &mut self.fields[field as usize];
        let mut from = len.min(LAST);
        while let Some(b) = f.bucket_from(from) {
            // the best fit in the bucket: its only capacity, or the smallest fitting one in the last
            let (mut best, mut at) = (None, f.head[b]);
            while at != NONE {
                let slot = at as usize;
                at = f.next[slot];
                let listed = f.listed[slot];
                if empty_capacity(slot) != Some(listed) { f.unlink(slot); continue; }
                if slot == own || listed < len { continue; }
                if best.is_none_or(|(c, s)| (listed, slot) < (c, s)) { best = Some((listed, slot)); }
                if b < LAST { break; }
            }
            if let Some((fitting, slot)) = best {
                if capacity>=len && fitting>=capacity {return None;}
                f.unlink(slot);
                return Some(slot);
            }
            from = b + 1;
            if from > LAST { break; }
        }
        None
    }
}
