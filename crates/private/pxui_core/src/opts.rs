//! The option records of the widgets that take closures (`Rc<dyn Fn>`): Rust-side only, never an ABI type. The engine keeps its own; the wrapper has the same records (`pxui::opts`).
use pxui_types::{geom::{Rect, Rgba}, node::Axis, ui::{CodeContext, Completion, ScrubPhase}, Flags, Size};
use crate::ui_core::Drop;
use std::rc::Rc;

pub type HitFn = Rc<dyn Fn(Rect) -> Rect>;

#[derive(Clone)]
pub struct BoxOpts {
    pub flags: Flags,
    pub w: Size,
    pub h: Size,
    pub max_h: f32,
    pub axis: Axis,
    pub padding: f32,
    pub at: Option<(f32, f32)>,
    pub scroll_step: f32,
    pub hit: Option<HitFn>,
    pub hit_rect: Option<Rect>,
}

impl Default for BoxOpts {
    fn default() -> BoxOpts {
        BoxOpts {
            flags: Flags::NONE,
            w: Size::Grow,
            h: Size::Fit,
            max_h: f32::INFINITY,
            axis: Axis::Column,
            padding: 0.0,
            at: None,
            scroll_step: 24.0,
            hit: None,
            hit_rect: None,
        }
    }
}

impl BoxOpts {
    pub fn flags(mut self, flags: Flags) -> BoxOpts {
        self.flags = flags;
        self
    }
    pub fn w(mut self, w: Size) -> BoxOpts {
        self.w = w;
        self
    }
    pub fn h(mut self, h: Size) -> BoxOpts {
        self.h = h;
        self
    }
    pub fn max_h(mut self, max_h: f32) -> BoxOpts {
        self.max_h = max_h;
        self
    }
    pub fn axis(mut self, axis: Axis) -> BoxOpts {
        self.axis = axis;
        self
    }
    pub fn padding(mut self, padding: f32) -> BoxOpts {
        self.padding = padding;
        self
    }
    pub fn at(mut self, at: (f32, f32)) -> BoxOpts {
        self.at = Some(at);
        self
    }
    pub fn scroll_step(mut self, step: f32) -> BoxOpts {
        self.scroll_step = step;
        self
    }
    pub fn hit(mut self, hit: HitFn) -> BoxOpts {
        self.hit = Some(hit);
        self
    }
}

/// Options of `value_field` beyond its place and key. Its `slide` and `scrub` callbacks are the call's arguments (borrowed for it).
#[derive(Clone, Default)]
pub struct ValueFieldOpts {
    pub size: Option<i64>,
    pub display: Option<String>,
    pub fraction: Option<f32>,
    pub left: bool,
    pub edit: bool,
    pub lead: Option<(String, Rgba)>,
    pub trail: Option<(String, Rgba)>,
    pub line: Option<Rgba>,
    pub bare: bool,
    pub placeholder: Option<String>,
    pub tracking: Option<f32>,
    /// the owner follows a drag of the slider between builds (`field_drag`, `patch_field`): the
    /// drag dirties no view (`DRAG_FREE`)
    pub kept_drag: bool,
}

