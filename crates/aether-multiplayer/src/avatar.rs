//! Avatar state representation for VR multiplayer.

use serde::{Deserialize, Serialize};

/// Maximum movement speed in units per tick for input validation.
const MAX_MOVEMENT_SPEED: f32 = 10.0;

/// Avatar state representing a VR player's tracked body parts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AvatarState {
    pub head_position: [f32; 3],
    pub head_rotation: [f32; 4],
    pub left_hand_position: [f32; 3],
    pub left_hand_rotation: [f32; 4],
    pub right_hand_position: [f32; 3],
    pub right_hand_rotation: [f32; 4],
}

impl Default for AvatarState {
    fn default() -> Self {
        Self {
            head_position: [0.0, 1.7, 0.0],
            head_rotation: [0.0, 0.0, 0.0, 1.0],
            left_hand_position: [-0.3, 1.0, -0.3],
            left_hand_rotation: [0.0, 0.0, 0.0, 1.0],
            right_hand_position: [0.3, 1.0, -0.3],
            right_hand_rotation: [0.0, 0.0, 0.0, 1.0],
        }
    }
}

impl AvatarState {
    /// Validate the avatar state, clamping values to reasonable ranges.
    /// Returns true if any values were clamped.
    pub fn validate_and_clamp(&mut self) -> bool {
        let mut clamped = false;
        clamped |= normalize_quaternion(&mut self.head_rotation);
        clamped |= normalize_quaternion(&mut self.left_hand_rotation);
        clamped |= normalize_quaternion(&mut self.right_hand_rotation);
        clamped |= clamp_position(&mut self.head_position);
        clamped |= clamp_position(&mut self.left_hand_position);
        clamped |= clamp_position(&mut self.right_hand_position);
        clamped
    }

    /// Check if the movement from a previous state exceeds the speed limit.
    pub fn exceeds_speed_limit(&self, previous: &AvatarState) -> bool {
        let dx = self.head_position[0] - previous.head_position[0];
        let dy = self.head_position[1] - previous.head_position[1];
        let dz = self.head_position[2] - previous.head_position[2];
        let distance = (dx * dx + dy * dy + dz * dz).sqrt();
        distance > MAX_MOVEMENT_SPEED
    }

    /// Linearly interpolate between two avatar states.
    pub fn lerp(a: &AvatarState, b: &AvatarState, t: f32) -> AvatarState {
        let t = t.clamp(0.0, 1.0);
        AvatarState {
            head_position: lerp_vec3(&a.head_position, &b.head_position, t),
            head_rotation: lerp_quat(&a.head_rotation, &b.head_rotation, t),
            left_hand_position: lerp_vec3(&a.left_hand_position, &b.left_hand_position, t),
            left_hand_rotation: lerp_quat(&a.left_hand_rotation, &b.left_hand_rotation, t),
            right_hand_position: lerp_vec3(&a.right_hand_position, &b.right_hand_position, t),
            right_hand_rotation: lerp_quat(&a.right_hand_rotation, &b.right_hand_rotation, t),
        }
    }

    /// Convert avatar state to an EntityState for use with StateSyncManager.
    pub fn to_entity_state(
        &self,
        entity_id: u64,
        tick: u64,
    ) -> aether_world_runtime::EntityState {
        aether_world_runtime::EntityState {
            entity_id,
            position: self.head_position,
            rotation: self.head_rotation,
            velocity: [0.0, 0.0, 0.0],
            tick,
        }
    }
}

/// Normalize a quaternion in place. Returns true if normalization was needed.
fn normalize_quaternion(q: &mut [f32; 4]) -> bool {
    let len_sq = q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3];
    if (len_sq - 1.0).abs() > 0.001 {
        let len = len_sq.sqrt();
        if len > 1e-10 {
            q[0] /= len;
            q[1] /= len;
            q[2] /= len;
            q[3] /= len;
        } else {
            *q = [0.0, 0.0, 0.0, 1.0];
        }
        return true;
    }
    false
}

/// Clamp position coordinates to a reasonable world range.
/// Returns true if any coordinate was clamped.
fn clamp_position(pos: &mut [f32; 3]) -> bool {
    const WORLD_BOUND: f32 = 10_000.0;
    let mut clamped = false;
    for coord in pos.iter_mut() {
        if coord.is_nan() || coord.is_infinite() {
            *coord = 0.0;
            clamped = true;
        } else if *coord > WORLD_BOUND {
            *coord = WORLD_BOUND;
            clamped = true;
        } else if *coord < -WORLD_BOUND {
            *coord = -WORLD_BOUND;
            clamped = true;
        }
    }
    clamped
}

