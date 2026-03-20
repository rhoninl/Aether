//! Procedural foot placement: ground-plane detection and foot IK.
//!
//! When foot trackers are absent, estimates foot positions from hip
//! position and locomotion state. Clamps feet to the ground plane.

// Skeleton math utilities used for foot position estimation.

/// Default ground plane Y coordinate.
const DEFAULT_GROUND_Y: f32 = 0.0;
/// Toe rotation offset when foot is clamped to ground (radians).
const TOE_GROUND_ANGLE: f32 = 0.15;
/// Step width (half distance between feet) in meters.
const DEFAULT_STEP_HALF_WIDTH: f32 = 0.1;

/// Configuration for foot placement.
#[derive(Debug, Clone)]
pub struct FootPlacement {
    /// Y coordinate of the ground plane.
    pub ground_y: f32,
    /// Maximum height above ground before foot is considered "in air".
    pub air_threshold: f32,
    /// Half-width between feet in standing pose.
    pub step_half_width: f32,
}

impl Default for FootPlacement {
    fn default() -> Self {
        Self {
            ground_y: DEFAULT_GROUND_Y,
            air_threshold: 0.05,
            step_half_width: DEFAULT_STEP_HALF_WIDTH,
        }
    }
}

/// Result of foot IK computation.
#[derive(Debug, Clone)]
pub struct FootIkResult {
    pub left_foot: [f32; 3],
    pub right_foot: [f32; 3],
    /// Toe pitch offset in radians (positive = toes up).
    pub left_toe_angle: f32,
    pub right_toe_angle: f32,
    /// Whether each foot was clamped to the ground.
    pub left_grounded: bool,
    pub right_grounded: bool,
}

/// Apply foot IK: clamp feet to ground plane and compute toe angles.
///
/// # Arguments
/// * `left_foot` - Current left foot position
/// * `right_foot` - Current right foot position
/// * `config` - Foot placement configuration
pub fn foot_ik(left_foot: [f32; 3], right_foot: [f32; 3], config: &FootPlacement) -> FootIkResult {
    let (left_clamped, left_grounded, left_toe) = clamp_foot_to_ground(left_foot, config);
    let (right_clamped, right_grounded, right_toe) = clamp_foot_to_ground(right_foot, config);

    FootIkResult {
        left_foot: left_clamped,
        right_foot: right_clamped,
        left_toe_angle: left_toe,
        right_toe_angle: right_toe,
        left_grounded,
        right_grounded,
    }
}

/// Estimate foot positions when no foot trackers are available.
///
/// Places feet in a default standing position relative to the hip.
pub fn estimate_feet_from_hip(
    hip: [f32; 3],
    leg_length: f32,
    config: &FootPlacement,
) -> FootIkResult {
    let foot_y = (hip[1] - leg_length).max(config.ground_y);
    let left_foot = [hip[0] - config.step_half_width, foot_y, hip[2]];
    let right_foot = [hip[0] + config.step_half_width, foot_y, hip[2]];

    foot_ik(left_foot, right_foot, config)
}

