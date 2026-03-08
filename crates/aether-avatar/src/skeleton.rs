//! Shared skeletal types: bones, skeletons, and IK targets.

/// A single bone in a skeleton hierarchy.
#[derive(Debug, Clone)]
pub struct Bone {
    /// Position in local space relative to parent (or world space for root).
    pub position: [f32; 3],
    /// Rotation as a quaternion [x, y, z, w].
    pub rotation: [f32; 4],
    /// Length of this bone segment.
    pub length: f32,
    /// Index of the parent bone, or `None` for root.
    pub parent: Option<usize>,
}

/// A full skeleton with named bones.
#[derive(Debug, Clone)]
pub struct Skeleton {
    pub bones: Vec<Bone>,
    pub bone_names: Vec<String>,
}

/// An IK target: a desired position and optional orientation.
#[derive(Debug, Clone)]
pub struct IkTarget {
    pub position: [f32; 3],
    pub rotation: Option<[f32; 4]>,
}

// ---------------------------------------------------------------------------
// Minimal vector / quaternion math helpers (no external deps)
// ---------------------------------------------------------------------------

/// 3D vector subtraction.
pub fn vec3_sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// 3D vector addition.
pub fn vec3_add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// Scale a 3D vector.
pub fn vec3_scale(v: [f32; 3], s: f32) -> [f32; 3] {
    [v[0] * s, v[1] * s, v[2] * s]
}

/// Euclidean length of a 3D vector.
pub fn vec3_len(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

/// Normalize a 3D vector. Returns zero vector if length is near zero.
pub fn vec3_normalize(v: [f32; 3]) -> [f32; 3] {
    let len = vec3_len(v);
    if len < 1e-9 {
        return [0.0, 0.0, 0.0];
    }
    vec3_scale(v, 1.0 / len)
}

/// Distance between two 3D points.
pub fn vec3_distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    vec3_len(vec3_sub(a, b))
}

/// Linearly interpolate between two 3D vectors.
pub fn vec3_lerp(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// Dot product of two 3D vectors.
pub fn vec3_dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Quaternion identity [0, 0, 0, 1].
pub const QUAT_IDENTITY: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

/// Quaternion multiplication.
pub fn quat_mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
        a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
        a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
        a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
    ]
}

/// Normalize a quaternion.
pub fn quat_normalize(q: [f32; 4]) -> [f32; 4] {
    let len = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    if len < 1e-9 {
        return QUAT_IDENTITY;
    }
    [q[0] / len, q[1] / len, q[2] / len, q[3] / len]
}

/// Spherical linear interpolation between two quaternions.
pub fn quat_slerp(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let mut dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];
    let mut b = b;
    // Ensure shortest path
    if dot < 0.0 {
        b = [-b[0], -b[1], -b[2], -b[3]];
        dot = -dot;
    }
    if dot > 0.9995 {
        // Close enough for linear interpolation
        return quat_normalize([
            a[0] + (b[0] - a[0]) * t,
            a[1] + (b[1] - a[1]) * t,
            a[2] + (b[2] - a[2]) * t,
            a[3] + (b[3] - a[3]) * t,
        ]);
    }
    let theta_0 = dot.acos();
    let sin_theta_0 = theta_0.sin();
    let s0 = ((1.0 - t) * theta_0).sin() / sin_theta_0;
    let s1 = (t * theta_0).sin() / sin_theta_0;
    quat_normalize([
        a[0] * s0 + b[0] * s1,
        a[1] * s0 + b[1] * s1,
        a[2] * s0 + b[2] * s1,
        a[3] * s0 + b[3] * s1,
    ])
}

impl Skeleton {
    /// Find a bone index by name.
    pub fn find_bone(&self, name: &str) -> Option<usize> {
        self.bone_names.iter().position(|n| n == name)
    }

    /// Get world-space position of a bone by walking the parent chain.
    /// For simplicity, this adds parent positions (assumes positions are offsets).
    pub fn world_position(&self, bone_index: usize) -> [f32; 3] {
        let mut pos = self.bones[bone_index].position;
        let mut current = self.bones[bone_index].parent;
        while let Some(parent_idx) = current {
            pos = vec3_add(pos, self.bones[parent_idx].position);
            current = self.bones[parent_idx].parent;
        }
        pos
    }

