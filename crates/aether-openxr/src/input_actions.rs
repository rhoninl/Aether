//! OpenXR input action management.
//!
//! Creates action sets and reads controller/hand tracking data,
//! converting to `TrackingSnapshot` from `aether-input`.

use aether_input::actions::Pose3;
use aether_input::openxr_tracking::{
    ControllerAnalog, ControllerButtons, ControllerState, Hand, TrackingConfidence,
    TrackingSnapshot,
};

fn zero_pose() -> Pose3 {
    Pose3 {
        position: [0.0; 3],
        rotation: [0.0, 0.0, 0.0, 1.0],
        linear_velocity: [0.0; 3],
        angular_velocity: [0.0; 3],
    }
}

/// Manages OpenXR input actions for controllers and hand tracking.
///
/// Placeholder: will wrap `openxr::ActionSet` and individual actions.
pub struct XrInputActions {
    frame_count: u64,
}

impl Default for XrInputActions {
    fn default() -> Self {
        Self::new()
    }
}

impl XrInputActions {
    /// Create input actions.
    pub fn new() -> Self {
        Self { frame_count: 0 }
    }

    /// Sync actions and produce a tracking snapshot.
    ///
    /// In production, this calls `xrSyncActions` and reads pose/button data.
    /// Currently returns a default snapshot for testing.
    pub fn sync_and_snapshot(&mut self, predicted_time_ns: u64) -> TrackingSnapshot {
        self.frame_count += 1;

        TrackingSnapshot {
            timestamp_ns: predicted_time_ns,
            predicted_display_time_ns: predicted_time_ns,
            head_pose: zero_pose(),
            head_confidence: TrackingConfidence::High,
            left_controller: default_controller(Hand::Left),
            right_controller: default_controller(Hand::Right),
            left_hand: None,
            right_hand: None,
        }
    }

    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }
}

fn default_controller(hand: Hand) -> ControllerState {
    ControllerState {
        hand,
        tracking_confidence: TrackingConfidence::High,
        connected: true,
        grip_pose: zero_pose(),
        aim_pose: zero_pose(),
        analog: ControllerAnalog {
            trigger: 0.0,
            grip: 0.0,
            thumbstick_x: 0.0,
            thumbstick_y: 0.0,
        },
        buttons: ControllerButtons::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_input_actions() {
        let actions = XrInputActions::new();
        assert_eq!(actions.frame_count(), 0);
    }

    #[test]
    fn sync_produces_snapshot() {
        let mut actions = XrInputActions::new();
        let snapshot = actions.sync_and_snapshot(1000);
        assert_eq!(snapshot.timestamp_ns, 1000);
        assert_eq!(snapshot.head_confidence, TrackingConfidence::High);
        assert!(snapshot.left_controller.connected);
        assert!(snapshot.right_controller.connected);
    }

    #[test]
    fn sync_increments_frame_count() {
        let mut actions = XrInputActions::new();
        actions.sync_and_snapshot(1000);
        actions.sync_and_snapshot(2000);
        assert_eq!(actions.frame_count(), 2);
    }

    #[test]
    fn snapshot_controller_hands() {
        let mut actions = XrInputActions::new();
        let snapshot = actions.sync_and_snapshot(1000);
        assert_eq!(snapshot.left_controller.hand, Hand::Left);
        assert_eq!(snapshot.right_controller.hand, Hand::Right);
    }

    #[test]
    fn snapshot_no_hand_tracking() {
        let mut actions = XrInputActions::new();
        let snapshot = actions.sync_and_snapshot(1000);
        assert!(snapshot.left_hand.is_none());
        assert!(snapshot.right_hand.is_none());
    }
}
