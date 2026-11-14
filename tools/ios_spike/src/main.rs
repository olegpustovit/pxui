#[cfg(target_os = "ios")]
extern "C" fn probe() -> i32 {
    let result = || -> Result<(), String> {
        let mut window = pxui_sdl::Window::open("PXUI iOS host spike", 390, 778)?;
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
        let token = window.token()?;
        if !token.alive() {
            return Err("Metal layer token is not live".into());
        }
        let mut ui = pxui::Ui::create(
            pxui::Platform::Apple,
            None,
            &[include_bytes!(
                "../../../assets/fonts/Pragmasevka-Regular.ttf"
            )],
        );
        let mut gallery = pxui_gallery::Gallery::new();
        let mut graphics = pxui_host::Graphics::open(&mut window, &ui)?;
        let mut events = Vec::with_capacity(64);
        for frame in 0..182 {
            mem_sys::local_reset();
            events.clear();
            if !window.pump(&mut events) {
                return Err("iOS gallery window closed during probe".into());
            }
            if let Some(error) = window.take_failure() {
                return Err(error);
            }
            ui.frame(
                &pxui::FrameIn {
                    width: extent.logical.0 as f32,
                    height: extent.logical.1 as f32,
                    scale: scale as f32,
                    time: frame as f64 / 60.0,
                    ..pxui::FrameIn::default()
                },
                &mut |ui| pxui_gallery::app(ui, &mut gallery),
            )
            .map_err(|code| format!("iOS gallery frame failed ({code})"))?;
            if !graphics.draw(&mut window, &ui, None)? {
                return Err(format!("No drawable for frame {frame}"));
            }
            let allocations = mem_sys::local_alloc_count();
            if frame >= 82 && allocations != 0 {
                return Err(format!(
                    "Warm gallery frame {frame} allocated {allocations} Rust objects"
                ));
            }
            if frame == 0 {
                println!("PXUI_IOS_GALLERY_FIRST_FRAME");
            }
        }
        graphics.close()?;
        let footprint = mem_sys::phys_footprint().ok_or("iOS physical footprint unavailable")?;
        let peak = mem_sys::peak_footprint().ok_or("iOS peak footprint unavailable")?;
        if footprint <= 0 || peak < footprint {
            return Err(format!(
                "Invalid iOS memory metrics: footprint={footprint}, peak={peak}"
            ));
        }
        window.destroy()?;
        if token.alive() {
            return Err("Destroyed window left its Metal layer token live".into());
        }
        println!(
            "PXUI_IOS_SPIKE_OK logical={:?} drawable={:?} scale={scale} frames=182 measured=100 warm_rust_allocs=0 footprint={footprint} peak={peak}",
            extent.logical, extent.drawable
        );
        Ok(())
    }();
    match result {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("PXUI_IOS_SPIKE_FAILED {error}");
            1
        }
    }
}

#[cfg(target_os = "ios")]
fn main() {
    std::process::exit(pxui_sdl::run_app(probe));
}

#[cfg(not(target_os = "ios"))]
fn main() {
    eprintln!("Build with packaging/ios-spike.sh for an iOS device or simulator.");
}
