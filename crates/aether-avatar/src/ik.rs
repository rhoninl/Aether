//! High-level IK solver: 3-point and 6-point body solving.
//!
//! Uses the FABRIK solver to solve individual limb chains, then
//! assembles them into a full body pose.

use crate::fabrik::{FabrikConfig, FabrikResult, FabrikSolver};
use crate::skeleton::{vec3_add, vec3_lerp, IkTarget};
use crate::tracking::TrackingFrame;

/// Default spine length estimate (head to hip) in meters.
const DEFAULT_SPINE_LENGTH: f32 = 0.55;
/// Default shoulder width (half, from spine to shoulder) in meters.
const DEFAULT_SHOULDER_HALF_WIDTH: f32 = 0.20;
/// Default upper arm length in meters.
const DEFAULT_UPPER_ARM_LENGTH: f32 = 0.32;
/// Default forearm length in meters.
const DEFAULT_FOREARM_LENGTH: f32 = 0.28;
/// Default thigh length in meters.
const DEFAULT_THIGH_LENGTH: f32 = 0.45;
/// Default shin length in meters.
const DEFAULT_SHIN_LENGTH: f32 = 0.43;
/// Number of spine bones for interpolation.
const SPINE_BONE_COUNT: usize = 4;

/// Result of a full-body IK solve.
#[derive(Debug, Clone)]
pub struct IkResult {
    /// Solved joint positions for the full body.
    pub joint_positions: FullBodyPose,
    /// Whether all chains converged.
    pub all_converged: bool,
}

/// Positions of all major body joints after IK solving.
#[derive(Debug, Clone)]
pub struct FullBodyPose {
    pub head: [f32; 3],
    pub neck: [f32; 3],
    pub spine: Vec<[f32; 3]>,
    pub hip: [f32; 3],
    pub left_shoulder: [f32; 3],
    pub left_elbow: [f32; 3],
    pub left_hand: [f32; 3],
    pub right_shoulder: [f32; 3],
    pub right_elbow: [f32; 3],
    pub right_hand: [f32; 3],
    pub left_hip_joint: [f32; 3],
    pub left_knee: [f32; 3],
    pub left_foot: [f32; 3],
    pub right_hip_joint: [f32; 3],
    pub right_knee: [f32; 3],
    pub right_foot: [f32; 3],
}

/// Body proportions used for IK solving.
#[derive(Debug, Clone)]
pub struct BodyProportions {
    pub spine_length: f32,
    pub shoulder_half_width: f32,
    pub upper_arm_length: f32,
    pub forearm_length: f32,
    pub thigh_length: f32,
    pub shin_length: f32,
}

impl Default for BodyProportions {
    fn default() -> Self {
        Self {
            spine_length: DEFAULT_SPINE_LENGTH,
            shoulder_half_width: DEFAULT_SHOULDER_HALF_WIDTH,
            upper_arm_length: DEFAULT_UPPER_ARM_LENGTH,
            forearm_length: DEFAULT_FOREARM_LENGTH,
            thigh_length: DEFAULT_THIGH_LENGTH,
            shin_length: DEFAULT_SHIN_LENGTH,
        }
    }
}

