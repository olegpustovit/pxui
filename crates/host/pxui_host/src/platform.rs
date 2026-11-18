use crate::Window;
use pxui::Error;

/// One UI/window pair's platform output and pending clipboard input: one per surface. The clipboard
/// itself is the system's (`pxui_sdl::clipboard_*`), shared by every window of the process.
#[derive(Default)]
pub struct Platform {
    applied: Option<(usize, u64)>,
    pending: Vec<input::Event>,
    environment: pxui::Environment,
    keyboard: pxui::Keyboard,
    #[cfg(target_os = "ios")]
    ios_clock: Option<f64>,
}
impl Platform {
    /// Appends pending paste input before the window's next batch.
    ///
    /// Returns false when the window closes. On iOS, a native environment read can
    /// return `Error::Host`.
    pub fn pump(
        &mut self,
        window: &mut Window,
        events: &mut Vec<input::Event>,
    ) -> Result<bool, Error> {
        events.append(&mut self.pending);
        let open = window.pump(events);
        #[cfg(target_os = "ios")]
        if open {
            let raw = pxui_sdl::ios::environment(window).map_err(|_| Error::Host("could not read native environment"))?;
            let offset = *self
                .ios_clock
                .get_or_insert_with(|| window.time() - raw.timestamp);
            let value = pxui::Environment {
                safe_area: raw.safe_area,
                keyboard_frame: raw.keyboard_frame,
                keyboard_from: raw.keyboard_from,
                keyboard_started: if raw.keyboard_started == 0.0 {
                    0.0
                } else {
                    raw.keyboard_started + offset
                },
                keyboard_duration: raw.keyboard_duration,
                keyboard_curve: raw.keyboard_curve,
                keyboard_visible: raw.keyboard_visible,
            };
            let keyboard = pxui::Keyboard { frame: raw.keyboard_current, flags: raw.keyboard_flags };
            if value != self.environment || keyboard != self.keyboard {
                self.keyboard = keyboard;
                self.environment = value;
                events.push(input::Event::EnvironmentChanged);
            }
        }
        Ok(open)
    }
    /// Feed this snapshot to FrameIn::environment after pumping input.
    pub fn environment(&self) -> pxui::Environment {
        self.environment
    }
    /// Current native geometry; pass to FrameIn::keyboard with the environment.
    pub fn keyboard(&self) -> pxui::Keyboard { self.keyboard }
    /// Applies the UI's text input, cursor, background and clipboard output to its window.
    ///
    /// Call after every frame, including frames with unchanged draw output. Native
    /// failures return `Error::Host` with the operation that failed.
    pub fn apply(&mut self, window: &mut Window, ui: &pxui::Ui) -> Result<(), Error> {
        let out = ui.output().platform;
        let ime = (out.has_ime != 0).then(|| {
            (
                (
                    out.ime_rect[0],
                    out.ime_rect[1],
                    out.ime_rect[2],
                    out.ime_rect[3],
                ),
                out.ime_caret,
            )
        });
        let options=if ime.is_some() {ui.text_input_options()} else {pxui::TextTraits::default()};
        window.set_text_input_with_options(ime,pxui_sdl::TextInputOptions {
            autocorrect:options.autocorrects() as u32,capitalization:options.capitalizes() as u32,action:options.return_key() as u32,
        }).map_err(|_| Error::Host("could not set native text input"))?;
        let frame = (ui.id(), ui.frame_index());
        if self.applied == Some(frame) {
            return Ok(());
        }
        #[cfg(not(target_os = "ios"))]
        {
            use pxui_sdl::CursorShape;
            let cursor = if out.has_cursor == 0 {
                CursorShape::Default
            } else {
                match out.cursor {
                    1 => CursorShape::HorizontalResize,
                    2 => CursorShape::VerticalResize,
                    3 => CursorShape::Text,
                    _ => CursorShape::Default,
                }
            };
            window.set_cursor(cursor).map_err(|_| Error::Host("could not set window cursor"))?;
        }
        if out.has_background != 0 {
            let color = out.background;
            window.set_background((
                f64::from(color.r) / 255.0,
                f64::from(color.g) / 255.0,
                f64::from(color.b) / 255.0,
            )).map_err(|_| Error::Host("could not set window background"))?;
        }
        if out.has_clipboard != 0
            && let Some(text) = ui.clipboard_request()
        {
            pxui_sdl::clipboard_set_text(text).map_err(|_| Error::Host("could not write the clipboard"))?;
        }
        if out.paste != 0 {
            self.pending
                .push(input::Event::Paste(pxui_sdl::clipboard_get_text().map_err(|_| Error::Host("could not read the clipboard"))?));
            pxui_sdl::wake();
        }
        self.applied = Some(frame);
        Ok(())
    }
}
