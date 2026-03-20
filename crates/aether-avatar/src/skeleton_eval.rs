//! Skeleton evaluation: bone transform computation and matrix palette generation.
//!
//! Converts a skeleton pose (per-bone position, rotation, scale) into a
//! flat array of 4x4 matrices suitable for GPU upload.

use crate::skeleton::{Skeleton, QUAT_IDENTITY};
use crate::skinning::{BoneMatrixPalette, Mat4, MAT4_IDENTITY};

/// Per-bone local transform (position, rotation, scale).
#[derive(Debug, Clone, Copy)]
pub struct BoneTransform {
    /// Local position relative to parent.
    pub position: [f32; 3],
    /// Local rotation as a quaternion [x, y, z, w].
    pub rotation: [f32; 4],
    /// Uniform scale factor.
    pub scale: f32,
}

impl Default for BoneTransform {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            rotation: QUAT_IDENTITY,
            scale: 1.0,
        }
    }
}

impl BoneTransform {
    /// Create a translation-only transform.
    pub fn translation(x: f32, y: f32, z: f32) -> Self {
        Self {
            position: [x, y, z],
            ..Self::default()
        }
    }

    /// Create a rotation-only transform from a quaternion.
    pub fn from_rotation(rotation: [f32; 4]) -> Self {
        Self {
            rotation,
            ..Self::default()
        }
    }

    /// Convert this local transform to a 4x4 matrix (column-major).
    pub fn to_matrix(&self) -> Mat4 {
        let [x, y, z, w] = self.rotation;
        let s = self.scale;
        let [tx, ty, tz] = self.position;

        // Rotation matrix from quaternion, scaled
        let x2 = x + x;
        let y2 = y + y;
        let z2 = z + z;
        let xx = x * x2;
        let xy = x * y2;
        let xz = x * z2;
        let yy = y * y2;
        let yz = y * z2;
        let zz = z * z2;
        let wx = w * x2;
        let wy = w * y2;
        let wz = w * z2;

        [
            // Column 0
            s * (1.0 - (yy + zz)),
            s * (xy + wz),
            s * (xz - wy),
            0.0,
            // Column 1
            s * (xy - wz),
            s * (1.0 - (xx + zz)),
            s * (yz + wx),
            0.0,
            // Column 2
            s * (xz + wy),
            s * (yz - wx),
            s * (1.0 - (xx + yy)),
            0.0,
            // Column 3
            tx,
            ty,
            tz,
            1.0,
        ]
    }
}

/// Multiply two 4x4 matrices (column-major). Result = A * B.
pub fn mat4_mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut result = [0.0f32; 16];
    for col in 0..4 {
        for row in 0..4 {
            let mut sum = 0.0;
            for k in 0..4 {
                sum += a[k * 4 + row] * b[col * 4 + k];
            }
            result[col * 4 + row] = sum;
        }
    }
    result
}

/// A complete skeleton pose: one `BoneTransform` per bone.
#[derive(Debug, Clone)]
pub struct SkeletonPose {
    /// Local transforms for each bone, indexed by bone index.
    pub transforms: Vec<BoneTransform>,
}

impl SkeletonPose {
    /// Create a pose with identity transforms for all bones.
    pub fn identity(bone_count: usize) -> Self {
        Self {
            transforms: vec![BoneTransform::default(); bone_count],
        }
    }

    /// Create a pose from a skeleton's rest positions (uses bone positions
    /// as translations, bone rotations as rotations, scale 1.0).
    pub fn from_skeleton(skeleton: &Skeleton) -> Self {
        let transforms = skeleton
            .bones
            .iter()
            .map(|bone| BoneTransform {
                position: bone.position,
                rotation: bone.rotation,
                scale: 1.0,
            })
            .collect();
        Self { transforms }
    }

    /// Number of bones in the pose.
    pub fn bone_count(&self) -> usize {
        self.transforms.len()
    }

    /// Set the transform for a specific bone.
    pub fn set_transform(&mut self, bone_index: usize, transform: BoneTransform) {
        if bone_index < self.transforms.len() {
            self.transforms[bone_index] = transform;
        }
    }
}

/// Compute world-space transforms by walking the parent chain.
///
/// Returns one world-space 4x4 matrix per bone.
pub fn compute_world_transforms(skeleton: &Skeleton, pose: &SkeletonPose) -> Vec<Mat4> {
    let n = skeleton.bones.len().min(pose.bone_count());
    let mut world_matrices = vec![MAT4_IDENTITY; n];

    for i in 0..n {
        let local = pose.transforms[i].to_matrix();
        world_matrices[i] = match skeleton.bones[i].parent {
            Some(parent_idx) if parent_idx < n => mat4_mul(&world_matrices[parent_idx], &local),
            _ => local,
        };
    }

    world_matrices
}

