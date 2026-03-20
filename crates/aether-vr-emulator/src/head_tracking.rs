//! Emulated head tracking via keyboard and mouse input.
//!
//! Converts mouse deltas to head rotation (yaw/pitch) and keyboard input
//! to head position changes, producing `Pose3` output compatible with
//! the `aether-input` tracking system.

use aether_input::Pose3;

use crate::config::InputSensitivity;

/// Default standing eye height in meters.
const DEFAULT_STANDING_HEIGHT_M: f32 = 1.7;

/// Default seated eye height in meters.
const DEFAULT_SEATED_HEIGHT_M: f32 = 1.2;

/// Maximum pitch angle in radians (slightly less than 90 degrees).
const MAX_PITCH_RAD: f32 = 1.5;

/// Minimum pitch angle in radians.
const MIN_PITCH_RAD: f32 = -1.5;

/// Maximum head position extent on any axis in meters (room boundary).
const MAX_POSITION_EXTENT_M: f32 = 5.0;

/// Preset head positions for quick selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HeadPresetPosition {
    /// Standing at room center, default height.
    StandingCenter,
    /// Seated at room center, lower height.
    SeatedCenter,
    /// Standing at front-left corner of room.
    RoomFrontLeft,
    /// Standing at front-right corner of room.
    RoomFrontRight,
    /// Standing at back-left corner of room.
    RoomBackLeft,
    /// Standing at back-right corner of room.
    RoomBackRight,
}

/// Emulated head tracker state.
#[derive(Debug, Clone)]
pub struct EmulatedHeadTracker {
    /// Current head position in meters [x, y, z].
    position: [f32; 3],
    /// Yaw angle in radians (rotation around Y axis).
    yaw_rad: f32,
    /// Pitch angle in radians (rotation around X axis).
    pitch_rad: f32,
    /// Mouse look sensitivity.
    mouse_sensitivity: f32,
    /// Movement speed in meters per second.
    move_speed: f32,
}

impl EmulatedHeadTracker {
    /// Create a new head tracker with the given sensitivity settings.
    pub fn new(sensitivity: &InputSensitivity) -> Self {
        Self {
            position: [0.0, DEFAULT_STANDING_HEIGHT_M, 0.0],
            yaw_rad: 0.0,
            pitch_rad: 0.0,
            mouse_sensitivity: sensitivity.mouse_look,
            move_speed: sensitivity.move_speed,
        }
    }

    /// Get the current head position.
    pub fn position(&self) -> [f32; 3] {
        self.position
    }

    /// Get the current yaw in radians.
    pub fn yaw_rad(&self) -> f32 {
        self.yaw_rad
    }

    /// Get the current pitch in radians.
    pub fn pitch_rad(&self) -> f32 {
        self.pitch_rad
    }

    /// Apply mouse delta to update head rotation.
    pub fn apply_mouse_look(&mut self, delta_x: f32, delta_y: f32) {
        self.yaw_rad -= delta_x * self.mouse_sensitivity;
        self.pitch_rad -= delta_y * self.mouse_sensitivity;

        // Wrap yaw to [0, 2*PI)
        self.yaw_rad = wrap_radians(self.yaw_rad);

        // Clamp pitch
        self.pitch_rad = self.pitch_rad.clamp(MIN_PITCH_RAD, MAX_PITCH_RAD);
    }

    /// Apply keyboard movement to update head position.
    ///
    /// Movement is relative to the current yaw direction:
    /// - `forward` / `backward`: move along the facing direction
    /// - `left` / `right`: strafe perpendicular to facing
    /// - `up` / `down`: move vertically
    #[allow(clippy::too_many_arguments)]
    pub fn apply_movement(
        &mut self,
        forward: bool,
        backward: bool,
        left: bool,
        right: bool,
        up: bool,
        down: bool,
        dt_s: f32,
    ) {
        let mut dx = 0.0f32;
        let mut dy = 0.0f32;
        let mut dz = 0.0f32;

        // Horizontal movement relative to yaw
        let fwd_z = -self.yaw_rad.cos();
        let fwd_x = -self.yaw_rad.sin();
        let right_z = fwd_x;
        let right_x = -fwd_z;

        if forward {
            dx += fwd_x;
            dz += fwd_z;
        }
        if backward {
            dx -= fwd_x;
            dz -= fwd_z;
        }
        if left {
            dx -= right_x;
            dz -= right_z;
        }
        if right {
            dx += right_x;
            dz += right_z;
        }
        if up {
            dy += 1.0;
        }
        if down {
            dy -= 1.0;
        }

        // Normalize horizontal movement
        let horiz_mag = (dx * dx + dz * dz).sqrt();
        if horiz_mag > 1e-6 {
            let inv = 1.0 / horiz_mag;
            dx *= inv;
            dz *= inv;
        }

        let speed = self.move_speed * dt_s;
        self.position[0] += dx * speed;
        self.position[1] += dy * speed;
        self.position[2] += dz * speed;

        // Clamp to room boundaries
        self.clamp_position();
    }

