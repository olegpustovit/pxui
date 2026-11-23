//! Kit widgets: a panel stacks rows of the PXUI design kit (rev 3): 24-point rows with 12 points at
//! each side, 20-point controls and the kit face (the inspector sheet's rows and sections are the
//! extension's, `kit_ext`). Every widget reads its appearance from the theme in effect (`Ui::theme`): it declares
//! nodes, picks each node's look from its `States` for the node's state, and the engine paints them;
//! no standard widget paints with a closure. Widgets must be built inside a `panel`.
use crate::ui_core::*;
use crate::bodies::{body_of, in_body, in_item, ButtonConfig, FooterConfig, GripConfig, LabelConfig, PanelConfig, SplitConfig, ToggleConfig};
pub use pxui_types::ui::{HeadAction, PanelOpts};
pub use pxui_types::Label;
use pxui_text::{Align as TextAlign, Ellipsis, Style, Wrap};
use pxui_types::{ffi::PxuiTextStyle as TextStyle, text::LabelFields, ui::{DismissOpts, Presentation}};
use crate::a11y::state as st;
use input::{Event, Key};
use crate::geom::Rgba;
use std::borrow::Cow;

pub type IRect = (i64, i64, i64, i64);

/// Added to a part's number before it is mixed into its widget's key: far outside the small child indices (`int_key(owner, 0..n)`)
/// that widgets and bodies number their ordinary children with, so a part never collides with one. Part of the contract: never changes.
pub const PART_BASE: i64 = 1 << 40;

/// The key of `part` of the widget keyed `owner`, wherever the body nests the node.
pub fn part_key(owner: i64, part: Part) -> i64 {
    int_key(owner, PART_BASE + part.0 as i64)
}

/// The built-in toggle: the label on its row and the switch (28 x 14 unless the caller sized it) 12 (or `inset`) from the row's end, the
/// knob 3 in from its side and `switch_h - 6` square. With a `label` style the title is line-box text in it (unset fields the toggle's
/// look); with a `detail` the title sits over the detail, a Label node in the theme's label look wrapped to the room the switch leaves,
/// and the row grows past its height to fit them (`space(1)` above and below).
pub(crate) fn default_toggle(this: &mut Ui, c: &ToggleConfig, track: Option<&States>, knob: Option<&States>, label: &str) {
    let t = this.theme();
    let on = State { checked: c.on, ..State::default() };
    let look = *t.toggle.pick(c.state);
    let (track, knob) = (*track.unwrap_or(&t.switch).pick(on), *knob.unwrap_or(&t.knob).pick(on));
    let (rh, w, h) = (if c.row_h > 0.0 { c.row_h } else { this.row_height() }, if c.switch_w > 0.0 { c.switch_w } else { 28.0 }, if c.switch_h > 0.0 { c.switch_h } else { 14.0 });
    // the switch's end is `space(2)` from the row's end at any density
    let side = if c.inset > 0.0 { c.inset } else { this.space(2) };
    this.set_look(&c.row, look, c.state);
    let mut top = ((rh - h) / 2.0).floor();
    if c.detail.is_empty() && c.label.is_none() {
        this.node_new(&c.row).inset = Edges { l: side, ..Edges::default() };
        this.text(&c.row, shown(label));
    } else {
        // the title (and the detail under it) in a growing column, the switch centred beside them
        this.a11y_name(&c.row, shown(label));
        let n = this.node_new(&c.row);
        (n.padding.l, n.align) = (side, Align::Center);
        if !c.detail.is_empty() {
            let pad = this.space(1);
            let n = this.node_new(&c.row);
            (n.h, n.min_h, n.padding.t, n.padding.b) = (Size::Fit, rh, pad, pad);
        }
        top = 0.0;
        let ink = |l: Look| Look { text: l.text, font: l.font, ..Look::NONE };
        let column = this.node_keyed(int_key(c.row.box_key, 2), Node { w: Size::Grow, h: Size::Fit, axis: Axis::Column, shrink: 1.0, ..Node::DEFAULT });
        let inside = this.enter(&column);
        match c.label {
            None => {
                let look = ink(look);
                let size = crate::ui_core::font_px(&look.font, this.font_px());
                let width = this.text_width(Some(size), label).max(1.0);
                let line = this.node_keyed(int_key(c.row.box_key, 3), Node { w: Size::Px(width), h: Size::Px((size + 4) as f32), shrink: 1.0, flags: Flags::FIT_TEXT, look, ..Node::DEFAULT });
                this.text(&line, label);
            }
            Some(style) => { toggle_line(this, int_key(c.row.box_key, 3), &style, &ink(look), label); }
        }
        if !c.detail.is_empty() {
            const DETAIL: Label = Label::DEFAULT.wrap();
            let look = ink(this.theme().label.idle);
            if let Some(line) = toggle_line(this, int_key(c.row.box_key, 4), DETAIL.fields(), &look, c.detail) {
                this.set_role(&line, Role::Label, 0);
                this.a11y_name(&line, shown(c.detail));
            }
        }
        this.leave(inside);
    }
    this.end_row(&c.row, side);
    let s = this.part(Part::HIT, Node { w: Size::Px(w), h: Size::Px(h), margin: Edges { t: top, ..Edges::ZERO }, look: track, ..Node::DEFAULT });
    this.text(&s, "");
    this.node_new(&s).state = on;
    let inner = this.enter(&s);
    let d = (h - 6.0).max(1.0);
    let k = this.placed(int_key(c.row.box_key, 1), (if c.on { w - 3.0 - d } else { 3.0 }, 3.0), Size::Px(d), d, knob, Edges::default(), "");
    this.node_new(&k).state = on;
    this.leave(inner);
}

/// One line-box text node of a toggle's column keyed `key`: `text` in `style`, its unset fields `look`'s (lines 1.2 times the size unless
/// it sets them), as wide as the column unless it sets a width; `None` for a style that does not resolve.
fn toggle_line(this: &mut Ui, key: i64, style: &LabelFields, look: &Look, text: &str) -> Option<Handle> {
    let token = style.color.map(|t| this.color(t.0));
    let resolved = resolve_label(style, look, this.font_size(), 0.0, token)?;
    let mut text_style = resolved.style;
    if text_style.line_height <= 0.0 { text_style.line_height = text_style.size * 1.2; }
    let line = this.node_keyed(key, Node { w: style.width.unwrap_or(Size::Grow), shrink: 1.0, look: Look { text: resolved.color, ..Look::NONE }, ..Node::DEFAULT });
    this.text_line_box_caps(&line, text, &text_style, resolved.caps);
    Some(line)
}

/// How `toggle_opts` varies the kit's toggle; `DEFAULT` is the plain `toggle`.
pub struct ToggleOpts {
    /// A disabled toggle takes no press or focus, shows its `disabled` look and reports disabled to accessibility; it still blocks what is under it.
    pub enabled: bool,
    /// The row's height in points (the kit's row height when `None`).
    pub row_h: Option<f32>,
    /// The switch's width and height in points (28 x 14 when `None`); the knob is 6 less than its height.
    pub switch: Option<(f32, f32)>,
    /// The looks of the switch and of its knob, picked by `checked`; `None` is the theme's `switch` and `knob`.
    pub track: Option<States>,
    pub knob: Option<States>,
    /// A secondary line under the label (empty: none).
    pub detail: &'static str,
    /// The label's and the switch's distance from the row's sides in points (the theme's second space step when `None`).
    pub inset: Option<f32>,
    /// A press anywhere on the row toggles it (else only on the switch the body builds as `Part::HIT`) and focuses it, so Enter or
    /// Space then flips it as a row of a list does; a plain toggle is a Tab stop only.
    pub press_row: bool,
    /// The title's text style; `None` is the theme's toggle font.
    pub label: Option<LabelFields>,
}

impl ToggleOpts {
    pub const DEFAULT: ToggleOpts = ToggleOpts { enabled: true, row_h: None, switch: None, track: None, knob: None, detail: "", inset: None, press_row: false, label: None };
}

