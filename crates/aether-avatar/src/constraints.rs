//! Joint angle constraints for IK solving.
//!
//! Constraints limit joint rotations to anatomically plausible ranges.

use std::f32::consts::PI;

/// Angular limits for a single joint (in radians).
#[derive(Debug, Clone)]
pub struct JointConstraint {
    /// Minimum angle per axis [x, y, z] in radians.
    pub min_angle: [f32; 3],
    /// Maximum angle per axis [x, y, z] in radians.
    pub max_angle: [f32; 3],
    /// Maximum twist rotation in radians.
    pub twist_limit: f32,
}

/// A set of constraints keyed by bone index.
#[derive(Debug, Clone)]
pub struct ConstraintSet {
    pub constraints: Vec<(usize, JointConstraint)>,
}

impl JointConstraint {
    /// Create a constraint with symmetric limits on all axes.
    pub fn symmetric(limit_rad: f32, twist_rad: f32) -> Self {
        Self {
            min_angle: [-limit_rad, -limit_rad, -limit_rad],
            max_angle: [limit_rad, limit_rad, limit_rad],
            twist_limit: twist_rad,
        }
    }

    /// A typical human elbow constraint (flexion only, limited twist).
    pub fn human_elbow() -> Self {
        Self {
            min_angle: [0.0, -0.1, -0.1],
            max_angle: [2.6, 0.1, 0.1],    // ~150 degrees flexion
            twist_limit: PI * 0.5,
        }
    }

    /// A typical human knee constraint (flexion only, no twist).
    pub fn human_knee() -> Self {
        Self {
            min_angle: [0.0, -0.05, -0.05],
            max_angle: [2.4, 0.05, 0.05],    // ~140 degrees flexion
            twist_limit: 0.05,
        }
    }

    /// A typical human shoulder constraint (wide range).
    pub fn human_shoulder() -> Self {
        Self {
            min_angle: [-PI * 0.5, -PI * 0.75, -PI * 0.5],
            max_angle: [PI * 0.5, PI * 0.75, PI * 0.5],
            twist_limit: PI * 0.5,
        }
    }

    /// A typical human hip constraint.
    pub fn human_hip() -> Self {
        Self {
            min_angle: [-0.5, -PI * 0.25, -0.3],
            max_angle: [2.0, PI * 0.25, 0.3],
            twist_limit: PI * 0.25,
        }
    }
}

/// Clamp a single Euler angle to the constraint range.
pub fn clamp_angle(angle: f32, min: f32, max: f32) -> f32 {
    angle.clamp(min, max)
}

/// Clamp a set of Euler angles [x, y, z] to a joint constraint.
pub fn clamp_euler(angles: [f32; 3], constraint: &JointConstraint) -> [f32; 3] {
    [
        clamp_angle(angles[0], constraint.min_angle[0], constraint.max_angle[0]),
        clamp_angle(angles[1], constraint.min_angle[1], constraint.max_angle[1]),
        clamp_angle(angles[2], constraint.min_angle[2], constraint.max_angle[2]),
    ]
}

/// Clamp a twist angle to the constraint's twist limit.
pub fn clamp_twist(twist: f32, constraint: &JointConstraint) -> f32 {
    twist.clamp(-constraint.twist_limit, constraint.twist_limit)
}

/// Convert a quaternion to Euler angles (XYZ order). Returns [pitch, yaw, roll].
pub fn quat_to_euler(q: [f32; 4]) -> [f32; 3] {
    let (x, y, z, w) = (q[0], q[1], q[2], q[3]);

    // Pitch (x-axis rotation)
    let sinr_cosp = 2.0 * (w * x + y * z);
    let cosr_cosp = 1.0 - 2.0 * (x * x + y * y);
    let pitch = sinr_cosp.atan2(cosr_cosp);

    // Yaw (y-axis rotation)
    let sinp = 2.0 * (w * y - z * x);
    let yaw = if sinp.abs() >= 1.0 {
        (PI / 2.0).copysign(sinp)
    } else {
        sinp.asin()
    };

    // Roll (z-axis rotation)
    let siny_cosp = 2.0 * (w * z + x * y);
    let cosy_cosp = 1.0 - 2.0 * (y * y + z * z);
    let roll = siny_cosp.atan2(cosy_cosp);

    [pitch, yaw, roll]
}

/// Convert Euler angles (XYZ order) to a quaternion [x, y, z, w].
pub fn euler_to_quat(euler: [f32; 3]) -> [f32; 4] {
    let half_x = euler[0] * 0.5;
    let half_y = euler[1] * 0.5;
    let half_z = euler[2] * 0.5;
    let (cx, sx) = (half_x.cos(), half_x.sin());
    let (cy, sy) = (half_y.cos(), half_y.sin());
    let (cz, sz) = (half_z.cos(), half_z.sin());

    [
        sx * cy * cz - cx * sy * sz,
        cx * sy * cz + sx * cy * sz,
        cx * cy * sz - sx * sy * cz,
        cx * cy * cz + sx * sy * sz,
    ]
}

/// Apply joint constraints to a quaternion rotation.
/// Converts to Euler, clamps, converts back.
pub fn apply_constraint(rotation: [f32; 4], constraint: &JointConstraint) -> [f32; 4] {
    let euler = quat_to_euler(rotation);
    let clamped = clamp_euler(euler, constraint);
    euler_to_quat(clamped)
}

