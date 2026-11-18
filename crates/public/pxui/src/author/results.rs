use crate::{Handle, Response};

/// The current selection and interactions of a pick-one widget for this frame.
#[derive(Clone, Copy, Debug)]
pub struct PickResponse {
    /// Interaction state and the control's handle.
    pub response: Response,
    /// The selected item index after the frame's input, or none for no selection.
    pub chosen: Option<usize>,
    /// The current index of the item where a secondary press (right button or touch long-press) began. It follows the item's id
    /// through a reorder and is never a primary choice.
    pub secondary: Option<usize>,
}

/// The value and interaction state of a slider or caller-painted scrubber.
///
/// Frames: while the pointer holds the track, `preview` is the value under it and `commit` is none. On the frame of a release, a
/// keyboard step or an accessibility SetValue, `commit` is the final value and `preview` equals it, so a caller that paints from
/// `preview` shows the committed value before its own (possibly asynchronous) state catches up. On other frames both are none.
#[derive(Clone, Copy, Debug)]
pub struct SliderResponse {
    /// Interaction state and the node's handle. Size or paint a scrubber using this handle.
    pub response: Response,
    /// The value to display, including a held preview. The caller owns the committed value.
    pub value: f32,
    /// The value to paint: the held value during a drag (even when unchanged), and the committed value on a commit frame.
    pub preview: Option<f32>,
    /// Final value on a drag release, a keyboard step or an accessibility adjustment; none on every other frame.
    pub commit: Option<f32>,
}

/// Whether this overlay continues to be built, and the handle of its root.
#[derive(Clone, Copy, Debug)]
pub struct OverlayResponse {
    /// The root built by this call. The empty handle accompanies a dismissed overlay.
    pub handle: Handle,
    /// Whether the caller should keep the overlay open. Panels and anchored overlays remain open.
    pub open: bool,
}

/// The divider interaction and caller-owned ratio of a split pane.
#[derive(Clone, Copy, Debug)]
pub struct SplitResponse {
    /// Interaction state and the divider handle.
    pub response: Response,
    /// Ratio proposed by the divider whenever it is held and on the frame it is released (the release value is final); none on
    /// other frames, and always none when a custom split body builds no grip.
    pub ratio: Option<f32>,
}
