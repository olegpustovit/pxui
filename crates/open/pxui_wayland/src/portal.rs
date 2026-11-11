//! File dialogs through the XDG desktop portal: `org.freedesktop.portal.FileChooser.OpenFile`/`SaveFile` on the
//! session bus, answered later by the request object's `Response` signal, which `Window::pump` turns into the
//! `Event::FileDialog` pxui_sdl delivers (the chosen paths, none when cancelled, or an error). The D-Bus layer is
//! portal.c, pxui_atspi's shim under its own prefix.
use crate::{DialogFilter, DialogKind, Error, NO_FILE_DIALOG};
use input::Event;
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicI64, Ordering};

#[repr(C)]
struct RawConn(c_void);
#[repr(C)]
struct RawMsg(c_void);

unsafe extern "C" {
    fn pd_open(address: *const c_char, err: *mut c_char, errlen: usize) -> *mut RawConn;
    fn pd_close(c: *mut RawConn);
    fn pd_fd(c: *mut RawConn) -> c_int;
    fn pd_unique_name(c: *mut RawConn) -> *const c_char;
    fn pd_add_match(c: *mut RawConn, rule: *const c_char, err: *mut c_char, errlen: usize) -> c_int;
    fn pd_new_call(dest: *const c_char, path: *const c_char, iface: *const c_char, member: *const c_char) -> *mut RawMsg;
    fn pd_free(m: *mut RawMsg);
    fn pd_put(m: *mut RawMsg, kind: c_int, p: *const c_void, len: usize);
    fn pd_open_container(m: *mut RawMsg, kind: c_int, sig: *const c_char);
    fn pd_close_container(m: *mut RawMsg);
    fn pd_send(c: *mut RawConn, m: *mut RawMsg) -> u32;
    fn pd_flush(c: *mut RawConn);
    fn pd_buffered(c: *mut RawConn) -> c_int;
    fn pd_call_block(c: *mut RawConn, m: *mut RawMsg, timeout_ms: c_int, err: *mut c_char, errlen: usize) -> *mut RawMsg;
    fn pd_pop(c: *mut RawConn, connected: *mut c_int) -> *mut RawMsg;
    fn pd_kind(m: *mut RawMsg) -> c_int;
    fn pd_path(m: *mut RawMsg) -> *const c_char;
    fn pd_member(m: *mut RawMsg) -> *const c_char;
    fn pd_error_name(m: *mut RawMsg) -> *const c_char;
    fn pd_reply_serial(m: *mut RawMsg) -> u32;
    fn pd_arg_type(m: *mut RawMsg) -> c_int;
    fn pd_get(m: *mut RawMsg, out: *mut c_void);
    fn pd_next(m: *mut RawMsg) -> c_int;
    fn pd_recurse(m: *mut RawMsg) -> c_int;
    fn pd_up(m: *mut RawMsg);
}

const METHOD_RETURN: i32 = 2;
const ERROR: i32 = 3;
const SIGNAL: i32 = 4;

fn cstr(text: &str) -> CString {
    CString::new(text).unwrap_or_default()
}

