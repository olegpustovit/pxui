//! Widget bodies: an app builds a widget kind's nodes itself. `Ui::register_toggle_body(name, f)` and its siblings (one per kind,
//! generated from `pxui_types::body_kinds!`) register `f`, a plain `fn(&mut Ui, &Config)`, through the one export
//! `pxui_body_register`; a theme (`theme.body.toggle = id`, or `toggle.body = NAME` in a theme file applied with `Theme::apply`) then picks
//! it. The body runs as a callback from the library under the callback panic rule: a panic in it is carried to the call that can run
//! callbacks (the frame) and the body calls after it are skipped. The config is the kind's C record: borrowed until the body returns,
//! text fields are read with `text_of`, handles are `PxuiBox`es the body may restyle and enter, and slots run the caller's content.
use crate::composite::{defer_callback_panic, deferred};
use crate::{as_str, check, Ui};
use core::{ffi::c_void, mem::ManuallyDrop, ptr::NonNull};
use pxui_types::{geom::Rect, theme::{BodyId, BodyKind, Part}, ffi::{PxuiBodyFn, PxuiSlot}, Node, PxuiBox, Str};

pub use pxui_types::ffi::{PxuiToggleConfig, PxuiSliderConfig, PxuiProgressConfig, PxuiButtonConfig, PxuiIconButtonConfig, PxuiCheckConfig, PxuiSpinnerConfig, PxuiBadgeConfig, PxuiLabelConfig, PxuiFooterConfig, PxuiGripConfig, PxuiPanelConfig, PxuiDialogConfig, PxuiSectionConfig, PxuiInspectorRowConfig, PxuiSplitConfig, PxuiScrollConfig, PxuiToastConfig, PxuiTipConfig, PxuiTabConfig, PxuiSegmentConfig, PxuiDropdownConfig, PxuiMenuItemConfig, PxuiMenuRowConfig, PxuiRowConfig, PxuiTreeRowConfig, PxuiTableHeadConfig, PxuiTableCellConfig, PxuiPickerRowConfig, PxuiSearchFieldConfig, PxuiTextFieldConfig, PxuiLineFieldConfig, PxuiValueFieldConfig, PxuiPickerFieldConfig};

library!(sys {
    fn pxui_body_register(ui: *mut PxuiUi, kind: u32, name: Str, body: PxuiBodyFn, user: *mut c_void) -> u16;
    fn pxui_part(ui: *mut PxuiUi, part: u32, node: *const Node, out: *mut PxuiBox) -> i32;
    fn pxui_part_box(ui: *mut PxuiUi, owner: i64, part: u32, out: *mut PxuiBox) -> i32;
    #[trusted] fn pxui_slot(ui: *mut PxuiUi, slot: PxuiSlot, out: *mut u8) -> i32;
});

/// The text of a config field, borrowed from the config (valid until the body returns).
pub fn text_of(value: &Str) -> &str {
    if value.len == 0 || value.ptr.is_null() { return ""; }
    core::str::from_utf8(unsafe { core::slice::from_raw_parts(value.ptr, value.len) }).unwrap_or("")
}

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
    ($($kind:ident $register:ident $body:ident $config:ident),* $(,)?) => {
        impl Ui {
            $(
                #[doc = concat!("Registers `body` as the ", stringify!($kind), " body called `name`, for a theme's `body.", stringify!($kind), "` (`", stringify!($kind), ".body = NAME` in a theme file). Registering a name again replaces its function and keeps its id; the error is the library's reason (`default` and names with a space, `=` or `#` are refused, and so is a call inside a frame).")]
                pub fn $register(&mut self, name: &str, body: fn(&mut Ui, &$config)) -> Result<BodyId, String> {
                    extern "C" fn entry(user: *mut c_void, raw: *mut c_void, config: *const c_void) {
                        enter(raw, config, &mut |ui| (unsafe { core::mem::transmute::<*mut c_void, fn(&mut Ui, &$config)>(user) })(ui, unsafe { &*config.cast::<$config>() }));
                    }
                    self.register_body(BodyKind::$kind, name, entry, body as *mut c_void)
                }
            )*
        }
    };
}
pxui_types::body_kinds!(typed);

impl Ui {
    fn register_body(&mut self, kind: BodyKind, name: &str, entry: extern "C" fn(*mut c_void, *mut c_void, *const c_void), user: *mut c_void) -> Result<BodyId, String> {
        match unsafe { sys::pxui_body_register(self.0.as_ptr(), kind as u32, as_str(name), Some(entry), user) } {
            0 => Err(self.last_error().to_owned()),
            id => Ok(BodyId(id)),
        }
    }

    /// Builds a node as `part` of the widget whose body is running: behaviour finds it again by role (see the config's documentation
    /// for the parts its widget reads). Only inside a body.
    #[track_caller]
    pub fn part(&mut self, part: Part, node: Node) -> PxuiBox {
        let mut out = PxuiBox::default();
        check(self, unsafe { sys::pxui_part(self.0.as_ptr(), part.0, &node, &mut out) });
        out
    }

    /// The box of `part` of the widget keyed `owner`, as the last build left it; `None` if its body built none.
    pub fn part_box(&self, owner: i64, part: Part) -> Option<PxuiBox> {
        let mut out = PxuiBox::default();
        (unsafe { sys::pxui_part_box(self.0.as_ptr(), owner, part.0, &mut out) } == 0).then_some(out)
    }

    /// The rectangle `part` of the widget keyed `owner` had last frame; `None` before it has been laid out or if the body builds none.
    pub fn part_rect(&self, owner: i64, part: Part) -> Option<Rect> {
        self.part_box(owner, part).map(|b| self.rect(&b)).filter(|r| r.w > 0.0 && r.h > 0.0)
    }

    /// Runs a config's slot (the caller's content, built where the body calls this); for a search field's editor slot the answer is
    /// whether the query is not empty, otherwise false. A slot is valid only during the body call that received it: a stale one panics.
    #[track_caller]
    pub fn slot(&mut self, slot: PxuiSlot) -> bool {
        let mut out = 0u8;
        check(self, unsafe { sys::pxui_slot(self.0.as_ptr(), slot, &mut out) });
        out != 0
    }
}
