//! Widget bodies: an app builds a widget kind's nodes itself. `Ui::register_toggle_body(name, f)` and its siblings (one per kind,
//! generated from `pxui_types::body_kinds!`) register `f`, a plain `fn(&mut Ui, &KindConfig)` (`ToggleConfig`, `MenuRowConfig`, ...: one wrapper per kind, named for the kind), through the one export
//! `pxui_body_register`; the theme's `toggle.body = NAME` line then picks
//! it. The body runs as a callback from the library under the callback panic rule: a panic in it is carried to the call that can run
//! callbacks (the frame) and the body calls after it are skipped. The config is the kind's C record behind its wrapper: borrowed until the body returns,
//! text fields are read with the wrapper's `text_of` (`BodyConfig`), handles are `Handle`s the body may restyle and enter, and slots run the caller's content.
use crate::Handle;
use crate::composite::{defer_callback_panic, deferred};
use crate::{as_text, check, status, Error, Ui};
use core::{ffi::c_void, mem::ManuallyDrop, ptr::NonNull};
use pxui_types::{geom::Rect, theme::{BodyId, BodyKind, Curve, Part, States}, ffi::{PxuiBodyFn, PxuiHintDesc, PxuiSlot, Slice}, Node, PxuiBox, Str};

use pxui_types::ffi::{PxuiToggleConfig, PxuiSliderConfig, PxuiProgressConfig, PxuiButtonConfig, PxuiIconButtonConfig, PxuiCheckConfig, PxuiSpinnerConfig, PxuiBadgeConfig, PxuiLabelConfig, PxuiFooterConfig, PxuiGripConfig, PxuiPanelConfig, PxuiDialogConfig, PxuiSectionConfig, PxuiInspectorRowConfig, PxuiSplitConfig, PxuiScrollConfig, PxuiToastConfig, PxuiTipConfig, PxuiTabConfig, PxuiSegmentConfig, PxuiDropdownConfig, PxuiMenuItemConfig, PxuiMenuRowConfig, PxuiRowConfig, PxuiTreeRowConfig, PxuiTableHeadConfig, PxuiPickerRowConfig, PxuiSearchFieldConfig, PxuiTextFieldConfig, PxuiLineFieldConfig, PxuiValueFieldConfig, PxuiPickerFieldConfig, PxuiSectionIndexConfig};

mod sealed {
    pub trait Sealed { fn bounds(&self) -> (usize, usize); }
}

/// What a registered Rust body reads a config through: the config wrapper (`ToggleConfig`, `MenuRowConfig`, ... one per C record,
/// `Deref` to its `Pxui*Config` record for the plain fields) and the accessors for its pointer fields. A wrapper cannot be built by
/// safe code and its record cannot be changed through it, and an accessor reads only a field that lies inside the wrapper's own
/// record (a copy of a field elsewhere reads as empty), so a body needs no `unsafe` and safe code cannot point one at memory.
///
/// ```
/// use pxui::bodies::{BodyConfig, ToggleConfig};
/// fn toggle(_ui: &mut pxui::Ui, config: &ToggleConfig) { let _label: &str = config.text_of(&config.label); }
/// let _register: fn(&mut pxui::Ui, &str) -> Result<_, pxui::Error> = |ui, name| ui.register_toggle_body(name, toggle);
/// ```
pub trait BodyConfig: sealed::Sealed {
    /// The text of a config field (`config.text_of(&config.label)`), borrowed from the config (valid until the body returns).
    ///
    /// Allocates: never.
    /// Panics: never; a field that is not part of this config, or text that is not UTF-8, reads as "".
    ///
    /// ```
    /// use pxui::bodies::BodyConfig;
    /// fn label(_ui: &mut pxui::Ui, config: &pxui::bodies::LabelConfig) { let _ = config.text_of(&config.text); }
    /// ```
    fn text_of(&self, field: &Str) -> &str {
        if !inside(self.bounds(), field as *const _ as usize, core::mem::size_of_val(field)) { return ""; }
        str_of(field)
    }

    /// Item `index` of a config's text list (a dialog body's `buttons`, a tooltip body's `lines`), borrowed from the config (valid until
    /// the body returns); "" past the end, for text that is not UTF-8 and for a list that is not part of this config.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::bodies::{BodyConfig, DialogConfig};
    /// fn dialog(_ui: &mut pxui::Ui, config: &DialogConfig) { let _ = config.text_at(&config.buttons, 1); }
    /// ```
    fn text_at(&self, list: &Slice<Str>, index: usize) -> &str {
        if !inside(self.bounds(), list as *const _ as usize, core::mem::size_of_val(list)) || index >= list.len || list.ptr.is_null() { return ""; }
        str_of(unsafe { &*list.ptr.add(index) })
    }

    /// Item `index` of a footer body's `hints` as (key, value) text, borrowed from the config; `None` past the end or for a list that is
    /// not part of this config.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::bodies::{BodyConfig, FooterConfig};
    /// fn footer(_ui: &mut pxui::Ui, config: &FooterConfig) { let _ = config.hint_at(&config.hints, 0); }
    /// ```
    fn hint_at(&self, list: &Slice<PxuiHintDesc>, index: usize) -> Option<(&str, &str)> {
        if !inside(self.bounds(), list as *const _ as usize, core::mem::size_of_val(list)) || index >= list.len || list.ptr.is_null() { return None; }
        let hint = unsafe { &*list.ptr.add(index) };
        Some((str_of(&hint.key), str_of(&hint.value)))
    }

    /// The looks per state a config field points at (`states` of a button, icon button or segmented config, `track` and `knob` of a
    /// toggle config: the caller's or the theme's), borrowed from the config; `None` for a null field or one that is not part of this config.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::bodies::{BodyConfig, ButtonConfig};
    /// fn button(_ui: &mut pxui::Ui, config: &ButtonConfig) { let _ = config.states_of(&config.states); }
    /// ```
    fn states_of(&self, field: &*const States) -> Option<&States> {
        if !inside(self.bounds(), field as *const _ as usize, core::mem::size_of_val(field)) || field.is_null() { return None; }
        Some(unsafe { &**field })
    }

    /// The press curves an icon button config's `press` points at (the scale's in on press and out on release), borrowed from the
    /// config; `None` when it is null (the kit's springs) or not part of this config.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::bodies::{BodyConfig, IconButtonConfig};
    /// fn icon_button(_ui: &mut pxui::Ui, config: &IconButtonConfig) { let _ = config.curves_of(&config.press); }
    /// ```
    fn curves_of(&self, field: &*const Curve) -> Option<&[Curve; 2]> {
        if !inside(self.bounds(), field as *const _ as usize, core::mem::size_of_val(field)) || field.is_null() { return None; }
        Some(unsafe { &*field.cast::<[Curve; 2]>() })
    }
}

/// The text a library-written `Str` names; "" when empty or not UTF-8. Callers vouch that `field` came from the config's records or lists.
fn str_of<'a>(field: &Str) -> &'a str {
    if field.len == 0 || field.ptr.is_null() { return ""; }
    core::str::from_utf8(unsafe { core::slice::from_raw_parts(field.ptr, field.len) }).unwrap_or("")
}

/// Whether the `size` bytes at address `at` lie wholly inside the record `bounds` (start, length) names.
fn inside((start, len): (usize, usize), at: usize, size: usize) -> bool { at >= start && at + size <= start + len }

library!(sys {
    fn pxui_body_register(ui: *mut PxuiUi, kind: u32, name: Str, body: PxuiBodyFn, user: *mut c_void, out: *mut u16) -> i32;
    fn pxui_part(ui: *mut PxuiUi, part: u32, node: *const Node, out: *mut PxuiBox) -> i32;
    fn pxui_part_box(ui: *mut PxuiUi, owner: i64, part: u32, out: *mut PxuiBox) -> i32;
    #[trusted] fn pxui_slot(ui: *mut PxuiUi, slot: PxuiSlot, out: *mut u8) -> i32;
});

/// What every registered body's C entry does: skip once a panic is pending, otherwise run `body` on the borrowed `Ui` under `catch_unwind`.
fn enter(raw: *mut c_void, config: *const c_void, body: &mut dyn FnMut(&mut Ui)) {
    if raw.is_null() || config.is_null() || deferred() { return; }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut ui = ManuallyDrop::new(Ui(NonNull::new(raw.cast()).unwrap()));
        body(&mut ui);
    }));
    if let Err(panic) = result { defer_callback_panic(panic); }
}

macro_rules! typed {
    ($($kind:ident $register:ident $body:ident $config:ident $wrapper:ident),* $(,)?) => {
        $(
            #[doc = concat!("The `", stringify!($config), "` record a registered `", stringify!($kind), "` body receives (borrowed until the body returns): its plain fields read through `Deref`, its text, lists and looks through `BodyConfig`.")]
            #[repr(transparent)]
            pub struct $wrapper($config);
            impl core::ops::Deref for $wrapper { type Target = $config; fn deref(&self) -> &$config { &self.0 } }
            impl sealed::Sealed for $wrapper { fn bounds(&self) -> (usize, usize) { (self as *const Self as usize, core::mem::size_of::<$config>()) } }
            impl BodyConfig for $wrapper {}
        )*
        impl Ui {
            $(
                #[doc = concat!(
                    "Registers a ", stringify!($kind), " body for use in theme files.\n\n",
                    "Call between frames. A name is local to this Ui and kind. Registering it again replaces the function and keeps its id. ",
                    "Each kind accepts up to 65,535 names. An invalid name returns `Error::InvalidInput`. ",
                    "A registration during a frame or beyond that capacity returns `Error::State`. ",
                    "`Ui::last_error` gives the reason.\n",
                    "Allocates: when the registry grows.\nPanics: never.\n\n",
                    "```\nlet mut ui = pxui::doc::frames(1, &mut |_| {});\n",
                    "ui.", stringify!($register), "(\"doc-body\", |_, _| {}).unwrap();\n```"
                )]
                pub fn $register(&mut self, name: &str, body: fn(&mut Ui, &$wrapper)) -> Result<BodyId, Error> {
                    extern "C" fn entry(user: *mut c_void, raw: *mut c_void, config: *const c_void) {
                        enter(raw, config, &mut |ui| (unsafe { core::mem::transmute::<*mut c_void, fn(&mut Ui, &$wrapper)>(user) })(ui, unsafe { &*config.cast::<$wrapper>() }));
                    }
                    self.register_body(BodyKind::$kind, name, entry, body as *mut c_void)
                }
            )*
        }
    };
}
pxui_types::body_kinds!(typed);

impl Ui {
    fn register_body(&mut self, kind: BodyKind, name: &str, entry: extern "C" fn(*mut c_void, *mut c_void, *const c_void), user: *mut c_void) -> Result<BodyId, Error> {
        let mut id = 0;
        status(unsafe { sys::pxui_body_register(self.0.as_ptr(), kind as u32, as_text(name), Some(entry), user, &mut id) })?;
        Ok(BodyId(id))
    }

    /// Builds a node as `part` of the widget whose body is running: behaviour finds it again by role (see the config's documentation
    /// for the parts its widget reads). Only inside a body, or, for `Part::HANDLE`, inside a reorderable list's row (`SectionList::reorder`,
    /// `SelectionList::reorder`).
    #[track_caller]
    // Inline: out of line, the 600-byte node is copied once more per part (abi_bench Lists page, static release).
    #[inline]
    pub fn part(&mut self, part: Part, node: Node) -> Handle {
        let mut out = Handle::default();
        #[cfg(feature = "static")]
        check(self, unsafe { pxui_abi_link::trusted::part(self.0.as_ptr(), part.0, &node, &mut out) });
        #[cfg(not(feature = "static"))]
        check(self, unsafe { sys::pxui_part(self.0.as_ptr(), part.0, &node, &mut out) });
        out
    }

