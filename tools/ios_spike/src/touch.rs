//! SDL-to-widget-to-Metal diagnostic, with synthetic fingers and the original frame budget.
use pxui::{
    Ui,
    ui::{CLICKABLE, Node, Role, SectionList, SectionPart, Size},
};

pub struct Probe {
    body: Option<pxui::Handle>,
    clicked: bool,
    taps: usize,
    rows: usize,
    caught: f32,
}
impl Probe {
    pub fn new() -> Self {
        Self {
            body: None,
            clicked: false,
            taps: 0,
            rows: 0,
            caught: 0.0,
        }
    }
    pub fn push(&self, frame: usize) -> Result<(), String> {
        if !sdl3_sys::test_util::touch_mouse_emulation_disabled() {
            return Err("SDL emulates a duplicate mouse/touch stream".into());
        }
        if frame < 2 {
            return Ok(());
        }
        let sample = match (frame - 2) % 10 {
            0 | 2 | 6 | 8 => Some((0, 400.0)),
            1 | 7 => Some((2, 400.0)),
            3 => Some((1, 360.0)),
            4 => Some((2, 360.0)),
            9 => Some((3, 400.0)),
            _ => None,
        };
        if let Some((phase, y)) = sample
            && !sdl3_sys::test_util::push_finger(phase, 50.0, y)
        {
            return Err("Could not queue the native SDL finger fixture".into());
        }
        Ok(())
    }
    pub fn view(&mut self, ui: &mut Ui) {
        self.rows = 0;
        self.clicked = false;
        self.body = Some(ui.section_list(
            "native touch songs",
            &SectionList {
                count: 10_000,
                row_height: 44.0,
                leading_height: 0.0,
                header_height: 0.0,
                trailing_height: 0.0,
                overlay_scrollbar: true,
            },
            &mut |ui, part| {
                if let SectionPart::Row(_) = part {
                    self.rows += 1;
                    let row = ui.node(
                        "song",
                        Node {
                            flags: CLICKABLE,
                            role: Role::Button,
                            w: Size::Grow,
                            h: Size::Grow,
                            ..Node::DEFAULT
                        },
                    );
                    ui.a11y_name(&row, "Native touch song");
                    ui.text(&row, "Native touch song");
                    self.clicked |= ui.signal_state(&row).clicked;
                }
            },
        ));
    }
    pub fn check(&mut self, ui: &Ui, frame: usize) -> Result<(), String> {
        if self.rows > 30 {
            return Err("Native touch built offscreen songs".into());
        }
        if frame < 2 {
            return Ok(());
        }
        let phase = (frame - 2) % 10;
        if self.clicked != (phase == 1) {
            return Err(format!(
                "Native finger phase {phase} delivered clicked={}",
                self.clicked
            ));
        }
        if self.clicked {
            self.taps += 1;
        }
        let position = ui.scroll_offset(&self.body.ok_or("Native touch list was not built")?);
        if phase == 6 {
            self.caught = position;
        }
        if phase == 7 && position != self.caught {
            return Err("Finger catch did not stop the coast".into());
        }
        if frame == 181 {
            if self.taps != 18 || position <= 700.0 {
                return Err(format!(
                    "Native touch trace incomplete: taps={} offset={position}",
                    self.taps
                ));
            }
            println!(
                "PXUI_IOS_TOUCH_OK taps={} scrolls=18 catches=18 cancels=18 songs=10000 offset={position}",
                self.taps
            );
        }
        Ok(())
    }
}
