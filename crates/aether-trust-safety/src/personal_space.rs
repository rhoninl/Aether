//! Personal space bubble enforcement.
//!
//! Computes push-away displacements when another avatar is too close.

use crate::control::PersonalSpaceBubble;

/// A simple 3D vector for position and displacement calculations.
#[derive(Debug, Clone, PartialEq)]
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
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }

    pub fn length(&self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    pub fn subtract(&self, other: &Vec3) -> Vec3 {
        Vec3 {
            x: self.x - other.x,
            y: self.y - other.y,
            z: self.z - other.z,
        }
    }

    pub fn scale(&self, factor: f32) -> Vec3 {
        Vec3 {
            x: self.x * factor,
            y: self.y * factor,
            z: self.z * factor,
        }
    }

    pub fn normalize(&self) -> Vec3 {
        let len = self.length();
        if len < f32::EPSILON {
            return Vec3::zero();
        }
        self.scale(1.0 / len)
    }
}

/// Default push force multiplier applied when an avatar is inside the bubble.
pub const DEFAULT_PUSH_FORCE: f32 = 1.0;

/// Default personal space radius in meters.
pub const DEFAULT_RADIUS_M: f32 = 0.5;

/// The result of a personal space push computation.
#[derive(Debug, Clone)]
pub struct PushResult {
    /// The id of the avatar being pushed.
    pub target_id: u64,
    /// The displacement vector to apply to push the other avatar away.
    pub displacement: Vec3,
}

/// Compute the push displacement for a single nearby avatar.
///
/// Returns `Some(PushResult)` if the other avatar is within the bubble radius,
/// `None` otherwise (or if the bubble is disabled).
///
/// The displacement magnitude is `push_force * (radius - distance) / radius`,
/// directed away from the bubble owner.
pub fn compute_push(
    bubble: &PersonalSpaceBubble,
    push_force: f32,
    self_pos: &Vec3,
    other_id: u64,
    other_pos: &Vec3,
) -> Option<PushResult> {
    if !bubble.enabled {
        return None;
    }

    let diff = other_pos.subtract(self_pos);
    let distance = diff.length();

    if distance >= bubble.radius_m {
        return None;
    }

    // When two avatars occupy the exact same position, push along +X as fallback.
    let direction = if distance < f32::EPSILON {
        Vec3::new(1.0, 0.0, 0.0)
    } else {
        diff.normalize()
    };

    let penetration_ratio = (bubble.radius_m - distance) / bubble.radius_m;
    let magnitude = push_force * penetration_ratio;

    Some(PushResult {
        target_id: other_id,
        displacement: direction.scale(magnitude),
    })
}