/// Solve 3-point IK: head + left hand + right hand -> full upper body.
///
/// Estimates hip position from the head, then solves each arm as a 2-bone
/// FABRIK chain. Legs are placed in a default standing pose.
pub fn solve_three_point(
    head: &IkTarget,
    left_hand: &IkTarget,
    right_hand: &IkTarget,
    proportions: &BodyProportions,
    fabrik_config: &FabrikConfig,
) -> IkResult {
    let solver = FabrikSolver::new(fabrik_config.clone());

    // Estimate hip from head position
    let hip = [
        head.position[0],
        head.position[1] - proportions.spine_length,
        head.position[2],
    ];

    let neck = vec3_lerp(hip, head.position, 0.85);

    // Estimate shoulder positions (offset from neck)
    let left_shoulder = vec3_add(neck, [-proportions.shoulder_half_width, 0.0, 0.0]);
    let right_shoulder = vec3_add(neck, [proportions.shoulder_half_width, 0.0, 0.0]);

    // Solve left arm
    let left_arm = solve_arm_chain(
        &solver,
        left_shoulder,
        left_hand.position,
        proportions.upper_arm_length,
        proportions.forearm_length,
    );

    // Solve right arm
    let right_arm = solve_arm_chain(
        &solver,
        right_shoulder,
        right_hand.position,
        proportions.upper_arm_length,
        proportions.forearm_length,
    );

    // Interpolate spine
    let spine = interpolate_spine(hip, head.position, SPINE_BONE_COUNT);

    // Default leg positions (standing)
    let leg_offset = proportions.shoulder_half_width * 0.6;
    let left_hip_joint = vec3_add(hip, [-leg_offset, 0.0, 0.0]);
    let right_hip_joint = vec3_add(hip, [leg_offset, 0.0, 0.0]);
    let left_knee = vec3_add(left_hip_joint, [0.0, -proportions.thigh_length, 0.0]);
    let right_knee = vec3_add(right_hip_joint, [0.0, -proportions.thigh_length, 0.0]);
    let left_foot = vec3_add(left_knee, [0.0, -proportions.shin_length, 0.0]);
    let right_foot = vec3_add(right_knee, [0.0, -proportions.shin_length, 0.0]);

    let all_converged = left_arm.converged && right_arm.converged;

    IkResult {
        joint_positions: FullBodyPose {
            head: head.position,
            neck,
            spine,
            hip,
            left_shoulder,
            left_elbow: left_arm.positions[1],
            left_hand: left_arm.positions[2],
            right_shoulder,
            right_elbow: right_arm.positions[1],
            right_hand: right_arm.positions[2],
            left_hip_joint,
            left_knee,
            left_foot,
            right_hip_joint,
            right_knee,
            right_foot,
        },
        all_converged,
    }
}

/// Solve 6-point IK: head + 2 hands + hip + 2 feet -> full body.
///
/// Uses actual tracker positions for hip and feet instead of estimates.
#[allow(clippy::too_many_arguments)]
pub fn solve_six_point(
    head: &IkTarget,
    left_hand: &IkTarget,
    right_hand: &IkTarget,
    hip: &IkTarget,
    left_foot: &IkTarget,
    right_foot: &IkTarget,
    proportions: &BodyProportions,
    fabrik_config: &FabrikConfig,
) -> IkResult {
    let solver = FabrikSolver::new(fabrik_config.clone());

    let neck = vec3_lerp(hip.position, head.position, 0.85);

    // Shoulder positions
    let left_shoulder = vec3_add(neck, [-proportions.shoulder_half_width, 0.0, 0.0]);
    let right_shoulder = vec3_add(neck, [proportions.shoulder_half_width, 0.0, 0.0]);

    // Solve arms
    let left_arm = solve_arm_chain(
        &solver,
        left_shoulder,
        left_hand.position,
        proportions.upper_arm_length,
        proportions.forearm_length,
    );
    let right_arm = solve_arm_chain(
        &solver,
        right_shoulder,
        right_hand.position,
        proportions.upper_arm_length,
        proportions.forearm_length,
    );

    // Solve legs
    let leg_offset = proportions.shoulder_half_width * 0.6;
    let left_hip_joint = vec3_add(hip.position, [-leg_offset, 0.0, 0.0]);
    let right_hip_joint = vec3_add(hip.position, [leg_offset, 0.0, 0.0]);

    let left_leg = solve_leg_chain(
        &solver,
        left_hip_joint,
        left_foot.position,
        proportions.thigh_length,
        proportions.shin_length,
    );
    let right_leg = solve_leg_chain(
        &solver,
        right_hip_joint,
        right_foot.position,
        proportions.thigh_length,
        proportions.shin_length,
    );

    // Interpolate spine
    let spine = interpolate_spine(hip.position, head.position, SPINE_BONE_COUNT);

    let all_converged =
        left_arm.converged && right_arm.converged && left_leg.converged && right_leg.converged;

    IkResult {
        joint_positions: FullBodyPose {
            head: head.position,
            neck,
            spine,
            hip: hip.position,
            left_shoulder,
            left_elbow: left_arm.positions[1],
            left_hand: left_arm.positions[2],
            right_shoulder,
            right_elbow: right_arm.positions[1],
            right_hand: right_arm.positions[2],
            left_hip_joint,
            left_knee: left_leg.positions[1],
            left_foot: left_leg.positions[2],
            right_hip_joint,
            right_knee: right_leg.positions[1],
            right_foot: right_leg.positions[2],
        },
        all_converged,
    }
}

