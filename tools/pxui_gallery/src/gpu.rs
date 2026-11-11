//! Gallery GPU setup and the adapter from the app-owned OGPU handles to PXUI's public renderer.
use ogpu_core::backend::{self as be, ColorAttachment, Queue, Receipt, RenderEncoder, RenderTarget, Texture};
use ogpu_core::error::{self, Error};
use ogpu_core::render_pass::{Load, Store};
use ogpu_core::types::{Extent, Origin, TextureDescriptor, TextureFormat, TextureUsage};
use pxui::{Commands as UiCommands, Gpu as UiGpu, Image, Pass as UiPass, Renderer, TargetDesc, Ui};

const TARGET_RGBA8: u32 = 1;

/// A GPU failure ends the app with its message.
pub fn fail(e: Error) -> ! {
    eprintln!("pxui_gallery: gpu: {}", error::to_string(&e));
    std::process::exit(1)
}

/// The device and queue stay app-owned. PXUI adopts a native lease while its renderer lives.
pub struct Gpu {
    pub device: be::Device,
    pub queue: Queue,
    textures: Vec<Texture>,
    submission: u64,
    completed: u64,
}

impl Gpu {
    /// Open the platform backend's device.
    pub fn open() -> Result<Gpu, String> {
        #[cfg(target_os = "macos")]
        ogpu_core::impl_::select(ogpu_metal::impl_::implementation());
        #[cfg(any(windows, target_os = "linux"))]
        ogpu_core::impl_::select(ogpu_core::impl_::Impl {
            backend: ogpu_vulkan::impl_::BACKEND,
            create_driver: ogpu_vulkan::backend::create,
            live_handles: ogpu_vulkan::impl_::live_handles,
        });
        let driver = ogpu_core::impl_::create_driver().map_err(|e| error::to_string(&e))?;
        let device = be::create_device(driver).map_err(|e| error::to_string(&e))?;
        let queue = be::create_queue(&device).map_err(|e| error::to_string(&e))?;
        Ok(Gpu { device, queue, textures: Vec::new(), submission: 0, completed: 0 })
    }

