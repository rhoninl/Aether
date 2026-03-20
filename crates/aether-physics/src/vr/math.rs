/// Lightweight vector/quaternion math utilities for VR interaction physics.
///
/// These operate on `[f32; 3]` (vectors) and `[f32; 4]` (quaternions in x,y,z,w order)
/// to avoid pulling in a full linear algebra crate for abstraction-level logic.
/// Compute the dot product of two 3D vectors.
pub fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Compute the squared length of a 3D vector.
pub fn length_sq(v: [f32; 3]) -> f32 {
    dot(v, v)
}

/// Compute the length (magnitude) of a 3D vector.
pub fn length(v: [f32; 3]) -> f32 {
    length_sq(v).sqrt()
}

/// Normalize a 3D vector. Returns zero vector if input length is near zero.
pub fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len = length(v);
    if len < f32::EPSILON {
        return [0.0; 3];
    }
    [v[0] / len, v[1] / len, v[2] / len]
}

/// Add two 3D vectors.
pub fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// Subtract vector b from vector a.
pub fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// Scale a 3D vector by a scalar.
pub fn scale(v: [f32; 3], s: f32) -> [f32; 3] {
    [v[0] * s, v[1] * s, v[2] * s]
}

/// Linearly interpolate between two 3D vectors.
pub fn lerp(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// Compute the distance between two 3D points.
pub fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    length(sub(a, b))
}

/// Cross product of two 3D vectors.
pub fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Identity quaternion (no rotation).
pub const QUAT_IDENTITY: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

/// Multiply two quaternions (Hamilton product). Quaternion format: [x, y, z, w].
pub fn quat_mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let (ax, ay, az, aw) = (a[0], a[1], a[2], a[3]);
    let (bx, by, bz, bw) = (b[0], b[1], b[2], b[3]);
    [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]
}

/// Conjugate (inverse for unit quaternions) of a quaternion.
pub fn quat_conjugate(q: [f32; 4]) -> [f32; 4] {
    [-q[0], -q[1], -q[2], q[3]]
}

/// Compute the relative rotation from quaternion `from` to quaternion `to`.
/// Result satisfies: `to = quat_mul(delta, from)`.
pub fn quat_delta(from: [f32; 4], to: [f32; 4]) -> [f32; 4] {
    quat_mul(to, quat_conjugate(from))
}

/// Normalize a quaternion. Returns identity if input length is near zero.
pub fn quat_normalize(q: [f32; 4]) -> [f32; 4] {
    let len_sq = q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3];
    if len_sq < f32::EPSILON {
        return QUAT_IDENTITY;
    }
    let len = len_sq.sqrt();
    [q[0] / len, q[1] / len, q[2] / len, q[3] / len]
}

/// Rotate a 3D vector by a unit quaternion.
pub fn quat_rotate(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let qv = [q[0], q[1], q[2]];
    let w = q[3];
    let t = scale(cross(qv, v), 2.0);
    add(add(v, scale(t, w)), cross(qv, t))
}