fn error_text(buf: &[u8]) -> String {
    CStr::from_bytes_until_nul(buf).map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

fn text<'a>(p: *const c_char) -> &'a str {
    // SAFETY: libdbus strings live as long as their message or connection, which outlive the borrow.
    if p.is_null() { "" } else { unsafe { CStr::from_ptr(p) }.to_str().unwrap_or("") }
}

/// A private connection to one bus.
struct Conn(NonNull<RawConn>);

impl Conn {
    fn open(address: &str) -> Result<Conn, String> {
        let address = cstr(address);
        let mut err = [0u8; 256];
        // SAFETY: a NUL-terminated address and an error buffer of the length given.
        let raw = unsafe { pd_open(address.as_ptr(), err.as_mut_ptr().cast(), err.len()) };
        NonNull::new(raw).map(Conn).ok_or_else(|| error_text(&err))
    }
    fn fd(&self) -> i32 {
        // SAFETY: a live connection (for every Conn method below too).
        unsafe { pd_fd(self.0.as_ptr()) }
    }
    fn unique_name(&self) -> String {
        text(unsafe { pd_unique_name(self.0.as_ptr()) }).to_owned()
    }
    fn add_match(&self, rule: &str) -> Result<(), String> {
        let rule = cstr(rule);
        let mut err = [0u8; 256];
        let ok = unsafe { pd_add_match(self.0.as_ptr(), rule.as_ptr(), err.as_mut_ptr().cast(), err.len()) };
        if ok != 0 { Ok(()) } else { Err(error_text(&err)) }
    }
    /// Queue `msg`; its serial, or `None` when it was malformed (e.g. invalid UTF-8) and was dropped.
    fn send(&self, msg: Msg) -> Option<u32> {
        let raw = msg.0.as_ptr();
        std::mem::forget(msg);
        // SAFETY: the shim takes the message and frees it.
        let serial = unsafe { pd_send(self.0.as_ptr(), raw) };
        unsafe { pd_flush(self.0.as_ptr()) };
        (serial != 0).then_some(serial)
    }
    fn call(&self, call: Msg, timeout_ms: i32) -> Result<Msg, String> {
        let raw = call.0.as_ptr();
        std::mem::forget(call);
        let mut err = [0u8; 256];
        // SAFETY: the shim takes the call and frees it; the reply is ours.
        let reply = unsafe { pd_call_block(self.0.as_ptr(), raw, timeout_ms, err.as_mut_ptr().cast(), err.len()) };
        NonNull::new(reply).map(Msg).ok_or_else(|| error_text(&err))
    }
    /// The next incoming message without blocking; `Err` once disconnected.
    fn pop(&self) -> Result<Option<Msg>, ()> {
        let mut connected = 0;
        let m = NonNull::new(unsafe { pd_pop(self.0.as_ptr(), &mut connected) }).map(Msg);
        if m.is_none() && connected == 0 { Err(()) } else { Ok(m) }
    }
    fn buffered(&self) -> bool {
        unsafe { pd_buffered(self.0.as_ptr()) != 0 }
    }
}

impl Drop for Conn {
    fn drop(&mut self) {
        // SAFETY: the connection is ours and closed once.
        unsafe { pd_close(self.0.as_ptr()) }
    }
}

/// A message being written or read, with one cursor.
struct Msg(NonNull<RawMsg>);

impl Msg {
    fn call(dest: &str, path: &str, iface: &str, member: &str) -> Msg {
        let (d, p, i, m) = (cstr(dest), cstr(path), cstr(iface), cstr(member));
        // SAFETY: NUL-terminated names; NULL only when libdbus is out of memory.
        Msg(NonNull::new(unsafe { pd_new_call(d.as_ptr(), p.as_ptr(), i.as_ptr(), m.as_ptr()) }).expect("libdbus: out of memory"))
    }
    fn put(&mut self, kind: u8, v: *const c_void, len: usize) -> &mut Msg {
        // SAFETY: a live message; `v` points at a value of `kind` (or `len` text bytes), which the shim copies.
        unsafe { pd_put(self.0.as_ptr(), c_int::from(kind), v, len) };
        self
    }
    fn s(&mut self, v: &str) -> &mut Msg { self.put(b's', v.as_ptr().cast(), v.len()) }
    fn u(&mut self, v: u32) -> &mut Msg { self.put(b'u', (&v as *const u32).cast(), 0) }
    fn y(&mut self, v: u8) -> &mut Msg { self.put(b'y', (&v as *const u8).cast(), 0) }
    fn b(&mut self, v: bool) -> &mut Msg { self.put(b'b', (&u32::from(v) as *const u32).cast(), 0) }
    /// Open a container: `b'a'` with the element signature, `b'v'` with the contents', `b'r'`/`b'e'` with "".
    fn open(&mut self, kind: u8, sig: &str) -> &mut Msg {
        let s = cstr(sig);
        // SAFETY: a live message and a NUL-terminated signature (or NULL).
        unsafe { pd_open_container(self.0.as_ptr(), c_int::from(kind), if sig.is_empty() { std::ptr::null() } else { s.as_ptr() }) };
        self
    }
    fn close(&mut self) -> &mut Msg {
        unsafe { pd_close_container(self.0.as_ptr()) };
        self
    }
    // SAFETY (the readers below): a live message; the shim checks the cursor's depth.
    fn kind(&self) -> i32 { unsafe { pd_kind(self.0.as_ptr()) } }
    fn path(&self) -> &str { text(unsafe { pd_path(self.0.as_ptr()) }) }
    fn member(&self) -> &str { text(unsafe { pd_member(self.0.as_ptr()) }) }
    fn error_name(&self) -> &str { text(unsafe { pd_error_name(self.0.as_ptr()) }) }
    fn reply_serial(&self) -> u32 { unsafe { pd_reply_serial(self.0.as_ptr()) } }
    /// The current argument's type code (0 past the end).
    fn arg(&self) -> u8 { unsafe { pd_arg_type(self.0.as_ptr()) as u8 } }
    /// A fixed-size basic argument of type `kind` as libdbus's 8-byte DBusBasicValue, and advance.
    fn get(&mut self, kind: u8) -> Option<[u8; 8]> {
        if self.arg() != kind {
            return None;
        }
        let mut v = [0u8; 8];
        unsafe {
            pd_get(self.0.as_ptr(), v.as_mut_ptr().cast());
            pd_next(self.0.as_ptr());
        }
        Some(v)
    }
    fn read_str(&mut self) -> Option<&str> {
        if self.arg() != b's' && self.arg() != b'o' {
            return None;
        }
        let mut p: *const c_char = std::ptr::null();
        unsafe {
            pd_get(self.0.as_ptr(), (&mut p as *mut *const c_char).cast());
            pd_next(self.0.as_ptr());
        }
        Some(text(p))
    }
    fn read_u32(&mut self) -> Option<u32> { self.get(b'u').map(|v| u32::from_ne_bytes([v[0], v[1], v[2], v[3]])) }
    /// Step into the current container; false if it is none.
    fn enter(&mut self) -> bool { unsafe { pd_recurse(self.0.as_ptr()) != 0 } }
    /// Leave the container and step past it.
    fn leave(&mut self) {
        unsafe {
            pd_up(self.0.as_ptr());
            pd_next(self.0.as_ptr());
        }
    }
}

impl Drop for Msg {
    fn drop(&mut self) {
        unsafe { pd_free(self.0.as_ptr()) }
    }
}

const DESKTOP: &str = "org.freedesktop.portal.Desktop";
const DESKTOP_PATH: &str = "/org/freedesktop/portal/desktop";
const DBUS: &str = "org.freedesktop.DBus";
/// pxui_sdl's limit (SDL's): a ninth open dialog is an error.
const MAX_OPEN: usize = 8;

static NEXT_DIALOG: AtomicI64 = AtomicI64::new(1);

/// One window's portal connection and its open dialogs. A portal that exits while dialogs are open (the
/// bus fails only unanswered calls) is seen by its name losing its owner: those dialogs fail.
pub(crate) struct Portal {
    conn: Conn,
    /// The request path prefix: the unique name without its ':' and with '.' as '_'.
    sender: String,
    open: Vec<Open>,
}

struct Open {
    id: i64,
    /// The OpenFile/SaveFile call's serial: its error reply ends the dialog.
    serial: u32,
    /// The request object whose Response ends it.
    path: String,
}

/// The session bus: `DBUS_SESSION_BUS_ADDRESS`, else `$XDG_RUNTIME_DIR/bus` (never X11 autolaunch).
fn session_address() -> Option<String> {
    std::env::var("DBUS_SESSION_BUS_ADDRESS").ok().filter(|a| !a.is_empty()).or_else(|| {
        let bus = std::path::Path::new(&std::env::var_os("XDG_RUNTIME_DIR")?).join("bus");
        bus.exists().then(|| format!("unix:path={}", bus.display()))
    })
}

/// The portal's name changing owner (it exited, or was activated).
const OWNER_RULE: &str = "type='signal',sender='org.freedesktop.DBus',interface='org.freedesktop.DBus',member='NameOwnerChanged',arg0='org.freedesktop.portal.Desktop'";

fn response_rule(path: &str) -> String {
    format!("type='signal',interface='org.freedesktop.portal.Request',member='Response',path='{path}'")
}

impl Portal {
    /// A connection to the bus at `address` (`None`: the session bus) on which a desktop portal runs or can be
    /// activated; `NO_FILE_DIALOG` otherwise.
    pub(crate) fn connect(address: Option<&str>) -> Result<Portal, Error> {
        let address = address.map(str::to_owned).or_else(session_address).ok_or_else(|| NO_FILE_DIALOG.to_owned())?;
        let conn = Conn::open(&address).map_err(|_| NO_FILE_DIALOG.to_owned())?;
        let ask = |member: &str, name: Option<&str>| {
            let mut call = Msg::call(DBUS, "/org/freedesktop/DBus", DBUS, member);
            if let Some(name) = name {
                call.s(name);
            }
            conn.call(call, 1000).ok()
        };
        let running = ask("NameHasOwner", Some(DESKTOP)).is_some_and(|mut r| r.get(b'b').is_some_and(|v| v[..4] != [0; 4]));
        let activatable = || {
            ask("ListActivatableNames", None).is_some_and(|mut r| {
                let mut found = false;
                if r.enter() {
                    while let Some(name) = r.read_str() {
                        found |= name == DESKTOP;
                    }
                }
                found
            })
        };
        if !running && !activatable() {
            return Err(NO_FILE_DIALOG.to_owned());
        }
        conn.add_match(OWNER_RULE).map_err(|_| NO_FILE_DIALOG.to_owned())?;
        let sender = conn.unique_name().trim_start_matches(':').replace('.', "_");
        Ok(Portal { conn, sender, open: Vec::new() })
    }

    /// The bus connection's descriptor, for the host's wait.
    pub(crate) fn fd(&self) -> i32 {
        self.conn.fd()
    }

    /// Messages already read off the descriptor: the host's wait must not sleep on it.
    pub(crate) fn buffered(&self) -> bool {
        self.conn.buffered()
    }

    /// Asks the portal for a dialog over the window `parent` ("wayland:<handle>", or "") and returns at once with
    /// its id; `pump` delivers the answer.
    pub(crate) fn show(&mut self, parent: &str, filters: &[DialogFilter], default_location: Option<&str>, kind: DialogKind) -> Result<i64, Error> {
        if self.open.len() >= MAX_OPEN {
            return Err(format!("at most {MAX_OPEN} file dialogs are open at once"));
        }
        let id = NEXT_DIALOG.fetch_add(1, Ordering::Relaxed);
        let token = format!("pxui_dialog{id}");
        let path = format!("{DESKTOP_PATH}/request/{}/{token}", self.sender);
        // Subscribed before the call: a fast portal answers before its reply names the request.
        self.conn.add_match(&response_rule(&path))?;
        let (member, title) = match kind {
            DialogKind::OpenFile => ("OpenFile", "Open File"),
            DialogKind::OpenFiles => ("OpenFile", "Open Files"),
            DialogKind::OpenFolder => ("OpenFile", "Choose Folder"),
            DialogKind::SaveFile => ("SaveFile", "Save File"),
        };
        let mut m = Msg::call(DESKTOP, DESKTOP_PATH, "org.freedesktop.portal.FileChooser", member);
        m.s(parent).s(title).open(b'a', "{sv}");
        option(&mut m, "handle_token", "s", &mut |m| { m.s(&token); });
        match kind {
            DialogKind::OpenFiles => option(&mut m, "multiple", "b", &mut |m| { m.b(true); }),
            DialogKind::OpenFolder => option(&mut m, "directory", "b", &mut |m| { m.b(true); }),
            _ => {}
        }
        if kind != DialogKind::OpenFolder && !filters.is_empty() {
            option(&mut m, "filters", "a(sa(us))", &mut |m| {
                m.open(b'a', "(sa(us))");
                for filter in filters {
                    m.open(b'r', "").s(&filter.name).open(b'a', "(us)");
                    for glob in globs(&filter.pattern) {
                        m.open(b'r', "").u(0).s(&glob).close();
                    }
                    m.close().close();
                }
                m.close();
            });
        }
        let (folder, name) = location(default_location, kind);
        if let Some(folder) = folder {
            // a NUL-terminated byte string
            option(&mut m, "current_folder", "ay", &mut |m| {
                m.open(b'a', "y");
                for byte in folder.bytes().chain([0]) {
                    m.y(byte);
                }
                m.close();
            });
        }
        if let Some(name) = name {
            option(&mut m, "current_name", "s", &mut |m| { m.s(name); });
        }
        m.close();
        let serial = self.conn.send(m).ok_or_else(|| "the file dialog request is not valid D-Bus (a NUL or invalid UTF-8)".to_owned())?;
        self.open.push(Open { id, serial, path });
        Ok(id)
    }

    /// Appends an `Event::FileDialog` for each dialog the portal answered.
    pub(crate) fn pump(&mut self, events: &mut Vec<Event>) {
        loop {
            match self.conn.pop() {
                Ok(Some(message)) => self.take(message, events),
                Ok(None) => return,
                Err(()) => {
                    for open in self.open.drain(..) {
                        events.push(Event::FileDialog { id: open.id, result: Err("the session bus closed while the file dialog was open".to_owned()) });
                    }
                    return;
                }
            }
        }
    }

    fn take(&mut self, mut m: Msg, events: &mut Vec<Event>) {
        let kind = m.kind();
        if kind == SIGNAL && m.member() == "NameOwnerChanged" {
            // (name, old owner, new owner): the portal's name with no new owner means it exited
            let name = m.read_str().unwrap_or("").to_owned();
            let _old = m.read_str();
            if name == DESKTOP && m.read_str().is_some_and(str::is_empty) {
                for open in self.open.drain(..) {
                    events.push(Event::FileDialog { id: open.id, result: Err("the desktop portal exited while the file dialog was open".to_owned()) });
                }
            }
            return;
        }
        let at = match kind {
            METHOD_RETURN | ERROR => self.open.iter().position(|o| o.serial == m.reply_serial()),
            SIGNAL if m.member() == "Response" => self.open.iter().position(|o| o.path == m.path()),
            _ => None,
        };
        let Some(at) = at else { return };
        if kind == METHOD_RETURN {
            // The request's handle: a portal older than handle_token picks its own path.
            if let Some(handle) = m.read_str().filter(|h| *h != self.open[at].path).map(str::to_owned) {
                let _ = self.conn.add_match(&response_rule(&handle));
                self.open[at].path = handle;
            }
            return;
        }
        let open = self.open.remove(at);
        let result = if kind == ERROR {
            let name = m.error_name().to_owned();
            Err(format!("the desktop portal refused the file dialog: {name} {}", m.read_str().unwrap_or("")))
        } else {
            let mut remove = Msg::call(DBUS, "/org/freedesktop/DBus", DBUS, "RemoveMatch");
            remove.s(&response_rule(&open.path));
            self.conn.send(remove);
            response(&mut m)
        };
        events.push(Event::FileDialog { id: open.id, result });
    }
}

/// One `a{sv}` entry: `key` and a variant of signature `sig` that `value` writes.
fn option(m: &mut Msg, key: &str, sig: &str, value: &mut dyn FnMut(&mut Msg)) {
    m.open(b'e', "").s(key).open(b'v', sig);
    value(m);
    m.close().close();
}

/// A Response's (code, results): 0 the chosen files' paths, 1 cancelled (none), else an error.
fn response(m: &mut Msg) -> Result<Vec<String>, String> {
    match m.read_u32() {
        Some(0) => {}
        Some(1) => return Ok(Vec::new()),
        _ => return Err("the file dialog ended without an answer".to_owned()),
    }
    let mut paths = Vec::new();
    if m.enter() {
        while m.arg() == b'e' {
            m.enter();
            let key = m.read_str().unwrap_or("").to_owned();
            if key == "uris" && m.enter() {
                if m.enter() {
                    while let Some(uri) = m.read_str() {
                        paths.push(uri_path(uri)?);
                    }
                    m.leave();
                }
                m.leave();
            }
            m.leave();
        }
    }
    Ok(paths)
}

/// SDL's pattern ("png;jpg", or "*") as the portal's case-insensitive globs ("*.[pP][nN][gG]"), as SDL's own
/// portal dialog writes them.
fn globs(pattern: &str) -> Vec<String> {
    pattern
        .split(';')
        .map(str::trim)
        .filter(|ext| !ext.is_empty())
        .map(|ext| {
            if ext == "*" {
                return "*".to_owned();
            }
            let mut glob = String::from("*.");
            for c in ext.chars() {
                if c.is_ascii_alphabetic() {
                    glob.extend(['[', c.to_ascii_lowercase(), c.to_ascii_uppercase(), ']']);
                } else {
                    glob.push(c);
                }
            }
            glob
        })
        .collect()
}

/// `default_location` as the portal's (current_folder, current_name): a folder (existing, or ending in '/') is
/// the folder; a file path names its folder and, for a save, the suggested name.
fn location(at: Option<&str>, kind: DialogKind) -> (Option<&str>, Option<&str>) {
    let Some(at) = at.filter(|at| !at.is_empty()) else { return (None, None) };
    if at.ends_with('/') || std::path::Path::new(at).is_dir() {
        return (Some(at), None);
    }
    let (folder, name) = at.rsplit_once('/').unwrap_or(("", at));
    (Some(folder).filter(|f| !f.is_empty()), (kind == DialogKind::SaveFile && !name.is_empty()).then_some(name))
}

/// A `file://` URI's path, percent-escapes decoded. Another scheme or host, a malformed escape or a path that is
/// not UTF-8 (the event carries text) is an error naming the URI.
pub(crate) fn uri_path(uri: &str) -> Result<String, String> {
    let rest = uri.strip_prefix("file://").ok_or_else(|| format!("the file dialog chose {uri}, which is not a local file"))?;
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    if !rest.starts_with('/') {
        return Err(format!("the file dialog chose {uri}, a file on another host"));
    }
    let mut bytes = Vec::with_capacity(rest.len());
    let mut it = rest.bytes();
    while let Some(b) = it.next() {
        if b != b'%' {
            bytes.push(b);
            continue;
        }
        let hex = |d: Option<u8>| d.and_then(|d| (d as char).to_digit(16));
        match (hex(it.next()), hex(it.next())) {
            (Some(hi), Some(lo)) => bytes.push((hi * 16 + lo) as u8),
            _ => return Err(format!("the file dialog chose {uri}, which has a malformed escape")),
        }
    }
    String::from_utf8(bytes).map_err(|_| format!("the file dialog chose {uri}, whose path is not UTF-8"))
}

#[cfg(test)]
mod tests;
