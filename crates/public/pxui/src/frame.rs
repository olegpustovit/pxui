use crate::{Appearance, Environment, Event, Keyboard, WireFrame};

/// The surface, events, geometry and preferences for a UI frame.
#[derive(Clone, Copy, Debug)]
pub struct FrameIn<'a> {
    /// The event-mode record. Keep all of its event records and their payloads alive
    /// through the frame call.
    pub wire: pxui_types::FrameIn,
    /// The frame's events, set with `events()`.
    pub(crate) events: &'a [Event],
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
        FrameIn { wire: WireFrame::new(width, height), events: &[] }
    }

    /// Names the surface this frame is for (default `pxui_types::FrameIn::MAIN`); it must be registered on the Ui and carried by every event.
    ///
    /// Allocates: never.
    /// Panics: never. The frame call validates the surface.
    ///
    /// ```
    /// let mut ui = pxui::Ui::new(&pxui::UiDesc::DEFAULT).unwrap();
    /// assert!(ui.frame(&pxui::FrameIn::new(10.0, 10.0).surface(99), &mut |_| {}).is_err());
    /// ```
    pub const fn surface(mut self, surface: u64) -> FrameIn<'a> { self.wire.surface = surface; self }

    /// Sets the frame's events, borrowed for the frame call (text and touch payloads included).
    ///
    /// At most 4096; each carries the frame's surface (`Event::surface`, `FrameIn::MAIN` by default). The borrow ends with the frame
    /// call, so no event pointer can dangle; whatever `wire.events` holds is ignored.
    /// Allocates: never.
    /// Panics: never. The frame call validates every event.
    ///
    /// ```
    /// let events = [pxui::Event::new(pxui::Event::FOCUS_GAINED)];
    /// let mut ui = pxui::doc::frames(1, &mut |_| {});
    /// ui.frame(&pxui::FrameIn::new(10.0, 10.0).events(&events), &mut |_| {}).unwrap();
    /// ```
    pub const fn events(mut self, events: &'a [Event]) -> FrameIn<'a> { self.events = events; self }

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
    /// let environment = Environment::new([10.0, 0.0, 0.0, 0.0]);
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
    /// let keyboard = Keyboard::new(pxui_types::geom::Rect::new(0.0, 10.0, 20.0, 10.0), Keyboard::VALID);
    /// assert_eq!(FrameIn::new(20.0, 20.0).keyboard(keyboard).wire.keyboard, keyboard);
    /// ```
    pub const fn keyboard(mut self, keyboard: Keyboard) -> FrameIn<'a> { self.wire.keyboard = keyboard; self }
}


/// Authoritative host state after the ordered events supplied in [FrameIn].
#[derive(Clone, Copy, Debug)]
pub struct InputSnapshot<'a> {
    /// Held key codes.
    pub keys: &'a [u32],
    /// Held pointer button codes.
    pub buttons: &'a [u32],
    /// Pointer position in logical points.
    pub pointer: [f64; 2],
    /// Pointer movement since the previous frame in logical points.
    pub mouse_delta: [f64; 2],
    /// Drawable pixels per logical point on each axis.
    pub pixel_scale: [f64; 2],
    /// Elapsed seconds since the previous frame.
    pub dt: f64,
    /// Frame serial.
    pub frame: i64,
    /// Input update serial.
    pub tick: i64,
    /// Frame rate in frames per second.
    pub fps: f64,
}
