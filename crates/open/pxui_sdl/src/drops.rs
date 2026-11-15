//! SDL can deliver one drop across multiple pumps. Own its paths until completion; focus loss
//! only dismisses hover. Bound retained transport data and reject an overflowing set wholly.
use input::Event;
use sdl3::{DropChange, Event as Native};

#[derive(Default)]
pub(super) struct Drops {
    active: bool,
    paths: Vec<String>,
    bytes: usize,
    invalid: bool,
}

impl Drops {
    pub(super) fn translate(&mut self, raw: Native) -> Option<Event> {
        let Native::Drop { change, x, y } = raw else { return super::translate::translate(raw); };
        match change {
            DropChange::Begin => {
                let cancelled = self.active;
                self.paths.clear(); self.bytes = 0; self.invalid = false; self.active = false;
                cancelled.then_some(Event::FileDragEnded)
            }
            DropChange::Position => {
                self.active = true;
                Some(Event::FileDragMoved((x,y)))
            }
            DropChange::File(path) => {
                self.bytes = self.bytes.saturating_add(path.len());
                if self.invalid || self.paths.len() == 1_000_000 || self.bytes > 64*1024*1024 || path.is_empty() || path.contains('\0') {
                    self.invalid = true;
                    self.paths.clear();
                } else {
                    self.paths.push(path);
                }
                // Capture the application owner even when SDL supplies no preceding hover.
                let first = !self.active;
                self.active = true;
                first.then_some(Event::FileDragMoved((x,y)))
            }
            DropChange::Complete => {
                let event = if self.invalid {
                    Event::FilesDropped {position:(x,y),paths:Err("The file drop was invalid or too large.".into())}
                } else if self.paths.is_empty() {
                    Event::FileDragEnded
                } else {
                    Event::FilesDropped {position:(x,y),paths:Ok(std::mem::take(&mut self.paths))}
                };
                self.active = false; self.bytes = 0; self.invalid = false;
                Some(event)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overflow_rejects_whole_batch_and_empty_cancel_reuses_storage() {
        let mut drops = Drops::default();
        let event = |change| Native::Drop {change,x:1.0,y:2.0};
        drops.translate(event(DropChange::File("/first.flac".into())));
        drops.bytes = 64*1024*1024;
        drops.translate(event(DropChange::File("/second.flac".into())));
        assert!(drops.paths.is_empty());
        assert!(matches!(drops.translate(event(DropChange::Complete)),Some(Event::FilesDropped {paths:Err(_),..})));
        drops.translate(event(DropChange::File("/abandoned.flac".into())));
        assert_eq!(drops.translate(event(DropChange::Begin)),Some(Event::FileDragEnded));
        assert_eq!(drops.translate(event(DropChange::Complete)),Some(Event::FileDragEnded));
        let capacity = drops.paths.capacity();
        mem_sys::local_reset();
        for _ in 0..100 {
            assert_eq!(drops.translate(event(DropChange::Begin)),None);
            assert_eq!(drops.translate(event(DropChange::Position)),Some(Event::FileDragMoved((1.0,2.0))));
            assert_eq!(drops.translate(Native::Window(sdl3::WindowChange::FocusLost)),Some(Event::FocusLost));
            assert_eq!(drops.translate(event(DropChange::Complete)),Some(Event::FileDragEnded));
        }
        assert_eq!(mem_sys::local_alloc_count(),0);
        assert_eq!(drops.paths.capacity(),capacity);
    }
}
