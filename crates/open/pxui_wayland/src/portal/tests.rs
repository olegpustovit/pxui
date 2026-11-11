//! The request encoding, the answer decoding, and the whole exchange against a fake portal: a second connection
//! on a private `dbus-daemon` that owns org.freedesktop.portal.Desktop, reads each FileChooser call and emits
//! the request's Response (open, several, save, a folder, cancel, a failed answer), and the portal-absent error.
use super::*;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

unsafe extern "C" {
    fn pd_new_reply(call: *mut RawMsg) -> *mut RawMsg;
    fn pd_new_signal(path: *const c_char, iface: *const c_char, member: *const c_char) -> *mut RawMsg;
    fn pd_sender(m: *mut RawMsg) -> *const c_char;
    fn pd_wait(c: *mut RawConn, timeout_ms: c_int, connected: *mut c_int) -> *mut RawMsg;
}

#[test]
fn filters_locations_and_uris() {
    assert_eq!(globs("png; JPG;"), ["*.[pP][nN][gG]", "*.[jJ][pP][gG]"]);
    assert_eq!(globs("*"), ["*"]);
    assert_eq!(globs("tar.gz"), ["*.[tT][aA][rR].[gG][zZ]"]);
    assert_eq!(location(Some("/tmp/"), DialogKind::SaveFile), (Some("/tmp/"), None));
    assert_eq!(location(Some("/"), DialogKind::OpenFile), (Some("/"), None), "an existing folder");
    assert_eq!(location(Some("/nowhere/out.rays"), DialogKind::SaveFile), (Some("/nowhere"), Some("out.rays")));
    assert_eq!(location(Some("/nowhere/in.rays"), DialogKind::OpenFile), (Some("/nowhere"), None));
    assert_eq!(location(Some("out.rays"), DialogKind::SaveFile), (None, Some("out.rays")));
    assert_eq!(location(Some(""), DialogKind::SaveFile), (None, None));
    assert_eq!(uri_path("file:///tmp/a%20b%25.png"), Ok("/tmp/a b%.png".to_owned()));
    assert_eq!(uri_path("file://localhost/x"), Ok("/x".to_owned()));
    assert_eq!(uri_path("file:///caf%C3%A9/%e2%9c%93"), Ok("/café/\u{2713}".to_owned()));
    assert!(uri_path("file:///latin1/caf%E9").is_err_and(|e| e.contains("not UTF-8")), "a non-UTF-8 path is an error, not a lossy path");
    assert!(uri_path("file:///bad%2").is_err() && uri_path("file:///bad%zz").is_err());
    assert!(uri_path("https://example.org/x").is_err() && uri_path("file://other-host/x").is_err());
}

