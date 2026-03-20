//! Hand collision detection with continuous collision detection (CCD) for VR.
//!
//! VR hands move very fast (e.g., during punches or swipes) and can tunnel
//! through thin objects with discrete collision detection. This module provides
//! swept-sphere CCD to detect collisions along the hand's motion path.

use aether_ecs::Entity;

use crate::vr::math;

/// Default number of CCD substeps for swept-sphere tests.
const DEFAULT_CCD_SUBSTEPS: u32 = 4;

/// Default hand collider radius in meters.
const DEFAULT_HAND_COLLIDER_RADIUS: f32 = 0.05;

/// Maximum hand velocity (m/s) above which CCD is mandatory.
const CCD_VELOCITY_THRESHOLD: f32 = 2.0;

/// Configuration for a hand collider.
#[derive(Debug, Clone, PartialEq)]
pub struct HandColliderConfig {
    /// Radius of the hand's sphere collider.
    pub radius: f32,
    /// Number of substeps for CCD swept-sphere test.
    pub ccd_substeps: u32,
    /// Whether CCD is enabled.
    pub ccd_enabled: bool,
    /// Velocity threshold above which CCD is auto-enabled.
    pub ccd_velocity_threshold: f32,
}

impl Default for HandColliderConfig {
    fn default() -> Self {
        Self {
            radius: DEFAULT_HAND_COLLIDER_RADIUS,
            ccd_substeps: DEFAULT_CCD_SUBSTEPS,
            ccd_enabled: true,
            ccd_velocity_threshold: CCD_VELOCITY_THRESHOLD,
        }
    }
}

impl HandColliderConfig {
    /// Create a hand collider config with custom radius.
    pub fn with_radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }

    /// Set the number of CCD substeps.
    pub fn with_ccd_substeps(mut self, substeps: u32) -> Self {
        self.ccd_substeps = substeps.max(1);
        self
    }

    /// Enable or disable CCD.
    pub fn with_ccd(mut self, enabled: bool) -> Self {
        self.ccd_enabled = enabled;
        self
    }
}

/// Result of a hand collision test.
#[derive(Debug, Clone, PartialEq)]
pub struct HandCollisionResult {
    /// The entity that was hit.
    pub entity: Entity,
    /// World-space contact point.
    pub contact_point: [f32; 3],
    /// Surface normal at the contact point.
    pub contact_normal: [f32; 3],
    /// Penetration depth (positive means overlapping).
    pub penetration_depth: f32,
    /// Parametric time of impact along the motion [0, 1].
    pub time_of_impact: f32,
    /// Estimated collision force based on velocity and penetration.
    pub estimated_force: f32,
}

/// A sphere in the world used for collision testing.
#[derive(Debug, Clone, PartialEq)]
pub struct CollisionSphere {
    pub center: [f32; 3],
    pub radius: f32,
    pub entity: Entity,
}

/// Hand collision detector with CCD support.
#[derive(Debug)]
pub struct HandCollisionDetector {
    config: HandColliderConfig,
    previous_position: Option<[f32; 3]>,
}

impl HandCollisionDetector {
    /// Create a new hand collision detector with default config.
    pub fn new() -> Self {
        Self {
            config: HandColliderConfig::default(),
            previous_position: None,
        }
    }

    /// Create with a specific configuration.
    pub fn with_config(config: HandColliderConfig) -> Self {
        Self {
            config,
            previous_position: None,
        }
    }

    /// Get the current configuration.
    pub fn config(&self) -> &HandColliderConfig {
        &self.config
    }

    /// Get the previous hand position, if any.
    pub fn previous_position(&self) -> Option<[f32; 3]> {
        self.previous_position
    }

    /// Returns whether CCD should be used given the hand's velocity.
    pub fn should_use_ccd(&self, velocity_magnitude: f32) -> bool {
        self.config.ccd_enabled && velocity_magnitude > self.config.ccd_velocity_threshold
    }

