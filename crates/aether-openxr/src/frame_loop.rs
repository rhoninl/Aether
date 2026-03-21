//! OpenXR frame loop helpers.
//!
//! Provides the wait/begin/end frame abstractions for the render loop.

use crate::session::XrSession;
use crate::OpenXrError;

/// Result of waiting for the next frame.
#[derive(Debug, Clone)]
pub struct FrameState {
    pub predicted_display_time_ns: u64,
    pub should_render: bool,
}

/// Wait for the next frame from the runtime.
///
/// Placeholder: returns frame state based on session state.
pub fn wait_frame(session: &XrSession) -> Result<FrameState, OpenXrError> {
    Ok(FrameState {
        predicted_display_time_ns: 0, // Will be filled by real xrWaitFrame
        should_render: session.should_render(),
    })
}

/// Begin the frame — must be called after wait_frame.
pub fn begin_frame(_session: &XrSession) -> Result<(), OpenXrError> {
    Ok(())
}

/// End the frame — submits composition layers to the runtime.
pub fn end_frame(_session: &XrSession, _predicted_display_time_ns: u64) -> Result<(), OpenXrError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instance::{InstanceConfig, XrInstance};
    use crate::session::XrSessionState;

    fn test_session() -> XrSession {
        let instance = XrInstance::new(InstanceConfig::default()).unwrap();
        XrSession::new(&instance).unwrap()
    }

    #[test]
    fn wait_frame_idle_no_render() {
        let session = test_session();
        let state = wait_frame(&session).unwrap();
        assert!(!state.should_render);
    }

    #[test]
    fn wait_frame_focused_should_render() {
        let mut session = test_session();
        session.transition_to(XrSessionState::Focused);
        let state = wait_frame(&session).unwrap();
        assert!(state.should_render);
    }

    #[test]
    fn begin_frame_ok() {
        let session = test_session();
        assert!(begin_frame(&session).is_ok());
    }

    #[test]
    fn end_frame_ok() {
        let session = test_session();
        assert!(end_frame(&session, 0).is_ok());
    }

    #[test]
    fn frame_state_debug() {
        let state = FrameState {
            predicted_display_time_ns: 12345,
            should_render: true,
        };
        let debug = format!("{:?}", state);
        assert!(debug.contains("12345"));
    }
}
