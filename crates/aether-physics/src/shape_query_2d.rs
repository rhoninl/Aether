//! 2D shape queries on the XZ plane.
//!
//! These helpers let gameplay code run quick overlap tests (circle vs circle,
//! AABB vs AABB, cone-contains-point) without pulling in the full Rapier query
//! pipeline. All math is pure `f32`.
//!
//! # XZ-plane convention
//!
//! For 3D callers, map world coordinates onto this module's 2D types as:
//!
//! - [`Vec2::x`] = `world.x`
//! - [`Vec2::y`] = `world.z`
//!
//! The world's Y axis (vertical / up) is intentionally ignored. These queries
//! are intended for top-down gameplay checks such as aggro radii, area-of-effect
//! skill shapes, and trigger volumes projected onto the ground plane.
//!
//! # Boundary semantics
//!
//! Overlap tests are inclusive: touching edges count as overlapping. This keeps
//! results stable when objects are exactly aligned on integer-ish coordinates.

/// Minimum squared length below which a direction vector is considered
/// degenerate.
const DIRECTION_EPSILON_SQ: f32 = 1.0e-12;

/// 2D point / vector on the XZ plane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Squared Euclidean distance to another point. Avoids a sqrt when you
    /// only need to compare distances.
    pub fn distance_squared(self, other: Vec2) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        dx * dx + dy * dy
    }

    /// Squared length of this vector.
    pub fn length_squared(self) -> f32 {
        self.x * self.x + self.y * self.y
    }

    /// Dot product.
    pub fn dot(self, other: Vec2) -> f32 {
        self.x * other.x + self.y * other.y
    }
}

/// Circle on the XZ plane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Circle2 {
    pub center: Vec2,
    pub radius: f32,
}

/// Axis-aligned rectangle (AABB) on the XZ plane.
///
/// Invariant: `min.x <= max.x && min.y <= max.y`. Callers are responsible for
/// constructing rects that satisfy this.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect2 {
    pub min: Vec2,
    pub max: Vec2,
}

/// Returns `true` if two circles overlap. Touching circles count as
/// overlapping (inclusive).
pub fn circle_overlap_2d(a: Circle2, b: Circle2) -> bool {
    let radius_sum = a.radius + b.radius;
    a.center.distance_squared(b.center) <= radius_sum * radius_sum
}

/// Returns `true` if two AABBs overlap. Touching edges count as overlapping
/// (inclusive).
pub fn rect_overlap_2d(a: Rect2, b: Rect2) -> bool {
    a.min.x <= b.max.x && a.max.x >= b.min.x && a.min.y <= b.max.y && a.max.y >= b.min.y
}

