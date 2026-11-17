use metal_sys::frames::Frames;
use ogpu_core::backend as be;
use pxui::{Commands, Gpu, Pass, Renderer, TargetDesc, Ui};
use pxui_sdl::Window;

fn message(error: ogpu_core::error::Error) -> String {
    ogpu_core::error::to_string(&error)
}

/// One surface's GPU state: its own device, queue, frame pacing (`Frames` on the window's layer),
/// renderer and bounded image cache (`images`: 1,024 entries, 128 MiB). Owned per surface, so N windows
/// hold N devices; sharing one device across surfaces is a later step. Main thread only.
///
/// The GPU backend choice (`ogpu_core::impl_::select`) is per thread by design, held by the main
/// thread: every `open` selects Metal, so all surfaces drive the same backend.
pub struct Graphics {
    images: crate::images::ImageCache,
    renderer: Option<Renderer>,
    frames: Option<Frames>,
    queue: be::Queue,
    device: be::Device,
    blur_extent: (u32, u32),
    acquire_wall: std::time::Duration,
}

impl Graphics {
    /// iOS draws only while active; macOS keeps its existing visibility policy in the window host.
    pub fn can_draw(&self) -> bool {
        self.frames.as_ref().is_some_and(Frames::can_draw)
    }
    pub fn open(window: &mut Window, ui: &Ui) -> Result<Self, String> {
        ogpu_core::impl_::select(ogpu_metal::impl_::implementation());
        let driver = ogpu_core::impl_::create_driver().map_err(message)?;
        let device = be::create_device(driver).map_err(message)?;
        let queue = match be::create_queue(&device) {
            Ok(queue) => queue,
            Err(error) => {
                let _ = be::destroy_device(&device);
                return Err(message(error));
            }
        };
        let mut host = Self {
            images: crate::images::ImageCache::default(),
            renderer: None,
            frames: None,
            queue,
            device,
            blur_extent: (0, 0),
            acquire_wall: std::time::Duration::ZERO,
        };
        host.frames = Some(Frames::new(&host.device, window.token()?)?);
        let native = be::native_leased(&host.device).map_err(message)?;
        let renderer = Renderer::new(
            ui,
            Gpu {
                size: native.size,
                backend: native.backend,
                instance: native.instance,
                physical_device: native.physical_device,
                device: native.device,
                queue: native.queue,
                queue_family: native.queue_family,
                api_version: native.api_version,
                features: native.features,
                get_instance_proc_addr: native.get_instance_proc_addr,
                pipeline_cache: native.pipeline_cache,
                mtl_device: native.mtl_device,
                wl_display: native.wl_display,
                wl_surface: native.wl_surface,
                metal_layer: native.metal_layer,
                hwnd: native.hwnd,
                owner_token: native.owner_token,
                release_owner: native.release_owner,
            },
            TargetDesc {
                format: 2,
                samples: 1,
            },
        )?;
        host.renderer = Some(renderer);
        Ok(host)
    }

    /// Borrow image descriptors and coalesced decode requests; GPU ownership stays in the host.
    pub fn images(&self) -> &pxui::images::Images {
        &self.images.descriptors
    }
    pub fn resident_image_bytes(&self) -> usize {
        self.images.resident_bytes()
    }
    #[cfg(test)]
    pub(crate) fn image_texture(&self, id: usize) -> Option<&be::Texture> {
        self.images.texture(id)
    }

    /// Errors: `Error::Gpu` or `Error::State` when an image could not be retired (see `upload_image`).
    pub fn configure_images(&mut self, density: f32, max_pixels: u32) -> Result<(), pxui::Error> {
        self.images.configure(density, max_pixels)
    }