fn lerp_vec3(a: &[f32; 3], b: &[f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn lerp_quat(a: &[f32; 4], b: &[f32; 4], t: f32) -> [f32; 4] {
    let mut result = [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ];
    let len = (result[0] * result[0]
        + result[1] * result[1]
        + result[2] * result[2]
        + result[3] * result[3])
        .sqrt();
    if len > 1e-10 {
        result[0] /= len;
        result[1] /= len;
        result[2] /= len;
        result[3] /= len;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_avatar_has_identity_rotations() {
        let avatar = AvatarState::default();
        assert_eq!(avatar.head_rotation, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(avatar.left_hand_rotation, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(avatar.right_hand_rotation, [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn default_avatar_head_at_standing_height() {
        let avatar = AvatarState::default();
        assert!((avatar.head_position[1] - 1.7).abs() < 0.01);
    }

    #[test]
    fn validate_normalizes_unnormalized_quaternion() {
        let mut avatar = AvatarState::default();
        avatar.head_rotation = [0.0, 0.0, 0.0, 2.0]; // not unit length
        let clamped = avatar.validate_and_clamp();
        assert!(clamped);
        let len = avatar.head_rotation.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((len - 1.0).abs() < 0.01);
    }

    #[test]
    fn validate_clamps_nan_position() {
        let mut avatar = AvatarState::default();
        avatar.head_position = [f32::NAN, 0.0, 0.0];
        let clamped = avatar.validate_and_clamp();
        assert!(clamped);
        assert_eq!(avatar.head_position[0], 0.0);
    }

    #[test]
    fn validate_clamps_infinite_position() {
        let mut avatar = AvatarState::default();
        avatar.head_position = [f32::INFINITY, 0.0, 0.0];
        let clamped = avatar.validate_and_clamp();
        assert!(clamped);
        assert!(avatar.head_position[0].is_finite());
    }

    #[test]
    fn validate_clamps_out_of_bounds_position() {
        let mut avatar = AvatarState::default();
        avatar.head_position = [20_000.0, -20_000.0, 0.0];
        let clamped = avatar.validate_and_clamp();
        assert!(clamped);
        assert!(avatar.head_position[0] <= 10_000.0);
        assert!(avatar.head_position[1] >= -10_000.0);
    }

    #[test]
    fn validate_does_not_clamp_valid_state() {
        let mut avatar = AvatarState::default();
        let clamped = avatar.validate_and_clamp();
        assert!(!clamped);
    }

    #[test]
    fn exceeds_speed_limit_detects_teleport() {
        let previous = AvatarState::default();
        let mut current = AvatarState::default();
        current.head_position = [100.0, 1.7, 0.0];
        assert!(current.exceeds_speed_limit(&previous));
    }

    #[test]
    fn normal_movement_within_speed_limit() {
        let previous = AvatarState::default();
        let mut current = AvatarState::default();
        current.head_position[0] += 0.1;
        assert!(!current.exceeds_speed_limit(&previous));
    }

    #[test]
    fn lerp_at_zero_returns_first() {
        let a = AvatarState::default();
        let mut b = AvatarState::default();
        b.head_position = [10.0, 10.0, 10.0];
        let result = AvatarState::lerp(&a, &b, 0.0);
        assert_eq!(result.head_position, a.head_position);
    }

    #[test]
    fn lerp_at_one_returns_second() {
        let a = AvatarState::default();
        let mut b = AvatarState::default();
        b.head_position = [10.0, 10.0, 10.0];
        let result = AvatarState::lerp(&a, &b, 1.0);
        for i in 0..3 {
            assert!((result.head_position[i] - b.head_position[i]).abs() < 0.01);
        }
    }

    #[test]
    fn lerp_midpoint() {
        let a = AvatarState::default();
        let mut b = AvatarState::default();
        b.head_position = [10.0, 1.7, 0.0];
        let result = AvatarState::lerp(&a, &b, 0.5);
        assert!((result.head_position[0] - 5.0).abs() < 0.01);
    }

    #[test]
    fn to_entity_state_uses_head_position() {
        let avatar = AvatarState::default();
        let entity = avatar.to_entity_state(42, 10);
        assert_eq!(entity.entity_id, 42);
        assert_eq!(entity.position, avatar.head_position);
        assert_eq!(entity.rotation, avatar.head_rotation);
        assert_eq!(entity.tick, 10);
    }

    #[test]
    fn serialization_roundtrip() {
        let avatar = AvatarState::default();
        let bytes = bincode::serialize(&avatar).unwrap();
        let deserialized: AvatarState = bincode::deserialize(&bytes).unwrap();
        assert_eq!(avatar, deserialized);
    }

    #[test]
    fn lerp_clamps_t_above_one() {
        let a = AvatarState::default();
        let mut b = AvatarState::default();
        b.head_position = [10.0, 1.7, 0.0];
        let result = AvatarState::lerp(&a, &b, 2.0);
        for i in 0..3 {
            assert!((result.head_position[i] - b.head_position[i]).abs() < 0.01);
        }
    }

    #[test]
    fn lerp_clamps_t_below_zero() {
        let a = AvatarState::default();
        let mut b = AvatarState::default();
        b.head_position = [10.0, 1.7, 0.0];
        let result = AvatarState::lerp(&a, &b, -1.0);
        assert_eq!(result.head_position, a.head_position);
    }
}
