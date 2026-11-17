//! Main-thread NSAccessibility tree over a live framework window layer (see ax.m). The native side
//! keeps one element per node id and posts notifications only when `sync` brings a changed tree.
use native_layer_token_sys::portable::Token;
use std::{ffi::c_void, io, marker::PhantomData, ptr::NonNull, rc::Rc};

/// `u32::MAX` ends a sibling chain and marks a top-level node.
pub const NONE: u32 = u32::MAX;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Value {
    /// 0 none, 1 text range, 2 number
    pub tag: u32,
    pub text_start: u32,
    pub text_end: u32,
    pub reserved: u32,
    pub now: f64,
    pub min: f64,
    pub max: f64,
    pub step: f64,
}

/// Same layout as `pxui_types::ffi::A11yNode`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Node {
    pub id: u64,
    pub parent: u32,
    pub role: u32,
    pub name: [u32; 2],
    pub description: [u32; 2],
    pub value: Value,
    pub rect: [f32; 4],
    pub states: u32,
    pub actions: u32,
    pub selection: [u32; 2],
    pub child_count: u32,
    pub first_child: u32,
    pub next_sibling: u32,
    pub last_child: u32,
}

/// A request from an assistive client: `action` is a pxui `a11y::actions` bit.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Action {
    pub id: u64,
    pub action: u32,
    pub reserved: u32,
    pub payload: f64,
}

/// Queue depth of the native side; matches the 32 actions `Ui::inject_a11y` accepts per call.
pub const QUEUE: usize = 32;

pub struct Bridge {
    raw: NonNull<c_void>,
    _main: PhantomData<Rc<()>>,
}
impl Bridge {
    /// `wake` runs on the main thread when an assistive client queues an action, to end the host's wait.
    pub fn open(layer: &Token, wake: extern "C" fn()) -> io::Result<Self> {
        let guard = layer.borrow_raw(layer.owner_id(), layer.generation()).ok_or_else(unavailable)?;
        NonNull::new(unsafe { pxui_sdl3_ax_open(guard.as_ptr(), wake) })
            .map(|raw| Self { raw, _main: PhantomData })
            .ok_or_else(unavailable)
    }
    /// Whether VoiceOver is running (read from the system; call at a low rate).
    pub fn voiceover() -> bool {
        unsafe { pxui_sdl3_ax_voiceover() != 0 }
    }
    /// Publishes a tree: `strings` is the text its ranges point into, `first_top` its first top-level
    /// node (`NONE` for an empty tree). Malformed input is refused and leaves the previous tree.
    pub fn sync(&mut self, nodes: &[Node], strings: &[u8], first_top: u32) -> io::Result<()> {
        let (Ok(count), Ok(length)) = (u32::try_from(nodes.len()), u32::try_from(strings.len())) else { return Err(refused()) };
        let done = unsafe { pxui_sdl3_ax_sync(self.raw.as_ptr(), nodes.as_ptr(), count, strings.as_ptr(), length, first_top) };
        if done == 0 { Err(refused()) } else { Ok(()) }
    }
    /// Drops every element: the client sees an empty window.
    pub fn clear(&mut self) {
        unsafe { pxui_sdl3_ax_clear(self.raw.as_ptr()) }
    }
    /// Moves queued client requests into `out` (oldest first) and returns how many.
    pub fn take(&mut self, out: &mut [Action]) -> usize {
        unsafe { pxui_sdl3_ax_take(self.raw.as_ptr(), out.as_mut_ptr(), out.len() as u32) as usize }
    }
    /// NSAccessibility notifications posted so far.
    pub fn notifications(&self) -> u64 {
        unsafe { pxui_sdl3_ax_notifications(self.raw.as_ptr()) }
    }
}
impl Drop for Bridge {
    fn drop(&mut self) {
        unsafe { pxui_sdl3_ax_close(self.raw.as_ptr()) }
    }
}
fn unavailable() -> io::Error {
    io::Error::other("Accessibility needs a live main-thread AppKit view")
}
fn refused() -> io::Error {
    io::Error::other("Accessibility tree is malformed")
}
const _: () = assert!(size_of::<Node>() == 128 && size_of::<Action>() == 24);
unsafe extern "C" {
    fn pxui_sdl3_ax_open(layer: *mut c_void, wake: extern "C" fn()) -> *mut c_void;
    fn pxui_sdl3_ax_close(host: *mut c_void);
    fn pxui_sdl3_ax_voiceover() -> i32;
    fn pxui_sdl3_ax_sync(host: *mut c_void, nodes: *const Node, count: u32, strings: *const u8, length: u32, first_top: u32) -> i32;
    fn pxui_sdl3_ax_clear(host: *mut c_void);
    fn pxui_sdl3_ax_take(host: *mut c_void, out: *mut Action, capacity: u32) -> u32;
    fn pxui_sdl3_ax_notifications(host: *mut c_void) -> u64;
}

