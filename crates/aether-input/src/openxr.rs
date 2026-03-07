//! OpenXR facade types for adapter integration.

use crate::adapter::{InputFrame, InputFrameError, RuntimeAdapter};
use crate::capabilities::InputBackend;
use crate::capabilities::InputFrameHint;
use crate::locomotion::LocomotionProfile;

#[derive(Debug)]
pub struct OpenXrAdapter {
    backend: String,
    profile: InputFrameHint,
    locomotion_profile: Option<LocomotionProfile>,
    frame_counter: u64,
}

impl OpenXrAdapter {
    pub fn new(profile: InputFrameHint) -> Self {
        Self {
            backend: "openxr".to_string(),
            profile,
            locomotion_profile: None,
            frame_counter: 0,
        }
    }
}

impl RuntimeAdapter for OpenXrAdapter {
    fn backend(&self) -> InputBackend {
        InputBackend::OpenXr
    }

    fn advertised_capabilities(&self) -> InputFrameHint {
        self.profile.clone()
    }

    fn poll_frame(&mut self) -> Result<InputFrame, InputFrameError> {
        self.frame_counter = self.frame_counter.saturating_add(1);
        if self.backend.is_empty() {
            return Err(InputFrameError::MissingBackend);
        }
        Ok(InputFrame {
            backend: InputBackend::OpenXr,
            player_id: 0,
            timestamp_ms: self.frame_counter,
            events: Vec::new(),
        })
    }

    fn apply_locomotion_profile(&mut self, profile: &LocomotionProfile) {
        self.locomotion_profile = Some(profile.clone());
    }
}
