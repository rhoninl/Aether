//! OpenXR session lifecycle state machine and reference space management.
//!
//! Models the OpenXR session state graph per the specification, including
//! valid state transitions, loss pending handling, and reference spaces.

use crate::actions::Pose3;

/// Default prediction offset in nanoseconds (~11ms for 90Hz displays).
pub const DEFAULT_PREDICTION_OFFSET_NS: u64 = 11_111_111;

/// OpenXR session states per the specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionState {
    /// Session has been created but not yet started.
    Idle,
    /// Session is ready to begin rendering.
    Ready,
    /// Session is running but not yet visible to the user.
    Synchronized,
    /// Session is visible to the user but does not have input focus.
    Visible,
    /// Session is visible and has input focus.
    Focused,
    /// Session is being stopped.
    Stopping,
    /// Session has lost its connection to the runtime (e.g., device unplugged).
    LossPending,
    /// Session is exiting and should be destroyed.
    Exiting,
}

/// Errors that can occur during session state transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionTransitionError {
    /// The requested transition is not valid from the current state.
    InvalidTransition {
        from: SessionState,
        to: SessionState,
    },
    /// The session is in a terminal state and cannot transition further.
    SessionTerminated,
}

/// OpenXR reference space types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReferenceSpaceType {
    /// Origin at the initial head position; recenters with runtime reset.
    Local,
    /// Origin at the center of the play area floor; provides room-scale boundaries.
    Stage,
    /// Origin locked to the head-mounted display; moves with the user's head.
    View,
}

/// A reference space with a type and optional offset pose.
#[derive(Debug, Clone)]
pub struct ReferenceSpace {
    /// The type of reference space.
    pub space_type: ReferenceSpaceType,
    /// An offset pose applied to the reference space origin.
    pub offset: Pose3,
}

impl ReferenceSpace {
    /// Create a new reference space with no offset.
    pub fn new(space_type: ReferenceSpaceType) -> Self {
        Self {
            space_type,
            offset: Pose3 {
                position: [0.0, 0.0, 0.0],
                rotation: [0.0, 0.0, 0.0, 1.0], // identity quaternion
                linear_velocity: [0.0, 0.0, 0.0],
                angular_velocity: [0.0, 0.0, 0.0],
            },
        }
    }

    /// Create a new reference space with a custom offset pose.
    pub fn with_offset(space_type: ReferenceSpaceType, offset: Pose3) -> Self {
        Self { space_type, offset }
    }
}

/// Configuration for creating an OpenXR session.
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Application name reported to the OpenXR runtime.
    pub application_name: String,
    /// Requested reference space type.
    pub reference_space: ReferenceSpaceType,
    /// Prediction offset in nanoseconds for frame timing.
    pub prediction_offset_ns: u64,
    /// Whether to request hand tracking extension.
    pub enable_hand_tracking: bool,
    /// Whether to enable haptic feedback.
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

/// Manages the OpenXR session lifecycle state machine.
///
/// Enforces valid state transitions according to the OpenXR specification.
/// Does not directly interact with the OpenXR runtime; instead it models
/// the state machine for use with mock or real backends.
#[derive(Debug)]
pub struct SessionManager {
    state: SessionState,
    config: SessionConfig,
    reference_space: ReferenceSpace,
    frame_count: u64,
    last_predicted_display_time_ns: u64,
}

impl SessionManager {
    /// Create a new session manager with the given configuration.
    pub fn new(config: SessionConfig) -> Self {
        let reference_space = ReferenceSpace::new(config.reference_space);
        Self {
            state: SessionState::Idle,
            config,
            reference_space,
            frame_count: 0,
            last_predicted_display_time_ns: 0,
        }
    }

    /// Get the current session state.
    pub fn state(&self) -> SessionState {
        self.state
    }

    /// Get the session configuration.
    pub fn config(&self) -> &SessionConfig {
        &self.config
    }

    /// Get the current reference space.
    pub fn reference_space(&self) -> &ReferenceSpace {
        &self.reference_space
    }

    /// Get the number of frames processed.
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Get the last predicted display time in nanoseconds.
    pub fn last_predicted_display_time_ns(&self) -> u64 {
        self.last_predicted_display_time_ns
    }

    /// Check if the session is in a state where rendering should occur.
    pub fn should_render(&self) -> bool {
        matches!(
            self.state,
            SessionState::Visible | SessionState::Focused
        )
    }