    pub fn draw(
        &mut self,
        window: &mut Window,
        ui: &Ui,
        trace: Option<&mut [u64; 4]>,
    ) -> Result<bool, String> {
        let count = || {
            if trace.is_some() {
                mem_sys::local_alloc_count()
            } else {
                0
            }
        };
        let before = count();
        #[cfg(target_os = "ios")]
        if !self.can_draw() { return Ok(false); }
        let extent = window.extent();
        let size = (extent.drawable.0 as u32, extent.drawable.1 as u32);
        if size.0 == 0 || size.1 == 0 {
            return Ok(false);
        }
        if self.blur_extent != size {
            self.renderer
                .as_mut()
                .expect("Live graphics renderer")
                .resize(size.0, size.1)
                .map_err(|code| format!("Renderer resize failed ({code})"))?;
            self.blur_extent = size;
        }
        let frames = self.frames.as_mut().expect("Live frame context");
        let acquiring = std::time::Instant::now();
        let began = frames.begin(extent.drawable.0 as u32, extent.drawable.1 as u32)?;
        self.acquire_wall = acquiring.elapsed();
        if !began {
            return Ok(false);
        }
        let frames = self.frames.as_mut().expect("Live frame context");
        let acquired = count();
        let renderer = self.renderer.as_mut().expect("Live graphics renderer");
        let commands = frames.commands();
        renderer
            .prepare(
                &Commands {
                    size: core::mem::size_of::<Commands>() as u32,
                    mtl_command_buffer: commands,
                    ..Commands::default()
                },
                frames.serial(),
                frames.completed(),
            )
            .map_err(|code| format!("Renderer prepare failed ({code})"))?;
        let prepared = count();
        let color = ui.theme().tokens.panel;
        let encoder = frames.pass([color.r, color.g, color.b])?;
        renderer
            .draw(&Pass {
                size: core::mem::size_of::<Pass>() as u32,
                mtl_command_buffer: commands,
                mtl_render_encoder: encoder,
                width_px: extent.drawable.0 as u32,
                height_px: extent.drawable.1 as u32,
                scale: window.scale() as f32,
                ..Pass::default()
            })
            .map_err(|code| format!("Renderer draw failed ({code})"))?;
        let drawn = count();
        if !frames.submit()? { return Ok(false); }
        self.images.submitted(frames.serial());
        let completed = frames.completed();
        self.retire_images(completed).map_err(|error| format!("Image retirement failed ({error})"))?;
        let submitted = count();
        if let Some(trace) = trace {
            *trace = [
                acquired - before,
                prepared - acquired,
                drawn - prepared,
                submitted - drawn,
            ];
        }
        Ok(true)
    }

    /// Wall time the last `draw` spent acquiring its drawable and frame slot: display pacing,
    /// not frame work, so callers can subtract it from a whole-frame wall span.
    pub fn acquire_wall(&self) -> std::time::Duration {
        self.acquire_wall
    }

    pub fn close(mut self) -> Result<(), String> {
        self.frames.as_mut().expect("Live frame context").wait()
    }
    fn retire_images(&mut self, completed: u64) -> Result<(), pxui::Error> {
        self.images.retire_completed(
            completed,
            self.renderer.as_mut().expect("Live graphics renderer"),
        )
    }
    pub fn sync_images(&mut self, namespace: usize) -> Result<bool, pxui::Error> {
        let completed = self
            .frames
            .as_ref()
            .expect("Live frame context")
            .completed();
        self.retire_images(completed)?;
        self.images.sync_namespace(namespace)
    }
    /// Deliver decoded RGBA pixels (or a cached miss), with a globally increasing delivery revision.
    /// An obsolete epoch or replayed delivery is ignored; no worker record or pixels are retained.
    /// Errors: `Error::InvalidInput` for pixels that are not `width * height * 4` bytes or a side of 0 or past 1024; `Error::Gpu` when
    /// the texture cannot be made; `Error::State` when retirement passes its three-frame bound (or, on iOS, without an active host).
    pub fn upload_image(
        &mut self,
        key: pxui::images::Want,
        epoch: u64,
        revision: u64,
        pixels: Option<(u32, u32, &[u8])>,
    ) -> Result<bool, pxui::Error> {
        #[cfg(target_os = "ios")]
        if !self.can_draw() { return Err(pxui::Error::State); }
        self.images
            .upload(&self.device, key, epoch, revision, pixels)
    }
}

impl Drop for Graphics {
    fn drop(&mut self) {
        // Renderer resources must outlive all submitted GPU work, including early error exits.
        if let Some(frames) = &mut self.frames {
            let _ = frames.wait();
        }
        drop(self.renderer.take());
        let _ = self.images.destroy();
        let _ = be::destroy_queue(&self.queue);
        let _ = be::destroy_device(&self.device);
    }
}
