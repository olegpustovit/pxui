use crate::PxuiThemeSnapshot;
use crate::state::{PxuiUi, get};
use pxui_core::theme::Tokens;
use pxui_types::theme::{Font, Look, Shadow};
use pxui_types::ffi::{PxuiFontDesc, PxuiLookDesc, PxuiMotion, PxuiRingDesc, PxuiShadowDesc, ThemeTokens};

pub(crate) fn tokens(t: Tokens) -> ThemeTokens {
    ThemeTokens {
        panel:t.panel, foreground:t.foreground, control:t.control, input:t.input, track:t.track, accent:t.accent,
        ink_ground:t.ink_ground.unwrap_or_default(), has_ink_ground:t.ink_ground.is_some() as u8, reserved:[0;3],
        space:t.space, radius:t.radius, display_size:t.display_size, title_size:t.title_size,
        font_size:t.font_size, label_size:t.label_size,
        motion:PxuiMotion { fast:t.motion.fast, normal:t.motion.normal, slow:t.motion.slow, ease:t.motion.ease },
        reduced_motion:t.reduced_motion as u8, reserved_tail:[0;3], density:t.density,
    }
}

fn font(v: Font) -> PxuiFontDesc {
    PxuiFontDesc { family:v.family, weight:v.weight, size:v.size, tracking:v.tracking, caps:v.caps as u8, reserved:[0;3] }
}

fn shadow(v: Shadow) -> PxuiShadowDesc {
    PxuiShadowDesc { color:v.color, offset:[v.offset.0,v.offset.1], blur:v.blur, spread:v.spread, inner:v.inner as u8, reserved:[0;3] }
}

fn look(v: Look) -> PxuiLookDesc {
    PxuiLookDesc {
        fill:v.fill, gradient:v.gradient.unwrap_or_default(), has_gradient:v.gradient.is_some() as u8, reserved:[0;3], angle:v.angle,
        border:v.border, border_w:v.border_w, sides:v.sides as u32, radius:v.radius,
        shadows:[shadow(v.shadows[0]),shadow(v.shadows[1])], text:v.text, font:font(v.font), opacity:v.opacity,
        ring:PxuiRingDesc { color:v.ring.color, width:v.ring.width, sides:v.ring.sides as u32 },
        selection:v.selection, caret:v.caret, dashed:v.dashed as u8, reserved_tail:[0;3],
    }
}

export!(pub unsafe fn pxui_ui_theme(ui:*mut PxuiUi) -> PxuiThemeSnapshot {
    let Some(ui)=(unsafe{get(ui)}) else { return unsafe{core::mem::zeroed()}; };
    let engine=unsafe{&*ui.engine};
    let theme=engine.theme();
    PxuiThemeSnapshot {
        size:core::mem::size_of::<PxuiThemeSnapshot>() as u32, reserved:0,
        tokens:tokens(theme.tokens), value:look(theme.value.idle), hint:look(theme.hint.idle),
        field:look(theme.field.idle), footer:look(theme.footer.idle),
    }
});