    /// Returns a named part of a widget from its last build.
    ///
    /// A stale owner or missing part returns None.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let node = ui.node("owner", pxui::Node::DEFAULT);
    ///     assert!(ui.part_box(&node, pxui::Part::HIT).is_none());
    /// });
    /// ```
    pub fn part_box(&self, owner: &Handle, part: Part) -> Option<Handle> {
        let live = self.box_of_resolved_key(owner.box_key)?;
        if (live.box_slot, live.generation) != (owner.box_slot, owner.generation) { return None; }
        let mut out = Handle::default();
        (unsafe { sys::pxui_part_box(self.0.as_ptr(), owner.box_key, part.0, &mut out) } == 0).then_some(out)
    }

    /// Returns the part's surface rectangle from the last layout.
    ///
    /// A missing part, stale owner or part without a positive laid-out size returns None.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let node = ui.node("owner", pxui::Node::DEFAULT);
    ///     assert!(ui.part_rect(&node, pxui::Part::HIT).is_none());
    /// });
    /// ```
    pub fn part_rect(&self, owner: &Handle, part: Part) -> Option<Rect> {
        self.part_box(owner, part).map(|b| self.rect(&b)).filter(|r| r.w > 0.0 && r.h > 0.0)
    }

    /// Runs a config's slot (the caller's content, built where the body calls this; a button or icon button config's `built_in`, the
    /// kit's own body for that config, so a registered body draws the kit's widget for configs it does not style); for a search field's editor slot
    /// the answer is whether the query is not empty, otherwise false. A slot is valid only during the body call that received it, and not inside its own content or a nested body: else this panics.
    #[track_caller]
    pub fn slot(&mut self, slot: PxuiSlot) -> bool {
        let mut out = 0u8;
        check(self, unsafe { sys::pxui_slot(self.0.as_ptr(), slot, &mut out) });
        out != 0
    }
}

impl ButtonConfig {
    /// The caller's leading icon (`Button::icon`) as the `Icon` it gave: the glyph, or the app's drawing with its word, so a
    /// registered body draws the app's icon by id (D18); `None` without one, or for a drawing a non-Rust caller gave through the C ABI.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::bodies::ButtonConfig;
    /// fn button(_ui: &mut pxui::Ui, config: &ButtonConfig) { if let Some(pxui::Icon::Draw(_, word)) = config.icon() { let _ = word; } }
    /// ```
    pub fn icon(&self) -> Option<crate::Icon> {
        icon_of(self.text_of(&self.lead), self.lead_draw, self.lead_user, self.lead_word)
    }

    /// The contents' axis the caller asked for (`Button::axis`); `None` for the kit's (or a value no `Axis` names).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::bodies::ButtonConfig;
    /// fn button(_ui: &mut pxui::Ui, config: &ButtonConfig) { let _column = config.content_axis() == Some(pxui::ui::Axis::Column); }
    /// ```
    pub fn content_axis(&self) -> Option<pxui_types::Axis> {
        match self.axis { 1 => Some(pxui_types::Axis::Row), 2 => Some(pxui_types::Axis::Column), 3 => Some(pxui_types::Axis::Stack), _ => None }
    }

    /// The width the caller asked for (`Button::size`): `Size::Px` points or `Size::Grow`; `None` for the kit's.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::bodies::ButtonConfig;
    /// fn button(_ui: &mut pxui::Ui, config: &ButtonConfig) { let _w = config.width().unwrap_or(pxui::Size::Fit); }
    /// ```
    pub fn width(&self) -> Option<pxui_types::Size> { size_of_kind(self.w_kind, self.w) }

    /// The height the caller asked for (`Button::size`): `Size::Px` points or `Size::Grow`; `None` for the kit's.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::bodies::ButtonConfig;
    /// fn button(_ui: &mut pxui::Ui, config: &ButtonConfig) { let _h = config.height().unwrap_or(pxui::Size::Fit); }
    /// ```
    pub fn height(&self) -> Option<pxui_types::Size> { size_of_kind(self.h_kind, self.h) }

    /// The caller's text style (`Button::label`) as the `Label` it gave, so a registered body draws the text with `Ui::label` in it;
    /// `None` without one (the body picks its own) or for a record no `Label` spells (a C caller's out-of-range value).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::bodies::{BodyConfig, ButtonConfig};
    /// fn button(ui: &mut pxui::Ui, config: &ButtonConfig) { if let Some(style) = config.label() { ui.label(config.text_of(&config.label), style); } }
    /// ```
    pub fn label(&self) -> Option<crate::Label> { label_of(self.has_label, &self.text_style, &self.text_box) }

    /// The caller's detail style (`Button::detail_label`) as the `Label` it gave; `None` without one (the theme's label style) or for a
    /// record no `Label` spells.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::bodies::{BodyConfig, ButtonConfig};
    /// fn button(ui: &mut pxui::Ui, config: &ButtonConfig) { ui.label(config.text_of(&config.detail), config.detail_label().unwrap_or(pxui::Label::DEFAULT)); }
    /// ```
    pub fn detail_label(&self) -> Option<crate::Label> { label_of(self.has_detail_style, &self.detail_style, &self.detail_box) }

    /// The caller's accessory style (`Button::accessory_label`) as the `Label` it gave; `None` without one (the theme's label style) or
    /// for a record no `Label` spells.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::bodies::{BodyConfig, ButtonConfig};
    /// fn button(ui: &mut pxui::Ui, config: &ButtonConfig) { ui.label(config.text_of(&config.accessory), config.accessory_label().unwrap_or(pxui::Label::DEFAULT)); }
    /// ```
    pub fn accessory_label(&self) -> Option<crate::Label> { label_of(self.has_accessory_style, &self.accessory_style, &self.accessory_box) }
}

/// The `Label` a body config's style and box records spell (built with the `Label` builders, field by field); `None` when `has` is
/// 0 or a record holds a number no label has.
fn label_of(has: u8, s: &pxui_types::ffi::PxuiLabelStyle, b: &pxui_types::ffi::PxuiLabelBox) -> Option<crate::Label> {
    use pxui_types::{a11y::Role, ffi::{PxuiLabelBox as B, PxuiLabelStyle as S, PxuiSizeDesc as D, PxuiTextStyle}, text::TextAlign, Size};
    if has == 0 { return None; }
    let on = |bit: u32| s.set & bit != 0;
    let mut l = crate::Label::DEFAULT;
    if on(S::SET_FONT) { l = l.font(s.font); }
    if on(S::SET_SIZE) { l = l.size(s.size_points); }
    if on(S::SET_WEIGHT) { l = l.weight(s.weight); }
    if on(S::SET_TRACKING) { l = l.tracking(s.tracking); }
    if on(S::SET_CAPS) { l = l.caps(s.caps != 0); }
    if on(S::SET_COLOR) { l = l.color(pxui_types::theme::ColorToken(s.color)); }
    if on(S::SET_WIDTH) {
        let v = s.width.value;
        l = l.w(match s.width.kind { D::PX => Size::Px(v), D::PCT => Size::Pct(v), D::REL => Size::Rel(s.width.relative?), D::GROW => Size::Grow, D::FIT => Size::Fit, _ => return None });
    }
    if on(S::SET_LINE_HEIGHT) { l = l.line_height(s.line_height); }
    if on(S::SET_PAD) { l = l.pad(s.pad); }
    l = match s.wrap { 0 => l, 1 => l.wrap(), 2 => l.wrap_glyph(), 3 => l.wrap_spaces(), _ => return None };
    l = match s.ellipsis { 0 => l, 1 => l.ellipsis(), 2 => l.ellipsis_middle(), 3 => l.ellipsis_head(), _ => return None };
    l = l.lines(s.max_lines).align(match s.align { 0 => TextAlign::Start, 1 => TextAlign::Center, 2 => TextAlign::End, _ => return None });
    if s.features & PxuiTextStyle::FEATURE_TABULAR_NUMBERS != 0 { l = l.tabular(); }
    if b.set & B::SET_HEIGHT != 0 { l = l.h_px(b.height); }
    if b.set & B::SET_ROLE != 0 {
        if b.role > Role::Separator as u8 { return None; }
        // checked against the last role above; `Role` is `repr(u8)` and its numbers are dense
        l = l.role(unsafe { core::mem::transmute::<u8, Role>(b.role) });
    }
    if b.whole_pixels != 0 { l = l.whole_pixels(); }
    Some(l)
}

impl IconButtonConfig {
    /// The icon the caller gave (`Ui::icon_button`'s `Icon`): the glyph, or the app's drawing with its word, so a registered body
    /// draws the app's icon by id (D18); `None` for an empty glyph or a drawing a non-Rust caller gave through the C ABI.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::bodies::IconButtonConfig;
    /// fn icon_button(_ui: &mut pxui::Ui, config: &IconButtonConfig) { let _ = config.icon(); }
    /// ```
    pub fn icon(&self) -> Option<crate::Icon> {
        icon_of(self.text_of(&self.icon), self.draw, self.draw_user, self.draw_word)
    }
}

/// A button config's size kind (`w_kind`, `h_kind`: 1 points, 2 fill) as the `Size` it stands for; `None` for the kit's.
fn size_of_kind(kind: u8, points: f32) -> Option<pxui_types::Size> {
    match kind { 1 => Some(pxui_types::Size::Px(points)), 2 => Some(pxui_types::Size::Grow), _ => None }
}

impl ModalConfig {
    /// How the modal sheet is presented (`Overlay::present`), so a body dresses it by its place; a value no `Presentation` names reads
    /// as `Centered`, as the record's rule says.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::bodies::ModalConfig;
    /// fn modal(ui: &mut pxui::Ui, config: &ModalConfig) { if config.presentation() == pxui::Presentation::Bottom { ui.slot(config.content); } }
    /// ```
    pub fn presentation(&self) -> pxui_types::ui::Presentation {
        use pxui_types::ui::Presentation;
        match self.present { 1 => Presentation::Bottom, 2 => Presentation::Full, 3 => Presentation::Top, _ => Presentation::Centered }
    }
}

impl PickerConfig {
    /// The row's image (`Picker::image`) as the descriptor the picker's image callback gave; `None` when it gave the row none.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::bodies::PickerConfig;
    /// fn row(_ui: &mut pxui::Ui, config: &PickerConfig) { if let Some(image) = config.image() { let _ = image.width; } }
    /// ```
    pub fn image(&self) -> Option<crate::Image> { (self.has_image != 0).then_some(self.image) }

    /// The heading box of the group the row starts (`PickRow::heading`), which the body lays out and styles; `None` for a row that
    /// starts no group.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::bodies::PickerConfig;
    /// fn row(ui: &mut pxui::Ui, config: &PickerConfig) { if let Some(head) = config.head() { ui.set_node_layout(&head, &pxui::Node::row()); } }
    /// ```
    pub fn head(&self) -> Option<Handle> { (self.has_head != 0).then_some(self.head) }

    /// The row's artwork or icon slot (`PickRow::art`) as the `PickArt` the fill gave: a glyph, an image id or the app's icon word, so a
    /// registered row body draws the app's icon by its word (`PickArt::Draw`); `None` for a row without one.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::bodies::PickerConfig;
    /// fn row(_ui: &mut pxui::Ui, config: &PickerConfig) { if let Some(pxui::PickArt::Draw(word)) = config.art() { let _ = word; } }
    /// ```
    pub fn art(&self) -> Option<crate::PickArt> {
        match self.art_kind {
            1 => char::from_u32(u32::try_from(self.art).ok()?).map(crate::PickArt::Glyph),
            2 => usize::try_from(self.art).ok().map(crate::PickArt::Image),
            3 => u32::try_from(self.art).ok().map(crate::PickArt::Draw),
            _ => None,
        }
    }
}

