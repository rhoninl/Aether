//! FABRIK (Forward And Backward Reaching Inverse Kinematics) solver.
//!
//! Iteratively solves a bone chain to reach a target position.
//! Supports joint constraints applied after each iteration.

use crate::constraints::{apply_constraint, ConstraintSet};
use crate::skeleton::{vec3_add, vec3_distance, vec3_normalize, vec3_scale, vec3_sub};

/// Configuration for the FABRIK solver.
#[derive(Debug, Clone)]
pub struct FabrikConfig {
    /// Maximum number of iterations before giving up.
    pub max_iterations: u32,
    /// Distance tolerance for convergence (in world units).
    pub tolerance: f32,
}

impl Default for FabrikConfig {
    fn default() -> Self {
        Self {
            max_iterations: 10,
            tolerance: 0.001,
        }
    }
}

/// Result of a FABRIK solve.
#[derive(Debug, Clone)]
pub struct FabrikResult {
    /// Solved joint positions.
    pub positions: Vec<[f32; 3]>,
    /// Whether the solver converged within tolerance.
    pub converged: bool,
    /// Number of iterations performed.
    pub iterations: u32,
    /// Final distance from end effector to target.
    pub final_distance: f32,
}

/// A FABRIK solver instance.
#[derive(Debug, Clone)]
pub struct FabrikSolver {
    pub config: FabrikConfig,
}

impl FabrikSolver {
    /// Create a new FABRIK solver with the given configuration.
    pub fn new(config: FabrikConfig) -> Self {
        Self { config }
    }

    /// Solve a bone chain to reach the target position.
    ///
    /// # Arguments
    /// * `positions` - Joint positions in the chain (first = root, last = end effector)
    /// * `bone_lengths` - Length of each bone segment (one fewer than positions)
    /// * `target` - Target position for the end effector
    /// * `constraints` - Optional joint constraints
    ///
    /// # Returns
    /// A `FabrikResult` with the solved positions.
    pub fn solve(
        &self,
        positions: &[[f32; 3]],
        bone_lengths: &[f32],
        target: [f32; 3],
        constraints: Option<&ConstraintSet>,
    ) -> FabrikResult {
        assert!(
            positions.len() >= 2,
            "FABRIK requires at least 2 joints"
        );
        assert_eq!(
            bone_lengths.len(),
            positions.len() - 1,
            "bone_lengths must be one fewer than positions"
        );

        let n = positions.len();
        let mut joints: Vec<[f32; 3]> = positions.to_vec();
        let root = joints[0];

        // Check if target is reachable
        let total_length: f32 = bone_lengths.iter().sum();
        let root_to_target = vec3_distance(root, target);

        // If target is beyond reach, stretch toward it
        if root_to_target > total_length {
            let dir = vec3_normalize(vec3_sub(target, root));
            let mut current_pos = root;
            joints[0] = root;
            for i in 0..bone_lengths.len() {
                current_pos = vec3_add(current_pos, vec3_scale(dir, bone_lengths[i]));
                joints[i + 1] = current_pos;
            }
            return FabrikResult {
                final_distance: vec3_distance(joints[n - 1], target),
                positions: joints,
                converged: false,
                iterations: 1,
            };
        }

        let mut iterations = 0;
        loop {
            iterations += 1;

            // Forward pass: move end effector to target, iterate up chain
            joints[n - 1] = target;
            for i in (0..n - 1).rev() {
                let dir = vec3_normalize(vec3_sub(joints[i], joints[i + 1]));
                joints[i] = vec3_add(joints[i + 1], vec3_scale(dir, bone_lengths[i]));
            }

            // Backward pass: fix root, iterate down chain
            joints[0] = root;
            for i in 0..n - 1 {
                let dir = vec3_normalize(vec3_sub(joints[i + 1], joints[i]));
                joints[i + 1] = vec3_add(joints[i], vec3_scale(dir, bone_lengths[i]));
            }

            // Apply constraints if provided
            if let Some(constraint_set) = constraints {
                self.apply_chain_constraints(&mut joints, bone_lengths, constraint_set);
            }

            // Check convergence
            let dist = vec3_distance(joints[n - 1], target);
            if dist < self.config.tolerance || iterations >= self.config.max_iterations {
                return FabrikResult {
                    positions: joints,
                    converged: dist < self.config.tolerance,
                    iterations,
                    final_distance: dist,
                };
            }
        }
    }

    /// Solve with a fixed root AND a fixed end effector (two-target).
    /// Useful for chains that must connect two known points (e.g., spine between hip and head).
    pub fn solve_two_target(
        &self,
        positions: &[[f32; 3]],
        bone_lengths: &[f32],
        root_target: [f32; 3],
        end_target: [f32; 3],
        constraints: Option<&ConstraintSet>,
    ) -> FabrikResult {
        let mut modified_positions = positions.to_vec();
        modified_positions[0] = root_target;
        let n = modified_positions.len();
        modified_positions[n - 1] = end_target;

        self.solve(&modified_positions, bone_lengths, end_target, constraints)
    }

