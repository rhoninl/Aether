//! PC-based VR emulator for development and testing without a headset.
//!
//! Provides an emulated VR environment that renders stereo or mono preview
//! in a desktop window, mapping keyboard/mouse input to VR controllers and
//! head tracking.

pub mod config;
pub mod controller;
pub mod display;
pub mod head_tracking;
pub mod session;
pub mod window;

pub use config::{
    ConfigError, DisplayConfig, EmulatorConfig, EyeResolution, HeadsetPreset, InputSensitivity,
    ViewMode,
};
pub use controller::{ControllerInput, EmulatedControllers};
pub use display::{Eye, EyeView, StereoDisplay, Viewport};
pub use head_tracking::{EmulatedHeadTracker, HeadPresetPosition};
pub use session::{EmulatorSession, EmulatorSessionState, FrameTiming, SessionError};
pub use window::{DebugOverlayInfo, EmulatorFrameBuffer, EmulatorWindow};

use aether_input::openxr_tracking::{TrackingConfidence, TrackingSnapshot};

/// The main VR emulator that ties all subsystems together.
///
/// Usage:
/// ```no_run
/// use aether_vr_emulator::{VrEmulator, HeadsetPreset};
///
/// let mut emulator = VrEmulator::new_windowed(HeadsetPreset::Quest2).unwrap();
/// while emulator.is_running() {
///     let snapshot = emulator.update(1.0 / 90.0);
///     // Use snapshot for rendering/game logic
///     let mut fb = emulator.create_framebuffer();
///     fb.clear_sky();
///     emulator.present(&fb).unwrap();
/// }
/// ```
pub struct VrEmulator {
    config: EmulatorConfig,
    session: EmulatorSession,
    head_tracker: EmulatedHeadTracker,
    controllers: EmulatedControllers,
    stereo_display: StereoDisplay,
    window: Option<EmulatorWindow>,
    last_snapshot: TrackingSnapshot,
    frame_time_ms: f32,
}

impl VrEmulator {
    /// Create a new VR emulator with a desktop window.
    pub fn new_windowed(preset: HeadsetPreset) -> Result<Self, String> {
        let config = EmulatorConfig::from_preset(preset);
        config.validate().map_err(|e| format!("{e}"))?;

        let mut window = EmulatorWindow::create(&config)?;
        window.set_target_fps(config.display.refresh_rate_hz as usize);

        let session = EmulatorSession::new(&config);
        let head_tracker = EmulatedHeadTracker::new(&config.input_sensitivity);
        let controllers = EmulatedControllers::new(&config.input_sensitivity);
        let stereo_display =
            StereoDisplay::new(&config.display, config.window_width, config.window_height);

        let mut emulator = Self {
            config,
            session,
            head_tracker,
            controllers,
            stereo_display,
            window: Some(window),
            last_snapshot: TrackingSnapshot::empty(0),
            frame_time_ms: 0.0,
        };

        // Auto-start the session
        emulator.session.start().map_err(|e| format!("{e}"))?;
        emulator
            .session
            .begin_running()
            .map_err(|e| format!("{e}"))?;

        Ok(emulator)
    }

    /// Create a new headless VR emulator (no window, for testing).
    pub fn new_headless(config: EmulatorConfig) -> Result<Self, String> {
        config.validate().map_err(|e| format!("{e}"))?;

        let session = EmulatorSession::new(&config);
        let head_tracker = EmulatedHeadTracker::new(&config.input_sensitivity);
        let controllers = EmulatedControllers::new(&config.input_sensitivity);
        let stereo_display =
            StereoDisplay::new(&config.display, config.window_width, config.window_height);

        let mut emulator = Self {
            config,
            session,
            head_tracker,
            controllers,
            stereo_display,
            window: None,
            last_snapshot: TrackingSnapshot::empty(0),
            frame_time_ms: 0.0,
        };

        emulator.session.start().map_err(|e| format!("{e}"))?;
        emulator
            .session
            .begin_running()
            .map_err(|e| format!("{e}"))?;

        Ok(emulator)
    }

    /// Check if the emulator is still running (session active and window open).
    pub fn is_running(&self) -> bool {
        let session_active = self.session.should_render();
        let window_open = self.window.as_ref().is_none_or(|w| w.is_open());
        session_active && window_open
    }

