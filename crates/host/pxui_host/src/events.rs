/// Retained ABI events, UTF-8 bytes and numeric touch payloads, owned independently of the input batch.
pub struct Events {
    wire: Vec<pxui::Event>,
    bytes: Vec<u8>,
    scratch: String,
    touches: Vec<pxui::Touch>,
}
impl Default for Events {
    fn default() -> Self {
        Self {
            wire: Vec::with_capacity(input::EVENT_RESERVE),
            bytes: Vec::new(),
            scratch: String::new(),
            touches: Vec::with_capacity(input::EVENT_RESERVE),
        }
    }
}
impl Events {
    /// The event span and its payloads remain valid until the next encode or this encoder's destruction.
    pub fn encode(&mut self, events: &[input::Event]) -> &[pxui::Event] {
        self.wire.clear();
        self.bytes.clear();
        self.touches.clear();
        for event in events {
            if let input::Event::Touch(t) = event {
                self.touches.push(pxui::Touch {
                    device: t.device,
                    finger: t.finger,
                    x: t.point.0,
                    y: t.point.1,
                    time: t.time,
                    pressure: t.pressure,
                    phase: t.phase.code(),
                });
                self.wire.push(pxui::Event {
                    kind: pxui::Event::TOUCH,
                    text: pxui::Str {
                        ptr: std::ptr::null(),
                        len: core::mem::size_of::<pxui::Touch>(),
                    },
                    ..Default::default()
                });
                continue;
            }
            self.scratch.clear();
            let raw = event.raw(&mut self.scratch);
            let text = raw.text.unwrap_or("");
            self.bytes.extend_from_slice(text.as_bytes());
            self.wire.push(pxui::Event {
                kind: raw.kind,
                code: raw.code,
                x: raw.x,
                y: raw.y,
                value: raw.value,
                text: pxui::Str {
                    ptr: std::ptr::null(),
                    len: text.len(),
                },
                ..Default::default()
            });
        }
        let mut offset = 0;
        let mut touch = 0;
        for event in &mut self.wire {
            if event.kind == pxui::Event::TOUCH {
                event.text.ptr = self.touches.as_ptr().wrapping_add(touch).cast();
                touch += 1;
                continue;
            }
            event.text.ptr = self.bytes.as_ptr().wrapping_add(offset);
            offset += event.text.len;
        }
        &self.wire
    }
    pub fn as_slice(&self) -> &[pxui::Event] {
        &self.wire
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::Events;
    use input::{Event, Key, MouseButton};

    pub(crate) fn encoded_input_owns_text_and_reuses_dialog_and_numeric_storage() {
        let source = vec![
            Event::KeyPressed(Key::Meta),
            Event::PointerPressed(MouseButton::LeftButton, (7.5, 12.0)),
            Event::TextInput("héllo".into()),
            Event::TextEditing {
                text: "compose".into(),
                start: 1,
                length: 2,
            },
            Event::FileDialog {
                id: 5,
                result: Ok(vec!["/music/one".into(), "/music/two".into()]),
            },
            Event::FileDialog {
                id: 6,
                result: Ok(Vec::new()),
            },
            Event::FileDialog {
                id: 7,
                result: Err("failed".into()),
            },
        ];
        let expected = source.clone();
        let mut encoded = Events::default();
        encoded.encode(&source);
        drop(source);
        let mut offset = 0;
        for (event, expected) in encoded.as_slice().iter().zip(&expected) {
            assert_eq!(event.text.ptr, encoded.bytes.as_ptr().wrapping_add(offset));
            let text =
                std::str::from_utf8(&encoded.bytes[offset..offset + event.text.len]).unwrap();
            let raw = input::wire::Raw {
                kind: event.kind,
                code: event.code,
                x: event.x,
                y: event.y,
                value: event.value,
                text: Some(text),
                touch: None,
            };
            assert_eq!(Event::from_raw(&raw).as_ref(), Some(expected));
            offset += event.text.len;
        }
        for round in 0..100 {
            mem_sys::local_reset();
            encoded.encode(&expected);
            if round > 1 {
                assert_eq!(
                    mem_sys::local_alloc_count(),
                    0,
                    "warm input encoding allocated"
                );
            }
        }
    }
    pub(crate) fn first_resume_batch_reuses_the_cold_event_reserve() {
        let source = [
            Event::VisibilityChanged(false),
            Event::FocusLost,
            Event::FocusGained,
            Event::VisibilityChanged(true),
            Event::EnvironmentChanged,
        ];
        let mut encoded = Events::default();
        encoded.encode(&source[..1]);
        for _ in 0..82 {
            encoded.encode(&[]);
        }
        mem_sys::local_reset();
        assert_eq!(encoded.encode(&source).len(), 5);
        assert_eq!(
            mem_sys::local_alloc_count(),
            0,
            "first native resume batch must not grow the encoder"
        );
    }
    pub(crate) fn touch_payload_growth_owns_full_identities_and_reuses_warm_storage() {
        let mut source = Vec::new();
        for finger in 0..32 {
            source.push(Event::Touch(input::Touch {
                device: u64::MAX,
                finger: u64::MAX - finger,
                point: (-12.5, 144.25),
                time: 10.0,
                pressure: 0.375,
                phase: input::TouchPhase::Moved,
            }));
        }
        let mut encoded = Events::default();
        encoded.encode(&source);
        for (index, event) in encoded.as_slice().iter().enumerate() {
            assert_eq!(event.kind, pxui::Event::TOUCH);
            assert_eq!(event.text.len, core::mem::size_of::<pxui::Touch>());
            assert_eq!(
                event.text.ptr,
                encoded.touches.as_ptr().wrapping_add(index).cast()
            );
            assert_eq!(encoded.touches[index].finger, u64::MAX - index as u64);
            assert_eq!(pxui::event_text(event), "");
        }
        for count in [1, 32, 0, 3, 32] {
            mem_sys::local_reset();
            encoded.encode(&source[..count]);
            assert_eq!(
                mem_sys::local_alloc_count(),
                0,
                "warm numeric touch encoding allocated"
            );
        }
        drop(source);
        assert_eq!(encoded.touches[31].device, u64::MAX);
        assert_eq!(encoded.touches[31].finger, u64::MAX - 31);
    }
}
