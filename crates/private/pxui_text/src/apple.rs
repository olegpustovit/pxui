//! Native masks are made on atlas misses. Face ids and physical cache keys still belong to cosmic-text.
use cosmic_text::{CacheKey, CacheKeyFlags, FontSystem, SubpixelBin, fontdb};
use font_sys::{MaskFace, MaskFont, MaskLayout, MaskRaster};
use std::collections::VecDeque;

const FACE_LIMIT: usize = 16;
const FONT_LIMIT: usize = 64;

struct Face {
    native: MaskFace,
    weight: Option<(f32, f32)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct SizedKey {
    face: fontdb::ID,
    ppem: u32,
    weight: u16,
}

#[derive(Default)]
pub(super) struct Masks {
    faces: VecDeque<(fontdb::ID, Option<Face>)>,
    fonts: VecDeque<(SizedKey, Option<MaskFont>)>,
    raster: MaskRaster,
}

impl Masks {
    pub(super) fn draw(&mut self, fs: &mut FontSystem, key: CacheKey) -> Option<(MaskLayout, &[u8])> {
        if key.flags.intersects(CacheKeyFlags::PIXEL_FONT | CacheKeyFlags::FAKE_ITALIC) { return None; }
        let wanted = SizedKey { face: key.font_id, ppem: key.font_size_bits, weight: key.font_weight.0 };
        let ppem = f32::from_bits(wanted.ppem);
        let font = match self.fonts.iter().find(|(known, _)| *known == wanted) {
            Some((_, font)) => font.as_ref()?,
            None => {
                let face = match self.faces.iter().find(|(id, _)| *id == wanted.face) {
                    Some((_, face)) => face,
                    None => {
                        if self.faces.len() == FACE_LIMIT { self.faces.pop_front(); }
                        self.faces.push_back((wanted.face, load_face(fs, key)));
                        &self.faces.back()?.1
                    }
                };
                let font = face.as_ref().and_then(|face| {
                    let weight = face.weight.map(|(min, max)| f32::from(wanted.weight).clamp(min, max));
                    face.native.at_size(ppem, weight)
                });
                if self.fonts.len() == FONT_LIMIT { self.fonts.pop_front(); }
                self.fonts.push_back((wanted, font));
                self.fonts.back()?.1.as_ref()?
            }
        };
        // At three phases the old quarter/three-quarter keys name the first/second third.
        // Use coordinates inside those quantization bins and let CoreText select the third.
        let phase = match (super::apple_x_bins(ppem), key.x_bin) {
            (3, SubpixelBin::One) => 0.375,
            (_, bin) => bin.as_float(),
        };
        self.raster.draw(font, key.glyph_id, phase)
    }
}

fn load_face(fs: &mut FontSystem, key: CacheKey) -> Option<Face> {
    let font = fs.get_font(key.font_id, key.font_weight)?;
    let face = font.as_swash();
    if [*b"sbix", *b"CBDT", *b"COLR", *b"SVG "].into_iter().any(|tag| face.table(u32::from_be_bytes(tag)).is_some()) {
        return None;
    }
    let (data, index) = fs.db().face_source(key.font_id)?;
    let data = match data {
        fontdb::Source::Binary(data) | fontdb::Source::SharedFile(_, data) => data,
        fontdb::Source::File(_) => return None,
    };
    let weight = face.variations().find_by_tag(u32::from_be_bytes(*b"wght"))
        .map(|axis| (axis.min_value(), axis.max_value()));
    let name = &fs.db().face(key.font_id)?.post_script_name;
    Some(Face { native: MaskFace::new(data, index, name)?, weight })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AtlasKind, PRAGMASEVKA, Style, Text};

    #[test]
    fn apple_atlas_upload_keeps_native_coverage_and_origin() {
        let mut text = Text::new(&[PRAGMASEVKA]);
        text.set_apple_raster(true);
        let style = Style { size: 13.0, ..Style::default() };
        text.layout(1, "j", &style, 0.0);
        let key = {
            let entry = &text.entries[&1];
            let run = entry.buf.layout_runs().next().unwrap();
            let g = &run.glyphs[0];
            assert_eq!(g.x + g.font_size * g.x_offset, 0.0);
            assert_eq!(g.y - g.font_size * g.y_offset, 0.0);
            CacheKey { font_id: g.font_id, glyph_id: g.glyph_id, font_size_bits: g.font_size.to_bits(),
                       x_bin: SubpixelBin::One, y_bin: SubpixelBin::Zero, font_weight: g.font_weight, flags: g.cache_key_flags }
        };

        let face = load_face(&mut text.g.fs, key).unwrap();
        let font = face.native.at_size(style.size, None).unwrap();
        let mut raster = MaskRaster::default();
        let (expected, expected_pixels) = raster.draw(&font, key.glyph_id, 0.375).unwrap();
        assert!(expected.width > 0 && expected.height > 0);

        let mut drawn = None;
        text.glyphs_at_baseline(1, (10.4, 25.0), 1.0, &mut |glyph| drawn = Some(glyph));
        let glyph = drawn.unwrap();
        assert_eq!(glyph.rect.x, (10 + expected.left) as f32);
        assert_eq!(glyph.rect.y, (25 - expected.top) as f32);
        assert_eq!((glyph.rect.w as u32, glyph.rect.h as u32), (expected.width, expected.height));
        assert_eq!(glyph.atlas, AtlasKind::Mask);
        let (width, height) = text.atlas_size(glyph.atlas);
        let x = (glyph.uv.x * width as f32).round() as usize;
        let y = (glyph.uv.y * height as f32).round() as usize;
        let width = width as usize;
        let pixels = text.atlas_pixels(glyph.atlas);
        for row in 0..expected.height as usize {
            let at = (y + row) * width + x;
            let from = row * expected.stride;
            let count = expected.width as usize;
            assert_eq!(&pixels[at..at + count], &expected_pixels[from..from + count], "mask row {row}");
        }
    }
}
