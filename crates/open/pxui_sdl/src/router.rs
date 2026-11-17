//! The process event router. SDL keeps one event queue per process; every `Window::pump` drains it here
//! and each event goes to the queue of the window it names, so two windows never take each other's input.
//!
//! Per process by design (the SDL queue is), and it lives on the initial thread with SDL itself: a
//! thread-local the other threads never reach. Bounds: one route per open window, each holding at most
//! `QUEUE_CAP` events between its pumps, plus one pending close; a closed window's route and queued
//! events go with `unregister`.
//!
//! Routing rules: an event naming a window goes to that window, or is dropped when it is closed or not
//! one of ours. Quit goes to every window. An event naming none (synthetic, a trackpad sample with no
//! window under the pointer, a dialog opened without a parent) goes to the window that last gained
//! focus, else to the only open window; with two or more windows and none focused it reaches no window
//! and is counted as unrouted on every window, which reports it. Past `QUEUE_CAP` an event is dropped
//! and counted as overflow, except a close request or quit: it is kept aside and always delivered.
use std::cell::RefCell;

/// Most events a window holds between two of its pumps; later ones are dropped and counted.
pub(crate) const QUEUE_CAP: usize = 4096;

struct Route {
    window: u32,
    queue: Vec<sdl3::Event>,
    /// a close request or quit that arrived while the queue was full
    closing: Option<sdl3::Event>,
    /// events dropped at `QUEUE_CAP`
    overflow: usize,
    /// window-less events no window could take (2+ windows, none focused)
    unrouted: usize,
}

/// What a window lost since its last pump.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Lost {
    pub(crate) overflow: usize,
    pub(crate) unrouted: usize,
}

#[derive(Default)]
struct Router {
    routes: Vec<Route>,
    polled: Vec<sdl3::event::Routed>,
    /// the window that last gained focus and has not lost it since (0: none)
    focused: u32,
}

thread_local! {
    static ROUTER: RefCell<Router> = RefCell::new(Router::default());
}

fn deliver(route: &mut Route, event: sdl3::Event) {
    if route.queue.len() < QUEUE_CAP {
        route.queue.push(event);
    } else if matches!(event, sdl3::Event::Quit | sdl3::Event::Window(sdl3::WindowChange::CloseRequested)) {
        route.closing = Some(event);
    } else {
        route.overflow += 1;
    }
}

impl Router {
    fn route(&mut self, window: u32, event: sdl3::Event) {
        if matches!(event, sdl3::Event::Quit) {
            for route in &mut self.routes {
                deliver(route, sdl3::Event::Quit);
            }
            return;
        }
        let target = if window != 0 {
            window
        } else if self.focused != 0 {
            self.focused
        } else if let [only] = self.routes.as_slice() {
            only.window
        } else {
            for route in &mut self.routes {
                route.unrouted += 1;
            }
            return;
        };
        let Some(route) = self.routes.iter_mut().find(|route| route.window == target) else { return };
        match event {
            sdl3::Event::Window(sdl3::WindowChange::FocusGained) => self.focused = target,
            sdl3::Event::Window(sdl3::WindowChange::FocusLost) if self.focused == target => self.focused = 0,
            _ => {}
        }
        deliver(route, event);
    }
}

/// Starts routing events that name `window` to it.
pub(crate) fn register(window: u32) {
    ROUTER.with(|router| {
        let mut router = router.borrow_mut();
        if !router.routes.iter().any(|route| route.window == window) {
            router.routes.push(Route { window, queue: Vec::with_capacity(256), closing: None, overflow: 0, unrouted: 0 });
        }
    });
}

/// Stops routing to `window` and drops what it had not taken.
pub(crate) fn unregister(window: u32) {
    // a thread-exit drop finds the router already gone: nothing is left to route then
    let _ = ROUTER.try_with(|router| {
        let mut router = router.borrow_mut();
        router.routes.retain(|route| route.window != window);
        if router.focused == window {
            router.focused = 0;
        }
    });
}

/// Drains the process queue into the windows' queues, then swaps `window`'s queue into `out` (cleared
/// first; both keep their capacity, so a warm pump allocates nothing). Returns what `window` lost
/// since its last pump. A poll failure keeps the queues for the next.
pub(crate) fn pump(window: u32, out: &mut Vec<sdl3::Event>) -> Result<Lost, sdl3::Error> {
    out.clear();
    // the queue is drained outside the borrow: SDL's live-resize callback may run app code meanwhile
    let mut polled = ROUTER.with(|router| std::mem::take(&mut router.borrow_mut().polled));
    let result = sdl3::event::poll_coalesced_into(&mut polled);
    ROUTER.with(|router| {
        let mut router = router.borrow_mut();
        if result.is_ok() {
            for (id, event) in polled.drain(..) {
                router.route(id, event);
            }
        }
        polled.clear();
        router.polled = polled;
        result?;
        Ok(match router.routes.iter_mut().find(|route| route.window == window) {
            Some(route) => {
                std::mem::swap(&mut route.queue, out);
                out.extend(route.closing.take());
                Lost { overflow: std::mem::take(&mut route.overflow), unrouted: std::mem::take(&mut route.unrouted) }
            }
            None => Lost::default(),
        })
    })
}

/// Whether `window` has events waiting that an earlier pump of another window routed to it.
pub(crate) fn pending(window: u32) -> bool {
    ROUTER.with(|router| router.borrow().routes.iter().any(|route| route.window == window && (!route.queue.is_empty() || route.closing.is_some())))
}