/// Solve from a `TrackingFrame` -- dispatches to 3-point or 6-point based on available data.
pub fn solve_from_tracking(
    frame: &TrackingFrame,
    proportions: &BodyProportions,
    fabrik_config: &FabrikConfig,
) -> IkResult {
    let head = IkTarget {
        position: [frame.head.x, frame.head.y, frame.head.z],
        rotation: None,
    };

    match (&frame.hands, &frame.hips, &frame.feet) {
        (Some((lh, rh)), Some(hip_pt), Some((lf, rf, _))) => {
            let left_hand = IkTarget {
                position: [lh.x, lh.y, lh.z],
                rotation: None,
            };
            let right_hand = IkTarget {
                position: [rh.x, rh.y, rh.z],
                rotation: None,
            };
            let hip = IkTarget {
                position: [hip_pt.x, hip_pt.y, hip_pt.z],
                rotation: None,
            };
            let left_foot = IkTarget {
                position: [lf.x, lf.y, lf.z],
                rotation: None,
            };
            let right_foot = IkTarget {
                position: [rf.x, rf.y, rf.z],
                rotation: None,
            };
            solve_six_point(
                &head,
                &left_hand,
                &right_hand,
                &hip,
                &left_foot,
                &right_foot,
                proportions,
                fabrik_config,
            )
        }
        (Some((lh, rh)), _, _) => {
            let left_hand = IkTarget {
                position: [lh.x, lh.y, lh.z],
                rotation: None,
            };
            let right_hand = IkTarget {
                position: [rh.x, rh.y, rh.z],
                rotation: None,
            };
            solve_three_point(&head, &left_hand, &right_hand, proportions, fabrik_config)
        }
        _ => {
            // Head-only: use default hand positions
            let left_hand = IkTarget {
                position: vec3_add(head.position, [-0.3, -0.3, 0.0]),
                rotation: None,
            };
            let right_hand = IkTarget {
                position: vec3_add(head.position, [0.3, -0.3, 0.0]),
                rotation: None,
            };
            solve_three_point(&head, &left_hand, &right_hand, proportions, fabrik_config)
        }
    }
}

/// Solve a 2-bone arm chain (shoulder -> elbow -> hand).
///
/// Solves without inline constraints for reliable convergence on short chains.
/// Joint constraints should be applied as a post-processing step if needed.
fn solve_arm_chain(
    solver: &FabrikSolver,
    shoulder: [f32; 3],
    hand_target: [f32; 3],
    upper_arm_len: f32,
    forearm_len: f32,
) -> FabrikResult {
    // Initial elbow estimate: midpoint biased downward
    let mid = vec3_lerp(shoulder, hand_target, 0.5);
    let elbow_init = vec3_add(mid, [0.0, -0.1, 0.0]);

    let positions = vec![shoulder, elbow_init, hand_target];
    let lengths = vec![upper_arm_len, forearm_len];

    solver.solve(&positions, &lengths, hand_target, None)
}

/// Solve a 2-bone leg chain (hip_joint -> knee -> foot).
///
/// Solves without inline constraints for reliable convergence on short chains.
/// Joint constraints should be applied as a post-processing step if needed.
fn solve_leg_chain(
    solver: &FabrikSolver,
    hip_joint: [f32; 3],
    foot_target: [f32; 3],
    thigh_len: f32,
    shin_len: f32,
) -> FabrikResult {
    // Initial knee estimate: midpoint biased forward
    let mid = vec3_lerp(hip_joint, foot_target, 0.5);
    let knee_init = vec3_add(mid, [0.0, 0.0, 0.1]);

    let positions = vec![hip_joint, knee_init, foot_target];
    let lengths = vec![thigh_len, shin_len];

    solver.solve(&positions, &lengths, foot_target, None)
}