/// How `button_opts` varies the kit's button; `DEFAULT` is the plain `button`.
pub struct ButtonOpts {
    /// The look per state; `None` is the theme's button (`button_bare` when `bare`). `selected` shows its `checked` look, a disabled button its `disabled` one.
    pub states: Option<States>,
    /// Width and height of the box in points (`Px`) or filling the line (`Grow`); `Fit` is the kit's own (the text and 14 wide, 20 high).
    pub w: Size,
    pub h: Size,
    /// A disabled button takes no press or focus, shows its `disabled` look and reports disabled to accessibility; it still blocks what is under it.
    pub enabled: bool,
    /// The button is the current one of a group: its `checked` look and the accessible SELECTED state.
    pub selected: bool,
    /// Where the box sits in its line: `Start` (5 in), `Center`, or `End` (8 in from the end).
    pub justify: Justify,
    /// Added inside the box; with a `Fit` width it widens the box by its left and right.
    pub padding: Edges,
    /// The theme's error ink (`tokens.invalid`) for the text in every state but disabled (`Button::danger`); a body is told.
    pub danger: bool,
    pub bare: bool,
    pub icon: bool,
    /// The box is exactly `w` by `h` with nothing around it in its line, its text centred in it (`Button::centered`).
    pub centered: bool,
    /// The text drawn through the label line box in this style (`Button::label`); `None` is the kit's text.
    pub label: Option<LabelFields>,
    /// The contents' axis (`Button::axis`); `None` is the kit's.
    pub axis: Option<Axis>,
    /// The space between the contents in points (`Button::gap`).
    pub gap: f32,
    /// The contents' place across the box (`Button::content_align`); `None` is the kit's.
    pub content_align: Option<crate::layout::Along>,
    /// A press focuses the button (`Button::focus_on_click`); else only Tab does.
    pub focus_on_click: bool,
    /// The app's drawing of the leading icon (`Button::icon`), which wins over the glyph `button_opts` is given.
    pub lead_draw: Option<(crate::ui_widgets::IconDraw, [usize; 4])>,
    /// The leading icon's square in points (`Button::icon_size`); 0 is the control-high one.
    pub icon_size: f32,
    /// The text styles of the detail and the accessory (`Button::detail_label`, `Button::accessory_label`); `None` is the theme's label style.
    pub detail_label: Option<LabelFields>,
    pub accessory_label: Option<LabelFields>,
}

impl ButtonOpts {
    pub const DEFAULT: ButtonOpts = ButtonOpts { states: None, w: Size::Fit, h: Size::Fit, enabled: true, selected: false, justify: Justify::Start, padding: Edges::ZERO, danger: false, bare: false, icon: false, centered: false,
        label: None, axis: None, gap: 0.0, content_align: None, focus_on_click: false, lead_draw: None, icon_size: 0.0, detail_label: None, accessory_label: None };
}

/// What happened to a button this frame, as the wrapper's `Response` says it.
#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct Response {
    /// Pointer released over it after a press on it, or Enter/Space while focused.
    pub clicked: bool,
    pub hovered: bool,
    pub held: bool,
    pub focused: bool,
    /// The click came from the keyboard; implies `clicked`.
    pub activated: bool,
    /// A secondary (right-button) press began on it this frame: a context-menu request.
    pub secondary: bool,
    /// The box a press lands on.
    pub handle: Handle,
}

impl Response {
    pub(crate) fn of(signal: &Signal, focused: bool, handle: Handle) -> Response {
        Response { clicked: signal.clicked, hovered: signal.hovered, held: signal.held, focused, activated: signal.clicked && !signal.released,
            secondary: signal.pressed && signal.button == Some(input::MouseButton::RightButton), handle }
    }
}

/// The built-in button: its text on a 20-point box (6 points of padding in a transparent 1-point edge; 4 when `bare`), the glyph
/// centred in the 20-point square of an `icon`, the box 5 points in from the start of its line (8 from the end with `at_end`). A
/// `centered` button's line is its box, its text centred in it. With a leading icon, a detail or an accessory (`button_parts`) the
/// box lays them out as a laid-out button does, `space(0)` apart unless the caller set a gap; a detail under a `Fit`-high text grows
/// the box to fit both lines.
#[inline(always)]
pub fn default_button(this: &mut Ui, c: &ButtonConfig, states: &States, text: &str, parts: [&str; 3]) {
    let given = parts;
    let parts = has_parts(c, given);
    let gap = if parts && c.gap == 0.0 { this.space(0) } else { c.gap };
    if c.centered {
        let line = this.node_new(&c.line);
        (line.w, line.h, line.axis, line.justify) = (c.w, c.h, Axis::Row, c.justify);
        let axis = c.axis.unwrap_or(Axis::Row);
        let (justify, align) = content_place(axis, c.content_align.unwrap_or(crate::layout::Along::Center));
        let row = this.node_new(&c.row);
        (row.w, row.h, row.axis, row.justify, row.align, row.gap) = (c.w, c.h, axis, justify, align, gap);
        if c.pad != Edges::ZERO { row.padding = c.pad; }
        this.style(&c.row, states, c.state);
        if parts { button_parts(this, c, states, text, given); } else if !text.is_empty() { button_text(this, c, states, text); }
        return;
    }
    let laid_out = c.label.is_some() || c.axis.is_some() || c.content_align.is_some() || c.gap != 0.0 || parts;
    if laid_out {
        // the contents laid out by the box: the text on a node of its own (or the caller's contents), the kit's side insets as padding
        let control_h = this.control_h();
        let rh = this.row_height();
        let pad = (rh - control_h) / 2.0;
        // two lines in a kit-high box: the box fits them, at least a control high
        let tall = !given[1].is_empty() && matches!(c.h, Size::Fit);
        let line = this.node_new(&c.line);
        line.h = match c.h { Size::Px(h) => Size::Px(h + 2.0 * pad), Size::Fit if tall => Size::Fit, Size::Fit => Size::Px(rh), h => h };
        (line.axis, line.justify) = (Axis::Row, if c.at_end { Justify::End } else { c.justify });
        line.padding = Edges { l: 5.0, t: pad, r: if c.at_end { 8.0 } else { 5.0 }, b: pad };
        let side = if c.bare { 5.0 } else { 7.0 };
        let axis = c.axis.unwrap_or(Axis::Row);
        let (justify, align) = content_place(axis, c.content_align.unwrap_or(crate::layout::Along::Start));
        let row = this.node_new(&c.row);
        (row.w, row.shrink, row.min_w, row.h) = (c.w, 1.0, 1.0, match c.h { Size::Fit if tall => Size::Fit, Size::Fit => Size::Grow, h => h });
        (row.axis, row.justify, row.align, row.gap) = (axis, justify, align, gap);
        row.padding = Edges { l: c.pad.l + side, t: c.pad.t, r: c.pad.r + side, b: c.pad.b };
        if tall { (row.min_h, row.padding.t, row.padding.b) = (control_h, c.pad.t + pad, c.pad.b + pad); }
        this.style(&c.row, states, c.state);
        if parts { button_parts(this, c, states, text, given); } else if !text.is_empty() { button_text(this, c, states, text); }
        return;
    }
    let measure = this.text_width(None, shown(text));
    let control_h = this.control_h();
    let rh = this.row_height();
    let width = if c.icon { control_h } else { measure + if c.bare { 10.0 } else { 14.0 } };
    let pad = (rh - control_h) / 2.0;
    let line = this.node_new(&c.line);
    line.h = match c.h { Size::Px(h) => Size::Px(h + 2.0 * pad), Size::Fit => Size::Px(rh), h => h };
    line.axis = Axis::Row;
    line.justify = if c.at_end { Justify::End } else { c.justify };
    line.padding = Edges { l: 5.0, t: pad, r: if c.at_end { 8.0 } else { 5.0 }, b: pad };
    let row = this.node_new(&c.row);
    (row.w, row.shrink, row.min_w, row.h) = (match c.w { Size::Fit => Size::Px(width + c.pad.l + c.pad.r), w => w }, 1.0, 1.0, match c.h { Size::Fit => Size::Grow, h => h });
    if c.pad != Edges::ZERO { row.padding = c.pad; }
    this.style(&c.row, states, c.state);
    let pad = if c.icon { ((width - measure) / 2.0).floor() } else if c.bare { 5.0 } else { 7.0 };
    let direction = match (c.icon, shown(text)) {
        (true, "<") => Some(Dir::Left),
        (true, ">") => Some(Dir::Right),
        (true, "v") => Some(Dir::Down),
        (true, "^") => Some(Dir::Up),
        _ => None,
    };
    let n = this.node_new(&c.row);
    n.inset = Edges { l: pad, ..Edges::default() };
    match direction {
        Some(d) => n.mark = Mark::Chevron(d),
        None => this.text(&c.row, shown(text)),
    }
}

/// Where a button's contents sit (`Button::content_align`) as the box's justify and align for its `axis`: `align` across the box,
/// centred down it.
fn content_place(axis: Axis, along: crate::layout::Along) -> (Justify, Align) {
    let across = match along { crate::layout::Along::Start => Justify::Start, crate::layout::Along::Center => Justify::Center, crate::layout::Along::End => Justify::End };
    let align = match along { crate::layout::Along::Start => Align::Start, crate::layout::Along::Center => Align::Center, crate::layout::Along::End => Align::End };
    match axis { Axis::Column => (Justify::Center, align), _ => (across, Align::Center) }
}