    /// Apply joint constraints along a chain by converting positional offsets to
    /// approximate Euler angles and clamping them.
    fn apply_chain_constraints(
        &self,
        joints: &mut [[f32; 3]],
        bone_lengths: &[f32],
        constraint_set: &ConstraintSet,
    ) {
        for i in 1..joints.len() - 1 {
            if let Some(constraint) = constraint_set.get(i) {
                // Compute the angle this joint makes
                let parent_dir = vec3_normalize(vec3_sub(joints[i], joints[i - 1]));
                let child_dir = vec3_normalize(vec3_sub(joints[i + 1], joints[i]));

                // Approximate rotation as Euler from the direction difference
                let euler = direction_to_euler(parent_dir, child_dir);
                let q = crate::constraints::euler_to_quat(euler);
                let constrained_q = apply_constraint(q, constraint);
                let constrained_euler = crate::constraints::quat_to_euler(constrained_q);
                let constrained_dir = euler_to_direction(parent_dir, constrained_euler);

                // Reposition the child joint
                joints[i + 1] = vec3_add(joints[i], vec3_scale(constrained_dir, bone_lengths[i]));
            }
        }
    }
}

/// Compute approximate Euler angles from a parent direction to a child direction.
fn direction_to_euler(parent_dir: [f32; 3], child_dir: [f32; 3]) -> [f32; 3] {
    // Compute angle between the directions
    let dot = (parent_dir[0] * child_dir[0]
        + parent_dir[1] * child_dir[1]
        + parent_dir[2] * child_dir[2])
    .clamp(-1.0, 1.0);
    let angle = dot.acos();

    // Project to approximate Euler components
    let diff = vec3_sub(child_dir, parent_dir);
    let len = (diff[0] * diff[0] + diff[1] * diff[1] + diff[2] * diff[2]).sqrt();
    if len < 1e-9 {
        return [0.0, 0.0, 0.0];
    }
    let norm = vec3_scale(diff, 1.0 / len);
    [norm[1] * angle, norm[2] * angle, norm[0] * angle]
}

/// Convert Euler angles back to a direction relative to a parent direction.
/// Simplified: applies small-angle rotation to the parent direction.
fn euler_to_direction(parent_dir: [f32; 3], euler: [f32; 3]) -> [f32; 3] {
    // Reconstruct a rotation quaternion and apply to parent direction
    let q = crate::constraints::euler_to_quat(euler);
    let rotated = rotate_vector_by_quat(parent_dir, q);
    vec3_normalize(rotated)
}

/// Rotate a vector by a quaternion.
fn rotate_vector_by_quat(v: [f32; 3], q: [f32; 4]) -> [f32; 3] {
    let (qx, qy, qz, qw) = (q[0], q[1], q[2], q[3]);
    let (vx, vy, vz) = (v[0], v[1], v[2]);

    // q * v * q^-1 (for unit quaternion, q^-1 = conjugate)
    let t = [
        2.0 * (qy * vz - qz * vy),
        2.0 * (qz * vx - qx * vz),
        2.0 * (qx * vy - qy * vx),
    ];
    [
        vx + qw * t[0] + (qy * t[2] - qz * t[1]),
        vy + qw * t[1] + (qz * t[0] - qx * t[2]),
        vz + qw * t[2] + (qx * t[1] - qy * t[0]),
    ]
}

impl Default for FabrikSolver {
    fn default() -> Self {
        Self::new(FabrikConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 0.01;

    fn straight_chain_3() -> (Vec<[f32; 3]>, Vec<f32>) {
        // 3 joints, 2 bones, each 1.0 long, along X axis
        let positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]];
        let lengths = vec![1.0, 1.0];
        (positions, lengths)
    }

