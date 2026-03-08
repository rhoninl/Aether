//! GPU skinning pipeline description types.
//!
//! Defines the data structures needed to describe a compute-shader-based
//! skinning pass: bone matrix palettes, per-vertex skin data, and dispatch
//! configuration.

/// Maximum bones a single vertex can be influenced by.
const MAX_BONES_PER_VERTEX: usize = 4;
/// Default compute shader workgroup size for skinning.
const DEFAULT_SKINNING_WORKGROUP_SIZE: u32 = 64;
/// Default maximum bone count supported by the pipeline.
const DEFAULT_MAX_BONES: u32 = 256;
/// Default maximum vertex count per dispatch.
const DEFAULT_MAX_VERTICES: u32 = 65_536;

/// Skinning algorithm used on the GPU.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkinningMethod {
    /// Standard linear blend skinning (LBS). Fast but can exhibit candy-wrapper
    /// artifacts at high joint rotations.
    LinearBlend,
    /// Dual quaternion skinning. Avoids volume loss at joints but slightly
    /// more expensive.
    DualQuaternion,
}

/// Configuration for a GPU skinning compute pipeline.
#[derive(Debug, Clone)]
pub struct GpuSkinningConfig {
    /// Skinning algorithm.
    pub method: SkinningMethod,
    /// Compute shader workgroup size (threads per group).
    pub workgroup_size: u32,
    /// Maximum number of bones the palette can hold.
    pub max_bones: u32,
    /// Maximum number of vertices per dispatch.
    pub max_vertices: u32,
}

impl Default for GpuSkinningConfig {
    fn default() -> Self {
        Self {
            method: SkinningMethod::LinearBlend,
            workgroup_size: DEFAULT_SKINNING_WORKGROUP_SIZE,
            max_bones: DEFAULT_MAX_BONES,
            max_vertices: DEFAULT_MAX_VERTICES,
        }
    }
}

impl GpuSkinningConfig {
    /// Compute the number of workgroups needed for a given vertex count.
    pub fn workgroup_count(&self, vertex_count: u32) -> u32 {
        (vertex_count + self.workgroup_size - 1) / self.workgroup_size
    }
}

/// A 4x4 matrix stored in column-major order, suitable for GPU upload.
pub type Mat4 = [f32; 16];

/// Identity 4x4 matrix.
pub const MAT4_IDENTITY: Mat4 = [
    1.0, 0.0, 0.0, 0.0, // col 0
    0.0, 1.0, 0.0, 0.0, // col 1
    0.0, 0.0, 1.0, 0.0, // col 2
    0.0, 0.0, 0.0, 1.0, // col 3
];

/// A palette of bone matrices uploaded to the GPU for skinning.
#[derive(Debug, Clone)]
pub struct BoneMatrixPalette {
    /// One 4x4 matrix per bone, in column-major order.
    pub matrices: Vec<Mat4>,
}

impl BoneMatrixPalette {
    /// Create a palette filled with identity matrices.
    pub fn identity(bone_count: usize) -> Self {
        Self {
            matrices: vec![MAT4_IDENTITY; bone_count],
        }
    }

    /// Number of bones in the palette.
    pub fn bone_count(&self) -> usize {
        self.matrices.len()
    }

    /// Get a flat slice of f32 values for GPU buffer upload.
    /// Returns 16 floats per bone.
    pub fn as_flat_slice(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.matrices.len() * 16);
        for m in &self.matrices {
            out.extend_from_slice(m);
        }
        out
    }
}

/// Per-vertex skinning data: bone indices and weights.
#[derive(Debug, Clone, Copy)]
pub struct SkinVertex {
    /// Position in object space.
    pub position: [f32; 3],
    /// Normal in object space.
    pub normal: [f32; 3],
    /// Indices into the bone matrix palette (up to 4).
    pub bone_indices: [u16; MAX_BONES_PER_VERTEX],
    /// Corresponding weights (should sum to 1.0).
    pub bone_weights: [f32; MAX_BONES_PER_VERTEX],
}

impl SkinVertex {
    /// Create a vertex influenced by a single bone.
    pub fn single_bone(position: [f32; 3], normal: [f32; 3], bone_index: u16) -> Self {
        Self {
            position,
            normal,
            bone_indices: [bone_index, 0, 0, 0],
            bone_weights: [1.0, 0.0, 0.0, 0.0],
        }
    }

    /// Validate that bone weights sum to approximately 1.0.
    pub fn weights_valid(&self) -> bool {
        let sum: f32 = self.bone_weights.iter().sum();
        (sum - 1.0).abs() < 0.01
    }
}

/// Descriptor for a single skinning compute dispatch.
#[derive(Debug, Clone)]
pub struct SkinningDispatch {
    /// Number of vertices to process.
    pub vertex_count: u32,
    /// Number of workgroups to dispatch.
    pub workgroup_count: u32,
    /// Number of bones in the palette.
    pub bone_count: u32,
    /// Skinning method for this dispatch.
    pub method: SkinningMethod,
}

