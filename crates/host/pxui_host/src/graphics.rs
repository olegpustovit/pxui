use metal_sys::frames::Frames;
use ogpu_core::backend as be;
use pxui::{Commands, DamageList, Error, Gpu, Pass, PxuiTargetDesc, Rect, Renderer, Ui};
use pxui_sdl::Window;

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
    /// Damage of frames that could not start (no drawable, an inactive app, an empty surface), in window points: the next
    /// draw adds it. Forced frames without a damage report redraw the whole viewport.
    held: Option<DamageList>,
    full_damage: bool,
    no_cull: bool,
    #[cfg(target_os = "macos")]
    presentation: Option<u64>,
    present_debt: bool,
}

impl Graphics {
    /// iOS draws only while active; macOS keeps its existing visibility policy in the window host.
    pub fn can_draw(&self) -> bool {
        self.frames.as_ref().is_some_and(Frames::can_draw)
    }
    /// Creates a device, queue, frame context and renderer for this surface on the main thread.
    ///
    /// A native startup failure returns `Error::Host` with the operation that failed.
    /// Renderer creation can return `Error::Gpu`.
    pub fn open(window: &mut Window, ui: &Ui) -> Result<Self, Error> {
        ogpu_core::impl_::select(ogpu_metal::impl_::implementation());
        let driver = ogpu_core::impl_::create_driver().map_err(|_| Error::Host("could not create graphics driver"))?;
        let device = be::create_device(driver).map_err(|_| Error::Host("could not create graphics device"))?;
        let queue = match be::create_queue(&device) {
            Ok(queue) => queue,
            Err(_) => {
                let _ = be::destroy_device(&device);
                return Err(Error::Host("could not create graphics queue"));
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
            held: None,
            full_damage: std::env::var_os("PXUI_FULL_DAMAGE").as_deref() == Some(std::ffi::OsStr::new("1")),
            no_cull: std::env::var_os("PXUI_NO_CULL").as_deref() == Some(std::ffi::OsStr::new("1")),
            present_debt: false,
            #[cfg(target_os = "macos")]
            presentation: None,
        };
        let layer = window.token().map_err(|_| Error::Host("could not get surface token"))?;
        host.frames = Some(Frames::new(&host.device, layer).map_err(|_| Error::Host("could not open native frame context"))?);
        let native = be::native_leased(&host.device).map_err(|_| Error::Host("could not lease graphics device"))?;
        let renderer = Renderer::new_with_target_record(
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
            PxuiTargetDesc {
                format: 2,
                reserved_tail: PxuiTargetDesc::BACKDROP_UI_DAMAGE,
                ..PxuiTargetDesc::default()
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

    #[cfg(test)]
    pub(crate) fn draw_calls(&self) -> u64 {
        self.renderer.as_ref().expect("Live graphics renderer").draw_calls()
    }

    /// Errors: `Error::Gpu` or `Error::State` when an image could not be retired (see `upload_image`).
    pub fn configure_images(&mut self, density: f32, max_pixels: u32) -> Result<(), pxui::Error> {
        self.images.configure(density, max_pixels)
    }

    /// Enables full-viewport redraws, or restores damage-limited rendering.
    pub fn set_full_damage(&mut self, full: bool) {
        self.full_damage = full;
    }

    /// Draw all geometry under each scissor. `PXUI_NO_CULL=1` also disables culling.
    pub fn set_no_cull(&mut self, no_cull: bool) {
        self.no_cull = no_cull;
    }

    /// Renders pending damage into the surface. Returns false if nothing needed presenting or no drawable was available.
    ///
    /// Native failures name the operation and renderer errors retain their library category.
    pub fn draw(
        &mut self,
        window: &mut Window,
        ui: &Ui,
        trace: Option<&mut [u64; 4]>,
    ) -> Result<bool, Error> {
        self.draw_inner(window, ui, trace, false)
    }

    /// Presents even when the UI is clean, for workloads that measure a fixed number of GPU frames.
    pub fn draw_forced(
        &mut self,
        window: &mut Window,
        ui: &Ui,
        trace: Option<&mut [u64; 4]>,
    ) -> Result<bool, Error> {
        self.draw_inner(window, ui, trace, true)
    }

    fn draw_inner(
        &mut self,
        window: &mut Window,
        ui: &Ui,
        trace: Option<&mut [u64; 4]>,
        force: bool,
    ) -> Result<bool, Error> {
        self.acquire_wall = std::time::Duration::ZERO;
        let count = || {
            if trace.is_some() {
                mem_sys::local_alloc_count()
            } else {
                0
            }
        };
        let before = count();
        let extent = window.extent();
        let size = (extent.drawable.0 as u32, extent.drawable.1 as u32);
        let viewport = Rect::new(0.0, 0.0, extent.logical.0 as f32, extent.logical.1 as f32);
        #[cfg(target_os = "macos")]
        let presentation = window.presentation();
        #[cfg(target_os = "macos")]
        {
            let Some(generation) = presentation else {
                self.presentation = None;
                return Ok(false);
            };
            if self.presentation != Some(generation) {
                self.held = Some(DamageList::full(viewport));
            }
        }
        let reported = ui.damage_list();
        let count_rects = usize::from(reported.count);
        if reported.full != 0 || count_rects > reported.rects.len() || (self.full_damage && count_rects != 0) {
            self.held = Some(DamageList::full(viewport));
        } else if count_rects != 0 {
            let held = self.held.get_or_insert_with(DamageList::default);
            for &[x, y, w, h] in &reported.rects[..count_rects] {
                held.add(Rect::new(x, y, w, h), viewport);
            }
        }
        #[cfg(target_os = "ios")]
        if !self.can_draw() { return Ok(false); }
        if size.0 == 0 || size.1 == 0 {
            self.blur_extent = (0, 0);
            self.held = Some(DamageList::full(viewport));
            return Ok(false);
        }
        if self.blur_extent != size {
            self.renderer
                .as_mut()
                .expect("Live graphics renderer")
                .resize(size.0, size.1)?;
            self.blur_extent = size;
            self.held = Some(DamageList::full(viewport));
        }
        if force && self.held.is_none() {
            self.held = Some(DamageList::full(viewport));
        }
        if self.held.is_none() && !self.present_debt {
            return Ok(false);
        }
        let frames = self.frames.as_mut().expect("Live frame context");
        let acquiring = std::time::Instant::now();
        let began = frames.begin(size.0, size.1);
        self.acquire_wall = acquiring.elapsed();
        match began {
            Ok(true) => {}
            Ok(false) => return Ok(false),
            Err(_) => {
                self.blur_extent = (0, 0);
                self.held = Some(DamageList::full(viewport));
                return Err(Error::Host("could not begin native frame"));
            }
        }
        let acquired = count();
        let mut prepared = acquired;
        let mut render = || -> Result<_, Error> {
            if let Some(held) = self.held {
                let renderer = self.renderer.as_mut().expect("Live graphics renderer");
                let commands = frames.commands();
                let scale = window.scale() as f32;
                let color = ui.theme().tokens.panel;
                let mut pixels = [[0; 4]; 8];
                let mut len = 0;
                if !held.is_full() {
                    for r in held.rects() {
                        pixels[len] = damage_px([r.x, r.y, r.w, r.h], scale, size);
                        len += 1;
                    }
                    if !self.no_cull && len > 1 {
                        len = merge_pixel_damage(&mut pixels[..len], size);
                    }
                }

                renderer.prepare(
                    &Commands {
                        size: core::mem::size_of::<Commands>() as u32,
                        mtl_command_buffer: commands,
                        ..Commands::default()
                    },
                    frames.serial(),
                    frames.completed(),
                )?;
                prepared = count();
                let (encoder, partial) = frames.pass([color.r, color.g, color.b], &pixels[..len])
                    .map_err(|_| Error::Host("could not open native render pass"))?;
                let mut pass = Pass {
                    size: core::mem::size_of::<Pass>() as u32,
                    mtl_command_buffer: commands,
                    mtl_render_encoder: encoder,
                    width_px: size.0,
                    height_px: size.1,
                    scale,
                    ..Pass::default()
                };
                // A cache miss needs a full canvas to reconstruct its backdrop.
                let mut redraw = false;
                let mut split = |record: pxui::Recorder| {
                    if partial != 0 {
                        redraw = true;
                        return Err(Error::Host("backdrop blur needs a full canvas"));
                    }
                    split_backdrop(frames, pass, record)
                };
                let result = if partial != 0 && !self.no_cull {
                    renderer.draw_damage(&pass, pixels[0], &mut split)
                } else {
                    renderer.draw_with_split(&pass, &mut split)
                };
                if redraw {
                    frames.end_pass().ok_or(Error::Host("could not end native render pass"))?;
                    pass.mtl_render_encoder = frames.pass([color.r, color.g, color.b], &[])
                        .map_err(|_| Error::Host("could not open full native render pass"))?.0;
                    renderer.draw_with_split(&pass, &mut |record| split_backdrop(frames, pass, record))?;
                } else {
                    result?;
                    for rect in pixels.iter().take(partial).skip(1) {
                        frames.scissor(rect);
                        if self.no_cull {
                            renderer.draw(&pass)?;
                        } else {
                            renderer.draw_damage(&pass, *rect, &mut |_| Err(Error::Host("backdrop blur needs a full canvas")))?;
                        }
                    }
                }
            }
            let drawn = count();
            self.acquire_wall += frames.acquire().map_err(|_| Error::Host("could not acquire native drawable"))?;
            let presented = frames.submit().map_err(|_| Error::Host("could not submit native frame"))?;
            Ok((drawn, presented))
        };
        let (drawn, presented) = match render() {
            Ok(result) => result,
            Err(error) => {
                frames.cancel();
                self.blur_extent = (0, 0);
                self.held = Some(DamageList::full(viewport));
                return Err(error);
            }
        };
        #[cfg(target_os = "macos")]
        if presented { self.presentation = presentation; }
        self.present_debt = !presented;
        self.held = None;
        // Suspending iOS may discard the command buffer while waiting for a drawable.
        #[cfg(target_os = "ios")]
        if !frames.can_draw() {
            self.blur_extent = (0, 0);
            self.held = Some(DamageList::full(viewport));
        }
        self.images.submitted(frames.serial());
        let completed = frames.completed();
        self.retire_images(completed)?;
        let submitted = count();
        if let Some(trace) = trace {
            *trace = [
                acquired - before,
                prepared - acquired,
                drawn - prepared,
                submitted - drawn,
            ];
        }
        Ok(presented)
    }

    /// Draws one frame like `draw` and returns what it presented: drawable-pixel width, height and
    /// RGBA rows, top row first, in the drawable's own color values (no display color matching).
    /// For development captures such as reference goldens; needs no screen-recording rights.
    ///
    /// Errors: `Error::Host` when no frame could be drawn (zero extent, hidden or inactive surface)
    /// or the GPU copy failed; renderer errors keep their library category.
    /// Allocates: the returned pixels and, on first use or growth, a retained shared GPU buffer.
    pub fn screenshot(&mut self, window: &mut Window, ui: &Ui) -> Result<(u32, u32, Vec<u8>), Error> {
        self.capture_frame(window, ui, true)
    }

    pub(crate) fn capture_frame(&mut self, window: &mut Window, ui: &Ui, full: bool) -> Result<(u32, u32, Vec<u8>), Error> {
        if full {
            self.blur_extent = (0, 0);
            self.held = Some(DamageList::full(Rect::default()));
        }
        let frames = self.frames.as_mut().expect("Live frame context");
        frames.capture(true).map_err(|_| Error::Host("could not start frame capture"))?;
        let drawn = self.draw_inner(window, ui, None, true);
        let frames = self.frames.as_mut().expect("Live frame context");
        let stopped = frames.capture(false);
        let drawn = drawn?;
        stopped.map_err(|_| Error::Host("could not stop frame capture"))?;
        if !drawn {
            return Err(Error::Host("no frame was drawn to capture"));
        }
        let (width, height, mut pixels) = frames
            .read_capture()
            .map_err(|_| {
                self.blur_extent = (0, 0);
                Error::Host("could not read the frame capture")
            })?
            .ok_or(Error::Host("no frame was captured"))?;
        for pixel in pixels.chunks_exact_mut(4) {
            pixel.swap(0, 2); // BGRA drawable to RGBA
        }
        Ok((width, height, pixels))
    }

    /// A newly visible or moved surface needs a full redraw even with an unchanged UI revision.
    pub fn surface_changed(&self, window: &Window) -> bool {
        #[cfg(target_os = "macos")]
        { window.presentation().is_some_and(|generation| Some(generation) != self.presentation) }
        #[cfg(target_os = "ios")]
        { let _ = window; false }
    }

    /// Whether encoded canvas content or held damage still needs to reach the surface.
    pub fn needs_present(&self) -> bool {
        self.present_debt || self.held.is_some()
    }

    /// Wall time the last `draw` spent acquiring its drawable and frame slot: display pacing,
    /// not frame work, so callers can subtract it from a whole-frame wall span.
    pub fn acquire_wall(&self) -> std::time::Duration {
        self.acquire_wall
    }

    /// Waits for submitted work, then releases this surface's graphics resources.
    ///
    /// A failed wait returns `Error::Host`. Drop still runs the graphics cleanup path.
    pub fn close(mut self) -> Result<(), Error> {
        self.frames.as_mut().expect("Live frame context").wait()
            .map_err(|_| Error::Host("could not wait for graphics completion"))
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

fn split_backdrop(frames: &mut Frames, mut pass: Pass, record: pxui::Recorder<'_>) -> Result<Pass, Error> {
    let canvas = frames.end_pass().ok_or(Error::Host("could not end native render pass"))?;
    record(&pxui::Image {
        mtl_texture: canvas.as_ptr(), width: pass.width_px, height: pass.height_px,
        format: pxui::Image::FORMAT_BGRA8, ..pxui::Image::default()
    })?;
    pass.mtl_render_encoder = frames.resume_pass()
        .ok_or(Error::Host("could not resume native render pass"))?.as_ptr();
    Ok(pass)
}

/// Rounds damage out to drawable pixels and pads one pixel for antialiasing.
fn damage_px([x, y, w, h]: [f32; 4], scale: f32, (width, height): (u32, u32)) -> [u32; 4] {
    let edge = |v: f32, limit: u32| v.clamp(0.0, limit as f32) as u32;
    let (x0, y0) = (edge((x * scale).floor() - 1.0, width), edge((y * scale).floor() - 1.0, height));
    let (x1, y1) = (edge(((x + w) * scale).ceil() + 1.0, width), edge(((y + h) * scale).ceil() + 1.0, height));
    [x0, y0, x1.saturating_sub(x0), y1.saturating_sub(y0)]
}

// Rounding and AA padding can join damage that was disjoint in points, saving a replay.
fn merge_pixel_damage(pixels: &mut [[u32; 4]], (width, height): (u32, u32)) -> usize {
    let mut damage = DamageList::default();
    let viewport = Rect::new(0.0, 0.0, width as f32, height as f32);
    for &[x, y, w, h] in pixels.iter() {
        damage.add(Rect::new(x as f32, y as f32, w as f32, h as f32), viewport);
    }
    let rects = damage.rects();
    if damage.is_full() || rects.is_empty() || rects.len() == pixels.len() {
        return pixels.len();
    }
    for (pixel, rect) in pixels.iter_mut().zip(rects) {
        *pixel = [rect.x, rect.y, rect.w, rect.h].map(|v| v as u32);
    }
    rects.len()
}

#[cfg(all(test, target_os = "macos"))]
pub(crate) fn damage_rounds_out_pads_and_clamps_to_the_drawable() {
    // 10.25..24.25 points at 2x: 20.5..48.5 px, out to 20..49, padded to 19..50
    assert_eq!(damage_px([10.25, 5.0, 14.0, 3.0], 2.0, (100, 80)), [19, 9, 31, 8]);
    assert_eq!(damage_px([-5.0, 30.0, 100.0, 100.0], 2.0, (100, 80)), [0, 59, 100, 21]);
    assert_eq!(damage_px([200.0, 0.0, 5.0, 5.0], 2.0, (100, 80))[2], 0);

    let mut pixels = [[9, 9, 4, 14], [13, 14, 4, 8], [17, 10, 4, 13], [60, 20, 4, 4]];
    let len = merge_pixel_damage(&mut pixels, (100, 80));
    assert_eq!(&pixels[..len], &[[9, 9, 12, 14], [60, 20, 4, 4]]);
}
