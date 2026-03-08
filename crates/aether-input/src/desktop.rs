//! Desktop (keyboard + mouse) input adapter for flat-screen development and testing.

use std::collections::HashSet;

use crate::actions::{ActionPhase, InteractionEvent, XRButton};
use crate::adapter::{InputFrame, InputFrameError, RuntimeAdapter};
use crate::capabilities::{InputActionPath, InputBackend, InputFrameHint};
use crate::locomotion::LocomotionProfile;

/// Keyboard key codes for desktop input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    W,
    A,
    S,
    D,
    Q,
    E,
    R,
    F,
    Space,
    Shift,
    Ctrl,
    Alt,
    Tab,
    Escape,
    Up,
    Down,
    Left,
    Right,
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
}

/// Mouse axis identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseAxis {
    X,
    Y,
    ScrollY,
}

/// Mouse button identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Configuration for the desktop input adapter.
#[derive(Debug, Clone)]
pub struct DesktopAdapterConfig {
    /// Player ID this adapter represents.
    pub player_id: u64,
    /// Mouse sensitivity multiplier for look rotation.
    pub mouse_sensitivity: f32,
    /// Session identifier.
    pub session_id: String,
}

impl Default for DesktopAdapterConfig {
    fn default() -> Self {
        Self {
            player_id: 1,
            mouse_sensitivity: 1.0,
            session_id: "desktop".to_string(),
        }
    }
}

/// Current state of desktop input devices.
#[derive(Debug, Clone)]
pub struct DesktopInputState {
    /// Currently pressed keyboard keys.
    pub pressed_keys: HashSet<KeyCode>,
    /// Keys that were just pressed this frame (not pressed last frame).
    pub just_pressed: HashSet<KeyCode>,
    /// Keys that were just released this frame (pressed last frame, not now).
    pub just_released: HashSet<KeyCode>,
    /// Currently pressed mouse buttons.
    pub pressed_mouse_buttons: HashSet<MouseButton>,
    /// Mouse buttons just pressed this frame.
    pub just_pressed_mouse: HashSet<MouseButton>,
    /// Mouse buttons just released this frame.
    pub just_released_mouse: HashSet<MouseButton>,
    /// Mouse movement delta since last frame.
    pub mouse_delta_x: f32,
    /// Mouse movement delta since last frame.
    pub mouse_delta_y: f32,
    /// Mouse scroll delta since last frame.
    pub scroll_delta_y: f32,
}

impl Default for DesktopInputState {
    fn default() -> Self {
        Self {
            pressed_keys: HashSet::new(),
            just_pressed: HashSet::new(),
            just_released: HashSet::new(),
            pressed_mouse_buttons: HashSet::new(),
            just_pressed_mouse: HashSet::new(),
            just_released_mouse: HashSet::new(),
            mouse_delta_x: 0.0,
            mouse_delta_y: 0.0,
            scroll_delta_y: 0.0,
        }
    }
}

/// Desktop adapter implementing `RuntimeAdapter` for keyboard + mouse.
pub struct DesktopAdapter {
    config: DesktopAdapterConfig,
    state: DesktopInputState,
    prev_keys: HashSet<KeyCode>,
    prev_mouse: HashSet<MouseButton>,
    locomotion_profile: Option<LocomotionProfile>,
    frame_counter: u64,
}

impl std::fmt::Debug for DesktopAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DesktopAdapter")
            .field("config", &self.config)
            .field("frame_counter", &self.frame_counter)
            .finish()
    }
}

impl DesktopAdapter {
    pub fn new(config: DesktopAdapterConfig) -> Self {
        Self {
            config,
            state: DesktopInputState::default(),
            prev_keys: HashSet::new(),
            prev_mouse: HashSet::new(),
            locomotion_profile: None,
            frame_counter: 0,
        }
    }

    /// Update a key's pressed state. Call before `poll_frame`.
    pub fn update_key(&mut self, key: KeyCode, pressed: bool) {
        if pressed {
            self.state.pressed_keys.insert(key);
        } else {
            self.state.pressed_keys.remove(&key);
        }
    }

    /// Update mouse movement deltas. Call before `poll_frame`.
    pub fn update_mouse(&mut self, dx: f32, dy: f32) {
        self.state.mouse_delta_x = dx;
        self.state.mouse_delta_y = dy;
    }

    /// Update mouse button state. Call before `poll_frame`.
    pub fn update_mouse_button(&mut self, button: MouseButton, pressed: bool) {
        if pressed {
            self.state.pressed_mouse_buttons.insert(button);
        } else {
            self.state.pressed_mouse_buttons.remove(&button);
        }
    }

    /// Update scroll wheel delta. Call before `poll_frame`.
    pub fn update_scroll(&mut self, delta_y: f32) {
        self.state.scroll_delta_y = delta_y;
    }

