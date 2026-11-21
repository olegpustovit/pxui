use crate::state::{PxuiUi, checked, core_box_of, fail, get};
use crate::{PXUI_ERR_ARGUMENT, PxuiBox, Str};

export!(pub unsafe fn pxui_ui_a11y_name(ui: *mut PxuiUi, box_: *const PxuiBox, name: Str) -> i32 {
    unsafe { set_name(ui, box_, name, true) }
});

unsafe fn set_name(ui: *mut PxuiUi, box_: *const PxuiBox, name: Str, report_stale: bool) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if box_.is_null() { return unsafe { fail(ui, "null accessibility name box") }; }
    let Some(name) = (unsafe { checked(name) }) else { return unsafe { fail(ui, "invalid accessibility name") }; };
    let Some(box_) = (unsafe { core_box_of(ui, *box_) }) else { return unsafe { crate::state::fail_code(ui, "stale accessibility box", crate::PXUI_ERR_STALE) }; };
    let engine = unsafe { &mut *state.engine };
    if engine.live(&box_) { engine.a11y_name(&box_, name); }
    else if report_stale { return unsafe { crate::state::fail_code(ui, "stale accessibility name box", crate::PXUI_ERR_STALE) }; }
    0
}

export!(pub unsafe fn pxui_ui_a11y_value(ui:*mut PxuiUi,box_:*const PxuiBox,value:Str,anchor:u32,caret:u32)->i32 {
    if unsafe {get(ui)}.is_none() {return PXUI_ERR_ARGUMENT;}
    let Some(value)=(unsafe {checked(value)}) else {return unsafe {fail(ui,"invalid accessibility value")};};
    if !value.is_char_boundary(anchor as usize)||!value.is_char_boundary(caret as usize) {return unsafe {fail(ui,"invalid accessibility selection")};}
    let (engine,box_)=match unsafe {crate::state::boxed(ui,box_)} {Ok(value)=>value,Err(code)=>return code};
    if !engine.live(&box_) {return unsafe {crate::state::fail_code(ui,"stale accessibility value box",crate::PXUI_ERR_STALE)};}
    let selection=(value[..anchor as usize].chars().count() as u32,value[..caret as usize].chars().count() as u32);
    engine.a11y_value(&box_,value,selection);0
});

/// The accessibility bridge of a window the app owns (Linux: AT-SPI2 on the accessibility bus). The app feeds it each frame's tree
/// (`FrameOut`/`Output.a11y`) and takes the actions assistive technology asked for; elsewhere the platform's own bridge lives in
/// the host and `pxui_a11y_start` returns null.
pub struct PxuiA11y {
    #[cfg(target_os = "linux")]
    bridge: pxui_atspi::Bridge,
    #[cfg(target_os = "linux")]
    actions: Vec<(u64, pxui_types::ffi::A11yAction)>,
    #[cfg(target_os = "linux")]
    span: Vec<pxui_types::ffi::A11yAction>,
    pub(crate) poisoned: bool,
}

// Connects to the accessibility bus as `name`; null when there is none (the reason is not an error: no assistive technology).
export!(bare pub unsafe fn pxui_a11y_start(name: Str) -> *mut PxuiA11y {
    #[cfg(target_os = "linux")]
    {
        let Some(name) = (unsafe { crate::state::checked_key(name) }) else { return core::ptr::null_mut() };
        return match pxui_atspi::Bridge::connect(name) { Ok(bridge) => Box::into_raw(Box::new(PxuiA11y { bridge, actions: Vec::new(), span: Vec::new(), poisoned: false })), Err(_) => core::ptr::null_mut() };
    }
    #[cfg(not(target_os = "linux"))]
    { let _ = name; core::ptr::null_mut() }
});

// The descriptor the bridge's events wake: the app adds it to its event loop.
export!(object pub unsafe fn pxui_a11y_fd(a11y: *mut PxuiA11y) -> i32 {
    #[cfg(target_os = "linux")]
    { return unsafe { a11y.as_ref() }.map_or(-1, |a| a.bridge.fd()); }
    #[cfg(not(target_os = "linux"))]
    { let _ = a11y; -1 }
});

// Publishes `tree` (a frame's `Output.a11y`, borrowed for the call) as the tree of `surface` (not 0): the bridge serves one application whose
// windows are the surfaces, each keeping its own tree.
export!(object pub unsafe fn pxui_a11y_update(a11y: *mut PxuiA11y, surface: u64, tree: *const pxui_types::ffi::A11yTree) -> i32 {
    #[cfg(target_os = "linux")]
    {
        let (Some(a), Some(tree), true) = (unsafe { a11y.as_mut() }, sized!(tree), surface != 0) else { return PXUI_ERR_ARGUMENT };
        return if unsafe { a.bridge.update_ffi(surface, tree) }.is_ok() { 0 } else { crate::PXUI_ERR_STATE };
    }
    #[cfg(not(target_os = "linux"))]
    { let _ = (a11y, surface, tree); PXUI_ERR_ARGUMENT }
});