/// What an assistive client reads from one element, queried through the NSAccessibility protocol.
#[cfg(feature = "test-util")]
pub mod test {
    use super::*;
    use std::ffi::{CString, c_char};

    #[repr(C)]
    pub struct Info {
        role: [u8; 64],
        label: [u8; 128],
        value: [u8; 128],
        pub has_number: i32,
        pub enabled: i32,
        pub selected: i32,
        pub expanded: i32,
        pub focused: i32,
        pub actionable: i32,
        pub children: i32,
        pub number: f64,
        pub min: f64,
        pub max: f64,
        /// screen coordinates, origin bottom-left
        pub frame: [f64; 4],
        pub identity: usize,
    }
    fn text(bytes: &[u8]) -> &str {
        let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
        std::str::from_utf8(&bytes[..end]).unwrap_or("")
    }
    impl Info {
        pub fn role(&self) -> &str { text(&self.role) }
        pub fn label(&self) -> &str { text(&self.label) }
        pub fn value(&self) -> &str { text(&self.value) }
    }
    unsafe extern "C" {
        fn pxui_sdl3_ax_test_find(host: *mut c_void, label: *const c_char, info: *mut Info) -> i32;
        fn pxui_sdl3_ax_test_perform(host: *mut c_void, label: *const c_char, kind: i32, number: f64) -> i32;
    }
    fn zeroed() -> Info {
        Info { role: [0; 64], label: [0; 128], value: [0; 128], has_number: 0, enabled: 0, selected: 0, expanded: 0, focused: 0,
            actionable: 0, children: 0, number: 0.0, min: 0.0, max: 0.0, frame: [0.0; 4], identity: 0 }
    }
    /// The element labelled `label`, found by walking `accessibilityChildren` from the content view.
    pub fn find(bridge: &Bridge, label: &str) -> Option<Info> {
        let label = CString::new(label).ok()?;
        let mut info = zeroed();
        (unsafe { pxui_sdl3_ax_test_find(bridge.raw.as_ptr(), label.as_ptr(), &mut info) } != 0).then_some(info)
    }
    /// How many top-level elements the content view reports.
    pub fn top_level(bridge: &Bridge) -> i32 {
        let mut info = zeroed();
        unsafe { pxui_sdl3_ax_test_find(bridge.raw.as_ptr(), std::ptr::null(), &mut info) };
        info.children
    }
    pub const PRESS: i32 = 0;
    pub const INCREMENT: i32 = 1;
    pub const DECREMENT: i32 = 2;
    pub const SHOW_MENU: i32 = 3;
    pub const SET_VALUE: i32 = 4;
    pub const FOCUS: i32 = 5;
    /// Calls the protocol method; `None` when there is no such element, else its BOOL result.
    pub fn perform(bridge: &Bridge, label: &str, kind: i32, number: f64) -> Option<bool> {
        let label = CString::new(label).ok()?;
        match unsafe { pxui_sdl3_ax_test_perform(bridge.raw.as_ptr(), label.as_ptr(), kind, number) } {
            -1 => None,
            r => Some(r != 0),
        }
    }
}
