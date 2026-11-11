//! The AT-SPI2 server: the application root and one object per tree node on the accessibility bus.
use crate::map;
use crate::wire::{Conn, ERROR, METHOD_CALL, METHOD_RETURN, Msg};
use pxui_core::a11y::{A11yAction, A11yDiff, A11yNode, A11yTree, Action, NO_PARENT, Role, Value, actions, change, diff, state};
use pxui_types::ffi::A11yTree as FfiA11yTree;
use std::io::Write as _;

pub const ROOT_PATH: &str = "/org/a11y/atspi/accessible/root";
pub const NODE_PREFIX: &str = "/org/a11y/atspi/accessible/";
pub const NULL_PATH: &str = "/org/a11y/atspi/null";
const REGISTRY: &str = "org.a11y.atspi.Registry";
const ACCESSIBLE: &str = "org.a11y.atspi.Accessible";
const APPLICATION: &str = "org.a11y.atspi.Application";
const COMPONENT: &str = "org.a11y.atspi.Component";
const ACTION: &str = "org.a11y.atspi.Action";
const VALUE: &str = "org.a11y.atspi.Value";
const TEXT: &str = "org.a11y.atspi.Text";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";
const EVENT_OBJECT: &str = "org.a11y.atspi.Event.Object";
const CACHE: &str = "org.a11y.atspi.Cache";
const CACHE_PATH: &str = "/org/a11y/atspi/cache";
const UNKNOWN_METHOD: &str = "org.freedesktop.DBus.Error.UnknownMethod";
const UNKNOWN_OBJECT: &str = "org.freedesktop.DBus.Error.UnknownObject";
const INVALID_ARGS: &str = "org.freedesktop.DBus.Error.InvalidArgs";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// No accessibility bus is running (no session bus, no `org.a11y.Bus`, or it refused us): the app runs
    /// without a bridge.
    NoBus(String),
    /// The bus answered but the bridge could not be set up.
    Failed(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Error::NoBus(e) => write!(f, "no accessibility bus: {e}"),
            Error::Failed(e) => write!(f, "accessibility bridge: {e}"),
        }
    }
}

/// The accessibility bus address: `AT_SPI_BUS_ADDRESS`, else `org.a11y.Bus.GetAddress` on the session bus at
/// `session` (`None`: the session bus from the environment).
pub fn a11y_bus_address(session: Option<&str>) -> Result<String, Error> {
    if session.is_none() {
        if let Some(address) = std::env::var("AT_SPI_BUS_ADDRESS").ok().filter(|a| !a.is_empty()) {
            return Ok(address);
        }
    }
    let conn = Conn::open(session).map_err(Error::NoBus)?;
    let mut reply = conn.call(Msg::call("org.a11y.Bus", "/org/a11y/bus", "org.a11y.Bus", "GetAddress"), 3000).map_err(Error::NoBus)?;
    reply.read_str().filter(|a| !a.is_empty()).map(str::to_owned).ok_or_else(|| Error::NoBus("org.a11y.Bus gave no address".into()))
}

/// An object path for a node id, or the root, on the stack.
struct Path {
    buf: [u8; 64],
    len: usize,
}

impl Path {
    fn of(id: Option<u64>) -> Path {
        let mut p = Path { buf: [0; 64], len: 0 };
        let mut w = &mut p.buf[..];
        match id {
            Some(id) => write!(w, "{NODE_PREFIX}{id}"),
            None => write!(w, "{ROOT_PATH}"),
        }
        .expect("path fits");
        p.len = 64 - w.len();
        p
    }
    fn as_str(&self) -> &str {
        std::str::from_utf8(&self.buf[..self.len]).unwrap_or(ROOT_PATH)
    }
}

/// What an object path names.
#[derive(Clone, Copy, PartialEq)]
enum Target {
    Root,
    Node(u32),
}

/// An event's variant: an int, a string or a reference to one of our objects (both given as text), a double.
#[derive(Clone, Copy)]
enum Any {
    Int(i32),
    Str,
    Num(f64),
    Obj,
    Rect(i32, i32, i32, i32),
}

pub struct Bridge {
    conn: Conn,
    app_name: String,
    unique: String,
    /// the registry's root after a successful Embed: (bus name, path)
    parent: Option<(String, String)>,
    /// the serial of the Socket.Embed call `pump` still waits for (0: answered)
    embed: u32,
    app_id: i32,
    tree: A11yTree,
    diff: A11yDiff,
    action_buf: Vec<A11yAction>,
}

impl Bridge {
    /// Connect to the accessibility bus of this session and register `app_name` with the registry.
    pub fn connect(app_name: &str) -> Result<Bridge, Error> {
        Bridge::connect_to(&a11y_bus_address(None)?, app_name)
    }

