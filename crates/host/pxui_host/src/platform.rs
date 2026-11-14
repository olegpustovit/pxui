use crate::Window;

/// One UI/window pair's platform output and pending clipboard input.
#[derive(Default)]
pub struct Platform {
    applied: Option<(usize, u64)>,
    pending: Vec<input::Event>,
}
impl Platform {
    /// Appends pending paste input before the window's next batch.
    pub fn pump(&mut self, window: &mut Window, events: &mut Vec<input::Event>) -> bool {
        events.append(&mut self.pending);
        window.pump(events)
    }
    /// Apply once after each UI frame, including frames whose draw revision did not change.
    pub fn apply(&mut self, window: &mut Window, ui: &pxui::Ui) -> Result<(), String> {
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
        window.set_text_input(ime)?;
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
            window.set_cursor(cursor)?;
        }
        if out.has_background != 0 {
            let color = out.background;
            window.set_background((
                f64::from(color.r) / 255.0,
                f64::from(color.g) / 255.0,
                f64::from(color.b) / 255.0,
            ))?;
        }
        if out.has_clipboard != 0
            && let Some(text) = ui.clipboard_request()
        {
            pxui_sdl::clipboard_set_text(text)?;
        }
        if out.paste != 0 {
            self.pending
                .push(input::Event::Paste(pxui_sdl::clipboard_get_text()?));
            pxui_sdl::wake();
        }
        self.applied = Some(frame);
        Ok(())
    }
}