// Stops serving a closed surface; unknown nonzero ids are ignored. No application tree is retained for that surface afterward.
export!(object pub unsafe fn pxui_a11y_remove_surface(a11y: *mut PxuiA11y, surface: u64) -> i32 {
    #[cfg(target_os = "linux")]
    {
        let (Some(a), true) = (unsafe { a11y.as_mut() }, surface != 0) else { return PXUI_ERR_ARGUMENT };
        a.bridge.remove_surface(surface);
        return 0;
    }
    #[cfg(not(target_os = "linux"))]
    { let _ = (a11y, surface); PXUI_ERR_ARGUMENT }
});

// Handles the bus's pending traffic. 1: the connection is alive, 0: it is gone (stop the bridge). The actions assistive technology
// asked for are in `out` (borrowed until the next pump), for `Ui::inject_a11y`.
export!(object pub unsafe fn pxui_a11y_pump(a11y: *mut PxuiA11y, out: *mut pxui_types::ffi::A11yActionSpan) -> i32 {
    #[cfg(target_os = "linux")]
    {
        let (Some(a), false) = (unsafe { a11y.as_mut() }, out.is_null()) else { return PXUI_ERR_ARGUMENT };
        a.actions.clear();
        let alive = a.bridge.pump_ffi(&mut a.actions);
        a.span.clear();
        a.span.extend(a.actions.iter().map(|(surface, action)| pxui_types::ffi::A11yAction { surface: *surface, ..*action }));
        unsafe { *out = pxui_types::ffi::Strided::of(&a.span); }
        return alive as i32;
    }
    #[cfg(not(target_os = "linux"))]
    { let _ = (a11y, out); PXUI_ERR_ARGUMENT }
});

