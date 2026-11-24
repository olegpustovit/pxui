//! Enable SDL resizing before configuring AppKit full-size content. Chrome uses the live Metal
//! layer and measures contentLayoutRect only on startup/native size change, never during UI builds.
//! Preserve native zoom/full-screen controls and the platform default app stack. Cache
//! appearance/reduced-motion notifications and wake only for changed facts; remove observers before
//! window destruction. Never poll OS/defaults in frames. Dialog callbacks publish copied outcomes
//! before waking idle hosts for success, cancellation and failure.
//!
//! The SDL3 window host: window lifecycle, the event pump translated to `input` events, cursor,
//! text input, clipboard and the native layer token a GPU backend presents to. A leaf: it knows
//! nothing of the UI engine or the renderer; callers pass plain values.
//!
//! Many windows per process: SDL's one process queue is drained by whichever window pumps and routed
//! by SDL window id (`router`), so each `Window::pump` returns only its own window's events. Focus,
//! text input (IME) and dialog outcomes are per window; the clipboard is the system's (per seat).
//!
//! Per-process state, all on the initial thread: the router (one queue per open window, 4096 events
//! each), SDL's dialog table (8 open dialogs; each outcome returns to the window that opened it), the
//! macOS trackpad monitor (a 256-sample ring) and `sdl3::event`'s one live-resize callback, which is
//! given the resized window's id to dispatch by.
//!
//! Threading: SDL, every `Window` and the router belong to the initial thread; only `wake` may be
//! called from others.
pub use sdl3::{FileSelection, TextInputOptions};

use sdl3::cursor::{Cursor, Shape};
use sdl3::window::{Flag, Window as SdlWindow};
use sdl3::{DialogFilter as SdlDialogFilter, DialogKind as SdlDialogKind, NativeView, Rect};
use std::time::Instant;

#[cfg(target_os = "macos")]
pub use sdl3::chrome::{Chrome, Snapshot as ChromeSnapshot};
#[cfg(target_os = "macos")]
pub use sdl3::ax;
#[cfg(all(target_os = "macos", feature = "test-util"))]
pub use sdl3::pacing::test_order_front;

pub mod translate;
mod drops;
mod router;
pub mod app;
#[cfg(target_os = "ios")]
pub mod ios;

pub use native_layer_token_sys::portable::Token;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use sdl3::preferences::{Preferences, Snapshot as PreferencesSnapshot};
/// Native binary startup policy; does not acquire SDL subsystems or change their ownership.
pub use sdl3::init::configure_native_process;
#[cfg(target_os = "ios")]
pub use sdl3::init::run_app;

/// Thread-safe; the caller must retain a live window until its workers have stopped.
pub fn wake() -> bool { sdl3::event::wake() }

pub type Error = String;

/// Most failures a window keeps between `take_failure` calls; later ones are counted.
pub const FAILURE_CAP: usize = 8;

/// Window size in logical points and drawable size in pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Extent {
    pub logical: (i64, i64),
    pub drawable: (i64, i64),
}

/// SDL announces a change of size or density with an event instead of making every frame ask.
/// The announcement is one bit: set by the event, taken by the next frame, which re-reads the
/// window only if it was set.
pub mod change_flag {
    #[derive(Debug)]
    pub struct ChangeFlag {
        announced: bool,
    }

    pub fn create() -> ChangeFlag {
        ChangeFlag { announced: false }
    }

    pub fn announce(flag: &mut ChangeFlag) {
        flag.announced = true;
    }

    /// Clears the announcement and says whether there was one.
    pub fn take(flag: &mut ChangeFlag) -> bool {
        std::mem::replace(&mut flag.announced, false)
    }