    /// Update the emulator for one frame.
    ///
    /// Polls input (if windowed), updates head tracking and controllers,
    /// and returns a tracking snapshot.
    pub fn update(&mut self, dt_s: f32) -> TrackingSnapshot {
        self.frame_time_ms = dt_s * 1000.0;
        let timing = self.session.tick_frame(dt_s);

        // Poll input from window if available
        let (controller_input, mouse_dx, mouse_dy) = if let Some(window) = &mut self.window {
            let (input, dx, dy) = window.poll_input();

            // Handle head movement via arrow keys
            let head_fwd = window.is_key_down(minifb::Key::Up);
            let head_back = window.is_key_down(minifb::Key::Down);
            let head_left = window.is_key_down(minifb::Key::Left);
            let head_right = window.is_key_down(minifb::Key::Right);
            let head_up = window.is_key_down(minifb::Key::R);
            let head_down = window.is_key_down(minifb::Key::F);

            self.head_tracker.apply_movement(
                head_fwd, head_back, head_left, head_right, head_up, head_down, dt_s,
            );

            // Handle preset positions
            if window.is_key_down(minifb::Key::F1) {
                self.head_tracker
                    .set_preset(HeadPresetPosition::StandingCenter);
            }
            if window.is_key_down(minifb::Key::F2) {
                self.head_tracker
                    .set_preset(HeadPresetPosition::SeatedCenter);
            }

            (input, dx, dy)
        } else {
            (ControllerInput::default(), 0.0, 0.0)
        };

        // Update head tracking from mouse
        self.head_tracker.apply_mouse_look(mouse_dx, mouse_dy);

        // Update controllers
        let head_pos = self.head_tracker.position();
        let head_yaw = self.head_tracker.yaw_rad();
        let (left_ctrl, right_ctrl) =
            self.controllers
                .update(&controller_input, head_pos, head_yaw, dt_s);

        // Build tracking snapshot
        let snapshot = TrackingSnapshot {
            timestamp_ns: timing
                .predicted_display_time_ns
                .saturating_sub(timing.target_interval_ns),
            predicted_display_time_ns: timing.predicted_display_time_ns,
            head_pose: self.head_tracker.to_pose(),
            head_confidence: TrackingConfidence::High,
            left_controller: left_ctrl,
            right_controller: right_ctrl,
            left_hand: None,
            right_hand: None,
        };

        self.last_snapshot = snapshot.clone();
        snapshot
    }

    /// Update with explicit controller input (for headless / testing mode).
    pub fn update_with_input(
        &mut self,
        dt_s: f32,
        controller_input: &ControllerInput,
        mouse_dx: f32,
        mouse_dy: f32,
    ) -> TrackingSnapshot {
        self.frame_time_ms = dt_s * 1000.0;
        let timing = self.session.tick_frame(dt_s);

        self.head_tracker.apply_mouse_look(mouse_dx, mouse_dy);

        let head_pos = self.head_tracker.position();
        let head_yaw = self.head_tracker.yaw_rad();
        let (left_ctrl, right_ctrl) =
            self.controllers
                .update(controller_input, head_pos, head_yaw, dt_s);

        let snapshot = TrackingSnapshot {
            timestamp_ns: timing
                .predicted_display_time_ns
                .saturating_sub(timing.target_interval_ns),
            predicted_display_time_ns: timing.predicted_display_time_ns,
            head_pose: self.head_tracker.to_pose(),
            head_confidence: TrackingConfidence::High,
            left_controller: left_ctrl,
            right_controller: right_ctrl,
            left_hand: None,
            right_hand: None,
        };

        self.last_snapshot = snapshot.clone();
        snapshot
    }

    /// Get the left eye view for rendering.
    pub fn left_eye_view(&self) -> EyeView {
        self.stereo_display.eye_view(
            Eye::Left,
            self.head_tracker.position(),
            self.head_tracker.yaw_rad(),
            self.head_tracker.pitch_rad(),
        )
    }

    /// Get the right eye view for rendering.
    pub fn right_eye_view(&self) -> EyeView {
        self.stereo_display.eye_view(
            Eye::Right,
            self.head_tracker.position(),
            self.head_tracker.yaw_rad(),
            self.head_tracker.pitch_rad(),
        )
    }

