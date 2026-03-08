//! Joint-based grab system for VR hand interactions.
//!
//! Supports three grab joint types: Fixed (rigid lock), Spring (compliant),
//! and Hinge (rotation around axis). Tracks grab state per hand entity and
//! handles grab initiation, constraint updates, and release.

use aether_ecs::Entity;

use crate::components::Transform;
use crate::vr::math;

/// Default spring stiffness for spring grab joints (N/m).
const DEFAULT_SPRING_STIFFNESS: f32 = 5000.0;

/// Default spring damping for spring grab joints (Ns/m).
const DEFAULT_SPRING_DAMPING: f32 = 100.0;

/// Default break force threshold (Newtons). Grab releases if exceeded.
const DEFAULT_BREAK_FORCE: f32 = 1000.0;

/// Default maximum grab distance. Hand must be within this to grab.
const DEFAULT_MAX_GRAB_DISTANCE: f32 = 0.3;

/// The type of joint used to attach a grabbed object to the hand.
#[derive(Debug, Clone, PartialEq)]
pub enum GrabJointKind {
    /// Rigid lock -- object follows hand exactly.
    Fixed,
    /// Compliant spring joint with stiffness and damping.
    Spring {
        stiffness: f32,
        damping: f32,
    },
    /// Hinge joint allowing rotation around a specified axis.
    Hinge {
        axis: [f32; 3],
    },
}

impl GrabJointKind {
    /// Create a spring joint with default stiffness and damping.
    pub fn spring_default() -> Self {
        GrabJointKind::Spring {
            stiffness: DEFAULT_SPRING_STIFFNESS,
            damping: DEFAULT_SPRING_DAMPING,
        }
    }
}

/// Describes how a grabbed object is constrained to the hand.
#[derive(Debug, Clone, PartialEq)]
pub struct GrabConstraint {
    /// What kind of joint attaches hand to object.
    pub joint_kind: GrabJointKind,
    /// Anchor offset in the hand's local space.
    pub hand_anchor: [f32; 3],
    /// Anchor offset in the object's local space.
    pub object_anchor: [f32; 3],
    /// Force threshold above which the grab automatically breaks.
    pub break_force: f32,
}

impl GrabConstraint {
    /// Create a fixed grab constraint with zero anchor offsets.
    pub fn fixed() -> Self {
        Self {
            joint_kind: GrabJointKind::Fixed,
            hand_anchor: [0.0; 3],
            object_anchor: [0.0; 3],
            break_force: DEFAULT_BREAK_FORCE,
        }
    }

    /// Create a spring grab constraint with default parameters.
    pub fn spring() -> Self {
        Self {
            joint_kind: GrabJointKind::spring_default(),
            hand_anchor: [0.0; 3],
            object_anchor: [0.0; 3],
            break_force: DEFAULT_BREAK_FORCE,
        }
    }

    /// Create a hinge grab constraint around the given axis.
    pub fn hinge(axis: [f32; 3]) -> Self {
        Self {
            joint_kind: GrabJointKind::Hinge {
                axis: math::normalize(axis),
            },
            hand_anchor: [0.0; 3],
            object_anchor: [0.0; 3],
            break_force: DEFAULT_BREAK_FORCE,
        }
    }

    /// Set custom anchor offsets.
    pub fn with_anchors(mut self, hand_anchor: [f32; 3], object_anchor: [f32; 3]) -> Self {
        self.hand_anchor = hand_anchor;
        self.object_anchor = object_anchor;
        self
    }

    /// Set a custom break force.
    pub fn with_break_force(mut self, force: f32) -> Self {
        self.break_force = force;
        self
    }
}

/// Per-hand grab state.
#[derive(Debug, Clone, PartialEq)]
pub enum GrabState {
    /// Hand is not grabbing anything.
    Idle,
    /// Hand is actively holding an object.
    Grabbing {
        /// The entity being grabbed.
        target: Entity,
        /// The constraint describing the attachment.
        constraint: GrabConstraint,
        /// The world-space point where the grab was initiated.
        grab_point: [f32; 3],
    },
}

impl Default for GrabState {
    fn default() -> Self {
        GrabState::Idle
    }
}

