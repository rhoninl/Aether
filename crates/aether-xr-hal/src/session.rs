//! Session lifecycle value types (design doc §5.3, P1-B).
//!
//! Models the OpenXR session state graph (`xrCreateSession` →
//! `xrBeginSession` → … → `xrEndSession`) and reference-space configuration.
//! The `XrSession` *trait* lives in P2-B and ties these value types to a
//! backend; here we keep only the data the trait operates on.

use crate::tracking::Pose3;

/// Default prediction offset in nanoseconds (~11ms for 90Hz displays).
pub const DEFAULT_PREDICTION_OFFSET_NS: u64 = 11_111_111;

/// OpenXR session states per the specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionState {
    Idle,
    Ready,
    Synchronized,
    Visible,
    Focused,
    Stopping,
    LossPending,
    Exiting,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionTransitionError {
    InvalidTransition {
        from: SessionState,
        to: SessionState,
    },
    SessionTerminated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReferenceSpaceType {
    /// Origin at the initial head position; recenters with runtime reset.
    Local,
    /// Origin at the centre of the play-area floor; provides room-scale boundaries.
    Stage,
    /// Origin locked to the head-mounted display; moves with the user's head.
    View,
}

#[derive(Debug, Clone)]
pub struct ReferenceSpace {
    pub space_type: ReferenceSpaceType,
    pub offset: Pose3,
}

impl ReferenceSpace {
    pub fn new(space_type: ReferenceSpaceType) -> Self {
        Self {
            space_type,
            offset: Pose3::default(),
        }
    }

    pub fn with_offset(space_type: ReferenceSpaceType, offset: Pose3) -> Self {
        Self { space_type, offset }
    }
}

#[derive(Debug, Clone)]
pub struct SessionConfig {
    pub application_name: String,
    pub reference_space: ReferenceSpaceType,
    pub prediction_offset_ns: u64,
    pub enable_hand_tracking: bool,
    pub enable_haptics: bool,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            application_name: "Aether".to_string(),
            reference_space: ReferenceSpaceType::Local,
            prediction_offset_ns: DEFAULT_PREDICTION_OFFSET_NS,
            enable_hand_tracking: false,
            enable_haptics: true,
        }
    }
}

/// Active XR session (design doc §5.3, P2-B).
///
/// Owns the lifecycle (`xrBeginSession` / `xrEndSession`), reference-space
/// creation, swapchain creation, action-set attachment, and frame
/// acquisition. The state itself is observed through `state()`; transitions
/// are driven by events from `XrInstance::poll_events()`.
pub trait XrSession {
    type Frame: crate::frame::XrFrame;
    type Swapchain: crate::swapchain::XrSwapchain;
    type ActionSet;
    type Error: std::error::Error + Send + Sync + 'static;

    fn state(&self) -> SessionState;

    /// `xrBeginSession`.
    fn begin(
        &mut self,
        view_config: crate::instance::ViewConfigType,
    ) -> Result<(), Self::Error>;

    /// `xrEndSession`.
    fn end(&mut self) -> Result<(), Self::Error>;

    /// `xrRequestExitSession`.
    fn request_exit(&mut self) -> Result<(), Self::Error>;

    /// `xrCreateReferenceSpace`.
    fn create_reference_space(
        &self,
        kind: ReferenceSpaceType,
        offset: crate::tracking::Pose3,
    ) -> Result<ReferenceSpace, Self::Error>;

    fn create_swapchain(
        &self,
        config: crate::swapchain::SwapchainConfig,
    ) -> Result<Self::Swapchain, Self::Error>;

    /// `xrAttachSessionActionSets`.
    fn attach_action_sets(&mut self, sets: &[Self::ActionSet]) -> Result<(), Self::Error>;

    /// `xrWaitFrame` — produces the per-frame handle.
    fn wait_frame(&mut self) -> Result<Self::Frame, Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_space_default_offset_is_identity_pose() {
        let r = ReferenceSpace::new(ReferenceSpaceType::Local);
        assert_eq!(r.space_type, ReferenceSpaceType::Local);
        assert_eq!(r.offset.rotation, [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn session_config_default_uses_local_space() {
        let c = SessionConfig::default();
        assert_eq!(c.reference_space, ReferenceSpaceType::Local);
        assert_eq!(c.prediction_offset_ns, DEFAULT_PREDICTION_OFFSET_NS);
        assert!(c.enable_haptics);
        assert!(!c.enable_hand_tracking);
    }
}
