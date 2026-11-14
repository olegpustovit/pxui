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
        window.destroy()?;
        if token.alive() {
            return Err("Destroyed window left its Metal layer token live".into());
        }
        println!(
            "PXUI_IOS_SPIKE_OK logical={:?} drawable={:?} scale={scale}",
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
