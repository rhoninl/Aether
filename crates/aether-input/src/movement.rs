//! Locomotion movement primitives: smooth movement, teleport, snap turn, smooth turn.
//!
//! All functions are pure computations with no side effects.

/// Result of a teleport attempt.
#[derive(Debug, Clone, PartialEq)]
pub enum TeleportResult {
    /// Teleport succeeded; contains the target position.
    Success { position: [f32; 3] },
    /// Teleport failed because the target is beyond max distance.
    OutOfRange {
        distance: f32,
        max_distance: f32,
    },
    /// Teleport target is invalid (e.g., blocked by geometry).
    InvalidTarget,
}

/// Compute a smooth movement position delta.
///
/// # Arguments
/// * `direction` - Normalized movement direction [x, y, z]
/// * `current_speed` - Current movement speed in m/s
/// * `acceleration` - Acceleration in m/s^2
/// * `max_speed` - Maximum speed in m/s
/// * `dt_s` - Delta time in seconds
///
/// # Returns
/// (position_delta [f32; 3], new_speed f32)
pub fn compute_smooth_move(
    direction: [f32; 3],
    current_speed: f32,
    acceleration: f32,
    max_speed: f32,
    dt_s: f32,
) -> ([f32; 3], f32) {
    let dir_magnitude = (direction[0] * direction[0]
        + direction[1] * direction[1]
        + direction[2] * direction[2])
    .sqrt();

    if dir_magnitude < 1e-6 {
        // No input: decelerate to zero
        let decel_speed = (current_speed - acceleration * dt_s).max(0.0);
        return ([0.0, 0.0, 0.0], decel_speed);
    }

    // Normalize direction
    let inv_mag = 1.0 / dir_magnitude;
    let norm_dir = [
        direction[0] * inv_mag,
        direction[1] * inv_mag,
        direction[2] * inv_mag,
    ];

    // Accelerate toward max speed
    let new_speed = (current_speed + acceleration * dt_s).min(max_speed);
    let displacement = new_speed * dt_s;

    let delta = [
        norm_dir[0] * displacement,
        norm_dir[1] * displacement,
        norm_dir[2] * displacement,
    ];

    (delta, new_speed)
}

/// Validate and compute a teleport.
///
/// # Arguments
/// * `origin` - Current position [x, y, z]
/// * `target` - Target position [x, y, z]
/// * `max_distance` - Maximum allowed teleport distance in meters
///
/// # Returns
/// `TeleportResult` indicating success or failure.
pub fn compute_teleport(
    origin: [f32; 3],
    target: [f32; 3],
    max_distance: f32,
) -> TeleportResult {
    let dx = target[0] - origin[0];
    let dy = target[1] - origin[1];
    let dz = target[2] - origin[2];
    let distance = (dx * dx + dy * dy + dz * dz).sqrt();

    if distance > max_distance {
        return TeleportResult::OutOfRange {
            distance,
            max_distance,
        };
    }

    TeleportResult::Success { position: target }
}

/// Compute a snap turn: rotate yaw by a fixed step.
///
/// # Arguments
/// * `current_yaw_deg` - Current yaw in degrees
/// * `step_deg` - Step size in degrees (positive = right, negative = left)
///
/// # Returns
/// New yaw in degrees, wrapped to [0, 360).
pub fn compute_snap_turn(current_yaw_deg: f32, step_deg: f32) -> f32 {
    let new_yaw = current_yaw_deg + step_deg;
    wrap_degrees(new_yaw)
}

/// Compute smooth turn: rotate yaw continuously.
///
/// # Arguments
/// * `current_yaw_deg` - Current yaw in degrees
/// * `speed_deg_per_s` - Rotation speed in degrees per second (positive = right)
/// * `dt_s` - Delta time in seconds
///
/// # Returns
/// New yaw in degrees, wrapped to [0, 360).
pub fn compute_smooth_turn(current_yaw_deg: f32, speed_deg_per_s: f32, dt_s: f32) -> f32 {
    let new_yaw = current_yaw_deg + speed_deg_per_s * dt_s;
    wrap_degrees(new_yaw)
}

/// Wrap degrees to [0, 360) range.
fn wrap_degrees(deg: f32) -> f32 {
    let result = deg % 360.0;
    if result < 0.0 {
        result + 360.0
    } else {
        result
    }
}

