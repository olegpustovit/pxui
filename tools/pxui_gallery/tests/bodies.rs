//! `bodies_default_identical`: the gallery pages built through the engine directly draw the same layer bytes as when the baseline
//! was recorded (`fixtures/bodies_baseline.bin`, made on the tree before widget bodies existed), so the default bodies reproduce
//! the widgets exactly. Every page at 1x and at 2x under the dusk theme; and, on the Kit and Controls pages, a pointer sweep
//! (move, press, drag, release at every grid point) whose frames are each kept as a hash. `RAYS_UPDATE_FIXTURES=1` rewrites it.
//! The engine is linked statically here; in the dynamic build the same engine runs behind the library, so the case is a no-op.
#[path = "../../t/run.rs"]
mod run;

#[cfg(feature = "static")]
mod identical {
    use abi_bench::{gallery::{page, Gallery, PAGES, PAGE_H, PAGE_W}, pxui::{text::Text, Size, Theme, Ui}};
    use input::{frame::Frame, Event, MouseButton, Platform};
    use std::path::PathBuf;

    const DEPARTURE_MONO: &[u8] = include_bytes!("../../../assets/fonts/DepartureMono-Regular.otf");
    const DUSK: &str = include_str!("../../../assets/themes/dusk.theme");
    /// The sweep's grid, in points: every cell of the page is pressed once.
    const GRID: (usize, usize) = (24, 7);

    fn fixture() -> PathBuf {
        PathBuf::from(format!("{}/fixtures/bodies_baseline.bin", env!("CARGO_MANIFEST_DIR")))
    }

    struct Rig { ui: Ui, state: Gallery, frame: Frame, count: u64 }

    impl Rig {
        fn new(theme: Option<&Theme>, density: f64) -> Rig {
            let mut input = input::at_time(0.0);
            input.size = (PAGE_W, PAGE_H);
            input.dt = 1.0 / 60.0;
            let frame = Frame { input, pixel_scale: (density, density), fps: 60.0, mouse_delta: (0.0, 0.0) };
            let mut ui = Ui::create(Platform::Apple, None, Text::new(&[DEPARTURE_MONO]));
            if let Some(theme) = theme { ui.set_theme(*theme); }
            Rig { ui, state: Gallery::new(), frame, count: 0 }
        }

        /// One frame of page `index` with `events`; `rebuild` gives the page's view a new revision.
        fn step(&mut self, index: usize, rebuild: bool, events: Vec<Event>) {
            self.count += 1;
            self.frame.input.t = self.count as f64 / 60.0;
            self.frame.input.events = events;
            let revision = if rebuild { self.count } else { 1 };
            let state = &mut self.state;
            self.ui.frame(&self.frame, &mut |ui: &mut Ui| { ui.view("page", revision, Size::Grow, Size::Grow, &mut |ui: &mut Ui| page(ui, state, index)); });
        }

        /// The frame's layers as bytes: key, id, every instance field, every break.
        fn bytes(&self, out: &mut Vec<u8>) {
            for layer in self.ui.output().draw {
                out.extend(layer.key.to_le_bytes());
                out.extend(layer.id.to_le_bytes());
                out.extend((layer.instances.len() as u32).to_le_bytes());
                for i in &layer.instances {
                    for v in i.rect { out.extend(v.to_le_bytes()); }
                    for v in i.clip { out.extend(v.to_le_bytes()); }
                    out.extend(i.color);
                    out.extend(i.color2);
                    for v in i.aux { out.extend(v.to_le_bytes()); }
                    for v in i.radii { out.extend(v.to_le_bytes()); }
                    out.extend(i.width.to_le_bytes());
                    out.extend([i.kind, i.flags]);
                    out.extend(i.view.to_le_bytes());
                    out.extend(i.clip_radius.to_le_bytes());
                    for v in i.extra { out.extend(v.to_le_bytes()); }
                }
                out.extend((layer.breaks.len() as u32).to_le_bytes());
                for b in &layer.breaks {
                    out.extend(b.at_instance.to_le_bytes());
                    out.extend(b.image.to_le_bytes());
                }
            }
        }
    }

    fn fnv(bytes: &[u8]) -> u64 {
        bytes.iter().fold(0xcbf29ce484222325, |h, b| (h ^ *b as u64).wrapping_mul(0x100000001b3))
    }

    /// The whole recording, and where each scenario starts in it.
    fn record() -> (Vec<u8>, Vec<(String, usize)>) {
        let dusk = Theme::parse(DUSK, &Theme::default()).expect("dusk.theme");
        let (mut out, mut marks) = (Vec::new(), Vec::new());
        for (index, name) in PAGES.iter().enumerate() {
            for (look, theme, density) in [("rays 1x", None, 1.0), ("dusk 2x", Some(&dusk), 2.0)] {
                marks.push((format!("{name} {look}"), out.len()));
                let mut rig = Rig::new(theme, density);
                let away = Event::PointerMoved((-100.0, -100.0));
                for _ in 0..3 { rig.step(index, false, vec![away.clone()]); }
                rig.bytes(&mut out);
            }
        }
        for index in [0, 1] {
            marks.push((format!("{} sweep", PAGES[index]), out.len()));
            let mut rig = Rig::new(None, 1.0);
            for _ in 0..3 { rig.step(index, false, vec![Event::PointerMoved((-100.0, -100.0))]); }
            let mut frame = Vec::new();
            for gy in 0..(PAGE_H as usize / GRID.1) {
                for gx in 0..(PAGE_W as usize / GRID.0) {
                    let p = ((gx * GRID.0 + 5) as f64, (gy * GRID.1 + 5) as f64);
                    let steps = [
                        vec![Event::PointerMoved(p)],
                        vec![Event::PointerPressed(MouseButton::LeftButton, p)],
                        vec![Event::PointerMoved((p.0 + 17.0, p.1))],
                        vec![Event::PointerReleased(MouseButton::LeftButton, (p.0 + 17.0, p.1))],
                    ];
                    for events in steps {
                        rig.step(index, true, events);
                        frame.clear();
                        rig.bytes(&mut frame);
                        out.extend(fnv(&frame).to_le_bytes());
                    }
                }
            }
        }
        (out, marks)
    }

    pub fn bodies_default_identical() {
        let (now, marks) = record();
        if std::env::var("RAYS_UPDATE_FIXTURES").as_deref() == Ok("1") {
            std::fs::write(fixture(), &now).expect("refresh the bodies baseline");
        }
        let baseline = std::fs::read(fixture()).expect("fixtures/bodies_baseline.bin");
        if baseline != now {
            let at = baseline.iter().zip(&now).position(|(a, b)| a != b).unwrap_or(baseline.len().min(now.len()));
            let scenario = marks.iter().rev().find(|(_, start)| *start <= at).map_or("?", |(name, _)| name.as_str());
            panic!("the default bodies drew differently from the baseline in `{scenario}`: first difference at byte {at} (baseline {} bytes, now {})", baseline.len(), now.len());
        }
    }
}

#[cfg(feature = "static")]
const CASES: &[(&str, fn())] = &[("bodies_default_identical", identical::bodies_default_identical)];
#[cfg(not(feature = "static"))]
const CASES: &[(&str, fn())] = &[];

fn main() {
    run::run(CASES);
}
