#[cfg(target_os = "ios")]
mod ios {
    use pxui_sdl::ios::{Schedule, Signal};

    struct Probe {
        ui: pxui::Ui,
        gallery: pxui_gallery::Gallery,
        graphics: pxui_host::Graphics,
        platform: pxui_host::Platform,
        encoded: pxui_host::Events,
        events: Vec<input::Event>,
        frames: usize,
        lifecycle: bool,
        paused: bool,
        resumed: bool,
        holding: bool,
        idle_until: Option<f64>,
        idle_callbacks: usize,
        worker: Option<std::thread::JoinHandle<bool>>,
        worker_started: f64,
        worker_woke: bool,
    }
    impl Probe {
        fn open(window: &mut pxui_sdl::Window) -> Result<Self, String> {
            let extent = window.extent();
            let scale = window.scale();
            if extent.logical.0 <= 0
                || extent.logical.1 <= 0
                || extent.drawable.0 < extent.logical.0
                || extent.drawable.1 < extent.logical.1
                || !scale.is_finite()
                || scale < 1.0
            {
                return Err(format!("Invalid iOS extent: {extent:?}, scale={scale}"));
            }
            if !window.token()?.alive() {
                return Err("Metal layer token is not live".into());
            }
            let ui = pxui::Ui::create(
                pxui::Platform::Apple,
                None,
                &[include_bytes!(
                    "../../../assets/fonts/Pragmasevka-Regular.ttf"
                )],
            );
            let graphics = pxui_host::Graphics::open(window, &ui)?;
            Ok(Self {
                ui,
                graphics,
                gallery: pxui_gallery::Gallery::new(),
                platform: pxui_host::Platform::default(),
                encoded: pxui_host::Events::default(),
                events: Vec::with_capacity(64),
                frames: 0,
                lifecycle: std::env::var_os("PXUI_IOS_LIFECYCLE_CHECK").is_some(),
                paused: false,
                resumed: false,
                holding: false,
                idle_until: None,
                idle_callbacks: 0,
                worker: None,
                worker_started: 0.0,
                worker_woke: false,
            })
        }
        fn signal(
            &mut self,
            window: &mut pxui_sdl::Window,
            signal: Signal,
        ) -> Result<Schedule, String> {
            match signal {
                Signal::Paused => {
                    let frames = self.frames;
                    mem_sys::local_reset();
                    if self.graphics.can_draw()
                        || self.graphics.draw(window, &self.ui, None)?
                        || self.frames != frames
                        || mem_sys::local_alloc_count() != 0
                    {
                        return Err("Inactive host submitted or allocated a frame".into());
                    }
                    self.paused = true;
                    println!("PXUI_IOS_PAUSED frames={frames} warm_rust_allocs=0");
                    return Ok(Schedule::WaitUntil(None));
                }
                Signal::Resumed => {
                    if self.paused {
                        self.resumed = true;
                    }
                    self.ui.request_frame();
                    println!("PXUI_IOS_RESUMED frames={}", self.frames);
                    return Ok(Schedule::NextFrame);
                }
                Signal::LowMemory => return Ok(Schedule::NextFrame),
                Signal::Frame => {}
                _ => return Ok(Schedule::NextFrame),
            }
            if self.lifecycle && self.frames == 90 && !self.resumed {
                if !self.holding {
                    self.holding = true;
                    println!("PXUI_IOS_LIFECYCLE_READY frames=90");
                }
                return Ok(Schedule::WaitUntil(None));
            }
            if self.frames == 182 {
                if let Some(worker) = self.worker.take() {
                    if window.time() - self.worker_started < 0.1 {
                        return Err("Indefinitely idle loop polled before the worker wake".into());
                    }
                    if !worker.join().map_err(|_| "Wake worker panicked")? {
                        return Err("Idle worker could not wake SDL".into());
                    }
                    self.worker_woke = true;
                    println!("PXUI_IOS_WORKER_WAKE_OK frames=182");
                    return Ok(Schedule::Exit);
                }
                if let Some(at) = self.idle_until {
                    self.idle_callbacks += 1;
                    if window.time() >= at {
                        if self.idle_callbacks > 3 {
                            return Err(format!(
                                "Idle loop kept polling: {} callbacks",
                                self.idle_callbacks
                            ));
                        }
                        println!(
                            "PXUI_IOS_IDLE_OK callbacks={} frames=182",
                            self.idle_callbacks
                        );
                        self.worker_started = window.time();
                        // Manual runtime probe: leave the native display link fully idle first.
                        self.worker = Some(std::thread::spawn(|| {
                            std::thread::sleep(std::time::Duration::from_millis(150));
                            pxui_sdl::wake()
                        }));
                        return Ok(Schedule::WaitUntil(None));
                    }
                    return Ok(Schedule::WaitUntil(Some(at)));
                }
                self.idle_until = Some(window.time() + 0.25);
                return Ok(Schedule::WaitUntil(self.idle_until));
            }
            mem_sys::local_reset();
            self.events.clear();
            if !self.platform.pump(window, &mut self.events) {
                return Err("iOS gallery window closed during probe".into());
            }
            if let Some(error) = window.take_failure() {
                return Err(error);
            }
            let extent = window.extent();
            let pumped = mem_sys::local_alloc_count();
            let wire = self.encoded.encode(&self.events);
            let encoded = mem_sys::local_alloc_count();
            self.ui
                .frame(
                    &pxui::FrameIn {
                        width: extent.logical.0 as f32,
                        height: extent.logical.1 as f32,
                        scale: window.scale() as f32,
                        time: window.time(),
                        events: wire.as_ptr(),
                        event_count: wire.len(),
                        ..pxui::FrameIn::default()
                    },
                    &mut |ui| pxui_gallery::app(ui, &mut self.gallery),
                )
                .map_err(|code| format!("iOS gallery frame failed ({code})"))?;
            let built = mem_sys::local_alloc_count();
            self.platform.apply(window, &self.ui)?;
            let output = mem_sys::local_alloc_count();
            if !self.graphics.draw(window, &self.ui, None)? {
                return Ok(Schedule::NextFrame);
            }
            let allocations = mem_sys::local_alloc_count();
            if self.frames >= 82 && allocations != 0 {
                return Err(format!(
                    "Warm gallery frame {} allocated {allocations} Rust objects (pump={pumped}, encode={encoded}, build={built}, output={output}, input={:?})",
                    self.frames, self.events
                ));
            }
            if self.frames == 0 {
                println!("PXUI_IOS_GALLERY_FIRST_FRAME");
            }
            self.frames += 1;
            Ok(Schedule::NextFrame)
        }
        fn finish(self, window: &mut pxui_sdl::Window) -> Result<(), String> {
            let complete = self.frames == 182
                && self.idle_callbacks > 0
                && self.worker_woke
                && (!self.lifecycle || (self.paused && self.resumed && self.holding));
            self.graphics.close()?;
            if !complete {
                return Err("iOS probe did not complete its frame/lifecycle/idle checks".into());
            }
            let footprint =
                mem_sys::phys_footprint().ok_or("iOS physical footprint unavailable")?;
            let peak = mem_sys::peak_footprint().ok_or("iOS peak footprint unavailable")?;
            if footprint <= 0 || peak < footprint {
                return Err(format!(
                    "Invalid iOS memory metrics: footprint={footprint}, peak={peak}"
                ));
            }
            let extent = window.extent();
            let scale = window.scale();
            let token = window.token()?;
            window.destroy()?;
            if token.alive() {
                return Err("Destroyed window left its Metal layer token live".into());
            }
            println!(
                "PXUI_IOS_SPIKE_OK logical={:?} drawable={:?} scale={scale} frames=182 measured=100 warm_rust_allocs=0 lifecycle={} footprint={footprint} peak={peak}",
                extent.logical, extent.drawable, self.lifecycle
            );
            Ok(())
        }
    }
    pub fn run() -> Result<(), String> {
        let mut probe = None;
        pxui_sdl::ios::run(
            "PXUI iOS host spike",
            390,
            778,
            Box::new(move |window, signal| {
                if signal == Signal::Initialize {
                    probe = Some(Probe::open(window)?);
                    return Ok(Schedule::NextFrame);
                }
                if signal == Signal::Finish {
                    if let Some(probe) = probe.take() {
                        probe.finish(window)?;
                    }
                    return Ok(Schedule::Exit);
                }
                probe
                    .as_mut()
                    .ok_or("iOS probe was not initialized")?
                    .signal(window, signal)
            }),
        )
    }
}

#[cfg(target_os = "ios")]
fn main() {
    if let Err(error) = ios::run() {
        eprintln!("PXUI_IOS_SPIKE_FAILED {error}");
        std::process::exit(1);
    }
}

#[cfg(not(target_os = "ios"))]
fn main() {
    eprintln!("Build with packaging/ios-spike.sh for an iOS device or simulator.");
}