impl GrabState {
    /// Returns true if the hand is currently grabbing an object.
    pub fn is_grabbing(&self) -> bool {
        matches!(self, GrabState::Grabbing { .. })
    }

    /// Returns the target entity if grabbing.
    pub fn target(&self) -> Option<Entity> {
        match self {
            GrabState::Grabbing { target, .. } => Some(*target),
            GrabState::Idle => None,
        }
    }
}

/// Result from a spring joint update: the force applied and the desired target position.
#[derive(Debug, Clone, PartialEq)]
pub struct GrabUpdateResult {
    /// The force magnitude computed by the spring constraint.
    pub force_magnitude: f32,
    /// Whether the grab was broken this frame due to exceeding break force.
    pub broke: bool,
    /// Target position for the grabbed object (world space).
    pub target_position: [f32; 3],
    /// Target rotation for the grabbed object (quaternion, world space).
    pub target_rotation: [f32; 4],
}

/// Manages grab interactions for one hand.
#[derive(Debug)]
pub struct GrabSystem {
    state: GrabState,
    max_grab_distance: f32,
}

impl Default for GrabSystem {
    fn default() -> Self {
        Self {
            state: GrabState::Idle,
            max_grab_distance: DEFAULT_MAX_GRAB_DISTANCE,
        }
    }
}

impl GrabSystem {
    /// Create a new grab system with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a grab system with custom max grab distance.
    pub fn with_max_distance(mut self, distance: f32) -> Self {
        self.max_grab_distance = distance;
        self
    }

    /// Get the current grab state.
    pub fn state(&self) -> &GrabState {
        &self.state
    }

    /// Whether the hand is currently grabbing.
    pub fn is_grabbing(&self) -> bool {
        self.state.is_grabbing()
    }

    /// Attempt to grab a target entity.
    ///
    /// Returns `true` if the grab was initiated. Returns `false` if:
    /// - The hand is already grabbing
    /// - The target is out of range
    pub fn try_grab(
        &mut self,
        hand_position: [f32; 3],
        target: Entity,
        target_position: [f32; 3],
        constraint: GrabConstraint,
    ) -> bool {
        if self.is_grabbing() {
            return false;
        }

        let dist = math::distance(hand_position, target_position);
        if dist > self.max_grab_distance {
            return false;
        }

        self.state = GrabState::Grabbing {
            target,
            constraint,
            grab_point: target_position,
        };
        true
    }

    /// Release the current grab. Returns the target entity if one was grabbed.
    pub fn release(&mut self) -> Option<Entity> {
        let old = self.state.take();
        match old {
            GrabState::Grabbing { target, .. } => Some(target),
            GrabState::Idle => None,
        }
    }