/// Compute push displacements for all nearby avatars.
///
/// `others` is a slice of `(user_id, position)` pairs.
pub fn compute_pushes(
    bubble: &PersonalSpaceBubble,
    push_force: f32,
    self_pos: &Vec3,
    others: &[(u64, Vec3)],
) -> Vec<PushResult> {
    others
        .iter()
        .filter_map(|(id, pos)| compute_push(bubble, push_force, self_pos, *id, pos))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bubble(enabled: bool, radius: f32) -> PersonalSpaceBubble {
        PersonalSpaceBubble {
            enabled,
            radius_m: radius,
        }
    }

    #[test]
    fn disabled_bubble_returns_none() {
        let bubble = make_bubble(false, 1.0);
        let result = compute_push(
            &bubble,
            DEFAULT_PUSH_FORCE,
            &Vec3::zero(),
            1,
            &Vec3::new(0.5, 0.0, 0.0),
        );
        assert!(result.is_none());
    }

    #[test]
    fn outside_radius_returns_none() {
        let bubble = make_bubble(true, 1.0);
        let result = compute_push(
            &bubble,
            DEFAULT_PUSH_FORCE,
            &Vec3::zero(),
            1,
            &Vec3::new(2.0, 0.0, 0.0),
        );
        assert!(result.is_none());
    }

    #[test]
    fn exactly_on_boundary_returns_none() {
        let bubble = make_bubble(true, 1.0);
        let result = compute_push(
            &bubble,
            DEFAULT_PUSH_FORCE,
            &Vec3::zero(),
            1,
            &Vec3::new(1.0, 0.0, 0.0),
        );
        assert!(result.is_none());
    }

    #[test]
    fn inside_radius_returns_push() {
        let bubble = make_bubble(true, 1.0);
        let result = compute_push(
            &bubble,
            DEFAULT_PUSH_FORCE,
            &Vec3::zero(),
            42,
            &Vec3::new(0.5, 0.0, 0.0),
        );
        let push = result.expect("should push");
        assert_eq!(push.target_id, 42);
        // displacement should be in +X direction
        assert!(push.displacement.x > 0.0);
        assert!((push.displacement.y).abs() < f32::EPSILON);
        assert!((push.displacement.z).abs() < f32::EPSILON);
        // magnitude = 1.0 * (1.0 - 0.5) / 1.0 = 0.5
        let mag = push.displacement.length();
        assert!((mag - 0.5).abs() < 0.01);
    }

    #[test]
    fn zero_distance_pushes_along_fallback() {
        let bubble = make_bubble(true, 1.0);
        let result = compute_push(
            &bubble,
            DEFAULT_PUSH_FORCE,
            &Vec3::zero(),
            1,
            &Vec3::zero(),
        );
        let push = result.expect("should push");
        // Should push along +X fallback
        assert!((push.displacement.x - 1.0).abs() < 0.01);
        assert!((push.displacement.y).abs() < f32::EPSILON);
    }

    #[test]
    fn push_force_scales_displacement() {
        let bubble = make_bubble(true, 1.0);
        let result = compute_push(
            &bubble,
            3.0,
            &Vec3::zero(),
            1,
            &Vec3::new(0.5, 0.0, 0.0),
        );
        let push = result.expect("should push");
        // magnitude = 3.0 * (1.0 - 0.5) / 1.0 = 1.5
        let mag = push.displacement.length();
        assert!((mag - 1.5).abs() < 0.01);
    }

    #[test]
    fn push_direction_is_away_from_self() {
        let bubble = make_bubble(true, 2.0);
        let self_pos = Vec3::new(5.0, 0.0, 0.0);
        let other_pos = Vec3::new(4.0, 0.0, 0.0); // to the left
        let result = compute_push(&bubble, 1.0, &self_pos, 1, &other_pos);
        let push = result.expect("should push");
        // Other is at x=4, self at x=5, so push should be in -X
        assert!(push.displacement.x < 0.0);
    }

    #[test]
    fn compute_pushes_filters_multiple() {
        let bubble = make_bubble(true, 1.0);
        let others = vec![
            (1, Vec3::new(0.5, 0.0, 0.0)),  // inside
            (2, Vec3::new(2.0, 0.0, 0.0)),  // outside
            (3, Vec3::new(0.0, 0.3, 0.0)),  // inside
        ];
        let results = compute_pushes(&bubble, 1.0, &Vec3::zero(), &others);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].target_id, 1);
        assert_eq!(results[1].target_id, 3);
    }

    #[test]
    fn diagonal_push() {
        let bubble = make_bubble(true, 2.0);
        let other_pos = Vec3::new(0.5, 0.5, 0.0);
        let result = compute_push(&bubble, 1.0, &Vec3::zero(), 1, &other_pos);
        let push = result.expect("should push");
        // Direction should be roughly 45 degrees in XY plane
        assert!(push.displacement.x > 0.0);
        assert!(push.displacement.y > 0.0);
        assert!((push.displacement.x - push.displacement.y).abs() < 0.01);
    }

    #[test]
    fn vec3_length() {
        let v = Vec3::new(3.0, 4.0, 0.0);
        assert!((v.length() - 5.0).abs() < f32::EPSILON);
    }

    #[test]
    fn vec3_normalize() {
        let v = Vec3::new(0.0, 0.0, 3.0);
        let n = v.normalize();
        assert!((n.z - 1.0).abs() < f32::EPSILON);
        assert!((n.length() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn vec3_normalize_zero_returns_zero() {
        let v = Vec3::zero();
        let n = v.normalize();
        assert!((n.length()).abs() < f32::EPSILON);
    }
}