    /// Connect to the accessibility bus at `address`. Without a registry on it (a plain bus) the bridge still
    /// serves its objects; `embedded` reports which.
    pub fn connect_to(address: &str, app_name: &str) -> Result<Bridge, Error> {
        let conn = Conn::open(Some(address)).map_err(Error::NoBus)?;
        let unique = conn.unique_name();
        // Embedding is asynchronous: `pump` takes the registry's answer, so a slow registry never stalls startup.
        let mut embed = Msg::call(REGISTRY, ROOT_PATH, "org.a11y.atspi.Socket", "Embed");
        embed.open(b'r', "").s(&unique).o(ROOT_PATH).close();
        let embed = conn.send(embed).unwrap_or(0);
        conn.flush();
        Ok(Bridge { conn, app_name: app_name.to_owned(), unique, parent: None, embed, app_id: 0, tree: A11yTree::new(), diff: A11yDiff::default(), action_buf: Vec::new() })
    }

    /// Whether the registry accepted the application (Socket.Embed answered; `pump` reads the answer).
    pub fn embedded(&self) -> bool {
        self.parent.is_some()
    }

    /// Whether the registry has yet to answer Socket.Embed (no registry answers with an error).
    pub fn embedding(&self) -> bool {
        self.embed != 0
    }

    /// This connection's unique bus name: the bus name of every object the bridge serves.
    pub fn bus_name(&self) -> &str {
        &self.unique
    }

    /// The connection's file descriptor, for the host's poll loop: readable means `pump` has work.
    pub fn fd(&self) -> i32 {
        self.conn.fd()
    }

    /// Serve `tree` from now on and emit the change events against the tree served before. Returns at once,
    /// allocation-free, when the revision is unchanged.
    pub fn update(&mut self, tree: &A11yTree) {
        if tree.revision == self.tree.revision {
            return;
        }
        let mut d = std::mem::take(&mut self.diff);
        diff(&self.tree, tree, &mut d);
        self.emit(&self.tree, tree, &d);
        self.diff = d;
        self.tree.copy_from(tree);
        self.conn.flush();
    }

