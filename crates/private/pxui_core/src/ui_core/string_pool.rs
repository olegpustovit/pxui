#[derive(Clone, Copy)]
pub(super) enum Field { Text, Name, Value }
impl Field {
    pub(super) const ALL: [Self;3] = [Self::Text,Self::Name,Self::Value];
}

/// Sorted (capacity, slot) pairs, built on the first donor query of a frame.
#[derive(Default)]
pub(super) struct EmptyStrings {
    indices: [Vec<(usize,usize)>;3],
    active: u8,
}
impl EmptyStrings {
    pub(super) fn reserve(&mut self, slots: usize) {
        for index in &mut self.indices { index.reserve(slots.saturating_sub(index.len())); }
    }
    pub(super) fn begin_frame(&mut self) { self.active=0; }
    pub(super) fn used(&mut self, field: Field, slot: usize, capacity: usize) {
        if self.active & (1 << field as usize) == 0 || capacity==0 { return; }
        let index=&mut self.indices[field as usize];
        if let Ok(at)=index.binary_search(&(capacity,slot)) { index.remove(at); }
    }
    pub(super) fn empty(&mut self, field: Field, slot: usize, capacity: usize) {
        if self.active & (1 << field as usize) == 0 || capacity==0 { return; }
        let index=&mut self.indices[field as usize];
        if let Err(at)=index.binary_search(&(capacity,slot)) { index.insert(at,(capacity,slot)); }
    }
    pub(super) fn active(&self) -> bool {self.active!=0}
    pub(super) fn rebuild(&mut self, field: Field) -> Option<&mut Vec<(usize,usize)>> {
        let bit=1 << field as usize;
        if self.active & bit != 0 {return None;}
        self.active|=bit;
        let index=&mut self.indices[field as usize];
        index.clear();
        Some(index)
    }
    pub(super) fn donor(&mut self, field: Field, own: usize, capacity: usize, len: usize) -> Option<usize> {
        self.used(field,own,capacity);
        let index=&mut self.indices[field as usize];
        let at=index.partition_point(|entry|entry.0<len);
        let &(fitting,slot)=index.get(at)?;
        if capacity>=len && fitting>=capacity {return None;}
        index.remove(at);
        Some(slot)
    }
}

