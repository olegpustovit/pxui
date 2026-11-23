use ogpu_core::backend::{self, RenderEncoder};
use ogpu_core::error::Error;
use ogpu_core::render_pass::Rect;

/// Each disjoint piece stays inside its original damage rectangle. A pixel belongs to the first
/// rectangle that contains it, so translucent geometry blends once even when rectangles overlap.
pub struct DamageClip {
    // At most 225 finished cells and 225 in the rectangle being partitioned.
    pieces: [[u32; 4]; 450],
    ends: [u8; 9],
    len: usize,
}

impl DamageClip {
    pub fn new(rects: &[[u32; 4]]) -> Self {
        let mut clip = Self { pieces: [[0; 4]; 450], ends: [0; 9], len: rects.len() };
        let mut end = 0;
        for (i, &rect) in rects.iter().enumerate() {
            let start = end;
            clip.pieces[end] = rect;
            end += 1;
            for &[x, y, w, h] in &rects[..i] {
                let mut index = start;
                while index < end {
                    let [left, top, width, height] = clip.pieces[index];
                    let (right, bottom) = (left + width, top + height);
                    let (l, t, r, b) = (left.max(x), top.max(y), right.min(x + w), bottom.min(y + h));
                    if l >= r || t >= b {
                        index += 1;
                        continue;
                    }
                    end -= 1;
                    clip.pieces[index] = clip.pieces[end];
                    for [l, t, r, b] in [[left, top, right, t], [left, b, right, bottom], [left, t, l, b], [r, t, right, b]] {
                        if l < r && t < b {
                            clip.pieces[end] = [l, t, r - l, b - t];
                            end += 1;
                        }
                    }
                }
            }
            // Eight rectangles have at most 16 edges per axis. Every piece contains a distinct
            // cell of that 15×15 grid after subtracting all earlier rectangles.
            clip.ends[i + 1] = end as u8;
        }
        clip
    }

    pub fn len(&self) -> usize { self.len }

    pub fn rect(&self, index: usize) -> &[[u32; 4]] {
        &self.pieces[usize::from(self.ends[index])..usize::from(self.ends[index + 1])]
    }

    pub fn all(&self) -> &[[u32; 4]] { &self.pieces[..usize::from(self.ends[self.len])] }
}

pub fn set_scissor(pass: &RenderEncoder, rect: [u32; 4]) -> Result<(), Error> {
    let [x, y, width, height] = rect.map(i64::from);
    backend::set_scissor(pass, Rect { x, y, width, height })
}
