//! Emulator session state machine and frame timing.
//!
//! Provides a simplified VR session lifecycle that mirrors the essential
//! states of OpenXR but runs entirely on the local machine.

use crate::config::EmulatorConfig;

/// Emulator session states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EmulatorSessionState {
    /// Session created but not started.
    Idle,
    /// Session is ready and about to begin rendering.
    Ready,
    /// Session is actively running and rendering.
    Running,
    /// Session is temporarily paused (e.g., window minimized).
    Paused,
    /// Session is shutting down.
    Stopping,
}

/// Errors during session state transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    /// The requested transition is not valid from the current state.
    InvalidTransition {
        from: EmulatorSessionState,
        to: EmulatorSessionState,
    },
    /// The session has already been stopped and cannot be reused.
    AlreadyStopped,
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionError::InvalidTransition { from, to } => {
                write!(f, "invalid session transition from {from:?} to {to:?}")
            }
            SessionError::AlreadyStopped => write!(f, "session has already been stopped"),
        }
    }
}

/// Frame timing information.
#[derive(Debug, Clone, Copy)]
pub struct FrameTiming {
    /// Target interval between frames in nanoseconds.
    pub target_interval_ns: u64,
    /// Predicted display time for the current frame in nanoseconds.
    pub predicted_display_time_ns: u64,
    /// Actual elapsed time since the last frame in seconds.
    pub delta_time_s: f32,
    /// Total elapsed time since session start in seconds.
    pub total_time_s: f64,
}

/// Manages the emulator session lifecycle and frame timing.
#[derive(Debug)]
pub struct EmulatorSession {
    state: EmulatorSessionState,
    target_interval_ns: u64,
    frame_count: u64,
    total_time_ns: u64,
    last_frame_time_ns: u64,
    stopped: bool,
}

impl EmulatorSession {
    /// Create a new emulator session from configuration.
    pub fn new(config: &EmulatorConfig) -> Self {
        Self {
            state: EmulatorSessionState::Idle,
            target_interval_ns: config.frame_interval_ns(),
            frame_count: 0,
            total_time_ns: 0,
            last_frame_time_ns: 0,
            stopped: false,
        }
    }

    /// Get the current session state.
    pub fn state(&self) -> EmulatorSessionState {
        self.state
    }

    /// Get the total number of frames rendered.
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Get total elapsed time in seconds.
    pub fn total_time_s(&self) -> f64 {
        self.total_time_ns as f64 / 1_000_000_000.0
    }

    /// Check if the session is in a renderable state.
    pub fn should_render(&self) -> bool {
        self.state == EmulatorSessionState::Running
    }

    /// Check if the session is actively running (Running or Paused).
    pub fn is_active(&self) -> bool {
        matches!(
            self.state,
            EmulatorSessionState::Running | EmulatorSessionState::Paused
        )
    }

    /// Start the session: Idle -> Ready.
    pub fn start(&mut self) -> Result<EmulatorSessionState, SessionError> {
        if self.stopped {
            return Err(SessionError::AlreadyStopped);
        }
        if self.state != EmulatorSessionState::Idle {
            return Err(SessionError::InvalidTransition {
                from: self.state,
                to: EmulatorSessionState::Ready,
            });
        }
        self.state = EmulatorSessionState::Ready;
        Ok(self.state)
    }

    /// Begin running: Ready -> Running.
    pub fn begin_running(&mut self) -> Result<EmulatorSessionState, SessionError> {
        if self.stopped {
            return Err(SessionError::AlreadyStopped);
        }
        if self.state != EmulatorSessionState::Ready {
            return Err(SessionError::InvalidTransition {
                from: self.state,
                to: EmulatorSessionState::Running,
            });
        }
        self.state = EmulatorSessionState::Running;
        Ok(self.state)
    }

    /// Pause the session: Running -> Paused.
    pub fn pause(&mut self) -> Result<EmulatorSessionState, SessionError> {
        if self.state != EmulatorSessionState::Running {
            return Err(SessionError::InvalidTransition {
                from: self.state,
                to: EmulatorSessionState::Paused,
            });
        }
        self.state = EmulatorSessionState::Paused;
        Ok(self.state)
    }

    /// Resume from pause: Paused -> Running.
    pub fn resume(&mut self) -> Result<EmulatorSessionState, SessionError> {
        if self.state != EmulatorSessionState::Paused {
            return Err(SessionError::InvalidTransition {
                from: self.state,
                to: EmulatorSessionState::Running,
            });
        }
        self.state = EmulatorSessionState::Running;
        Ok(self.state)
    }

    /// Stop the session: Running|Paused -> Stopping.
    pub fn stop(&mut self) -> Result<EmulatorSessionState, SessionError> {
        match self.state {
            EmulatorSessionState::Running | EmulatorSessionState::Paused => {
                self.state = EmulatorSessionState::Stopping;
                self.stopped = true;
                Ok(self.state)
            }
            _ => Err(SessionError::InvalidTransition {
                from: self.state,
                to: EmulatorSessionState::Stopping,
            }),
        }
    }

