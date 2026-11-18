//! Typed keys for frame queries and test input.

/// A character or named key from the host's keyboard events.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyCode {
    /// One Unicode scalar, matched with the case reported by the host.
    Char(char),
    /// The up arrow.
    ArrowUp,
    /// The down arrow.
    ArrowDown,
    /// The left arrow.
    ArrowLeft,
    /// The right arrow.
    ArrowRight,
    /// The space bar.
    Space,
    /// Return or Enter.
    Enter,
    /// Escape.
    Escape,
    /// Backspace.
    Backspace,
    /// Tab.
    Tab,
    /// Shift.
    Shift,
    /// Control.
    Ctrl,
    /// Alt or Option.
    Alt,
    /// Command or the Super key.
    Meta,
    /// Function key F1.
    F1,
    /// Function key F2.
    F2,
    /// Function key F3.
    F3,
    /// Function key F4.
    F4,
    /// Function key F5.
    F5,
    /// Function key F6.
    F6,
    /// Function key F7.
    F7,
    /// Function key F8.
    F8,
    /// Function key F9.
    F9,
    /// Function key F10.
    F10,
    /// Function key F11.
    F11,
    /// Function key F12.
    F12,
    /// Home.
    Home,
    /// End.
    End,
    /// Page Up.
    PageUp,
    /// Page Down.
    PageDown,
    /// Insert.
    Insert,
    /// Forward Delete.
    Delete,
}

impl KeyCode {
    pub(crate) const fn wire(self) -> u32 {
        let index = match self {
            Self::Char(ch) => return ch as u32,
            Self::ArrowUp => 0,
            Self::ArrowDown => 1,
            Self::ArrowLeft => 2,
            Self::ArrowRight => 3,
            Self::Space => 4,
            Self::Enter => 5,
            Self::Escape => 6,
            Self::Backspace => 7,
            Self::Tab => 8,
            Self::Shift => 9,
            Self::Ctrl => 10,
            Self::Alt => 11,
            Self::Meta => 12,
            Self::F1 => 13,
            Self::F2 => 14,
            Self::F3 => 15,
            Self::F4 => 16,
            Self::F5 => 17,
            Self::F6 => 18,
            Self::F7 => 19,
            Self::F8 => 20,
            Self::F9 => 21,
            Self::F10 => 22,
            Self::F11 => 23,
            Self::F12 => 24,
            Self::Home => 25,
            Self::End => 26,
            Self::PageUp => 27,
            Self::PageDown => 28,
            Self::Insert => 29,
            Self::Delete => 30,
        };
        pxui_types::KEY_SPECIAL | index
    }
}

/// The modifier keys required at the instant of a key press.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Mods(pub(crate) u32);

impl Mods {
    /// Accepts a press with any modifier state.
    pub const ANY: Mods = Mods(1 << 4);
    /// Accepts a press only when no modifier was held.
    pub const NONE: Mods = Mods(0);
    /// Requires Command or the Super key.
    pub const META: Mods = Mods(1 << 3);
    /// Requires Shift.
    pub const SHIFT: Mods = Mods(1);
    /// Requires Alt or Option.
    pub const ALT: Mods = Mods(1 << 2);
    /// Requires Control.
    pub const CTRL: Mods = Mods(1 << 1);

    /// Requires the keys from both masks at the same press.
    ///
    /// `ANY` accepts all modifier states and takes precedence in a combination.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::Mods;
    /// assert_eq!(Mods::SHIFT.with(Mods::NONE), Mods::SHIFT);
    /// assert_eq!(Mods::META.with(Mods::ANY), Mods::ANY);
    /// ```
    pub const fn with(self, other: Mods) -> Mods {
        let mask = self.0 | other.0;
        if mask & Self::ANY.0 != 0 { Self::ANY } else { Self(mask) }
    }
}
