//! Object manipulation (rotate, scale) while held in VR.
//!
//! Supports single-hand rotation and two-hand rotate+scale gestures.
//! Single-hand mode tracks the hand's rotation delta to apply to the object.
//! Two-hand mode uses inter-hand distance for scale and midpoint rotation.

use crate::components::Transform;
use crate::vr::math;

/// Minimum allowed scale factor for manipulated objects.
const DEFAULT_SCALE_MIN: f32 = 0.1;

/// Maximum allowed scale factor for manipulated objects.
const DEFAULT_SCALE_MAX: f32 = 10.0;

/// Minimum inter-hand distance (meters) below which scaling is not applied.
const MIN_INTER_HAND_DISTANCE: f32 = 0.05;

/// Deadzone for rotation (radians). Rotations smaller than this are ignored.
const ROTATION_DEADZONE: f32 = 0.001;

/// Available manipulation modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManipulationMode {
    /// No manipulation active.
    None,
    /// Single-hand rotation.
    Rotate,
    /// Two-hand scaling.
    Scale,
    /// Two-hand rotation and scaling simultaneously.
    RotateAndScale,
}

/// Configuration for the manipulation system.
#[derive(Debug, Clone, PartialEq)]
pub struct ManipulationConfig {
    /// Minimum scale factor.
    pub scale_min: f32,
    /// Maximum scale factor.
    pub scale_max: f32,
    /// Rotation sensitivity multiplier.
    pub rotation_sensitivity: f32,
    /// Scale sensitivity multiplier.
    pub scale_sensitivity: f32,
}

impl Default for ManipulationConfig {
    fn default() -> Self {
        Self {
            scale_min: DEFAULT_SCALE_MIN,
            scale_max: DEFAULT_SCALE_MAX,
            rotation_sensitivity: 1.0,
            scale_sensitivity: 1.0,
        }
    }
}

impl ManipulationConfig {
    pub fn with_scale_range(mut self, min: f32, max: f32) -> Self {
        self.scale_min = min;
        self.scale_max = max;
        self
    }

    pub fn with_rotation_sensitivity(mut self, sensitivity: f32) -> Self {
        self.rotation_sensitivity = sensitivity;
        self
    }

    pub fn with_scale_sensitivity(mut self, sensitivity: f32) -> Self {
        self.scale_sensitivity = sensitivity;
        self
    }
}

/// Result of a manipulation update.
#[derive(Debug, Clone, PartialEq)]
pub struct ManipulationResult {
    /// Delta rotation quaternion to apply to the object.
    pub rotation_delta: [f32; 4],
    /// Scale factor to apply (1.0 = no change).
    pub scale_factor: f32,
    /// The current manipulation mode.
    pub mode: ManipulationMode,
}

impl ManipulationResult {
    /// No-op result (identity rotation, unit scale).
    pub fn identity() -> Self {
        Self {
            rotation_delta: math::QUAT_IDENTITY,
            scale_factor: 1.0,
            mode: ManipulationMode::None,
        }
    }

    /// Returns true if this result represents no change.
    pub fn is_identity(&self) -> bool {
        self.scale_factor == 1.0
            && self.rotation_delta == math::QUAT_IDENTITY
    }
}

/// Tracks object manipulation state.
#[derive(Debug)]
pub struct ManipulationState {
    config: ManipulationConfig,
    mode: ManipulationMode,
    /// Current cumulative scale of the manipulated object.
    current_scale: f32,
    /// Previous primary hand rotation (for computing deltas).
    prev_primary_rotation: Option<[f32; 4]>,
    /// Previous inter-hand distance (for computing scale deltas).
    prev_inter_hand_distance: Option<f32>,
    /// Previous midpoint rotation for two-hand mode.
    prev_midpoint_rotation: Option<[f32; 4]>,
}

impl ManipulationState {
    /// Create a new manipulation state with default config.
    pub fn new() -> Self {
        Self {
            config: ManipulationConfig::default(),
            mode: ManipulationMode::None,
            current_scale: 1.0,
            prev_primary_rotation: None,
            prev_inter_hand_distance: None,
            prev_midpoint_rotation: None,
        }
    }

