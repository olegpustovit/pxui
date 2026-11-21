//! VoiceOver: pxui's accessibility tree published as NSAccessibility elements (sdl3_sys `ax`), and the
//! client's actions fed back as `inject_a11y` input. The tree is built only while VoiceOver runs.
use crate::Window;
use pxui_sdl::ax::{self, Action, Bridge, Node, Value};

/// Polls the system for VoiceOver once per this many frames.
const POLL_FRAMES: u32 = 30;

pub struct Accessibility {
    bridge: Bridge,
    nodes: Vec<Node>,
    enabled: bool,
    forced: bool,
    listening: bool,
    polls: u32,
    revision: Option<u64>,
}

extern "C" fn wake() {
    pxui_sdl::wake();
}

impl Accessibility {
    /// Attaches to the window's content view; call once after the window opens, on the main thread.
    /// `PXUI_ACCESSIBILITY=1` keeps the tree on without VoiceOver (Accessibility Inspector, tests).
    /// Returns `Error::Host` if the window token or native bridge is refused.
    pub fn open(window: &mut Window) -> Result<Self, pxui::Error> {
        let token = window.token().map_err(|_| pxui::Error::Host("could not get accessibility window token"))?;
        let bridge = Bridge::open(&token, wake).map_err(|_| pxui::Error::Host("could not open accessibility bridge"))?;
        let forced = std::env::var_os("PXUI_ACCESSIBILITY").is_some_and(|v| v == "1");
        Ok(Self { bridge, nodes: Vec::new(), enabled: false, forced, listening: false, polls: 0, revision: None })
    }

    /// Keeps the tree on regardless of VoiceOver.
    pub fn force(&mut self, on: bool) {
        self.forced = on;
    }

    /// Call once per frame before `Ui::frame`: follows VoiceOver on/off, publishes the previous frame's
    /// tree when its revision changed, and injects the client's queued actions for the frame's registered `surface`.
    pub fn frame(&mut self, ui: &mut pxui::Ui, surface: u64) {
        if self.polls == 0 {
            self.listening = Bridge::voiceover();
        }
        self.polls = (self.polls + 1) % POLL_FRAMES;
        let wanted = self.forced || self.listening;
        if wanted != self.enabled {
            ui.set_a11y(wanted);
            self.enabled = wanted;
            if !wanted {
                self.bridge.clear();
                self.revision = None;
            }
        }
        if !self.enabled {
            return;
        }
        let pxui::A11yTree { strings, revision, first_child: first_top } = ui.a11y_tree();
        if self.revision != Some(revision) {
            self.nodes.clear();
            self.nodes.extend(ui.a11y_nodes().iter().map(|n| Node {
                id: n.id,
                parent: n.parent,
                role: n.role,
                name: [n.name_start, n.name_end],
                description: [n.description_start, n.description_end],
                value: Value { tag: n.value.tag, text_start: n.value.text_start, text_end: n.value.text_end, reserved: 0,
                    now: n.value.now, min: n.value.min, max: n.value.max, step: n.value.step },
                rect: [n.rect.x, n.rect.y, n.rect.w, n.rect.h],
                states: n.states,
                actions: n.actions,
                selection: n.selection,
                child_count: n.child_count,
                first_child: n.first_child,
                next_sibling: n.next_sibling,
                last_child: n.last_child,
            }));
            // A refused tree is retried next frame only if its revision moves; the client keeps the last good one.
            let _ = self.bridge.sync(&self.nodes, strings, first_top);
            self.revision = Some(revision);
        }
        let mut queued = [Action::default(); ax::QUEUE];
        let count = self.bridge.take(&mut queued);
        if count > 0 {
            let mut actions = [pxui::A11yAction::default(); ax::QUEUE];
            for (to, from) in actions.iter_mut().zip(&queued[..count]) {
                *to = pxui::A11yAction { id: from.id, action_code: from.action, payload: from.payload, surface, ..Default::default() };
            }
            ui.inject_a11y(&actions[..count]);
        }
    }

    /// NSAccessibility notifications posted so far.
    pub fn notifications(&self) -> u64 {
        self.bridge.notifications()
    }

    #[cfg(test)]
    pub(crate) fn bridge(&self) -> &Bridge {
        &self.bridge
    }
    #[cfg(test)]
    pub(crate) fn bridge_mut(&mut self) -> &mut Bridge {
        &mut self.bridge
    }
}