    /// Get the stereo display.
    pub fn display(&self) -> &StereoDisplay {
        &self.stereo_display
    }

    /// Get the last tracking snapshot.
    pub fn last_snapshot(&self) -> &TrackingSnapshot {
        &self.last_snapshot
    }

    /// Get the emulator configuration.
    pub fn config(&self) -> &EmulatorConfig {
        &self.config
    }

    /// Get the session.
    pub fn session(&self) -> &EmulatorSession {
        &self.session
    }

    /// Get a mutable reference to the head tracker.
    pub fn head_tracker_mut(&mut self) -> &mut EmulatedHeadTracker {
        &mut self.head_tracker
    }

    /// Get a reference to the head tracker.
    pub fn head_tracker(&self) -> &EmulatedHeadTracker {
        &self.head_tracker
    }

    /// Create a new framebuffer matching the window dimensions.
    pub fn create_framebuffer(&self) -> EmulatorFrameBuffer {
        EmulatorFrameBuffer::new(self.config.window_width, self.config.window_height)
    }

    /// Build debug overlay info from current state.
    pub fn build_debug_info(&self) -> DebugOverlayInfo {
        let fps = if self.frame_time_ms > 0.0 {
            1000.0 / self.frame_time_ms
        } else {
            0.0
        };

        DebugOverlayInfo {
            fps,
            frame_time_ms: self.frame_time_ms,
            head_position: self.head_tracker.position(),
            head_yaw_deg: self.head_tracker.yaw_rad().to_degrees(),
            head_pitch_deg: self.head_tracker.pitch_rad().to_degrees(),
            left_controller_pos: self.last_snapshot.left_controller.grip_pose.position,
            right_controller_pos: self.last_snapshot.right_controller.grip_pose.position,
            session_state: format!("{:?}", self.session.state()),
            frame_count: self.session.frame_count(),
        }
    }

    /// Present a framebuffer to the window (with optional debug overlay).
    pub fn present(&mut self, fb: &EmulatorFrameBuffer) -> Result<(), String> {
        if let Some(window) = &mut self.window {
            window.present(fb)
        } else {
            Ok(())
        }
    }

    /// Present a framebuffer with debug overlay automatically drawn.
    pub fn present_with_overlay(&mut self, fb: &mut EmulatorFrameBuffer) -> Result<(), String> {
        if self.config.show_debug_overlay {
            let info = self.build_debug_info();
            window::draw_debug_overlay(fb, &info);
        }

        if self.config.display.view_mode == ViewMode::Stereo {
            fb.draw_stereo_divider();
        }

        if let Some(window) = &mut self.window {
            window.present(fb)
        } else {
            Ok(())
        }
    }