    /// Update the grab constraint for one physics step.
    ///
    /// For fixed joints, the target moves exactly to the hand.
    /// For spring joints, a force is computed based on displacement.
    /// For hinge joints, the object position is projected onto the allowed rotation.
    ///
    /// Returns `None` if not grabbing.
    pub fn update(
        &mut self,
        hand_transform: &Transform,
        object_transform: &Transform,
        object_velocity: [f32; 3],
        dt: f32,
    ) -> Option<GrabUpdateResult> {
        let (_target_entity, constraint, _grab_point) = match &self.state {
            GrabState::Grabbing {
                target,
                constraint,
                grab_point,
            } => (*target, constraint.clone(), *grab_point),
            GrabState::Idle => return None,
        };

        let hand_world_anchor = math::add(
            hand_transform.position,
            math::quat_rotate(hand_transform.rotation, constraint.hand_anchor),
        );

        let object_world_anchor = math::add(
            object_transform.position,
            math::quat_rotate(object_transform.rotation, constraint.object_anchor),
        );

        let displacement = math::sub(hand_world_anchor, object_world_anchor);
        let _disp_length = math::length(displacement);

        match &constraint.joint_kind {
            GrabJointKind::Fixed => {
                let result = GrabUpdateResult {
                    force_magnitude: 0.0,
                    broke: false,
                    target_position: math::sub(hand_world_anchor, math::quat_rotate(
                        hand_transform.rotation,
                        constraint.object_anchor,
                    )),
                    target_rotation: hand_transform.rotation,
                };
                Some(result)
            }
            GrabJointKind::Spring {
                stiffness,
                damping,
            } => {
                // Spring force: F = -k * x - c * v
                let spring_force = math::scale(displacement, *stiffness);
                let damping_force = math::scale(object_velocity, -*damping);
                let total_force = math::add(spring_force, damping_force);
                let force_magnitude = math::length(total_force);

                if force_magnitude > constraint.break_force {
                    // Break the grab
                    self.state = GrabState::Idle;
                    return Some(GrabUpdateResult {
                        force_magnitude,
                        broke: true,
                        target_position: object_transform.position,
                        target_rotation: object_transform.rotation,
                    });
                }

                // Target = current + force * dt^2 / mass (simplified: assume unit mass)
                let target_position = math::add(
                    object_transform.position,
                    math::scale(total_force, dt * dt),
                );

                Some(GrabUpdateResult {
                    force_magnitude,
                    broke: false,
                    target_position,
                    target_rotation: hand_transform.rotation,
                })
            }
            GrabJointKind::Hinge { axis } => {
                // Project displacement onto plane perpendicular to hinge axis
                let axis_component = math::scale(*axis, math::dot(displacement, *axis));
                let planar_disp = math::sub(displacement, axis_component);
                let force_magnitude = math::length(planar_disp) * DEFAULT_SPRING_STIFFNESS;

                if force_magnitude > constraint.break_force {
                    self.state = GrabState::Idle;
                    return Some(GrabUpdateResult {
                        force_magnitude,
                        broke: true,
                        target_position: object_transform.position,
                        target_rotation: object_transform.rotation,
                    });
                }

                // Allow object to move along the hinge axis freely
                // but constrain perpendicular motion
                let target_position = math::add(
                    object_transform.position,
                    math::scale(planar_disp, dt),
                );

                Some(GrabUpdateResult {
                    force_magnitude,
                    broke: false,
                    target_position,
                    target_rotation: object_transform.rotation,
                })
            }
        }
    }
}

