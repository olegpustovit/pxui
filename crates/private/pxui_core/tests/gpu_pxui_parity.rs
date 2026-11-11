//! Pixel goldens rendered headless into an offscreen texture: no SDL, no window, any display.
#[path = "cases/gpu.rs"]
mod gpu;
#[path = "cases/gpu_parity.rs"]
mod gpu_parity;
#[path = "../../../../tools/t/png.rs"]
mod png;
#[path = "../../../../tools/t/run.rs"]
mod run;
#[path = "cases/support.rs"]
#[allow(dead_code)]
mod support;
#[path = "cases/widgets.rs"]
#[allow(dead_code)]
mod widgets;

fn main() {
    run::run(gpu_parity::CASES);
    gpu::finish();
}