    /// Compute just_pressed / just_released diffs and generate events.
    fn compute_frame_diffs(&mut self) {
        self.state.just_pressed.clear();
        self.state.just_released.clear();
        self.state.just_pressed_mouse.clear();
        self.state.just_released_mouse.clear();

        for key in &self.state.pressed_keys {
            if !self.prev_keys.contains(key) {
                self.state.just_pressed.insert(*key);
            }
        }
        for key in &self.prev_keys {
            if !self.state.pressed_keys.contains(key) {
                self.state.just_released.insert(*key);
            }
        }

        for btn in &self.state.pressed_mouse_buttons {
            if !self.prev_mouse.contains(btn) {
                self.state.just_pressed_mouse.insert(*btn);
            }
        }
        for btn in &self.prev_mouse {
            if !self.state.pressed_mouse_buttons.contains(btn) {
                self.state.just_released_mouse.insert(*btn);
            }
        }
    }

    /// Save current state as previous for next frame.
    fn save_prev_state(&mut self) {
        self.prev_keys = self.state.pressed_keys.clone();
        self.prev_mouse = self.state.pressed_mouse_buttons.clone();
    }

    /// Build interaction events from the current state.
    fn build_events(&self) -> Vec<InteractionEvent> {
        let mut events = Vec::new();
        let pid = self.config.player_id;

        // Map just-pressed keys to Started events
        for key in &self.state.just_pressed {
            if let Some((button, hand)) = map_key_to_xr(*key) {
                events.push(InteractionEvent {
                    player_id: pid,
                    hand,
                    button,
                    phase: ActionPhase::Started,
                    force: 1.0,
                    target: None,
                    hand_pose: None,
                });
            }
        }

        // Map just-released keys to Canceled events
        for key in &self.state.just_released {
            if let Some((button, hand)) = map_key_to_xr(*key) {
                events.push(InteractionEvent {
                    player_id: pid,
                    hand,
                    button,
                    phase: ActionPhase::Canceled,
                    force: 0.0,
                    target: None,
                    hand_pose: None,
                });
            }
        }

        // Map mouse buttons
        for btn in &self.state.just_pressed_mouse {
            if let Some((button, hand)) = map_mouse_button_to_xr(*btn) {
                events.push(InteractionEvent {
                    player_id: pid,
                    hand,
                    button,
                    phase: ActionPhase::Started,
                    force: 1.0,
                    target: None,
                    hand_pose: None,
                });
            }
        }

        for btn in &self.state.just_released_mouse {
            if let Some((button, hand)) = map_mouse_button_to_xr(*btn) {
                events.push(InteractionEvent {
                    player_id: pid,
                    hand,
                    button,
                    phase: ActionPhase::Canceled,
                    force: 0.0,
                    target: None,
                    hand_pose: None,
                });
            }
        }

        events
    }

    /// Get the current input state (for external pipeline use).
    pub fn state(&self) -> &DesktopInputState {
        &self.state
    }
}

/// Map a keyboard key to an XR button + hand for default desktop bindings.
fn map_key_to_xr(key: KeyCode) -> Option<(XRButton, InputActionPath)> {
    match key {
        KeyCode::Space => Some((XRButton::Trigger, InputActionPath::RightHand)),
        KeyCode::Shift => Some((XRButton::Grip, InputActionPath::LeftHand)),
        KeyCode::E => Some((XRButton::A, InputActionPath::RightHand)),
        KeyCode::Q => Some((XRButton::B, InputActionPath::LeftHand)),
        KeyCode::R => Some((XRButton::X, InputActionPath::RightHand)),
        KeyCode::F => Some((XRButton::Y, InputActionPath::LeftHand)),
        KeyCode::Tab => Some((XRButton::Menu, InputActionPath::LeftHand)),
        KeyCode::Escape => Some((XRButton::System, InputActionPath::LeftHand)),
        _ => None,
    }
}

/// Map a mouse button to an XR button + hand.
fn map_mouse_button_to_xr(button: MouseButton) -> Option<(XRButton, InputActionPath)> {
    match button {
        MouseButton::Left => Some((XRButton::Trigger, InputActionPath::RightHand)),
        MouseButton::Right => Some((XRButton::Grip, InputActionPath::RightHand)),
        MouseButton::Middle => Some((XRButton::Thumbstick, InputActionPath::RightHand)),
    }
}

impl RuntimeAdapter for DesktopAdapter {
    fn backend(&self) -> InputBackend {
        InputBackend::Unknown
    }

    fn advertised_capabilities(&self) -> InputFrameHint {
        InputFrameHint {
            backend: InputBackend::Unknown,
            session_id: self.config.session_id.clone(),
            capabilities: vec![],
        }
    }

    fn poll_frame(&mut self) -> Result<InputFrame, InputFrameError> {
        self.frame_counter = self.frame_counter.saturating_add(1);
        self.compute_frame_diffs();

        let events = self.build_events();

        // Save state for next frame diff
        self.save_prev_state();

        // Clear per-frame deltas
        self.state.mouse_delta_x = 0.0;
        self.state.mouse_delta_y = 0.0;
        self.state.scroll_delta_y = 0.0;

        Ok(InputFrame {
            backend: InputBackend::Unknown,
            player_id: self.config.player_id,
            timestamp_ms: self.frame_counter,
            events,
        })
    }