impl TableHeadConfig {
    /// The heading's sort direction: `Some(true)` ascending, `Some(false)` descending, `None` when the table is not sorted by this
    /// column (the record's `has_ascending`/`ascending`).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::bodies::TableHeadConfig;
    /// fn head(_ui: &mut pxui::Ui, config: &TableHeadConfig) { let _mark = match config.ascending() { Some(true) => "▲", Some(false) => "▼", None => "" }; }
    /// ```
    pub fn ascending(&self) -> Option<bool> { (self.has_ascending != 0).then_some(self.ascending != 0) }
}

impl SegmentedConfig {
    /// The chosen option's index (`has_selected`/`selected`), after this frame's press or key; `None` when no option is chosen.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::bodies::SegmentedConfig;
    /// fn segment(_ui: &mut pxui::Ui, config: &SegmentedConfig) { let _chosen = config.selected() == Some(config.index); }
    /// ```
    pub fn selected(&self) -> Option<usize> { (self.has_selected != 0).then_some(self.selected) }

    /// The choice the caller passed this frame (`has_stored`/`stored`), before a press or key moved `selected`, as a toggle's stored
    /// value; `None` when the caller passed none.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::bodies::SegmentedConfig;
    /// fn segment(_ui: &mut pxui::Ui, config: &SegmentedConfig) { let _shown = config.stored().or(config.selected()) == Some(config.index); }
    /// ```
    pub fn stored(&self) -> Option<usize> { (self.has_stored != 0).then_some(self.stored) }
}

impl ProgressConfig {
    /// The progress fraction the caller gave (`has_value`/`value`; the body clamps it to 0..=1); `None` for an indeterminate track.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::bodies::ProgressConfig;
    /// fn progress(_ui: &mut pxui::Ui, config: &ProgressConfig) { let _fraction = config.value().map_or(0.0, |v| v.clamp(0.0, 1.0)); }
    /// ```
    pub fn value(&self) -> Option<f32> { (self.has_value != 0).then_some(self.value) }
}