impl SkinningDispatch {
    /// Build a dispatch descriptor from a config and vertex/bone counts.
    pub fn new(config: &GpuSkinningConfig, vertex_count: u32, bone_count: u32) -> Self {
        Self {
            vertex_count,
            workgroup_count: config.workgroup_count(vertex_count),
            bone_count,
            method: config.method,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = GpuSkinningConfig::default();
        assert_eq!(config.method, SkinningMethod::LinearBlend);
        assert_eq!(config.workgroup_size, DEFAULT_SKINNING_WORKGROUP_SIZE);
        assert_eq!(config.max_bones, DEFAULT_MAX_BONES);
        assert_eq!(config.max_vertices, DEFAULT_MAX_VERTICES);
    }

    #[test]
    fn test_workgroup_count_exact_multiple() {
        let config = GpuSkinningConfig {
            workgroup_size: 64,
            ..GpuSkinningConfig::default()
        };
        assert_eq!(config.workgroup_count(128), 2);
        assert_eq!(config.workgroup_count(64), 1);
    }

    #[test]
    fn test_workgroup_count_with_remainder() {
        let config = GpuSkinningConfig {
            workgroup_size: 64,
            ..GpuSkinningConfig::default()
        };
        assert_eq!(config.workgroup_count(65), 2);
        assert_eq!(config.workgroup_count(1), 1);
        assert_eq!(config.workgroup_count(127), 2);
    }

    #[test]
    fn test_workgroup_count_zero_vertices() {
        let config = GpuSkinningConfig::default();
        assert_eq!(config.workgroup_count(0), 0);
    }

    #[test]
    fn test_bone_matrix_palette_identity() {
        let palette = BoneMatrixPalette::identity(3);
        assert_eq!(palette.bone_count(), 3);
        for m in &palette.matrices {
            assert_eq!(*m, MAT4_IDENTITY);
        }
    }

    #[test]
    fn test_bone_matrix_palette_empty() {
        let palette = BoneMatrixPalette::identity(0);
        assert_eq!(palette.bone_count(), 0);
        assert!(palette.as_flat_slice().is_empty());
    }

    #[test]
    fn test_bone_matrix_palette_flat_slice_length() {
        let palette = BoneMatrixPalette::identity(5);
        let flat = palette.as_flat_slice();
        assert_eq!(flat.len(), 5 * 16);
    }

    #[test]
    fn test_bone_matrix_palette_flat_slice_values() {
        let palette = BoneMatrixPalette::identity(1);
        let flat = palette.as_flat_slice();
        // Column-major identity: [1,0,0,0, 0,1,0,0, 0,0,1,0, 0,0,0,1]
        assert!((flat[0] - 1.0).abs() < 1e-6);
        assert!((flat[1]).abs() < 1e-6);
        assert!((flat[5] - 1.0).abs() < 1e-6);
        assert!((flat[10] - 1.0).abs() < 1e-6);
        assert!((flat[15] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_skin_vertex_single_bone() {
        let v = SkinVertex::single_bone([1.0, 2.0, 3.0], [0.0, 1.0, 0.0], 5);
        assert_eq!(v.bone_indices[0], 5);
        assert!((v.bone_weights[0] - 1.0).abs() < 1e-6);
        assert!(v.bone_weights[1].abs() < 1e-6);
        assert!(v.weights_valid());
    }

    #[test]
    fn test_skin_vertex_weights_valid() {
        let v = SkinVertex {
            position: [0.0; 3],
            normal: [0.0, 1.0, 0.0],
            bone_indices: [0, 1, 2, 3],
            bone_weights: [0.5, 0.3, 0.15, 0.05],
        };
        assert!(v.weights_valid());
    }

    #[test]
    fn test_skin_vertex_weights_invalid() {
        let v = SkinVertex {
            position: [0.0; 3],
            normal: [0.0, 1.0, 0.0],
            bone_indices: [0, 1, 0, 0],
            bone_weights: [0.5, 0.3, 0.0, 0.0],
        };
        assert!(!v.weights_valid());
    }

    #[test]
    fn test_skinning_dispatch_new() {
        let config = GpuSkinningConfig {
            workgroup_size: 32,
            method: SkinningMethod::DualQuaternion,
            ..GpuSkinningConfig::default()
        };
        let dispatch = SkinningDispatch::new(&config, 100, 50);
        assert_eq!(dispatch.vertex_count, 100);
        assert_eq!(dispatch.workgroup_count, 4); // ceil(100/32)
        assert_eq!(dispatch.bone_count, 50);
        assert_eq!(dispatch.method, SkinningMethod::DualQuaternion);
    }

    #[test]
    fn test_skinning_dispatch_zero_vertices() {
        let config = GpuSkinningConfig::default();
        let dispatch = SkinningDispatch::new(&config, 0, 10);
        assert_eq!(dispatch.vertex_count, 0);
        assert_eq!(dispatch.workgroup_count, 0);
    }

    #[test]
    fn test_skinning_method_equality() {
        assert_eq!(SkinningMethod::LinearBlend, SkinningMethod::LinearBlend);
        assert_ne!(SkinningMethod::LinearBlend, SkinningMethod::DualQuaternion);
    }

    #[test]
    fn test_mat4_identity_diagonal() {
        for i in 0..4 {
            assert!((MAT4_IDENTITY[i * 4 + i] - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn test_mat4_identity_off_diagonal_zero() {
        for col in 0..4 {
            for row in 0..4 {
                if col != row {
                    assert!(MAT4_IDENTITY[col * 4 + row].abs() < 1e-6);
                }
            }
        }
    }

    #[test]
    fn test_bone_matrix_palette_set_custom() {
        let mut palette = BoneMatrixPalette::identity(2);
        let mut custom = MAT4_IDENTITY;
        custom[12] = 5.0; // translate x
        palette.matrices[1] = custom;
        assert!((palette.matrices[1][12] - 5.0).abs() < 1e-6);
        assert_eq!(palette.matrices[0], MAT4_IDENTITY);
    }

    #[test]
    fn test_large_workgroup_count() {
        let config = GpuSkinningConfig {
            workgroup_size: 64,
            max_vertices: 1_000_000,
            ..GpuSkinningConfig::default()
        };
        assert_eq!(config.workgroup_count(1_000_000), 15_625);
    }
}