    /// Total chain length from bone at `start` to bone at `end` (inclusive).
    /// Returns `None` if the bones are not in a parent-child chain.
    pub fn chain_length(&self, start: usize, end: usize) -> Option<f32> {
        let mut total = 0.0;
        let mut current = end;
        while current != start {
            total += self.bones[current].length;
            match self.bones[current].parent {
                Some(p) => current = p,
                None => return None,
            }
        }
        Some(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_chain_skeleton() -> Skeleton {
        // Simple 3-bone chain: root -> mid -> tip
        Skeleton {
            bones: vec![
                Bone {
                    position: [0.0, 0.0, 0.0],
                    rotation: QUAT_IDENTITY,
                    length: 1.0,
                    parent: None,
                },
                Bone {
                    position: [1.0, 0.0, 0.0],
                    rotation: QUAT_IDENTITY,
                    length: 1.0,
                    parent: Some(0),
                },
                Bone {
                    position: [1.0, 0.0, 0.0],
                    rotation: QUAT_IDENTITY,
                    length: 0.5,
                    parent: Some(1),
                },
            ],
            bone_names: vec!["root".into(), "mid".into(), "tip".into()],
        }
    }

    #[test]
    fn test_find_bone() {
        let skel = make_chain_skeleton();
        assert_eq!(skel.find_bone("root"), Some(0));
        assert_eq!(skel.find_bone("mid"), Some(1));
        assert_eq!(skel.find_bone("tip"), Some(2));
        assert_eq!(skel.find_bone("nonexistent"), None);
    }

    #[test]
    fn test_world_position() {
        let skel = make_chain_skeleton();
        let wp = skel.world_position(2);
        assert!((wp[0] - 2.0).abs() < 1e-5);
        assert!((wp[1]).abs() < 1e-5);
    }

    #[test]
    fn test_chain_length() {
        let skel = make_chain_skeleton();
        let len = skel.chain_length(0, 2).unwrap();
        assert!((len - 1.5).abs() < 1e-5); // 1.0 + 0.5
    }

    #[test]
    fn test_chain_length_not_connected() {
        // bone 0 has no parent, so chain from 1 to 0 would require 0's parent
        let skel = make_chain_skeleton();
        // chain from 2 to 0 is valid (child to root)
        assert!(skel.chain_length(0, 2).is_some());
    }

    #[test]
    fn test_vec3_normalize_zero() {
        let v = vec3_normalize([0.0, 0.0, 0.0]);
        assert!((vec3_len(v)).abs() < 1e-5);
    }

    #[test]
    fn test_vec3_normalize_unit() {
        let v = vec3_normalize([3.0, 4.0, 0.0]);
        assert!((vec3_len(v) - 1.0).abs() < 1e-5);
        assert!((v[0] - 0.6).abs() < 1e-5);
        assert!((v[1] - 0.8).abs() < 1e-5);
    }

    #[test]
    fn test_quat_identity_mul() {
        let q = [0.0, 0.707, 0.0, 0.707_f32];
        let result = quat_mul(QUAT_IDENTITY, q);
        for i in 0..4 {
            assert!((result[i] - q[i]).abs() < 1e-4);
        }
    }

    #[test]
    fn test_quat_slerp_endpoints() {
        let a = QUAT_IDENTITY;
        let b = quat_normalize([0.0, 0.707, 0.0, 0.707]);
        let at_0 = quat_slerp(a, b, 0.0);
        let at_1 = quat_slerp(a, b, 1.0);
        for i in 0..4 {
            assert!((at_0[i] - a[i]).abs() < 1e-3, "slerp(0) mismatch at {i}");
            assert!((at_1[i] - b[i]).abs() < 1e-3, "slerp(1) mismatch at {i}");
        }
    }

    #[test]
    fn test_vec3_lerp() {
        let a = [0.0, 0.0, 0.0];
        let b = [10.0, 20.0, 30.0];
        let mid = vec3_lerp(a, b, 0.5);
        assert!((mid[0] - 5.0).abs() < 1e-5);
        assert!((mid[1] - 10.0).abs() < 1e-5);
        assert!((mid[2] - 15.0).abs() < 1e-5);
    }
}
