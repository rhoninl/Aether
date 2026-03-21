//! Debug overlay data formatting.
//!
//! Collects metrics and formats them into text lines for rendering.

use aether_input::openxr_tracking::TrackingSnapshot;

/// Debug data to display in the overlay panel.
#[derive(Debug, Clone)]
pub struct DebugOverlayData {
    pub fps: f32,
    pub frame_time_ms: f32,
    pub head_position: [f32; 3],
    pub head_yaw_deg: f32,
    pub head_pitch_deg: f32,
    pub left_controller_pos: [f32; 3],
    pub right_controller_pos: [f32; 3],
    pub left_trigger: f32,
    pub right_trigger: f32,
    pub tracking_confidence: String,
    pub session_state: String,
    pub frame_count: u64,
}

impl Default for DebugOverlayData {
    fn default() -> Self {
        Self {
            fps: 0.0,
            frame_time_ms: 0.0,
            head_position: [0.0; 3],
            head_yaw_deg: 0.0,
            head_pitch_deg: 0.0,
            left_controller_pos: [0.0; 3],
            right_controller_pos: [0.0; 3],
            left_trigger: 0.0,
            right_trigger: 0.0,
            tracking_confidence: "None".to_string(),
            session_state: "Idle".to_string(),
            frame_count: 0,
        }
    }
}

impl DebugOverlayData {
    /// Build overlay data from a tracking snapshot and frame metrics.
    pub fn from_snapshot(
        snapshot: &TrackingSnapshot,
        fps: f32,
        frame_time_ms: f32,
        session_state: &str,
        frame_count: u64,
    ) -> Self {
        let head_pos = snapshot.head_pose.position;
        let head_rot = snapshot.head_pose.rotation;

        // Extract yaw/pitch from quaternion
        let (yaw, pitch) = quaternion_to_yaw_pitch(head_rot);

        let left_pos = snapshot.left_controller.grip_pose.position;
        let right_pos = snapshot.right_controller.grip_pose.position;

        Self {
            fps,
            frame_time_ms,
            head_position: head_pos,
            head_yaw_deg: yaw.to_degrees(),
            head_pitch_deg: pitch.to_degrees(),
            left_controller_pos: left_pos,
            right_controller_pos: right_pos,
            left_trigger: snapshot.left_controller.analog.trigger,
            right_trigger: snapshot.right_controller.analog.trigger,
            tracking_confidence: format!("{:?}", snapshot.head_confidence),
            session_state: session_state.to_string(),
            frame_count,
        }
    }

    /// Format the debug data into text lines for rendering.
    pub fn format_lines(&self) -> Vec<String> {
        vec![
            format!(
                "FPS: {:.0}  Frame: {:.1}ms  State: {}  #{}",
                self.fps, self.frame_time_ms, self.session_state, self.frame_count
            ),
            format!(
                "Head: ({:.2}, {:.2}, {:.2})  Yaw: {:.1}  Pitch: {:.1}",
                self.head_position[0],
                self.head_position[1],
                self.head_position[2],
                self.head_yaw_deg,
                self.head_pitch_deg
            ),
            format!(
                "L-Ctrl: ({:.2}, {:.2}, {:.2})  R-Ctrl: ({:.2}, {:.2}, {:.2})",
                self.left_controller_pos[0],
                self.left_controller_pos[1],
                self.left_controller_pos[2],
                self.right_controller_pos[0],
                self.right_controller_pos[1],
                self.right_controller_pos[2],
            ),
            format!(
                "Confidence: {}  L-Trig: {:.2}  R-Trig: {:.2}",
                self.tracking_confidence, self.left_trigger, self.right_trigger
            ),
        ]
    }
}

