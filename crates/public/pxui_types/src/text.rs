//! Stable text inputs and outputs shared with libpxui.

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TextStyle {
    pub size: u32,
    pub font: u16,
    pub weight: u16,
    pub size_points: f32,
    pub line_height: f32,
    pub wrap: u32,
    pub align: u32,
    pub max_lines: u16,
    pub italic: u8,
    /// ELLIPSIS_NONE, END, MIDDLE or HEAD. max_lines 0 uses one visible line when enabled.
    pub ellipsis: u8,
    /// OpenType feature bits; 0 preserves the font defaults.
    pub features: u32,
    pub tracking: f32,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            size: core::mem::size_of::<Self>() as u32,
            font: 0,
            weight: 400,
            size_points: 14.0,
            line_height: 0.0,
            wrap: 0,
            align: 0,
            max_lines: 0,
            italic: 0,
            ellipsis: 0,
            features: 0,
            tracking: 0.0,
        }
    }
}

impl TextStyle {
    pub const ELLIPSIS_NONE: u8 = 0;
    pub const ELLIPSIS_END: u8 = 1;
    pub const ELLIPSIS_MIDDLE: u8 = 2;
    pub const ELLIPSIS_HEAD: u8 = 3;
    pub const FEATURE_TABULAR_NUMBERS: u32 = 1;
    pub const WRAP_NONE: u32 = 0;
    pub const WRAP_WORD: u32 = 1;
    pub const WRAP_GLYPH: u32 = 2;
    pub const ALIGN_LEFT: u32 = 0;
    pub const ALIGN_CENTER: u32 = 1;
    pub const ALIGN_RIGHT: u32 = 2;
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TextMetrics {
    pub width: f32,
    pub height: f32,
    pub baseline: f32,
    pub lines: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TextGlyph {
    pub rect: [f32; 4],
    pub uv: [f32; 4],
    /// 0/1: first mask/colour pages; 2/3: second mask/colour pages (ABI 1.7).
    pub atlas: u32,
    pub reserved: u32,
}


/// A nonempty UTF-8 byte range. Ranges are ordered and do not overlap; weight 0 inherits the base style.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextSpan {
    pub start: u32,
    pub end: u32,
    pub weight: u16,
    pub flags: u16,
    pub color: crate::geom::Rgba,
    pub background: crate::geom::Rgba,
    /// A 1.5-point rule at the bottom of the line box, independent of glyph colour.
    pub underline: crate::geom::Rgba,
}
impl TextSpan {
    pub const COLOR: u16=1;
    pub const BACKGROUND: u16=2;
    pub const UNDERLINE: u16=4;
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextSpans {
    pub size: u32,
    pub count: u32,
    pub spans: [TextSpan;8],
}
impl Default for TextSpans {
    fn default() -> Self { Self { size:core::mem::size_of::<Self>() as u32,count:0,spans:[TextSpan::default();8] } }
}
impl TextSpans {
    pub const MAX: usize=8;
    pub fn new(spans: &[TextSpan]) -> Option<Self> {
        if spans.len()>Self::MAX { return None; }
        let mut value=Self::default();
        value.count=spans.len() as u32;
        value.spans[..spans.len()].copy_from_slice(spans);
        Some(value)
    }
    pub fn as_slice(&self) -> Option<&[TextSpan]> { self.spans.get(..self.count as usize) }
}
