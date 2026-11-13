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
    pub atlas: u32,
    pub reserved: u32,
}