/// A laid-out button's text inside `c.row`: on a node of its own as wide as it in whole pixels, or through the label line box in the
/// caller's `Button::label` style (as `Ui::label` draws it; an unset width is the text's, an unset pad none, an unset colour the
/// look's text colour for the state).
fn button_text(this: &mut Ui, c: &ButtonConfig, states: &States, text: &str) {
    // the caller's table is as given (U2b.4.30): the ink a label takes directly resolves here; declared looks resolve on declare
    let mut look = *states.pick(c.state);
    look.text = this.resolve_color(look.text);
    let saved = this.enter(&c.row);
    button_title(this, c, &look, text, Size::Grow);
    this.leave(saved);
}

/// Whether the caller asked for a leading icon, a detail or an accessory (`Button::icon`, `detail`, `accessory`).
fn has_parts(c: &ButtonConfig, [lead, detail, accessory]: [&str; 3]) -> bool { !lead.is_empty() || c.lead_draw.is_some() || !detail.is_empty() || !accessory.is_empty() }

/// A button's parts inside `c.row`, in its order: the leading icon in a control-high square (the glyph centred, or the app's drawing) in
/// the look's text colour, the text (as `button_text`) with the detail under it in a column, then a growing space and the accessory at the
/// end. The detail and the accessory are labels in the caller's style or the theme's label style (an unset width is the text's, an unset
/// pad none).
fn button_parts(this: &mut Ui, c: &ButtonConfig, states: &States, text: &str, [lead, detail, accessory]: [&str; 3]) {
    let mut look = *states.pick(c.state);
    look.text = this.resolve_color(look.text);
    let saved = this.enter(&c.row);
    if !lead.is_empty() || c.lead_draw.is_some() {
        let ch = if c.icon_size > 0.0 { c.icon_size } else { this.control_h() };
        let icon = this.node_keyed(int_key(c.row.box_key, 2), Node { w: Size::Px(ch), h: Size::Px(ch), look: text_only(&look), ..Node::DEFAULT });
        match c.lead_draw {
            Some(draw) => this.icon_draw_in(&icon, draw, look.text),
            None => {
                let inset = ((ch - this.text_width(None, lead)) / 2.0).floor();
                this.node_new(&icon).inset = Edges { l: inset, ..Edges::ZERO };
                this.text(&icon, lead);
            }
        }
    }
    let column = (!detail.is_empty()).then(|| this.node_keyed(int_key(c.row.box_key, 3), Node { w: Size::Fit, h: Size::Fit, axis: Axis::Column, shrink: 1.0, ..Node::DEFAULT }));
    let inside = column.map(|column| this.enter(&column));
    // over a detail the kit's text is a control high (it fills the box's height otherwise)
    let title_h = if column.is_some() { Size::Px(this.control_h()) } else { Size::Grow };
    if !text.is_empty() { button_title(this, c, &look, text, title_h); }
    if !detail.is_empty() { button_part_label(this, detail, c.detail_label); }
    if let Some(inside) = inside { this.leave(inside); }
    if !accessory.is_empty() {
        this.node_keyed(int_key(c.row.box_key, 4), Node { w: Size::Grow, h: Size::Px(1.0), shrink: 1.0, ..Node::DEFAULT });
        button_part_label(this, accessory, c.accessory_label);
    }
    this.leave(saved);
}

/// A detail or an accessory: `text` as a label in `style` (`None`: the theme's), its width the text's and no pad unless the style sets them.
fn button_part_label(this: &mut Ui, text: &str, style: Option<LabelFields>) {
    let mut style = style.unwrap_or(*Label::DEFAULT.fields());
    style.width = style.width.or(Some(Size::Fit));
    style.pad = style.pad.or(Some(0.0));
    this.label_styled(text, &style, false);
}

/// The text of a button in the entered parent: as `button_text` draws it, `look` the button's for its state; `h` is the kit text's height.
fn button_title(this: &mut Ui, c: &ButtonConfig, look: &Look, text: &str, h: Size) {
    let look = *look;
    match c.label {
        Some(mut style) => {
            style.width = style.width.or(Some(Size::Fit));
            style.pad = style.pad.or(Some(0.0));
            if let Some(n) = this.label_styled(text, &style, false).filter(|_| style.color.is_none()) { this.node_mut(&n).look.text = look.text; }
        }
        None => {
            let n = this.node_keyed(int_key(c.row.box_key, 1), Node { w: Size::Fit, h, flags: Flags::FIT_TEXT_PIXEL, look: text_only(&look), ..Node::DEFAULT });
            this.text(&n, shown(text));
        }
    }
}

/// The built-in grip: the line in the separator look (none when the caller placed it), the accent while held, taking presses
/// 3 points either side of it.
pub(crate) fn default_grip(this: &mut Ui, c: &GripConfig) {
    let look = if c.state.pressed { Look { fill: this.theme().tokens.accent, ..Look::NONE } } else if !c.floating { this.theme().separator.idle } else { Look::NONE };
    this.set_look(&c.grip, look, c.state);
    this.hit_outset(&c.grip, if c.vertical { Edges { l: 3.0, r: 3.0, ..Edges::ZERO } } else { Edges { t: 3.0, b: 3.0, ..Edges::ZERO } });
}

/// A label's text as drawn: its style, colour and capitals, with every unset field of the `Label` taken from the theme.
pub(crate) struct LabelText {
    pub style: Style,
    pub color: Rgba,
    pub caps: bool,
}

/// `label` with its unset fields from the theme's label `look` (font, size, weight, tracking, capitals, colour) and `row_height`
/// (the line height when unset; 0 is 1.2 times the size); `token` is its colour token already resolved. `None` for a size that
/// is not positive and finite, a non-finite tracking or line height, or a feature bit no style has.
pub(crate) fn resolve_label(label: &LabelFields, look: &Look, font_size: f32, row_height: f32, token: Option<Rgba>) -> Option<LabelText> {
    use pxui_types::text as T;
    let size = label.size.unwrap_or_else(|| look.font.points(font_size));
    let tracking = label.tracking.unwrap_or(look.font.tracking);
    let line_height = label.line_height.unwrap_or(row_height);
    if !size.is_finite() || size <= 0.0 || !tracking.is_finite() || !line_height.is_finite() || line_height < 0.0 || label.features & !TextStyle::FEATURE_TABULAR_NUMBERS != 0 { return None; }
    let style = Style {
        font: label.font.unwrap_or(look.font.family), size, weight: label.weight.unwrap_or(look.font.weight), italic: false, line_height,
        wrap: match label.wrap { T::Wrap::None => Wrap::None, T::Wrap::Word => Wrap::Word, T::Wrap::Glyph => Wrap::Glyph, T::Wrap::WordSpaces => Wrap::WordSpaces },
        align: match label.align { T::TextAlign::Start => TextAlign::Left, T::TextAlign::Center => TextAlign::Center, T::TextAlign::End => TextAlign::Right },
        max_lines: label.max_lines,
        ellipsis: match label.ellipsis { T::Ellipsis::None => Ellipsis::None, T::Ellipsis::End => Ellipsis::End, T::Ellipsis::Middle => Ellipsis::Middle, T::Ellipsis::Head => Ellipsis::Head },
        tracking: tracking * size,
        features: label.features,
    };
    Some(LabelText { style, color: token.unwrap_or(look.text), caps: label.caps.unwrap_or(look.font.caps) })
}

/// Whether `label` is kit text, one line in the label look's font a row high: nothing set that only line-box text draws (wrap, a
/// line limit, an ellipsis, an alignment, features, a line height, a width, a box height, whole pixels) and a whole-point size.
fn kit_label(label: &LabelFields) -> bool {
    use pxui_types::text as T;
    label.wrap == T::Wrap::None && label.max_lines == 0 && label.ellipsis == T::Ellipsis::None && label.align == T::TextAlign::Start
        && label.features == 0 && label.line_height.is_none() && label.width.is_none() && label.size.is_none_or(|s| s >= 1.0 && s.fract() == 0.0)
        && label.height.is_none() && !label.whole_pixels
}

/// The built-in label, one meaning for every `Label`: what it leaves unset is the theme's label (ink-2 capitals in the label font, a
/// full-width row 12 points in, 4 more above it when it is the first thing of its parent). Kit text when it asks for nothing
/// more (`Label::DEFAULT` is exactly the theme's label); otherwise line-box text as tall as the lines it shows (or `height` with the
/// lines centred in it), or with `digits` one cached character layout at a time, as wide as the characters with digits on the
/// advance of `0`. `whole_pixels` rounds a fitted width up to whole pixels.
pub(crate) fn default_label(this: &mut Ui, c: &LabelConfig, text: &str) {
    build_label(this, c, text, None);
}

