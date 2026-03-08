//! Server-side movement speed validation.
//!
//! Validates that player movement between ticks does not exceed
//! physics-allowed velocity and acceleration limits.

use std::fmt;

/// Default maximum player speed in meters per second.
const DEFAULT_MAX_SPEED: f32 = 20.0;

/// Default maximum player acceleration in meters per second squared.
const DEFAULT_MAX_ACCELERATION: f32 = 50.0;

/// Default tolerance multiplier for speed/acceleration checks.
/// A value of 1.1 allows 10% above the configured limit to account
/// for floating-point drift and minor network jitter.
const DEFAULT_SPEED_TOLERANCE: f32 = 1.1;

/// A 3D position vector.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn zero() -> Self {
        Self::new(0.0, 0.0, 0.0)
    }

    pub fn distance_to(&self, other: &Vec3) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
}

/// Configuration for movement validation, all fields configurable.
#[derive(Debug, Clone)]
pub struct MovementConfig {
    /// Maximum allowed speed in m/s.
    pub max_speed: f32,
    /// Maximum allowed acceleration in m/s^2.
    pub max_acceleration: f32,
    /// Tolerance multiplier applied to limits (e.g., 1.1 = 10% grace).
    pub tolerance: f32,
}

impl Default for MovementConfig {
    fn default() -> Self {
        Self {
            max_speed: DEFAULT_MAX_SPEED,
            max_acceleration: DEFAULT_MAX_ACCELERATION,
            tolerance: DEFAULT_SPEED_TOLERANCE,
        }
    }
}

/// Result of a movement validation check.
#[derive(Debug, Clone, PartialEq)]
pub enum MovementResult {
    /// Movement is within acceptable limits.
    Valid {
        /// Computed speed in m/s.
        speed: f32,
    },
    /// Movement exceeds speed limit.
    SpeedViolation {
        /// Actual speed in m/s.
        actual_speed: f32,
        /// Maximum allowed speed (including tolerance).
        max_allowed: f32,
    },
    /// Movement exceeds acceleration limit.
    AccelerationViolation {
        /// Actual acceleration in m/s^2.
        actual_acceleration: f32,
        /// Maximum allowed acceleration (including tolerance).
        max_allowed: f32,
    },
    /// Invalid input (e.g., zero or negative delta time).
    InvalidInput {
        reason: String,
    },
}

impl fmt::Display for MovementResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MovementResult::Valid { speed } => {
                write!(f, "valid movement at {:.2} m/s", speed)
            }
            MovementResult::SpeedViolation {
                actual_speed,
                max_allowed,
            } => {
                write!(
                    f,
                    "speed violation: {:.2} m/s exceeds limit {:.2} m/s",
                    actual_speed, max_allowed
                )
            }
            MovementResult::AccelerationViolation {
                actual_acceleration,
                max_allowed,
            } => {
                write!(
                    f,
                    "acceleration violation: {:.2} m/s^2 exceeds limit {:.2} m/s^2",
                    actual_acceleration, max_allowed
                )
            }
            MovementResult::InvalidInput { reason } => {
                write!(f, "invalid input: {}", reason)
            }
        }
    }
}

/// Validates a single movement step (position change over a time delta).
///
/// # Arguments
/// - `prev_pos`: The server-authoritative previous position.
/// - `new_pos`: The client-claimed new position.
/// - `dt_secs`: Time delta in seconds since last validated tick.
/// - `config`: Movement limits configuration.
///
/// # Returns
/// A `MovementResult` indicating whether the movement is valid.
pub fn validate_movement(
    prev_pos: &Vec3,
    new_pos: &Vec3,
    dt_secs: f32,
    config: &MovementConfig,
) -> MovementResult {
    if dt_secs <= 0.0 {
        return MovementResult::InvalidInput {
            reason: "delta time must be positive".to_string(),
        };
    }

    let distance = prev_pos.distance_to(new_pos);
    let speed = distance / dt_secs;
    let max_speed = config.max_speed * config.tolerance;

    if speed > max_speed {
        return MovementResult::SpeedViolation {
            actual_speed: speed,
            max_allowed: max_speed,
        };
    }

    MovementResult::Valid { speed }
}