    /// Check if the session has input focus.
    pub fn has_input_focus(&self) -> bool {
        self.state == SessionState::Focused
    }

    /// Check if the session is in a terminal state.
    pub fn is_terminated(&self) -> bool {
        self.state == SessionState::Exiting
    }

    /// Check if the session is running (not idle or terminal).
    pub fn is_running(&self) -> bool {
        !matches!(
            self.state,
            SessionState::Idle | SessionState::Exiting
        )
    }

    /// Request to begin the session (Idle -> Ready).
    pub fn request_begin(&mut self) -> Result<SessionState, SessionTransitionError> {
        self.transition(SessionState::Ready)
    }

    /// Handle the runtime reporting synchronized state (Ready -> Synchronized).
    pub fn on_synchronized(&mut self) -> Result<SessionState, SessionTransitionError> {
        self.transition(SessionState::Synchronized)
    }

    /// Handle the runtime reporting visible state (Synchronized -> Visible).
    pub fn on_visible(&mut self) -> Result<SessionState, SessionTransitionError> {
        self.transition(SessionState::Visible)
    }

    /// Handle the runtime reporting focused state (Visible -> Focused).
    pub fn on_focused(&mut self) -> Result<SessionState, SessionTransitionError> {
        self.transition(SessionState::Focused)
    }

    /// Handle focus lost (Focused -> Visible).
    pub fn on_focus_lost(&mut self) -> Result<SessionState, SessionTransitionError> {
        if self.state != SessionState::Focused {
            return Err(SessionTransitionError::InvalidTransition {
                from: self.state,
                to: SessionState::Visible,
            });
        }
        self.state = SessionState::Visible;
        Ok(self.state)
    }

    /// Handle becoming invisible (Visible -> Synchronized).
    pub fn on_invisible(&mut self) -> Result<SessionState, SessionTransitionError> {
        if self.state != SessionState::Visible {
            return Err(SessionTransitionError::InvalidTransition {
                from: self.state,
                to: SessionState::Synchronized,
            });
        }
        self.state = SessionState::Synchronized;
        Ok(self.state)
    }

    /// Request to end the session (Synchronized|Visible|Focused -> Stopping).
    pub fn request_end(&mut self) -> Result<SessionState, SessionTransitionError> {
        match self.state {
            SessionState::Synchronized
            | SessionState::Visible
            | SessionState::Focused => {
                self.state = SessionState::Stopping;
                Ok(self.state)
            }
            SessionState::Exiting => Err(SessionTransitionError::SessionTerminated),
            other => Err(SessionTransitionError::InvalidTransition {
                from: other,
                to: SessionState::Stopping,
            }),
        }
    }

    /// Handle the runtime reporting session stopped (Stopping -> Idle).
    pub fn on_stopped(&mut self) -> Result<SessionState, SessionTransitionError> {
        if self.state != SessionState::Stopping {
            return Err(SessionTransitionError::InvalidTransition {
                from: self.state,
                to: SessionState::Idle,
            });
        }
        self.state = SessionState::Idle;
        Ok(self.state)
    }

    /// Handle runtime loss pending (any running state -> LossPending).
    pub fn on_loss_pending(&mut self) -> Result<SessionState, SessionTransitionError> {
        match self.state {
            SessionState::Ready
            | SessionState::Synchronized
            | SessionState::Visible
            | SessionState::Focused => {
                self.state = SessionState::LossPending;
                Ok(self.state)
            }
            SessionState::Exiting => Err(SessionTransitionError::SessionTerminated),
            other => Err(SessionTransitionError::InvalidTransition {
                from: other,
                to: SessionState::LossPending,
            }),
        }
    }

    /// Handle session exit (Idle|LossPending -> Exiting).
    pub fn on_exit(&mut self) -> Result<SessionState, SessionTransitionError> {
        match self.state {
            SessionState::Idle | SessionState::LossPending => {
                self.state = SessionState::Exiting;
                Ok(self.state)
            }
            SessionState::Exiting => Err(SessionTransitionError::SessionTerminated),
            other => Err(SessionTransitionError::InvalidTransition {
                from: other,
                to: SessionState::Exiting,
            }),
        }
    }

    /// Update the reference space type. Only valid when the session is not running.
    pub fn set_reference_space(
        &mut self,
        space_type: ReferenceSpaceType,
    ) -> Result<(), SessionTransitionError> {
        if self.is_terminated() {
            return Err(SessionTransitionError::SessionTerminated);
        }
        self.reference_space = ReferenceSpace::new(space_type);
        Ok(())
    }