    /// Create with a specific configuration.
    pub fn with_config(config: ManipulationConfig) -> Self {
        Self {
            config,
            mode: ManipulationMode::None,
            current_scale: 1.0,
            prev_primary_rotation: None,
            prev_inter_hand_distance: None,
            prev_midpoint_rotation: None,
        }
    }

    /// Get the current manipulation mode.
    pub fn mode(&self) -> ManipulationMode {
        self.mode
    }

    /// Get the current cumulative scale.
    pub fn current_scale(&self) -> f32 {
        self.current_scale
    }

    /// Get the configuration.
    pub fn config(&self) -> &ManipulationConfig {
        &self.config
    }

    /// Begin single-hand rotation manipulation.
    pub fn begin_rotate(&mut self, hand_rotation: [f32; 4]) {
        self.mode = ManipulationMode::Rotate;
        self.prev_primary_rotation = Some(hand_rotation);
    }

    /// Begin two-hand manipulation (rotate and/or scale).
    pub fn begin_two_hand(
        &mut self,
        left_transform: &Transform,
        right_transform: &Transform,
        mode: ManipulationMode,
    ) {
        self.mode = mode;
        let dist = math::distance(left_transform.position, right_transform.position);
        self.prev_inter_hand_distance = Some(dist);

        // Compute a midpoint rotation from the two hands
        let midpoint_rotation = self.compute_midpoint_rotation(
            left_transform.rotation,
            right_transform.rotation,
        );
        self.prev_midpoint_rotation = Some(midpoint_rotation);
    }

    /// Update single-hand rotation manipulation.
    ///
    /// Returns the rotation delta to apply to the held object.
    pub fn update_one_hand(&mut self, hand_rotation: [f32; 4]) -> ManipulationResult {
        if self.mode != ManipulationMode::Rotate {
            return ManipulationResult::identity();
        }

        let prev = match self.prev_primary_rotation {
            Some(r) => r,
            None => {
                self.prev_primary_rotation = Some(hand_rotation);
                return ManipulationResult::identity();
            }
        };

        let delta = math::quat_delta(prev, hand_rotation);
        let delta = math::quat_normalize(delta);

        // Check deadzone: if rotation angle is very small, skip
        let angle = quat_angle(delta);
        let effective_delta = if angle < ROTATION_DEADZONE {
            math::QUAT_IDENTITY
        } else {
            delta
        };

        self.prev_primary_rotation = Some(hand_rotation);

        ManipulationResult {
            rotation_delta: effective_delta,
            scale_factor: 1.0,
            mode: ManipulationMode::Rotate,
        }
    }

    /// Update two-hand manipulation (rotate + scale).
    pub fn update_two_hands(
        &mut self,
        left_transform: &Transform,
        right_transform: &Transform,
    ) -> ManipulationResult {
        if self.mode != ManipulationMode::RotateAndScale
            && self.mode != ManipulationMode::Scale
        {
            return ManipulationResult::identity();
        }

        let curr_dist = math::distance(left_transform.position, right_transform.position);

        // Compute scale delta
        let scale_factor = match self.prev_inter_hand_distance {
            Some(prev_dist) if prev_dist > MIN_INTER_HAND_DISTANCE => {
                let raw_scale = curr_dist / prev_dist;
                // Apply sensitivity
                let sensitive_scale = 1.0 + (raw_scale - 1.0) * self.config.scale_sensitivity;
                let new_scale = self.current_scale * sensitive_scale;
                let clamped_scale = math::clamp(new_scale, self.config.scale_min, self.config.scale_max);
                let factor = clamped_scale / self.current_scale;
                self.current_scale = clamped_scale;
                factor
            }
            _ => 1.0,
        };

        // Compute rotation delta
        let rotation_delta = if self.mode == ManipulationMode::RotateAndScale {
            let midpoint_rotation = self.compute_midpoint_rotation(
                left_transform.rotation,
                right_transform.rotation,
            );

            let delta = match self.prev_midpoint_rotation {
                Some(prev) => {
                    let d = math::quat_delta(prev, midpoint_rotation);
                    math::quat_normalize(d)
                }
                None => math::QUAT_IDENTITY,
            };

            self.prev_midpoint_rotation = Some(midpoint_rotation);
            delta
        } else {
            math::QUAT_IDENTITY
        };

        self.prev_inter_hand_distance = Some(curr_dist);

        ManipulationResult {
            rotation_delta,
            scale_factor,
            mode: self.mode,
        }
    }