/// Compute a `BoneMatrixPalette` from a skeleton and a pose.
///
/// This is the main entry point for converting animation output into
/// GPU-ready bone matrices.
pub fn compute_bone_matrices(skeleton: &Skeleton, pose: &SkeletonPose) -> BoneMatrixPalette {
    let world_transforms = compute_world_transforms(skeleton, pose);
    BoneMatrixPalette {
        matrices: world_transforms,
    }
}

/// Compute bone matrices with bind-pose inverse (skinning matrices).
///
/// For each bone: `skinning_matrix = world_transform * inverse_bind_pose`.
/// The bind pose inverses should be precomputed from the skeleton's rest pose.
pub fn compute_skinning_matrices(
    skeleton: &Skeleton,
    pose: &SkeletonPose,
    inverse_bind_poses: &[Mat4],
) -> BoneMatrixPalette {
    let world_transforms = compute_world_transforms(skeleton, pose);
    let n = world_transforms.len().min(inverse_bind_poses.len());
    let matrices: Vec<Mat4> = (0..n)
        .map(|i| mat4_mul(&world_transforms[i], &inverse_bind_poses[i]))
        .collect();
    BoneMatrixPalette { matrices }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skeleton::Bone;

    const EPSILON: f32 = 1e-4;

    fn make_two_bone_skeleton() -> Skeleton {
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
            ],
            bone_names: vec!["root".to_string(), "child".to_string()],
        }
    }

    fn make_three_bone_skeleton() -> Skeleton {
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
                    position: [0.0, 1.0, 0.0],
                    rotation: QUAT_IDENTITY,
                    length: 0.5,
                    parent: Some(1),
                },
            ],
            bone_names: vec!["root".to_string(), "mid".to_string(), "tip".to_string()],
        }
    }

    #[test]
    fn test_bone_transform_default() {
        let t = BoneTransform::default();
        assert_eq!(t.position, [0.0, 0.0, 0.0]);
        assert_eq!(t.rotation, QUAT_IDENTITY);
        assert!((t.scale - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_bone_transform_translation() {
        let t = BoneTransform::translation(1.0, 2.0, 3.0);
        assert!((t.position[0] - 1.0).abs() < EPSILON);
        assert!((t.position[1] - 2.0).abs() < EPSILON);
        assert!((t.position[2] - 3.0).abs() < EPSILON);
        assert_eq!(t.rotation, QUAT_IDENTITY);
    }

    #[test]
    fn test_bone_transform_from_rotation() {
        let q = [0.0, 0.707, 0.0, 0.707_f32];
        let t = BoneTransform::from_rotation(q);
        assert_eq!(t.rotation, q);
        assert_eq!(t.position, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_identity_transform_to_matrix() {
        let t = BoneTransform::default();
        let m = t.to_matrix();
        for i in 0..16 {
            assert!(
                (m[i] - MAT4_IDENTITY[i]).abs() < EPSILON,
                "element {i}: expected {}, got {}",
                MAT4_IDENTITY[i],
                m[i]
            );
        }
    }

    #[test]
    fn test_translation_to_matrix() {
        let t = BoneTransform::translation(5.0, 3.0, -1.0);
        let m = t.to_matrix();
        // Translation is in column 3: m[12], m[13], m[14]
        assert!((m[12] - 5.0).abs() < EPSILON);
        assert!((m[13] - 3.0).abs() < EPSILON);
        assert!((m[14] - (-1.0)).abs() < EPSILON);
        assert!((m[15] - 1.0).abs() < EPSILON);
        // Diagonal should be 1.0 (identity rotation, scale 1)
        assert!((m[0] - 1.0).abs() < EPSILON);
        assert!((m[5] - 1.0).abs() < EPSILON);
        assert!((m[10] - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_scale_to_matrix() {
        let t = BoneTransform {
            scale: 2.0,
            ..BoneTransform::default()
        };
        let m = t.to_matrix();
        // Diagonal should be 2.0
        assert!((m[0] - 2.0).abs() < EPSILON);
        assert!((m[5] - 2.0).abs() < EPSILON);
        assert!((m[10] - 2.0).abs() < EPSILON);
    }

    #[test]
    fn test_90_degree_y_rotation_to_matrix() {
        // 90 degrees around Y axis: quat = [0, sin(45), 0, cos(45)]
        let s = std::f32::consts::FRAC_PI_4.sin();
        let c = std::f32::consts::FRAC_PI_4.cos();
        let t = BoneTransform::from_rotation([0.0, s, 0.0, c]);
        let m = t.to_matrix();
        // After 90-degree Y rotation:
        // X axis -> Z axis (col 0): [0, 0, -1, 0]
        // Z axis -> -X axis (col 2): [1, 0, 0, 0]... actually:
        // For a 90-degree rotation about Y:
        // col 0: [cos(90), 0, -sin(90), 0] = [0, 0, -1, 0]
        // col 2: [sin(90), 0, cos(90), 0] = [1, 0, 0, 0]
        assert!(m[0].abs() < EPSILON); // was cos(90) = 0
        assert!((m[2] - (-1.0)).abs() < EPSILON); // was -sin(90) = -1
        assert!((m[8] - 1.0).abs() < EPSILON); // sin(90) = 1
        assert!(m[10].abs() < EPSILON); // cos(90) = 0
    }

    #[test]
    fn test_mat4_mul_identity() {
        let m = BoneTransform::translation(3.0, 4.0, 5.0).to_matrix();
        let result = mat4_mul(&MAT4_IDENTITY, &m);
        for i in 0..16 {
            assert!(
                (result[i] - m[i]).abs() < EPSILON,
                "element {i}: expected {}, got {}",
                m[i],
                result[i]
            );
        }
    }

    #[test]
    fn test_mat4_mul_translations() {
        let a = BoneTransform::translation(1.0, 0.0, 0.0).to_matrix();
        let b = BoneTransform::translation(0.0, 2.0, 0.0).to_matrix();
        let result = mat4_mul(&a, &b);
        // Combined translation should be (1, 2, 0)
        assert!((result[12] - 1.0).abs() < EPSILON);
        assert!((result[13] - 2.0).abs() < EPSILON);
        assert!((result[14]).abs() < EPSILON);
    }

    #[test]
    fn test_skeleton_pose_identity() {
        let pose = SkeletonPose::identity(5);
        assert_eq!(pose.bone_count(), 5);
        for t in &pose.transforms {
            assert_eq!(t.rotation, QUAT_IDENTITY);
            assert_eq!(t.position, [0.0, 0.0, 0.0]);
        }
    }

    #[test]
    fn test_skeleton_pose_from_skeleton() {
        let skel = make_two_bone_skeleton();
        let pose = SkeletonPose::from_skeleton(&skel);
        assert_eq!(pose.bone_count(), 2);
        assert_eq!(pose.transforms[0].position, [0.0, 0.0, 0.0]);
        assert_eq!(pose.transforms[1].position, [1.0, 0.0, 0.0]);
    }

    #[test]
    fn test_skeleton_pose_set_transform() {
        let mut pose = SkeletonPose::identity(3);
        pose.set_transform(1, BoneTransform::translation(5.0, 0.0, 0.0));
        assert!((pose.transforms[1].position[0] - 5.0).abs() < EPSILON);
    }

    #[test]
    fn test_skeleton_pose_set_out_of_bounds() {
        let mut pose = SkeletonPose::identity(2);
        pose.set_transform(10, BoneTransform::translation(5.0, 0.0, 0.0));
        // Should be a no-op
        assert_eq!(pose.bone_count(), 2);
    }

    #[test]
    fn test_compute_world_transforms_root_only() {
        let skel = Skeleton {
            bones: vec![Bone {
                position: [0.0, 0.0, 0.0],
                rotation: QUAT_IDENTITY,
                length: 1.0,
                parent: None,
            }],
            bone_names: vec!["root".to_string()],
        };
        let pose = SkeletonPose::from_skeleton(&skel);
        let world = compute_world_transforms(&skel, &pose);
        assert_eq!(world.len(), 1);
        for i in 0..16 {
            assert!((world[0][i] - MAT4_IDENTITY[i]).abs() < EPSILON);
        }
    }

    #[test]
    fn test_compute_world_transforms_parent_child() {
        let skel = make_two_bone_skeleton();
        let pose = SkeletonPose::from_skeleton(&skel);
        let world = compute_world_transforms(&skel, &pose);
        assert_eq!(world.len(), 2);
        // Child world position = parent world * child local
        // Parent at origin, child translated [1,0,0] from parent
        // World position of child should be [1,0,0]
        assert!((world[1][12] - 1.0).abs() < EPSILON);
        assert!((world[1][13]).abs() < EPSILON);
    }

    #[test]
    fn test_compute_world_transforms_chain() {
        let skel = make_three_bone_skeleton();
        let pose = SkeletonPose::from_skeleton(&skel);
        let world = compute_world_transforms(&skel, &pose);
        assert_eq!(world.len(), 3);
        // tip: position [0,1,0] relative to mid, which is at [1,0,0] in world
        // So tip world pos should be [1,1,0]
        assert!((world[2][12] - 1.0).abs() < EPSILON);
        assert!((world[2][13] - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_compute_bone_matrices() {
        let skel = make_two_bone_skeleton();
        let pose = SkeletonPose::from_skeleton(&skel);
        let palette = compute_bone_matrices(&skel, &pose);
        assert_eq!(palette.bone_count(), 2);
    }

    #[test]
    fn test_compute_bone_matrices_identity_pose() {
        let skel = make_two_bone_skeleton();
        let pose = SkeletonPose::identity(2);
        let palette = compute_bone_matrices(&skel, &pose);
        // Identity pose means all matrices should be identity
        for m in &palette.matrices {
            for i in 0..16 {
                assert!(
                    (m[i] - MAT4_IDENTITY[i]).abs() < EPSILON,
                    "element {i}: expected {}, got {}",
                    MAT4_IDENTITY[i],
                    m[i]
                );
            }
        }
    }

    #[test]
    fn test_compute_skinning_matrices() {
        let skel = make_two_bone_skeleton();
        let pose = SkeletonPose::from_skeleton(&skel);
        let inverse_binds = vec![MAT4_IDENTITY; 2];
        let palette = compute_skinning_matrices(&skel, &pose, &inverse_binds);
        assert_eq!(palette.bone_count(), 2);
    }

    #[test]
    fn test_compute_skinning_matrices_with_identity_bind() {
        let skel = make_two_bone_skeleton();
        let pose = SkeletonPose::from_skeleton(&skel);
        let inverse_binds = vec![MAT4_IDENTITY; 2];
        let palette = compute_skinning_matrices(&skel, &pose, &inverse_binds);
        // With identity inverse bind, skinning matrices = world transforms
        let world = compute_world_transforms(&skel, &pose);
        for i in 0..2 {
            for j in 0..16 {
                assert!(
                    (palette.matrices[i][j] - world[i][j]).abs() < EPSILON,
                    "bone {i}, element {j}: {} vs {}",
                    palette.matrices[i][j],
                    world[i][j]
                );
            }
        }
    }

    #[test]
    fn test_compute_skinning_matrices_mismatched_lengths() {
        let skel = make_three_bone_skeleton();
        let pose = SkeletonPose::from_skeleton(&skel);
        // Only provide 2 inverse bind poses for 3 bones
        let inverse_binds = vec![MAT4_IDENTITY; 2];
        let palette = compute_skinning_matrices(&skel, &pose, &inverse_binds);
        // Should produce min(3, 2) = 2 matrices
        assert_eq!(palette.bone_count(), 2);
    }

    #[test]
    fn test_pose_with_custom_rotation() {
        let skel = make_two_bone_skeleton();
        let mut pose = SkeletonPose::from_skeleton(&skel);
        // Rotate root 90 degrees around Z
        let s = std::f32::consts::FRAC_PI_4.sin();
        let c = std::f32::consts::FRAC_PI_4.cos();
        pose.set_transform(0, BoneTransform::from_rotation([0.0, 0.0, s, c]));
        let world = compute_world_transforms(&skel, &pose);
        // Root should have the rotation
        // Child inherits parent rotation, so its world position changes
        assert_eq!(world.len(), 2);
    }

    #[test]
    fn test_mat4_mul_associativity() {
        let a = BoneTransform::translation(1.0, 2.0, 3.0).to_matrix();
        let b = BoneTransform::translation(4.0, 5.0, 6.0).to_matrix();
        let c = BoneTransform::translation(7.0, 8.0, 9.0).to_matrix();

        let ab_c = mat4_mul(&mat4_mul(&a, &b), &c);
        let a_bc = mat4_mul(&a, &mat4_mul(&b, &c));

        for i in 0..16 {
            assert!(
                (ab_c[i] - a_bc[i]).abs() < EPSILON,
                "associativity failed at {i}"
            );
        }
    }
}