/// The `Icon` a config's glyph text or drawing pointers spell. Only a drawing whose callback is this wrapper's own `Icon::Draw`
/// trampoline (`paint::icon_draw`) carries a Rust `fn` as its user pointer; any other callback (a C caller's) has an opaque user
/// pointer that must never become callable, so it reads as no icon.
fn icon_of(glyph: &str, draw: pxui_types::ffi::PxuiIconDraw, user: *mut c_void, word: u32) -> Option<crate::Icon> {
    if let Some(ch) = glyph.chars().next() { return Some(crate::Icon::Glyph(ch)); }
    let ours = crate::paint::icon_draw as extern "C" fn(*mut c_void, *mut c_void, Rect, pxui_types::geom::Rgba, *const pxui_types::ffi::PxuiPaintInfo);
    #[allow(unpredictable_function_pointer_comparisons)] // same crate, same item: the wrapper wrote this exact pointer
    let is_ours = draw.is_some_and(|f| f == ours);
    if !is_ours || user.is_null() { return None; }
    let f: fn(&mut crate::Paint<'_>, Rect, pxui_types::geom::Rgba, u32) = unsafe { core::mem::transmute(user) };
    Some(crate::Icon::Draw(f, word))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ThemeHandle;
    use core::cell::{Cell, RefCell};
    use crate::FrameIn;
    use pxui_types::{Size};
    use std::panic::{catch_unwind, AssertUnwindSafe};

    /// U2b.7 (D18): a registered button and icon button body gets the app's icon with its word (to draw it by id) and the button's
    /// `icon_size`; a search field body gets the field's variant, and the focused field's keyboard takes its return key. Both link modes.
    #[test]
    fn bodies_get_the_app_icon_its_size_and_the_search_variant() {
        thread_local! { static GOT: Cell<(u32, u32, f32, u16)> = const { Cell::new((0, 0, 0.0, 0)) }; }
        fn draw(_: &mut crate::Paint<'_>, _: Rect, _: pxui_types::geom::Rgba, _: u32) {}
        fn button(_: &mut Ui, c: &ButtonConfig) { if let Some(crate::Icon::Draw(_, w)) = c.icon() { GOT.with(|g| g.set((w, g.get().1, c.icon_size, g.get().3))); } }
        fn icon_button(_: &mut Ui, c: &IconButtonConfig) { if let Some(crate::Icon::Draw(_, w)) = c.icon() { GOT.with(|g| g.set((g.get().0, w, g.get().2, g.get().3))); } }
        fn search(ui: &mut Ui, c: &SearchFieldConfig) { GOT.with(|g| g.set((g.get().0, g.get().1, g.get().2, c.variant))); ui.slot(c.editor); }
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
        ui.register_button_body("b", button).unwrap();
        ui.register_icon_button_body("ib", icon_button).unwrap();
        ui.register_search_field_body("s", search).unwrap();
        ui.set_theme(&crate::ThemeHandle::parse("button.body = b\nicon_button.body = ib\nsearch_field.body = s").unwrap()).unwrap();
        let mut build = |ui: &mut Ui| {
            ui.button("add", "Add", &crate::Button::DEFAULT.icon(crate::Icon::Draw(draw, 7)).icon_size(24.0));
            ui.icon_button("nine", crate::Icon::Draw(draw, 9), "Nine", &crate::IconButton::DEFAULT);
            ui.search_field("find", "q", &crate::SearchField::DEFAULT.name("Find").variant(2).action(crate::ReturnKey::Done));
        };
        for _ in 0..2 { crate::testing::frame(&mut ui, &[], &mut build); }
        assert_eq!(GOT.with(|g| g.get()), (7, 9, 24.0, 2), "the bodies' icons, icon size and search variant");
        let mut build = |ui: &mut Ui| { ui.search_field("find", "q", &crate::SearchField::DEFAULT.name("Find").action(crate::ReturnKey::Done)); };
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
        crate::testing::frame(&mut ui, &[], &mut build);
        crate::testing::key(&mut ui, crate::KeyCode::Tab, &mut build);
        crate::testing::frame(&mut ui, &[], &mut build);
        assert_eq!(ui.text_traits(), pxui_types::TextTraits::SEARCH.action(crate::ReturnKey::Done), "the focused search field's keyboard");
        // a focused search field holding a query paints it from the frame's edit buffer: warm frames allocate nothing
        mem_sys::set_peer(crate::library_counter);
        let scope = mem_sys::warm_scope();
        for round in 0..4 {
            mem_sys::local_reset();
            crate::testing::frame(&mut ui, &[], &mut build);
            assert_eq!(mem_sys::local_alloc_count(), 0, "a warm focused search field frame allocated (round {round})");
        }
        drop(scope);
        assert!(mem_sys::take_reports().is_empty());
    }

    /// U4a.2: `SearchField::action(ReturnKey::Done)` ends editing on Return in that frame, as `TextField::action` does; the Search key
    /// keeps focus. A warm click, Done, Return cycle allocates nothing, the kit's editor or a body's (before, every key press copied the
    /// frame's key events twice). Both link modes.
    #[test]
    fn a_search_field_with_done_ends_editing_on_return() {
        for (traits, kept) in [(crate::SearchField::DEFAULT.name("Find").action(crate::ReturnKey::Done), false), (crate::SearchField::DEFAULT.name("Find"), true)] {
            let mut build = |ui: &mut Ui| { ui.search_field("find", "q", &traits); };
            let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
            crate::testing::frame(&mut ui, &[], &mut build);
            crate::testing::key(&mut ui, crate::KeyCode::Tab, &mut build);
            assert!(ui.ime().is_some(), "Tab focuses the field");
            crate::testing::key(&mut ui, crate::KeyCode::Enter, &mut build);
            assert_eq!(ui.ime().is_some(), kept, "Return with Done ends editing in its frame; with Search it does not");
            crate::testing::frame(&mut ui, &[], &mut build);
            assert_eq!(ui.ime().is_some(), kept, "and stays so");
        }
        fn body(ui: &mut Ui, c: &SearchFieldConfig) {
            ui.part(Part::EDITOR, Node::DEFAULT.w_px(200.0).h_px(40.0));
            ui.slot(c.editor);
        }
        for bodied in [false, true] {
            let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
            if bodied {
                ui.register_search_field_body("s", body).unwrap();
                ui.set_theme(&crate::ThemeHandle::parse("search_field.body = s").unwrap()).unwrap();
            }
            let field = crate::SearchField::DEFAULT.name("Find").action(crate::ReturnKey::Done);
            let mut build = |ui: &mut Ui| { ui.search_field("find", "Track", &field); };
            ui.set_a11y(true);
            crate::testing::frame(&mut ui, &[], &mut build);
            let r = crate::testing::find(&ui, pxui_types::a11y::Role::TextField, "Find").expect("the field");
            mem_sys::set_peer(crate::library_counter);
            let scope = mem_sys::warm_scope();
            for tick in 0..60 {
                mem_sys::local_reset();
                match tick % 3 {
                    0 => crate::testing::click(&mut ui, f64::from(r.x + r.w / 2.0), f64::from(r.y + r.h / 2.0), &mut build),
                    _ => crate::testing::key(&mut ui, crate::KeyCode::Enter, &mut build),
                }
                if tick >= 30 { assert_eq!(mem_sys::local_alloc_count(), 0, "warm focus/Done cycle allocated (body {bodied}, tick {tick})"); }
            }
            drop(scope);
            mem_sys::take_reports();
        }
    }

    /// U2b.4.4: a search field body that builds `Part::EDITOR` before its editor slot edits on that node (in a line it sized, U2b.4.8): the one text field in the
    /// accessibility tree, focus reported to the body, its look kept, the caret placed in the node's look font (26 points here), Escape
    /// clears; warm focused frames allocate nothing. Both link modes.
    #[test]
    fn a_search_body_edits_on_its_own_editor_part_in_its_font() {
        use pxui_types::a11y::Role;
        thread_local! { static FOCUSED: Cell<bool> = const { Cell::new(false) }; }
        const EDITOR: crate::theme::Look = crate::theme::Look::NONE.font(crate::theme::Font::BODY.size(26.0)).fill(crate::Rgba::rgb(1, 2, 3));
        fn search(ui: &mut Ui, c: &SearchFieldConfig) {
            FOCUSED.with(|f| f.set(c.state.focused != 0));
            ui.set_node_layout(&c.line, &Node::row().w(Size::Grow).h_px(56.0).align(pxui_types::layout::Align::Center));
            ui.part(Part::EDITOR, Node::DEFAULT.w_px(200.0).h_px(40.0).look(EDITOR));
            ui.slot(c.editor);
        }
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
        ui.set_a11y(true);
        ui.register_search_field_body("s", search).unwrap();
        ui.set_theme(&crate::ThemeHandle::parse("search_field.body = s").unwrap()).unwrap();
        let changed = RefCell::new(None::<String>);
        let mut build = |ui: &mut Ui| {
            if let Some(q) = ui.search_field("find", "query", &crate::SearchField::DEFAULT.name("Find")).changed { *changed.borrow_mut() = Some(q.to_owned()); }
        };
        crate::testing::frame(&mut ui, &[], &mut build);
        let fields = |ui: &Ui| ui.a11y_nodes().iter().filter(|n| n.role == Role::TextField as u32).count();
        assert_eq!(fields(&ui), 1, "the body's editor is the only text field");
        let editor = crate::testing::find(&ui, Role::TextField, "Find").expect("the editor part carries the field's name");
        assert_eq!((editor.w, editor.h), (200.0, 40.0), "the body's editor part is the field");
        assert_eq!((editor.x, editor.y), (0.0, 8.0), "the body sized the line (U2b.4.8): 56 high from the window's edge, the editor centred in it");
        assert!(ui.layers().flat_map(|l| l.instances.iter()).any(|i| i.color == [1, 2, 3, 255]), "the editor keeps the body's look");
        // past the text's end: the caret lands after "query", wherever that is in the field's font
        crate::testing::click(&mut ui, f64::from(editor.x + 150.0), f64::from(editor.y + editor.h / 2.0), &mut build);
        crate::testing::frame(&mut ui, &[], &mut build);
        assert!(FOCUSED.with(|f| f.get()), "the body sees the field focused");
        let caret = ui.ime().expect("the focused editor places the input method").caret;
        let end = ui.text_service().width("query", crate::Label::BODY.size(26.0), None).unwrap();
        assert_eq!(caret, (2.0 + end).round(), "the caret is measured in the editor's 26-point font");
        mem_sys::set_peer(crate::library_counter);
        let scope = mem_sys::warm_scope();
        for round in 0..3 {
            mem_sys::local_reset();
            crate::testing::frame(&mut ui, &[], &mut build);
            assert_eq!(mem_sys::local_alloc_count(), 0, "a warm focused body-editor frame allocated (round {round})");
        }
        drop(scope);
        assert!(mem_sys::take_reports().is_empty());
        crate::testing::key(&mut ui, crate::KeyCode::Escape, &mut build);
        assert_eq!(changed.borrow().as_deref(), Some(""), "Escape clears the query on the body's editor");
    }

    /// U2b.4.23: a search field body styles the clear key it builds for the state behaviour gives it (`SearchFieldConfig.clear`), with
    /// no signal of its own: idle, then hovered under the pointer, then pressed (the kit's priority over hovered) while held; the
    /// release clears the query. Both link modes.
    #[test]
    fn a_search_body_styles_its_clear_key_for_the_kits_state() {
        use pxui_types::a11y::Role;
        thread_local! { static SEEN: Cell<crate::theme::State> = const { Cell::new(crate::theme::State { hovered: false, pressed: false, focused: false, disabled: false, checked: false }) }; }
        const KEY: crate::theme::States = crate::theme::States::of(crate::theme::Look::NONE.fill(crate::Rgba::rgb(1, 1, 1)))
            .hover(crate::theme::Look::NONE.fill(crate::Rgba::rgb(2, 2, 2))).pressed(crate::theme::Look::NONE.fill(crate::Rgba::rgb(3, 3, 3)));
        fn search(ui: &mut Ui, c: &SearchFieldConfig) {
            let state = crate::theme::State::default().hovered(c.clear.hovered != 0).pressed(c.clear.pressed != 0);
            SEEN.with(|s| s.set(state));
            ui.part(Part::EDITOR, Node::DEFAULT.w_px(200.0).h_px(40.0));
            ui.slot(c.editor);
            let clear = ui.part(Part::CLEAR, Node::DEFAULT.w_px(40.0).h_px(40.0));
            ui.style(&clear, &KEY, state);
        }
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
        ui.set_a11y(true);
        ui.register_search_field_body("s", search).unwrap();
        ui.set_theme(&crate::ThemeHandle::parse("search_field.body = s").unwrap()).unwrap();
        let changed = RefCell::new(None::<String>);
        let mut build = |ui: &mut Ui| {
            if let Some(q) = ui.search_field("find", "query", &crate::SearchField::DEFAULT.name("Find")).changed { *changed.borrow_mut() = Some(q.to_owned()); }
        };
        let filled = |ui: &Ui, v: u8| ui.layers().flat_map(|l| l.instances.iter()).any(|i| i.color == [v, v, v, 255]);
        crate::testing::frame(&mut ui, &[], &mut build);
        let key = crate::testing::find(&ui, Role::Button, "Clear").expect("behaviour names the body's clear key");
        let (x, y) = (f64::from(key.x + key.w / 2.0), f64::from(key.y + key.h / 2.0));
        assert!(filled(&ui, 1) && SEEN.with(|s| s.get()) == crate::theme::State::default(), "idle");
        crate::testing::frame(&mut ui, &[crate::Event { kind: crate::Event::POINTER_MOVED, x, y, ..Default::default() }], &mut build);
        crate::testing::frame(&mut ui, &[], &mut build);
        assert!(SEEN.with(|s| s.get()).hovered && filled(&ui, 2), "the body sees the key hovered and shows its hovered look");
        crate::testing::frame(&mut ui, &[crate::Event { kind: crate::Event::POINTER_PRESSED, x, y, ..Default::default() }], &mut build);
        crate::testing::frame(&mut ui, &[], &mut build);
        assert!(SEEN.with(|s| s.get()).pressed && filled(&ui, 3), "held: the pressed look wins over hovered");
        crate::testing::frame(&mut ui, &[crate::Event { kind: crate::Event::POINTER_RELEASED, x, y, ..Default::default() }], &mut build);
        assert_eq!(changed.borrow().as_deref(), Some(""), "the release clears the query");
    }

    /// U2b.4.15: a picker field body that builds `Part::EDITOR` gets the query painted on that part in its look's font, 2 points in;
    /// warm frames with the body's editor allocate nothing. Both link modes.
    #[test]
    fn a_picker_field_body_paints_the_query_on_its_editor_part_in_its_font() {
        use pxui_types::{a11y::Role, draw::kind};
        const EDITOR: crate::theme::Look = crate::theme::Look::NONE.font(crate::theme::Font::BODY.size(26.0));
        fn field(ui: &mut Ui, _: &PickerFieldConfig) {
            let part = ui.part(Part::EDITOR, Node::DEFAULT.w_px(200.0).h_px(40.0).look(EDITOR).role(Role::Image));
            ui.a11y_name(&part, "editor");
        }
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
        ui.set_a11y(true);
        ui.register_picker_field_body("f", field).unwrap();
        ui.set_theme(&crate::ThemeHandle::parse("picker_field.body = f").unwrap()).unwrap();
        let mut build = |ui: &mut Ui| { let _ = ui.picker("pick", "Search", "query", &crate::Picker::DEFAULT, &mut |_, _| {}); };
        for _ in 0..2 { crate::testing::frame(&mut ui, &[], &mut build); }
        let editor = crate::testing::find(&ui, Role::Image, "editor").expect("the body's editor part");
        // the query's glyphs: those inside the editor part
        let end = ui.layers().flat_map(|l| l.instances.iter()).filter(|i| i.kind == kind::MASK && i.rect[0] >= editor.x && i.rect[2] <= editor.x + editor.w
            && i.rect[1] >= editor.y && i.rect[3] <= editor.y + editor.h).map(|i| i.rect[2]).fold(f32::MIN, f32::max);
        let (big, small) = (ui.text_service().width("query", crate::Label::BODY.size(26.0), None).unwrap(), ui.text_service().width("query", crate::Label::BODY.size(15.0), None).unwrap());
        assert!((end - (editor.x + 2.0 + big)).abs() <= 3.0, "the query ends where the 26-point font puts it: {end} vs {}", editor.x + 2.0 + big);
        assert!(end > editor.x + 2.0 + small + 10.0, "not in the kit's font");
        mem_sys::set_peer(crate::library_counter);
        let scope = mem_sys::warm_scope();
        for round in 0..3 {
            mem_sys::local_reset();
            crate::testing::frame(&mut ui, &[], &mut build);
            assert_eq!(mem_sys::local_alloc_count(), 0, "a warm picker frame with the body's editor allocated (round {round})");
        }
        drop(scope);
        assert!(mem_sys::take_reports().is_empty());
    }

    /// U2b.4.15: a picker row body gets the row's image (`Picker::image`) and, on a row that starts a group, the heading box, which it
    /// lays out; it sets its own row height and the results stack the measured rows. The built-in body draws the image (a texture
    /// break) and gives the heading a row's height. Warm frames allocate nothing. Both link modes.
    #[test]
    fn a_picker_row_body_gets_its_image_and_heading_and_sets_its_height() {
        use pxui_types::a11y::Role;
        thread_local! { static GOT: RefCell<Vec<(i64, u32, bool)>> = const { RefCell::new(Vec::new()) }; }
        const IMAGE: crate::Image = crate::Image { mtl_texture: core::ptr::dangling_mut(), width: 8, height: 4, format: crate::Image::FORMAT_RGBA8, ..crate::Image::rgba8(8, 4, core::ptr::null_mut(), core::ptr::null_mut()) };
        fn row(ui: &mut Ui, c: &PickerConfig) {
            GOT.with(|g| g.borrow_mut().push((c.index, c.image().map_or(0, |i| i.width), c.head().is_some())));
            ui.set_node_layout(&c.row, &Node::row().w(Size::Grow).h_px(44.0));
            if let Some(head) = c.head() { ui.set_node_layout(&head, &Node::row().w(Size::Grow).h_px(30.0)); }
        }
        let labels = ["R0", "R1", "R2"];
        let heads = ["Songs", "", "Albums"];
        let image = |i: usize| (i == 1).then_some(IMAGE);
        let picker = crate::Picker::DEFAULT.image(&image);
        let mut build = |ui: &mut Ui| {
            let _ = ui.picker("pick", "Search", "", &picker, &mut |_, rows| for i in 0..3 { rows.push(crate::PickRow::new(labels[i]).heading(heads[i])) });
        };
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
        ui.set_a11y(true);
        for _ in 0..3 { crate::testing::frame(&mut ui, &[], &mut build); }
        let breaks: usize = ui.layers().map(|l| l.breaks).sum();
        assert!(breaks > 0, "the built-in row draws the image");
        let kit_row = crate::testing::find(&ui, Role::ListItem, "R0").unwrap().h;
        assert_eq!(crate::testing::find(&ui, Role::Heading, "Songs").unwrap().h, kit_row, "the built-in body gives a heading a row's height");
        ui.register_picker_body("r", row).unwrap();
        ui.set_theme(&crate::ThemeHandle::parse("picker.body = r").unwrap()).unwrap();
        for _ in 0..3 { GOT.with(|g| g.borrow_mut().clear()); crate::testing::frame(&mut ui, &[], &mut build); }
        assert_eq!(GOT.with(|g| g.borrow().clone()), [(0, 0, true), (1, 8, false), (2, 0, true)], "image and heading per row (`image()`, `head()`)");
        let at = |role, name| crate::testing::find(&ui, role, name).unwrap();
        let (songs, r0, r1, albums, r2) = (at(Role::Heading, "Songs"), at(Role::ListItem, "R0"), at(Role::ListItem, "R1"), at(Role::Heading, "Albums"), at(Role::ListItem, "R2"));
        assert_eq!((songs.h, r0.h, r1.h, albums.h, r2.h), (30.0, 44.0, 44.0, 30.0, 44.0), "the body's heights");
        assert_eq!((r0.y, r1.y, albums.y, r2.y), (songs.y + 30.0, r0.y + 44.0, r1.y + 44.0, albums.y + 30.0), "the measured rows stack");
        mem_sys::set_peer(crate::library_counter);
        let scope = mem_sys::warm_scope();
        for round in 0..3 {
            mem_sys::local_reset();
            crate::testing::frame(&mut ui, &[], &mut build);
            assert_eq!(mem_sys::local_alloc_count(), 0, "a warm picker frame with images and headings allocated (round {round})");
        }
        drop(scope);
        assert!(mem_sys::take_reports().is_empty());
    }

    /// U2b.4.15: the picker's results scroll: `limit` rows of the kit's height show, the wheel scrolls the rest into view, and the
    /// keyboard's row is kept in view. Both link modes.
    #[test]
    fn picker_results_scroll_with_the_wheel_and_follow_the_keyboard() {
        use pxui_types::a11y::Role;
        use crate::Event;
        const NAMES: [&str; 30] = ["P00", "P01", "P02", "P03", "P04", "P05", "P06", "P07", "P08", "P09", "P10", "P11", "P12", "P13", "P14",
            "P15", "P16", "P17", "P18", "P19", "P20", "P21", "P22", "P23", "P24", "P25", "P26", "P27", "P28", "P29"];
        let mut build = |ui: &mut Ui| { let _ = ui.picker("pick", "Search", "", &crate::Picker::DEFAULT.limit(5), &mut |_, rows| for n in NAMES { rows.push(crate::PickRow::new(n)) }); };
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
        ui.set_a11y(true);
        let mut time = 0.0;
        let mut step = |ui: &mut Ui, events: &[Event]| {
            time += 1.0 / 60.0;
            let mut input = FrameIn::new(320.0, 240.0).time(time);
            (input.wire.events, input.wire.event_count) = (events.as_ptr(), events.len());
            ui.frame(&input, &mut build).unwrap();
        };
        for _ in 0..3 { step(&mut ui, &[]); }
        let list = ui.a11y_nodes().iter().find(|n| n.role == Role::List as u32).map(|n| n.rect).expect("the results list");
        let row = crate::testing::find(&ui, Role::ListItem, "P00").unwrap();
        assert_eq!(list.h, 5.0 * row.h, "limit rows of the kit's height show");
        let first = row.y;
        let over = (f64::from(list.x + 10.0), f64::from(list.y + 10.0));
        step(&mut ui, &[crate::Event { kind: crate::Event::POINTER_MOVED, x: over.0, y: over.1, ..Default::default() }]);
        step(&mut ui, &[Event { kind: Event::SCROLLED, y: -3.0 * f64::from(row.h), ..Default::default() }]);
        for _ in 0..30 { step(&mut ui, &[]); }
        assert!(crate::testing::find(&ui, Role::ListItem, "P00").is_none_or(|r| r.y < first), "the wheel scrolled the results");
        for _ in 0..20 { step(&mut ui, &[Event { kind: Event::KEY_PRESSED, code: crate::KeyCode::ArrowDown.wire(), ..Default::default() }]); step(&mut ui, &[]); }
        for _ in 0..10 { step(&mut ui, &[]); }
        let current = crate::testing::find(&ui, Role::ListItem, "P20").expect("the keyboard's row is built");
        assert!(current.y >= list.y - 0.5 && current.y + current.h <= list.y + list.h + 0.5, "the keyboard's row is in view: {current:?} in {list:?}");
    }

    /// U2b.4.16: a picker field body lays out the results region (`PickerFieldConfig.list`): its maximum height caps the rows it
    /// shows, its padding insets them inside the scroll, and its minimum height holds an empty region open; warm frames allocate
    /// nothing. Both link modes.
    #[test]
    fn a_picker_field_body_lays_out_the_results_region() {
        use pxui_types::a11y::Role;
        fn field(ui: &mut Ui, c: &PickerFieldConfig) {
            ui.set_node_layout(&c.list, &Node::column().w(Size::Grow).h(Size::Fit).pad(6.0).min_h(60.0).max_h(100.0));
        }
        const NAMES: [&str; 12] = ["P00", "P01", "P02", "P03", "P04", "P05", "P06", "P07", "P08", "P09", "P10", "P11"];
        let rows = core::cell::Cell::new(NAMES.len());
        let mut build = |ui: &mut Ui| { let _ = ui.picker("pick", "Search", "", &crate::Picker::DEFAULT, &mut |_, out| for n in &NAMES[..rows.get()] { out.push(crate::PickRow::new(n)) }); };
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
        ui.set_a11y(true);
        ui.register_picker_field_body("f", field).unwrap();
        ui.set_theme(&crate::ThemeHandle::parse("picker_field.body = f").unwrap()).unwrap();
        for _ in 0..3 { crate::testing::frame(&mut ui, &[], &mut build); }
        let list = |ui: &Ui| ui.a11y_nodes().iter().find(|n| n.role == Role::List as u32).map(|n| n.rect).expect("the results list");
        let (region, first) = (list(&ui), crate::testing::find(&ui, Role::ListItem, "P00").unwrap());
        assert_eq!(region.h, 100.0, "the body's maximum height, not ten kit rows");
        assert_eq!(first.y, region.y + 6.0, "the body's padding insets the first row");
        mem_sys::set_peer(crate::library_counter);
        let scope = mem_sys::warm_scope();
        for round in 0..3 {
            mem_sys::local_reset();
            crate::testing::frame(&mut ui, &[], &mut build);
            assert_eq!(mem_sys::local_alloc_count(), 0, "a warm picker frame with a laid-out region allocated (round {round})");
        }
        drop(scope);
        assert!(mem_sys::take_reports().is_empty());
        rows.set(0);
        for _ in 0..3 { crate::testing::frame(&mut ui, &[], &mut build); }
        assert_eq!(list(&ui).h, 60.0, "the body's minimum height holds the empty region open");
    }

    /// U2b.4.1: a registered button body reads the caller's text, detail and accessory styles back as the `Label`s it gave (every
    /// field), `None` for those it did not give. Both link modes.
    #[test]
    fn a_button_body_reads_back_the_callers_labels() {
        use pxui_types::{a11y::Role, text::TextAlign, theme::ColorToken};
        thread_local! { static GOT: RefCell<Vec<Option<String>>> = const { RefCell::new(Vec::new()) }; }
        fn button(_: &mut Ui, c: &ButtonConfig) {
            GOT.with(|g| g.borrow_mut().extend([c.label(), c.detail_label(), c.accessory_label()].map(|l| l.map(|l| format!("{:?}", l.fields())))));
        }
        const TEXT: crate::Label = crate::Label::BODY.size(13.0).weight(500).tracking(-0.02).caps(true).color(ColorToken(7)).grow().line_height(18.0).pad(2.0)
            .wrap_spaces().lines(2).ellipsis_middle().align(TextAlign::Center).h_px(20.0).whole_pixels().role(Role::Unknown);
        const DETAIL: crate::Label = crate::Label::MONO.font(1).w_px(44.0).ellipsis_head();
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
        ui.register_button_body("b", button).unwrap();
        ui.set_theme(&crate::ThemeHandle::parse("button.body = b").unwrap()).unwrap();
        let mut build = |ui: &mut Ui| {
            ui.button("full", "Full", &crate::Button::DEFAULT.label(TEXT).detail("d").detail_label(DETAIL).accessory("⌘K"));
        };
        crate::testing::frame(&mut ui, &[], &mut build);
        let want = [Some(format!("{:?}", TEXT.fields())), Some(format!("{:?}", DETAIL.fields())), None];
        assert_eq!(GOT.with(|g| g.borrow().clone()), want, "each style as the caller gave it; none for the unstyled accessory");
    }

    /// U2b.4.2: a registered button body that runs its config's `built_in` slot draws exactly what the kit's body draws (instances and
    /// accessibility) for plain, sized, laid-out, part-carrying, content and disabled buttons; warm frames allocate nothing. Both link modes.
    #[test]
    fn a_button_body_running_built_in_draws_the_kit_button() {
        fn kit(ui: &mut Ui, c: &ButtonConfig) { ui.slot(c.built_in); }
        let mut build = |ui: &mut Ui| {
            use crate::{Button, Label};
            ui.button("plain", "Plain", &Button::DEFAULT);
            ui.button("sized", "Sized", &Button::sized(90.0, 30.0).selected(true));
            ui.button("laid", "Laid", &Button::DEFAULT.size(Size::Grow, Size::Px(32.0)).label(Label::BODY.size(13.0)).gap(4.0));
            ui.button("parts", "Search", &Button::DEFAULT.icon(crate::Icon::glyph('?')).detail("Library").accessory("⌘K").disabled(true));
            ui.button_content("content", &Button::sized(60.0, 24.0).name("Content"), |ui, _| { ui.label("inside", Label::DEFAULT); });
        };
        let draw = |ui: &Ui| (format!("{:?}", ui.layers().flat_map(|l| l.instances.iter()).collect::<Vec<_>>()), ui.a11y_nodes().iter().map(|n| (n.role, n.states, ui.a11y_name_of(n).to_owned())).collect::<Vec<_>>());
        let (mut plain, mut bodied) = (Ui::new(&crate::UiDesc::DEFAULT).unwrap(), Ui::new(&crate::UiDesc::DEFAULT).unwrap());
        bodied.register_button_body("kit", kit).unwrap();
        bodied.set_theme(&crate::ThemeHandle::parse("button.body = kit").unwrap()).unwrap();
        for ui in [&mut plain, &mut bodied] { ui.set_a11y(true); for _ in 0..2 { crate::testing::frame(ui, &[], &mut build); } }
        assert_eq!(draw(&bodied), draw(&plain), "the built-in slot draws the kit's button");
        mem_sys::set_peer(crate::library_counter);
        let scope = mem_sys::warm_scope();
        for round in 0..3 {
            mem_sys::local_reset();
            crate::testing::frame(&mut bodied, &[], &mut build);
            assert_eq!(mem_sys::local_alloc_count(), 0, "a warm frame of built-in slots allocated (round {round})");
        }
        drop(scope);
        assert!(mem_sys::take_reports().is_empty());
    }

    /// U2b.4.6: a registered icon button body that runs its config's `built_in` slot draws exactly what the kit's body draws (instances
    /// and accessibility) for glyph, drawn, sized, pressed-scale, selected and disabled icon buttons; warm frames allocate nothing. Both
    /// link modes.
    #[test]
    fn an_icon_button_body_running_built_in_draws_the_kit_icon_button() {
        fn kit(ui: &mut Ui, c: &IconButtonConfig) { ui.slot(c.built_in); }
        fn draw(p: &mut crate::Paint<'_>, r: Rect, ink: pxui_types::geom::Rgba, _: u32) { p.fill(r, crate::Fill::new(ink)); }
        let mut build = |ui: &mut Ui| {
            use crate::{Icon, IconButton};
            ui.icon_button("glyph", Icon::glyph('?'), "Glyph", &IconButton::DEFAULT);
            ui.icon_button("drawn", Icon::Draw(draw, 3), "Drawn", &IconButton::sized(44.0, 40.0).selected(true));
            ui.icon_button("dip", Icon::Draw(draw, 4), "Dip", &IconButton::sized(36.0, 36.0).press_scale(0.96));
            ui.icon_button("off", Icon::glyph('x'), "Off", &IconButton::DEFAULT.disabled(true));
        };
        let draw_of = |ui: &Ui| (format!("{:?}", ui.layers().flat_map(|l| l.instances.iter()).collect::<Vec<_>>()), ui.a11y_nodes().iter().map(|n| (n.role, n.states, ui.a11y_name_of(n).to_owned())).collect::<Vec<_>>());
        let (mut plain, mut bodied) = (Ui::new(&crate::UiDesc::DEFAULT).unwrap(), Ui::new(&crate::UiDesc::DEFAULT).unwrap());
        bodied.register_icon_button_body("kit", kit).unwrap();
        bodied.set_theme(&crate::ThemeHandle::parse("icon_button.body = kit").unwrap()).unwrap();
        for ui in [&mut plain, &mut bodied] { ui.set_a11y(true); for _ in 0..2 { crate::testing::frame(ui, &[], &mut build); } }
        assert_eq!(draw_of(&bodied), draw_of(&plain), "the built-in slot draws the kit's icon button");
        mem_sys::set_peer(crate::library_counter);
        let scope = mem_sys::warm_scope();
        for round in 0..3 {
            mem_sys::local_reset();
            crate::testing::frame(&mut bodied, &[], &mut build);
            assert_eq!(mem_sys::local_alloc_count(), 0, "a warm frame of icon button built-in slots allocated (round {round})");
        }
        drop(scope);
        assert!(mem_sys::take_reports().is_empty());
    }

    /// U2b.4.9: a selected icon button keeps its checked look while hovered and held (the kit's body and the ink a registered body is
    /// given), its press scale still springs, and an unselected one still takes its hover look. Both link modes.
    #[test]
    fn a_selected_icon_button_keeps_its_checked_look_while_hovered_and_held() {
        use crate::theme::{Look, States};
        const LOOKS: States = States::of(Look::NONE.fill(crate::Rgba::rgb(1, 1, 1))).hover(Look::NONE.fill(crate::Rgba::rgb(2, 2, 2)))
            .pressed(Look::NONE.fill(crate::Rgba::rgb(3, 3, 3))).checked(Look::NONE.fill(crate::Rgba::rgb(4, 4, 4)));
        let on = Cell::new(true);
        let mut build = |ui: &mut Ui| { ui.icon_button("key", crate::Icon::glyph('?'), "Key", &crate::IconButton::sized(40.0, 40.0).look(&LOOKS).press_scale(0.9).selected(on.get())); };
        let fill = |ui: &Ui, c: u8| ui.layers().flat_map(|l| l.instances.iter()).any(|i| i.color == [c, c, c, 255]);
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
        crate::testing::frame(&mut ui, &[], &mut build);
        let over = crate::Event { kind: crate::Event::POINTER_MOVED, x: 20.0, y: 20.0, ..Default::default() };
        crate::testing::frame(&mut ui, &[over], &mut build);
        assert!(fill(&ui, 4) && !fill(&ui, 2), "hovered and on: the checked look");
        let down = crate::Event { kind: crate::Event::POINTER_PRESSED, ..over };
        crate::testing::frame(&mut ui, &[down], &mut build);
        crate::testing::frame(&mut ui, &[], &mut build);
        assert!(fill(&ui, 4) && !fill(&ui, 3), "held and on: the checked look");
        assert!(ui.repaint_at().is_some(), "the press scale springs");
        crate::testing::frame(&mut ui, &[crate::Event { kind: crate::Event::POINTER_RELEASED, ..over }], &mut build);
        on.set(false);
        crate::testing::frame(&mut ui, &[], &mut build);
        crate::testing::frame(&mut ui, &[over], &mut build);
        assert!(fill(&ui, 2), "hovered and off: the hover look");
    }

    /// U2b.4.20: one look priority for every kit widget (`States::pick`): a selected button keeps its checked look while hovered and
    /// held, as the icon button does; disabled wins over checked. Fails with the old order (hovered before checked). Both link modes.
    #[test]
    fn a_selected_button_keeps_its_checked_look_and_disabled_wins() {
        use crate::theme::{Look, States};
        const LOOKS: States = States::of(Look::NONE.fill(crate::Rgba::rgb(1, 1, 1))).hover(Look::NONE.fill(crate::Rgba::rgb(2, 2, 2)))
            .pressed(Look::NONE.fill(crate::Rgba::rgb(3, 3, 3))).checked(Look::NONE.fill(crate::Rgba::rgb(4, 4, 4)))
            .disabled(Look::NONE.fill(crate::Rgba::rgb(5, 5, 5)));
        let off = Cell::new(false);
        let mut build = |ui: &mut Ui| { ui.button("key", "Key", &crate::Button::sized(80.0, 40.0).look(&LOOKS).selected(true).disabled(off.get())); };
        let fill = |ui: &Ui, c: u8| ui.layers().flat_map(|l| l.instances.iter()).any(|i| i.color == [c, c, c, 255]);
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
        crate::testing::frame(&mut ui, &[], &mut build);
        let over = crate::Event { kind: crate::Event::POINTER_MOVED, x: 20.0, y: 20.0, ..Default::default() };
        crate::testing::frame(&mut ui, &[over], &mut build);
        assert!(fill(&ui, 4) && !fill(&ui, 2), "hovered and selected: the checked look");
        crate::testing::frame(&mut ui, &[crate::Event { kind: crate::Event::POINTER_PRESSED, ..over }], &mut build);
        crate::testing::frame(&mut ui, &[], &mut build);
        assert!(fill(&ui, 4) && !fill(&ui, 3), "held and selected: the checked look");
        crate::testing::frame(&mut ui, &[crate::Event { kind: crate::Event::POINTER_RELEASED, ..over }], &mut build);
        off.set(true);
        crate::testing::frame(&mut ui, &[], &mut build);
        assert!(fill(&ui, 5) && !fill(&ui, 4), "disabled and selected: the disabled look");
    }

    #[test]
    fn icon_of_only_trusts_the_wrappers_trampoline() {
        extern "C" fn foreign(_: *mut c_void, _: *mut c_void, _: Rect, _: pxui_types::geom::Rgba, _: *const pxui_types::ffi::PxuiPaintInfo) {}
        fn app(_: &mut crate::Paint<'_>, _: Rect, _: pxui_types::geom::Rgba, _: u32) {}
        let user = app as fn(&mut crate::Paint<'_>, Rect, pxui_types::geom::Rgba, u32) as *mut c_void;
        assert!(icon_of("", Some(foreign), user, 0).is_none(), "a C caller's user pointer is never called");
        assert!(matches!(icon_of("", Some(crate::paint::icon_draw), user, 3), Some(crate::Icon::Draw(_, 3))));
        assert!(matches!(icon_of("x", None, core::ptr::null_mut(), 0), Some(crate::Icon::Glyph('x'))));
        assert!(icon_of("", Some(crate::paint::icon_draw), core::ptr::null_mut(), 0).is_none());
    }

    thread_local! {
        static TOGGLES: RefCell<Vec<(String, bool, bool)>> = const { RefCell::new(Vec::new()) };
        static ROWS: Cell<i64> = const { Cell::new(0) };
        static CONTENT: Cell<u32> = const { Cell::new(0) };
        static SLOT: Cell<PxuiSlot> = const { Cell::new(PxuiSlot { serial: 0 }) };
        static HINTS: RefCell<Option<(Option<(String, String)>, bool, bool, bool, bool)>> = const { RefCell::new(None) };
        static FORGED: Cell<(bool, bool)> = const { Cell::new((false, false)) };
        static BOOM: Cell<bool> = const { Cell::new(true) };
        static TAB: RefCell<Option<Handle>> = const { RefCell::new(None) };
        static TAB_DETAIL: RefCell<String> = const { RefCell::new(String::new()) };
    }

    fn input(time: f64) -> FrameIn<'static> { FrameIn::new(320.0, 240.0).time(time) }
    fn source(text: &str) -> ThemeHandle { ThemeHandle::parse(text).expect("theme source") }

    fn probe(ui: &mut Ui, c: &crate::bodies::ToggleConfig) {
        TOGGLES.with(|t| t.borrow_mut().push((c.text_of(&c.label).to_owned(), c.on != 0, c.state.hovered != 0)));
        ROWS.with(|r| r.set(c.row.box_key));
        let b = ui.part(Part::HIT, Node { w: Size::Px(12.0), h: Size::Px(12.0), ..Node::DEFAULT });
        ui.text(&b, "x");
    }

    /// A registered body is called with its kind's record (text, value, state, the row it owns), builds a part behaviour then finds, is picked
    /// by theme text (the Ui's own and a scoped theme) and only inside the scope that names it.
    #[test]
    fn a_registered_body_is_selected_by_theme_text_and_gets_its_config() {
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        let id = ui.register_toggle_body("probe", probe).expect("registered");
        assert_eq!(id, BodyId(1));
        assert_eq!(ui.register_toggle_body("probe", probe).expect("again"), id, "a name again keeps its id");
        let scoped = crate::ThemeHandle::parse("toggle.body = probe").expect("scoped theme");
        // outside the scope the built-in body builds
        for n in 0..2 { ui.frame(&input(n as f64), &mut |ui: &mut Ui| { ui.toggle("toggle", "plain", true, &crate::Toggle::DEFAULT); }).expect("frame"); }
        assert!(TOGGLES.with(|t| t.borrow().is_empty()), "the body ran with no theme naming it");
        for n in 2..4 {
            ui.frame(&input(n as f64), &mut |ui: &mut Ui| { ui.toggle("toggle", "outside", false, &crate::Toggle::DEFAULT); ui.themed(&scoped, &mut |ui: &mut Ui| { ui.toggle("toggle", "Wi-Fi", true, &crate::Toggle::DEFAULT); }); }).expect("frame");
        }
        let seen = TOGGLES.with(|t| t.borrow().clone());
        assert!(!seen.is_empty() && seen.iter().all(|s| *s == ("Wi-Fi".to_owned(), true, false)), "only the scoped toggle ran the body: {seen:?}");
        let row = ui.box_of_key(crate::Key::Resolved(ROWS.with(Cell::get))).expect("the row");
        assert!(ui.part_rect(&row, Part::HIT).is_some(), "the part the body built is found by its widget's handle");
        assert!(ui.part_box(&row, Part::TRACK).is_none());
        // the Ui's own theme, from the theme text
        TOGGLES.with(|t| t.borrow_mut().clear());
        ui.set_theme(&source("toggle.body = probe")).expect("theme text");
        assert_eq!(ui.theme().body.toggle, id);
        ui.frame(&input(5.0), &mut |ui: &mut Ui| { ui.toggle("toggle", "Own", false, &crate::Toggle::DEFAULT); }).expect("frame");
        assert_eq!(TOGGLES.with(|t| t.borrow().clone()), vec![("Own".to_owned(), false, false)]);
    }

    /// U1c.3 (D6): the extension set's inspector toggle and button are built through the toggle and button bodies, so a registered body
    /// draws them: the switch is a toggle without text, the button a button with its label.
    #[test]
    fn the_inspector_toggle_and_button_are_built_through_their_bodies() {
        thread_local! { static BUTTONS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) }; }
        fn counting(_: &mut Ui, c: &crate::bodies::ButtonConfig) { BUTTONS.with(|b| b.borrow_mut().push(c.text_of(&c.label).to_owned())); }
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        ui.register_toggle_body("probe", probe).expect("toggle body");
        ui.register_button_body("counting", counting).expect("button body");
        ui.set_theme(&source("toggle.body = probe\nbutton.body = counting")).expect("theme text");
        TOGGLES.with(|t| t.borrow_mut().clear());
        ui.frame(&input(0.0), &mut |ui: &mut Ui| {
            let _ = crate::ext::inspector_toggle(ui, "snap", "Snap", true);
            let _ = crate::ext::inspector_button(ui, "reset", "Reset");
        }).expect("frame");
        assert_eq!(TOGGLES.with(|t| t.borrow().clone()), vec![(String::new(), true, false)], "the switch is the toggle body's");
        assert_eq!(BUTTONS.with(|b| b.borrow().clone()), vec!["Reset".to_owned()], "the button is the button body's");
    }

    /// A theme text that names no registered body, and every bad registration, are errors, not crashes.
    #[test]
    fn unknown_body_names_and_bad_registrations_are_errors() {
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        for name in ["default", "", "two words", "a=b", "x#"] {
            assert_eq!(ui.register_toggle_body(name, probe), Err(crate::Error::InvalidInput), "{name:?}");
            assert!(ui.last_error().contains("invalid body name"));
        }
        ui.register_toggle_body("probe", probe).unwrap();
        let before = *ui.theme();
        let invalid = source("tokens.accent = #ff0000\ntoggle.body = probe\nlabel.body = nope");
        assert_eq!(ui.set_theme(&invalid), Err(crate::Error::InvalidInput));
        assert!(ui.last_error().contains("nope"), "{}", ui.last_error());
        assert_eq!(*ui.theme(), before, "body validation precedes theme installation");
        // a body of another kind is not this kind's
        ui.register_slider_body("slider", |_, _| {}).expect("slider");
        assert_eq!(ui.set_theme(&source("toggle.body = slider")), Err(crate::Error::InvalidInput));
        let mut late = None;
        ui.frame(&input(0.0), &mut |ui: &mut Ui| { late = Some(ui.register_toggle_body("late", probe)); }).expect("frame");
        assert_eq!(late.expect("ran"), Err(crate::Error::State));
        assert!(ui.last_error().contains("outside a frame"));
        // a part outside a body is a caller bug (outside a frame, it is refused as every builder is)
        let refused = catch_unwind(AssertUnwindSafe(|| ui.part(Part::HIT, Node::DEFAULT)));
        assert!(refused.is_err());
        assert!(ui.last_error().contains("frame callback"), "{}", ui.last_error());
        ui.frame(&input(1.0), &mut |ui: &mut Ui| { ui.label("still usable", crate::Label::DEFAULT); }).expect("the Ui is not poisoned");
    }

    #[test]
    fn detached_theme_resolves_per_ui_and_scopes_use_the_builtin_fallback() {
        let theme = source("toggle.body = probe");
        let mut unregistered = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
        assert_eq!(unregistered.set_theme(&theme), Err(crate::Error::InvalidInput));
        unregistered.frame(&input(0.0), &mut |ui| ui.themed(&theme, &mut |ui| {
            assert_eq!(ui.theme().body.toggle, BodyId::DEFAULT);
            ui.toggle("toggle", "Builtin", false, &crate::Toggle::DEFAULT);
        })).unwrap();

        let mut registered = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
        registered.register_toggle_body("earlier", probe).unwrap();
        let id = registered.register_toggle_body("probe", probe).unwrap();
        assert_ne!(id, BodyId::DEFAULT);
        registered.set_theme(&theme).unwrap();
        assert_eq!(registered.theme().body.toggle, id);
    }

    fn boom(_: &mut Ui, _: &crate::bodies::ToggleConfig) {
        TOGGLES.with(|t| t.borrow_mut().push((String::new(), false, false)));
        if BOOM.with(Cell::get) { panic!("boom") }
    }

    /// A body that panics follows the callback panic rule: the panic is carried to the frame and resumed there, the bodies after it
    /// are skipped, and the next frame builds again.
    #[test]
    fn a_body_that_panics_follows_the_callback_panic_rule() {
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        ui.register_toggle_body("boom", boom).expect("registered");
        ui.set_theme(&source("toggle.body = boom")).expect("theme");
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _ = ui.frame(&input(0.0), &mut |ui: &mut Ui| { ui.toggle("toggle", "first", false, &crate::Toggle::DEFAULT); ui.toggle("toggle", "second", true, &crate::Toggle::DEFAULT); });
        }));
        assert_eq!(result.expect_err("the panic surfaces at frame()").downcast_ref::<&str>(), Some(&"boom"));
        assert_eq!(TOGGLES.with(|t| t.borrow().len()), 1, "the second body ran on a half-built tree");
        BOOM.with(|b| b.set(false));
        ui.frame(&input(1.0), &mut |ui: &mut Ui| { ui.toggle("toggle", "first", false, &crate::Toggle::DEFAULT); ui.toggle("toggle", "second", true, &crate::Toggle::DEFAULT); }).expect("the next frame renders");
        assert_eq!(TOGGLES.with(|t| t.borrow().len()), 3);
    }

    fn forger(_: &mut Ui, c: &crate::bodies::ToggleConfig) {
        let mut copy: pxui_types::ffi::PxuiToggleConfig = **c;
        copy.label = Str { ptr: "forged".as_ptr(), len: 6 };
        FORGED.with(|f| f.set((c.text_of(&c.label) == "Real", c.text_of(&copy.label).is_empty())));
    }

    /// An accessor reads only fields inside the config the library gave: a forged or copied field reads as empty.
    #[test]
    fn accessors_refuse_fields_outside_their_config() {
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        ui.register_toggle_body("forger", forger).expect("registered");
        ui.set_theme(&source("toggle.body = forger")).expect("theme");
        ui.frame(&input(0.0), &mut |ui: &mut Ui| { ui.toggle("toggle", "Real", true, &crate::Toggle::DEFAULT); }).expect("frame");
        assert_eq!(FORGED.with(Cell::get), (true, true));
    }

    fn hinter(_: &mut Ui, c: &crate::bodies::FooterConfig) {
        let real = c.hint_at(&c.hints, 0).map(|(k, v)| (k.to_owned(), v.to_owned()));
        let past = c.hint_at(&c.hints, c.hints.len).is_none();
        let copy = c.hints;
        let texts = [Str { ptr: "forged".as_ptr(), len: 6 }];
        let forged = pxui_types::ffi::Slice { ptr: texts.as_ptr(), len: 1 };
        let forged_hints = pxui_types::ffi::Slice::<pxui_types::ffi::PxuiHintDesc> { ptr: c.hints.ptr, len: c.hints.len };
        HINTS.with(|h| *h.borrow_mut() = Some((real, past, c.hint_at(&copy, 0).is_none(), c.text_at(&forged, 0).is_empty(), c.hint_at(&forged_hints, 0).is_none())));
    }

    /// A footer body reads its hints' real text; a list copied out of the config, or built by the app, reads as nothing.
    #[test]
    fn hint_and_list_accessors_read_library_lists_and_refuse_forged_ones() {
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        ui.register_footer_body("hinter", hinter).expect("registered");
        ui.set_theme(&source("footer.body = hinter")).expect("theme");
        ui.frame(&input(0.0), &mut |ui: &mut Ui| { crate::ext::footer(ui, None, &[("Esc", "close")]); }).expect("frame");
        let seen = HINTS.with(|h| h.borrow().clone()).expect("the body ran");
        assert_eq!(seen, (Some(("Esc".to_owned(), "close".to_owned())), true, true, true, true));
    }

    fn scroll(ui: &mut Ui, c: &crate::bodies::ScrollViewConfig) {
        SLOT.with(|s| s.set(c.content));
        assert!(!ui.slot(c.content), "a content slot answers false");
    }

    /// A body runs the caller's content through its slot, once; the slot is stale after the body call and refused, without poisoning.
    #[test]
    fn a_slot_runs_the_callers_content_and_is_refused_once_stale() {
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        ui.register_scroll_view_body("probe", scroll).expect("registered");
        ui.set_theme(&source("scroll_view.body = probe")).expect("theme");
        ui.frame(&input(0.0), &mut |ui: &mut Ui| { ui.scroll_view("list", crate::Node::DEFAULT.h(Size::Grow), &mut |ui: &mut Ui| { CONTENT.with(|c| c.set(c.get() + 1)); ui.label("row", crate::Label::DEFAULT); }); }).expect("frame");
        assert_eq!(CONTENT.with(Cell::get), 1);
        let stale = catch_unwind(AssertUnwindSafe(|| ui.slot(SLOT.with(Cell::get))));
        assert!(stale.is_err());
        assert!(ui.last_error().contains("stale slot"), "{}", ui.last_error());
        ui.frame(&input(1.0), &mut |ui: &mut Ui| { ui.label("still usable", crate::Label::DEFAULT); }).expect("the Ui is not poisoned");
    }

    /// Content run through a slot that panics follows the callback panic rule too (in the static build it unwinds through the slot to
    /// the body's entry): the panic surfaces at the frame, and the next frame's body runs its slot again.
    #[test]
    fn a_slot_whose_content_panics_follows_the_callback_panic_rule() {
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        ui.register_scroll_view_body("probe", |ui, c| { ui.slot(c.content); }).expect("registered");
        ui.set_theme(&source("scroll_view.body = probe")).expect("theme");
        let ran = Cell::new(0);
        let build = |ui: &mut Ui, boom: bool| { ui.scroll_view("list", crate::Node::DEFAULT.h(Size::Grow), &mut |_: &mut Ui| { ran.set(ran.get() + 1); if boom { panic!("content") } }); };
        let result = catch_unwind(AssertUnwindSafe(|| { let _ = ui.frame(&input(0.0), &mut |ui: &mut Ui| build(ui, true)); }));
        assert_eq!(result.expect_err("the panic surfaces at frame()").downcast_ref::<&str>(), Some(&"content"));
        ui.frame(&input(1.0), &mut |ui: &mut Ui| build(ui, false)).expect("the next frame renders");
        assert_eq!(ran.get(), 2, "the slot ran again the frame after");
    }

    fn wide_tab(ui: &mut Ui, c: &crate::bodies::TabsConfig) {
        let mut node = ui.node_layout(&c.node);
        node.w = Size::Px(100.0);
        ui.set_node_layout(&c.node, &node);
        ui.text(&c.node, c.text_of(&c.text));
        if c.index == 1 { TAB.with(|t| *t.borrow_mut() = Some(c.node)); TAB_DETAIL.with(|d| *d.borrow_mut() = c.text_of(&c.detail).to_string()); }
    }

    /// A body sizes the node its widget gave it (`set_node_layout`): the second tab is 100 points wide. A registered tab body is given
    /// the tab's detail (`Segment::detail`, `PxuiTabConfig.detail`).
    #[test]
    fn a_body_sizes_its_node_with_node_layout() {
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        ui.register_tabs_body("wide", wide_tab).expect("registered");
        ui.set_theme(&source("tabs.body = wide")).expect("theme");
        for n in 0..3 { ui.frame(&input(n as f64), &mut |ui: &mut Ui| { ui.tabs("t", &[crate::Segment::new("a"), crate::Segment::new("b").detail("12")], Some(0), &crate::Tabs::DEFAULT); }).expect("frame"); }
        let tab = TAB.with(|t| t.borrow().expect("the body ran"));
        assert_eq!(ui.rect(&tab).w, 100.0);
        assert_eq!(TAB_DETAIL.with(|d| d.borrow().clone()), "12");
    }

    fn tall_bar(ui: &mut Ui, c: &crate::bodies::TabsConfig) {
        ui.set_node_layout(&c.bar, &crate::Node::row().w(Size::Grow).h_px(44.0).gap(20.0));
        ui.text(&c.node, c.text_of(&c.text));
    }

    /// A tabs body owns its bar (`TabsConfig.bar`, U2b.4.29): it sizes it 44 points high and spaces its tabs 20 apart.
    #[test]
    fn a_tabs_body_sizes_and_spaces_its_bar() {
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        ui.set_a11y(true);
        ui.register_tabs_body("tall", tall_bar).expect("registered");
        ui.set_theme(&source("tabs.body = tall")).expect("theme");
        const BAR: crate::Tabs = crate::Tabs::DEFAULT.name("Bar");
        for n in 0..3 { ui.frame(&input(n as f64), &mut |ui: &mut Ui| { ui.tabs("t", &[crate::Segment::new("a"), crate::Segment::new("b")], Some(0), &BAR); }).expect("frame"); }
        let bar = crate::testing::find(&ui, crate::Role::TabList, "Bar").expect("the bar");
        let (a, b) = (crate::testing::find(&ui, crate::Role::Tab, "a").expect("a"), crate::testing::find(&ui, crate::Role::Tab, "b").expect("b"));
        assert_eq!(bar.h, 44.0, "{bar:?}");
        assert_eq!(b.x - (a.x + a.w), 20.0, "{a:?} {b:?}");
    }

    /// The `_with` forms build the caller's own children for each row and cell, in both link modes.
    #[test]
    fn collections_build_the_callers_cells() {
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        let (mut list, mut tree) = (crate::ListState::new(), crate::TreeState::new());
        let nodes = [crate::TreeNode { depth: 0, children: 1 }, crate::TreeNode { depth: 1, children: 0 }, crate::TreeNode { depth: 0, children: 0 }];
        let mut table = crate::TableState::new();
        const COLUMNS: [crate::Column; 2] = [crate::Column::new("a").w_px(80.0), crate::Column::new("b").w_px(80.0)];
        let (rows, cells, nodes_built) = (Cell::new(0), Cell::new(0), Cell::new(0));
        for n in 0..3 {
            ui.frame(&input(n as f64), &mut |ui: &mut Ui| {
                ui.selection_list("list", 5, &|i| i as i64, &mut list, &crate::SelectionList::DEFAULT, &mut |ui: &mut Ui, part, _| if let crate::ui::SectionPart::Row(row) = part { rows.set(rows.get() + 1); ui.label(&format!("row {row}"), crate::Label::DEFAULT); });
                let shown = ui.tree("tree", &nodes, &mut tree, &mut |ui: &mut Ui, node| { nodes_built.set(nodes_built.get() + 1); ui.label(&format!("node {node}"), crate::Label::DEFAULT); }).shown;
                assert_eq!(shown, 2, "the first node's child is closed");
                let _ = ui.table("table", &COLUMNS, 3, &|i| i as i64, &mut table, &crate::Table::DEFAULT, &mut |ui: &mut Ui, row, column, _| { cells.set(cells.get() + 1); ui.label(&format!("{row}:{column}"), crate::Label::DEFAULT); });
            }).expect("frame");
        }
        assert!(rows.get() > 0 && nodes_built.get() > 0 && cells.get() > 0, "rows {}, nodes {}, cells {}", rows.get(), nodes_built.get(), cells.get());
        assert_eq!(tree.visible[..2], [0, 2]);
    }

    thread_local! { static VARIANTS: RefCell<Vec<(&'static str, u16)>> = const { RefCell::new(Vec::new()) }; }

    /// One body per kind that builds two structures by `variant`: variant 1 a 120-point node, else a 40-point one.
    fn sized_label(ui: &mut Ui, c: &crate::bodies::LabelConfig) {
        VARIANTS.with(|v| v.borrow_mut().push(("label", c.variant)));
        let mut node = ui.node_layout(&c.node);
        node.w = Size::Px(if c.variant == 1 { 120.0 } else { 40.0 });
        ui.set_node_layout(&c.node, &node);
    }
    fn sized_button(ui: &mut Ui, c: &crate::bodies::ButtonConfig) {
        VARIANTS.with(|v| v.borrow_mut().push(("button", c.variant)));
        let mut node = ui.node_layout(&c.row);
        node.w = Size::Px(if c.variant == 1 { 120.0 } else { 40.0 });
        ui.set_node_layout(&c.row, &node);
        ui.text(&c.row, c.text_of(&c.label));
    }
    fn sized_field(_: &mut Ui, c: &crate::bodies::TextFieldConfig) { VARIANTS.with(|v| v.borrow_mut().push(("field", c.variant))); }
    fn sized_icon_button(_: &mut Ui, c: &crate::bodies::IconButtonConfig) { VARIANTS.with(|v| v.borrow_mut().push(("icon", c.variant))); }

    /// A call's style `variant` reaches the registered body's config (labels, buttons, icon buttons, text fields; 0 when the call sets none), so one body
    /// builds two structures and the app picks per call without comparing text; the variant never reaches a later widget, a built-in body
    /// ignores it, and warm frames allocate nothing.
    #[test]
    fn a_style_variant_reaches_the_registered_body_and_picks_its_structure() {
        use crate::{Button, Icon, IconButton, Label};
        mem_sys::set_peer(crate::library_counter);
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        ui.register_label_body("sized", sized_label).expect("label body");
        ui.register_button_body("sized", sized_button).expect("button body");
        ui.register_icon_button_body("sized", sized_icon_button).expect("icon body");
        ui.register_text_field_body("sized", sized_field).expect("field body");
        let scoped = crate::ThemeHandle::parse("label.body = sized\nbutton.body = sized\nicon_button.body = sized\ntext_field.body = sized").expect("theme");
        let (plain, wide) = (Label::DEFAULT, Label::DEFAULT.variant(1));
        let handles = Cell::new((None, None));
        let mut build = |ui: &mut Ui| {
            // built-in bodies ignore the variant and leave nothing behind for the next widget
            ui.label("built-in", wide);
            ui.themed(&scoped, &mut |ui: &mut Ui| {
                ui.label("plain", plain);
                ui.label("wide", wide);
                handles.set((Some(ui.button("narrow", "Narrow", &Button::DEFAULT).handle), Some(ui.button("wide", "Wide", &Button::DEFAULT.variant(1)).handle)));
                ui.icon_button("icon", Icon::Glyph('x'), "Icon", &IconButton::DEFAULT.variant(7));
                ui.icon_button("icon2", Icon::Glyph('x'), "Icon", &IconButton::DEFAULT);
                ui.text_field("Find", "", &crate::TextField::SEARCH.variant(3));
                ui.text_field("Other", "", &crate::TextField::SEARCH);
            });
        };
        for n in 0..3 { VARIANTS.with(|v| v.borrow_mut().clear()); ui.frame(&input(n as f64), &mut build).expect("frame"); }
        let seen = VARIANTS.with(|v| v.borrow().clone());
        assert_eq!(seen, vec![("label", 0), ("label", 1), ("button", 0), ("button", 1), ("icon", 7), ("icon", 0), ("field", 3), ("field", 0)], "each call's own variant, none leaked");
        let (narrow, wide) = handles.get();
        assert_eq!((ui.rect(&narrow.unwrap()).w, ui.rect(&wide.unwrap()).w), (40.0, 120.0), "one body, two structures by variant");
        let scope = mem_sys::warm_scope();
        for n in 3..60 {
            VARIANTS.with(|v| v.borrow_mut().clear());
            mem_sys::local_reset();
            ui.frame(&input(n as f64), &mut build).expect("frame");
            assert_eq!(mem_sys::local_alloc_count(), 0, "warm frames with variants allocate nothing");
        }
        drop(scope);
        assert!(mem_sys::take_reports().is_empty());
    }

    fn menu_row(_: &mut Ui, c: &crate::bodies::MenuRowConfig) { VARIANTS.with(|v| v.borrow_mut().push(("row", c.variant))); }
    fn toggle_probe(_: &mut Ui, c: &crate::bodies::ToggleConfig) { VARIANTS.with(|v| v.borrow_mut().push(("toggle", c.variant))); }
    fn segment_probe(_: &mut Ui, c: &crate::bodies::SegmentedConfig) { VARIANTS.with(|v| v.borrow_mut().push(("segment", c.variant))); }
    fn nested_label(_: &mut Ui, c: &crate::bodies::LabelConfig) { VARIANTS.with(|v| v.borrow_mut().push(("nested", c.variant))); }

    /// A menu's variant reaches every row's body, a toggle's and a segmented control's reach theirs (every segment); the contents of a button with a variant are given none.
    #[test]
    fn a_variant_reaches_every_menu_row_and_never_a_nested_widget() {
        use crate::{ui::{Menu, MenuItem}, Button, Label};
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        ui.register_menu_row_body("rows", menu_row).expect("row body");
        ui.register_label_body("nested", nested_label).expect("label body");
        ui.register_toggle_body("t", toggle_probe).expect("toggle body");
        ui.register_segmented_body("s", segment_probe).expect("segment body");
        let scoped = crate::ThemeHandle::parse("menu_row.body = rows\nlabel.body = nested\ntoggle.body = t\nsegmented.body = s").expect("theme");
        const ITEMS: [MenuItem; 3] = [MenuItem::new("A"), MenuItem::new("B"), MenuItem::new("C")];
        let mut build = |ui: &mut Ui| ui.themed(&scoped, &mut |ui: &mut Ui| {
            ui.menu("menu", crate::Point::new(10.0, 10.0), &ITEMS, &Menu::DEFAULT.variant(2));
            ui.menu("plain", crate::Point::new(100.0, 10.0), &ITEMS, &Menu::DEFAULT);
            // the built-in button body runs the contents; the registered label inside is given no variant
            ui.button_content("pill", &Button::DEFAULT.variant(1).size(Size::Px(60.0), Size::Px(30.0)).name("Pill"), |ui, _| { ui.label("inside", Label::DEFAULT); });
            ui.label("after", Label::DEFAULT.variant(5));
            ui.toggle("t", "T", true, &crate::Toggle::DEFAULT.variant(4));
            ui.toggle("t0", "T", true, &crate::Toggle::DEFAULT);
            ui.segmented("s", &[crate::Segment::new("a"), crate::Segment::new("b")], Some(0), &crate::Segmented::DEFAULT.variant(6));
        });
        for n in 0..3 { VARIANTS.with(|v| v.borrow_mut().clear()); ui.frame(&input(n as f64), &mut build).expect("frame"); }
        let seen = VARIANTS.with(|v| v.borrow().clone());
        let rows: Vec<u16> = seen.iter().filter(|s| s.0 == "row").map(|s| s.1).collect();
        assert_eq!(rows, [2, 2, 2, 0, 0, 0], "the menu's three rows, then the plain menu's: {seen:?}");
        let others: Vec<(&str, u16)> = seen.iter().filter(|s| s.0 == "toggle" || s.0 == "segment").copied().collect();
        assert_eq!(others, [("toggle", 4), ("toggle", 0), ("segment", 6), ("segment", 6)], "a toggle's and every segment's body: {seen:?}");
        let nested: Vec<u16> = seen.iter().filter(|s| s.0 == "nested").map(|s| s.1).collect();
        assert_eq!(nested, [0, 5], "the label in the button's contents sees 0, the label with its own variant 5: {seen:?}");
    }

    fn scroll_probe(ui: &mut Ui, c: &crate::bodies::ScrollViewConfig) {
        VARIANTS.with(|v| v.borrow_mut().push(("scroll", c.variant)));
        ui.slot(c.content);
    }

    /// A menu that scrolls (`max_rows`) scrolls its sheet and builds no `scroll_view` inside its call (ABI 1.58); the pending variant is for
    /// the menu's rows, so every row sees it.
    #[test]
    fn a_menu_variant_is_for_its_rows_and_a_long_menu_builds_no_scroll_view() {
        use crate::ui::{Menu, MenuItem};
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        ui.register_menu_row_body("rows", menu_row).expect("row body");
        ui.register_scroll_view_body("scroll", scroll_probe).expect("scroll body");
        let scoped = crate::ThemeHandle::parse("menu_row.body = rows\nscroll_view.body = scroll").expect("theme");
        const ITEMS: [MenuItem; 5] = [MenuItem::new("A"), MenuItem::new("B"), MenuItem::new("C"), MenuItem::new("D"), MenuItem::new("E")];
        let mut build = |ui: &mut Ui| ui.themed(&scoped, &mut |ui: &mut Ui| { ui.menu("menu", crate::Point::new(10.0, 10.0), &ITEMS, &Menu::DEFAULT.max_rows(2).variant(1)); });
        for n in 0..3 { VARIANTS.with(|v| v.borrow_mut().clear()); ui.frame(&input(n as f64), &mut build).expect("frame"); }
        let seen = VARIANTS.with(|v| v.borrow().clone());
        assert!(!seen.iter().any(|s| s.0 == "scroll"), "the sheet scrolls, no scroll view body runs: {seen:?}");
        let rows: Vec<u16> = seen.iter().filter(|s| s.0 == "row").map(|s| s.1).collect();
        assert!(!rows.is_empty() && rows.iter().all(|v| *v == 1), "every row sees 1: {seen:?}");
    }

    /// A variant a caller left pending between frames is gone when the next frame starts.
    #[test]
    fn a_pending_variant_does_not_survive_into_the_next_frame() {
        use pxui_types::theme::BodyKind;
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        ui.register_label_body("nested", nested_label).expect("label body");
        let scoped = crate::ThemeHandle::parse("label.body = nested").expect("theme");
        ui.set_variant(BodyKind::label, 9);
        VARIANTS.with(|v| v.borrow_mut().clear());
        ui.frame(&input(0.0), &mut |ui: &mut Ui| ui.themed(&scoped, &mut |ui: &mut Ui| { ui.label("x", crate::Label::DEFAULT); })).expect("frame");
        assert_eq!(VARIANTS.with(|v| v.borrow().clone()), [("nested", 0)]);
    }
}