/// Compute a movement direction vector from WASD-style input flags.
///
/// Uses a right-handed coordinate system where:
/// - +X is right
/// - +Y is up
/// - -Z is forward
///
/// The result is normalized if non-zero.
pub fn direction_from_keys(forward: bool, backward: bool, left: bool, right: bool) -> [f32; 3] {
    let mut x = 0.0f32;
    let mut z = 0.0f32;

    if forward {
        z -= 1.0;
    }
    if backward {
        z += 1.0;
    }
    if left {
        x -= 1.0;
    }
    if right {
        x += 1.0;
    }

    let mag = (x * x + z * z).sqrt();
    if mag < 1e-6 {
        return [0.0, 0.0, 0.0];
    }

    let inv = 1.0 / mag;
    [x * inv, 0.0, z * inv]
}

/// Rotate a direction vector by a yaw angle (around Y axis).
///
/// # Arguments
/// * `direction` - Input direction [x, y, z]
/// * `yaw_deg` - Yaw angle in degrees
///
/// # Returns
/// Rotated direction [x, y, z]
pub fn rotate_direction_by_yaw(direction: [f32; 3], yaw_deg: f32) -> [f32; 3] {
    let yaw_rad = yaw_deg.to_radians();
    let cos_y = yaw_rad.cos();
    let sin_y = yaw_rad.sin();

    [
        direction[0] * cos_y + direction[2] * sin_y,
        direction[1],
        -direction[0] * sin_y + direction[2] * cos_y,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-4;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    // ---- Smooth movement tests ----

    #[test]
    fn smooth_move_forward() {
        let dir = [0.0, 0.0, -1.0]; // Forward
        let (delta, speed) = compute_smooth_move(dir, 0.0, 5.0, 3.0, 0.1);
        // After 0.1s with 5 m/s^2 accel from 0: speed = 0.5
        // displacement = 0.5 * 0.1 = 0.05
        assert!(approx_eq(speed, 0.5), "speed={speed}");
        assert!(approx_eq(delta[2], -0.05), "delta_z={}", delta[2]);
        assert!(approx_eq(delta[0], 0.0));
        assert!(approx_eq(delta[1], 0.0));
    }

    #[test]
    fn smooth_move_caps_at_max_speed() {
        let dir = [1.0, 0.0, 0.0];
        let (_, speed) = compute_smooth_move(dir, 2.8, 5.0, 3.0, 1.0);
        assert!(approx_eq(speed, 3.0), "speed={speed}");
    }

    #[test]
    fn smooth_move_no_input_decelerates() {
        let dir = [0.0, 0.0, 0.0];
        let (delta, speed) = compute_smooth_move(dir, 2.0, 5.0, 3.0, 0.1);
        assert!(speed < 2.0, "speed={speed}");
        assert!(approx_eq(delta[0], 0.0));
        assert!(approx_eq(delta[1], 0.0));
        assert!(approx_eq(delta[2], 0.0));
    }

    #[test]
    fn smooth_move_decel_does_not_go_negative() {
        let dir = [0.0, 0.0, 0.0];
        let (_, speed) = compute_smooth_move(dir, 0.1, 5.0, 3.0, 1.0);
        assert!(approx_eq(speed, 0.0), "speed={speed}");
    }

    #[test]
    fn smooth_move_diagonal_is_normalized() {
        let dir = [1.0, 0.0, -1.0]; // Diagonal
        let (delta, _) = compute_smooth_move(dir, 1.0, 5.0, 3.0, 0.1);
        // Diagonal should not be faster than cardinal
        let mag = (delta[0] * delta[0] + delta[2] * delta[2]).sqrt();
        let (single_delta, _) = compute_smooth_move([1.0, 0.0, 0.0], 1.0, 5.0, 3.0, 0.1);
        let single_mag = single_delta[0].abs();
        assert!(
            approx_eq(mag, single_mag),
            "diagonal={mag}, cardinal={single_mag}"
        );
    }

    // ---- Teleport tests ----

    #[test]
    fn teleport_within_range_succeeds() {
        let result = compute_teleport([0.0, 0.0, 0.0], [3.0, 0.0, 4.0], 10.0);
        assert_eq!(
            result,
            TeleportResult::Success {
                position: [3.0, 0.0, 4.0]
            }
        );
    }

    #[test]
    fn teleport_beyond_range_fails() {
        let result = compute_teleport([0.0, 0.0, 0.0], [3.0, 0.0, 4.0], 4.0);
        match result {
            TeleportResult::OutOfRange {
                distance,
                max_distance,
            } => {
                assert!(approx_eq(distance, 5.0), "distance={distance}");
                assert!(approx_eq(max_distance, 4.0));
            }
            other => panic!("Expected OutOfRange, got {other:?}"),
        }
    }

    #[test]
    fn teleport_same_position_succeeds() {
        let result = compute_teleport([1.0, 2.0, 3.0], [1.0, 2.0, 3.0], 1.0);
        assert_eq!(
            result,
            TeleportResult::Success {
                position: [1.0, 2.0, 3.0]
            }
        );
    }

    #[test]
    fn teleport_at_exact_max_range_succeeds() {
        // distance = 5.0
        let result = compute_teleport([0.0, 0.0, 0.0], [3.0, 0.0, 4.0], 5.0);
        assert_eq!(
            result,
            TeleportResult::Success {
                position: [3.0, 0.0, 4.0]
            }
        );
    }

    // ---- Snap turn tests ----

    #[test]
    fn snap_turn_right() {
        let yaw = compute_snap_turn(0.0, 30.0);
        assert!(approx_eq(yaw, 30.0), "yaw={yaw}");
    }

    #[test]
    fn snap_turn_left() {
        let yaw = compute_snap_turn(0.0, -30.0);
        assert!(approx_eq(yaw, 330.0), "yaw={yaw}");
    }

    #[test]
    fn snap_turn_wraps_at_360() {
        let yaw = compute_snap_turn(350.0, 30.0);
        assert!(approx_eq(yaw, 20.0), "yaw={yaw}");
    }

    #[test]
    fn snap_turn_multiple_wraps() {
        let yaw = compute_snap_turn(0.0, 720.0);
        assert!(approx_eq(yaw, 0.0), "yaw={yaw}");
    }

    // ---- Smooth turn tests ----

    #[test]
    fn smooth_turn_right() {
        let yaw = compute_smooth_turn(0.0, 90.0, 1.0);
        assert!(approx_eq(yaw, 90.0), "yaw={yaw}");
    }

    #[test]
    fn smooth_turn_with_dt() {
        let yaw = compute_smooth_turn(0.0, 180.0, 0.5);
        assert!(approx_eq(yaw, 90.0), "yaw={yaw}");
    }

    #[test]
    fn smooth_turn_wraps() {
        let yaw = compute_smooth_turn(350.0, 100.0, 1.0);
        assert!(approx_eq(yaw, 90.0), "yaw={yaw}");
    }

    #[test]
    fn smooth_turn_negative_speed() {
        let yaw = compute_smooth_turn(30.0, -60.0, 1.0);
        assert!(approx_eq(yaw, 330.0), "yaw={yaw}");
    }

    // ---- Direction from keys tests ----

    #[test]
    fn direction_forward_only() {
        let dir = direction_from_keys(true, false, false, false);
        assert!(approx_eq(dir[0], 0.0));
        assert!(approx_eq(dir[1], 0.0));
        assert!(approx_eq(dir[2], -1.0));
    }

    #[test]
    fn direction_backward_only() {
        let dir = direction_from_keys(false, true, false, false);
        assert!(approx_eq(dir[2], 1.0));
    }

    #[test]
    fn direction_diagonal_is_normalized() {
        let dir = direction_from_keys(true, false, true, false);
        let mag = (dir[0] * dir[0] + dir[2] * dir[2]).sqrt();
        assert!(approx_eq(mag, 1.0), "mag={mag}");
    }

    #[test]
    fn direction_no_keys_is_zero() {
        let dir = direction_from_keys(false, false, false, false);
        assert!(approx_eq(dir[0], 0.0));
        assert!(approx_eq(dir[1], 0.0));
        assert!(approx_eq(dir[2], 0.0));
    }

    #[test]
    fn direction_opposing_keys_cancel() {
        let dir = direction_from_keys(true, true, false, false);
        assert!(approx_eq(dir[0], 0.0));
        assert!(approx_eq(dir[2], 0.0));
    }

    // ---- Rotate direction tests ----

    #[test]
    fn rotate_direction_zero_yaw_unchanged() {
        let dir = [0.0, 0.0, -1.0];
        let rotated = rotate_direction_by_yaw(dir, 0.0);
        assert!(approx_eq(rotated[0], 0.0));
        assert!(approx_eq(rotated[2], -1.0));
    }

    #[test]
    fn rotate_direction_90_deg() {
        let dir = [0.0, 0.0, -1.0]; // Forward
        let rotated = rotate_direction_by_yaw(dir, 90.0);
        // After 90 degree right turn, forward becomes right (+X)
        assert!(approx_eq(rotated[0], -1.0), "x={}", rotated[0]);
        assert!(approx_eq(rotated[2], 0.0), "z={}", rotated[2]);
    }
}
