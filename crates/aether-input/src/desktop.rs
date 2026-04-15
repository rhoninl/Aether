//! Pure-data keyboard and mouse input frame for non-VR (desktop) games.
//!
//! This module is deliberately runtime-free and serde-free. It exposes a small
//! value type that games can populate each tick from whatever OS-level polling
//! source they prefer (minifb, winit, SDL, etc.). `aether-input` itself does
//! not pull in any desktop windowing dependency — the game wires the actual
//! polling and just feeds keycodes and mouse state into `DesktopInputFrame`.
//!
//! Enable with the `desktop` cargo feature.

use std::collections::HashSet;

/// Logical mouse button. The numeric value is used as a bit position inside
/// [`DesktopInputFrame::mouse_buttons`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum MouseButton {
    #[default]
    Left,
    Right,
    Middle,
    X1,
    X2,
}

impl MouseButton {
    /// Bit index assigned to this button inside the `mouse_buttons` bitmask.
    #[inline]
    pub const fn bit(self) -> u32 {
        match self {
            MouseButton::Left => 0,
            MouseButton::Right => 1,
            MouseButton::Middle => 2,
            MouseButton::X1 => 3,
            MouseButton::X2 => 4,
        }
    }

    /// Bitmask for this button: `1 << bit`.
    #[inline]
    pub const fn mask(self) -> u32 {
        1u32 << self.bit()
    }
}

/// Snapshot of keyboard and mouse state for one frame.
///
/// All fields are plain data. `keys_pressed` holds whatever raw keycode
/// convention the game decides on (e.g. `minifb::Key as u32`). The framework
/// does not interpret them.
#[derive(Clone, Debug, Default)]
pub struct DesktopInputFrame {
    pub keys_pressed: HashSet<u32>,
    pub mouse_pos: (f32, f32),
    pub mouse_delta: (f32, f32),
    pub mouse_buttons: u32,
    pub scroll: f32,
    pub cursor_locked: bool,
}

impl DesktopInputFrame {
    /// Create an empty frame: no keys, zero mouse position/delta, no buttons.
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder: mark `key` as pressed.
    pub fn with_key(mut self, key: u32) -> Self {
        self.keys_pressed.insert(key);
        self
    }

    /// Builder: set mouse position and delta for this frame.
    pub fn with_mouse(mut self, pos: (f32, f32), delta: (f32, f32)) -> Self {
        self.mouse_pos = pos;
        self.mouse_delta = delta;
        self
    }

    /// Builder: mark `btn` as held down.
    pub fn with_button(mut self, btn: MouseButton) -> Self {
        self.mouse_buttons |= btn.mask();
        self
    }

    /// Builder: set whether the cursor is locked / captured this frame.
    pub fn with_cursor_locked(mut self, locked: bool) -> Self {
        self.cursor_locked = locked;
        self
    }

    /// True if `key` is in `keys_pressed`.
    pub fn key_pressed(&self, key: u32) -> bool {
        self.keys_pressed.contains(&key)
    }

    /// True if `btn`'s bit is set in `mouse_buttons`.
    pub fn button_down(&self, btn: MouseButton) -> bool {
        (self.mouse_buttons & btn.mask()) != 0
    }

    /// Accessor for the mouse delta tuple.
    pub fn mouse_delta(&self) -> (f32, f32) {
        self.mouse_delta
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_frame_is_empty() {
        let frame = DesktopInputFrame::default();
        assert!(frame.keys_pressed.is_empty());
        assert_eq!(frame.mouse_pos, (0.0, 0.0));
        assert_eq!(frame.mouse_delta, (0.0, 0.0));
        assert_eq!(frame.mouse_buttons, 0);
        assert_eq!(frame.scroll, 0.0);
        assert!(!frame.cursor_locked);
    }

    #[test]
    fn with_key_round_trips_via_key_pressed() {
        let frame = DesktopInputFrame::new().with_key(42);
        assert!(frame.key_pressed(42));
        assert!(!frame.key_pressed(43));
    }

    #[test]
    fn with_key_twice_records_both() {
        let frame = DesktopInputFrame::new().with_key(1).with_key(2);
        assert!(frame.key_pressed(1));
        assert!(frame.key_pressed(2));
        assert_eq!(frame.keys_pressed.len(), 2);
    }

    #[test]
    fn with_button_round_trips_for_every_button() {
        for btn in [
            MouseButton::Left,
            MouseButton::Right,
            MouseButton::Middle,
            MouseButton::X1,
            MouseButton::X2,
        ] {
            let frame = DesktopInputFrame::new().with_button(btn);
            assert!(frame.button_down(btn), "button {:?} should be down", btn);
        }
    }

    #[test]
    fn button_bits_are_distinct() {
        let frame = DesktopInputFrame::new()
            .with_button(MouseButton::Left)
            .with_button(MouseButton::X2);
        assert!(frame.button_down(MouseButton::Left));
        assert!(frame.button_down(MouseButton::X2));
        assert!(!frame.button_down(MouseButton::Right));
        assert!(!frame.button_down(MouseButton::Middle));
        assert!(!frame.button_down(MouseButton::X1));
    }

    #[test]
    fn with_mouse_stores_pos_and_delta() {
        let frame = DesktopInputFrame::new().with_mouse((10.0, 20.0), (1.5, -2.5));
        assert_eq!(frame.mouse_pos, (10.0, 20.0));
        assert_eq!(frame.mouse_delta(), (1.5, -2.5));
    }

    #[test]
    fn with_cursor_locked_sets_flag() {
        let frame = DesktopInputFrame::new().with_cursor_locked(true);
        assert!(frame.cursor_locked);
        let unlocked = DesktopInputFrame::new().with_cursor_locked(false);
        assert!(!unlocked.cursor_locked);
    }

    #[test]
    fn frames_with_different_keys_differ() {
        let a = DesktopInputFrame::new().with_key(1);
        let b = DesktopInputFrame::new().with_key(2);
        assert_ne!(a.keys_pressed, b.keys_pressed);
    }
}