/// A spawned daemon, terminated on drop.
struct Daemon(Child);
impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn private_bus() -> Option<(Daemon, String)> {
    let mut child = Command::new("dbus-daemon")
        .args(["--session", "--nofork", "--nopidfile", "--print-address=1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut line = String::new();
    BufReader::new(child.stdout.take()?).read_line(&mut line).ok()?;
    Some((Daemon(child), line.trim().to_owned()))
}

/// What the fake portal read from one request.
#[derive(Debug, Default, PartialEq)]
struct Seen {
    member: String,
    parent: String,
    title: String,
    token: String,
    multiple: bool,
    directory: bool,
    filters: Vec<(String, Vec<(u32, String)>)>,
    folder: Vec<u8>,
    name: String,
}

fn owned(m: &mut Msg) -> String {
    m.read_str().unwrap_or("").to_owned()
}

fn flag(m: &mut Msg) -> bool {
    m.get(b'b').is_some_and(|v| v[..4] != [0; 4])
}

/// Reads the next FileChooser call on `fake`, replies with its request handle and answers on that request with
/// `code` and `uris`.
fn serve(fake: &Conn, code: u32, uris: &[&str]) -> Seen {
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut call = loop {
        assert!(Instant::now() < deadline, "the fake portal got no call");
        let mut connected = 0;
        // SAFETY: a live connection; the popped message is ours.
        if let Some(m) = NonNull::new(unsafe { pd_wait(fake.0.as_ptr(), 50, &mut connected) }).map(Msg)
            && m.kind() == 1
        {
            break m;
        }
    };
    let mut seen = Seen { member: call.member().to_owned(), ..Seen::default() };
    // SAFETY: the call is live; the sender string is copied at once.
    let sender = text(unsafe { pd_sender(call.0.as_ptr()) }).trim_start_matches(':').replace('.', "_");
    seen.parent = owned(&mut call);
    seen.title = owned(&mut call);
    assert!(call.enter(), "options");
    while call.arg() == b'e' {
        call.enter();
        let key = owned(&mut call);
        assert!(call.enter(), "{key} is a variant");
        match key.as_str() {
            "handle_token" => seen.token = owned(&mut call),
            "multiple" => seen.multiple = flag(&mut call),
            "directory" => seen.directory = flag(&mut call),
            "current_name" => seen.name = owned(&mut call),
            "current_folder" => {
                call.enter();
                while let Some(v) = call.get(b'y') {
                    seen.folder.push(v[0]);
                }
                call.leave();
            }
            "filters" => {
                call.enter();
                while call.arg() == b'r' {
                    call.enter();
                    let name = owned(&mut call);
                    let mut globs = Vec::new();
                    call.enter();
                    while call.arg() == b'r' {
                        call.enter();
                        let kind = call.read_u32().unwrap_or(9);
                        globs.push((kind, owned(&mut call)));
                        call.leave();
                    }
                    call.leave();
                    call.leave();
                    seen.filters.push((name, globs));
                }
                call.leave();
            }
            // SAFETY: a live message; steps past the value
            _ => unsafe { pd_next(call.0.as_ptr()); },
        }
        call.leave();
        call.leave();
    }
    let handle = format!("{DESKTOP_PATH}/request/{sender}/{}", seen.token);
    // SAFETY: a reply to the live call.
    let mut reply = Msg(NonNull::new(unsafe { pd_new_reply(call.0.as_ptr()) }).unwrap());
    reply.put(b'o', handle.as_ptr().cast(), handle.len());
    fake.send(reply).expect("reply sent");
    if code == u32::MAX {
        // accepted: the dialog stays open
        return seen;
    }
    let (p, i, m) = (cstr(&handle), cstr("org.freedesktop.portal.Request"), cstr("Response"));
    // SAFETY: NUL-terminated names.
    let mut signal = Msg(NonNull::new(unsafe { pd_new_signal(p.as_ptr(), i.as_ptr(), m.as_ptr()) }).unwrap());
    signal.u(code).open(b'a', "{sv}");
    if !uris.is_empty() {
        option(&mut signal, "uris", "as", &mut |m| {
            m.open(b'a', "s");
            for uri in uris {
                m.s(uri);
            }
            m.close();
        });
    }
    signal.close();
    fake.send(signal).expect("response sent");
    seen
}

/// Pumps until the portal delivers a dialog's outcome.
fn answer(portal: &mut Portal) -> (i64, Result<Vec<String>, String>) {
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut events = Vec::new();
    loop {
        portal.pump(&mut events);
        if let Some(Event::FileDialog { id, result }) = events.pop() {
            assert!(events.is_empty(), "one event per answer");
            return (id, result);
        }
        assert!(Instant::now() < deadline, "no FileDialog event within 3 s");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn fake_portal() {
    let Some((_daemon, address)) = private_bus() else {
        println!("RAYS_TEST_SKIP: pxui_wayland portal: no dbus-daemon to run a fake portal on");
        return;
    };
    let absent = Some(NO_FILE_DIALOG.to_owned());
    assert_eq!(Portal::connect(Some(&address)).err(), absent, "no portal on the bus");
    assert_eq!(Portal::connect(Some("unix:path=/nonexistent/pxui-bus")).err(), absent, "no bus");

    let fake = Conn::open(&address).expect("fake portal connection");
    let mut request = Msg::call(DBUS, "/org/freedesktop/DBus", DBUS, "RequestName");
    request.s(DESKTOP).u(4);
    assert_eq!(fake.call(request, 1000).ok().and_then(|mut r| r.read_u32()), Some(1), "the fake owns the portal name");
    let mut portal = Portal::connect(Some(&address)).expect("the fake portal answers");
    let images = [DialogFilter { name: "Images".into(), pattern: "png;jpg".into() }, DialogFilter { name: "All".into(), pattern: "*".into() }];
    let image_globs = vec![
        ("Images".to_owned(), vec![(0, "*.[pP][nN][gG]".to_owned()), (0, "*.[jJ][pP][gG]".to_owned())]),
        ("All".to_owned(), vec![(0, "*".to_owned())]),
    ];

    let id = portal.show("wayland:exported", &images, Some("/tmp/"), DialogKind::OpenFile).expect("open");
    let seen = serve(&fake, 0, &["file:///tmp/a%20b.png"]);
    assert_eq!((seen.member.as_str(), seen.parent.as_str(), seen.title.as_str()), ("OpenFile", "wayland:exported", "Open File"));
    assert_eq!(seen.token, format!("pxui_dialog{id}"));
    assert_eq!((seen.multiple, seen.directory, &seen.filters, seen.folder.as_slice()), (false, false, &image_globs, &b"/tmp/\0"[..]));
    assert_eq!(answer(&mut portal), (id, Ok(vec!["/tmp/a b.png".to_owned()])));

    let id = portal.show("", &images, None, DialogKind::OpenFiles).expect("open several");
    let seen = serve(&fake, 0, &["file:///tmp/one.png", "file:///tmp/caf%C3%A9.jpg"]);
    assert!(seen.multiple && !seen.directory && seen.folder.is_empty() && seen.parent.is_empty());
    assert_eq!(answer(&mut portal), (id, Ok(vec!["/tmp/one.png".to_owned(), "/tmp/café.jpg".to_owned()])));

    let id = portal.show("", &images[..1], Some("/nowhere/out.png"), DialogKind::SaveFile).expect("save");
    let seen = serve(&fake, 0, &["file:///nowhere/renamed.png"]);
    assert_eq!((seen.member.as_str(), seen.name.as_str(), seen.folder.as_slice()), ("SaveFile", "out.png", &b"/nowhere\0"[..]));
    assert_eq!(answer(&mut portal), (id, Ok(vec!["/nowhere/renamed.png".to_owned()])));

    let id = portal.show("", &images, None, DialogKind::OpenFolder).expect("folder");
    let seen = serve(&fake, 0, &["file:///home"]);
    assert!(seen.directory && !seen.multiple && seen.filters.is_empty(), "a folder dialog has no file filters: {seen:?}");
    assert_eq!(answer(&mut portal), (id, Ok(vec!["/home".to_owned()])));

    let id = portal.show("", &[], None, DialogKind::OpenFile).expect("cancelled");
    serve(&fake, 1, &[]);
    assert_eq!(answer(&mut portal), (id, Ok(Vec::new())), "a cancel chooses nothing");

    let id = portal.show("", &[], None, DialogKind::OpenFile).expect("ended");
    serve(&fake, 2, &[]);
    assert!(matches!(answer(&mut portal), (got, Err(_)) if got == id), "a dialog that ended otherwise is an error");

    let id = portal.show("", &[], None, DialogKind::OpenFile).expect("not UTF-8");
    serve(&fake, 0, &["file:///latin1/caf%E9"]);
    assert!(matches!(answer(&mut portal), (got, Err(e)) if got == id && e.contains("not UTF-8")));

    // one dialog the portal accepted (its call answered, no Response yet) and seven unanswered calls
    let accepted = portal.show("", &[], None, DialogKind::OpenFile).expect("accepted");
    serve(&fake, u32::MAX, &[]);
    let mut events = Vec::new();
    portal.pump(&mut events);
    assert!(events.is_empty(), "an accepted dialog has no answer yet");
    let mut ids: Vec<i64> = (1..MAX_OPEN).map(|_| portal.show("", &[], None, DialogKind::OpenFile).expect("eight at once")).collect();
    ids.push(accepted);
    ids.sort();
    assert!(portal.show("", &[], None, DialogKind::OpenFile).is_err(), "a ninth open dialog is an error");
    drop(fake);
    let mut answered = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(3);
    while answered.len() < ids.len() && Instant::now() < deadline {
        let mut events = Vec::new();
        portal.pump(&mut events);
        answered.extend(events.into_iter().filter_map(|e| match e {
            Event::FileDialog { id, result: Err(_) } => Some(id),
            _ => None,
        }));
        std::thread::sleep(Duration::from_millis(5));
    }
    answered.sort();
    assert_eq!(answered, ids, "a portal that leaves the bus fails its open dialogs, the accepted one too (NameOwnerChanged)");
}