export!(bare pub unsafe fn pxui_a11y_stop(a11y: *mut PxuiA11y) {
    #[cfg(target_os = "linux")]
    if !a11y.is_null() { drop(unsafe { Box::from_raw(a11y) }); }
    #[cfg(not(target_os = "linux"))]
    { let _ = a11y; }
});

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use pxui_atspi::wire::{Conn, Msg, ERROR, METHOD_RETURN};
    use pxui_types::ffi::{A11yAction, A11yActionSpan};
    use std::{io::{BufRead, BufReader}, process::{Child, Command, Stdio}, time::{Duration, Instant}};

    struct Bus(Child);
    impl Drop for Bus { fn drop(&mut self) { let _ = self.0.kill(); let _ = self.0.wait(); } }
    struct Bridge(*mut PxuiA11y);
    impl Drop for Bridge { fn drop(&mut self) { unsafe { pxui_a11y_stop(self.0); } } }
    struct Ui(*mut PxuiUi);
    impl Drop for Ui { fn drop(&mut self) { unsafe { crate::pxui_ui_destroy(self.0); } } }

    fn tree(surface: u64) -> Ui {
        extern "C" fn build(user: *mut core::ffi::c_void, raw: *mut core::ffi::c_void) {
            let text = Str { ptr: b"Button".as_ptr(), len: 6 };
            let mut response = pxui_types::ffi::PxuiResponse::EMPTY;
            unsafe { *user.cast::<i32>() = crate::pxui_ui_button_plain(raw.cast(), text, text, &mut response); }
        }
        let ui = Ui(unsafe { crate::ui::create_test_ui() });
        assert!(!ui.0.is_null());
        assert_eq!(unsafe { crate::pxui_ui_remove_surface(ui.0, pxui_types::FrameIn::MAIN) }, 0);
        assert_eq!(unsafe { crate::pxui_ui_add_surface(ui.0, surface) }, 0);
        assert_eq!(unsafe { crate::pxui_ui_set_a11y(ui.0, 1) }, 0);
        let mut status = i32::MIN;
        let input = pxui_types::FrameIn { width: 320.0, height: 240.0, surface, ..Default::default() };
        assert_eq!(unsafe { crate::pxui_frame(ui.0, &input, Some(build), (&mut status as *mut i32).cast(), &mut pxui_types::FrameOut::default()) }, 0);
        assert_eq!(status, 0);
        ui
    }

    fn call(bridge: &Bridge, conn: &Conn, message: Msg, actions: &mut Vec<A11yAction>) -> Msg {
        let serial = conn.send(message).unwrap();
        conn.flush();
        let deadline = Instant::now() + Duration::from_secs(1);
        while Instant::now() < deadline {
            let mut span = A11yActionSpan::default();
            assert_eq!(unsafe { pxui_a11y_pump(bridge.0, &mut span) }, 1);
            for i in 0..span.len { actions.push(unsafe { *span.at(i) }); }
            if let Some(reply) = conn.wait(1).unwrap() {
                if reply.reply_serial() == serial && matches!(reply.kind(), METHOD_RETURN | ERROR) { return reply; }
            }
        }
        panic!("the private accessibility bus did not answer");
    }

    // Real private D-Bus integration: service startup and protocol round trips are the work under test.
    #[test]
    fn c_bridge_routes_equal_node_ids_by_surface_and_removes_closed_trees() {
        let mut bus = Bus(Command::new("dbus-daemon").args(["--session", "--nofork", "--nopidfile", "--print-address=1"])
            .stdout(Stdio::piped()).stderr(Stdio::null()).spawn().expect("dbus-daemon is required for Linux accessibility qualification"));
        let mut address = String::new();
        BufReader::new(bus.0.stdout.take().unwrap()).read_line(&mut address).unwrap();
        let address = address.trim();
        let uis = [tree(7), tree(9)];
        let bridge = Bridge(Box::into_raw(Box::new(PxuiA11y {
            bridge: pxui_atspi::Bridge::connect_to(address, "C surface routing").unwrap(), actions: Vec::new(), span: Vec::new(), poisoned: false,
        })));
        assert!(unsafe { pxui_a11y_fd(bridge.0) } >= 0);
        let name = unsafe { (*bridge.0).bridge.bus_name() }.to_owned();
        let mut ids = Vec::new();
        for (ui, surface) in uis.iter().zip([7, 9]) {
            let tree = unsafe { &(*crate::pxui_ui_output(ui.0)).a11y };
            let nodes = unsafe { core::slice::from_raw_parts(tree.nodes.ptr, tree.nodes.len) };
            ids.push(nodes.iter().find(|n| n.role == pxui_types::a11y::Role::Button as u32).unwrap().id);
            assert_eq!(unsafe { pxui_a11y_update(bridge.0, surface, tree) }, 0);
        }
        assert_eq!(ids[0], ids[1]);
        let conn = Conn::open(Some(address)).unwrap();
        let mut actions = Vec::new();
        for surface in [7, 9] {
            let path = format!("{}{surface}/{}", pxui_atspi::NODE_PREFIX, ids[0]);
            let mut request = Msg::call(&name, &path, "org.a11y.atspi.Action", "DoAction");
            request.i(0);
            let mut reply = call(&bridge, &conn, request, &mut actions);
            assert_eq!(reply.kind(), METHOD_RETURN);
            assert_eq!(reply.read_bool(), Some(true));
        }
        assert_eq!(actions.iter().map(|a| (a.surface, a.id, a.action_code)).collect::<Vec<_>>(), [(7, ids[0], 1), (9, ids[0], 1)]);
        assert!(actions.iter().all(|a| a.size as usize == core::mem::size_of::<A11yAction>()));
        assert_eq!(unsafe { pxui_a11y_remove_surface(bridge.0, 0) }, PXUI_ERR_ARGUMENT);
        assert_eq!(unsafe { pxui_a11y_remove_surface(bridge.0, 9) }, 0);
        assert_eq!(unsafe { pxui_a11y_remove_surface(bridge.0, 9) }, 0);
        let request = Msg::call(&name, pxui_atspi::ROOT_PATH, "org.a11y.atspi.Accessible", "GetChildren");
        let mut reply = call(&bridge, &conn, request, &mut actions);
        assert_eq!(reply.kind(), METHOD_RETURN);
        assert!(reply.enter());
        let mut children = Vec::new();
        while reply.arg() == b'r' {
            assert!(reply.enter());
            reply.read_str();
            children.push(reply.read_str().unwrap().to_owned());
            reply.leave();
        }
        assert_eq!(children.len(), 1);
        assert!(children[0].starts_with(&format!("{}7/", pxui_atspi::NODE_PREFIX)));
        let path = format!("{}9/{}", pxui_atspi::NODE_PREFIX, ids[0]);
        let mut request = Msg::call(&name, &path, "org.a11y.atspi.Action", "DoAction");
        request.i(0);
        assert_eq!(call(&bridge, &conn, request, &mut actions).kind(), ERROR);
        assert_eq!(actions.len(), 2, "a removed surface dispatches no request");
    }
}
