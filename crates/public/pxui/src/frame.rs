use crate::{Appearance, Environment, Input, Keyboard, WireFrame};

/// The input to one UI frame. Event frames use the wire record. A host with a complete
/// input snapshot can supply that record instead.
#[derive(Clone, Copy, Debug)]
pub struct FrameIn<'a> {
    /// The event-mode record. Keep all of its event records and their payloads alive
    /// through the frame call. The snapshot source carries its own complete input.
    pub wire: pxui_types::FrameIn,
    pub(crate) snapshot: Option<&'a Input>,
}

impl Default for FrameIn<'_> {
    fn default() -> Self { Self::new(0.0, 0.0) }
}

impl<'a> FrameIn<'a> {
    /// Creates an event frame for a surface of `width` by `height` logical points.
    ///
    /// Starts with no events, time zero and scale one. It uses the Ui's retained input
    /// state to interpret the ordered events the window supplies.
    /// Allocates: never.
    /// Panics: never. The frame call validates the input.
    ///
    /// ```
    /// let frame = pxui::FrameIn::new(320.0, 240.0);
    /// assert_eq!((frame.wire.width, frame.wire.height, frame.wire.scale), (320.0, 240.0, 1.0));
    /// ```
    pub const fn new(width: f32, height: f32) -> FrameIn<'a> {
        FrameIn { wire: WireFrame::new(width, height), snapshot: None }
    }

    /// Sets the event frame's drawable pixels per logical point.
    ///
    /// Allocates: never.
    /// Panics: never. The frame call validates the scale.
    ///
    /// ```
    /// assert_eq!(pxui::FrameIn::new(10.0, 10.0).scale(2.0).wire.scale, 2.0);
    /// ```
    pub const fn scale(mut self, scale: f32) -> FrameIn<'a> { self.wire.scale = scale; self }

    /// Sets the event frame's time in seconds on the host's monotonic clock.
    ///
    /// Allocates: never.
    /// Panics: never. The frame call validates the timestamp.
    ///
    /// ```
    /// assert_eq!(pxui::FrameIn::new(10.0, 10.0).time(0.25).wire.time, 0.25);
    /// ```
    pub const fn time(mut self, time: f64) -> FrameIn<'a> { self.wire.time = time; self }

    /// Reports the host appearance for an event frame.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{Appearance, FrameIn};
    /// assert_eq!(FrameIn::new(10.0, 10.0).appearance(Appearance::Dark).wire.appearance, Appearance::Dark as u32);
    /// ```
    pub const fn appearance(mut self, appearance: Appearance) -> FrameIn<'a> { self.wire.appearance = appearance as u32; self }

    /// Reports the host's reduced-motion setting for an event frame.
    ///
    /// This applies in every theme scope.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui::FrameIn::new(10.0, 10.0).reduced_motion(true).wire.reduced_motion, 1);
    /// ```
    pub const fn reduced_motion(mut self, on: bool) -> FrameIn<'a> { self.wire.reduced_motion = on as u32; self }

    /// Sets safe-area insets and keyboard animation data for an event frame in logical points.
    ///
    /// Allocates: never.
    /// Panics: never. The frame call validates the geometry.
    ///
    /// ```
    /// use pxui::{Environment, FrameIn};
    /// let environment = Environment { safe_area: [10.0, 0.0, 0.0, 0.0], ..Environment::default() };
    /// assert_eq!(FrameIn::new(20.0, 20.0).environment(environment).wire.environment.safe_area[0], 10.0);
    /// ```
    pub const fn environment(mut self, environment: Environment) -> FrameIn<'a> { self.wire.environment = environment; self }

    /// Sets the current native keyboard presentation geometry for an event frame in logical points.
    ///
    /// Allocates: never.
    /// Panics: never. The frame call validates the geometry.
    ///
    /// ```
    /// use pxui::{FrameIn, Keyboard};
    /// let keyboard = Keyboard { frame: [0.0, 10.0, 20.0, 10.0], flags: Keyboard::VALID };
    /// assert_eq!(FrameIn::new(20.0, 20.0).keyboard(keyboard).wire.keyboard, keyboard);
    /// ```
    pub const fn keyboard(mut self, keyboard: Keyboard) -> FrameIn<'a> { self.wire.keyboard = keyboard; self }
}