/// Interpolate spine bones between hip and head.
fn interpolate_spine(hip: [f32; 3], head: [f32; 3], bone_count: usize) -> Vec<[f32; 3]> {
    (0..bone_count)
        .map(|i| {
            let t = (i as f32 + 1.0) / (bone_count as f32 + 1.0);
            vec3_lerp(hip, head, t)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skeleton::vec3_distance;
    use crate::tracking::IkPoint;

    const EPSILON: f32 = 0.05;

    fn default_config() -> FabrikConfig {
        FabrikConfig {
            max_iterations: 20,
            tolerance: 0.001,
        }
    }

    fn default_proportions() -> BodyProportions {
        BodyProportions::default()
    }

    #[test]
    fn test_three_point_produces_valid_pose() {
        let head = IkTarget {
            position: [0.0, 1.7, 0.0],
            rotation: None,
        };
        let left_hand = IkTarget {
            position: [-0.4, 1.2, 0.2],
            rotation: None,
        };
        let right_hand = IkTarget {
            position: [0.4, 1.2, 0.2],
            rotation: None,
        };

        let result = solve_three_point(
            &head,
            &left_hand,
            &right_hand,
            &default_proportions(),
            &default_config(),
        );

        let pose = &result.joint_positions;

        // Head should be at input position
        assert!((pose.head[1] - 1.7).abs() < EPSILON);

        // Hip should be below head
        assert!(pose.hip[1] < pose.head[1]);

        // Shoulders should be at roughly head height
        assert!((pose.left_shoulder[1] - pose.right_shoulder[1]).abs() < 0.1);

        // Left shoulder should be left of right shoulder
        assert!(pose.left_shoulder[0] < pose.right_shoulder[0]);

        // Spine should be between hip and head
        for sp in &pose.spine {
            assert!(sp[1] >= pose.hip[1] - EPSILON);
            assert!(sp[1] <= pose.head[1] + EPSILON);
        }
    }

    #[test]
    fn test_three_point_converges() {
        let head = IkTarget {
            position: [0.0, 1.7, 0.0],
            rotation: None,
        };
        // Hands at roughly shoulder height, within arm reach
        let left_hand = IkTarget {
            position: [-0.4, 1.5, 0.2],
            rotation: None,
        };
        let right_hand = IkTarget {
            position: [0.4, 1.5, 0.2],
            rotation: None,
        };

        let result = solve_three_point(
            &head,
            &left_hand,
            &right_hand,
            &default_proportions(),
            &default_config(),
        );

        assert!(result.all_converged, "arms should converge");
    }

    #[test]
    fn test_six_point_produces_valid_pose() {
        let head = IkTarget {
            position: [0.0, 1.7, 0.0],
            rotation: None,
        };
        // Hands within arm reach of shoulder
        let left_hand = IkTarget {
            position: [-0.35, 1.4, 0.2],
            rotation: None,
        };
        let right_hand = IkTarget {
            position: [0.35, 1.4, 0.2],
            rotation: None,
        };
        let hip = IkTarget {
            position: [0.0, 1.0, 0.0],
            rotation: None,
        };
        // Feet within leg reach of hip joints
        let left_foot = IkTarget {
            position: [-0.12, 0.15, 0.0],
            rotation: None,
        };
        let right_foot = IkTarget {
            position: [0.12, 0.15, 0.0],
            rotation: None,
        };

        let result = solve_six_point(
            &head,
            &left_hand,
            &right_hand,
            &hip,
            &left_foot,
            &right_foot,
            &default_proportions(),
            &default_config(),
        );

        let pose = &result.joint_positions;

        // Hip should be at tracked position
        assert!((pose.hip[1] - 1.0).abs() < EPSILON);

        // Feet should be near tracked positions
        assert!(
            vec3_distance(pose.left_foot, left_foot.position) < 0.15,
            "left foot too far: {:?} vs {:?}, dist={}",
            pose.left_foot,
            left_foot.position,
            vec3_distance(pose.left_foot, left_foot.position)
        );
        assert!(
            vec3_distance(pose.right_foot, right_foot.position) < 0.15,
            "right foot too far: {:?} vs {:?}, dist={}",
            pose.right_foot,
            right_foot.position,
            vec3_distance(pose.right_foot, right_foot.position)
        );
    }

    #[test]
    fn test_six_point_converges() {
        let head = IkTarget {
            position: [0.0, 1.7, 0.0],
            rotation: None,
        };
        // Hands within arm reach
        let left_hand = IkTarget {
            position: [-0.35, 1.4, 0.2],
            rotation: None,
        };
        let right_hand = IkTarget {
            position: [0.35, 1.4, 0.2],
            rotation: None,
        };
        let hip = IkTarget {
            position: [0.0, 1.0, 0.0],
            rotation: None,
        };
        // Feet within leg reach
        let left_foot = IkTarget {
            position: [-0.12, 0.15, 0.0],
            rotation: None,
        };
        let right_foot = IkTarget {
            position: [0.12, 0.15, 0.0],
            rotation: None,
        };

        let result = solve_six_point(
            &head,
            &left_hand,
            &right_hand,
            &hip,
            &left_foot,
            &right_foot,
            &default_proportions(),
            &default_config(),
        );

        assert!(result.all_converged);
    }

    #[test]
    fn test_hands_out_of_reach() {
        let head = IkTarget {
            position: [0.0, 1.7, 0.0],
            rotation: None,
        };
        // Hands way too far away
        let left_hand = IkTarget {
            position: [-5.0, 1.2, 0.0],
            rotation: None,
        };
        let right_hand = IkTarget {
            position: [5.0, 1.2, 0.0],
            rotation: None,
        };

        let result = solve_three_point(
            &head,
            &left_hand,
            &right_hand,
            &default_proportions(),
            &default_config(),
        );

        // Should not converge for out-of-reach targets
        assert!(!result.all_converged);
    }

    #[test]
    fn test_interpolate_spine() {
        let hip = [0.0, 1.0, 0.0];
        let head = [0.0, 1.7, 0.0];
        let spine = interpolate_spine(hip, head, 4);

        assert_eq!(spine.len(), 4);
        // All spine points should be between hip and head
        for sp in &spine {
            assert!(sp[1] > hip[1]);
            assert!(sp[1] < head[1]);
        }
        // Should be in ascending order
        for i in 1..spine.len() {
            assert!(spine[i][1] > spine[i - 1][1]);
        }
    }

    #[test]
    fn test_solve_from_tracking_three_point() {
        let frame = TrackingFrame {
            player_id: 1,
            source: crate::tracking::TrackingSource::ThreePoint,
            head: IkPoint {
                x: 0.0,
                y: 1.7,
                z: 0.0,
            },
            hands: Some((
                IkPoint {
                    x: -0.35,
                    y: 1.4,
                    z: 0.2,
                },
                IkPoint {
                    x: 0.35,
                    y: 1.4,
                    z: 0.2,
                },
            )),
            feet: None,
            hips: None,
            timestamp_ms: 0,
        };

        let result = solve_from_tracking(&frame, &default_proportions(), &default_config());
        assert!(result.all_converged);
    }

    #[test]
    fn test_solve_from_tracking_six_point() {
        let frame = TrackingFrame {
            player_id: 1,
            source: crate::tracking::TrackingSource::SixPoint,
            head: IkPoint {
                x: 0.0,
                y: 1.7,
                z: 0.0,
            },
            hands: Some((
                IkPoint {
                    x: -0.35,
                    y: 1.4,
                    z: 0.2,
                },
                IkPoint {
                    x: 0.35,
                    y: 1.4,
                    z: 0.2,
                },
            )),
            hips: Some(IkPoint {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            }),
            feet: Some((
                IkPoint {
                    x: -0.12,
                    y: 0.15,
                    z: 0.0,
                },
                IkPoint {
                    x: 0.12,
                    y: 0.15,
                    z: 0.0,
                },
                IkPoint {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
            )),
            timestamp_ms: 0,
        };

        let result = solve_from_tracking(&frame, &default_proportions(), &default_config());
        assert!(result.all_converged);
    }

    #[test]
    fn test_solve_from_tracking_head_only() {
        let frame = TrackingFrame {
            player_id: 1,
            source: crate::tracking::TrackingSource::HmdOnly,
            head: IkPoint {
                x: 0.0,
                y: 1.7,
                z: 0.0,
            },
            hands: None,
            feet: None,
            hips: None,
            timestamp_ms: 0,
        };

        let result = solve_from_tracking(&frame, &default_proportions(), &default_config());
        // Should produce a pose even with head only
        assert!((result.joint_positions.head[1] - 1.7).abs() < EPSILON);
    }

    #[test]
    fn test_body_proportions_default() {
        let props = BodyProportions::default();
        assert!(props.spine_length > 0.0);
        assert!(props.upper_arm_length > 0.0);
        assert!(props.forearm_length > 0.0);
        assert!(props.thigh_length > 0.0);
        assert!(props.shin_length > 0.0);
    }
}