    fn apply_locomotion_profile(&mut self, profile: &LocomotionProfile) {
        self.locomotion_profile = Some(profile.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_adapter() -> DesktopAdapter {
        DesktopAdapter::new(DesktopAdapterConfig {
            player_id: 1,
            mouse_sensitivity: 1.0,
            session_id: "test".to_string(),
        })
    }

    #[test]
    fn no_input_produces_empty_events() {
        let mut adapter = make_adapter();
        let frame = adapter.poll_frame().unwrap();
        assert!(frame.events.is_empty());
        assert_eq!(frame.player_id, 1);
    }

    #[test]
    fn key_press_generates_started_event() {
        let mut adapter = make_adapter();
        adapter.update_key(KeyCode::Space, true);
        let frame = adapter.poll_frame().unwrap();
        assert_eq!(frame.events.len(), 1);
        assert_eq!(frame.events[0].button, XRButton::Trigger);
        assert_eq!(frame.events[0].phase, ActionPhase::Started);
        assert_eq!(frame.events[0].force, 1.0);
    }

    #[test]
    fn key_release_generates_canceled_event() {
        let mut adapter = make_adapter();
        // First press
        adapter.update_key(KeyCode::Space, true);
        let _ = adapter.poll_frame().unwrap();
        // Then release
        adapter.update_key(KeyCode::Space, false);
        let frame = adapter.poll_frame().unwrap();
        assert_eq!(frame.events.len(), 1);
        assert_eq!(frame.events[0].phase, ActionPhase::Canceled);
        assert_eq!(frame.events[0].force, 0.0);
    }

    #[test]
    fn held_key_does_not_repeat_started() {
        let mut adapter = make_adapter();
        adapter.update_key(KeyCode::Space, true);
        let frame1 = adapter.poll_frame().unwrap();
        assert_eq!(frame1.events.len(), 1); // Started

        // Same key still held, no new press
        let frame2 = adapter.poll_frame().unwrap();
        assert!(frame2.events.is_empty()); // No repeat
    }

    #[test]
    fn mouse_button_generates_events() {
        let mut adapter = make_adapter();
        adapter.update_mouse_button(MouseButton::Left, true);
        let frame = adapter.poll_frame().unwrap();
        assert_eq!(frame.events.len(), 1);
        assert_eq!(frame.events[0].button, XRButton::Trigger);
        assert_eq!(frame.events[0].phase, ActionPhase::Started);
    }

    #[test]
    fn mouse_delta_is_cleared_after_poll() {
        let mut adapter = make_adapter();
        adapter.update_mouse(10.0, -5.0);
        let _ = adapter.poll_frame().unwrap();
        assert_eq!(adapter.state().mouse_delta_x, 0.0);
        assert_eq!(adapter.state().mouse_delta_y, 0.0);
    }

    #[test]
    fn unmapped_key_produces_no_event() {
        let mut adapter = make_adapter();
        // WASD keys are not mapped to XR buttons (they're for movement via action map)
        adapter.update_key(KeyCode::W, true);
        let frame = adapter.poll_frame().unwrap();
        assert!(frame.events.is_empty());
    }

    #[test]
    fn multiple_keys_produce_multiple_events() {
        let mut adapter = make_adapter();
        adapter.update_key(KeyCode::Space, true);
        adapter.update_key(KeyCode::E, true);
        let frame = adapter.poll_frame().unwrap();
        assert_eq!(frame.events.len(), 2);
    }

    #[test]
    fn backend_is_unknown_for_desktop() {
        let adapter = make_adapter();
        assert_eq!(adapter.backend(), InputBackend::Unknown);
    }

    #[test]
    fn frame_counter_increments() {
        let mut adapter = make_adapter();
        let f1 = adapter.poll_frame().unwrap();
        let f2 = adapter.poll_frame().unwrap();
        assert_eq!(f1.timestamp_ms, 1);
        assert_eq!(f2.timestamp_ms, 2);
    }

    #[test]
    fn apply_locomotion_profile_stores_profile() {
        let mut adapter = make_adapter();
        let profile = LocomotionProfile {
            allowed_modes: vec![crate::locomotion::LocomotionMode::Smooth],
            active: crate::locomotion::LocomotionMode::Smooth,
            comfort: crate::locomotion::ComfortProfile {
                enabled: true,
                style: crate::locomotion::ComfortStyle::SnapTurnStepDeg(30),
                rotation_speed_deg_per_s: 120.0,
                snap_turn_enabled: true,
                seated_mode: false,
            },
            acceleration_mps2: 5.0,
            max_speed_mps: 3.0,
        };
        adapter.apply_locomotion_profile(&profile);
        assert!(adapter.locomotion_profile.is_some());
    }
}