/// Extract yaw and pitch from a quaternion [x, y, z, w].
fn quaternion_to_yaw_pitch(q: [f32; 4]) -> (f32, f32) {
    let [x, y, z, w] = q;
    let yaw = (2.0 * (w * y + x * z)).atan2(1.0 - 2.0 * (y * y + x * x));
    let sin_pitch = 2.0 * (w * x - z * y);
    let pitch = if sin_pitch.abs() >= 1.0 {
        std::f32::consts::FRAC_PI_2.copysign(sin_pitch)
    } else {
        sin_pitch.asin()
    };
    (yaw, pitch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_input::actions::Pose3;
    use aether_input::openxr_tracking::{
        ControllerAnalog, ControllerButtons, ControllerState, Hand, TrackingConfidence,
    };

    fn default_pose() -> Pose3 {
        Pose3 {
            position: [0.0, 1.7, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            linear_velocity: [0.0; 3],
            angular_velocity: [0.0; 3],
        }
    }

    fn default_controller(hand: Hand) -> ControllerState {
        ControllerState {
            hand,
            tracking_confidence: TrackingConfidence::High,
            connected: true,
            grip_pose: default_pose(),
            aim_pose: default_pose(),
            analog: ControllerAnalog {
                trigger: 0.0,
                grip: 0.0,
                thumbstick_x: 0.0,
                thumbstick_y: 0.0,
            },
            buttons: ControllerButtons::default(),
        }
    }

    fn test_snapshot() -> TrackingSnapshot {
        TrackingSnapshot {
            timestamp_ns: 1000,
            predicted_display_time_ns: 2000,
            head_pose: default_pose(),
            head_confidence: TrackingConfidence::High,
            left_controller: default_controller(Hand::Left),
            right_controller: default_controller(Hand::Right),
            left_hand: None,
            right_hand: None,
        }
    }

    #[test]
    fn default_overlay_data() {
        let data = DebugOverlayData::default();
        assert_eq!(data.fps, 0.0);
        assert_eq!(data.session_state, "Idle");
        assert_eq!(data.frame_count, 0);
    }

    #[test]
    fn from_snapshot_basic() {
        let snap = test_snapshot();
        let data = DebugOverlayData::from_snapshot(&snap, 90.0, 11.1, "Running", 42);
        assert_eq!(data.fps, 90.0);
        assert_eq!(data.frame_time_ms, 11.1);
        assert_eq!(data.session_state, "Running");
        assert_eq!(data.frame_count, 42);
        assert_eq!(data.head_position, [0.0, 1.7, 0.0]);
    }

    #[test]
    fn from_snapshot_controller_positions() {
        let mut snap = test_snapshot();
        snap.left_controller.grip_pose.position = [-0.2, 1.4, -0.3];
        snap.right_controller.grip_pose.position = [0.2, 1.4, -0.3];
        let data = DebugOverlayData::from_snapshot(&snap, 60.0, 16.6, "Running", 1);
        assert_eq!(data.left_controller_pos, [-0.2, 1.4, -0.3]);
        assert_eq!(data.right_controller_pos, [0.2, 1.4, -0.3]);
    }

    #[test]
    fn from_snapshot_trigger_values() {
        let mut snap = test_snapshot();
        snap.left_controller.analog.trigger = 0.75;
        snap.right_controller.analog.trigger = 1.0;
        let data = DebugOverlayData::from_snapshot(&snap, 90.0, 11.0, "Running", 1);
        assert_eq!(data.left_trigger, 0.75);
        assert_eq!(data.right_trigger, 1.0);
    }

    #[test]
    fn format_lines_count() {
        let data = DebugOverlayData::default();
        let lines = data.format_lines();
        assert_eq!(lines.len(), 4);
    }

    #[test]
    fn format_lines_contains_fps() {
        let mut data = DebugOverlayData::default();
        data.fps = 120.0;
        let lines = data.format_lines();
        assert!(lines[0].contains("120"));
    }

    #[test]
    fn format_lines_contains_state() {
        let mut data = DebugOverlayData::default();
        data.session_state = "Focused".to_string();
        let lines = data.format_lines();
        assert!(lines[0].contains("Focused"));
    }

    #[test]
    fn format_lines_contains_head_position() {
        let mut data = DebugOverlayData::default();
        data.head_position = [1.23, 4.56, 7.89];
        let lines = data.format_lines();
        assert!(lines[1].contains("1.23"));
        assert!(lines[1].contains("4.56"));
        assert!(lines[1].contains("7.89"));
    }

    #[test]
    fn format_lines_contains_controller_positions() {
        let mut data = DebugOverlayData::default();
        data.left_controller_pos = [-0.50, 1.40, -0.30];
        let lines = data.format_lines();
        assert!(lines[2].contains("-0.50"));
        assert!(lines[2].contains("L-Ctrl"));
    }

    #[test]
    fn format_lines_contains_confidence() {
        let mut data = DebugOverlayData::default();
        data.tracking_confidence = "High".to_string();
        let lines = data.format_lines();
        assert!(lines[3].contains("High"));
    }

    #[test]
    fn quaternion_identity_gives_zero() {
        let (yaw, pitch) = quaternion_to_yaw_pitch([0.0, 0.0, 0.0, 1.0]);
        assert!(yaw.abs() < 0.001);
        assert!(pitch.abs() < 0.001);
    }

    #[test]
    fn quaternion_90_yaw() {
        // 90 degrees around Y axis: q = [0, sin(45), 0, cos(45)]
        let s = std::f32::consts::FRAC_PI_4.sin();
        let c = std::f32::consts::FRAC_PI_4.cos();
        let (yaw, _) = quaternion_to_yaw_pitch([0.0, s, 0.0, c]);
        assert!((yaw - std::f32::consts::FRAC_PI_2).abs() < 0.01);
    }
}