impl GrabState {
    /// Take the current state, replacing it with `Idle`.
    fn take(&mut self) -> Self {
        std::mem::replace(self, GrabState::Idle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(index: u32) -> Entity {
        unsafe { std::mem::transmute::<(u32, u32), Entity>((index, 0)) }
    }

    fn identity_transform_at(pos: [f32; 3]) -> Transform {
        Transform {
            position: pos,
            rotation: [0.0, 0.0, 0.0, 1.0],
        }
    }

    // --- GrabJointKind tests ---

    #[test]
    fn grab_joint_kind_variants() {
        let fixed = GrabJointKind::Fixed;
        assert_eq!(fixed, GrabJointKind::Fixed);

        let spring = GrabJointKind::spring_default();
        match spring {
            GrabJointKind::Spring { stiffness, damping } => {
                assert_eq!(stiffness, DEFAULT_SPRING_STIFFNESS);
                assert_eq!(damping, DEFAULT_SPRING_DAMPING);
            }
            _ => panic!("Expected Spring"),
        }

        let hinge = GrabJointKind::Hinge {
            axis: [0.0, 1.0, 0.0],
        };
        match hinge {
            GrabJointKind::Hinge { axis } => {
                assert_eq!(axis, [0.0, 1.0, 0.0]);
            }
            _ => panic!("Expected Hinge"),
        }
    }

    // --- GrabConstraint tests ---

    #[test]
    fn constraint_fixed_defaults() {
        let c = GrabConstraint::fixed();
        assert_eq!(c.joint_kind, GrabJointKind::Fixed);
        assert_eq!(c.hand_anchor, [0.0; 3]);
        assert_eq!(c.object_anchor, [0.0; 3]);
        assert_eq!(c.break_force, DEFAULT_BREAK_FORCE);
    }

    #[test]
    fn constraint_spring_defaults() {
        let c = GrabConstraint::spring();
        match c.joint_kind {
            GrabJointKind::Spring { stiffness, damping } => {
                assert_eq!(stiffness, DEFAULT_SPRING_STIFFNESS);
                assert_eq!(damping, DEFAULT_SPRING_DAMPING);
            }
            _ => panic!("Expected Spring"),
        }
    }

    #[test]
    fn constraint_hinge_normalizes_axis() {
        let c = GrabConstraint::hinge([0.0, 3.0, 0.0]);
        match c.joint_kind {
            GrabJointKind::Hinge { axis } => {
                let len = math::length(axis);
                assert!((len - 1.0).abs() < 1e-5, "Axis should be normalized");
            }
            _ => panic!("Expected Hinge"),
        }
    }

    #[test]
    fn constraint_with_anchors() {
        let c = GrabConstraint::fixed()
            .with_anchors([0.1, 0.0, 0.0], [0.0, 0.1, 0.0]);
        assert_eq!(c.hand_anchor, [0.1, 0.0, 0.0]);
        assert_eq!(c.object_anchor, [0.0, 0.1, 0.0]);
    }

    #[test]
    fn constraint_with_break_force() {
        let c = GrabConstraint::fixed().with_break_force(500.0);
        assert_eq!(c.break_force, 500.0);
    }

    // --- GrabState tests ---

    #[test]
    fn grab_state_default_is_idle() {
        let state = GrabState::default();
        assert!(!state.is_grabbing());
        assert!(state.target().is_none());
    }

    #[test]
    fn grab_state_grabbing() {
        let state = GrabState::Grabbing {
            target: entity(42),
            constraint: GrabConstraint::fixed(),
            grab_point: [1.0, 2.0, 3.0],
        };
        assert!(state.is_grabbing());
        assert_eq!(state.target().unwrap().index(), 42);
    }

    // --- GrabSystem tests ---

    #[test]
    fn grab_system_default_is_idle() {
        let system = GrabSystem::new();
        assert!(!system.is_grabbing());
        assert!(matches!(system.state(), GrabState::Idle));
    }

    #[test]
    fn grab_system_custom_distance() {
        let system = GrabSystem::new().with_max_distance(1.0);
        assert_eq!(system.max_grab_distance, 1.0);
    }

    #[test]
    fn try_grab_succeeds_within_range() {
        let mut system = GrabSystem::new().with_max_distance(1.0);
        let hand_pos = [0.0, 0.0, 0.0];
        let target_pos = [0.5, 0.0, 0.0];
        let target = entity(1);

        let result = system.try_grab(hand_pos, target, target_pos, GrabConstraint::fixed());
        assert!(result);
        assert!(system.is_grabbing());
        assert_eq!(system.state().target().unwrap().index(), 1);
    }

    #[test]
    fn try_grab_fails_out_of_range() {
        let mut system = GrabSystem::new().with_max_distance(0.5);
        let hand_pos = [0.0, 0.0, 0.0];
        let target_pos = [10.0, 0.0, 0.0]; // far away
        let target = entity(1);

        let result = system.try_grab(hand_pos, target, target_pos, GrabConstraint::fixed());
        assert!(!result);
        assert!(!system.is_grabbing());
    }

    #[test]
    fn try_grab_fails_when_already_grabbing() {
        let mut system = GrabSystem::new().with_max_distance(1.0);
        let hand_pos = [0.0, 0.0, 0.0];
        let target_pos = [0.1, 0.0, 0.0];

        system.try_grab(hand_pos, entity(1), target_pos, GrabConstraint::fixed());
        let second = system.try_grab(hand_pos, entity(2), target_pos, GrabConstraint::fixed());
        assert!(!second);
        assert_eq!(system.state().target().unwrap().index(), 1);
    }

    #[test]
    fn release_returns_target() {
        let mut system = GrabSystem::new().with_max_distance(1.0);
        system.try_grab(
            [0.0; 3],
            entity(5),
            [0.1, 0.0, 0.0],
            GrabConstraint::fixed(),
        );

        let released = system.release();
        assert_eq!(released.unwrap().index(), 5);
        assert!(!system.is_grabbing());
    }

    #[test]
    fn release_when_idle_returns_none() {
        let mut system = GrabSystem::new();
        assert!(system.release().is_none());
    }

    #[test]
    fn update_when_idle_returns_none() {
        let mut system = GrabSystem::new();
        let hand = identity_transform_at([0.0; 3]);
        let obj = identity_transform_at([0.0; 3]);
        assert!(system.update(&hand, &obj, [0.0; 3], 1.0 / 60.0).is_none());
    }

    #[test]
    fn update_fixed_joint_snaps_to_hand() {
        let mut system = GrabSystem::new().with_max_distance(5.0);
        system.try_grab(
            [0.0; 3],
            entity(1),
            [0.0; 3],
            GrabConstraint::fixed(),
        );

        let hand = identity_transform_at([1.0, 2.0, 3.0]);
        let obj = identity_transform_at([0.0; 3]);
        let result = system.update(&hand, &obj, [0.0; 3], 1.0 / 60.0).unwrap();

        assert!(!result.broke);
        assert_eq!(result.force_magnitude, 0.0);
        // Fixed joint: target should be at hand position
        assert!(
            (result.target_position[0] - 1.0).abs() < 1e-5,
            "x: {}",
            result.target_position[0]
        );
    }

    #[test]
    fn update_spring_joint_computes_force() {
        let mut system = GrabSystem::new().with_max_distance(5.0);
        system.try_grab(
            [0.0; 3],
            entity(1),
            [0.0; 3],
            GrabConstraint::spring().with_break_force(f32::MAX),
        );

        let hand = identity_transform_at([0.1, 0.0, 0.0]); // 0.1m away, force = 500 N
        let obj = identity_transform_at([0.0; 3]);
        let result = system.update(&hand, &obj, [0.0; 3], 1.0 / 60.0).unwrap();

        assert!(!result.broke);
        // Force = stiffness * displacement = 5000 * 0.1 = 500 N
        assert!(result.force_magnitude > 0.0);
    }

    #[test]
    fn spring_joint_breaks_when_force_exceeds_threshold() {
        let mut system = GrabSystem::new().with_max_distance(100.0);
        system.try_grab(
            [0.0; 3],
            entity(1),
            [0.0; 3],
            GrabConstraint::spring().with_break_force(10.0), // very low threshold
        );

        // Large displacement -> large force -> should break
        let hand = identity_transform_at([100.0, 0.0, 0.0]);
        let obj = identity_transform_at([0.0; 3]);
        let result = system.update(&hand, &obj, [0.0; 3], 1.0 / 60.0).unwrap();

        assert!(result.broke);
        assert!(!system.is_grabbing()); // state should reset to idle
    }

    #[test]
    fn hinge_joint_constrains_to_perpendicular_plane() {
        let mut system = GrabSystem::new().with_max_distance(5.0);
        system.try_grab(
            [0.0; 3],
            entity(1),
            [0.0; 3],
            GrabConstraint::hinge([0.0, 1.0, 0.0]).with_break_force(f32::MAX), // Y axis hinge
        );

        let hand = identity_transform_at([0.1, 0.0, 0.0]); // small displacement in X
        let obj = identity_transform_at([0.0; 3]);
        let result = system.update(&hand, &obj, [0.0; 3], 1.0 / 60.0).unwrap();

        assert!(!result.broke);
        assert!(result.force_magnitude > 0.0);
    }

    #[test]
    fn hinge_joint_break_force() {
        let mut system = GrabSystem::new().with_max_distance(100.0);
        system.try_grab(
            [0.0; 3],
            entity(1),
            [0.0; 3],
            GrabConstraint::hinge([0.0, 1.0, 0.0]).with_break_force(1.0),
        );

        let hand = identity_transform_at([100.0, 0.0, 0.0]);
        let obj = identity_transform_at([0.0; 3]);
        let result = system.update(&hand, &obj, [0.0; 3], 1.0 / 60.0).unwrap();

        assert!(result.broke);
        assert!(!system.is_grabbing());
    }

    #[test]
    fn grab_release_regrab_cycle() {
        let mut system = GrabSystem::new().with_max_distance(5.0);

        // Grab
        assert!(system.try_grab(
            [0.0; 3],
            entity(1),
            [0.0; 3],
            GrabConstraint::fixed(),
        ));
        assert!(system.is_grabbing());

        // Release
        system.release();
        assert!(!system.is_grabbing());

        // Grab again
        assert!(system.try_grab(
            [0.0; 3],
            entity(2),
            [0.0; 3],
            GrabConstraint::spring(),
        ));
        assert_eq!(system.state().target().unwrap().index(), 2);
    }
}