/// Clamp a single foot position to the ground plane.
/// Returns (clamped_position, is_grounded, toe_angle).
fn clamp_foot_to_ground(foot: [f32; 3], config: &FootPlacement) -> ([f32; 3], bool, f32) {
    if foot[1] <= config.ground_y + config.air_threshold {
        // Foot is at or below ground -- clamp to ground
        let clamped = [foot[0], config.ground_y, foot[2]];
        (clamped, true, TOE_GROUND_ANGLE)
    } else {
        // Foot is in the air
        (foot, false, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 0.001;

    #[test]
    fn test_foot_above_ground_unchanged() {
        let config = FootPlacement::default();
        let left = [0.0, 0.5, 0.0];
        let right = [0.2, 0.5, 0.0];
        let result = foot_ik(left, right, &config);

        assert!(!result.left_grounded);
        assert!(!result.right_grounded);
        assert!((result.left_foot[1] - 0.5).abs() < EPSILON);
        assert!((result.right_foot[1] - 0.5).abs() < EPSILON);
        assert!((result.left_toe_angle).abs() < EPSILON);
    }

    #[test]
    fn test_foot_below_ground_clamped() {
        let config = FootPlacement::default();
        let left = [0.0, -0.1, 0.0];
        let right = [0.2, -0.2, 0.0];
        let result = foot_ik(left, right, &config);

        assert!(result.left_grounded);
        assert!(result.right_grounded);
        assert!((result.left_foot[1] - 0.0).abs() < EPSILON);
        assert!((result.right_foot[1] - 0.0).abs() < EPSILON);
    }

    #[test]
    fn test_foot_at_ground_is_grounded() {
        let config = FootPlacement::default();
        let left = [0.0, 0.03, 0.0]; // within air_threshold of 0.05
        let right = [0.2, 0.0, 0.0];
        let result = foot_ik(left, right, &config);

        assert!(result.left_grounded);
        assert!(result.right_grounded);
    }

    #[test]
    fn test_toe_angle_when_grounded() {
        let config = FootPlacement::default();
        let left = [0.0, 0.0, 0.0];
        let right = [0.2, 0.0, 0.0];
        let result = foot_ik(left, right, &config);

        assert!((result.left_toe_angle - TOE_GROUND_ANGLE).abs() < EPSILON);
        assert!((result.right_toe_angle - TOE_GROUND_ANGLE).abs() < EPSILON);
    }

    #[test]
    fn test_toe_angle_when_airborne() {
        let config = FootPlacement::default();
        let left = [0.0, 1.0, 0.0];
        let right = [0.2, 1.0, 0.0];
        let result = foot_ik(left, right, &config);

        assert!((result.left_toe_angle).abs() < EPSILON);
        assert!((result.right_toe_angle).abs() < EPSILON);
    }

    #[test]
    fn test_estimate_feet_from_hip_standing() {
        let config = FootPlacement::default();
        let hip = [0.0, 1.0, 0.0];
        let leg_length = 0.82;
        let result = estimate_feet_from_hip(hip, leg_length, &config);

        // Feet should be at ground level (hip_y - leg_length = 0.18, above ground)
        assert!(result.left_foot[1] >= config.ground_y);
        assert!(result.right_foot[1] >= config.ground_y);
        // Feet should be spread apart
        assert!(result.left_foot[0] < result.right_foot[0]);
    }

    #[test]
    fn test_estimate_feet_from_hip_clamped() {
        let config = FootPlacement::default();
        let hip = [0.0, 0.3, 0.0]; // Very low hip
        let leg_length = 0.82;
        let result = estimate_feet_from_hip(hip, leg_length, &config);

        // Feet should be clamped to ground (hip_y - leg_length = -0.52)
        assert!((result.left_foot[1] - config.ground_y).abs() < EPSILON);
        assert!(result.left_grounded);
    }

    #[test]
    fn test_custom_ground_plane() {
        let config = FootPlacement {
            ground_y: 1.0,
            air_threshold: 0.05,
            step_half_width: 0.1,
        };
        let left = [0.0, 0.8, 0.0]; // below ground_y=1.0
        let right = [0.2, 1.5, 0.0]; // above ground_y=1.0
        let result = foot_ik(left, right, &config);

        assert!(result.left_grounded);
        assert!(!result.right_grounded);
        assert!((result.left_foot[1] - 1.0).abs() < EPSILON);
        assert!((result.right_foot[1] - 1.5).abs() < EPSILON);
    }

    #[test]
    fn test_x_z_preserved_when_clamped() {
        let config = FootPlacement::default();
        let left = [1.5, -0.3, 2.0];
        let right = [3.0, -0.5, 4.0];
        let result = foot_ik(left, right, &config);

        assert!((result.left_foot[0] - 1.5).abs() < EPSILON);
        assert!((result.left_foot[2] - 2.0).abs() < EPSILON);
        assert!((result.right_foot[0] - 3.0).abs() < EPSILON);
        assert!((result.right_foot[2] - 4.0).abs() < EPSILON);
    }
}