    /// Serve the public ABI tree while its backing UI output is alive.
    ///
    /// # Safety
    /// Every non-empty pointer in `tree` must reference readable memory for the duration of this call.
    pub unsafe fn update_ffi(&mut self, tree: &FfiA11yTree) -> Result<(), &'static str> {
        if tree.revision == self.tree.revision { return Ok(()); }
        let converted = unsafe { copy_ffi_tree(tree)? };
        self.update(&converted);
        Ok(())
    }

    /// Answer every pending D-Bus call without blocking; requests for the UI are appended to `out`. Returns
    /// false once the bus has gone.
    pub fn pump(&mut self, out: &mut Vec<A11yAction>) -> bool {
        loop {
            match self.conn.pop() {
                Err(()) => return false,
                Ok(None) => break,
                Ok(Some(mut msg)) => {
                    if msg.kind() == METHOD_CALL {
                        let reply = self.handle(&mut msg, out);
                        self.conn.send(reply);
                    } else if self.embed != 0 && msg.reply_serial() == self.embed && (msg.kind() == METHOD_RETURN || msg.kind() == ERROR) {
                        self.embed = 0;
                        if msg.kind() == METHOD_RETURN && msg.enter() {
                            self.parent = (|| Some((msg.read_str()?.to_owned(), msg.read_str()?.to_owned())))();
                        }
                    }
                }
            }
        }
        self.conn.flush();
        true
    }

    /// Public-ABI counterpart to `pump`; actions remain in the bridge's reusable buffer between calls.
    pub fn pump_ffi(&mut self, out: &mut Vec<pxui_types::ffi::A11yAction>) -> bool {
        let mut pending = std::mem::take(&mut self.action_buf);
        pending.clear();
        let alive = self.pump(&mut pending);
        out.extend(pending.iter().map(|a| pxui_types::ffi::A11yAction {
            id: a.id, action_code: a.action as u32, reserved: 0, payload: a.payload, end: a.end as u32, reserved_tail: 0,
        }));
        self.action_buf = pending;
        alive
    }

    // ------------------------------------------------------------------ events

    fn signal(&self, path: &str, iface: &str, member: &str, detail: &str, d1: i32, d2: i32, any: Any, text: &str) {
        let mut m = Msg::signal(path, iface, member);
        m.s(detail).i(d1).i(d2);
        match any {
            Any::Int(v) => m.open(b'v', "i").i(v).close(),
            Any::Str => m.open(b'v', "s").s(text).close(),
            Any::Num(v) => m.open(b'v', "d").d(v).close(),
            Any::Obj => m.open(b'v', "(so)").open(b'r', "").s(&self.unique).o(text).close().close(),
            Any::Rect(x, y, w, h) => m.open(b'v', "(iiii)").open(b'r', "").i(x).i(y).i(w).i(h).close().close(),
        };
        m.open(b'a', "{sv}").close();
        self.conn.send(m);
    }

    fn emit(&self, old: &A11yTree, new: &A11yTree, d: &A11yDiff) {
        let parent_path = |tree: &A11yTree, n: &A11yNode| Path::of((n.parent != NO_PARENT).then(|| tree.nodes[n.parent as usize].id));
        // The cache follows every node; a removed or added subtree's events are announced once, at its top.
        for &id in &d.removed {
            let mut m = Msg::signal(CACHE_PATH, CACHE, "RemoveAccessible");
            m.open(b'r', "").s(&self.unique).o(Path::of(Some(id)).as_str()).close();
            self.conn.send(m);
        }
        for &id in &d.added {
            let mut m = Msg::signal(CACHE_PATH, CACHE, "AddAccessible");
            self.cache_item(&mut m, new, Target::Node(new.find(id).expect("added id is in the new tree")));
            self.conn.send(m);
        }
        for &id in &d.removed {
            let i = old.find(id).expect("removed id is in the old tree");
            let n = &old.nodes[i as usize];
            if n.parent == NO_PARENT || new.find(old.nodes[n.parent as usize].id).is_some() {
                let child = Path::of(Some(id));
                self.signal(parent_path(old, n).as_str(), EVENT_OBJECT, "ChildrenChanged", "remove", old.index_in_parent(i) as i32, 0, Any::Obj, child.as_str());
            }
        }
        for &id in &d.added {
            let i = new.find(id).expect("added id is in the new tree");
            let n = &new.nodes[i as usize];
            if n.parent == NO_PARENT || old.find(new.nodes[n.parent as usize].id).is_some() {
                let child = Path::of(Some(id));
                self.signal(parent_path(new, n).as_str(), EVENT_OBJECT, "ChildrenChanged", "add", new.index_in_parent(i) as i32, 0, Any::Obj, child.as_str());
            }
        }
        for &(id, mask) in &d.changed {
            let (o, n) = (&old.nodes[old.find(id).unwrap() as usize], &new.nodes[new.find(id).unwrap() as usize]);
            let path = Path::of(Some(id));
            let path = path.as_str();
            if mask & change::NAME != 0 {
                self.signal(path, EVENT_OBJECT, "PropertyChange", "accessible-name", 0, 0, Any::Str, new.str(n.name));
            }
            if mask & change::DESCRIPTION != 0 {
                self.signal(path, EVENT_OBJECT, "PropertyChange", "accessible-description", 0, 0, Any::Str, new.str(n.description));
            }
            if mask & change::VALUE != 0 {
                match n.value {
                    Value::Number { now, .. } => self.signal(path, EVENT_OBJECT, "PropertyChange", "accessible-value", 0, 0, Any::Num(now), ""),
                    _ if is_text(n) => {
                        // only what changed between the common prefix and suffix
                        let (at, gone, added) = map::text_edit(old.value_text(o), new.value_text(n));
                        if !gone.is_empty() {
                            self.signal(path, EVENT_OBJECT, "TextChanged", "delete", at, gone.chars().count() as i32, Any::Str, gone);
                        }
                        if !added.is_empty() {
                            self.signal(path, EVENT_OBJECT, "TextChanged", "insert", at, added.chars().count() as i32, Any::Str, added);
                        }
                    }
                    _ => {}
                }
            }
            if mask & change::STATES != 0 {
                for (bit, atspi) in map::STATES {
                    if (o.states ^ n.states) & bit != 0 {
                        let on = (n.states & bit != 0) as i32;
                        for (_, detail) in *atspi {
                            self.signal(path, EVENT_OBJECT, "StateChanged", detail, on, 0, Any::Int(0), "");
                        }
                    }
                }
                if n.states & !o.states & state::FOCUSED != 0 {
                    self.signal(path, "org.a11y.atspi.Event.Focus", "Focus", "", 0, 0, Any::Int(0), "");
                }
            }
            if mask & change::SELECTION != 0 && is_text(n) {
                self.signal(path, EVENT_OBJECT, "TextCaretMoved", "", n.selection.1 as i32, 0, Any::Int(0), "");
                if o.selection.0 != o.selection.1 || n.selection.0 != n.selection.1 {
                    self.signal(path, EVENT_OBJECT, "TextSelectionChanged", "", 0, 0, Any::Int(0), "");
                }
            }
            if mask & change::RECT != 0 {
                let r = n.rect;
                let at = |v: f32| v.round() as i32;
                self.signal(path, EVENT_OBJECT, "BoundsChanged", "", 0, 0, Any::Rect(at(r.x), at(r.y), at(r.w), at(r.h)), "");
            }
            if mask & change::ROLE != 0 {
                self.signal(path, EVENT_OBJECT, "PropertyChange", "accessible-role", 0, 0, Any::Int(map::role(n.role).0 as i32), "");
            }
            if mask & change::PARENT != 0 {
                let parent = parent_path(new, n);
                self.signal(path, EVENT_OBJECT, "PropertyChange", "accessible-parent", 0, 0, Any::Obj, parent.as_str());
            }
        }
    }

    // ------------------------------------------------------------------ calls

    /// One Cache item of `target` in `tree`: object, application, parent, index in parent, child count,
    /// interfaces, name, role, description, states.
    fn cache_item(&self, m: &mut Msg, tree: &A11yTree, target: Target) {
        let node = match target {
            Target::Root => None,
            Target::Node(i) => Some(&tree.nodes[i as usize]),
        };
        let object = Path::of(node.map(|n| n.id));
        m.open(b'r', "").open(b'r', "").s(&self.unique).o(object.as_str()).close();
        m.open(b'r', "").s(&self.unique).o(ROOT_PATH).close();
        match (node, &self.parent) {
            (None, Some((bus, path))) => m.open(b'r', "").s(bus).o(path).close(),
            (None, None) => m.open(b'r', "").s(&self.unique).o(NULL_PATH).close(),
            (Some(n), _) => {
                let parent = Path::of((n.parent != NO_PARENT).then(|| tree.nodes[n.parent as usize].id));
                m.open(b'r', "").s(&self.unique).o(parent.as_str()).close()
            }
        };
        let index = match target {
            Target::Root => -1,
            Target::Node(i) => tree.index_in_parent(i) as i32,
        };
        m.i(index).i(tree.child_count(Bridge::index(target)) as i32);
        interfaces(m, node);
        match node {
            None => m.s(&self.app_name).u(map::ROLE_APPLICATION).s(""),
            Some(n) => m.s(tree.str(n.name)).u(map::role(n.role).0).s(tree.str(n.description)),
        };
        let set = node.map_or([0, 0], map::states);
        m.open(b'a', "u").u(set[0]).u(set[1]).close().close();
    }

    fn target(&self, path: &str) -> Option<Target> {
        if path == ROOT_PATH {
            return Some(Target::Root);
        }
        let id = path.strip_prefix(NODE_PREFIX)?.parse::<u64>().ok()?;
        self.tree.find(id).map(Target::Node)
    }

    fn put_ref(&self, m: &mut Msg, target: Option<Target>) {
        let path = match target {
            None => Path { buf: [0; 64], len: 0 },
            Some(Target::Root) => Path::of(None),
            Some(Target::Node(i)) => Path::of(Some(self.tree.nodes[i as usize].id)),
        };
        m.open(b'r', "").s(&self.unique).o(if path.len == 0 { NULL_PATH } else { path.as_str() }).close();
    }

    fn parent_of(&self, target: Target) -> Option<Target> {
        match target {
            Target::Root => None,
            Target::Node(i) => {
                let p = self.tree.nodes[i as usize].parent;
                Some(if p == NO_PARENT { Target::Root } else { Target::Node(p) })
            }
        }
    }

    fn put_parent(&self, m: &mut Msg, target: Target) {
        match (target, &self.parent) {
            (Target::Root, Some((bus, path))) => {
                m.open(b'r', "").s(bus).o(path).close();
            }
            _ => self.put_ref(m, self.parent_of(target)),
        }
    }

    fn index(target: Target) -> u32 {
        match target {
            Target::Root => NO_PARENT,
            Target::Node(i) => i,
        }
    }

    fn node(&self, target: Target) -> Option<&A11yNode> {
        match target {
            Target::Root => None,
            Target::Node(i) => Some(&self.tree.nodes[i as usize]),
        }
    }

    /// Write property `prop` of `iface` as a variant; false when there is no such property.
    fn put_property(&self, m: &mut Msg, target: Target, iface: &str, prop: &str) -> bool {
        let node = self.node(target);
        let text_node = node.is_some_and(is_text);
        match (iface, prop, node.is_some()) {
            (ACCESSIBLE, "Name", _) => {
                m.open(b'v', "s").s(if node.is_some() { self.tree.str(node.unwrap().name) } else { &self.app_name }).close();
            }
            (ACCESSIBLE, "Description", _) => {
                m.open(b'v', "s").s(if node.is_some() { self.tree.str(node.unwrap().description) } else { "" }).close();
            }
            (ACCESSIBLE, "Parent", _) => {
                m.open(b'v', "(so)");
                self.put_parent(m, target);
                m.close();
            }
            (ACCESSIBLE, "ChildCount", _) => {
                m.open(b'v', "i").i(self.tree.child_count(Bridge::index(target)) as i32).close();
            }
            (ACCESSIBLE, "Locale" | "AccessibleId" | "HelpText", _) => {
                m.open(b'v', "s").s("").close();
            }
            (APPLICATION, "ToolkitName", false) => {
                m.open(b'v', "s").s("PXUI").close();
            }
            (APPLICATION, "Version", false) => {
                m.open(b'v', "s").s(env!("CARGO_PKG_VERSION")).close();
            }
            (APPLICATION, "AtspiVersion", false) => {
                m.open(b'v', "s").s("2.1").close();
            }
            (APPLICATION, "Id", false) => {
                m.open(b'v', "i").i(self.app_id).close();
            }
            (ACTION, "NActions", true) => {
                m.open(b'v', "i").i(map::node_action_count(node.unwrap()) as i32).close();
            }
            (VALUE, _, true) => {
                let Value::Number { now, min, max, step } = node.unwrap().value else { return false };
                match prop {
                    "CurrentValue" => m.open(b'v', "d").d(now).close(),
                    "MinimumValue" => m.open(b'v', "d").d(min).close(),
                    "MaximumValue" => m.open(b'v', "d").d(max).close(),
                    "MinimumIncrement" => m.open(b'v', "d").d(step).close(),
                    "Text" => m.open(b'v', "s").s("").close(),
                    _ => return false,
                };
            }
            (TEXT, "CharacterCount", true) if text_node => {
                m.open(b'v', "i").i(self.tree.value_text(node.unwrap()).chars().count() as i32).close();
            }
            (TEXT, "CaretOffset", true) if text_node => {
                m.open(b'v', "i").i(node.unwrap().selection.1 as i32).close();
            }
            _ => return false,
        }
        true
    }

    fn handle(&mut self, msg: &mut Msg, out: &mut Vec<A11yAction>) -> Msg {
        if msg.path() == "/org/a11y/atspi/cache" && msg.interface() == "org.a11y.atspi.Cache" && msg.member() == "GetItems" {
            // every object, so a reader walks the tree without a call per object; update keeps it current
            let mut r = msg.reply();
            r.open(b'a', "((so)(so)(so)iiassusau)");
            self.cache_item(&mut r, &self.tree, Target::Root);
            for i in 0..self.tree.nodes.len() as u32 {
                self.cache_item(&mut r, &self.tree, Target::Node(i));
            }
            r.close();
            return r;
        }
        let Some(target) = self.target(msg.path()) else {
            return msg.error(UNKNOWN_OBJECT, "no such accessible");
        };
        let (iface, member) = (msg.interface().to_owned(), msg.member().to_owned());
        let mut r = msg.reply();
        let node = self.node(target).copied();
        let id = node.map_or(0, |n| n.id);
        let ok = match (iface.as_str(), member.as_str()) {
            (PROPERTIES, "Get") => {
                let (Some(i), Some(p)) = (msg.read_str().map(str::to_owned), msg.read_str().map(str::to_owned)) else {
                    return msg.error(INVALID_ARGS, "Get(ss)");
                };
                if !self.put_property(&mut r, target, &i, &p) {
                    return msg.error(INVALID_ARGS, "no such property");
                }
                true
            }
            (PROPERTIES, "GetAll") => {
                let Some(i) = msg.read_str().map(str::to_owned) else { return msg.error(INVALID_ARGS, "GetAll(s)") };
                const NAMES: &[&str] = &[
                    "Name", "Description", "Parent", "ChildCount", "Locale", "AccessibleId", "HelpText", "ToolkitName", "Version",
                    "AtspiVersion", "Id", "NActions", "CurrentValue", "MinimumValue", "MaximumValue", "MinimumIncrement", "Text",
                    "CharacterCount", "CaretOffset",
                ];
                r.open(b'a', "{sv}");
                for p in NAMES {
                    // designed limit (specification/pxui.md): a probe message per name keeps one property writer; fine for GetAll's rare use.
                    let mut probe = Msg::signal(ROOT_PATH, ACCESSIBLE, "Probe");
                    if self.put_property(&mut probe, target, &i, p) {
                        r.open(b'e', "").s(p);
                        self.put_property(&mut r, target, &i, p);
                        r.close();
                    }
                }
                r.close();
                true
            }
            (PROPERTIES, "Set") => {
                let (Some(i), Some(p)) = (msg.read_str().map(str::to_owned), msg.read_str().map(str::to_owned)) else {
                    return msg.error(INVALID_ARGS, "Set(ssv)");
                };
                msg.enter();
                match (i.as_str(), p.as_str(), node) {
                    (APPLICATION, "Id", None) => self.app_id = msg.read_i32().unwrap_or(0),
                    (VALUE, "CurrentValue", Some(n)) if n.actions & actions::SET_VALUE != 0 => {
                        let Some(v) = msg.read_f64().filter(|v| v.is_finite()) else { return msg.error(INVALID_ARGS, "CurrentValue is a double") };
                        out.push(A11yAction { id, action: Action::SetValue, payload: v, end: 0.0 });
                    }
                    _ => return msg.error(INVALID_ARGS, "property is read-only"),
                }
                true
            }
            (ACCESSIBLE, "GetChildAtIndex") => {
                let child = msg.read_i32().and_then(|k| u32::try_from(k).ok()).and_then(|k| self.tree.child(Bridge::index(target), k));
                self.put_ref(&mut r, child.map(Target::Node));
                true
            }
            (ACCESSIBLE, "GetChildren") => {
                r.open(b'a', "(so)");
                let mut c = self.tree.first_child(Bridge::index(target));
                while c != NO_PARENT {
                    self.put_ref(&mut r, Some(Target::Node(c)));
                    c = self.tree.nodes[c as usize].next_sibling;
                }
                r.close();
                true
            }
            (ACCESSIBLE, "GetIndexInParent") => {
                r.i(match target {
                    Target::Root => -1,
                    Target::Node(i) => self.tree.index_in_parent(i) as i32,
                });
                true
            }
            (ACCESSIBLE, "GetRelationSet") => {
                // designed limit (specification/pxui.md): no relations (labelled-by, controller-for); add with the engine's label links.
                r.open(b'a', "(ua(so))").close();
                true
            }
            (ACCESSIBLE, "GetRole") => {
                r.u(node.map_or(map::ROLE_APPLICATION, |n| map::role(n.role).0));
                true
            }
            (ACCESSIBLE, "GetRoleName" | "GetLocalizedRoleName") => {
                r.s(node.map_or("application", |n| map::role(n.role).1));
                true
            }
            (ACCESSIBLE, "GetState") => {
                let set = node.map_or([0, 0], |n| map::states(&n));
                r.open(b'a', "u").u(set[0]).u(set[1]).close();
                true
            }
            (ACCESSIBLE, "GetAttributes") => {
                r.open(b'a', "{ss}").open(b'e', "").s("toolkit").s("PXUI").close().close();
                true
            }
            (ACCESSIBLE, "GetApplication") => {
                self.put_ref(&mut r, Some(Target::Root));
                true
            }
            (ACCESSIBLE, "GetInterfaces") => {
                interfaces(&mut r, self.node(target));
                true
            }
            (APPLICATION, "GetLocale") if node.is_none() => {
                r.s("");
                true
            }
            (COMPONENT, _) if node.is_some() => self.component(&member, msg, &mut r, node.unwrap(), out),
            (ACTION, _) if node.is_some_and(|n| map::has_action_interface(&n)) => {
                let n = node.unwrap();
                let pick = |msg: &mut Msg| msg.read_i32().and_then(|k| usize::try_from(k).ok()).and_then(|k| map::node_action(&n, k));
                match member.as_str() {
                    "GetName" | "GetLocalizedName" | "GetDescription" | "GetKeyBinding" => {
                        let Some(a) = pick(msg) else { return msg.error(INVALID_ARGS, "no such action") };
                        r.s(match member.as_str() {
                            "GetDescription" => a.2,
                            "GetKeyBinding" => "",
                            _ => a.1,
                        });
                        true
                    }
                    "GetActions" => {
                        r.open(b'a', "(sss)");
                        for a in (0..map::node_action_count(&n)).filter_map(|k| map::node_action(&n, k)) {
                            r.open(b'r', "").s(a.1).s(a.2).s("").close();
                        }
                        r.close();
                        true
                    }
                    "DoAction" => {
                        let Some(a) = pick(msg) else { return msg.error(INVALID_ARGS, "no such action") };
                        out.push(A11yAction { id, action: a.0, payload: 0.0, end: 0.0 });
                        r.b(true);
                        true
                    }
                    _ => false,
                }
            }
            (TEXT, _) if node.is_some_and(|n| is_text(&n)) => {
                let n = node.unwrap();
                let text = self.tree.value_text(&n);
                let count = text.chars().count() as i32;
                match member.as_str() {
                    "GetText" => {
                        let (Some(start), Some(end)) = (msg.read_i32(), msg.read_i32()) else { return msg.error(INVALID_ARGS, "GetText(ii)") };
                        r.s(chars(text, start, if end < 0 { count } else { end }));
                        true
                    }
                    "GetCharacterAtOffset" => {
                        let k = msg.read_i32().unwrap_or(-1);
                        r.i(usize::try_from(k).ok().and_then(|k| text.chars().nth(k)).map_or(0, |c| c as i32));
                        true
                    }
                    "GetStringAtOffset" => {
                        let (Some(k), Some(granularity)) = (msg.read_i32(), msg.read_u32()) else { return msg.error(INVALID_ARGS, "GetStringAtOffset(iu)") };
                        let (s, e) = map::text_span(text, k, granularity);
                        r.s(chars(text, s, e)).i(s).i(e);
                        true
                    }
                    "GetNSelections" => {
                        r.i((n.selection.0 != n.selection.1) as i32);
                        true
                    }
                    "GetSelection" => {
                        let (a, c) = n.selection;
                        r.i(a.min(c) as i32).i(a.max(c) as i32);
                        true
                    }
                    // caret and selection setting reach the field as a Select action (one selection at most)
                    "SetCaretOffset" | "SetSelection" | "AddSelection" | "RemoveSelection" => {
                        let ints: Vec<i32> = std::iter::from_fn(|| msg.read_i32()).take(3).collect();
                        let caret = n.selection.1 as i32;
                        let (anchor, to) = match (member.as_str(), ints.as_slice()) {
                            ("SetCaretOffset", [k]) => (*k, *k),
                            ("SetSelection", [0, s, e]) | ("AddSelection", [s, e]) => (*s, *e),
                            ("RemoveSelection", [0]) => (caret, caret),
                            ("SetSelection" | "RemoveSelection", [_, ..]) => {
                                r.b(false);
                                return r;
                            }
                            _ => return msg.error(INVALID_ARGS, "offsets are ints"),
                        };
                        let clamp = |k: i32| f64::from(k.clamp(0, count));
                        out.push(A11yAction { id, action: Action::Select, payload: clamp(anchor), end: clamp(to) });
                        r.b(true);
                        true
                    }
                    // PXUI text carries no attribute runs: one run of no attributes over the whole text
                    "GetAttributes" | "GetAttributeRun" => {
                        r.open(b'a', "{ss}").close().i(0).i(count);
                        true
                    }
                    "GetDefaultAttributes" => {
                        r.open(b'a', "{ss}").close();
                        true
                    }
                    "GetDefaultAttributeSet" => {
                        r.open(b'a', "s").close();
                        true
                    }
                    // designed limit (specification/pxui.md): glyph geometry (GetCharacterExtents, GetRangeExtents,
                    // GetOffsetAtPoint, GetBoundedRanges), EditableText and Hypertext are not served.
                    _ => false,
                }
            }
            _ => false,
        };
        if ok { r } else { msg.error(UNKNOWN_METHOD, "not implemented by the PXUI bridge") }
    }

    fn component(&self, member: &str, msg: &mut Msg, r: &mut Msg, n: A11yNode, out: &mut Vec<A11yAction>) -> bool {
        // designed limit (specification/pxui.md): window-relative only; "screen" coordinates equal window ones (Wayland gives no window position).
        let origin = |coord: u32| -> (f32, f32) {
            if coord == 2 && n.parent != NO_PARENT {
                let p = self.tree.nodes[n.parent as usize].rect;
                (p.x, p.y)
            } else {
                (0.0, 0.0)
            }
        };
        match member {
            "GetExtents" => {
                let (ox, oy) = origin(msg.read_u32().unwrap_or(0));
                r.open(b'r', "").i((n.rect.x - ox).round() as i32).i((n.rect.y - oy).round() as i32).i(n.rect.w.round() as i32).i(n.rect.h.round() as i32).close();
            }
            "GetPosition" => {
                let (ox, oy) = origin(msg.read_u32().unwrap_or(0));
                r.i((n.rect.x - ox).round() as i32).i((n.rect.y - oy).round() as i32);
            }
            "GetSize" => {
                r.i(n.rect.w.round() as i32).i(n.rect.h.round() as i32);
            }
            "Contains" | "GetAccessibleAtPoint" => {
                let (Some(x), Some(y)) = (msg.read_i32(), msg.read_i32()) else { return false };
                let (ox, oy) = origin(msg.read_u32().unwrap_or(0));
                let p = (x as f32 + ox, y as f32 + oy);
                if member == "Contains" {
                    r.b(n.rect.contains(p));
                } else {
                    // The deepest descendant under the point; later siblings are on top.
                    let start = self.tree.find(n.id).unwrap();
                    let mut at = n.rect.contains(p).then_some(start);
                    while let Some(i) = at {
                        let (mut c, mut hit) = (self.tree.first_child(i), None);
                        while c != NO_PARENT {
                            if self.tree.nodes[c as usize].rect.contains(p) {
                                hit = Some(c);
                            }
                            c = self.tree.nodes[c as usize].next_sibling;
                        }
                        if hit.is_none() {
                            break;
                        }
                        at = hit;
                    }
                    self.put_ref(r, at.map(Target::Node));
                }
            }
            "GetLayer" => {
                r.u(if n.role == Role::Window { 7 } else { 3 });
            }
            "GetMDIZOrder" => {
                r.n(0);
            }
            "GetAlpha" => {
                r.d(1.0);
            }
            "GrabFocus" => {
                let ok = n.actions & actions::FOCUS != 0;
                if ok {
                    out.push(A11yAction { id: n.id, action: Action::Focus, payload: 0.0, end: 0.0 });
                }
                r.b(ok);
            }
            "ScrollTo" => {
                out.push(A11yAction { id: n.id, action: Action::Scroll, payload: 0.0, end: 0.0 });
                r.b(true);
            }
            _ => return false,
        }
        true
    }
}

