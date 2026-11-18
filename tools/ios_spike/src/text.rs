//! Native UITextInput fixtures; real IME/dictation interaction still needs device qualification.
use input::Event;
use pxui::{TextField as Options, Ui};

pub struct Probe {
    value: String,
    checked: usize,
}
impl Probe {
    pub fn new() -> Self {
        Self {
            value: "café track".into(),
            checked: 0,
        }
    }
    pub fn options(frame: usize) -> Options<'static> {
        if frame >= 32 {
            Options::SEARCH.action(pxui::ReturnKey::Done)
        } else {
            Options::SEARCH
        }
    }
    pub fn inject(&mut self, frame: usize) -> Result<(), String> {
        let step = match frame {
            3 => 9,
            4 => 0,
            8 => 1,
            12 => 2,
            16 => 3,
            20 => 4,
            24 => 5,
            28 => 6,
            34 => 7,
            70 => 8,
            _ => return Ok(()),
        };
        if !sdl3_sys::test_util::ios_text_step(step) {
            return Err(format!("Native text fixture failed at step {step}"));
        }
        self.checked += 1;
        Ok(())
    }
    pub fn check_events(&self, frame: usize, events: &[Event]) -> Result<(), String> {
        let expected = match frame {
            4 => Some(("ni", 2, 0)),
            8 => Some(("你😀", 1, 1)),
            24 => Some(("cancel", 6, 0)),
            28 => Some(("", 0, 0)),
            _ => None,
        };
        if let Some((want, a, b)) = expected {
            if events.iter().filter(|event| matches!(event,Event::TextEditing {text,start,length} if text==want && *start==a && *length==b)).count()!=1 {
                return Err(format!("Native marked-text mismatch at frame {frame}: {events:?}"));
            }
        }
        Ok(())
    }
    pub fn view(&mut self, ui: &mut Ui, frame: usize) {
        if let Some(value) =
            ui.text_field("Mobile input", &self.value, &Self::options(frame)).changed
        {
            self.value.clear();
            self.value.push_str(value);
        }
    }
    pub fn check(&self, ui: &Ui, frame: usize) -> Result<(), String> {
        let expected = match frame {
            0..=11 => "café track",
            12..=15 => "café track你好",
            16..=19 => "café track你好 café 🎵",
            _ => "café track你好 café ",
        };
        if self.value != expected {
            return Err(format!(
                "Native edit mismatch at frame {frame}: {:?}, expected {expected:?}",
                self.value
            ));
        }
        if frame == 34 && (ui.text_input_focused() || ui.ime().is_some()) {
            return Err("Done left the field or IME focused".into());
        }
        Ok(())
    }
    pub fn complete(&self) -> bool {
        self.checked == 10 && self.value == "café track你好 café "
    }
}