    /// Create a public PXUI renderer that borrows `ui` and adopts this app-owned device.
    pub fn renderer<'ui>(&self, ui: &'ui mut Ui) -> Result<Renderer<'ui>, String> {
        // one lease, handed to the renderer: its device releases it when the renderer goes
        let native = be::native_leased(&self.device).map_err(|e| error::to_string(&e))?;
        let gpu = UiGpu {
            size: native.size, backend: native.backend, instance: native.instance, physical_device: native.physical_device,
            device: native.device, queue: native.queue, queue_family: native.queue_family, api_version: native.api_version,
            features: native.features, get_instance_proc_addr: native.get_instance_proc_addr, pipeline_cache: native.pipeline_cache,
            mtl_device: native.mtl_device, wl_display: native.wl_display, wl_surface: native.wl_surface, metal_layer: native.metal_layer,
            hwnd: native.hwnd, owner_token: native.owner_token, release_owner: native.release_owner,
        };
        Renderer::new(ui, gpu, TargetDesc { format: TARGET_RGBA8, samples: 1 }).ok_or_else(|| "pxui renderer creation failed".to_owned())
    }

    /// An RGBA8 image `w` x `h` as a public host image. `Gpu` keeps its native texture alive.
    pub fn image(&mut self, w: i64, h: i64, rgba: &[u8]) -> Image {
        let texture = be::create_texture(&self.device, TextureDescriptor {
            label: Some("pxui_gallery.image".into()), width: w, height: h, depth: 1, mip_levels: 1, sample_count: 1,
            format: TextureFormat::Rgba8Unorm, usage: vec![TextureUsage::TextureBinding, TextureUsage::TextureCopyDst],
        }).unwrap_or_else(|e| fail(e));
        be::write_texture(&texture, None, Origin { x: 0, y: 0, z: 0 }, Extent { width: w, height: h, depth: 1 }, w * 4, rgba).unwrap_or_else(|e| fail(e));
        let native = be::native_texture(&texture).unwrap_or_else(|e| fail(e));
        self.textures.push(texture);
        Image {
            size: core::mem::size_of::<Image>() as u32, vk_image_view: native.vk_image_view, mtl_texture: native.mtl_texture,
            width: native.width, height: native.height, format: Image::FORMAT_RGBA8,
        }
    }

    /// A render target `w` x `h` pixels that can be read back.
    pub fn target(&self, w: u32, h: u32) -> Texture {
        be::create_texture(&self.device, TextureDescriptor {
            label: Some("pxui_gallery.target".into()), width: w as i64, height: h as i64, depth: 1, mip_levels: 1, sample_count: 1,
            format: TextureFormat::Rgba8Unorm, usage: vec![TextureUsage::RenderAttachment, TextureUsage::TextureBinding, TextureUsage::TextureCopySrc],
        }).unwrap_or_else(|e| fail(e))
    }

    /// Prepare PXUI's retained renderer before opening a host render encoder.
    pub fn prepare(&mut self, renderer: &mut Renderer<'_>, commands: &be::Commands) -> Result<(), String> {
        let native = be::native_commands(commands).map_err(|e| error::to_string(&e))?;
        let commands = UiCommands { size: core::mem::size_of::<UiCommands>() as u32, vk_command_buffer: native.vk_command_buffer, mtl_command_buffer: native.mtl_command_buffer };
        let submission = self.submission.checked_add(1).ok_or_else(|| "PXUI gallery submission counter overflow".to_owned())?;
        self.submission = submission;
        renderer.prepare(&commands, submission, self.completed).map_err(|code| format!("pxui renderer prepare failed ({code})"))?;
        Ok(())
    }

    /// Draw PXUI's retained layers into the already-open host pass. PXUI leaves its own pipeline, bindings
    /// and dynamic state bound, so the app's encoder forgets what it cached before it draws anything else.
    pub fn draw(&self, renderer: &mut Renderer<'_>, commands: &be::Commands, encoder: &RenderEncoder, size_px: [u32; 2], scale: f32) -> Result<(), String> {
        let commands = be::native_commands(commands).map_err(|e| error::to_string(&e))?;
        let native = be::native_pass(encoder).map_err(|e| error::to_string(&e))?;
        let pass = UiPass {
            size: core::mem::size_of::<UiPass>() as u32, vk_command_buffer: native.vk_command_buffer,
            mtl_render_encoder: native.mtl_render_encoder, width_px: size_px[0], height_px: size_px[1], scale,
            mtl_command_buffer: commands.mtl_command_buffer,
        };
        renderer.draw(&pass).map_err(|code| format!("pxui renderer draw failed ({code})"))?;
        be::invalidate_state(encoder).map_err(|e| error::to_string(&e))
    }

    /// Complete the just-submitted queue work; gallery qualification renders synchronously.
    pub fn complete(&mut self, receipt: &Receipt) {
        be::complete_through(&self.queue, receipt.epoch).unwrap_or_else(|e| fail(e));
        self.completed = self.submission;
    }

    /// Render the last frame to an offscreen target and return its RGBA pixels.
    pub fn render(&mut self, renderer: &mut Renderer<'_>, size: (u32, u32), density: u32, ground: [u8; 4]) -> Vec<u8> {
        let (w, h) = (size.0 * density, size.1 * density);
        let target = self.target(w, h);
        let commands = be::begin_commands(&self.queue).unwrap_or_else(|e| fail(e));
        self.prepare(renderer, &commands).unwrap_or_else(|e| { eprintln!("pxui_gallery: {e}"); std::process::exit(1) });
        let clear = ground.map(|c| c as f64 / 255.0);
        let encoder = be::render_encoder(&commands, &RenderTarget {
            colors: vec![ColorAttachment { texture: target.clone(), resolve: None, load: Load::Clear, store: Store::Store, clear: (clear[0], clear[1], clear[2], clear[3]) }],
            depth: None,
        }).unwrap_or_else(|e| fail(e));
        self.draw(renderer, &commands, &encoder, [w, h], density as f32).unwrap_or_else(|e| { eprintln!("pxui_gallery: {e}"); std::process::exit(1) });
        be::end_render(&encoder).unwrap_or_else(|e| fail(e));
        let receipt = be::commit(&commands).unwrap_or_else(|e| fail(e));
        self.complete(&receipt);
        let pixels = be::read_texture(&target, (w * 4) as i64).unwrap_or_else(|e| fail(e));
        be::destroy_texture(&target).unwrap_or_else(|e| fail(e));
        pixels
    }

    /// Release the device after every renderer and surface made from it.
    pub fn close(self) {
        for texture in &self.textures { be::destroy_texture(texture).unwrap_or_else(|e| fail(e)); }
        be::destroy_queue(&self.queue).unwrap_or_else(|e| fail(e));
        be::destroy_device(&self.device).unwrap_or_else(|e| fail(e));
    }
}

/// A small RGBA8 gradient card with a border and a light disc.
pub fn test_card(w: i64, h: i64) -> Vec<u8> {
    let mut pixels = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let (u, v) = (x as f32 / (w - 1) as f32, y as f32 / (h - 1) as f32);
            let (dx, dy) = (x as f32 - w as f32 / 2.0, y as f32 - h as f32 / 2.0);
            let disc = dx * dx + dy * dy < (h as f32 / 4.0).powi(2);
            let edge = x < 2 || y < 2 || x >= w - 2 || y >= h - 2;
            let color = if edge { [30, 34, 48] } else if disc { [250, 240, 220] } else { [(240.0 * u) as u8 + 15, (90.0 + 120.0 * v) as u8, (220.0 * (1.0 - u)) as u8 + 20] };
            pixels.extend_from_slice(&[color[0], color[1], color[2], 255]);
        }
    }
    pixels
}
