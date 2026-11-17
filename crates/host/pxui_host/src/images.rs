//! Retire textures only after actual completed GPU serials. Forget every image on each sampling
//! renderer after its final sampling serial completes and before destroying/reusing its native
//! handle. Cache bounds remain 1,024 entries and 128 MiB resident texels. Scan only pending
//! retirements, without extra frames to observe completion. Retained segments with no IMAGE
//! instances must ignore stale bindings.
//!
//! Main-thread texture ownership; frame builders borrow only `Images` descriptors.
use ogpu_core::{
    backend as be,
    types::{Extent, Origin, TextureDescriptor, TextureFormat, TextureUsage},
};
use pxui::images::{BYTE_LIMIT, CAPACITY, Images, Want};
use pxui::Error;

const RETIRED_LIMIT: usize = CAPACITY * 3;
struct Texture {
    id: usize,
    value: be::Texture,
    bytes: usize,
    serial: u64,
    image: pxui::Image,
}
/// A GPU refusal: the backend's own detail stays in its log; the caller gets the typed error.
fn error(_: ogpu_core::error::Error) -> Error {
    Error::Gpu
}

pub struct ImageCache {
    pub descriptors: Images,
    /// One texture per cached id that holds pixels, at most `CAPACITY` (the descriptors' bound).
    textures: Vec<Texture>,
    retired: Vec<Texture>,
    bytes: usize,
    namespace: usize,
    seen: u64,
}
impl Default for ImageCache {
    fn default() -> Self {
        Self {
            descriptors: Images::default(),
            textures: Vec::with_capacity(CAPACITY),
            retired: Vec::with_capacity(RETIRED_LIMIT),
            bytes: 0,
            namespace: 0,
            seen: 0,
        }
    }
}
impl ImageCache {
    pub(crate) fn configure(&mut self, density: f32, max_pixels: u32) -> Result<(), Error> {
        if self.descriptors.configure(density, max_pixels) {
            self.evict_all()?;
        }
        Ok(())
    }
    pub fn resident_bytes(&self) -> usize {
        self.bytes
    }
    #[cfg(test)]
    pub(crate) fn texture(&self, id: usize) -> Option<&be::Texture> {
        self.textures.iter().find(|texture| texture.id == id).map(|texture| &texture.value)
    }
    pub(crate) fn retire_completed(
        &mut self,
        completed: u64,
        renderer: &mut pxui::Renderer,
    ) -> Result<(), Error> {
        let mut index = 0;
        while index < self.retired.len() {
            let entry = &self.retired[index];
            if entry.serial <= completed {
                renderer
                    .forget_image(&entry.image, completed)?;
                be::destroy_texture(&entry.value).map_err(error)?;
                self.retired.swap_remove(index);
            } else {
                index += 1;
            }
        }
        Ok(())
    }
    /// Retires image `id`'s texture (if it has one) for release after its last sampling frame.
    fn retire(&mut self, id: usize) -> Result<(), Error> {
        if let Some(index) = self.textures.iter().position(|texture| texture.id == id) {
            if self.retired.len() == RETIRED_LIMIT {
                return Err(Error::State);
            }
            let texture = self.textures.swap_remove(index);
            self.bytes -= texture.bytes;
            self.retired.push(texture);
        }
        Ok(())
    }
    /// Retires image `id`'s texture and forgets its descriptor.
    fn evict(&mut self, id: usize) -> Result<(), Error> {
        self.retire(id)?;
        self.descriptors.remove(id);
        Ok(())
    }
    fn evict_all(&mut self) -> Result<(), Error> {
        while let Some(texture) = self.textures.last() {
            self.retire(texture.id)?;
        }
        self.descriptors.clear();
        Ok(())
    }
    pub(crate) fn sync_namespace(&mut self, namespace: usize) -> Result<bool, Error> {
        if self.namespace == namespace {
            return Ok(false);
        }
        self.evict_all()?;
        self.namespace = namespace;
        Ok(true)
    }
    pub(crate) fn upload(
        &mut self,
        device: &be::Device,
        key: Want,
        epoch: u64,
        revision: u64,
        pixels: Option<(u32, u32, &[u8])>,
    ) -> Result<bool, Error> {
        if revision <= self.seen {
            return Ok(false);
        }
        if epoch != self.descriptors.epoch() {
            self.seen = revision;
            return Ok(false);
        }
        if self
            .descriptors
            .quality(key.id())
            .is_some_and(|quality| quality >= key.max_pixels())
        {
            self.seen = revision;
            return Ok(false);
        }
        let (mut owned, mut image, mut bytes) = (None, None, 0);
        if let Some((width, height, pixels)) = pixels {
            if width == 0
                || height == 0
                || width > 1024
                || height > 1024
                || pixels.len() != width as usize * height as usize * 4
            {
                return Err(Error::InvalidInput);
            }
            bytes = pixels.len();
            while self.bytes + bytes > BYTE_LIMIT {
                let oldest = self.descriptors.oldest_image().ok_or(Error::State)?;
                self.evict(oldest)?;
            }
            let texture = be::create_texture(
                device,
                TextureDescriptor {
                    label: None,
                    width: width as i64,
                    height: height as i64,
                    depth: 1,
                    mip_levels: 1,
                    sample_count: 1,
                    format: TextureFormat::Rgba8Unorm,
                    usage: vec![
                        TextureUsage::TextureBinding,
                        TextureUsage::TextureCopyDst,
                        TextureUsage::TextureCopySrc,
                    ],
                },
            )
            .map_err(error)?;
            let prepared = be::write_texture(
                &texture,
                None,
                Origin { x: 0, y: 0, z: 0 },
                Extent {
                    width: width as i64,
                    height: height as i64,
                    depth: 1,
                },
                width as i64 * 4,
                pixels,
            )
            .and_then(|_| be::native_texture(&texture));
            let native = match prepared {
                Ok(native) => native,
                Err(failure) => {
                    let _ = be::destroy_texture(&texture);
                    return Err(error(failure));
                }
            };
            image = Some(pxui::Image {
                vk_image_view: native.vk_image_view,
                mtl_texture: native.mtl_texture,
                width: native.width,
                height: native.height,
                format: native.format,
                ..Default::default()
            });
            owned = Some(Texture {
                id: key.id(),
                value: texture,
                bytes,
                serial: 0,
                image: image.unwrap(),
            });
        }
        // the id's old texture and the image `store` may drop for room retire at most one each: room for both, before any change
        if self.retired.len() + 2 > RETIRED_LIMIT {
            if let Some(owned) = owned {
                let _ = be::destroy_texture(&owned.value);
            }
            return Err(Error::State);
        }
        self.evict(key.id())?;
        if let Some(dropped) = self.descriptors.store(key, image) {
            self.retire(dropped)?;
        }
        self.textures.extend(owned);
        self.bytes += bytes;
        self.seen = revision;
        Ok(true)
    }
    pub(crate) fn submitted(&mut self, serial: u64) {
        for texture in &mut self.textures {
            texture.serial = serial;
        }
    }
    /// The owner waits for frames and drops the renderer before destroying this cache.
    pub(crate) fn destroy(&mut self) -> Result<(), Error> {
        for texture in self.textures.iter().chain(&self.retired) {
            be::destroy_texture(&texture.value).map_err(error)?;
        }
        self.textures.clear();
        self.retired.clear();
        self.bytes = 0;
        self.descriptors.clear();
        Ok(())
    }
}