    /// Finalize the stop: Stopping -> Idle.
    pub fn finalize_stop(&mut self) -> Result<EmulatorSessionState, SessionError> {
        if self.state != EmulatorSessionState::Stopping {
            return Err(SessionError::InvalidTransition {
                from: self.state,
                to: EmulatorSessionState::Idle,
            });
        }
        self.state = EmulatorSessionState::Idle;
        Ok(self.state)
    }

    /// Record a frame tick with the given delta time in seconds.
    /// Returns frame timing information.
    pub fn tick_frame(&mut self, dt_s: f32) -> FrameTiming {
        let dt_ns = (dt_s as f64 * 1_000_000_000.0) as u64;
        self.total_time_ns = self.total_time_ns.saturating_add(dt_ns);
        self.frame_count = self.frame_count.saturating_add(1);

        let predicted_display_time_ns = self.total_time_ns.saturating_add(self.target_interval_ns);
        self.last_frame_time_ns = dt_ns;

        FrameTiming {
            target_interval_ns: self.target_interval_ns,
            predicted_display_time_ns,
            delta_time_s: dt_s,
            total_time_s: self.total_time_ns as f64 / 1_000_000_000.0,
        }
    }

    /// Get the target refresh rate in Hz.
    pub fn target_refresh_rate_hz(&self) -> u32 {
        if self.target_interval_ns == 0 {
            return 0;
        }
        (1_000_000_000 / self.target_interval_ns) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::HeadsetPreset;

    fn make_session() -> EmulatorSession {
        let config = EmulatorConfig::from_preset(HeadsetPreset::Quest2);
        EmulatorSession::new(&config)
    }

    // ---- Initial state ----

    #[test]
    fn initial_state_is_idle() {
        let session = make_session();
        assert_eq!(session.state(), EmulatorSessionState::Idle);
    }

    #[test]
    fn initial_frame_count_is_zero() {
        let session = make_session();
        assert_eq!(session.frame_count(), 0);
    }

    #[test]
    fn initial_should_render_false() {
        let session = make_session();
        assert!(!session.should_render());
    }

    #[test]
    fn initial_is_not_active() {
        let session = make_session();
        assert!(!session.is_active());
    }

    #[test]
    fn initial_total_time_zero() {
        let session = make_session();
        assert!((session.total_time_s() - 0.0).abs() < 1e-9);
    }

    // ---- Happy path lifecycle ----

    #[test]
    fn idle_to_ready() {
        let mut session = make_session();
        let state = session.start().unwrap();
        assert_eq!(state, EmulatorSessionState::Ready);
    }

    #[test]
    fn ready_to_running() {
        let mut session = make_session();
        session.start().unwrap();
        let state = session.begin_running().unwrap();
        assert_eq!(state, EmulatorSessionState::Running);
        assert!(session.should_render());
        assert!(session.is_active());
    }

    #[test]
    fn running_to_paused() {
        let mut session = make_session();
        session.start().unwrap();
        session.begin_running().unwrap();
        let state = session.pause().unwrap();
        assert_eq!(state, EmulatorSessionState::Paused);
        assert!(!session.should_render());
        assert!(session.is_active());
    }

    #[test]
    fn paused_to_running() {
        let mut session = make_session();
        session.start().unwrap();
        session.begin_running().unwrap();
        session.pause().unwrap();
        let state = session.resume().unwrap();
        assert_eq!(state, EmulatorSessionState::Running);
        assert!(session.should_render());
    }

    #[test]
    fn running_to_stopping() {
        let mut session = make_session();
        session.start().unwrap();
        session.begin_running().unwrap();
        let state = session.stop().unwrap();
        assert_eq!(state, EmulatorSessionState::Stopping);
        assert!(!session.should_render());
        assert!(!session.is_active());
    }

    #[test]
    fn paused_to_stopping() {
        let mut session = make_session();
        session.start().unwrap();
        session.begin_running().unwrap();
        session.pause().unwrap();
        let state = session.stop().unwrap();
        assert_eq!(state, EmulatorSessionState::Stopping);
    }

    #[test]
    fn stopping_to_idle() {
        let mut session = make_session();
        session.start().unwrap();
        session.begin_running().unwrap();
        session.stop().unwrap();
        let state = session.finalize_stop().unwrap();
        assert_eq!(state, EmulatorSessionState::Idle);
    }

    // ---- Invalid transitions ----

    #[test]
    fn cannot_start_when_not_idle() {
        let mut session = make_session();
        session.start().unwrap();
        let err = session.start().unwrap_err();
        assert_eq!(
            err,
            SessionError::InvalidTransition {
                from: EmulatorSessionState::Ready,
                to: EmulatorSessionState::Ready,
            }
        );
    }

    #[test]
    fn cannot_begin_running_when_idle() {
        let mut session = make_session();
        let err = session.begin_running().unwrap_err();
        assert_eq!(
            err,
            SessionError::InvalidTransition {
                from: EmulatorSessionState::Idle,
                to: EmulatorSessionState::Running,
            }
        );
    }

    #[test]
    fn cannot_pause_when_idle() {
        let mut session = make_session();
        let err = session.pause().unwrap_err();
        assert_eq!(
            err,
            SessionError::InvalidTransition {
                from: EmulatorSessionState::Idle,
                to: EmulatorSessionState::Paused,
            }
        );
    }

    #[test]
    fn cannot_resume_when_running() {
        let mut session = make_session();
        session.start().unwrap();
        session.begin_running().unwrap();
        let err = session.resume().unwrap_err();
        assert_eq!(
            err,
            SessionError::InvalidTransition {
                from: EmulatorSessionState::Running,
                to: EmulatorSessionState::Running,
            }
        );
    }

    #[test]
    fn cannot_stop_when_idle() {
        let mut session = make_session();
        let err = session.stop().unwrap_err();
        assert_eq!(
            err,
            SessionError::InvalidTransition {
                from: EmulatorSessionState::Idle,
                to: EmulatorSessionState::Stopping,
            }
        );
    }

    #[test]
    fn cannot_finalize_when_not_stopping() {
        let mut session = make_session();
        let err = session.finalize_stop().unwrap_err();
        assert_eq!(
            err,
            SessionError::InvalidTransition {
                from: EmulatorSessionState::Idle,
                to: EmulatorSessionState::Idle,
            }
        );
    }

    #[test]
    fn cannot_restart_after_stop() {
        let mut session = make_session();
        session.start().unwrap();
        session.begin_running().unwrap();
        session.stop().unwrap();
        session.finalize_stop().unwrap();
        let err = session.start().unwrap_err();
        assert_eq!(err, SessionError::AlreadyStopped);
    }

    // ---- Frame timing ----

    #[test]
    fn tick_frame_increments_count() {
        let mut session = make_session();
        session.tick_frame(1.0 / 60.0);
        session.tick_frame(1.0 / 60.0);
        assert_eq!(session.frame_count(), 2);
    }

    #[test]
    fn tick_frame_accumulates_time() {
        let mut session = make_session();
        session.tick_frame(0.5);
        session.tick_frame(0.5);
        let total = session.total_time_s();
        assert!((total - 1.0).abs() < 0.01, "total={total}");
    }

    #[test]
    fn tick_frame_returns_correct_delta() {
        let mut session = make_session();
        let timing = session.tick_frame(0.016);
        assert!((timing.delta_time_s - 0.016).abs() < 1e-6);
    }

    #[test]
    fn tick_frame_returns_correct_total_time() {
        let mut session = make_session();
        session.tick_frame(1.0);
        let timing = session.tick_frame(0.5);
        assert!((timing.total_time_s - 1.5).abs() < 0.01);
    }

    #[test]
    fn tick_frame_predicted_display_time_is_future() {
        let mut session = make_session();
        let timing = session.tick_frame(0.011);
        assert!(timing.predicted_display_time_ns > 0);
        // predicted = total_time_ns + target_interval_ns, so it's always ahead
        let total_ns = (0.011f64 * 1_000_000_000.0) as u64;
        assert!(timing.predicted_display_time_ns > total_ns);
    }

    #[test]
    fn target_interval_matches_config() {
        let mut session = make_session();
        let timing = session.tick_frame(0.01);
        assert_eq!(timing.target_interval_ns, 1_000_000_000 / 90);
    }

    // ---- Refresh rate ----

    #[test]
    fn target_refresh_rate_90hz() {
        let session = make_session();
        assert_eq!(session.target_refresh_rate_hz(), 90);
    }

    #[test]
    fn target_refresh_rate_120hz() {
        let config = EmulatorConfig::from_preset(HeadsetPreset::Quest3);
        let session = EmulatorSession::new(&config);
        assert_eq!(session.target_refresh_rate_hz(), 120);
    }

    #[test]
    fn target_refresh_rate_144hz() {
        let config = EmulatorConfig::from_preset(HeadsetPreset::ValveIndex);
        let session = EmulatorSession::new(&config);
        assert_eq!(session.target_refresh_rate_hz(), 144);
    }

    // ---- Error display ----

    #[test]
    fn session_error_display() {
        let err = SessionError::InvalidTransition {
            from: EmulatorSessionState::Idle,
            to: EmulatorSessionState::Running,
        };
        let msg = format!("{err}");
        assert!(msg.contains("Idle"));
        assert!(msg.contains("Running"));
    }

    #[test]
    fn already_stopped_error_display() {
        let err = SessionError::AlreadyStopped;
        let msg = format!("{err}");
        assert!(msg.contains("stopped"));
    }
}