/// What a code editor knows about its text. A host supplies one (the editor's Lisp); the text area
/// itself is language-free. Byte positions are byte offsets into the text.
#[derive(Clone)]
pub struct Language {
    pub colorize_range: Option<Rc<dyn Fn(&str, std::ops::Range<usize>) -> Vec<(i64, i64, Rgba)>>>,
    pub brackets_range: Option<Rc<dyn Fn(&str, std::ops::Range<usize>, &mut Vec<(i64, i64)>)>>,
    /// sorted, non-overlapping byte spans and their colour; the rest is the foreground
    pub colorize: Rc<dyn Fn(&str) -> Vec<(i64, i64, Rgba)>>,
    /// the matched bracket pairs as (open, close) byte positions: the pair at the caret is lit
    pub brackets: Rc<dyn Fn(&str) -> Vec<(i64, i64)>>,
    /// `indent text caret`: the indentation Enter puts after the line break at `caret`
    pub indent: Rc<dyn Fn(&str, i64) -> String>,
    /// brackets typed in pairs: the opener wraps the selection or inserts both, the closer typed
    /// before itself steps over it, Backspace between an empty pair takes both
    pub pairs: Vec<(char, char)>,
    /// `complete text caret`: the ranked suggestions for the token ending at `caret`, best first
    pub complete: Rc<dyn Fn(&str, i64) -> Vec<Completion>>,
    /// `describe text byte`: the token at `byte` and its description, shown as a tooltip
    pub describe: Rc<dyn Fn(&str, i64, &mut String) -> Option<(i64, i64)>>,
    /// `number_at text byte`: the numeric literal at `byte`; dragging it sideways changes the value
    pub number_at: Rc<dyn Fn(&str, i64) -> Option<(i64, i64)>>,
    /// `context_at text caret`: what a character typed at `caret` lands in
    pub context_at: Rc<dyn Fn(&str, i64) -> CodeContext>,
    /// what starts a line comment: Command-/ comments or uncomments the lines the selection touches
    pub comment: String,
    /// `rewrite text caret`, run after every frame that edited the text: `None` when unchanged
    /// (no allocation), else the kept text and where `caret` lands in it
    pub rewrite: Option<Rc<dyn Fn(&str, i64) -> (Option<String>, i64)>>,
}

impl Language {
    pub(crate) fn pairs_in(&self, text: &str, range: std::ops::Range<usize>, out: &mut Vec<(i64,i64)>) {
        out.clear();
        if let Some(f) = &self.brackets_range { f(text, range, out); }
        else { *out = (self.brackets)(text); }
    }
}

/// Options of `text_area_submit` (see there).
#[derive(Clone)]
pub struct TextAreaOpts {
    pub wrap: bool,
    pub errors: Vec<i64>,
    pub messages: Vec<(i64, Option<(i64, i64)>, String)>,
    pub spans: Vec<(i64, i64)>,
    pub reveal: Option<i64>,
    pub reveal_nonce: i64,
    pub select: Option<(i64, i64)>,
    pub splice: Vec<(i64, i64, String)>,
    pub language: Option<Language>,
    pub on_context: Option<Rc<dyn Fn((f32, f32))>>,
    pub on_scrub: Option<Rc<dyn Fn(ScrubPhase)>>,
    pub on_scrub_edit: Option<Rc<dyn Fn((i64, i64), &str)>>,
    pub on_click: Option<Rc<dyn Fn(i64, bool)>>,
    pub on_caret: Option<Rc<dyn Fn(i64)>>,
    pub on_caret_move: Option<Rc<dyn Fn(i64)>>,
    pub on_select: Option<Rc<dyn Fn((i64, i64))>>,
    pub on_drop: Option<Rc<dyn Fn(i64, Drop)>>,
    pub chips: Vec<(i64, i64, Rgba)>,
}

impl Default for TextAreaOpts {
    fn default() -> TextAreaOpts {
        TextAreaOpts {
            wrap: false,
            errors: Vec::new(),
            messages: Vec::new(),
            spans: Vec::new(),
            reveal: None,
            reveal_nonce: 0,
            select: None,
            splice: Vec::new(),
            language: None,
            on_context: None,
            on_scrub: None,
            on_scrub_edit: None,
            on_click: None,
            on_caret: None,
            on_caret_move: None,
            on_select: None,
            on_drop: None,
            chips: Vec::new(),
        }
    }
}

/// A picker's options (its row functions are `PickerRowFns`, borrowed for the call).
#[derive(Clone, Copy)]
pub struct PickerOpts {
    pub limit: i64,
    pub slash: bool,
    pub at_rest: bool,
}

impl Default for PickerOpts {
    fn default() -> Self {
        PickerOpts { limit: 10, slash: true, at_rest: false }
    }
}

/// A picker's row functions, borrowed for the call (nothing kept, nothing allocated): `mark` a row's colour square, `off` a row that
/// cannot be picked, `image` a row's image (U2b.4.15: `PickerRowConfig::image`, drawn in the art slot).
pub type PickerRowFns<'a> = (Option<&'a dyn Fn(i64) -> Option<Rgba>>, Option<&'a dyn Fn(i64) -> bool>, Option<&'a dyn Fn(i64) -> Option<crate::ui_core::Image>>);