fn build_label(this: &mut Ui, c: &LabelConfig, text: &str, resolved: Option<LabelText>) {
    let label = &c.style;
    let look = &this.theme().label.idle;
    let token = label.color.map(|t| this.color(t.0));
    let gap = if c.first && label.pad.is_none() { 4.0 } else { 0.0 };
    let inset = Edges { l: label.pad.unwrap_or_else(|| this.space(2)), t: gap, r: 0.0, b: 0.0 };
    let text = shown(text);
    if !c.digits && kit_label(label) {
        let mut look = *look;
        if let Some(token) = token { look.text = token; }
        if let Some(family) = label.font { look.font.family = family; }
        if let Some(size) = label.size { look.font.size = size as i64; }
        if let Some(weight) = label.weight { look.font.weight = weight; }
        if let Some(tracking) = label.tracking { look.font.tracking = tracking; }
        if let Some(caps) = label.caps { look.font.caps = caps; }
        let h = gap + this.row_height();
        let n = this.node_new(&c.node);
        (n.h, n.look, n.inset) = (Size::Px(h), look, inset);
        this.text(&c.node, text);
        return;
    }
    let Some(mut resolved) = resolved.or_else(|| resolve_label(label, look, this.font_size(), this.row_height(), None)) else { return };
    if let Some(token) = token { resolved.color = token; }
    let mut style = resolved.style;
    if style.line_height <= 0.0 { style.line_height = style.size * 1.2; }
    if !c.digits {
        let n = this.node_new(&c.node);
        (n.w, n.look, n.inset) = (label.width.unwrap_or(Size::Grow), Look { text: resolved.color, ..Look::NONE }, inset);
        if let Some(h) = label.height { n.h = Size::Px(h + gap); }
        if label.whole_pixels { n.flags = n.flags | Flags::FIT_TEXT_PIXEL; }
        this.text_line_box_caps(&c.node, text, &style, resolved.caps);
        return;
    }
    let (zero, line) = { let m = this.text_metrics(&style, 0.0, "0"); (m.w, m.h) };
    let mut width = 0.0;
    let mut utf8 = [0u8; 4];
    for ch in text.chars() {
        let ch = if resolved.caps { ch.to_ascii_uppercase() } else { ch };
        width += if ch.is_ascii_digit() { zero } else { this.text_metrics(&style, 0.0, ch.encode_utf8(&mut utf8)).w };
    }
    if label.whole_pixels { let d = this.density() as f32; width = ((width + inset.l) * d).ceil() / d - inset.l; }
    let height = label.height.unwrap_or(line);
    let n = this.node_new(&c.node);
    (n.w, n.h) = (Size::Px(width + inset.l), Size::Px(height + gap));
    let mut shown = [0u8; LABEL_DIGITS];
    let mut len = text.len().min(LABEL_DIGITS);
    while !text.is_char_boundary(len) { len -= 1; }
    shown[..len].copy_from_slice(&text.as_bytes()[..len]);
    let (color, caps, at) = (resolved.color, resolved.caps, (inset.l, gap + (height - line) / 2.0));
    this.canvas(&c.node, move |paint, rect| {
        if let Ok(text) = core::str::from_utf8(&shown[..len]) { paint.chars((rect.x + at.0, rect.y + at.1), &style, Some(color), caps, text); }
    });
}

/// The bytes of a digits label the built-in body draws; the rest is dropped at a character boundary.
pub(crate) const LABEL_DIGITS: usize = 128;

/// The built-in footer: the hairline under 4 points of space, the hints and the key at the end.
pub(crate) fn default_footer(this: &mut Ui, c: &FooterConfig, right: Option<&str>, hints: &[(&str, &str)]) {
    let row = c.row;
    let rh = this.row_height();
    let t = this.theme();
    let (rule, key, hint, label) = (t.footer.idle, t.key.idle, t.hint.idle, t.label.idle);
    // the hairline 4 points under what is above
    let n = this.node_new(&row);
    (n.h, n.margin, n.look) = (Size::Px(1.0 + rh), Edges { t: 4.0, ..Edges::ZERO }, rule);
    let size = crate::ui_core::font_px(&key.font, this.font_px());
    let mut pen = this.space(2);
    for (i, (k, what)) in hints.iter().enumerate() {
        let kw = this.text_width(Some(size), k);
        this.placed(int_key(row.box_key, 2 * i as i64), (pen, 1.0), Size::Px(kw.max(1.0)), rh, key, Edges::default(), k);
        pen += kw + 8.0;
        let ww = this.text_width(None, what);
        this.placed(int_key(row.box_key, 2 * i as i64 + 1), (pen, 1.0), Size::Px(ww.max(1.0)), rh, hint, Edges::default(), what);
        pen += ww + 8.0;
    }
    if let Some(text) = right {
        let cw = this.cap_width(text);
        this.end_row(&row, this.space(2));
        this.flow_part(int_key(row.box_key, -1), cw, rh, Edges { t: 1.0, ..Edges::ZERO }, label, text);
    }
}

/// The built-in sheet body (panel, window, popup, modal): the content in the sheet as the kit made it.
pub(crate) fn default_panel(this: &mut Ui, _c: &PanelConfig, _label: &str, content: &mut dyn FnMut(&mut Ui)) {
    content(this);
}

/// One pane of the built-in split: it grows by its share of the room and holds at its minimum along the split.
pub(crate) fn default_pane(this: &mut Ui, c: &SplitConfig, label: &str, grow: f32, min: f32, content: &mut dyn FnMut(&mut Ui)) {
    let row = c.axis == Axis::Row;
    let pane = this.node_ref(label, &Node { w: Size::Grow, h: Size::Grow, grow, min_w: if row { min } else { 0.0 }, min_h: if row { 0.0 } else { min }, flags: Flags::CLIP, ..Node::DEFAULT });
    this.within(&pane, content);
}

/// The built-in split: the first pane, the grip, the second pane.
pub(crate) fn default_split(this: &mut Ui, c: &SplitConfig, a: &mut dyn FnMut(&mut Ui), b: &mut dyn FnMut(&mut Ui)) {
    default_pane(this, c, "a", c.ratio, c.min_a, a);
    this.grip(format_args!("grip"), c.axis, None);
    default_pane(this, c, "b", 1.0 - c.ratio, c.min_b, b);
}

/// The sheets a panel body is called for.
#[derive(Clone, Copy)]
enum Sheet {
    Panel,
    Window,
    Popup,
    Modal,
}

impl Ui {
    /// A part of a widget: a node at `at` in the current parent, `w` x `h`, with `look` and `text`
    /// inset by `inset`. Keyed by `key` (an `int_key` of its widget's key).
    pub(crate) fn placed(&mut self, key: i64, at: (f32, f32), w: Size, h: f32, look: Look, inset: Edges, text: &str) -> Handle {
        let n = self.node_keyed(key, Node { w, h: Size::Px(h), at: Some(at).into(), look, inset, ..Node::DEFAULT });
        self.text(&n, text);
        n
    }

    /// Text whose top is exactly `at` (a part as high as its text), `w` wide.
    pub(crate) fn text_part(&mut self, key: i64, at: (f32, f32), w: f32, look: Look, text: &str) -> Handle {
        let size = look.font.points(self.font_size());
        self.placed(key, at, Size::Px(w.max(1.0)), size, look, Edges::default(), text)
    }

    /// A flow part `w` x `h` with `look` and `text`, `margin` round it: in a row laid out from the
    /// end (`end_row`), what sits at a row's right edge.
    pub(crate) fn flow_part(&mut self, key: i64, w: f32, h: f32, margin: Edges, look: Look, text: &str) -> Handle {
        let n = self.node_keyed(key, Node { w: Size::Px(w.max(1.0)), h: Size::Px(h), margin, look, ..Node::DEFAULT });
        self.text(&n, text);
        n
    }

    /// The width `b` (declared this frame) will be laid out at, from its ancestors' declarations:
    /// a `Px` width less the padding and margins down to it, through `Grow` widths across columns.
    /// Anything else (a share of a row, `Pct`, `Rel`, `Fit`) is known only after layout: last frame's.
    // Last frame's width when only layout knows it: a width layout changes (a scroll gutter that
    // appears with overflow) settles the frame, so the picker's labels are cut right the same frame.
    pub(crate) fn width_of(&self, b: &UiBox) -> f32 {
        let mut index = b.index;
        let mut less = 0.0;
        while index != NONE && index != 0 {
            let n = *self.node_at(index);
            match n.w {
                Size::Px(w) => return (w - less).max(0.0),
                Size::Grow if self.node_at(self.box_parent(index)).axis != Axis::Row => {}
                _ => break,
            }
            index = self.box_parent(index);
            let p = self.node_at(index);
            less += n.margin.l + n.margin.r + p.padding.l + p.padding.r;
        }
        self.expect(b.box_slot, self.slot_rw(b.box_slot), f32::NAN);
        self.rect(b).w
    }