/// Clamp a value between min and max.
pub fn clamp(value: f32, min: f32, max: f32) -> f32 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-5;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    fn vec_approx_eq(a: [f32; 3], b: [f32; 3]) -> bool {
        approx_eq(a[0], b[0]) && approx_eq(a[1], b[1]) && approx_eq(a[2], b[2])
    }

    fn quat_approx_eq(a: [f32; 4], b: [f32; 4]) -> bool {
        // Quaternions q and -q represent the same rotation
        let same = approx_eq(a[0], b[0])
            && approx_eq(a[1], b[1])
            && approx_eq(a[2], b[2])
            && approx_eq(a[3], b[3]);
        let negated = approx_eq(a[0], -b[0])
            && approx_eq(a[1], -b[1])
            && approx_eq(a[2], -b[2])
            && approx_eq(a[3], -b[3]);
        same || negated
    }

    #[test]
    fn test_dot_product() {
        assert!(approx_eq(dot([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]), 0.0));
        assert!(approx_eq(dot([1.0, 2.0, 3.0], [4.0, 5.0, 6.0]), 32.0));
        assert!(approx_eq(dot([1.0, 0.0, 0.0], [1.0, 0.0, 0.0]), 1.0));
    }

    #[test]
    fn test_length() {
        assert!(approx_eq(length([3.0, 4.0, 0.0]), 5.0));
        assert!(approx_eq(length([0.0, 0.0, 0.0]), 0.0));
        assert!(approx_eq(length([1.0, 0.0, 0.0]), 1.0));
    }

    #[test]
    fn test_normalize() {
        let n = normalize([3.0, 0.0, 0.0]);
        assert!(vec_approx_eq(n, [1.0, 0.0, 0.0]));

        // Zero vector stays zero
        let z = normalize([0.0, 0.0, 0.0]);
        assert!(vec_approx_eq(z, [0.0, 0.0, 0.0]));
    }

    #[test]
    fn test_add_sub() {
        let a = [1.0, 2.0, 3.0];
        let b = [4.0, 5.0, 6.0];
        assert!(vec_approx_eq(add(a, b), [5.0, 7.0, 9.0]));
        assert!(vec_approx_eq(sub(a, b), [-3.0, -3.0, -3.0]));
    }

    #[test]
    fn test_scale_vector() {
        assert!(vec_approx_eq(scale([1.0, 2.0, 3.0], 2.0), [2.0, 4.0, 6.0]));
        assert!(vec_approx_eq(scale([1.0, 2.0, 3.0], 0.0), [0.0, 0.0, 0.0]));
    }

    #[test]
    fn test_lerp() {
        let a = [0.0, 0.0, 0.0];
        let b = [10.0, 20.0, 30.0];
        assert!(vec_approx_eq(lerp(a, b, 0.0), a));
        assert!(vec_approx_eq(lerp(a, b, 1.0), b));
        assert!(vec_approx_eq(lerp(a, b, 0.5), [5.0, 10.0, 15.0]));
    }

    #[test]
    fn test_distance() {
        assert!(approx_eq(distance([0.0, 0.0, 0.0], [3.0, 4.0, 0.0]), 5.0));
        assert!(approx_eq(distance([1.0, 1.0, 1.0], [1.0, 1.0, 1.0]), 0.0));
    }

    #[test]
    fn test_cross_product() {
        let x = [1.0, 0.0, 0.0];
        let y = [0.0, 1.0, 0.0];
        let z = cross(x, y);
        assert!(vec_approx_eq(z, [0.0, 0.0, 1.0]));

        // Anti-commutative
        let neg_z = cross(y, x);
        assert!(vec_approx_eq(neg_z, [0.0, 0.0, -1.0]));
    }

    #[test]
    fn test_quat_identity_multiplication() {
        let q = [0.0, 0.707, 0.0, 0.707]; // ~90 deg around Y
        let result = quat_mul(QUAT_IDENTITY, q);
        assert!(quat_approx_eq(result, q));

        let result2 = quat_mul(q, QUAT_IDENTITY);
        assert!(quat_approx_eq(result2, q));
    }

    #[test]
    fn test_quat_conjugate() {
        let q = [0.1, 0.2, 0.3, 0.9];
        let conj = quat_conjugate(q);
        assert!(approx_eq(conj[0], -0.1));
        assert!(approx_eq(conj[1], -0.2));
        assert!(approx_eq(conj[2], -0.3));
        assert!(approx_eq(conj[3], 0.9));
    }

    #[test]
    fn test_quat_delta_identity() {
        let q = quat_normalize([0.0, 0.707, 0.0, 0.707]);
        let delta = quat_delta(q, q);
        assert!(quat_approx_eq(delta, QUAT_IDENTITY));
    }

    #[test]
    fn test_quat_normalize() {
        let q = [0.0, 0.0, 0.0, 2.0];
        let n = quat_normalize(q);
        assert!(quat_approx_eq(n, [0.0, 0.0, 0.0, 1.0]));

        // Zero quat returns identity
        let z = quat_normalize([0.0, 0.0, 0.0, 0.0]);
        assert!(quat_approx_eq(z, QUAT_IDENTITY));
    }

    #[test]
    fn test_quat_rotate_vector() {
        // 180 degrees around Y: x -> -x, z -> -z
        let q = quat_normalize([0.0, 1.0, 0.0, 0.0]);
        let v = [1.0, 0.0, 0.0];
        let rotated = quat_rotate(q, v);
        assert!(vec_approx_eq(rotated, [-1.0, 0.0, 0.0]));
    }

    #[test]
    fn test_quat_rotate_identity() {
        let v = [1.0, 2.0, 3.0];
        let rotated = quat_rotate(QUAT_IDENTITY, v);
        assert!(vec_approx_eq(rotated, v));
    }

    #[test]
    fn test_clamp() {
        assert!(approx_eq(clamp(5.0, 0.0, 10.0), 5.0));
        assert!(approx_eq(clamp(-1.0, 0.0, 10.0), 0.0));
        assert!(approx_eq(clamp(15.0, 0.0, 10.0), 10.0));
    }
}
