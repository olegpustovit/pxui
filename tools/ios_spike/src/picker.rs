//! Manual UIKit presentation/delegate/lifetime probe; provider grants still need a real device.
use pxui_sdl::{DialogFilter, DialogKind, Window};
use sdl3_sys::test_util as native;

pub struct Probe {
    phase: u8,
    id: i64,
    identity: usize,
    deadline: f64,
    events: Vec<input::Event>,
    worker: Option<std::thread::JoinHandle<Result<(), String>>>,
}
impl Probe {
    pub fn new(now: f64) -> Self {
        Self {
            phase: 0,
            id: 0,
            identity: 0,
            deadline: now + 10.0,
            events: Vec::with_capacity(32),
            worker: None,
        }
    }
    fn open(&mut self, window: &mut Window, kind: DialogKind) -> Result<(), String> {
        if !native::ios_picker_idle() { return Ok(()); }
        let filters = [DialogFilter {
            name: "Audio".into(),
            pattern: "mp3;flac;m4a".into(),
        }];
        self.id = window.show_dialog(&filters, None, kind)?;
        self.phase += 1;
        self.deadline = window.time() + 5.0;
        Ok(())
    }
    pub fn step(&mut self, window: &mut Window) -> Result<bool, String> {
        if window.time() >= self.deadline {
            return Err(format!("Document picker timed out at phase {}", self.phase));
        }
        self.events.clear();
        if !window.pump(&mut self.events) {
            return Err("Window closed during document picker probe".into());
        }
        match self.phase {
            0 => {
                if window.show_dialog(&[], None, DialogKind::SaveFile).is_ok() {
                    return Err("Path-only export was incorrectly accepted".into());
                }
                self.open(window, DialogKind::OpenFolder)?;
            }
            1 | 5 | 8 | 11 if native::ios_picker_ready() => {
                let mode = match self.phase {
                    5 => 0,
                    8 => 2,
                    _ => 1,
                };
                let path = std::env::temp_dir();
                self.identity = native::ios_picker_finish(
                    mode,
                    path.to_str().ok_or("Non-UTF-8 temporary directory")?,
                );
                if self.identity == 0 {
                    return Err("Native picker delegate could not finish".into());
                }
                self.phase += 1;
            }
            2 | 6 | 9 | 12 => {
                let result = self.events.iter().find_map(|event| match event {
                    input::Event::FileDialog { id, result } if *id == self.id => Some(result),
                    _ => None,
                });
                let Some(result) = result else {
                    return Ok(false);
                };
                if self.events.iter().filter(|event| matches!(event, input::Event::FileDialog { id, .. } if *id == self.id)).count() != 1 {
                    return Err("Document picker delivered duplicate outcomes".into());
                }
                match self.phase {
                    2 => {
                        if result.as_ref().map_or(true, |paths| paths.len() != 1) {
                            return Err("Folder selection lost its path".into());
                        }
                        let mut selected = window.take_dialog_selection(self.id)?;
                        if selected.len() != 1 || window.take_dialog_selection(self.id).is_ok() {
                            return Err("Document capability was not transferred once".into());
                        }
                        let file = selected.pop().unwrap();
                        if file.bookmark().is_ok() {
                            return Err("Document bookmark ran on the UI thread".into());
                        }
                        let identity = self.identity;
                        self.worker = Some(std::thread::spawn(move || {
                            let result = (|| {
                                let data = file
                                    .bookmark()
                                    .map_err(|error| error.to_string())?
                                    .ok_or("Native document grant was lost")?;
                                if data.is_empty() {
                                    return Err("Native bookmark is empty".into());
                                }
                                let (seen, calls, starts, stops) = native::file_selection_status();
                                if seen != identity || calls != 1 || starts != stops {
                                    return Err(
                                        "Bookmark did not use the original URL with balanced scope"
                                            .into(),
                                    );
                                }
                                Ok(())
                            })();
                            pxui_sdl::wake();
                            result
                        }));
                    }
                    6 => {
                        if !matches!(result, Ok(paths) if paths.is_empty()) {
                            return Err("Picker cancellation was not empty".into());
                        }
                    }
                    9 => {
                        if result.is_ok() {
                            return Err("Non-file picker URL was accepted".into());
                        }
                    }
                    12 => {
                        if result.as_ref().map_or(true, |paths| paths.len() != 1) {
                            return Err("Expiry fixture did not choose a file".into());
                        }
                    }
                    _ => unreachable!(),
                }
                self.phase += 1;
            }
            3 if self
                .worker
                .as_ref()
                .is_some_and(|worker| worker.is_finished()) =>
            {
                self.worker
                    .take()
                    .unwrap()
                    .join()
                    .map_err(|_| "Document worker panicked")??;
                println!(
                    "PXUI_IOS_PICKER_ORIGINAL_URL_OK worker_bookmark=1 main_thread_refused=1 balanced_scope=1"
                );
                self.phase = 4;
            }
            4 => self.open(window, DialogKind::OpenFiles)?,
            7 => self.open(window, DialogKind::OpenFile)?,
            10 => self.open(window, DialogKind::OpenFile)?,
            13 => {
                if window.take_dialog_selection(self.id).is_ok() {
                    return Err("Ignored document capability survived its next pump".into());
                }
                self.phase = 14;
            }
            14 => self.open(window, DialogKind::OpenFolder)?,
            15 if native::ios_picker_ready() => {
                if !native::ios_picker_close() {
                    return Err("Closing the window left a document picker alive".into());
                }
                let slots: Vec<_> = (0..8).map(|_| native::dialog_reserve()).collect();
                let full = native::dialog_reserve();
                for id in &slots {
                    native::dialog_abandon(*id);
                }
                if full != 0 {
                    native::dialog_abandon(full);
                }
                if slots.iter().any(|id| *id <= 0) || full != 0 {
                    return Err(
                        "Document teardown leaked or exceeded the eight native slots".into(),
                    );
                }
                self.phase = 16;
                println!(
                    "PXUI_IOS_PICKER_OK chosen=1 cancel=1 invalid=1 duplicate_suppressed=1 expired=1 teardown_slots=8"
                );
                return Ok(true);
            }
            _ => {}
        }
        Ok(false)
    }
}