    /// Record a frame begin with a predicted display time.
    pub fn begin_frame(&mut self, predicted_display_time_ns: u64) {
        self.frame_count = self.frame_count.saturating_add(1);
        self.last_predicted_display_time_ns = predicted_display_time_ns;
    }

    /// Validate and execute a state transition.
    fn transition(
        &mut self,
        target: SessionState,
    ) -> Result<SessionState, SessionTransitionError> {
        if self.state == SessionState::Exiting {
            return Err(SessionTransitionError::SessionTerminated);
        }

        if !Self::is_valid_transition(self.state, target) {
            return Err(SessionTransitionError::InvalidTransition {
                from: self.state,
                to: target,
            });
        }

        self.state = target;
        Ok(self.state)
    }

    /// Check if a transition from `from` to `to` is valid.
    fn is_valid_transition(from: SessionState, to: SessionState) -> bool {
        matches!(
            (from, to),
            (SessionState::Idle, SessionState::Ready)
                | (SessionState::Ready, SessionState::Synchronized)
                | (SessionState::Synchronized, SessionState::Visible)
                | (SessionState::Visible, SessionState::Focused)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_manager() -> SessionManager {
        SessionManager::new(SessionConfig::default())
    }

    /// Walk through the full happy-path lifecycle: Idle -> Ready -> Synchronized -> Visible -> Focused.
    fn advance_to_focused(mgr: &mut SessionManager) {
        mgr.request_begin().unwrap();
        mgr.on_synchronized().unwrap();
        mgr.on_visible().unwrap();
        mgr.on_focused().unwrap();
    }

    // ---- Initial state tests ----

    #[test]
    fn initial_state_is_idle() {
        let mgr = make_manager();
        assert_eq!(mgr.state(), SessionState::Idle);
    }

    #[test]
    fn initial_frame_count_is_zero() {
        let mgr = make_manager();
        assert_eq!(mgr.frame_count(), 0);
    }

    #[test]
    fn initial_should_render_is_false() {
        let mgr = make_manager();
        assert!(!mgr.should_render());
    }

    #[test]
    fn initial_has_input_focus_is_false() {
        let mgr = make_manager();
        assert!(!mgr.has_input_focus());
    }

    #[test]
    fn initial_is_not_terminated() {
        let mgr = make_manager();
        assert!(!mgr.is_terminated());
    }

    #[test]
    fn initial_is_not_running() {
        let mgr = make_manager();
        assert!(!mgr.is_running());
    }

    // ---- Happy path lifecycle ----

    #[test]
    fn idle_to_ready() {
        let mut mgr = make_manager();
        let state = mgr.request_begin().unwrap();
        assert_eq!(state, SessionState::Ready);
        assert!(mgr.is_running());
    }

    #[test]
    fn ready_to_synchronized() {
        let mut mgr = make_manager();
        mgr.request_begin().unwrap();
        let state = mgr.on_synchronized().unwrap();
        assert_eq!(state, SessionState::Synchronized);
    }

    #[test]
    fn synchronized_to_visible() {
        let mut mgr = make_manager();
        mgr.request_begin().unwrap();
        mgr.on_synchronized().unwrap();
        let state = mgr.on_visible().unwrap();
        assert_eq!(state, SessionState::Visible);
        assert!(mgr.should_render());
        assert!(!mgr.has_input_focus());
    }

    #[test]
    fn visible_to_focused() {
        let mut mgr = make_manager();
        mgr.request_begin().unwrap();
        mgr.on_synchronized().unwrap();
        mgr.on_visible().unwrap();
        let state = mgr.on_focused().unwrap();
        assert_eq!(state, SessionState::Focused);
        assert!(mgr.should_render());
        assert!(mgr.has_input_focus());
    }

    #[test]
    fn full_lifecycle_to_focused() {
        let mut mgr = make_manager();
        advance_to_focused(&mut mgr);
        assert_eq!(mgr.state(), SessionState::Focused);
    }

    // ---- Focus loss and regain ----

    #[test]
    fn focused_to_visible_on_focus_lost() {
        let mut mgr = make_manager();
        advance_to_focused(&mut mgr);
        let state = mgr.on_focus_lost().unwrap();
        assert_eq!(state, SessionState::Visible);
        assert!(mgr.should_render());
        assert!(!mgr.has_input_focus());
    }

    #[test]
    fn visible_to_focused_after_focus_regain() {
        let mut mgr = make_manager();
        advance_to_focused(&mut mgr);
        mgr.on_focus_lost().unwrap();
        let state = mgr.on_focused().unwrap();
        assert_eq!(state, SessionState::Focused);
    }

    #[test]
    fn visible_to_synchronized_on_invisible() {
        let mut mgr = make_manager();
        advance_to_focused(&mut mgr);
        mgr.on_focus_lost().unwrap();
        let state = mgr.on_invisible().unwrap();
        assert_eq!(state, SessionState::Synchronized);
        assert!(!mgr.should_render());
    }

    // ---- Session end lifecycle ----

    #[test]
    fn focused_to_stopping_on_request_end() {
        let mut mgr = make_manager();
        advance_to_focused(&mut mgr);
        let state = mgr.request_end().unwrap();
        assert_eq!(state, SessionState::Stopping);
    }

    #[test]
    fn visible_to_stopping_on_request_end() {
        let mut mgr = make_manager();
        advance_to_focused(&mut mgr);
        mgr.on_focus_lost().unwrap();
        let state = mgr.request_end().unwrap();
        assert_eq!(state, SessionState::Stopping);
    }

    #[test]
    fn synchronized_to_stopping_on_request_end() {
        let mut mgr = make_manager();
        mgr.request_begin().unwrap();
        mgr.on_synchronized().unwrap();
        let state = mgr.request_end().unwrap();
        assert_eq!(state, SessionState::Stopping);
    }

    #[test]
    fn stopping_to_idle_on_stopped() {
        let mut mgr = make_manager();
        advance_to_focused(&mut mgr);
        mgr.request_end().unwrap();
        let state = mgr.on_stopped().unwrap();
        assert_eq!(state, SessionState::Idle);
        assert!(!mgr.is_running());
    }

    #[test]
    fn idle_to_exiting_on_exit() {
        let mut mgr = make_manager();
        let state = mgr.on_exit().unwrap();
        assert_eq!(state, SessionState::Exiting);
        assert!(mgr.is_terminated());
    }

    #[test]
    fn full_lifecycle_to_exit() {
        let mut mgr = make_manager();
        advance_to_focused(&mut mgr);
        mgr.request_end().unwrap();
        mgr.on_stopped().unwrap();
        mgr.on_exit().unwrap();
        assert!(mgr.is_terminated());
        assert!(!mgr.is_running());
    }

    // ---- Loss pending ----

    #[test]
    fn focused_to_loss_pending() {
        let mut mgr = make_manager();
        advance_to_focused(&mut mgr);
        let state = mgr.on_loss_pending().unwrap();
        assert_eq!(state, SessionState::LossPending);
    }

    #[test]
    fn ready_to_loss_pending() {
        let mut mgr = make_manager();
        mgr.request_begin().unwrap();
        let state = mgr.on_loss_pending().unwrap();
        assert_eq!(state, SessionState::LossPending);
    }

    #[test]
    fn synchronized_to_loss_pending() {
        let mut mgr = make_manager();
        mgr.request_begin().unwrap();
        mgr.on_synchronized().unwrap();
        let state = mgr.on_loss_pending().unwrap();
        assert_eq!(state, SessionState::LossPending);
    }

    #[test]
    fn visible_to_loss_pending() {
        let mut mgr = make_manager();
        mgr.request_begin().unwrap();
        mgr.on_synchronized().unwrap();
        mgr.on_visible().unwrap();
        let state = mgr.on_loss_pending().unwrap();
        assert_eq!(state, SessionState::LossPending);
    }

    #[test]
    fn loss_pending_to_exiting() {
        let mut mgr = make_manager();
        advance_to_focused(&mut mgr);
        mgr.on_loss_pending().unwrap();
        let state = mgr.on_exit().unwrap();
        assert_eq!(state, SessionState::Exiting);
    }

    // ---- Invalid transitions ----

    #[test]
    fn idle_cannot_transition_to_synchronized() {
        let mut mgr = make_manager();
        let err = mgr.on_synchronized().unwrap_err();
        assert_eq!(
            err,
            SessionTransitionError::InvalidTransition {
                from: SessionState::Idle,
                to: SessionState::Synchronized,
            }
        );
    }

    #[test]
    fn idle_cannot_request_end() {
        let mut mgr = make_manager();
        let err = mgr.request_end().unwrap_err();
        assert_eq!(
            err,
            SessionTransitionError::InvalidTransition {
                from: SessionState::Idle,
                to: SessionState::Stopping,
            }
        );
    }

    #[test]
    fn ready_cannot_transition_to_visible() {
        let mut mgr = make_manager();
        mgr.request_begin().unwrap();
        let err = mgr.on_visible().unwrap_err();
        assert_eq!(
            err,
            SessionTransitionError::InvalidTransition {
                from: SessionState::Ready,
                to: SessionState::Visible,
            }
        );
    }

    #[test]
    fn synchronized_cannot_transition_to_focused() {
        let mut mgr = make_manager();
        mgr.request_begin().unwrap();
        mgr.on_synchronized().unwrap();
        let err = mgr.on_focused().unwrap_err();
        assert_eq!(
            err,
            SessionTransitionError::InvalidTransition {
                from: SessionState::Synchronized,
                to: SessionState::Focused,
            }
        );
    }

    #[test]
    fn idle_cannot_on_focus_lost() {
        let mut mgr = make_manager();
        let err = mgr.on_focus_lost().unwrap_err();
        assert_eq!(
            err,
            SessionTransitionError::InvalidTransition {
                from: SessionState::Idle,
                to: SessionState::Visible,
            }
        );
    }

    #[test]
    fn ready_cannot_on_invisible() {
        let mut mgr = make_manager();
        mgr.request_begin().unwrap();
        let err = mgr.on_invisible().unwrap_err();
        assert_eq!(
            err,
            SessionTransitionError::InvalidTransition {
                from: SessionState::Ready,
                to: SessionState::Synchronized,
            }
        );
    }

    #[test]
    fn stopping_cannot_on_stopped_twice() {
        let mut mgr = make_manager();
        advance_to_focused(&mut mgr);
        mgr.request_end().unwrap();
        mgr.on_stopped().unwrap();
        // Now in Idle, on_stopped is invalid
        let err = mgr.on_stopped().unwrap_err();
        assert_eq!(
            err,
            SessionTransitionError::InvalidTransition {
                from: SessionState::Idle,
                to: SessionState::Idle,
            }
        );
    }

    #[test]
    fn exiting_cannot_transition() {
        let mut mgr = make_manager();
        mgr.on_exit().unwrap();
        let err = mgr.request_begin().unwrap_err();
        assert_eq!(err, SessionTransitionError::SessionTerminated);
    }

    #[test]
    fn exiting_cannot_on_loss_pending() {
        let mut mgr = make_manager();
        mgr.on_exit().unwrap();
        let err = mgr.on_loss_pending().unwrap_err();
        assert_eq!(err, SessionTransitionError::SessionTerminated);
    }

    #[test]
    fn exiting_cannot_request_end() {
        let mut mgr = make_manager();
        mgr.on_exit().unwrap();
        let err = mgr.request_end().unwrap_err();
        assert_eq!(err, SessionTransitionError::SessionTerminated);
    }

    #[test]
    fn exiting_cannot_on_exit_again() {
        let mut mgr = make_manager();
        mgr.on_exit().unwrap();
        let err = mgr.on_exit().unwrap_err();
        assert_eq!(err, SessionTransitionError::SessionTerminated);
    }

    #[test]
    fn idle_cannot_on_loss_pending() {
        let mut mgr = make_manager();
        let err = mgr.on_loss_pending().unwrap_err();
        assert_eq!(
            err,
            SessionTransitionError::InvalidTransition {
                from: SessionState::Idle,
                to: SessionState::LossPending,
            }
        );
    }

    #[test]
    fn stopping_cannot_on_loss_pending() {
        let mut mgr = make_manager();
        advance_to_focused(&mut mgr);
        mgr.request_end().unwrap();
        let err = mgr.on_loss_pending().unwrap_err();
        assert_eq!(
            err,
            SessionTransitionError::InvalidTransition {
                from: SessionState::Stopping,
                to: SessionState::LossPending,
            }
        );
    }

    #[test]
    fn ready_cannot_request_end() {
        let mut mgr = make_manager();
        mgr.request_begin().unwrap();
        let err = mgr.request_end().unwrap_err();
        assert_eq!(
            err,
            SessionTransitionError::InvalidTransition {
                from: SessionState::Ready,
                to: SessionState::Stopping,
            }
        );
    }

    // ---- Double begin ----

    #[test]
    fn cannot_begin_when_already_ready() {
        let mut mgr = make_manager();
        mgr.request_begin().unwrap();
        let err = mgr.request_begin().unwrap_err();
        assert_eq!(
            err,
            SessionTransitionError::InvalidTransition {
                from: SessionState::Ready,
                to: SessionState::Ready,
            }
        );
    }

    // ---- Reference space ----

    #[test]
    fn default_reference_space_is_local() {
        let mgr = make_manager();
        assert_eq!(
            mgr.reference_space().space_type,
            ReferenceSpaceType::Local
        );
    }

    #[test]
    fn set_reference_space_changes_type() {
        let mut mgr = make_manager();
        mgr.set_reference_space(ReferenceSpaceType::Stage).unwrap();
        assert_eq!(
            mgr.reference_space().space_type,
            ReferenceSpaceType::Stage
        );
    }

    #[test]
    fn set_reference_space_resets_offset() {
        let mut mgr = make_manager();
        mgr.set_reference_space(ReferenceSpaceType::View).unwrap();
        let offset = &mgr.reference_space().offset;
        assert_eq!(offset.position, [0.0, 0.0, 0.0]);
        assert_eq!(offset.rotation, [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn set_reference_space_fails_when_terminated() {
        let mut mgr = make_manager();
        mgr.on_exit().unwrap();
        let err = mgr
            .set_reference_space(ReferenceSpaceType::Stage)
            .unwrap_err();
        assert_eq!(err, SessionTransitionError::SessionTerminated);
    }

    #[test]
    fn reference_space_with_offset() {
        let pose = Pose3 {
            position: [1.0, 2.0, 3.0],
            rotation: [0.0, 0.707, 0.0, 0.707],
            linear_velocity: [0.0, 0.0, 0.0],
            angular_velocity: [0.0, 0.0, 0.0],
        };
        let space = ReferenceSpace::with_offset(ReferenceSpaceType::Stage, pose);
        assert_eq!(space.space_type, ReferenceSpaceType::Stage);
        assert_eq!(space.offset.position, [1.0, 2.0, 3.0]);
    }

    // ---- Frame counting ----

    #[test]
    fn begin_frame_increments_count() {
        let mut mgr = make_manager();
        mgr.begin_frame(1_000_000);
        mgr.begin_frame(2_000_000);
        assert_eq!(mgr.frame_count(), 2);
    }

    #[test]
    fn begin_frame_stores_predicted_time() {
        let mut mgr = make_manager();
        mgr.begin_frame(42_000_000);
        assert_eq!(mgr.last_predicted_display_time_ns(), 42_000_000);
    }

    // ---- Config ----

    #[test]
    fn config_returns_provided_config() {
        let config = SessionConfig {
            application_name: "TestApp".to_string(),
            reference_space: ReferenceSpaceType::Stage,
            prediction_offset_ns: 5_000_000,
            enable_hand_tracking: true,
            enable_haptics: false,
        };
        let mgr = SessionManager::new(config);
        assert_eq!(mgr.config().application_name, "TestApp");
        assert_eq!(
            mgr.config().reference_space,
            ReferenceSpaceType::Stage
        );
        assert!(mgr.config().enable_hand_tracking);
        assert!(!mgr.config().enable_haptics);
    }

    #[test]
    fn default_config_values() {
        let config = SessionConfig::default();
        assert_eq!(config.application_name, "Aether");
        assert_eq!(config.reference_space, ReferenceSpaceType::Local);
        assert_eq!(config.prediction_offset_ns, DEFAULT_PREDICTION_OFFSET_NS);
        assert!(!config.enable_hand_tracking);
        assert!(config.enable_haptics);
    }

    // ---- Restart lifecycle ----

    #[test]
    fn session_can_restart_after_stop() {
        let mut mgr = make_manager();
        // First lifecycle
        advance_to_focused(&mut mgr);
        mgr.request_end().unwrap();
        mgr.on_stopped().unwrap();
        assert_eq!(mgr.state(), SessionState::Idle);

        // Restart
        mgr.request_begin().unwrap();
        mgr.on_synchronized().unwrap();
        mgr.on_visible().unwrap();
        mgr.on_focused().unwrap();
        assert_eq!(mgr.state(), SessionState::Focused);
    }
}
