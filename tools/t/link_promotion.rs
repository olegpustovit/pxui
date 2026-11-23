//! Shared macOS and iOS regression for the display-link drawable handoff.
use super::Graphics;
use pxui::{Fill, FrameIn, Node, Rgba, Ui};
use pxui_sdl::{app::{Schedule, Signal}, Window};

pub fn run() -> Result<(), String> {
    let expect_link = std::env::var_os("PXUI_TIMER_PACING").as_deref() != Some(std::ffi::OsStr::new("1"));
    let mut state = None;
    let mut events = Vec::new();
    pxui_sdl::app::run_with_density("PXUI display-link promotion", 160, 100, false, Box::new(move |window, signal| {
        if signal == Signal::Initialize {
            let mut ui = Ui::new(&pxui::UiDesc::DEFAULT).map_err(|e| e.to_string())?;
            let mut graphics = Graphics::open(window, &mut ui).map_err(|e| e.to_string())?;
            graphics.set_no_layers(&mut ui, false);
            state = Some((ui, graphics, false, window.time()));
            return Ok(Schedule::NextFrame);
        }
        if signal == Signal::Finish {
            if let Some((_, graphics, _, _)) = state.take() {
                graphics.close().map_err(|e| e.to_string())?;
            }
            return Ok(Schedule::Exit);
        }
        if signal != Signal::Frame { return Ok(Schedule::NextFrame); }
        events.clear();
        if !window.pump(&mut events) { return Err("Promotion probe window closed".into()); }
        let (ui, graphics, deferred, started) = state.as_mut().ok_or("Promotion probe was not initialized")?;
        let (owned, loan, _) = graphics.test_link_state();
        if expect_link && !loan {
            if window.time() - *started > 2.0 { return Err("CAMetalDisplayLink did not lend a drawable".into()); }
            return Ok(Schedule::NextFrame);
        }
        assert_eq!(owned, expect_link);
        if *deferred {
            assert!(graphics.draw(window, ui, None).map_err(|e| e.to_string())?);
            assert_eq!(graphics.test_link_state(), (true, false, Some(24.0)));
            assert!(!graphics.needs_present());
            println!("PXUI_DISPLAY_LINK_PROMOTION_OK linked=true");
            return Ok(Schedule::Exit);
        }
        build(window, ui, 8)?;
        assert!(graphics.draw(window, ui, None).map_err(|e| e.to_string())?);
        assert_eq!(graphics.test_link_state(), (owned, false, Some(8.0)));
        build(window, ui, 24)?;
        let presented = graphics.draw(window, ui, None).map_err(|e| e.to_string())?;
        assert_eq!(presented, !owned);
        if !owned {
            assert_eq!(graphics.test_link_state(), (false, false, Some(24.0)));
            println!("PXUI_DISPLAY_LINK_PROMOTION_OK linked=false");
            return Ok(Schedule::Exit);
        }
        assert!(graphics.needs_present());
        assert_eq!(graphics.test_link_state(), (true, false, Some(8.0)));
        let (width, _, pixels) = graphics.screenshot(window, ui).map_err(|e| e.to_string())?;
        let scale = window.scale() as usize;
        let at = (9 * scale * width as usize + 25 * scale) * 4;
        assert_eq!(&pixels[at..at + 4], &[255; 4], "linked capture includes the moved bar");
        assert_eq!(graphics.test_link_state(), (true, false, Some(8.0)));
        *deferred = true;
        Ok(Schedule::NextFrame)
    }))
}

fn build(window: &Window, ui: &mut Ui, x: u32) -> Result<(), String> {
    let (w, h) = window.extent().logical;
    ui.frame(&FrameIn::new(w as f32, h as f32).scale(window.scale() as f32).time(window.time()), &mut |ui| {
        ui.column(Node::column().w_px(4.0).h_px(8.0).at(x as f32, 8.0), |ui| {
            let view = ui.view_of("linked bar", &0, |ui| {
                let rect = ui.node("fill", Node::column().w_px(4.0).h_px(8.0));
                ui.canvas(&rect, |paint, rect| {
                    paint.fill(rect, Fill::new(Rgba::rgb(255, 255, 255)).fractional());
                });
            });
            ui.promote(&view).unwrap();
        });
    }).map_err(|e| e.to_string())?;
    assert_eq!(ui.promoted().len(), 1);
    Ok(())
}