/// Returns `true` if `point` lies within a 2D cone (circular sector).
///
/// - `origin`: tip of the cone.
/// - `forward`: unit-length direction the cone points in. If the vector is
///   (numerically) zero, the cone is degenerate and no point is contained.
/// - `half_angle_rad`: half of the total cone angle in radians. A value of `0`
///   means the cone collapses to the forward ray; only points exactly on the
///   ray (within f32 rounding) are contained. Values `>= PI` mean a full disk.
/// - `radius`: maximum distance from `origin` for points to be considered
///   inside.
/// - `point`: the query point.
///
/// The cone contains its boundary (inclusive radius and angle).
pub fn cone_contains_point_2d(
    origin: Vec2,
    forward: Vec2,
    half_angle_rad: f32,
    radius: f32,
    point: Vec2,
) -> bool {
    if radius < 0.0 || half_angle_rad < 0.0 {
        return false;
    }

    let to_point = Vec2::new(point.x - origin.x, point.y - origin.y);
    let dist_sq = to_point.length_squared();

    // The origin is always inside a non-empty cone; bail before dividing by
    // zero on the angle test.
    if dist_sq == 0.0 {
        return true;
    }

    if dist_sq > radius * radius {
        return false;
    }

    let forward_len_sq = forward.length_squared();
    if forward_len_sq < DIRECTION_EPSILON_SQ {
        return false;
    }

    // Containment holds iff cos(angle_between) >= cos(half_angle). Compare in
    // squared space to avoid a sqrt, but track the sign of `dot` separately
    // since squaring collapses it.
    let dot = forward.dot(to_point);
    let cos_half = half_angle_rad.cos();
    let cos_half_sq_scaled = cos_half * cos_half * forward_len_sq * dist_sq;
    let dot_sq = dot * dot;

    if cos_half >= 0.0 {
        // Narrow cone (half_angle <= PI/2): the point must be on the forward
        // side of the origin AND within the angular wedge.
        dot >= 0.0 && dot_sq >= cos_half_sq_scaled
    } else {
        // Wide cone (half_angle > PI/2): everything on the forward side is in;
        // behind the origin, the blind wedge grows as |cos_half| grows.
        dot >= 0.0 || dot_sq <= cos_half_sq_scaled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    fn v(x: f32, y: f32) -> Vec2 {
        Vec2::new(x, y)
    }

    #[test]
    fn circle_overlap_intersecting() {
        let a = Circle2 {
            center: v(0.0, 0.0),
            radius: 1.0,
        };
        let b = Circle2 {
            center: v(1.5, 0.0),
            radius: 1.0,
        };
        assert!(circle_overlap_2d(a, b));
    }

    #[test]
    fn circle_overlap_separated() {
        let a = Circle2 {
            center: v(0.0, 0.0),
            radius: 1.0,
        };
        let b = Circle2 {
            center: v(5.0, 0.0),
            radius: 1.0,
        };
        assert!(!circle_overlap_2d(a, b));
    }

    #[test]
    fn circle_overlap_touching_is_inclusive() {
        // Distance between centers = 2.0, sum of radii = 2.0 → touching.
        let a = Circle2 {
            center: v(0.0, 0.0),
            radius: 1.0,
        };
        let b = Circle2 {
            center: v(2.0, 0.0),
            radius: 1.0,
        };
        assert!(circle_overlap_2d(a, b));
    }

    #[test]
    fn circle_overlap_degenerate_zero_radius() {
        // A zero-radius circle acts like a point. Point inside other circle.
        let point = Circle2 {
            center: v(0.5, 0.0),
            radius: 0.0,
        };
        let disk = Circle2 {
            center: v(0.0, 0.0),
            radius: 1.0,
        };
        assert!(circle_overlap_2d(point, disk));

        // Point outside.
        let far_point = Circle2 {
            center: v(2.0, 0.0),
            radius: 0.0,
        };
        assert!(!circle_overlap_2d(far_point, disk));

        // Two zero-radius circles only overlap when exactly coincident.
        let p1 = Circle2 {
            center: v(1.0, 1.0),
            radius: 0.0,
        };
        let p2 = Circle2 {
            center: v(1.0, 1.0),
            radius: 0.0,
        };
        assert!(circle_overlap_2d(p1, p2));
    }

    #[test]
    fn rect_overlap_intersecting() {
        let a = Rect2 {
            min: v(0.0, 0.0),
            max: v(2.0, 2.0),
        };
        let b = Rect2 {
            min: v(1.0, 1.0),
            max: v(3.0, 3.0),
        };
        assert!(rect_overlap_2d(a, b));
    }

    #[test]
    fn rect_overlap_separated() {
        let a = Rect2 {
            min: v(0.0, 0.0),
            max: v(1.0, 1.0),
        };
        let b = Rect2 {
            min: v(2.0, 2.0),
            max: v(3.0, 3.0),
        };
        assert!(!rect_overlap_2d(a, b));
    }

    #[test]
    fn rect_overlap_touching_edge_is_inclusive() {
        let a = Rect2 {
            min: v(0.0, 0.0),
            max: v(1.0, 1.0),
        };
        let b = Rect2 {
            min: v(1.0, 0.0),
            max: v(2.0, 1.0),
        };
        assert!(rect_overlap_2d(a, b));
    }

    #[test]
    fn cone_point_inside_angle_and_radius() {
        let origin = v(0.0, 0.0);
        let forward = v(1.0, 0.0);
        // Point directly in front, well within radius.
        assert!(cone_contains_point_2d(
            origin,
            forward,
            PI / 4.0,
            5.0,
            v(2.0, 0.0)
        ));
        // Point slightly off-axis but inside half-angle.
        assert!(cone_contains_point_2d(
            origin,
            forward,
            PI / 4.0,
            5.0,
            v(2.0, 1.0)
        ));
    }

    #[test]
    fn cone_point_outside_angle() {
        let origin = v(0.0, 0.0);
        let forward = v(1.0, 0.0);
        // Point 90° off the forward axis, inside the radius. Half-angle of 30°
        // should exclude it.
        assert!(!cone_contains_point_2d(
            origin,
            forward,
            PI / 6.0,
            5.0,
            v(0.0, 2.0)
        ));
        // Directly behind origin.
        assert!(!cone_contains_point_2d(
            origin,
            forward,
            PI / 6.0,
            5.0,
            v(-2.0, 0.0)
        ));
    }

    #[test]
    fn cone_point_inside_angle_outside_radius() {
        let origin = v(0.0, 0.0);
        let forward = v(1.0, 0.0);
        // Directly forward, but beyond radius.
        assert!(!cone_contains_point_2d(
            origin,
            forward,
            PI / 2.0,
            1.0,
            v(5.0, 0.0)
        ));
    }

    #[test]
    fn cone_degenerate_half_angle_zero() {
        let origin = v(0.0, 0.0);
        let forward = v(1.0, 0.0);
        // A zero half-angle means only points exactly on the forward ray
        // qualify. A point on the ray within radius is accepted.
        assert!(cone_contains_point_2d(
            origin,
            forward,
            0.0,
            5.0,
            v(3.0, 0.0)
        ));
        // Any off-axis point should be rejected.
        assert!(!cone_contains_point_2d(
            origin,
            forward,
            0.0,
            5.0,
            v(3.0, 0.1)
        ));
    }

    #[test]
    fn cone_origin_point_is_contained() {
        let origin = v(4.0, -2.0);
        let forward = v(0.0, 1.0);
        assert!(cone_contains_point_2d(
            origin,
            forward,
            PI / 4.0,
            1.0,
            origin
        ));
    }

    #[test]
    fn cone_degenerate_forward_rejects_all() {
        let origin = v(0.0, 0.0);
        let forward = v(0.0, 0.0);
        // Off-origin query with zero forward vector: cannot decide direction,
        // return false.
        assert!(!cone_contains_point_2d(
            origin,
            forward,
            PI / 2.0,
            5.0,
            v(1.0, 0.0)
        ));
    }

    #[test]
    fn cone_wide_half_angle_accepts_behind() {
        let origin = v(0.0, 0.0);
        let forward = v(1.0, 0.0);
        // 150° half-angle (total 300°) leaves only a 60° blind wedge straight
        // behind the origin. A point at (-2, 3) is roughly 124° off forward,
        // which is inside the cone.
        let wide = PI * 5.0 / 6.0;
        assert!(cone_contains_point_2d(
            origin,
            forward,
            wide,
            5.0,
            v(-2.0, 3.0)
        ));
        // A point straight behind is exactly 180° off forward, inside the
        // 60° blind wedge, so it must be rejected.
        assert!(!cone_contains_point_2d(
            origin,
            forward,
            wide,
            5.0,
            v(-2.0, 0.0)
        ));
    }

    #[test]
    fn vec2_helpers() {
        let a = v(3.0, 4.0);
        assert_eq!(a.length_squared(), 25.0);
        assert_eq!(a.distance_squared(v(0.0, 0.0)), 25.0);
        assert_eq!(a.dot(v(1.0, 0.0)), 3.0);
    }
}
