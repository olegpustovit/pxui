//! Main-thread texture ownership; frame builders borrow only `Images` descriptors.
use ogpu_core::{
    backend as be,
    types::{Extent, Origin, TextureDescriptor, TextureFormat, TextureUsage},
};
use pxui::images::{BYTE_LIMIT, CAPACITY, Images, Key};

const RETIRED_LIMIT: usize = CAPACITY * 3;
struct Texture {
    value: be::Texture,
    bytes: usize,
    serial: u64,
    image: pxui::Image,
}
fn error(value: ogpu_core::error::Error) -> String {
    ogpu_core::error::to_string(&value)
}

pub struct ImageCache {
    pub descriptors: Images,
    textures: Vec<Option<Texture>>,
    retired: Vec<Texture>,
    bytes: usize,
    namespace: usize,
    seen: u64,
}
impl Default for ImageCache {
    fn default() -> Self {
        Self {
            descriptors: Images::default(),
            textures: (0..CAPACITY).map(|_| None).collect(),
            retired: Vec::with_capacity(RETIRED_LIMIT),
            bytes: 0,
            namespace: 0,
            seen: 0,
        }
    }
}
impl ImageCache {
    pub(crate) fn configure(&mut self, density: f32, max_pixels: u32) -> Result<(), String> {
        if self.descriptors.configure(density, max_pixels) {
            for index in 0..CAPACITY {
                self.evict(index)?;
            }
            self.descriptors.clear();
        }
        Ok(())
    }
    pub fn resident_bytes(&self) -> usize {
        self.bytes
    }
    pub fn texture(&self, id: usize) -> Option<&be::Texture> {
        self.descriptors
            .index(id)
            .and_then(|index| self.textures[index].as_ref())
            .map(|texture| &texture.value)
    }
    pub(crate) fn retire_completed(
        &mut self,
        completed: u64,
        renderer: &mut pxui::Renderer,
    ) -> Result<(), String> {
        let mut index = 0;
        while index < self.retired.len() {
            let entry = &self.retired[index];
            if entry.serial <= completed {
                renderer
                    .forget_image(&entry.image, completed)
                    .map_err(|code| format!("Image release failed ({code})"))?;
                be::destroy_texture(&entry.value).map_err(error)?;
                self.retired.swap_remove(index);
            } else {
                index += 1;
            }
        }
        Ok(())
    }
    fn evict(&mut self, index: usize) -> Result<(), String> {
        if let Some(entry) = &self.textures[index] {
            let bytes = entry.bytes;
            if self.retired.len() == RETIRED_LIMIT {
                return Err("Image retirement exceeded its three-frame bound".into());
            }
            self.retired.push(self.textures[index].take().unwrap());
            self.bytes -= bytes;
        }
        self.descriptors.remove(index);
        Ok(())
    }
    pub(crate) fn sync_namespace(&mut self, namespace: usize) -> Result<bool, String> {
        if self.namespace == namespace {
            return Ok(false);
        }
        for index in 0..CAPACITY {
            self.evict(index)?;
        }
        self.descriptors.clear();
        self.namespace = namespace;
        Ok(true)
    }
    pub(crate) fn upload(
        &mut self,
        device: &be::Device,
        key: Key,
        epoch: u64,
        revision: u64,
        pixels: Option<(u32, u32, &[u8])>,
    ) -> Result<bool, String> {
        if key.id == 0 || key.path.as_str().is_empty() || !(1..=1024).contains(&key.max_pixels) {
            return Err("Invalid image source key".into());
        }
        if revision <= self.seen {
            return Ok(false);
        }
        if epoch != self.descriptors.epoch() {
            self.seen = revision;
            return Ok(false);
        }
        if self
            .descriptors
            .quality(key.id)
            .is_some_and(|quality| quality >= key.max_pixels)
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
                return Err("Invalid decoded images dimensions".into());
            }
            bytes = pixels.len();
            while self.bytes + bytes > BYTE_LIMIT {
                let oldest = (0..CAPACITY)
                    .filter(|index| self.textures[*index].is_some())
                    .min_by_key(|index| self.descriptors.last_used(*index))
                    .ok_or("Invalid images byte accounting")?;
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
                value: texture,
                bytes,
                serial: 0,
                image: image.unwrap(),
            });
        }
        let index = self
            .descriptors
            .index(key.id)
            .unwrap_or_else(|| self.descriptors.oldest());
        if let Err(failure) = self.evict(index) {
            if let Some(owned) = owned {
                let _ = be::destroy_texture(&owned.value);
            }
            return Err(failure);
        }
        self.textures[index] = owned;
        self.bytes += bytes;
        self.descriptors.insert(index, key, image);
        self.seen = revision;
        Ok(true)
    }
    pub(crate) fn submitted(&mut self, serial: u64) {
        for texture in self.textures.iter_mut().flatten() {
            texture.serial = serial;
        }
    }
    /// The owner waits for frames and drops the renderer before destroying this cache.
    pub(crate) fn destroy(&mut self) -> Result<(), String> {
        for texture in self.textures.iter().flatten().chain(&self.retired) {
            be::destroy_texture(&texture.value).map_err(error)?;
        }
        for entry in &mut self.textures {
            entry.take();
        }
        self.retired.clear();
        self.bytes = 0;
        self.descriptors.clear();
        Ok(())
    }
}
