//! Blend shape (morph target) GPU evaluation types.
//!
//! Defines targets, weight maps, and compute dispatch configuration for
//! evaluating blend shapes on the GPU.

/// Default compute shader workgroup size for blend shape evaluation.
const DEFAULT_BLEND_WORKGROUP_SIZE: u32 = 64;
/// Default maximum active blend shape targets per dispatch.
const DEFAULT_MAX_ACTIVE_TARGETS: u32 = 16;
/// Maximum weight value for a single blend shape target.
const MAX_WEIGHT: f32 = 1.0;
/// Minimum weight value for a single blend shape target.
const MIN_WEIGHT: f32 = 0.0;
/// Threshold below which a weight is considered inactive.
const WEIGHT_EPSILON: f32 = 0.001;

/// A single vertex delta for a blend shape target.
#[derive(Debug, Clone, Copy)]
pub struct BlendShapeVertexDelta {
    /// Vertex index this delta applies to.
    pub vertex_index: u32,
    /// Position offset to add when this target is fully active.
    pub position_delta: [f32; 3],
    /// Normal offset to add when this target is fully active.
    pub normal_delta: [f32; 3],
}

/// A named blend shape target with its vertex deltas.
#[derive(Debug, Clone)]
pub struct BlendShapeTarget {
    /// Human-readable name (e.g., "smile", "browRaise", "viseme_aa").
    pub name: String,
    /// Index of this target in the blend shape set.
    pub index: u32,
    /// Vertex deltas for this target.
    pub deltas: Vec<BlendShapeVertexDelta>,
}

impl BlendShapeTarget {
    /// Number of vertices affected by this target.
    pub fn affected_vertex_count(&self) -> usize {
        self.deltas.len()
    }
}

/// A collection of blend shape targets for a single mesh.
#[derive(Debug, Clone)]
pub struct BlendShapeSet {
    /// All available blend shape targets.
    pub targets: Vec<BlendShapeTarget>,
    /// Maximum number of targets that can be active simultaneously.
    pub max_active: u32,
}

impl BlendShapeSet {
    /// Create an empty set.
    pub fn new(max_active: u32) -> Self {
        Self {
            targets: Vec::new(),
            max_active,
        }
    }

    /// Add a target to the set.
    pub fn add_target(&mut self, target: BlendShapeTarget) {
        self.targets.push(target);
    }

    /// Number of targets in the set.
    pub fn target_count(&self) -> usize {
        self.targets.len()
    }

    /// Find a target by name.
    pub fn find_target(&self, name: &str) -> Option<&BlendShapeTarget> {
        self.targets.iter().find(|t| t.name == name)
    }
}

/// Current blend shape weights, keyed by target index.
#[derive(Debug, Clone)]
pub struct BlendShapeWeights {
    /// Weight per target index. Missing entries are treated as 0.0.
    weights: Vec<f32>,
}

impl BlendShapeWeights {
    /// Create weights for a given number of targets, all set to zero.
    pub fn zeroed(target_count: usize) -> Self {
        Self {
            weights: vec![0.0; target_count],
        }
    }

    /// Set the weight for a target index, clamped to [0.0, 1.0].
    pub fn set(&mut self, target_index: usize, weight: f32) {
        if target_index < self.weights.len() {
            self.weights[target_index] = weight.clamp(MIN_WEIGHT, MAX_WEIGHT);
        }
    }

    /// Get the weight for a target index.
    pub fn get(&self, target_index: usize) -> f32 {
        self.weights.get(target_index).copied().unwrap_or(0.0)
    }

    /// Count how many targets have a non-trivial weight.
    pub fn active_count(&self) -> usize {
        self.weights.iter().filter(|w| **w > WEIGHT_EPSILON).count()
    }

    /// Get the indices of active targets sorted by weight (descending).
    pub fn active_indices_sorted(&self) -> Vec<usize> {
        let mut active: Vec<(usize, f32)> = self
            .weights
            .iter()
            .enumerate()
            .filter(|(_, w)| **w > WEIGHT_EPSILON)
            .map(|(i, w)| (i, *w))
            .collect();
        active.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        active.into_iter().map(|(i, _)| i).collect()
    }

    /// Get the raw weight slice for GPU upload.
    pub fn as_slice(&self) -> &[f32] {
        &self.weights
    }

    /// Number of target slots.
    pub fn len(&self) -> usize {
        self.weights.len()
    }

