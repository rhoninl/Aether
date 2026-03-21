//! OpenXR session management.
//!
//! Wraps the OpenXR session lifecycle: creation, event polling, state transitions.
//! Drives the `SessionManager` from `aether-input`.

use crate::instance::XrInstance;
use crate::OpenXrError;

/// Session states matching the OpenXR specification.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum XrSessionState {
    #[default]
    Idle,
    Ready,
    Synchronized,
    Visible,
    Focused,
    Stopping,
    LossPending,
    Exiting,
}

impl std::fmt::Display for XrSessionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            XrSessionState::Idle => write!(f, "Idle"),
            XrSessionState::Ready => write!(f, "Ready"),
            XrSessionState::Synchronized => write!(f, "Synchronized"),
            XrSessionState::Visible => write!(f, "Visible"),
            XrSessionState::Focused => write!(f, "Focused"),
            XrSessionState::Stopping => write!(f, "Stopping"),
            XrSessionState::LossPending => write!(f, "LossPending"),
            XrSessionState::Exiting => write!(f, "Exiting"),
        }
    }
}

/// Represents an OpenXR session.
///
/// Currently a placeholder that tracks session state.
/// Will wrap `openxr::Session<OpenGL>` when the real crate is linked.
pub struct XrSession {
    state: XrSessionState,
    app_name: String,
}

impl XrSession {
    /// Create a new session from an XR instance.
    pub fn new(instance: &XrInstance) -> Result<Self, OpenXrError> {
        if !instance.is_initialized() {
            return Err(OpenXrError::SessionCreation(
                "instance not initialized".to_string(),
            ));
        }

        log::info!("Creating OpenXR session for '{}'", instance.app_name());

        Ok(Self {
            state: XrSessionState::Idle,
            app_name: instance.app_name().to_string(),
        })
    }

    pub fn state(&self) -> XrSessionState {
        self.state
    }

    /// Simulate transitioning to a new state (placeholder for event polling).
    pub fn transition_to(&mut self, new_state: XrSessionState) {
        log::info!("Session state: {} -> {}", self.state, new_state);
        self.state = new_state;
    }

    /// Whether the session is in a renderable state.
    pub fn should_render(&self) -> bool {
        matches!(
            self.state,
            XrSessionState::Visible | XrSessionState::Focused
        )
    }

    /// Whether the session is still active (not exiting/stopped).
    pub fn is_active(&self) -> bool {
        !matches!(
            self.state,
            XrSessionState::Exiting | XrSessionState::LossPending
        )
    }

    pub fn app_name(&self) -> &str {
        &self.app_name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instance::InstanceConfig;

    fn test_instance() -> XrInstance {
        XrInstance::new(InstanceConfig::default()).unwrap()
    }

    #[test]
    fn create_session() {
        let instance = test_instance();
        let session = XrSession::new(&instance).unwrap();
        assert_eq!(session.state(), XrSessionState::Idle);
    }

    #[test]
    fn session_state_transitions() {
        let instance = test_instance();
        let mut session = XrSession::new(&instance).unwrap();
        assert_eq!(session.state(), XrSessionState::Idle);

        session.transition_to(XrSessionState::Ready);
        assert_eq!(session.state(), XrSessionState::Ready);

        session.transition_to(XrSessionState::Focused);
        assert_eq!(session.state(), XrSessionState::Focused);
    }

    #[test]
    fn should_render_when_visible() {
        let instance = test_instance();
        let mut session = XrSession::new(&instance).unwrap();
        assert!(!session.should_render());

        session.transition_to(XrSessionState::Visible);
        assert!(session.should_render());

        session.transition_to(XrSessionState::Focused);
        assert!(session.should_render());

        session.transition_to(XrSessionState::Synchronized);
        assert!(!session.should_render());
    }

    #[test]
    fn is_active() {
        let instance = test_instance();
        let mut session = XrSession::new(&instance).unwrap();
        assert!(session.is_active());

        session.transition_to(XrSessionState::Exiting);
        assert!(!session.is_active());
    }

    #[test]
    fn session_state_display() {
        assert_eq!(XrSessionState::Idle.to_string(), "Idle");
        assert_eq!(XrSessionState::Focused.to_string(), "Focused");
        assert_eq!(XrSessionState::Exiting.to_string(), "Exiting");
    }

    #[test]
    fn session_default_state() {
        assert_eq!(XrSessionState::default(), XrSessionState::Idle);
    }

    #[test]
    fn session_preserves_app_name() {
        let instance = test_instance();
        let session = XrSession::new(&instance).unwrap();
        assert_eq!(session.app_name(), "Aether VR");
    }
}