/// The interfaces an object (`None`: the application) serves.
fn interfaces(m: &mut Msg, node: Option<&A11yNode>) {
    m.open(b'a', "s").s(ACCESSIBLE);
    match node {
        None => {
            m.s(APPLICATION);
        }
        Some(n) => {
            m.s(COMPONENT);
            if map::has_action_interface(n) {
                m.s(ACTION);
            }
            if matches!(n.value, Value::Number { .. }) {
                m.s(VALUE);
            }
            if is_text(n) {
                m.s(TEXT);
            }
        }
    }
    m.close();
}

/// Nodes that serve the Text interface: editable fields and text areas.
fn is_text(n: &A11yNode) -> bool {
    n.states & state::EDITABLE != 0 || matches!(n.role, Role::TextField | Role::TextArea)
}

/// Chars `start..end` of `text` (clamped).
fn chars(text: &str, start: i32, end: i32) -> &str {
    let at = |k: i32| text.char_indices().nth(k.max(0) as usize).map_or(text.len(), |(b, _)| b);
    let (s, e) = (at(start), at(end));
    if s <= e { &text[s..e] } else { "" }
}

unsafe fn copy_ffi_tree(raw: &FfiA11yTree) -> Result<A11yTree, &'static str> {
    if raw.nodes.len > 1_000_000 || raw.strings.len > 16 * 1024 * 1024
        || (raw.nodes.len != 0 && raw.nodes.ptr.is_null())
        || (raw.strings.len != 0 && raw.strings.ptr.is_null())
    {
        return Err("invalid accessibility tree storage");
    }
    let nodes = if raw.nodes.len == 0 { &[] } else { unsafe { std::slice::from_raw_parts(raw.nodes.ptr, raw.nodes.len) } };
    let bytes = if raw.strings.len == 0 { &[] } else { unsafe { std::slice::from_raw_parts(raw.strings.ptr, raw.strings.len) } };
    let strings = std::str::from_utf8(bytes).map_err(|_| "invalid accessibility strings")?;
    let mut out = A11yTree::new();
    out.strings.push_str(strings);
    let mut ids = std::collections::HashSet::with_capacity(nodes.len());
    for (i, n) in nodes.iter().enumerate() {
        if (n.parent != NO_PARENT && n.parent as usize >= i) || n.role > Role::Separator as u32 || !ids.insert(n.id) {
            return Err("invalid accessibility node");
        }
        let range_ok = |start: u32, end: u32| {
            start <= end
                && end as usize <= strings.len()
                && strings.is_char_boundary(start as usize)
                && strings.is_char_boundary(end as usize)
        };
        if !range_ok(n.name_start, n.name_end) || !range_ok(n.description_start, n.description_end) {
            return Err("invalid accessibility string range");
        }
        let role = unsafe { std::mem::transmute::<u8, Role>(n.role as u8) };
        let index = out.push(n.parent, n.id, role, n.rect);
        let dst = &mut out.nodes[index as usize];
        dst.name = pxui_core::a11y::Str { start: n.name_start, end: n.name_end };
        dst.description = pxui_core::a11y::Str { start: n.description_start, end: n.description_end };
        dst.states = n.states;
        dst.actions = n.actions;
        dst.selection = (n.selection[0], n.selection[1]);
        dst.value = match n.value.tag {
            0 => Value::None,
            1 if range_ok(n.value.text_start, n.value.text_end) => Value::Text(pxui_core::a11y::Str { start: n.value.text_start, end: n.value.text_end }),
            2 => Value::Number { now: n.value.now, min: n.value.min, max: n.value.max, step: n.value.step },
            _ => return Err("invalid accessibility value"),
        };
    }
    out.end();
    out.revision = raw.revision;
    Ok(out)
}

#[cfg(test)]
mod ffi_tests {
    use super::*;
    use pxui_types::{Str, a11y::Role, ffi::{A11yNode as FfiNode, A11yValue, Slice}};

    #[test]
    fn public_tree_copy_keeps_ranges_and_parent_links() {
        let text = b"GalleryButton";
        let nodes = [FfiNode { id: 9, parent: NO_PARENT, role: Role::Button as u32, name_start: 0, name_end: 7, value: A11yValue::default(), ..FfiNode::default() },
            FfiNode { id: 10, parent: 0, role: Role::Label as u32, name_start: 7, name_end: 13, value: A11yValue::default(), ..FfiNode::default() }];
        let raw = FfiA11yTree { nodes: Slice { ptr: nodes.as_ptr(), len: nodes.len() }, strings: Str { ptr: text.as_ptr(), len: text.len() }, revision: 4, first_child: 0, child_count: 1, actions: Slice { ptr: std::ptr::null(), len: 0 } };
        let tree = unsafe { copy_ffi_tree(&raw) }.unwrap();
        assert_eq!(tree.revision, 4);
        assert_eq!(tree.child_count(NO_PARENT), 1);
        assert_eq!(tree.str(tree.nodes[1].name), "Button");
    }
}
