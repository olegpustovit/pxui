/// Retained ABI events and UTF-8 bytes. Published text belongs to this encoder, not the input batch.
#[derive(Default)]
pub struct Events {
    wire: Vec<pxui::Event>,
    bytes: Vec<u8>,
    scratch: String,
}
impl Events {
    /// The event span and its text remain valid until the next encode or this encoder's destruction.
    pub fn encode(&mut self, events: &[input::Event]) -> &[pxui::Event] {
        self.wire.clear();
        self.bytes.clear();
        for event in events {
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
            });
        }
        let mut offset = 0;
        for event in &mut self.wire {
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
mod tests {
    use super::Events;
    use input::{Event, Key, MouseButton};

    #[test]
    fn encoded_input_owns_text_and_reuses_dialog_and_numeric_storage() {
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
}