impl ConstraintSet {
    /// Create an empty constraint set.
    pub fn new() -> Self {
        Self {
            constraints: Vec::new(),
        }
    }

    /// Add a constraint for a specific bone index.
    pub fn add(&mut self, bone_index: usize, constraint: JointConstraint) {
        self.constraints.push((bone_index, constraint));
    }

    /// Look up the constraint for a given bone index.
    pub fn get(&self, bone_index: usize) -> Option<&JointConstraint> {
        self.constraints
            .iter()
            .find(|(idx, _)| *idx == bone_index)
            .map(|(_, c)| c)
    }

    /// Apply all constraints to a skeleton's bone rotations in-place.
    pub fn apply_all(&self, rotations: &mut [[f32; 4]]) {
        for (idx, constraint) in &self.constraints {
            if *idx < rotations.len() {
                rotations[*idx] = apply_constraint(rotations[*idx], constraint);
            }
        }
    }
}

impl Default for ConstraintSet {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-4;

    #[test]
    fn test_clamp_angle_within_range() {
        assert!((clamp_angle(0.5, -1.0, 1.0) - 0.5).abs() < EPSILON);
    }

    #[test]
    fn test_clamp_angle_below_min() {
        assert!((clamp_angle(-2.0, -1.0, 1.0) - (-1.0)).abs() < EPSILON);
    }

    #[test]
    fn test_clamp_angle_above_max() {
        assert!((clamp_angle(2.0, -1.0, 1.0) - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_clamp_euler_all_axes() {
        let constraint = JointConstraint::symmetric(1.0, 0.5);
        let angles = [2.0, -2.0, 0.5];
        let clamped = clamp_euler(angles, &constraint);
        assert!((clamped[0] - 1.0).abs() < EPSILON);
        assert!((clamped[1] - (-1.0)).abs() < EPSILON);
        assert!((clamped[2] - 0.5).abs() < EPSILON);
    }

    #[test]
    fn test_clamp_twist() {
        let constraint = JointConstraint::symmetric(1.0, 0.5);
        assert!((clamp_twist(0.3, &constraint) - 0.3).abs() < EPSILON);
        assert!((clamp_twist(1.0, &constraint) - 0.5).abs() < EPSILON);
        assert!((clamp_twist(-1.0, &constraint) - (-0.5)).abs() < EPSILON);
    }

    #[test]
    fn test_euler_quat_roundtrip() {
        let euler = [0.3, 0.5, -0.2];
        let q = euler_to_quat(euler);
        let back = quat_to_euler(q);
        for i in 0..3 {
            assert!(
                (euler[i] - back[i]).abs() < 0.01,
                "axis {i}: expected {}, got {}",
                euler[i],
                back[i]
            );
        }
    }

    #[test]
    fn test_identity_quat_to_euler() {
        let euler = quat_to_euler([0.0, 0.0, 0.0, 1.0]);
        for i in 0..3 {
            assert!(euler[i].abs() < EPSILON, "axis {i} should be zero");
        }
    }

    #[test]
    fn test_apply_constraint_clamps() {
        let constraint = JointConstraint::symmetric(0.5, 0.3);
        // Create a rotation with large angles
        let large_euler = [1.0, -1.0, 0.8];
        let q = euler_to_quat(large_euler);
        let constrained = apply_constraint(q, &constraint);
        let result_euler = quat_to_euler(constrained);
        for i in 0..3 {
            assert!(
                result_euler[i] >= -0.5 - EPSILON && result_euler[i] <= 0.5 + EPSILON,
                "axis {i} out of range: {}",
                result_euler[i]
            );
        }
    }

    #[test]
    fn test_constraint_set_lookup() {
        let mut set = ConstraintSet::new();
        set.add(0, JointConstraint::human_elbow());
        set.add(2, JointConstraint::human_knee());
        assert!(set.get(0).is_some());
        assert!(set.get(1).is_none());
        assert!(set.get(2).is_some());
    }

    #[test]
    fn test_constraint_set_apply_all() {
        let mut set = ConstraintSet::new();
        set.add(0, JointConstraint::symmetric(0.1, 0.1));
        let large_euler = [1.0, 1.0, 1.0];
        let q = euler_to_quat(large_euler);
        let mut rotations = vec![q, [0.0, 0.0, 0.0, 1.0]];
        set.apply_all(&mut rotations);
        let result = quat_to_euler(rotations[0]);
        for i in 0..3 {
            assert!(
                result[i].abs() <= 0.1 + EPSILON,
                "axis {i} should be clamped: {}",
                result[i]
            );
        }
        // Second bone should be unchanged (no constraint)
        assert!((rotations[1][3] - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_human_elbow_no_hyperextension() {
        let constraint = JointConstraint::human_elbow();
        // Negative flexion (hyperextension) should be clamped to 0
        let euler = [-0.5, 0.0, 0.0];
        let clamped = clamp_euler(euler, &constraint);
        assert!((clamped[0] - 0.0).abs() < EPSILON);
    }

    #[test]
    fn test_human_knee_no_hyperextension() {
        let constraint = JointConstraint::human_knee();
        let euler = [-1.0, 0.0, 0.0];
        let clamped = clamp_euler(euler, &constraint);
        assert!((clamped[0] - 0.0).abs() < EPSILON);
    }
}