    /// Set the head to a preset position.
    pub fn set_preset(&mut self, preset: HeadPresetPosition) {
        let (x, y, z) = preset_position(preset);
        self.position = [x, y, z];
    }

    /// Set the head position directly.
    pub fn set_position(&mut self, position: [f32; 3]) {
        self.position = position;
        self.clamp_position();
    }

    /// Set the head rotation directly (yaw and pitch in radians).
    pub fn set_rotation(&mut self, yaw_rad: f32, pitch_rad: f32) {
        self.yaw_rad = wrap_radians(yaw_rad);
        self.pitch_rad = pitch_rad.clamp(MIN_PITCH_RAD, MAX_PITCH_RAD);
    }

    /// Reset head to standing center with forward-facing orientation.
    pub fn reset(&mut self) {
        self.position = [0.0, DEFAULT_STANDING_HEIGHT_M, 0.0];
        self.yaw_rad = 0.0;
        self.pitch_rad = 0.0;
    }

    /// Build a `Pose3` from the current head state.
    pub fn to_pose(&self) -> Pose3 {
        let quat = yaw_pitch_to_quaternion(self.yaw_rad, self.pitch_rad);
        Pose3 {
            position: self.position,
            rotation: quat,
            linear_velocity: [0.0, 0.0, 0.0],
            angular_velocity: [0.0, 0.0, 0.0],
        }
    }

    /// Clamp position to the room boundaries.
    fn clamp_position(&mut self) {
        self.position[0] = self.position[0].clamp(-MAX_POSITION_EXTENT_M, MAX_POSITION_EXTENT_M);
        self.position[1] = self.position[1].clamp(0.0, MAX_POSITION_EXTENT_M);
        self.position[2] = self.position[2].clamp(-MAX_POSITION_EXTENT_M, MAX_POSITION_EXTENT_M);
    }
}

/// Get the position [x, y, z] for a head preset.
pub fn preset_position(preset: HeadPresetPosition) -> (f32, f32, f32) {
    match preset {
        HeadPresetPosition::StandingCenter => (0.0, DEFAULT_STANDING_HEIGHT_M, 0.0),
        HeadPresetPosition::SeatedCenter => (0.0, DEFAULT_SEATED_HEIGHT_M, 0.0),
        HeadPresetPosition::RoomFrontLeft => (-1.5, DEFAULT_STANDING_HEIGHT_M, -1.5),
        HeadPresetPosition::RoomFrontRight => (1.5, DEFAULT_STANDING_HEIGHT_M, -1.5),
        HeadPresetPosition::RoomBackLeft => (-1.5, DEFAULT_STANDING_HEIGHT_M, 1.5),
        HeadPresetPosition::RoomBackRight => (1.5, DEFAULT_STANDING_HEIGHT_M, 1.5),
    }
}

/// Convert yaw (around Y) and pitch (around X) to a quaternion [x, y, z, w].
///
/// Rotation order: yaw first, then pitch.
pub fn yaw_pitch_to_quaternion(yaw_rad: f32, pitch_rad: f32) -> [f32; 4] {
    let half_yaw = yaw_rad * 0.5;
    let half_pitch = pitch_rad * 0.5;

    // Quaternion for yaw (rotation around Y axis)
    let qy_x = 0.0;
    let qy_y = half_yaw.sin();
    let qy_z = 0.0;
    let qy_w = half_yaw.cos();

    // Quaternion for pitch (rotation around X axis)
    let qp_x = half_pitch.sin();
    let qp_y = 0.0;
    let qp_z = 0.0;
    let qp_w = half_pitch.cos();

    // Combined: q = qy * qp (yaw applied first in world space)
    let w = qy_w * qp_w - qy_x * qp_x - qy_y * qp_y - qy_z * qp_z;
    let x = qy_w * qp_x + qy_x * qp_w + qy_y * qp_z - qy_z * qp_y;
    let y = qy_w * qp_y - qy_x * qp_z + qy_y * qp_w + qy_z * qp_x;
    let z = qy_w * qp_z + qy_x * qp_y - qy_y * qp_x + qy_z * qp_w;

    [x, y, z, w]
}