/// Validates movement with acceleration check, requiring the previous velocity.
///
/// # Arguments
/// - `prev_pos`: The server-authoritative previous position.
/// - `new_pos`: The client-claimed new position.
/// - `prev_speed`: The speed at the previous tick in m/s.
/// - `dt_secs`: Time delta in seconds since last validated tick.
/// - `config`: Movement limits configuration.
///
/// # Returns
/// A `MovementResult` indicating whether the movement is valid.
pub fn validate_movement_with_acceleration(
    prev_pos: &Vec3,
    new_pos: &Vec3,
    prev_speed: f32,
    dt_secs: f32,
    config: &MovementConfig,
) -> MovementResult {
    if dt_secs <= 0.0 {
        return MovementResult::InvalidInput {
            reason: "delta time must be positive".to_string(),
        };
    }

    let distance = prev_pos.distance_to(new_pos);
    let speed = distance / dt_secs;
    let max_speed = config.max_speed * config.tolerance;

    if speed > max_speed {
        return MovementResult::SpeedViolation {
            actual_speed: speed,
            max_allowed: max_speed,
        };
    }

    let acceleration = (speed - prev_speed).abs() / dt_secs;
    let max_accel = config.max_acceleration * config.tolerance;

    if acceleration > max_accel {
        return MovementResult::AccelerationViolation {
            actual_acceleration: acceleration,
            max_allowed: max_accel,
        };
    }

    MovementResult::Valid { speed }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> MovementConfig {
        MovementConfig::default()
    }

    // --- Vec3 tests ---

    #[test]
    fn test_vec3_zero() {
        let v = Vec3::zero();
        assert_eq!(v.x, 0.0);
        assert_eq!(v.y, 0.0);
        assert_eq!(v.z, 0.0);
    }

    #[test]
    fn test_vec3_distance_same_point() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        assert!((a.distance_to(&a) - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_vec3_distance_axis_aligned() {
        let a = Vec3::zero();
        let b = Vec3::new(3.0, 4.0, 0.0);
        assert!((a.distance_to(&b) - 5.0).abs() < 1e-5);
    }

    // --- Movement validation: valid cases ---

    #[test]
    fn test_valid_stationary() {
        let pos = Vec3::new(10.0, 0.0, 10.0);
        let result = validate_movement(&pos, &pos, 0.1, &default_config());
        match result {
            MovementResult::Valid { speed } => assert!(speed < f32::EPSILON),
            other => panic!("expected Valid, got {:?}", other),
        }
    }

    #[test]
    fn test_valid_slow_movement() {
        let prev = Vec3::zero();
        let next = Vec3::new(1.0, 0.0, 0.0); // 1 meter
        let dt = 1.0; // 1 second -> speed = 1 m/s
        let result = validate_movement(&prev, &next, dt, &default_config());
        match result {
            MovementResult::Valid { speed } => {
                assert!((speed - 1.0).abs() < 1e-5);
            }
            other => panic!("expected Valid, got {:?}", other),
        }
    }

    #[test]
    fn test_valid_at_max_speed() {
        let config = default_config();
        let prev = Vec3::zero();
        // Move exactly at max speed (20 m/s * 1 second = 20 meters)
        let next = Vec3::new(20.0, 0.0, 0.0);
        let dt = 1.0;
        let result = validate_movement(&prev, &next, dt, &config);
        assert!(matches!(result, MovementResult::Valid { .. }));
    }

    #[test]
    fn test_valid_within_tolerance() {
        let config = default_config();
        // Speed = 21 m/s, limit = 20 * 1.1 = 22 m/s -> should be valid
        let prev = Vec3::zero();
        let next = Vec3::new(21.0, 0.0, 0.0);
        let dt = 1.0;
        let result = validate_movement(&prev, &next, dt, &config);
        assert!(matches!(result, MovementResult::Valid { .. }));
    }

    // --- Movement validation: violations ---

    #[test]
    fn test_speed_violation() {
        let config = default_config();
        // Speed = 30 m/s, limit = 20 * 1.1 = 22 m/s -> violation
        let prev = Vec3::zero();
        let next = Vec3::new(30.0, 0.0, 0.0);
        let dt = 1.0;
        let result = validate_movement(&prev, &next, dt, &config);
        match result {
            MovementResult::SpeedViolation {
                actual_speed,
                max_allowed,
            } => {
                assert!((actual_speed - 30.0).abs() < 1e-5);
                assert!((max_allowed - 22.0).abs() < 1e-5);
            }
            other => panic!("expected SpeedViolation, got {:?}", other),
        }
    }

    #[test]
    fn test_speed_violation_just_over_tolerance() {
        let config = default_config();
        // Speed = 22.01 m/s, limit = 22 m/s -> violation
        let prev = Vec3::zero();
        let next = Vec3::new(22.01, 0.0, 0.0);
        let dt = 1.0;
        let result = validate_movement(&prev, &next, dt, &config);
        assert!(matches!(result, MovementResult::SpeedViolation { .. }));
    }

    // --- Invalid input ---

    #[test]
    fn test_invalid_zero_dt() {
        let pos = Vec3::zero();
        let result = validate_movement(&pos, &pos, 0.0, &default_config());
        assert!(matches!(result, MovementResult::InvalidInput { .. }));
    }

    #[test]
    fn test_invalid_negative_dt() {
        let pos = Vec3::zero();
        let result = validate_movement(&pos, &pos, -1.0, &default_config());
        assert!(matches!(result, MovementResult::InvalidInput { .. }));
    }

    // --- Acceleration validation ---

    #[test]
    fn test_valid_acceleration() {
        let config = default_config();
        let prev = Vec3::zero();
        let next = Vec3::new(5.0, 0.0, 0.0);
        let dt = 1.0;
        let prev_speed = 0.0;
        // speed = 5 m/s, accel = 5 m/s^2, limit = 50 * 1.1 = 55 -> valid
        let result =
            validate_movement_with_acceleration(&prev, &next, prev_speed, dt, &config);
        assert!(matches!(result, MovementResult::Valid { .. }));
    }

    #[test]
    fn test_acceleration_violation() {
        let config = default_config();
        let prev = Vec3::zero();
        // Speed = 20 m/s in 0.1 seconds from standing = 200 m/s^2
        // Accel limit = 50 * 1.1 = 55 m/s^2 -> violation
        let next = Vec3::new(2.0, 0.0, 0.0);
        let dt = 0.1;
        let prev_speed = 0.0;
        let result =
            validate_movement_with_acceleration(&prev, &next, prev_speed, dt, &config);
        assert!(matches!(
            result,
            MovementResult::AccelerationViolation { .. }
        ));
    }

    #[test]
    fn test_acceleration_check_speed_violation_first() {
        let config = default_config();
        // Speed itself violates -> should get SpeedViolation not AccelerationViolation
        let prev = Vec3::zero();
        let next = Vec3::new(30.0, 0.0, 0.0);
        let dt = 1.0;
        let result =
            validate_movement_with_acceleration(&prev, &next, 0.0, dt, &config);
        assert!(matches!(result, MovementResult::SpeedViolation { .. }));
    }

    #[test]
    fn test_acceleration_zero_dt() {
        let pos = Vec3::zero();
        let result =
            validate_movement_with_acceleration(&pos, &pos, 0.0, 0.0, &default_config());
        assert!(matches!(result, MovementResult::InvalidInput { .. }));
    }

    // --- Custom config ---

    #[test]
    fn test_custom_config_strict() {
        let config = MovementConfig {
            max_speed: 5.0,
            max_acceleration: 10.0,
            tolerance: 1.0, // no tolerance
        };
        let prev = Vec3::zero();
        let next = Vec3::new(5.01, 0.0, 0.0);
        let dt = 1.0;
        let result = validate_movement(&prev, &next, dt, &config);
        assert!(matches!(result, MovementResult::SpeedViolation { .. }));
    }

    #[test]
    fn test_custom_config_lenient() {
        let config = MovementConfig {
            max_speed: 100.0,
            max_acceleration: 200.0,
            tolerance: 2.0,
        };
        let prev = Vec3::zero();
        let next = Vec3::new(150.0, 0.0, 0.0);
        let dt = 1.0;
        let result = validate_movement(&prev, &next, dt, &config);
        assert!(matches!(result, MovementResult::Valid { .. }));
    }

    // --- Display ---

    #[test]
    fn test_display_valid() {
        let r = MovementResult::Valid { speed: 5.0 };
        assert!(r.to_string().contains("valid"));
    }

    #[test]
    fn test_display_speed_violation() {
        let r = MovementResult::SpeedViolation {
            actual_speed: 30.0,
            max_allowed: 22.0,
        };
        let s = r.to_string();
        assert!(s.contains("speed violation"));
        assert!(s.contains("30.00"));
    }

    #[test]
    fn test_display_acceleration_violation() {
        let r = MovementResult::AccelerationViolation {
            actual_acceleration: 100.0,
            max_allowed: 55.0,
        };
        let s = r.to_string();
        assert!(s.contains("acceleration violation"));
    }

    #[test]
    fn test_display_invalid_input() {
        let r = MovementResult::InvalidInput {
            reason: "bad dt".to_string(),
        };
        assert!(r.to_string().contains("bad dt"));
    }

    // --- 3D movement ---

    #[test]
    fn test_3d_diagonal_movement() {
        let config = default_config();
        let prev = Vec3::zero();
        // diagonal distance = sqrt(10^2 + 10^2 + 10^2) = 17.32 m/s -> within 22 limit
        let next = Vec3::new(10.0, 10.0, 10.0);
        let dt = 1.0;
        let result = validate_movement(&prev, &next, dt, &config);
        assert!(matches!(result, MovementResult::Valid { .. }));
    }

    #[test]
    fn test_small_dt_high_speed() {
        let config = default_config();
        let prev = Vec3::zero();
        // 1 meter in 0.01 seconds = 100 m/s -> violation
        let next = Vec3::new(1.0, 0.0, 0.0);
        let dt = 0.01;
        let result = validate_movement(&prev, &next, dt, &config);
        assert!(matches!(result, MovementResult::SpeedViolation { .. }));
    }
}