    /// Stop the emulator session.
    pub fn stop(&mut self) -> Result<(), String> {
        self.session.stop().map_err(|e| format!("{e}"))?;
        self.session.finalize_stop().map_err(|e| format!("{e}"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_input::openxr_tracking::Hand;

    fn headless_emulator() -> VrEmulator {
        VrEmulator::new_headless(EmulatorConfig::default()).unwrap()
    }

    // ---- Creation ----

    #[test]
    fn headless_emulator_creates_successfully() {
        let emulator = headless_emulator();
        assert!(emulator.is_running());
    }

    #[test]
    fn headless_emulator_invalid_config_fails() {
        let mut config = EmulatorConfig::default();
        config.display.ipd_mm = 0.0; // invalid
        let result = VrEmulator::new_headless(config);
        assert!(result.is_err());
    }

    #[test]
    fn headless_emulator_starts_in_running_state() {
        let emulator = headless_emulator();
        assert_eq!(emulator.session().state(), EmulatorSessionState::Running);
    }

    // ---- Update ----

    #[test]
    fn update_returns_snapshot() {
        let mut emulator = headless_emulator();
        let snapshot = emulator.update(0.016);
        assert_eq!(snapshot.head_confidence, TrackingConfidence::High);
    }

    #[test]
    fn update_controllers_connected() {
        let mut emulator = headless_emulator();
        let snapshot = emulator.update(0.016);
        assert!(snapshot.left_controller.connected);
        assert!(snapshot.right_controller.connected);
        assert_eq!(snapshot.left_controller.hand, Hand::Left);
        assert_eq!(snapshot.right_controller.hand, Hand::Right);
    }

    #[test]
    fn update_increments_frame_count() {
        let mut emulator = headless_emulator();
        emulator.update(0.016);
        emulator.update(0.016);
        assert_eq!(emulator.session().frame_count(), 2);
    }

    #[test]
    fn update_with_input_applies_controller() {
        let mut emulator = headless_emulator();
        let mut input = ControllerInput::default();
        input.left_trigger = true;
        let snapshot = emulator.update_with_input(0.016, &input, 0.0, 0.0);
        assert!(snapshot.left_controller.buttons.trigger_click);
    }

    #[test]
    fn update_with_mouse_changes_head_pose() {
        let mut emulator = headless_emulator();
        let snap1 = emulator.update_with_input(0.016, &ControllerInput::default(), 0.0, 0.0);
        let snap2 = emulator.update_with_input(0.016, &ControllerInput::default(), 100.0, 0.0);
        // Head rotation should have changed
        assert_ne!(snap1.head_pose.rotation, snap2.head_pose.rotation);
    }

    // ---- Eye views ----

    #[test]
    fn left_and_right_eye_views_differ() {
        let mut emulator = headless_emulator();
        emulator.update(0.016);
        let left = emulator.left_eye_view();
        let right = emulator.right_eye_view();
        // Positions should differ by IPD
        assert_ne!(left.position[0], right.position[0]);
    }

    #[test]
    fn eye_views_same_forward_direction() {
        let mut emulator = headless_emulator();
        emulator.update(0.016);
        let left = emulator.left_eye_view();
        let right = emulator.right_eye_view();
        // Both eyes look in the same direction
        for i in 0..3 {
            assert!(
                (left.forward[i] - right.forward[i]).abs() < 1e-4,
                "forward[{i}] differs"
            );
        }
    }

    // ---- Framebuffer ----

    #[test]
    fn create_framebuffer_correct_size() {
        let emulator = headless_emulator();
        let fb = emulator.create_framebuffer();
        assert_eq!(fb.width, emulator.config().window_width);
        assert_eq!(fb.height, emulator.config().window_height);
    }

    // ---- Debug info ----

    #[test]
    fn build_debug_info_populated() {
        let mut emulator = headless_emulator();
        emulator.update(0.016);
        let info = emulator.build_debug_info();
        assert!(info.fps > 0.0);
        assert_eq!(info.frame_count, 1);
        assert_eq!(info.session_state, "Running");
    }

    // ---- Headless present ----

    #[test]
    fn headless_present_succeeds() {
        let mut emulator = headless_emulator();
        let fb = emulator.create_framebuffer();
        assert!(emulator.present(&fb).is_ok());
    }

    #[test]
    fn headless_present_with_overlay_succeeds() {
        let mut emulator = headless_emulator();
        emulator.update(0.016);
        let mut fb = emulator.create_framebuffer();
        fb.clear_sky();
        assert!(emulator.present_with_overlay(&mut fb).is_ok());
    }

    // ---- Stop ----

    #[test]
    fn stop_transitions_to_idle() {
        let mut emulator = headless_emulator();
        emulator.stop().unwrap();
        assert!(!emulator.is_running());
    }

    // ---- Last snapshot ----

    #[test]
    fn last_snapshot_updated_after_update() {
        let mut emulator = headless_emulator();
        let snap = emulator.update(0.016);
        let stored = emulator.last_snapshot();
        assert_eq!(stored.timestamp_ns, snap.timestamp_ns);
    }

    // ---- Config access ----

    #[test]
    fn config_returns_correct_preset() {
        let emulator = headless_emulator();
        assert_eq!(emulator.config().display.refresh_rate_hz, 90);
    }

    // ---- Head tracker access ----

    #[test]
    fn head_tracker_mut_allows_modification() {
        let mut emulator = headless_emulator();
        emulator
            .head_tracker_mut()
            .set_preset(HeadPresetPosition::SeatedCenter);
        let pos = emulator.head_tracker().position();
        assert!((pos[1] - 1.2).abs() < 0.01);
    }
}