    /// A control's height (`Theme::control_h`) at the theme's density.
    pub fn control_h(&self) -> f32 {
        let t = self.theme();
        (t.control_h * t.tokens.density).round()
    }

    /// Step `i` of the theme's spacing scale (4, 8, 12, 16 by default) at its density.
    pub fn space(&self, i: usize) -> f32 {
        let t = self.theme();
        (t.tokens.space[i.min(3)] * t.tokens.density).round()
    }

    /// Lay `row`'s flow children out from its end, `end` points in from it.
    pub(crate) fn end_row(&mut self, row: &Handle, end: f32) {
        let n = self.node_mut(row);
        (n.axis, n.justify, n.padding.r) = (Axis::Row, Justify::End, end);
    }

    /// `text` cut with an ellipsis to `width` at `size` points (the ellipsis alone when nothing fits).
    pub(crate) fn fit_text<'a>(&mut self, size: i64, width: f32, text: &'a str) -> Cow<'a, str> {
        let shown = ellipsis(&mut |s| self.text_width(Some(size), s), width, text);
        if shown.is_empty() && !(text.is_empty() && width >= 0.0) { Cow::Borrowed("\u{2026}") } else { shown }
    }


    pub(crate) fn kit_row(&mut self, flags: Option<Flags>, hit: Option<HitFn>, label: &str) -> UiBox {
        let flags = flags.unwrap_or(Flags::CLICKABLE | Flags::FOCUSABLE | Flags::BLOCKING | Flags::TAB_ONLY);
        let mut opts = BoxOpts::default().flags(flags).w(Size::Grow).h(Size::Px(self.row_height()));
        opts.hit = hit;
        self.box_(label, opts)
    }

    /// Where the top of kit text of `size` points goes to sit centred in a row at `y` of height
    /// `h`. Every row, bar and field places its text with this one rule.
    pub fn text_top(&self, size: Option<i64>, y: f32, h: f32) -> f32 {
        text_top_for(self.font_px(), size, y, h)
    }
}

/// A look's text alone (colour, font, opacity): a part that carries a widget's text without its ground.
pub fn text_only(look: &Look) -> Look {
    Look { text: look.text, font: look.font, opacity: look.opacity, ..Look::NONE }
}

pub fn ints(r: Rect) -> IRect {
    (r.x as i64, r.y as i64, r.w as i64, r.h as i64)
}

pub fn floats(r: IRect) -> Rect {
    Rect::new(r.0 as f32, r.1 as f32, r.2 as f32, r.3 as f32)
}

/// Kit rev 3 geometry from a row rectangle: 12 points at each side, a label column of 112 (half of
/// a narrow row), then the 20-point control.
pub const SIDE: i64 = 12;

pub fn label_y(font_size: i64, y: i64, h: i64) -> i64 {
    y + (h - font_size) / 2
}

pub fn value_column(w: i64) -> i64 {
    value_column_in(w - 2 * SIDE)
}

/// The label column of a row whose sides are taken off: `inner` is its width less both.
fn value_column_in(inner: i64) -> i64 {
    112.min(0.max((inner - 8) / 2))
}

pub fn value_control(r: IRect) -> IRect {
    value_control_at(r, SIDE)
}

/// `value_control` with `side` points at each end of the row (`Ui::space(2)`, the theme's density).
pub fn value_control_at(r: IRect, side: i64) -> IRect {
    let (x, y, w, h) = r;
    let cx = x + side + value_column_in(w - 2 * side) + 8;
    (cx, y + 2, 1.max(x + w - side - cx), 1.max(h - 4))
}

/// The width of `value_control_at`'s control as a flow child of a row padded by the side at both
/// ends and laid out from its end: `inner` is the row's width less both sides, so one plain
/// function serves every density.
pub extern "C" fn value_control_w(inner: f32) -> f32 {
    let inner = inner as i64;
    1.max(inner - 8 - value_column_in(inner)) as f32
}

pub fn toggle_control(r: IRect) -> IRect {
    toggle_control_at(r, SIDE)
}

/// `toggle_control` with the switch `side` points from the row's end.
pub fn toggle_control_at(r: IRect, side: i64) -> IRect {
    let (x, y, w, h) = r;
    (x + w - side - 28, y + (h - 14) / 2, 28, 14)
}

pub fn fraction_at(r: IRect, pointer_x: f32) -> f32 {
    let (x, _, w, _) = r;
    0.0f32.max(1.0f32.min((pointer_x - x as f32) / 1.max(w - 1) as f32))
}

pub fn cap_y(paint: &Paint, y: f32, h: f32) -> f32 {
    paint.text_top(Some(8.max(paint.font_size - 2)), y, h)
}

/// The width the kit's CSS gives `text` of `size` points: the face is monospaced at half an em
/// (plus `tracking` a character).
pub fn mono_width(tracking: f32, size: i64, text: &str) -> f32 {
    let count = text.bytes().filter(|b| b & 0xc0 != 0x80).count();
    count as f32 * (size as f32 / 2.0 + tracking)
}

pub struct PopupOpts {
    pub stroke: Option<Rgba>,
    /// the sheet's look (the theme's popup in `stroke`'s colour by default)
    pub look: Option<Look>,
    pub max_height: Option<f32>,
    pub dismiss_initial: bool,
    pub attached: bool,
    /// what the sheet is to a screen reader (a window by default; a menu, a dialog)
    pub role: Role,
    /// How a modal is presented; a bottom or full-screen sheet takes the drag-to-dismiss with `dismiss`'s thresholds
    pub present: Presentation,
    pub dismiss: DismissOpts,
}

impl Default for PopupOpts {
    fn default() -> Self {
        PopupOpts { stroke: None, look: None, max_height: None, dismiss_initial: true, attached: false, role: Role::Window, present: Presentation::Centered, dismiss: DismissOpts::DEFAULT }
    }
}

impl Ui {
    /// A light panel at `(x, y)` (default `(12, 12)`, width 280). `height` fills a fixed pane;
    /// otherwise content sets the height. Rows beyond the height or `max_height` use the shared
    /// elastic scroll. `window` lays inspector rows and heads out as a floating window's.
    /// Keyed by `key` (hashed under the parent's seed), placed and dressed by `overlay` (280 wide by default) and named by its name
    /// (none without).
    pub fn panel(&mut self, key: &str, overlay: &Overlay, f: &mut dyn FnMut(&mut Ui)) { self.panel_root(key, overlay, f); }

    /// `panel`, returning the panel's root.
    pub fn panel_root(&mut self, key: &str, overlay: &Overlay, f: &mut dyn FnMut(&mut Ui)) -> Handle {
        let opts = panel_opts(overlay, 280.0);
        self.sheet(&opts, if opts.window { Sheet::Window } else { Sheet::Panel }, key, Some(overlay.name.unwrap_or("")), None, (Presentation::Centered, DismissOpts::DEFAULT), f)
    }

    /// A sheet of `kind`: its node (named `name`, the label when `None`; given `role` and its states before the body runs, so a
    /// registered body may set another), then the body of that kind around the caller's content.
    /// A bottom or full-screen `present` takes the drag-to-dismiss with `dismiss`'s thresholds and is moved down by the drag.
    #[allow(clippy::too_many_arguments)]
    fn sheet(&mut self, opts: &PanelOpts, kind: Sheet, label: &str, name: Option<&str>, role: Option<(Role, u32)>, style: (Presentation, DismissOpts), f: &mut dyn FnMut(&mut Ui)) -> UiBox {
        let (present, dismiss) = style;
        let (node, previous) = self.panel_enter(opts, label);
        if let Some(name) = name { self.a11y_name(&node, name); }
        if let Some((role, states)) = role { self.set_role(&node, role, states); }
        if present != Presentation::Centered {
            // the engine's drag-to-dismiss on the sheet's node; its offset moves the sheet down (the commit closes it at the next build)
            let drag = self.dismiss(&node, dismiss.fraction.unwrap_or(0.0), dismiss.velocity.unwrap_or(0.0));
            self.node_mut(&node).at = Some((opts.x, opts.y + drag.offset)).into();
        }
        let c = PanelConfig { node, present };
        // a body is given the sheet's name, never its key (a label's part after `##`)
        let text = name.unwrap_or(shown(label));
        match kind {
            Sheet::Panel => in_body!(self, node.box_key, node, panel, default_panel(&c, text, f)),
            Sheet::Window => in_body!(self, node.box_key, node, window, default_panel(&c, text, f)),
            Sheet::Popup => in_body!(self, node.box_key, node, popup, default_panel(&c, text, f)),
            Sheet::Modal => in_body!(self, node.box_key, node, modal, default_panel(&c, text, f)),
        }
        self.panel_leave(previous);
        node
    }