    fn straight_chain_4() -> (Vec<[f32; 3]>, Vec<f32>) {
        let positions = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [3.0, 0.0, 0.0],
        ];
        let lengths = vec![1.0, 1.0, 1.0];
        (positions, lengths)
    }

    #[test]
    fn test_reach_target_in_range() {
        let (positions, lengths) = straight_chain_3();
        let solver = FabrikSolver::default();
        let target = [1.5, 1.0, 0.0];
        let result = solver.solve(&positions, &lengths, target, None);

        assert!(result.converged, "should converge");
        assert!(
            result.final_distance < 0.01,
            "end effector should be near target, got {}",
            result.final_distance
        );

        // Root should stay at origin
        assert!((result.positions[0][0]).abs() < EPSILON);
        assert!((result.positions[0][1]).abs() < EPSILON);
    }

    #[test]
    fn test_already_at_target() {
        let (positions, lengths) = straight_chain_3();
        let solver = FabrikSolver::default();
        let target = [2.0, 0.0, 0.0]; // end effector is already here
        let result = solver.solve(&positions, &lengths, target, None);

        assert!(result.converged);
        // Should need very few iterations
        assert!(result.iterations <= 2);
    }

    #[test]
    fn test_target_unreachable() {
        let (positions, lengths) = straight_chain_3();
        let solver = FabrikSolver::default();
        let target = [10.0, 0.0, 0.0]; // total chain is 2.0, target at 10
        let result = solver.solve(&positions, &lengths, target, None);

        assert!(!result.converged);
        // Chain should be stretched toward target
        let chain_end = result.positions[2];
        assert!(
            (chain_end[0] - 2.0).abs() < EPSILON,
            "chain should be fully extended: {:?}",
            chain_end
        );
    }

    #[test]
    fn test_bone_lengths_preserved() {
        let (positions, lengths) = straight_chain_3();
        let solver = FabrikSolver::default();
        let target = [0.5, 1.5, 0.0];
        let result = solver.solve(&positions, &lengths, target, None);

        for i in 0..lengths.len() {
            let actual_len =
                vec3_distance(result.positions[i], result.positions[i + 1]);
            assert!(
                (actual_len - lengths[i]).abs() < EPSILON,
                "bone {} length should be {}, got {}",
                i,
                lengths[i],
                actual_len
            );
        }
    }

    #[test]
    fn test_root_stays_fixed() {
        let (positions, lengths) = straight_chain_3();
        let solver = FabrikSolver::default();
        let target = [0.0, 2.0, 0.0];
        let result = solver.solve(&positions, &lengths, target, None);

        assert!((result.positions[0][0]).abs() < EPSILON);
        assert!((result.positions[0][1]).abs() < EPSILON);
        assert!((result.positions[0][2]).abs() < EPSILON);
    }

    #[test]
    fn test_4_bone_chain() {
        let (positions, lengths) = straight_chain_4();
        let solver = FabrikSolver::default();
        let target = [1.0, 2.0, 1.0];
        let result = solver.solve(&positions, &lengths, target, None);

        assert!(result.converged);
        assert!(result.final_distance < 0.01);

        // Verify bone lengths
        for i in 0..lengths.len() {
            let actual_len =
                vec3_distance(result.positions[i], result.positions[i + 1]);
            assert!(
                (actual_len - lengths[i]).abs() < EPSILON,
                "bone {} length mismatch",
                i
            );
        }
    }

    #[test]
    fn test_target_at_root() {
        let (positions, lengths) = straight_chain_3();
        let solver = FabrikSolver::default();
        let target = [0.0, 0.0, 0.0]; // target at root
        let result = solver.solve(&positions, &lengths, target, None);

        // Should fold the chain back on itself or converge close
        // Root stays at origin
        assert!((result.positions[0][0]).abs() < EPSILON);
    }

    #[test]
    fn test_solve_with_constraints() {
        let (positions, lengths) = straight_chain_3();
        let solver = FabrikSolver::new(FabrikConfig {
            max_iterations: 20,
            tolerance: 0.01,
        });
        let mut constraints = ConstraintSet::new();
        constraints.add(
            1,
            crate::constraints::JointConstraint::symmetric(0.5, 0.5),
        );
        let target = [0.0, 2.0, 0.0];
        let result = solver.solve(&positions, &lengths, target, Some(&constraints));

        // Should still attempt to reach target, constraints may prevent full reach
        assert!(result.iterations > 0);
        // Root stays fixed
        assert!((result.positions[0][0]).abs() < EPSILON);
    }

    #[test]
    fn test_two_target_solve() {
        let (positions, lengths) = straight_chain_4();
        let solver = FabrikSolver::default();
        let root_target = [0.0, 0.0, 0.0];
        let end_target = [2.0, 1.5, 0.0];
        let result =
            solver.solve_two_target(&positions, &lengths, root_target, end_target, None);

        assert!((result.positions[0][0]).abs() < EPSILON);
        assert!((result.positions[0][1]).abs() < EPSILON);
    }

    #[test]
    fn test_convergence_iterations() {
        let (positions, lengths) = straight_chain_3();
        let solver = FabrikSolver::new(FabrikConfig {
            max_iterations: 100,
            tolerance: 0.0001,
        });
        let target = [1.0, 1.0, 0.0];
        let result = solver.solve(&positions, &lengths, target, None);

        assert!(result.converged);
        assert!(
            result.iterations < 100,
            "should converge well before max iterations, took {}",
            result.iterations
        );
    }

    #[test]
    fn test_negative_target() {
        let (positions, lengths) = straight_chain_3();
        let solver = FabrikSolver::default();
        let target = [-1.0, -1.0, 0.0];
        let result = solver.solve(&positions, &lengths, target, None);

        assert!(result.converged);
        assert!(result.final_distance < 0.01);
    }

    #[test]
    fn test_3d_target() {
        let (positions, lengths) = straight_chain_3();
        let solver = FabrikSolver::default();
        let target = [0.5, 0.5, 1.5];
        let result = solver.solve(&positions, &lengths, target, None);

        assert!(result.converged);
        assert!(result.final_distance < 0.01);
    }
}