    /// Whether the weight set is empty.
    pub fn is_empty(&self) -> bool {
        self.weights.is_empty()
    }
}

/// Configuration for GPU blend shape evaluation.
#[derive(Debug, Clone)]
pub struct GpuBlendShapeConfig {
    /// Maximum number of simultaneously active targets.
    pub max_active_targets: u32,
    /// Compute shader workgroup size.
    pub workgroup_size: u32,
}

impl Default for GpuBlendShapeConfig {
    fn default() -> Self {
        Self {
            max_active_targets: DEFAULT_MAX_ACTIVE_TARGETS,
            workgroup_size: DEFAULT_BLEND_WORKGROUP_SIZE,
        }
    }
}

impl GpuBlendShapeConfig {
    /// Compute the number of workgroups needed for a given vertex count.
    pub fn workgroup_count(&self, vertex_count: u32) -> u32 {
        vertex_count.div_ceil(self.workgroup_size)
    }
}

/// Descriptor for a single blend shape compute dispatch.
#[derive(Debug, Clone)]
pub struct BlendShapeDispatch {
    /// Number of active targets in this dispatch.
    pub active_target_count: u32,
    /// Total vertex count of the mesh.
    pub vertex_count: u32,
    /// Number of workgroups to dispatch.
    pub workgroup_count: u32,
}

impl BlendShapeDispatch {
    /// Build a dispatch descriptor from config, weights, and vertex count.
    pub fn new(
        config: &GpuBlendShapeConfig,
        weights: &BlendShapeWeights,
        vertex_count: u32,
    ) -> Self {
        let active = weights.active_count() as u32;
        let capped = active.min(config.max_active_targets);
        Self {
            active_target_count: capped,
            vertex_count,
            workgroup_count: config.workgroup_count(vertex_count),
        }
    }