    /// Open a panel: the box and the kit state to give back to `panel_leave`.
    pub fn panel_enter(&mut self, opts: &PanelOpts, label: &str) -> (UiBox, (i64, bool)) {
        let row_height = (self.theme().row_h * self.theme().tokens.density).round() as i64;
        if !(opts.padding.is_finite() && opts.padding >= 0.0) {
            panic!("Ui.panel: padding must be non-negative");
        }
        let width = 180.0f32.max(opts.width);
        let max_h = match opts.max_height {
            Some(height) => height.max(row_height as f32 + 2.0 * opts.padding),
            None => f32::INFINITY,
        };
        let padding = opts.padding;
        let h = match opts.height {
            Some(height) => Size::Px(height),
            None => Size::Fit,
        };
        let t = self.theme();
        // a floating sheet (a menu, a window) is the input white; a docked panel is the ground
        let look = match (opts.look, opts.stroke) {
            (Some(look), _) => look,
            (None, Some(edge)) => Look { border: edge, ..t.popup.idle },
            (None, None) => t.panel.idle,
        };
        let thumb = t.scrollbar.idle;
        let bar = Bar::new(thumb.fill, t.scrollbar_w).radius(thumb.radius[0]).gap(t.scrollbar_gap).margin(padding as f32).whole();
        let panel = self.node_ref(
            label, &Node {
                flags: Flags::SCROLL | Flags::CLIP | Flags::BLOCKING,
                w: Size::Px(width),
                h,
                max_h,
                padding: Edges::all(padding as f32),
                at: Some((opts.x, opts.y)).into(),
                scroll_step: row_height as f32,
                look,
                bar: Some(bar).into(),
                ..Node::DEFAULT
            },
        );
        self.set_role(&panel, if opts.window { Role::Window } else { Role::Panel }, 0);
        self.a11y_name(&panel, shown(label));
        let previous = (self.row_px(), self.kit_window_flag());
        self.set_kit_row_height(row_height);
        self.set_kit_window(opts.window);
        (panel, previous)
    }

    pub fn panel_leave(&mut self, previous: (i64, bool)) {
        self.set_kit_row_height(previous.0);
        self.set_kit_window(previous.1);
    }

    /// A floating panel dismissed by Escape, focus loss, or a press outside its last laid-out
    /// bounds. `height` supplies the first-frame hit area. Build it before the body it shields; it
    /// paints on top and consumes underlying input, including its opening/dismissal frame. An
    /// `attached` popup (a submenu, built after the popup it belongs to) shares that popup's events
    /// and is never dismissed by itself. Returns whether it is open (and `f` ran).
    /// `name` is the sheet's accessible name, given before its body runs (`None`: the label).
    #[allow(clippy::too_many_arguments)]
    pub fn popup_with_related(&mut self, opts: PopupOpts, at: (f32, f32), width: f32, height: f32, label: &str, name: Option<&str>, keep: &[Rect], related: &[i64], f: &mut dyn FnMut(&mut Ui)) -> Option<UiBox> {
        let (x, y) = at;
        let key = key_of(self.current_seed(), label);
        // Build a popup before the body it shields. Previous popups arbitrate in `route`; this also
        // shields a newly opened/dismissed popup's frame. An `attached` popup shields nothing of its
        // own: it shares the modal's events, and only the modal one is dismissed, by a press outside
        // it and every `keep` rectangle.
        self.set_modal_in_frame(true);
        // a popup under the one input was routed to (a sheet stacked over it) leaves that sheet its signals
        if !opts.attached && !self.under_routed_modal(key) {
            self.retain_popup_state(key, related);
        }
        let slot = self.slot_for(key);
        let rect = match slot {
            Some(slot) => self.slot_rect(slot),
            None => Rect::new(x, y, width, height),
        };
        let mut dismissed = false;
        for event in self.frame_events() {
            match event {
                Event::KeyPressed(Key::Escape) | Event::FocusLost => dismissed = true,
                Event::PointerPressed(_, point) => {
                    if (slot.is_some() || opts.dismiss_initial)
                        && !rect.contains(p32(*point))
                        && !keep.iter().any(|r| r.contains(p32(*point)))
                    {
                        dismissed = true;
                    }
                }
                _ => {}
            }
        }
        // a bottom or full-screen sheet closes on the frame its drag-to-dismiss commits, as on Escape
        dismissed |= opts.present != Presentation::Centered && slot.is_some_and(|slot| self.dismiss_committed(slot));
        if dismissed && !opts.attached {
            return None;
        }
        // Popups live at the root wherever they are built, so a pane's hit area never clips them,
        // and are moved last so they paint on top.
        let saved = self.enter_root_parent();
        let index = self.box_count();
        // a window's 1-point edge is part of its box: its content starts inside it
        let mut popts = PanelOpts::default().at(x, y).width(width);
        popts.stroke = opts.stroke;
        popts.look = opts.look;
        popts.max_height = opts.max_height;
        popts.padding = if opts.stroke.is_none() && opts.look.is_none() { 0.0 } else { 1.0 };
        // the sheet's role (a menu, a dialog) is set before its body runs: a registered body's own role stays
        let modal = if opts.role == Role::Dialog { st::MODAL } else { 0 };
        if opts.present == Presentation::Full { popts.height = Some(self.content_rect().h); }
        let root = self.sheet(&popts, if opts.role == Role::Dialog { Sheet::Modal } else { Sheet::Popup }, label, name, Some((opts.role, modal)), (opts.present, opts.dismiss), f);
        self.leave(saved);
        self.push_overlay(index, if opts.role == Role::Dialog { LayerRole::Modal } else { LayerRole::Popup });
        if opts.attached {
            self.push_modal_extra(key);
        } else {
            self.set_modal_key(Some(key));
        }
        Some(root)
    }

    /// A popup keyed by `key` (hashed under the parent's seed), placed and dressed by `overlay` and named by its name (none without).
    /// Returns its root while open (and `f` ran).
    pub fn popup_root(&mut self, key: &str, overlay: &Overlay, f: &mut dyn FnMut(&mut Ui)) -> Option<Handle> {
        let opts = PopupOpts { stroke: overlay.stroke, look: None, max_height: overlay.max_h, dismiss_initial: overlay.dismiss_initial, attached: overlay.attached, ..PopupOpts::default() };
        self.popup_with_related(opts, overlay.at, overlay.w.unwrap_or(0.0), overlay.h.unwrap_or(0.0), key, Some(overlay.name.unwrap_or("")), &[], &[], f)
    }

    /// A modal keyed by `key`, `overlay`'s width (320 by default) and named by its name (none without); a press outside it dismisses
    /// it. Returns its root while open (and `f` ran).
    pub fn modal_root(&mut self, key: &str, overlay: &Overlay, f: &mut dyn FnMut(&mut Ui)) -> Option<Handle> {
        self.modal_named(overlay, key, Some(overlay.name.unwrap_or("")), f)
    }

    /// The one body of `modal` (and of `dialog`): the sheet named `name` (`None`: the label), presented as `overlay.present` says (centred
    /// by default; at the content area's bottom edge; filling it), `overlay.w` wide (320 centred, the area's width otherwise).
    pub(crate) fn modal_named(&mut self, overlay: &Overlay, label: &str, name: Option<&str>, f: &mut dyn FnMut(&mut Ui)) -> Option<UiBox> {
        let present = overlay.present;
        let key = key_of(self.current_seed(), label);
        let slot = self.slot_for(key);
        if let Some(slot) = slot {
            self.remember_modal_height(key, slot);
        }
        let height = self.modal_height(key);
        let content = self.content_rect();
        let (view_w, view_h) = (content.w, content.h);
        let width = overlay.w.unwrap_or(if matches!(present, Presentation::Centered | Presentation::Top) { 320.0 } else { view_w });
        let x = content.x + (0.0f32.max((view_w - width) / 2.0)).round();
        let y = content.y + match present {
            Presentation::Centered => (0.0f32.max((view_h - height) / 2.0)).round(),
            Presentation::Bottom => (0.0f32.max(view_h - height)).round(),
            Presentation::Full => 0.0,
            Presentation::Top => overlay.at.1.max(0.0).round(),
        };
        let look = self.theme().modal.idle;
        let cap = 48.0f32.max(view_h - 32.0);
        let max_height = match present { Presentation::Centered => Some(cap), Presentation::Bottom => Some(overlay.max_h.unwrap_or(cap)), Presentation::Full => None, Presentation::Top => Some(overlay.max_h.unwrap_or(cap).min(48.0f32.max(view_h - (y - content.y) - 16.0))) };
        let opts = PopupOpts { look: Some(look), max_height, dismiss_initial: false, role: Role::Dialog, present, dismiss: overlay.dismiss, ..PopupOpts::default() };
        self.popup_with_related(opts, (x, y), width, height, label, name, &[], &[], f)
    }

