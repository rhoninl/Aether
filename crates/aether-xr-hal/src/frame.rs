//! Per-frame handle (design doc §5.4, §6, P2-B).

use crate::action::ActionSetHandle;
use crate::layer::{LayerBuilder, LayerSubmission};
use crate::session::ReferenceSpace;
use crate::tracking::Pose3;
use crate::view::View;

/// OpenXR's `XrTime` — nanoseconds since an unspecified epoch. Newtyped so it
/// can never be mistaken for a `u64` count or a frame index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct XrTime(pub i64);

impl XrTime {
    pub const ZERO: Self = Self(0);

    pub fn as_nanos(self) -> i64 {
        self.0
    }
}

/// RAII per-frame handle. Drop without calling `end()` is a programmer error
/// (the OpenXR runtime requires every `xrBeginFrame` to be paired with
/// `xrEndFrame`); backends should panic in their `Drop` impl if this happens.
pub trait XrFrame {
    type Error: std::error::Error + Send + Sync + 'static;

    fn predicted_display_time(&self) -> XrTime;
    fn should_render(&self) -> bool;

    /// `xrLocateViews` — returns one [`View`] per eye in the active view config.
    fn locate_views(&self, space: &ReferenceSpace) -> Result<Vec<View>, Self::Error>;

    /// `xrSyncActions` — refresh action state for all attached sets.
    fn sync_actions(&mut self, sets: &[ActionSetHandle]) -> Result<(), Self::Error>;

    /// `xrBeginFrame` — open the frame for layer recording. Returned builder
    /// borrows from `self` so layer references can't outlive the frame.
    fn begin(&mut self) -> Result<LayerBuilder<'_>, Self::Error>;

    /// `xrEndFrame` — submit recorded layers and consume the frame handle.
    fn end(self, layers: LayerSubmission) -> Result<(), Self::Error>;
}

/// Minimal pose-locating API broken out so consumers that only need a head
/// pose don't have to drag in the full `XrFrame` trait. Backends typically
/// implement both via the same `xrLocateSpace` / `xrLocateViews` calls.
pub fn identity_pose() -> Pose3 {
    Pose3::default()
}
