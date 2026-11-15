//! On-demand MAE of pxui_text's glyph raster against CoreText (`tools/glyph_ref/run.sh`; macOS only).
//! Skipped unless `PXUI_GLYPH_REF` names a directory of reference PGMs. Writes `mae.tsv` there:
//! MAE = mean per-pixel channel difference (scriptc-ui's metric), `other` = the unchanged raster.
use pxui_text::{AtlasKind, Style, Text};

const TEXT: &str = "Hamburgefonstiv Quick 0123456789";

fn pgm(path: &str) -> Option<(usize, usize, Vec<u8>)> {
    let b = std::fs::read(path).ok()?;
    let mut it = b.splitn(4, |c| *c == b'\n');
    it.next();
    let dims = String::from_utf8_lossy(it.next()?).into_owned();
    it.next();
    let (w, h) = dims.split_once(' ')?;
    Some((w.parse().ok()?, h.parse().ok()?, it.next()?.to_vec()))
}

fn render(t: &mut Text, size: f32, scale: f32, white: bool, w: usize, h: usize) -> Vec<u8> {
    let st = Style { size, ..Style::default() };
    let m = t.layout(1, TEXT, &st, f32::INFINITY);
    let mut quads = Vec::new();
    t.glyphs(1, (8.0, 40.0 - m.baseline), scale, &mut |g| quads.push(g));
    let (bg, fg) = if white { (0.0, 255.0) } else { (255.0, 0.0) };
    let mut out = vec![bg as u8; w * h];
    for g in quads {
        let (aw, ah) = t.atlas_size(g.atlas);
        let atlas = t.atlas_pixels(g.atlas);
        let (u, v) = ((g.uv.x * aw as f32).round() as usize, (g.uv.y * ah as f32).round() as usize);
        for y in 0..g.rect.h as usize {
            for x in 0..g.rect.w as usize {
                let (px, py) = (g.rect.x as i32 + x as i32, g.rect.y as i32 + y as i32);
                if px < 0 || py < 0 || px as usize >= w || py as usize >= h || g.atlas != AtlasKind::Mask && g.atlas != AtlasKind::Mask2 {
                    continue;
                }
                let a = atlas[(v + y) * aw as usize + u + x] as f32 / 255.0;
                let o = &mut out[py as usize * w + px as usize];
                *o = (*o as f32 + (fg - *o as f32) * a).round() as u8;
            }
        }
    }
    out
}

#[test]
fn glyph_ref_mae() {
    let Ok(dir) = std::env::var("PXUI_GLYPH_REF") else { return };
    let fonts_dir = std::fs::read_to_string(format!("{dir}/fonts_dir")).unwrap().trim().to_string();
    let faces = [
        ("hg", format!("{fonts_dir}/HostGrotesk.ttf")),
        ("gm", format!("{fonts_dir}/GeistMono.ttf")),
        ("ps", concat!(env!("CARGO_MANIFEST_DIR"), "/../../../assets/fonts/Pragmasevka-Regular.ttf").to_string()),
    ];
    let mut tsv = String::from("font\tsize\tscale\tpolarity\tother\tapple\n");
    for (name, path) in faces {
        let bytes = std::fs::read(path).unwrap();
        for size in [11, 13, 15, 17, 20, 28] {
            for scale in [1, 2] {
                for pol in ["black", "white"] {
                    let Some((w, h, reference)) = pgm(&format!("{dir}/{name}-{size}-{scale}x-{pol}.pgm")) else { continue };
                    let mut row = format!("{name}\t{size}\t{scale}x\t{pol}");
                    for apple in [false, true] {
                        let mut t = Text::new(&[&bytes]);
                        t.set_apple_raster(apple);
                        let ours = render(&mut t, size as f32, scale as f32, pol == "white", w, h);
                        let sum: u64 = ours.iter().zip(&reference).map(|(a, b)| a.abs_diff(*b) as u64).sum();
                        row += &format!("\t{:.3}", sum as f64 / (w * h) as f64);
                    }
                    println!("{row}");
                    tsv += &(row + "\n");
                }
            }
        }
    }
    std::fs::write(format!("{dir}/mae.tsv"), tsv).unwrap();
}