    /// A label: `text` in `style` through the label body, identity by its place among its siblings (every label has the one key
    /// the n-th positional label of its scope), role `Label`. `Label::DEFAULT` is the theme's label: a window's title or a
    /// panel's caption, ink-2 capitals 12 points from the left, 4 points above it when it is the first thing of its parent.
    pub fn label(&mut self, text: &str, style: Label) {
        let style = *style.fields();
        assert!(self.label_styled(text, &style, false).is_some(), "Ui.label: invalid style {style:?}");
    }

    /// A keyed label in an explicit rectangle, using rounded line-box baselines.
    pub fn label_at(&mut self, key: &str, rect: Rect, text: &str, label: &LabelFields) -> Option<Handle> {
        let ink = label.color.map(|token| self.color(token.0));
        let LabelText { style, color, .. } = resolve_label(label, &self.theme().label.idle, self.font_size(), self.row_height(), ink)?;
        if style.line_height <= 0.0 || !style.tracking.is_finite() || self.text_mut().font_metrics(style.font).is_none() {
            return None;
        }
        Some(self.positioned_label(key, rect, color, label.color, text, &style))
    }

    /// The text style `label` shapes with, resolved against this `Ui`'s theme label as `label` resolves it (unset fields are the
    /// theme label's; tracking becomes points): what `label` would shape. `None` for a style that does not resolve.
    pub fn resolve(&self, label: &LabelFields) -> Option<TextStyle> {
        let s = resolve_label(label, &self.theme().label.idle, self.font_size(), self.row_height(), None)?.style;
        use pxui_types::text as T;
        Some(TextStyle {
            size: core::mem::size_of::<TextStyle>() as u32, font: s.font, weight: s.weight, size_points: s.size, line_height: s.line_height,
            wrap: match s.wrap { Wrap::None => T::Wrap::None, Wrap::Word => T::Wrap::Word, Wrap::Glyph => T::Wrap::Glyph, Wrap::WordSpaces => T::Wrap::WordSpaces } as u32,
            align: match s.align { TextAlign::Left => T::TextAlign::Start, TextAlign::Center => T::TextAlign::Center, TextAlign::Right => T::TextAlign::End } as u32,
            max_lines: s.max_lines, italic: s.italic as u8,
            ellipsis: match s.ellipsis { Ellipsis::None => T::Ellipsis::None, Ellipsis::End => T::Ellipsis::End, Ellipsis::Middle => T::Ellipsis::Middle, Ellipsis::Head => T::Ellipsis::Head } as u8,
            features: s.features, tracking: s.tracking,
        })
    }

    /// `label`, or with `digits` a number drawn one cached character layout at a time (its accessible value the text). `false`, with
    /// nothing built, when the style does not resolve (`resolve_label`), its padding or width is not finite, or its font is not loaded.
    pub fn label_styled(&mut self, text: &str, style: &LabelFields, digits: bool) -> Option<Handle> {
        // kit text needs only a finite tracking and padding and a loaded font when it names one; anything else resolves in full
        let resolved = if kit_label(style) && !digits {
            if !style.tracking.is_none_or(f32::is_finite) || !style.pad.is_none_or(f32::is_finite) || style.font.is_some_and(|font| self.text_mut().font_metrics(font).is_none()) { return None; }
            None
        } else {
            let width = match style.width { Some(Size::Px(v) | Size::Pct(v)) => v.is_finite(), _ => true };
            if !width || !style.pad.is_none_or(f32::is_finite) || !style.height.is_none_or(|h| h.is_finite() && h >= 0.0) { return None; }
            let resolved = resolve_label(style, &self.theme().label.idle, self.font_size(), self.row_height(), None)?;
            if self.text_mut().font_metrics(resolved.style.font).is_none() { return None; }
            Some(resolved)
        };
        let first = self.last_child_of_parent() == NONE;
        // every label is the n-th positional label of its scope: siblings in a row resolve their duplicates without looking the key up again
        self.require_building();
        let unique = self.unique_key_repeated(positional_key(self.current_seed(), Positional::Label));
        let node = self.node_unique(unique, &Node { role: style.role, ..Node::DEFAULT });
        if digits { self.a11y_value(&node, shown(text), (0, 0)); }
        let config = LabelConfig { node, first, style: *style, digits };
        let body = body_of!(self, label);
        if body.is_none() {
            build_label(self, &config, text, resolved);
        } else {
            in_item!(flat; self, body, node.box_key, node, default_label(&config, text));
        }
        Some(node)
    }

    /// A window's foot: a line-2 hairline under 4 points of space, then the keys in ink-3 each
    /// followed by what it does in ink-2, 8 points apart, and `right` (a key or a count) at the end.
    pub fn footer(&mut self, right: Option<&str>, hints: &[(&str, &str)]) {
        let row = self.node("footer".into(), Node { role: Role::StatusBar, ..Node::DEFAULT });
        in_body!(self, row.box_key, row, footer, default_footer(&FooterConfig { row }, right, hints));
    }

    /// `true` on the frame a press and release both land inside the button.
    pub fn button(&mut self, text: &str) -> bool {
        self.button_opts(text, text, None, &ButtonOpts::DEFAULT, [""; 3], None, None).clicked
    }

    /// A button is its text on a 20-point box (6 points of padding in a transparent 1-point edge):
    /// a fill on hover and press, an accent line inside the bottom edge with the keyboard. The box
    /// starts 5 points in so that its text lines up with the labels at 12. `opts` varies it (see `ButtonOpts`); `key` is hashed
    /// under the parent's seed, `content` replaces the text (which stays the accessible name unless `name` is set). `signal`, when
    /// given, receives the box's signal as it was read for it, without its key events (`key_events_ref`): the C ABI's signal
    /// without a second read.
    #[allow(clippy::too_many_arguments)]
    pub fn button_opts(&mut self, key: &str, text: &str, name: Option<&str>, opts: &ButtonOpts, parts: [&str; 3], content: Option<&mut dyn FnMut(&mut Ui, State)>, signal_out: Option<&mut Signal>) -> Response {
        // the row lays the box out: 5 in from the start (8 from the end at the end), the control
        // centred in the row's height; the box is the button (its key, its hit area)
        // the box's key is resolved first (one key on several buttons, as `button(text)` gives, takes its duplicates), so the line's,
        // derived from it, is unique without a second resolution
        let enabled = opts.enabled && !self.scope_disabled();
        self.require_building();
        let unique = self.unique_key_repeated(key_of(self.current_seed(), key));
        let line = self.node_keyed(int_key(unique.0, 0x6c69), Node::DEFAULT);
        let saved = self.enter(&line);
        let tab_only = if opts.focus_on_click { Flags::NONE } else { Flags::TAB_ONLY };
        let flags = if enabled { Flags::CLICKABLE | Flags::FOCUSABLE | Flags::BLOCKING | tab_only | Flags::FOCUS_MARK } else { Flags::BLOCKING | Flags::TAB_ONLY };
        let row = self.node_unique(unique, &Node { flags, role: Role::Button, ..Node::DEFAULT });
        self.a11y_name(&row, name.unwrap_or(text));
        self.leave(saved);
        let mut signal = self.signal(&row);
        // a disabled button reports no click from any input (a press begun while it was enabled, an assistive technology's press)
        signal.clicked &= enabled;
        let focused = self.focused(&row);
        let state = State { hovered: signal.hovered, pressed: signal.held, focused, disabled: !enabled, checked: opts.selected };
        let mut states = match opts.states { Some(s) => s, None if opts.bare => self.theme().button_bare, None => self.theme().button };
        if let Some(ink) = opts.danger.then(|| self.resolve_color(self.theme().tokens.invalid)) {
            for look in [&mut states.idle, &mut states.hover, &mut states.pressed, &mut states.focus, &mut states.checked] {
                look.text = ink;
            }
        }
        // the caller's contents replace the text and its parts: the leading icon, detail and accessory are not drawn or given to a body
        let (parts, lead_draw) = if content.is_some() { ([""; 3], None) } else { (parts, opts.lead_draw) };
        let config = ButtonConfig { state, bare: opts.bare, icon: opts.icon, at_end: matches!(opts.justify, Justify::End), w: opts.w, h: opts.h, justify: opts.justify, pad: opts.padding, centered: opts.centered, danger: opts.danger, line, row,
            label: opts.label, axis: opts.axis, gap: opts.gap, content_align: opts.content_align, lead_draw, icon_size: opts.icon_size,
            detail_label: opts.detail_label, accessory_label: opts.accessory_label };
        let shown_text = if content.is_some() { "" } else { text };
        in_body!(flat; self, row.box_key, row, button, default_button(&config, &states, shown_text, parts));
        // size, placement and padding went to the body in its config; the accessible states are the engine's
        let n = self.node_new(&row);
        n.state.disabled = !enabled;
        self.set_role(&row, Role::Button, if opts.selected { st::SELECTED } else { 0 });
        if let Some(f) = content {
            let inside = self.enter(&row);
            f(self, state);
            self.leave(inside);
        }
        let response = Response::of(&signal, focused, row);
        if let Some(out) = signal_out { *out = signal; }
        response
    }

