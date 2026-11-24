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
            let started = if raw.keyboard_started == 0.0 { 0.0 } else { raw.keyboard_started + offset };
            let mut value = pxui::Environment::new(raw.safe_area).keyboard_animation(
                raw.keyboard_frame, raw.keyboard_from, started, raw.keyboard_duration, raw.keyboard_curve, raw.keyboard_visible != 0,
            );
            // the native flag is the 0 or 1 UIKit reports: keep it as given, the frame call validates it
            value.keyboard_visible = raw.keyboard_visible;
            let mut keyboard = pxui::Keyboard::default();
            (keyboard.frame, keyboard.flags) = (raw.keyboard_current, raw.keyboard_flags);
            if keyboard.flags & pxui::Keyboard::ANIMATING != 0 && raw.keyboard_duration > 0.0 {
                let points = match raw.keyboard_curve {
                    1 => [0.42, 0.0, 1.0, 1.0],
                    2 => [0.0, 0.0, 0.58, 1.0],
                    3 => [0.0, 0.0, 1.0, 1.0],
                    _ => [0.42, 0.0, 0.58, 1.0],
                };
                let curve = pxui::ui::Curve::Bezier { duration: raw.keyboard_duration as f32, points };
                let t = curve.at((raw.timestamp - raw.keyboard_started) as f32);
                let bottom = window.extent().logical.1 as f32 - raw.safe_area[2];
                let mut start = raw.keyboard_from;
                let mut end = raw.keyboard_frame;
                // The layout guide rests on the safe-area edge when the keyboard is offscreen.
                start[1] = start[1].min(bottom);
                end[1] = end[1].min(bottom);
                keyboard.frame = std::array::from_fn(|i| start[i] + (end[i] - start[i]) * t);
            }
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
    /// Current keyboard geometry sampled by pxui; pass to FrameIn::keyboard with the environment.
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
        let options=if ime.is_some() {ui.text_traits()} else {pxui::TextTraits::default()};
        window.set_text_input_with_options(ime,pxui_sdl::TextInputOptions {
            autocorrect:options.autocorrects() as u32,capitalization:options.capitalizes() as u32,action:options.return_key() as u32,
        }).map_err(|_| Error::Host("could not set native text input"))?;
        let frame = (ui.instance_id(), ui.frame_index());
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