    /// `query` runs once per announcement, however many arrived; true when it ran.
    pub fn refresh(flag: &mut ChangeFlag, query: &mut dyn FnMut()) -> bool {
        let announced = take(flag);
        if announced {
            query();
        }
        announced
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorShape {
    Default,
    HorizontalResize,
    VerticalResize,
    Text,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogKind {
    OpenFile,
    OpenFiles,
    SaveFile,
    OpenFolder,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DialogFilter {
    pub name: String,
    pub pattern: String,
}

pub struct Window {
    handle: SdlWindow,
    /// SDL's id for the window: the key the process router delivers its events by
    id: u32,
    view: NativeView,
    #[cfg(target_os = "macos")]
    pacing: Option<sdl3::pacing::Pacing>,
    #[cfg(target_os = "macos")]
    offscreen: bool,
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    repaint_hz: u32,
    vsync: bool,
    cursors: Vec<(CursorShape, Cursor)>,
    cursor_shape: Option<CursorShape>,
    text_input: bool,
    text_input_options: TextInputOptions,
    ime_area: Option<((i64, i64, i64, i64), i64)>,
    changed: change_flag::ChangeFlag,
    /// window coordinates per logical point: SDL's display scale over its pixel density
    unit: f64,
    extent: Extent,
    started: Instant,
    frame_time: f64,
    dead: bool,
    /// event-poll and extent re-read failures since `take_failure` (the next pump tries again), at most
    /// `FAILURE_CAP`; later ones are only counted in `failures_dropped`
    failures: Vec<Error>,
    failures_dropped: usize,
    /// events this window lost since `take_failure`
    lost: router::Lost,
    polled: Vec<sdl3::Event>,
    drops: drops::Drops,
    dialog_selections: Vec<(i64, Vec<FileSelection>)>,
}

fn fail(failure: sdl3::Error) -> Error {
    failure.to_string()
}

/// Offers `text` on the system clipboard. SDL's clipboard belongs to the seat, not to a window, so
/// every window of the process shares it.
pub fn clipboard_set_text(text: &str) -> Result<(), Error> {
    sdl3::clipboard::set_text(text).map_err(fail)
}

/// The system clipboard's text (the seat's, shared by every window).
pub fn clipboard_get_text() -> Result<String, Error> {
    sdl3::clipboard::get_text().map_err(fail)
}

// macOS reports window coordinates in points (display scale 2 over density 2: one coordinate a
// point); Windows reports pixels with density 1 and the monitor's scaling as the display scale
// (1.5 at 150%). Rays lays out in points, so window coordinates are divided by that ratio here
// and multiplied back where a size or a rect goes to SDL.
fn window_unit(window: &SdlWindow) -> Result<f64, Error> {
    let scale = window.display_scale().map_err(fail)? as f64;
    let density = window.pixel_density().map_err(fail)? as f64;
    let unit = scale / density;
    Ok(if (unit - 1.).abs() < 1e-3 { 1. } else { unit })
}

// Round halfway values away from zero.
fn to_points(unit: f64, size: i32) -> i64 {
    std::cmp::max(1, ((size as f64) / unit).round() as i64)
}

fn to_window(unit: f64, size: i64) -> i32 {
    std::cmp::max(1, ((size as f64) * unit).round() as i64) as i32
}

fn extent_in(unit: f64, window: &SdlWindow) -> Result<Extent, Error> {
    let (lw, lh) = window.size().map_err(fail)?;
    let (dw, dh) = window.size_in_pixels().map_err(fail)?;
    if lw > 0 && lh > 0 && dw > 0 && dh > 0 {
        Ok(Extent { logical: (to_points(unit, lw), to_points(unit, lh)), drawable: (dw as i64, dh as i64) })
    } else {
        Err("window dimensions are invalid".to_owned())
    }
}

// Every step is checked: the first failure is the result.
fn reveal(window: &SdlWindow) -> Result<(), sdl3::Error> {
    window.show()?;
    #[cfg(not(target_os = "ios"))]
    {
        window.restore()?;
        window.raise_window()?;
        window.center()?;
    }
    window.sync()
}

impl Window {
    /// Cached system facts; drop the monitor before destroying this window.
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    pub fn preferences(&self, wake: extern "C" fn()) -> Result<Preferences, String> {
        self.live()?;
        Preferences::open(self.handle.id().map_err(|e| e.to_string())?, wake)
            .map_err(|e| e.to_string())
    }
    /// A visible, vsynced, high-density window.
    pub fn open(title: &str, width: i64, height: i64) -> Result<Window, Error> {
        Window::open_with(title, width, height, true, false, true)
    }

    /// `high_density` asks for a drawable at the display's native density; false keeps the
    /// drawable at the logical size, so a capture is the same bytes on every display.
    pub fn open_with(title: &str, width: i64, height: i64, vsync: bool, hidden: bool, high_density: bool) -> Result<Window, Error> {
        if width <= 0 || height <= 0 {
            return Err("dimensions must be positive".to_owned());
        }
        sdl3::init::init(&[sdl3::init::Subsystem::Video]).map_err(fail)?;
        // The window and view release themselves when `build` returns; a failure then quits video.
        let build = || -> Result<Window, Error> {
            // Typed touch owns gesture arbitration; emulated mouse/touch would deliver it twice.
            sdl3::hint::set("SDL_TOUCH_MOUSE_EVENTS", "0").map_err(fail)?;
            sdl3::hint::set("SDL_MOUSE_TOUCH_EVENTS", "0").map_err(fail)?;
            // SDL itself turns a Control-click into the secondary click.
            sdl3::hint::control_click_is_right_click(true).map_err(fail)?;
            if cfg!(windows) {
                sdl3::hint::mouse_relative_system_scale(true).map_err(fail)?;
            }
            let mut flags = vec![Flag::NativeView];
            if high_density {
                flags.push(Flag::HighPixelDensity);
            }
            if hidden || cfg!(target_os = "ios") {
                flags.push(Flag::Hidden);
            }
            let handle = SdlWindow::create(title, width as i32, height as i32, &flags).map_err(fail)?;
            let unit = window_unit(&handle)?;
            if unit != 1. {
                handle.set_size(to_window(unit, width), to_window(unit, height)).map_err(fail)?;
            }
            #[cfg(not(target_os = "ios"))]
            if !hidden {
                reveal(&handle).map_err(fail)?;
            }
            let mut view = NativeView::create(&handle).map_err(fail)?;
            let _token = view.layer().map_err(fail)?;
            #[cfg(target_os = "ios")]
            if !hidden {
                reveal(&handle).map_err(fail)?;
            }
            // Size the target for the actual drawable once, rather than resizing it after.
            let extent = extent_in(unit, &handle)?;
            let id = handle.id().map_err(fail)?;
            router::register(id);
            Ok(Window {
                handle,
                id,
                view,
                #[cfg(target_os = "macos")]
                pacing: sdl3::pacing::Pacing::open(&_token),
                #[cfg(target_os = "macos")]
                offscreen: hidden,
                #[cfg(any(target_os = "macos", target_os = "ios"))]
                repaint_hz: 0,
                vsync,
                cursors: Vec::new(),
                cursor_shape: None,
                text_input: false,
                text_input_options: TextInputOptions::default(),
                ime_area: None,
                changed: change_flag::create(),
                unit,
                extent,
                started: Instant::now(),
                frame_time: 0.0,
                dead: false,
                failures: Vec::new(),
                failures_dropped: 0,
                lost: router::Lost::default(),
                polled: Vec::with_capacity(256),
                drops: drops::Drops::default(),
                dialog_selections: Vec::with_capacity(8),
            })
        };
        let created = build();
        if created.is_err() {
            let _ = sdl3::init::quit_subsystems(&[sdl3::init::Subsystem::Video]);
        }
        created
    }

    fn live(&self) -> Result<(), Error> {
        if self.dead { Err("window is destroyed".to_owned()) } else { Ok(()) }
    }

    /// Appends this window's events (positions in logical points) and says whether the window is still
    /// open: false once a close request or quit arrived. Events of the process's other windows stay
    /// queued for their own pumps.
    pub fn pump(&mut self, events: &mut Vec<input::Event>) -> bool {
        self.dialog_selections.clear();
        let mut open = true;
        match router::pump(self.id, &mut self.polled) {
            Err(error) => self.record(fail(error)),
            Ok(lost) => {
                self.lost.overflow += lost.overflow;
                self.lost.unrouted += lost.unrouted;
                for raw in self.polled.drain(..) {
                    if matches!(raw, sdl3::Event::Window(sdl3::WindowChange::Resized(..) | sdl3::WindowChange::PixelSizeChanged(..))) {
                        change_flag::announce(&mut self.changed);
                    }
                    let event = match raw {
                        sdl3::Event::Dialog { id, outcome: sdl3::DialogOutcome::Chosen(files) } => {
                            let paths = files.iter().map(|file| file.path().to_string_lossy().into_owned()).collect();
                            self.dialog_selections.push((i64::from(id), files));
                            Some(input::Event::FileDialog { id: i64::from(id), result: Ok(paths) })
                        }
                        raw => self.drops.translate(translate::in_points(self.unit, raw)),
                    };
                    if let Some(event) = event {
                        open &= !matches!(event, input::Event::Closed);
                        events.push(event);
                    }
                }
            }
        }
        // a failed re-read keeps the last extent, is reported by `take_failure` and is retried
        if let Err(error) = self.sync() {
            change_flag::announce(&mut self.changed);
            self.record(error);
        }
        open
    }

    fn record(&mut self, failure: Error) {
        if self.failures.len() < FAILURE_CAP {
            self.failures.push(failure);
        } else {
            self.failures_dropped += 1;
        }
    }

    /// Everything that went wrong since the last call, as one message, taken: failures to re-read the
    /// extent after a resize (the extent kept the last good one) or to poll events, in order (at most
    /// `FAILURE_CAP`, then a count), and how many events this window lost: past the router's
    /// 4096-event queue while it went unpumped, or window-less events that reached no window because
    /// two or more windows were open and none had focus. `None` when nothing went wrong.
    pub fn take_failure(&mut self) -> Option<Error> {
        let lost = std::mem::take(&mut self.lost);
        let more = std::mem::take(&mut self.failures_dropped);
        if self.failures.is_empty() && more == 0 && lost == router::Lost::default() {
            return None;
        }
        let mut parts: Vec<String> = self.failures.drain(..).collect();
        if more > 0 {
            parts.push(format!("{more} more failures"));
        }
        if lost.overflow > 0 {
            parts.push(format!("{} events dropped: the window was not pumped for {} events", lost.overflow, router::QUEUE_CAP));
        }
        if lost.unrouted > 0 {
            parts.push(format!("{} window-less events reached no window: several windows open and none focused", lost.unrouted));
        }
        Some(parts.join("; "))
    }

    /// Sleeps until an event arrives or `deadline` (seconds on the `time` clock) passes; `None` waits
    /// for an event. The events stay queued for the next `pump` (`pxui_wayland::Window::wait`).
    pub fn wait(&mut self, deadline: Option<f64>) {
        self.wait_for_event(deadline);
    }

    fn wait_for_event(&mut self, deadline: Option<f64>) -> bool {
        // another window's pump may already have routed events here
        if router::pending(self.id) {
            return true;
        }
        let ms = deadline.map_or(i32::MAX, |at| ((at - self.time()).max(0.0) * 1000.0).ceil().min(f64::from(i32::MAX)) as i32);
        sdl3::event::wait(ms).unwrap_or(false)
    }

    /// SDL's id for this window: the surface id its events are routed by, unique for the process's life.
    pub fn id(&self) -> u32 {
        self.id
    }

    /// Logical size in points.
    pub fn size(&self) -> (i64, i64) {
        self.extent.logical
    }

    pub fn extent(&self) -> Extent {
        self.extent
    }

    /// Drawable pixels per logical point.
    pub fn scale(&self) -> f64 {
        self.extent.drawable.0 as f64 / self.extent.logical.0 as f64
    }

    /// Seconds since the window opened.
    pub fn time(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }

    pub fn vsync(&self) -> bool {
        self.vsync
    }

    /// Window coordinates per logical point: 1 on macOS, the monitor's scaling (1.5 at 150%) on
    /// Windows, whose SDL events and sizes are in pixels.
    pub fn unit(&self) -> f64 {
        self.unit
    }

    /// The native layer a GPU backend presents to.
    pub fn token(&mut self) -> Result<Token, Error> {
        self.live()?;
        self.view.layer().map_err(fail)
    }

    /// Drawable pixels per logical point as the display reports it.
    pub fn pixel_density(&self) -> Result<f64, Error> {
        Ok(self.handle.pixel_density().map_err(fail)? as f64 * self.unit)
    }

    /// Tell the window that SDL announced a size or density change, so `sync` re-reads it.
    /// The window is not polled between announcements.
    pub fn announce_change(&mut self) {
        change_flag::announce(&mut self.changed);
    }

    /// Re-reads unit and extent if a change was announced; no SDL call otherwise.
    pub fn sync(&mut self) -> Result<(), Error> {
        if self.dead || !change_flag::take(&mut self.changed) {
            return Ok(());
        }
        self.unit = window_unit(&self.handle)?;
        self.extent = extent_in(self.unit, &self.handle)?;
        Ok(())
    }

    /// Resizes to `width`x`height` logical points.
    pub fn resize(&mut self, width: i64, height: i64) -> Result<(), Error> {
        self.live()?;
        if width <= 0 || height <= 0 {
            return Err("dimensions must be positive".to_owned());
        }
        self.handle.set_size(to_window(self.unit, width), to_window(self.unit, height)).map_err(fail)?;
        self.extent = extent_in(self.unit, &self.handle)?;
        Ok(())
    }

    pub fn set_resizable(&mut self, enabled: bool) -> Result<(), Error> {
        self.live()?;
        self.handle.set_resizable(enabled).map_err(fail)
    }

    /// The native window's own background (sRGB, 0..1): what the title bar shows, since a Rays
    /// window hides its title and keeps only the traffic lights.
    pub fn set_background(&mut self, color: (f64, f64, f64)) -> Result<(), Error> {
        self.live()?;
        self.handle.set_background(color.0 as f32, color.1 as f32, color.2 as f32).map_err(fail)
    }

    /// Hide and capture the pointer, reporting relative motion (fly cameras).
    pub fn set_relative_mouse(&mut self, enabled: bool) -> Result<(), Error> {
        self.live()?;
        self.handle.set_relative_mouse(enabled).map_err(fail)
    }

    pub fn set_cursor(&mut self, shape: CursorShape) -> Result<(), Error> {
        self.live()?;
        if self.cursor_shape == Some(shape) {
            return Ok(());
        }
        let index = match self.cursors.iter().position(|(s, _)| *s == shape) {
            Some(i) => i,
            None => {
                let native = match shape {
                    CursorShape::Default => Shape::Default,
                    CursorShape::HorizontalResize => Shape::EwResize,
                    CursorShape::VerticalResize => Shape::NsResize,
                    CursorShape::Text => Shape::Text,
                };
                // Lookup is by key; retained list order determines destruction order.
                self.cursors.insert(0, (shape, Cursor::create(native).map_err(fail)?));
                0
            }
        };
        self.cursors[index].1.set().map_err(fail)?;
        self.cursor_shape = Some(shape);
        Ok(())
    }

    /// Accepted for pxui_wayland parity: SDL's text input takes no surrounding text.
    pub fn set_surrounding_text(&mut self, text: Option<(&str, usize, usize)>) -> Result<(), Error> {
        let _ = text;
        self.live().map(drop)
    }

    /// Text input runs only while a text field has focus: `Some (region, cursor)` (logical points)
    /// starts it and places the input method there, `None` stops it. Repeating the same answer
    /// costs no SDL call.
    pub fn set_text_input(&mut self, area: Option<((i64, i64, i64, i64), i64)>) -> Result<(), Error> {
        self.set_text_input_with_options(area, TextInputOptions::default())
    }
    pub fn set_text_input_with_options(&mut self, area: Option<((i64, i64, i64, i64), i64)>, options: TextInputOptions) -> Result<(), Error> {
        self.live()?;
        if options.autocorrect>1 || options.capitalization>3 || options.action>2 {return Err("Invalid text input options".into());}
        match area {
            None => {
                if self.text_input {
                    sdl3::text_input::stop(&self.handle).map_err(fail)?;
                    self.text_input = false;
                    self.ime_area = None;
                }
                Ok(())
            }
            Some(((x, y, width, height), cursor)) => {
                if width <= 0 || height <= 0 || cursor < 0 { return Err("Text-input area dimensions must be positive and its cursor non-negative".into()); }
                let area = Some(((x, y, width, height), cursor));
                if self.text_input && self.ime_area == area && self.text_input_options == options { return Ok(()); }
                if !self.text_input || self.text_input_options != options {
                    sdl3::text_input::start_with_options(&self.handle, options).map_err(fail)?;
                    self.text_input = true;
                    self.text_input_options = options;
                }
                let unit = self.unit;
                let scale = |v: i64| (v as f64 * unit).round() as i32;
                // pxui owns keyboard avoidance; iOS must not translate the entire Metal view.
                if cfg!(target_os="ios") { sdl3::text_input::clear_area(&self.handle) }
                else { sdl3::text_input::set_area(
                    &self.handle,
                    Rect { x: scale(x), y: scale(y), width: scale(width), height: scale(height) },
                    scale(cursor),
                ) }
                .map_err(fail)?;
                self.ime_area = area;
                Ok(())
            }
        }
    }

    pub fn show(&mut self) -> Result<(), Error> {
        self.live()?;
        reveal(&self.handle).map_err(fail)?;
        #[cfg(target_os = "macos")]
        {
            self.offscreen = false;
            if let Some(pacing) = &self.pacing { pacing.refresh_visibility(); }
        }
        self.announce_change();
        self.sync()
    }

    /// Hides the window. A later call to `show` can return it to the display.
    pub fn hide(&mut self) -> Result<(), Error> {
        self.live()?;
        self.handle.hide().map_err(fail)?;
        #[cfg(target_os = "macos")]
        if let Some(pacing) = &self.pacing { pacing.refresh_visibility(); }
        Ok(())
    }

    /// Opens a native file dialog and returns at once with its id; the outcome is an
    /// `Event::FileDialog` carrying the same id. `pattern` lists extensions separated by
    /// semicolons ("png;jpg"), or "*". At most 8 dialogs are open at once; a ninth is an error.
    pub fn show_dialog(&mut self, filters: &[DialogFilter], default_location: Option<&str>, kind: DialogKind) -> Result<i64, Error> {
        self.live()?;
        let filters: Vec<SdlDialogFilter> =
            filters.iter().map(|f| SdlDialogFilter { name: f.name.clone(), pattern: f.pattern.clone() }).collect();
        let native = match kind {
            DialogKind::OpenFile => SdlDialogKind::OpenFile,
            DialogKind::OpenFiles => SdlDialogKind::OpenFiles,
            DialogKind::SaveFile => SdlDialogKind::SaveFile,
            DialogKind::OpenFolder => SdlDialogKind::OpenFolder,
        };
        Ok(sdl3::dialog::show(&self.handle, native, &filters, default_location).map_err(fail)? as i64)
    }

    /// Takes the original file capabilities for a chosen FileDialog event, once, before the next pump.
    /// Move them to a worker before creating bookmarks or doing file I/O. Ignored outcomes expire.
    pub fn take_dialog_selection(&mut self, id: i64) -> Result<Vec<FileSelection>, Error> {
        self.live()?;
        let index = self.dialog_selections.iter().position(|(chosen, _)| *chosen == id)
            .ok_or_else(|| "Dialog selection is missing, expired or already consumed".to_owned())?;
        Ok(self.dialog_selections.swap_remove(index).1)
    }

    /// Shown and neither minimized nor covered. AppKit reports exposure asynchronously.
    pub fn visible(&mut self) -> Result<bool, Error> {
        self.live()?;
        #[cfg(target_os = "macos")]
        if let Some(pacing) = &self.pacing {
            return Ok(pacing.presentation().is_some());
        }
        let state = self.handle.state().map_err(fail)?;
        Ok(!(state.hidden || state.minimized || state.occluded))
    }

    /// Visible surface generation changes when its window reappears or changes displays.
    /// Explicit offscreen surfaces can render for capture and native checks.
    #[cfg(target_os = "macos")]
    pub fn presentation(&self) -> Option<u64> {
        if self.offscreen { Some(0) } else { self.pacing.as_ref().map_or(Some(1), sdl3::pacing::Pacing::presentation) }
    }

    /// Drives one native callback and holds its drawable until the returned tick is dropped.
    #[cfg(all(target_os = "macos", feature = "test-util"))]
    #[doc(hidden)]
    pub fn test_display_link_tick(&self) -> Option<sdl3::pacing::Tick> {
        self.pacing.as_ref()?.tick(0)
    }

    /// Monotonic time for the current frame's intended presentation, in seconds on the window clock.
    pub fn frame_time(&self) -> f64 {
        self.time().max(self.frame_time)
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    fn mark_frame_time(&mut self, lead: f64) {
        self.frame_time = self.frame_time.max(self.time() + lead);
    }

    /// Sets the preferred ambient animation rate in Hz. Zero requests the display rate.
    pub fn set_repaint_rate(&mut self, hz: u32) {
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        { self.repaint_hz = hz; }
        #[cfg(not(any(target_os = "macos", target_os = "ios")))]
        let _ = hz;
    }

    pub fn destroy(&mut self) -> Result<(), Error> {
        if self.dead {
            return Ok(());
        }
        self.dead = true;
        router::unregister(self.id);
        #[cfg(target_os = "macos")]
        drop(self.pacing.take());
        self.dialog_selections.clear();
        let mut failure: Option<Error> = None;
        let mut record = |result: Result<(), sdl3::Error>| {
            if let Err(e) = result {
                failure.get_or_insert(e.to_string());
            }
        };
        if self.text_input {
            record(sdl3::text_input::stop(&self.handle));
        }
        for (_, cursor) in self.cursors.iter_mut() {
            record(cursor.destroy());
        }
        self.cursors.clear();
        #[cfg(target_os = "ios")]
        record(self.handle.hide());
        record(self.view.destroy());
        record(self.handle.destroy());
        record(sdl3::init::quit_subsystems(&[sdl3::init::Subsystem::Video]));
        failure.map_or(Ok(()), Err)
    }
}

impl Drop for Window {
    /// A window dropped without `destroy` stops receiving events; its native handles release themselves.
    fn drop(&mut self) {
        router::unregister(self.id);
        #[cfg(target_os = "macos")]
        drop(self.pacing.take());
    }
}