    /// A button of a body that builds its own row of buttons (the dialog's): the box keyed `key` is a child of `line`, the entered row,
    /// named `text` and built through the button body as `button_opts` builds it, without contents.
    pub(crate) fn button_in(&mut self, line: &Handle, key: i64, text: &str, opts: &ButtonOpts, parts: [&str; 3]) -> Response {
        let unique = self.unique_key(key);
        let flags = if opts.enabled { Flags::CLICKABLE | Flags::FOCUSABLE | Flags::BLOCKING | Flags::TAB_ONLY | Flags::FOCUS_MARK } else { Flags::BLOCKING | Flags::TAB_ONLY };
        let row = self.node_unique(unique, &Node { flags, role: Role::Button, ..Node::DEFAULT });
        self.a11y_name(&row, text);
        let mut signal = self.signal(&row);
        signal.clicked &= opts.enabled;
        let focused = self.focused(&row);
        let state = State { hovered: signal.hovered, pressed: signal.held, focused, disabled: !opts.enabled, checked: opts.selected };
        let mut states = self.theme().button;
        if let Some(ink) = opts.danger.then(|| self.resolve_color(self.theme().tokens.invalid)) {
            for look in [&mut states.idle, &mut states.hover, &mut states.pressed, &mut states.focus, &mut states.checked] {
                look.text = ink;
            }
        }
        let config = ButtonConfig { state, bare: false, icon: false, at_end: matches!(opts.justify, Justify::End), w: opts.w, h: opts.h, justify: opts.justify, pad: opts.padding, centered: opts.centered, danger: opts.danger, line: *line, row,
            label: opts.label, axis: opts.axis, gap: opts.gap, content_align: opts.content_align, lead_draw: opts.lead_draw, icon_size: opts.icon_size,
            detail_label: opts.detail_label, accessory_label: opts.accessory_label };
        in_body!(flat; self, row.box_key, row, button, default_button(&config, &states, text, parts));
        self.node_mut(&row).state.disabled = !opts.enabled;
        self.set_role(&row, Role::Button, 0);
        Response::of(&signal, focused, row)
    }

    /// A switch row: the label and the 28 x 14 switch at the end (the default body; a theme's `body.toggle` picks another).
    /// Behaviour: the row, a press on the part its body builds as `Part::HIT` (the whole row without one), the value, the role.
    pub fn toggle(&mut self, text: &str, value: bool) -> bool {
        value ^ self.toggle_opts(text, text, value, &ToggleOpts::DEFAULT).clicked
    }

    /// `toggle` keyed by `key` (hashed under the parent's seed, duplicates in order) with the accessible name `text`, varied by `opts`;
    /// reports what happened to the row (a click flips `value`: the caller owns it).
    pub fn toggle_opts(&mut self, key: &str, text: &str, value: bool, opts: &ToggleOpts) -> Response {
        let enabled = opts.enabled && !self.scope_disabled();
        // a whole-row toggle takes the focus on a press, as a row does; the switch alone is a Tab stop, as a button is
        let tab = if opts.press_row { Flags::NONE } else { Flags::TAB_ONLY };
        let flags = if enabled { Flags::CLICKABLE | Flags::FOCUSABLE | Flags::BLOCKING | tab } else { Flags::BLOCKING | tab };
        let row_h = opts.row_h.unwrap_or(0.0);
        let h = if row_h > 0.0 { row_h } else { self.row_height() };
        self.require_building();
        let unique = self.unique_key_repeated(key_of(self.current_seed(), key));
        let row = self.node_unique(unique, &Node { flags, w: Size::Grow, h: Size::Px(h), ..Node::DEFAULT });
        if !opts.press_row { self.hit_part(&row, Part::HIT); }
        let mut signal = self.signal(&row);
        signal.clicked &= enabled;
        let (stored, value) = (value, value ^ signal.clicked);
        self.set_role(&row, Role::Toggle, st::CHECKABLE | if value { st::CHECKED } else { 0 });
        let (switch_w, switch_h) = opts.switch.unwrap_or((0.0, 0.0));
        let config = ToggleConfig { state: State { hovered: enabled && signal.hovered, disabled: !enabled, ..State::default() }, on: value, row, row_h, switch_w, switch_h,
            inset: opts.inset.unwrap_or(0.0), press_row: opts.press_row, detail: opts.detail, label: opts.label, stored, clicked: signal.clicked };
        in_body!(self, row.box_key, row, toggle, default_toggle(&config, opts.track.as_ref(), opts.knob.as_ref(), text));
        if !enabled { self.node_new(&row).state.disabled = true; }
        Response::of(&signal, self.focused(&row), row)
    }

    /// A splitter's grip: a node across `axis` (`Row`: a vertical line between side-by-side panes)
    /// taking presses 3 points either side of it, asking for the resize cursor while hovered or held
    /// and filled with the accent while held. At `rect` (in its parent) when given,
    /// else a 1-point flow child that fills its row or column across.
    pub fn grip(&mut self, label: std::fmt::Arguments, axis: Axis, rect: Option<Rect>) -> (Handle, Signal) {
        let row = axis == Axis::Row;
        let mut opts = BoxOpts::default().flags(Flags::CLICKABLE | Flags::BLOCKING);
        opts = match rect {
            Some(r) => opts.at((r.x, r.y)).w(Size::Px(r.w)).h(Size::Px(r.h)),
            None if row => opts.w(Size::Px(1.0)).h(Size::Grow),
            None => opts.w(Size::Grow).h(Size::Px(1.0)),
        };
        let b = self.box_fmt(label, opts);
        self.set_role(&b, Role::Splitter, if row { st::VERTICAL } else { st::HORIZONTAL });
        let signal = self.signal_owned(&b);
        if signal.hovered || signal.held {
            self.request_cursor(if row { Cursor::HorizontalResize } else { Cursor::VerticalResize });
        }
        let config = GripConfig { state: State { pressed: signal.held, ..State::default() }, vertical: row, floating: rect.is_some(), grip: b };
        in_body!(flat; self, b.box_key, b, grip, default_grip(&config));
        (b, signal)
    }

    /// Two panes side by side (`Row`) or stacked (`Column`) filling the parent, `a` taking `ratio` of the room the grip leaves and `b`
    /// the rest, neither below its minimum. Reports the grip's response (the empty one without a grip), whether the grip moved along
    /// the axis, and the ratio under the pointer whenever the grip is held and on its release frame (the caller stores it).
    #[allow(clippy::too_many_arguments)]
    pub fn split_result(&mut self, label: &str, axis: Axis, ratio: f32, min_a: f32, min_b: f32, a: &mut dyn FnMut(&mut Ui), b: &mut dyn FnMut(&mut Ui)) -> (Response, bool, Option<f32>) {
        let row = axis == Axis::Row;
        let split = self.node_ref(label, &Node { w: Size::Grow, h: Size::Grow, axis, ..Node::DEFAULT });
        let config = SplitConfig { split, axis, ratio: ratio.clamp(0.0, 1.0), min_a, min_b };
        in_body!(self, split.box_key, split, split, default_split(&config, a, b));
        // the grip is the box the body built under the key `Ui::grip` gives "grip" (none: the split cannot be dragged)
        let Some(grip) = self.box_of_key(key_of(split.box_key, "grip")).filter(|g| self.live(g)) else { return (Response::default(), false, None) };
        let signal = self.signal_owned(&grip);
        let response = Response::of(&signal, self.focused(&grip), grip);
        if !(signal.held || signal.released) {
            return (response, false, None);
        }
        let moved = (if row { signal.drag.0 } else { signal.drag.1 }) != 0.0;
        let r = self.rect(&split);
        let (start, len, at) = if row { (r.x, r.w, signal.pointer.0) } else { (r.y, r.h, signal.pointer.1) };
        let room = (len - 1.0).max(1.0);
        let along = (at - start).clamp(min_a, (room - min_b).max(min_a));
        (response, moved, Some(along / room))
    }
}

// Method bodies outside `impl` blocks: each module is its own codegen unit, so the big bodies
// compile in parallel with the rest of the crate (rustc places methods with their type).

/// The engine's panel record of `overlay` for a call whose own width is `width`.
fn panel_opts(overlay: &Overlay, width: f32) -> PanelOpts {
    PanelOpts { stroke: overlay.stroke, look: overlay.look.copied(), window: overlay.window, x: overlay.at.0, y: overlay.at.1, width: overlay.w.unwrap_or(width), height: overlay.h,
        max_height: overlay.max_h, padding: overlay.padding }
}