    /// Update the hand position and detect collisions against a set of spheres.
    ///
    /// Uses swept-sphere CCD when the hand velocity exceeds the threshold,
    /// otherwise performs a simple overlap test.
    pub fn detect(
        &mut self,
        current_position: [f32; 3],
        dt: f32,
        world_spheres: &[CollisionSphere],
    ) -> Vec<HandCollisionResult> {
        let mut results = Vec::new();

        let velocity_magnitude = if let Some(prev) = self.previous_position {
            if dt > 0.0 {
                math::distance(prev, current_position) / dt
            } else {
                0.0
            }
        } else {
            0.0
        };

        if self.should_use_ccd(velocity_magnitude) {
            if let Some(prev) = self.previous_position {
                results = self.swept_sphere_test(prev, current_position, world_spheres);
            }
        } else {
            // Simple overlap test at current position
            results = self.overlap_test(current_position, world_spheres);
        }

        self.previous_position = Some(current_position);
        results
    }

    /// Perform a swept-sphere CCD test from `start` to `end` against world spheres.
    pub fn swept_sphere_test(
        &self,
        start: [f32; 3],
        end: [f32; 3],
        world_spheres: &[CollisionSphere],
    ) -> Vec<HandCollisionResult> {
        let substeps = self.config.ccd_substeps.max(1);
        let mut results = Vec::new();

        for sphere in world_spheres {
            if let Some(result) = self.swept_sphere_vs_sphere(start, end, sphere, substeps) {
                results.push(result);
            }
        }

        results
    }

    /// Test a swept hand sphere against a single world sphere.
    ///
    /// Subdivides the motion into `substeps` and finds the earliest contact.
    fn swept_sphere_vs_sphere(
        &self,
        start: [f32; 3],
        end: [f32; 3],
        target: &CollisionSphere,
        substeps: u32,
    ) -> Option<HandCollisionResult> {
        let combined_radius = self.config.radius + target.radius;
        let combined_radius_sq = combined_radius * combined_radius;

        let mut earliest_t: Option<f32> = None;

        for step in 0..=substeps {
            let t = step as f32 / substeps as f32;
            let sample_pos = math::lerp(start, end, t);
            let to_target = math::sub(target.center, sample_pos);
            let dist_sq = math::length_sq(to_target);

            if dist_sq < combined_radius_sq {
                earliest_t = Some(t);
                break;
            }
        }

        let t = earliest_t?;

        let contact_pos = math::lerp(start, end, t);
        let to_target = math::sub(target.center, contact_pos);
        let dist = math::length(to_target);
        let penetration = combined_radius - dist;

        let normal = if dist > f32::EPSILON {
            math::normalize(to_target)
        } else {
            [0.0, 1.0, 0.0] // fallback normal
        };

        let contact_point = math::add(contact_pos, math::scale(normal, self.config.radius));

        // Estimate force from velocity and penetration
        let motion_dist = math::distance(start, end);
        let estimated_force = motion_dist * penetration.max(0.0) * 100.0;

        Some(HandCollisionResult {
            entity: target.entity,
            contact_point,
            contact_normal: normal,
            penetration_depth: penetration,
            time_of_impact: t,
            estimated_force,
        })
    }

    /// Simple overlap test at a single position.
    fn overlap_test(
        &self,
        position: [f32; 3],
        world_spheres: &[CollisionSphere],
    ) -> Vec<HandCollisionResult> {
        let mut results = Vec::new();

        for sphere in world_spheres {
            let to_target = math::sub(sphere.center, position);
            let dist = math::length(to_target);
            let combined_radius = self.config.radius + sphere.radius;

            if dist < combined_radius {
                let penetration = combined_radius - dist;
                let normal = if dist > f32::EPSILON {
                    math::normalize(to_target)
                } else {
                    [0.0, 1.0, 0.0]
                };
                let contact_point = math::add(position, math::scale(normal, self.config.radius));

                results.push(HandCollisionResult {
                    entity: sphere.entity,
                    contact_point,
                    contact_normal: normal,
                    penetration_depth: penetration,
                    time_of_impact: 1.0,
                    estimated_force: penetration * 10.0,
                });
            }
        }

        results
    }