    /// End the current manipulation.
    pub fn end(&mut self) {
        self.mode = ManipulationMode::None;
        self.prev_primary_rotation = None;
        self.prev_inter_hand_distance = None;
        self.prev_midpoint_rotation = None;
    }

    /// Reset the state including accumulated scale.
    pub fn reset(&mut self) {
        self.end();
        self.current_scale = 1.0;
    }

    /// Compute a "midpoint" rotation by averaging two hand rotations.
    /// This is a simplified SLERP at t=0.5.
    fn compute_midpoint_rotation(&self, left: [f32; 4], right: [f32; 4]) -> [f32; 4] {
        // Simple average + normalize (good enough for small angular differences)
        let avg = [
            (left[0] + right[0]) * 0.5,
            (left[1] + right[1]) * 0.5,
            (left[2] + right[2]) * 0.5,
            (left[3] + right[3]) * 0.5,
        ];
        math::quat_normalize(avg)
    }
}

impl Default for ManipulationState {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute the rotation angle (in radians) of a quaternion.
fn quat_angle(q: [f32; 4]) -> f32 {
    // angle = 2 * acos(|w|)
    let w = q[3].abs().min(1.0);
    2.0 * w.acos()
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-4;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    fn transform_at(pos: [f32; 3], rot: [f32; 4]) -> Transform {
        Transform {
            position: pos,
            rotation: rot,
        }
    }

    fn identity_transform_at(pos: [f32; 3]) -> Transform {
        transform_at(pos, math::QUAT_IDENTITY)
    }

    // --- ManipulationConfig tests ---

    #[test]
    fn config_defaults() {
        let c = ManipulationConfig::default();
        assert_eq!(c.scale_min, DEFAULT_SCALE_MIN);
        assert_eq!(c.scale_max, DEFAULT_SCALE_MAX);
        assert_eq!(c.rotation_sensitivity, 1.0);
        assert_eq!(c.scale_sensitivity, 1.0);
    }

    #[test]
    fn config_with_scale_range() {
        let c = ManipulationConfig::default().with_scale_range(0.5, 5.0);
        assert_eq!(c.scale_min, 0.5);
        assert_eq!(c.scale_max, 5.0);
    }

    #[test]
    fn config_with_sensitivities() {
        let c = ManipulationConfig::default()
            .with_rotation_sensitivity(2.0)
            .with_scale_sensitivity(0.5);
        assert_eq!(c.rotation_sensitivity, 2.0);
        assert_eq!(c.scale_sensitivity, 0.5);
    }

    // --- ManipulationResult tests ---

    #[test]
    fn identity_result() {
        let r = ManipulationResult::identity();
        assert!(r.is_identity());
        assert_eq!(r.mode, ManipulationMode::None);
        assert_eq!(r.scale_factor, 1.0);
    }

    // --- ManipulationState tests ---

    #[test]
    fn state_default_is_none() {
        let s = ManipulationState::new();
        assert_eq!(s.mode(), ManipulationMode::None);
        assert!(approx_eq(s.current_scale(), 1.0));
    }

    #[test]
    fn begin_rotate_sets_mode() {
        let mut s = ManipulationState::new();
        s.begin_rotate(math::QUAT_IDENTITY);
        assert_eq!(s.mode(), ManipulationMode::Rotate);
    }

    #[test]
    fn one_hand_rotate_identity_when_no_movement() {
        let mut s = ManipulationState::new();
        let rot = math::QUAT_IDENTITY;
        s.begin_rotate(rot);
        let result = s.update_one_hand(rot);
        assert_eq!(result.rotation_delta, math::QUAT_IDENTITY);
        assert!(approx_eq(result.scale_factor, 1.0));
    }

    #[test]
    fn one_hand_rotate_detects_rotation() {
        let mut s = ManipulationState::new();
        s.begin_rotate(math::QUAT_IDENTITY);

        // Rotate 90 degrees around Y
        let rotated = math::quat_normalize([0.0, 0.7071, 0.0, 0.7071]);
        let result = s.update_one_hand(rotated);

        // Should produce a non-identity delta
        assert_ne!(result.rotation_delta, math::QUAT_IDENTITY);
        assert_eq!(result.mode, ManipulationMode::Rotate);
    }

    #[test]
    fn one_hand_rotate_wrong_mode_returns_identity() {
        let mut s = ManipulationState::new();
        // Mode is None, not Rotate
        let result = s.update_one_hand(math::QUAT_IDENTITY);
        assert!(result.is_identity());
    }

    #[test]
    fn begin_two_hand_sets_mode() {
        let mut s = ManipulationState::new();
        let left = identity_transform_at([-0.3, 1.0, 0.0]);
        let right = identity_transform_at([0.3, 1.0, 0.0]);
        s.begin_two_hand(&left, &right, ManipulationMode::RotateAndScale);
        assert_eq!(s.mode(), ManipulationMode::RotateAndScale);
    }

    #[test]
    fn two_hand_scale_increases() {
        let mut s = ManipulationState::new();
        let left = identity_transform_at([-0.3, 1.0, 0.0]);
        let right = identity_transform_at([0.3, 1.0, 0.0]);
        s.begin_two_hand(&left, &right, ManipulationMode::Scale);

        // Move hands further apart -> scale up
        let left2 = identity_transform_at([-0.6, 1.0, 0.0]);
        let right2 = identity_transform_at([0.6, 1.0, 0.0]);
        let result = s.update_two_hands(&left2, &right2);

        assert!(result.scale_factor > 1.0, "Scale should increase");
        assert!(s.current_scale() > 1.0);
    }

    #[test]
    fn two_hand_scale_decreases() {
        let mut s = ManipulationState::new();
        let left = identity_transform_at([-0.6, 1.0, 0.0]);
        let right = identity_transform_at([0.6, 1.0, 0.0]);
        s.begin_two_hand(&left, &right, ManipulationMode::Scale);

        // Move hands closer -> scale down
        let left2 = identity_transform_at([-0.3, 1.0, 0.0]);
        let right2 = identity_transform_at([0.3, 1.0, 0.0]);
        let result = s.update_two_hands(&left2, &right2);

        assert!(result.scale_factor < 1.0, "Scale should decrease");
        assert!(s.current_scale() < 1.0);
    }

    #[test]
    fn two_hand_scale_clamps_min() {
        let mut s = ManipulationState::with_config(
            ManipulationConfig::default().with_scale_range(0.5, 5.0),
        );
        let left = identity_transform_at([-1.0, 0.0, 0.0]);
        let right = identity_transform_at([1.0, 0.0, 0.0]);
        s.begin_two_hand(&left, &right, ManipulationMode::Scale);

        // Hands very close -> try to scale below min
        let left2 = identity_transform_at([-0.01, 0.0, 0.0]);
        let right2 = identity_transform_at([0.01, 0.0, 0.0]);
        s.update_two_hands(&left2, &right2);

        assert!(
            s.current_scale() >= 0.5 - EPSILON,
            "Scale should not go below min: {}",
            s.current_scale(),
        );
    }

    #[test]
    fn two_hand_scale_clamps_max() {
        let mut s = ManipulationState::with_config(
            ManipulationConfig::default().with_scale_range(0.1, 2.0),
        );
        let left = identity_transform_at([-0.1, 0.0, 0.0]);
        let right = identity_transform_at([0.1, 0.0, 0.0]);
        s.begin_two_hand(&left, &right, ManipulationMode::Scale);

        // Hands very far apart -> try to scale above max
        let left2 = identity_transform_at([-10.0, 0.0, 0.0]);
        let right2 = identity_transform_at([10.0, 0.0, 0.0]);
        s.update_two_hands(&left2, &right2);

        assert!(
            s.current_scale() <= 2.0 + EPSILON,
            "Scale should not go above max: {}",
            s.current_scale(),
        );
    }

    #[test]
    fn two_hand_wrong_mode_returns_identity() {
        let mut s = ManipulationState::new();
        // Mode is None
        let left = identity_transform_at([-0.3, 0.0, 0.0]);
        let right = identity_transform_at([0.3, 0.0, 0.0]);
        let result = s.update_two_hands(&left, &right);
        assert!(result.is_identity());
    }

    #[test]
    fn end_resets_mode() {
        let mut s = ManipulationState::new();
        s.begin_rotate(math::QUAT_IDENTITY);
        assert_eq!(s.mode(), ManipulationMode::Rotate);

        s.end();
        assert_eq!(s.mode(), ManipulationMode::None);
    }

    #[test]
    fn end_preserves_scale() {
        let mut s = ManipulationState::new();
        let left = identity_transform_at([-0.3, 0.0, 0.0]);
        let right = identity_transform_at([0.3, 0.0, 0.0]);
        s.begin_two_hand(&left, &right, ManipulationMode::Scale);

        let left2 = identity_transform_at([-0.6, 0.0, 0.0]);
        let right2 = identity_transform_at([0.6, 0.0, 0.0]);
        s.update_two_hands(&left2, &right2);
        let scale_after = s.current_scale();

        s.end();
        assert!(approx_eq(s.current_scale(), scale_after));
    }

    #[test]
    fn reset_clears_scale() {
        let mut s = ManipulationState::new();
        let left = identity_transform_at([-0.3, 0.0, 0.0]);
        let right = identity_transform_at([0.3, 0.0, 0.0]);
        s.begin_two_hand(&left, &right, ManipulationMode::Scale);

        let left2 = identity_transform_at([-0.6, 0.0, 0.0]);
        let right2 = identity_transform_at([0.6, 0.0, 0.0]);
        s.update_two_hands(&left2, &right2);

        s.reset();
        assert!(approx_eq(s.current_scale(), 1.0));
        assert_eq!(s.mode(), ManipulationMode::None);
    }

    #[test]
    fn quat_angle_identity_is_zero() {
        let angle = quat_angle(math::QUAT_IDENTITY);
        assert!(approx_eq(angle, 0.0));
    }

    #[test]
    fn quat_angle_180_degrees() {
        // 180 degrees around Y
        let q = math::quat_normalize([0.0, 1.0, 0.0, 0.0]);
        let angle = quat_angle(q);
        assert!(
            (angle - std::f32::consts::PI).abs() < 0.01,
            "Expected ~PI, got {}",
            angle,
        );
    }

    #[test]
    fn manipulation_mode_equality() {
        assert_eq!(ManipulationMode::None, ManipulationMode::None);
        assert_ne!(ManipulationMode::Rotate, ManipulationMode::Scale);
        assert_ne!(ManipulationMode::Scale, ManipulationMode::RotateAndScale);
    }

    #[test]
    fn rotate_and_scale_mode_produces_both() {
        let mut s = ManipulationState::new();
        let left = identity_transform_at([-0.3, 0.0, 0.0]);
        let right = identity_transform_at([0.3, 0.0, 0.0]);
        s.begin_two_hand(&left, &right, ManipulationMode::RotateAndScale);

        // Change both position and rotation
        let left2 = transform_at([-0.6, 0.0, 0.0], math::quat_normalize([0.0, 0.1, 0.0, 1.0]));
        let right2 = transform_at([0.6, 0.0, 0.0], math::quat_normalize([0.0, 0.1, 0.0, 1.0]));
        let result = s.update_two_hands(&left2, &right2);

        assert_eq!(result.mode, ManipulationMode::RotateAndScale);
        assert!(result.scale_factor > 1.0);
    }
}