    /// Whether this dispatch is a no-op (no active targets).
    pub fn is_noop(&self) -> bool {
        self.active_target_count == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blend_shape_target_affected_count() {
        let target = BlendShapeTarget {
            name: "smile".to_string(),
            index: 0,
            deltas: vec![
                BlendShapeVertexDelta {
                    vertex_index: 0,
                    position_delta: [0.1, 0.0, 0.0],
                    normal_delta: [0.0, 0.0, 0.0],
                },
                BlendShapeVertexDelta {
                    vertex_index: 1,
                    position_delta: [0.0, 0.1, 0.0],
                    normal_delta: [0.0, 0.0, 0.0],
                },
            ],
        };
        assert_eq!(target.affected_vertex_count(), 2);
    }

    #[test]
    fn test_blend_shape_target_empty() {
        let target = BlendShapeTarget {
            name: "empty".to_string(),
            index: 0,
            deltas: vec![],
        };
        assert_eq!(target.affected_vertex_count(), 0);
    }

    #[test]
    fn test_blend_shape_set_add_and_find() {
        let mut set = BlendShapeSet::new(8);
        set.add_target(BlendShapeTarget {
            name: "smile".to_string(),
            index: 0,
            deltas: vec![],
        });
        set.add_target(BlendShapeTarget {
            name: "frown".to_string(),
            index: 1,
            deltas: vec![],
        });

        assert_eq!(set.target_count(), 2);
        assert!(set.find_target("smile").is_some());
        assert!(set.find_target("frown").is_some());
        assert!(set.find_target("wink").is_none());
    }

    #[test]
    fn test_blend_shape_set_empty() {
        let set = BlendShapeSet::new(4);
        assert_eq!(set.target_count(), 0);
        assert!(set.find_target("anything").is_none());
    }

    #[test]
    fn test_blend_shape_weights_zeroed() {
        let weights = BlendShapeWeights::zeroed(5);
        assert_eq!(weights.len(), 5);
        assert!(!weights.is_empty());
        assert_eq!(weights.active_count(), 0);
        for i in 0..5 {
            assert!((weights.get(i)).abs() < 1e-6);
        }
    }

    #[test]
    fn test_blend_shape_weights_empty() {
        let weights = BlendShapeWeights::zeroed(0);
        assert!(weights.is_empty());
        assert_eq!(weights.active_count(), 0);
    }

    #[test]
    fn test_blend_shape_weights_set_and_get() {
        let mut weights = BlendShapeWeights::zeroed(3);
        weights.set(0, 0.5);
        weights.set(2, 1.0);
        assert!((weights.get(0) - 0.5).abs() < 1e-6);
        assert!((weights.get(1)).abs() < 1e-6);
        assert!((weights.get(2) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_blend_shape_weights_clamp_above() {
        let mut weights = BlendShapeWeights::zeroed(2);
        weights.set(0, 2.0);
        assert!((weights.get(0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_blend_shape_weights_clamp_below() {
        let mut weights = BlendShapeWeights::zeroed(2);
        weights.set(0, -0.5);
        assert!((weights.get(0)).abs() < 1e-6);
    }

    #[test]
    fn test_blend_shape_weights_out_of_bounds_set() {
        let mut weights = BlendShapeWeights::zeroed(2);
        weights.set(5, 1.0); // should be a no-op
        assert!((weights.get(5)).abs() < 1e-6);
    }

    #[test]
    fn test_blend_shape_weights_active_count() {
        let mut weights = BlendShapeWeights::zeroed(5);
        weights.set(0, 0.8);
        weights.set(2, 0.3);
        weights.set(4, 0.0001); // below epsilon, not active
        assert_eq!(weights.active_count(), 2);
    }

    #[test]
    fn test_blend_shape_weights_active_indices_sorted() {
        let mut weights = BlendShapeWeights::zeroed(4);
        weights.set(0, 0.3);
        weights.set(1, 0.9);
        weights.set(3, 0.5);

        let sorted = weights.active_indices_sorted();
        assert_eq!(sorted, vec![1, 3, 0]);
    }

    #[test]
    fn test_blend_shape_weights_active_indices_empty() {
        let weights = BlendShapeWeights::zeroed(3);
        assert!(weights.active_indices_sorted().is_empty());
    }

    #[test]
    fn test_blend_shape_weights_as_slice() {
        let mut weights = BlendShapeWeights::zeroed(3);
        weights.set(1, 0.5);
        let slice = weights.as_slice();
        assert_eq!(slice.len(), 3);
        assert!((slice[1] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_gpu_blend_shape_config_default() {
        let config = GpuBlendShapeConfig::default();
        assert_eq!(config.max_active_targets, DEFAULT_MAX_ACTIVE_TARGETS);
        assert_eq!(config.workgroup_size, DEFAULT_BLEND_WORKGROUP_SIZE);
    }

    #[test]
    fn test_gpu_blend_shape_config_workgroup_count() {
        let config = GpuBlendShapeConfig {
            workgroup_size: 32,
            ..GpuBlendShapeConfig::default()
        };
        assert_eq!(config.workgroup_count(64), 2);
        assert_eq!(config.workgroup_count(65), 3);
        assert_eq!(config.workgroup_count(0), 0);
    }

    #[test]
    fn test_blend_shape_dispatch_new() {
        let config = GpuBlendShapeConfig {
            max_active_targets: 8,
            workgroup_size: 64,
        };
        let mut weights = BlendShapeWeights::zeroed(10);
        weights.set(0, 0.5);
        weights.set(3, 1.0);
        weights.set(7, 0.2);

        let dispatch = BlendShapeDispatch::new(&config, &weights, 1000);
        assert_eq!(dispatch.active_target_count, 3);
        assert_eq!(dispatch.vertex_count, 1000);
        assert_eq!(dispatch.workgroup_count, 16); // ceil(1000/64)
    }

    #[test]
    fn test_blend_shape_dispatch_caps_active() {
        let config = GpuBlendShapeConfig {
            max_active_targets: 2,
            workgroup_size: 64,
        };
        let mut weights = BlendShapeWeights::zeroed(5);
        weights.set(0, 1.0);
        weights.set(1, 1.0);
        weights.set(2, 1.0);
        weights.set(3, 1.0);

        let dispatch = BlendShapeDispatch::new(&config, &weights, 100);
        assert_eq!(dispatch.active_target_count, 2); // capped at max_active_targets
    }

    #[test]
    fn test_blend_shape_dispatch_noop() {
        let config = GpuBlendShapeConfig::default();
        let weights = BlendShapeWeights::zeroed(5);
        let dispatch = BlendShapeDispatch::new(&config, &weights, 100);
        assert!(dispatch.is_noop());
    }

    #[test]
    fn test_blend_shape_dispatch_not_noop() {
        let config = GpuBlendShapeConfig::default();
        let mut weights = BlendShapeWeights::zeroed(5);
        weights.set(0, 0.5);
        let dispatch = BlendShapeDispatch::new(&config, &weights, 100);
        assert!(!dispatch.is_noop());
    }
}