    /// Reset the detector state (clears previous position).
    pub fn reset(&mut self) {
        self.previous_position = None;
    }
}

impl Default for HandCollisionDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(index: u32) -> Entity {
        unsafe { std::mem::transmute::<(u32, u32), Entity>((index, 0)) }
    }

    // --- HandColliderConfig tests ---

    #[test]
    fn config_defaults() {
        let c = HandColliderConfig::default();
        assert_eq!(c.radius, DEFAULT_HAND_COLLIDER_RADIUS);
        assert_eq!(c.ccd_substeps, DEFAULT_CCD_SUBSTEPS);
        assert!(c.ccd_enabled);
        assert_eq!(c.ccd_velocity_threshold, CCD_VELOCITY_THRESHOLD);
    }

    #[test]
    fn config_with_radius() {
        let c = HandColliderConfig::default().with_radius(0.1);
        assert_eq!(c.radius, 0.1);
    }

    #[test]
    fn config_ccd_substeps_minimum_one() {
        let c = HandColliderConfig::default().with_ccd_substeps(0);
        assert_eq!(c.ccd_substeps, 1);
    }

    #[test]
    fn config_disable_ccd() {
        let c = HandColliderConfig::default().with_ccd(false);
        assert!(!c.ccd_enabled);
    }

    // --- HandCollisionDetector tests ---

    #[test]
    fn detector_default_no_previous_position() {
        let d = HandCollisionDetector::new();
        assert!(d.previous_position().is_none());
    }

    #[test]
    fn detector_with_config() {
        let config = HandColliderConfig::default().with_radius(0.2);
        let d = HandCollisionDetector::with_config(config);
        assert_eq!(d.config().radius, 0.2);
    }

    #[test]
    fn should_use_ccd_above_threshold() {
        let d = HandCollisionDetector::new();
        assert!(!d.should_use_ccd(1.0)); // below threshold
        assert!(d.should_use_ccd(3.0)); // above threshold
    }

    #[test]
    fn should_use_ccd_disabled() {
        let config = HandColliderConfig::default().with_ccd(false);
        let d = HandCollisionDetector::with_config(config);
        assert!(!d.should_use_ccd(100.0));
    }

    #[test]
    fn detect_simple_overlap() {
        let mut detector = HandCollisionDetector::with_config(
            HandColliderConfig::default()
                .with_radius(0.5)
                .with_ccd(false),
        );

        let spheres = vec![CollisionSphere {
            center: [0.3, 0.0, 0.0],
            radius: 0.5,
            entity: entity(1),
        }];

        let results = detector.detect([0.0, 0.0, 0.0], 1.0 / 60.0, &spheres);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].entity.index(), 1);
        assert!(results[0].penetration_depth > 0.0);
    }

    #[test]
    fn detect_no_overlap() {
        let mut detector = HandCollisionDetector::with_config(
            HandColliderConfig::default()
                .with_radius(0.1)
                .with_ccd(false),
        );

        let spheres = vec![CollisionSphere {
            center: [10.0, 0.0, 0.0],
            radius: 0.1,
            entity: entity(1),
        }];

        let results = detector.detect([0.0, 0.0, 0.0], 1.0 / 60.0, &spheres);
        assert!(results.is_empty());
    }

    #[test]
    fn detect_ccd_catches_fast_motion() {
        let config = HandColliderConfig::default()
            .with_radius(0.1)
            .with_ccd(true)
            .with_ccd_substeps(8);

        let mut detector = HandCollisionDetector::with_config(config);

        // Place a sphere in the path of the hand's motion
        let spheres = vec![CollisionSphere {
            center: [5.0, 0.0, 0.0],
            radius: 0.5,
            entity: entity(1),
        }];

        // First call sets previous position
        detector.detect([0.0, 0.0, 0.0], 1.0 / 60.0, &spheres);

        // Second call: hand teleported across the sphere (fast motion)
        let results = detector.detect([10.0, 0.0, 0.0], 1.0 / 60.0, &spheres);
        assert!(!results.is_empty(), "CCD should detect the collision");
        assert!(
            results[0].time_of_impact < 1.0,
            "Should hit before end of motion"
        );
    }

    #[test]
    fn detect_ccd_not_triggered_for_slow_motion() {
        let config = HandColliderConfig::default()
            .with_radius(0.1)
            .with_ccd(true);

        let mut detector = HandCollisionDetector::with_config(config);

        // Sphere far from the motion path
        let spheres = vec![CollisionSphere {
            center: [0.0, 10.0, 0.0],
            radius: 0.1,
            entity: entity(1),
        }];

        // Slow motion: just a tiny step
        detector.detect([0.0, 0.0, 0.0], 1.0 / 60.0, &spheres);
        let results = detector.detect([0.001, 0.0, 0.0], 1.0 / 60.0, &spheres);
        assert!(results.is_empty());
    }

    #[test]
    fn detect_multiple_collisions() {
        let mut detector = HandCollisionDetector::with_config(
            HandColliderConfig::default()
                .with_radius(0.5)
                .with_ccd(false),
        );

        let spheres = vec![
            CollisionSphere {
                center: [0.3, 0.0, 0.0],
                radius: 0.5,
                entity: entity(1),
            },
            CollisionSphere {
                center: [0.0, 0.3, 0.0],
                radius: 0.5,
                entity: entity(2),
            },
        ];

        let results = detector.detect([0.0, 0.0, 0.0], 1.0 / 60.0, &spheres);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn collision_result_has_valid_normal() {
        let mut detector = HandCollisionDetector::with_config(
            HandColliderConfig::default()
                .with_radius(0.5)
                .with_ccd(false),
        );

        let spheres = vec![CollisionSphere {
            center: [1.0, 0.0, 0.0],
            radius: 0.6,
            entity: entity(1),
        }];

        let results = detector.detect([0.0, 0.0, 0.0], 1.0 / 60.0, &spheres);
        assert_eq!(results.len(), 1);

        let normal_len = math::length(results[0].contact_normal);
        assert!(
            (normal_len - 1.0).abs() < 1e-3,
            "Normal should be unit length, got {}",
            normal_len
        );
    }

    #[test]
    fn reset_clears_previous_position() {
        let mut detector = HandCollisionDetector::new();
        detector.detect([1.0, 2.0, 3.0], 1.0 / 60.0, &[]);
        assert!(detector.previous_position().is_some());

        detector.reset();
        assert!(detector.previous_position().is_none());
    }

    #[test]
    fn swept_sphere_test_direct() {
        let detector = HandCollisionDetector::with_config(
            HandColliderConfig::default()
                .with_radius(0.1)
                .with_ccd_substeps(10),
        );

        let spheres = vec![CollisionSphere {
            center: [5.0, 0.0, 0.0],
            radius: 0.5,
            entity: entity(1),
        }];

        let results = detector.swept_sphere_test([0.0, 0.0, 0.0], [10.0, 0.0, 0.0], &spheres);
        assert_eq!(results.len(), 1);
        assert!(results[0].time_of_impact > 0.0);
        assert!(results[0].time_of_impact < 1.0);
    }

    #[test]
    fn swept_sphere_misses() {
        let detector =
            HandCollisionDetector::with_config(HandColliderConfig::default().with_radius(0.1));

        let spheres = vec![CollisionSphere {
            center: [0.0, 100.0, 0.0], // way off the path
            radius: 0.1,
            entity: entity(1),
        }];

        let results = detector.swept_sphere_test([0.0, 0.0, 0.0], [10.0, 0.0, 0.0], &spheres);
        assert!(results.is_empty());
    }

    #[test]
    fn empty_world_no_collisions() {
        let mut detector = HandCollisionDetector::new();
        let results = detector.detect([0.0, 0.0, 0.0], 1.0 / 60.0, &[]);
        assert!(results.is_empty());
    }
}