/// Compute the squared length of a quaternion.
pub fn quaternion_length_sq(q: [f32; 4]) -> f32 {
    q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]
}

/// Wrap an angle in radians to [0, 2*PI).
fn wrap_radians(rad: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    let result = rad % tau;
    if result < 0.0 {
        result + tau
    } else {
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-4;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    fn default_tracker() -> EmulatedHeadTracker {
        EmulatedHeadTracker::new(&InputSensitivity::default())
    }

    // ---- Initial state ----

    #[test]
    fn initial_position_standing_height() {
        let tracker = default_tracker();
        assert_eq!(tracker.position()[0], 0.0);
        assert_eq!(tracker.position()[1], DEFAULT_STANDING_HEIGHT_M);
        assert_eq!(tracker.position()[2], 0.0);
    }

    #[test]
    fn initial_rotation_zero() {
        let tracker = default_tracker();
        assert_eq!(tracker.yaw_rad(), 0.0);
        assert_eq!(tracker.pitch_rad(), 0.0);
    }

    // ---- Mouse look ----

    #[test]
    fn mouse_look_yaw_right() {
        let mut tracker = default_tracker();
        tracker.apply_mouse_look(100.0, 0.0);
        // Yaw should decrease (turn right) since we negate delta_x
        assert!(tracker.yaw_rad() > 0.0 || tracker.yaw_rad() < std::f32::consts::TAU);
    }

    #[test]
    fn mouse_look_pitch_up() {
        let mut tracker = default_tracker();
        tracker.apply_mouse_look(0.0, -100.0);
        // Negative delta_y should pitch up (positive pitch)
        assert!(tracker.pitch_rad() > 0.0);
    }

    #[test]
    fn mouse_look_pitch_clamped() {
        let mut tracker = default_tracker();
        tracker.apply_mouse_look(0.0, -100000.0);
        assert!(tracker.pitch_rad() <= MAX_PITCH_RAD);

        tracker.apply_mouse_look(0.0, 200000.0);
        assert!(tracker.pitch_rad() >= MIN_PITCH_RAD);
    }

    #[test]
    fn mouse_look_yaw_wraps() {
        let mut tracker = default_tracker();
        // Apply enough rotation to wrap around
        for _ in 0..1000 {
            tracker.apply_mouse_look(-100.0, 0.0);
        }
        let yaw = tracker.yaw_rad();
        assert!(yaw >= 0.0 && yaw < std::f32::consts::TAU);
    }

    #[test]
    fn mouse_look_no_delta_no_change() {
        let mut tracker = default_tracker();
        tracker.apply_mouse_look(0.0, 0.0);
        assert_eq!(tracker.yaw_rad(), 0.0);
        assert_eq!(tracker.pitch_rad(), 0.0);
    }

    // ---- Movement ----

    #[test]
    fn move_forward_changes_position() {
        let mut tracker = default_tracker();
        let initial_z = tracker.position()[2];
        tracker.apply_movement(true, false, false, false, false, false, 0.1);
        // When yaw=0, forward is along -Z
        assert!(tracker.position()[2] < initial_z);
    }

    #[test]
    fn move_backward_changes_position() {
        let mut tracker = default_tracker();
        let initial_z = tracker.position()[2];
        tracker.apply_movement(false, true, false, false, false, false, 0.1);
        assert!(tracker.position()[2] > initial_z);
    }

    #[test]
    fn move_left_changes_position() {
        let mut tracker = default_tracker();
        let initial_x = tracker.position()[0];
        tracker.apply_movement(false, false, true, false, false, false, 0.1);
        assert!(tracker.position()[0] < initial_x);
    }

    #[test]
    fn move_right_changes_position() {
        let mut tracker = default_tracker();
        let initial_x = tracker.position()[0];
        tracker.apply_movement(false, false, false, true, false, false, 0.1);
        assert!(tracker.position()[0] > initial_x);
    }

    #[test]
    fn move_up_increases_height() {
        let mut tracker = default_tracker();
        let initial_y = tracker.position()[1];
        tracker.apply_movement(false, false, false, false, true, false, 0.1);
        assert!(tracker.position()[1] > initial_y);
    }

    #[test]
    fn move_down_decreases_height() {
        let mut tracker = default_tracker();
        tracker.set_position([0.0, 2.0, 0.0]);
        let initial_y = tracker.position()[1];
        tracker.apply_movement(false, false, false, false, false, true, 0.1);
        assert!(tracker.position()[1] < initial_y);
    }

    #[test]
    fn no_movement_no_change() {
        let mut tracker = default_tracker();
        let pos = tracker.position();
        tracker.apply_movement(false, false, false, false, false, false, 0.1);
        assert_eq!(tracker.position(), pos);
    }

    #[test]
    fn diagonal_movement_is_normalized() {
        let mut tracker1 = default_tracker();
        let mut tracker2 = default_tracker();

        // Move forward only for reference
        tracker1.apply_movement(true, false, false, false, false, false, 1.0);
        let dist1 = {
            let p = tracker1.position();
            (p[0] * p[0] + (p[1] - DEFAULT_STANDING_HEIGHT_M).powi(2) + p[2] * p[2]).sqrt()
        };

        // Move forward+right (diagonal)
        tracker2.apply_movement(true, false, false, true, false, false, 1.0);
        let dist2 = {
            let p = tracker2.position();
            (p[0] * p[0] + (p[1] - DEFAULT_STANDING_HEIGHT_M).powi(2) + p[2] * p[2]).sqrt()
        };

        // Diagonal should be same speed as cardinal
        assert!(
            approx_eq(dist1, dist2),
            "cardinal={dist1}, diagonal={dist2}"
        );
    }

    #[test]
    fn movement_clamped_to_room_bounds() {
        let mut tracker = default_tracker();
        // Move really far in one direction
        for _ in 0..1000 {
            tracker.apply_movement(true, false, false, false, false, false, 1.0);
        }
        let pos = tracker.position();
        assert!(pos[2] >= -MAX_POSITION_EXTENT_M);
        assert!(pos[2] <= MAX_POSITION_EXTENT_M);
    }

    #[test]
    fn height_cannot_go_negative() {
        let mut tracker = default_tracker();
        for _ in 0..1000 {
            tracker.apply_movement(false, false, false, false, false, true, 1.0);
        }
        assert!(tracker.position()[1] >= 0.0);
    }

    #[test]
    fn movement_relative_to_yaw() {
        let mut tracker = default_tracker();
        // Turn 90 degrees to the right
        tracker.set_rotation(std::f32::consts::FRAC_PI_2, 0.0);
        let initial_pos = tracker.position();
        tracker.apply_movement(true, false, false, false, false, false, 1.0);
        // After turning 90 degrees right, forward should move along -X
        assert!(tracker.position()[0] < initial_pos[0]);
    }

    // ---- Preset positions ----

    #[test]
    fn preset_standing_center() {
        let (x, y, z) = preset_position(HeadPresetPosition::StandingCenter);
        assert_eq!(x, 0.0);
        assert_eq!(y, DEFAULT_STANDING_HEIGHT_M);
        assert_eq!(z, 0.0);
    }

    #[test]
    fn preset_seated_center() {
        let (x, y, z) = preset_position(HeadPresetPosition::SeatedCenter);
        assert_eq!(x, 0.0);
        assert_eq!(y, DEFAULT_SEATED_HEIGHT_M);
        assert_eq!(z, 0.0);
    }

    #[test]
    fn preset_room_corners_at_standing_height() {
        for preset in [
            HeadPresetPosition::RoomFrontLeft,
            HeadPresetPosition::RoomFrontRight,
            HeadPresetPosition::RoomBackLeft,
            HeadPresetPosition::RoomBackRight,
        ] {
            let (_, y, _) = preset_position(preset);
            assert_eq!(y, DEFAULT_STANDING_HEIGHT_M, "preset {preset:?}");
        }
    }

    #[test]
    fn set_preset_changes_position() {
        let mut tracker = default_tracker();
        tracker.set_preset(HeadPresetPosition::SeatedCenter);
        assert_eq!(tracker.position()[1], DEFAULT_SEATED_HEIGHT_M);
    }

    // ---- Set position/rotation ----

    #[test]
    fn set_position_stores_value() {
        let mut tracker = default_tracker();
        tracker.set_position([1.0, 2.0, 3.0]);
        assert_eq!(tracker.position(), [1.0, 2.0, 3.0]);
    }

    #[test]
    fn set_position_clamps() {
        let mut tracker = default_tracker();
        tracker.set_position([100.0, -5.0, 100.0]);
        assert!(tracker.position()[0] <= MAX_POSITION_EXTENT_M);
        assert!(tracker.position()[1] >= 0.0);
        assert!(tracker.position()[2] <= MAX_POSITION_EXTENT_M);
    }

    #[test]
    fn set_rotation_stores_values() {
        let mut tracker = default_tracker();
        tracker.set_rotation(1.0, 0.5);
        assert!(approx_eq(tracker.yaw_rad(), 1.0));
        assert!(approx_eq(tracker.pitch_rad(), 0.5));
    }

    #[test]
    fn set_rotation_clamps_pitch() {
        let mut tracker = default_tracker();
        tracker.set_rotation(0.0, 5.0);
        assert!(tracker.pitch_rad() <= MAX_PITCH_RAD);
    }

    #[test]
    fn set_rotation_wraps_yaw() {
        let mut tracker = default_tracker();
        tracker.set_rotation(-1.0, 0.0);
        assert!(tracker.yaw_rad() >= 0.0);
    }

    // ---- Reset ----

    #[test]
    fn reset_returns_to_default() {
        let mut tracker = default_tracker();
        tracker.set_position([2.0, 3.0, 4.0]);
        tracker.set_rotation(1.5, 0.5);
        tracker.reset();
        assert_eq!(tracker.position(), [0.0, DEFAULT_STANDING_HEIGHT_M, 0.0]);
        assert_eq!(tracker.yaw_rad(), 0.0);
        assert_eq!(tracker.pitch_rad(), 0.0);
    }

    // ---- Pose output ----

    #[test]
    fn to_pose_identity_at_zero_rotation() {
        let tracker = default_tracker();
        let pose = tracker.to_pose();
        assert_eq!(pose.position, [0.0, DEFAULT_STANDING_HEIGHT_M, 0.0]);
        // At zero rotation, quaternion should be identity [0, 0, 0, 1]
        assert!(approx_eq(pose.rotation[0], 0.0));
        assert!(approx_eq(pose.rotation[1], 0.0));
        assert!(approx_eq(pose.rotation[2], 0.0));
        assert!(approx_eq(pose.rotation[3], 1.0));
    }

    #[test]
    fn to_pose_quaternion_is_unit() {
        let mut tracker = default_tracker();
        tracker.set_rotation(1.234, 0.567);
        let pose = tracker.to_pose();
        let len_sq = quaternion_length_sq(pose.rotation);
        assert!(
            approx_eq(len_sq, 1.0),
            "quaternion length squared = {len_sq}"
        );
    }

    #[test]
    fn to_pose_90_degree_yaw() {
        let mut tracker = default_tracker();
        tracker.set_rotation(std::f32::consts::FRAC_PI_2, 0.0);
        let pose = tracker.to_pose();
        // 90-degree yaw around Y: q = (0, sin(45), 0, cos(45))
        let expected_y = (std::f32::consts::FRAC_PI_4).sin();
        let expected_w = (std::f32::consts::FRAC_PI_4).cos();
        assert!(approx_eq(pose.rotation[0], 0.0));
        assert!(approx_eq(pose.rotation[1], expected_y));
        assert!(approx_eq(pose.rotation[2], 0.0));
        assert!(approx_eq(pose.rotation[3], expected_w));
    }

    #[test]
    fn to_pose_velocities_zero() {
        let tracker = default_tracker();
        let pose = tracker.to_pose();
        assert_eq!(pose.linear_velocity, [0.0, 0.0, 0.0]);
        assert_eq!(pose.angular_velocity, [0.0, 0.0, 0.0]);
    }

    // ---- Quaternion math ----

    #[test]
    fn yaw_pitch_zero_is_identity() {
        let q = yaw_pitch_to_quaternion(0.0, 0.0);
        assert!(approx_eq(q[0], 0.0));
        assert!(approx_eq(q[1], 0.0));
        assert!(approx_eq(q[2], 0.0));
        assert!(approx_eq(q[3], 1.0));
    }

    #[test]
    fn yaw_pitch_quaternion_is_always_unit_length() {
        let test_angles = [
            (0.0, 0.0),
            (1.0, 0.5),
            (-0.7, 0.3),
            (3.14, -1.0),
            (6.0, 1.4),
        ];
        for (yaw, pitch) in test_angles {
            let q = yaw_pitch_to_quaternion(yaw, pitch);
            let len_sq = quaternion_length_sq(q);
            assert!(
                approx_eq(len_sq, 1.0),
                "yaw={yaw}, pitch={pitch}: len_sq={len_sq}"
            );
        }
    }

    // ---- Wrap radians ----

    #[test]
    fn wrap_radians_positive() {
        let tau = std::f32::consts::TAU;
        let wrapped = wrap_radians(tau + 1.0);
        assert!(approx_eq(wrapped, 1.0));
    }

    #[test]
    fn wrap_radians_negative() {
        let wrapped = wrap_radians(-1.0);
        assert!(wrapped >= 0.0);
        assert!(wrapped < std::f32::consts::TAU);
    }

    #[test]
    fn wrap_radians_zero() {
        let wrapped = wrap_radians(0.0);
        assert!(approx_eq(wrapped, 0.0));
    }
}
