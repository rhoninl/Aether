//! Mesh data types and LOD generation.

/// A 3D vertex with position, normal, and UV coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

impl Vertex {
    pub fn new(position: [f32; 3], normal: [f32; 3], uv: [f32; 2]) -> Self {
        Self {
            position,
            normal,
            uv,
        }
    }
}

/// A mesh consisting of vertices and triangle indices.
#[derive(Debug, Clone)]
pub struct ImportedMesh {
    pub name: String,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl ImportedMesh {
    /// Number of triangles in this mesh.
    pub fn triangle_count(&self) -> u32 {
        (self.indices.len() / 3) as u32
    }

    /// Approximate size in bytes (vertices + indices).
    pub fn size_bytes(&self) -> u64 {
        let vertex_size = std::mem::size_of::<Vertex>() * self.vertices.len();
        let index_size = std::mem::size_of::<u32>() * self.indices.len();
        (vertex_size + index_size) as u64
    }
}

/// Trait for mesh optimization backends.
pub trait MeshOptimizer {
    /// Generate a simplified mesh at the given target ratio (0.0..=1.0).
    /// A ratio of 1.0 keeps all triangles; 0.5 targets half.
    fn simplify(&self, mesh: &ImportedMesh, target_ratio: f32) -> ImportedMesh;
}

/// A single level in an LOD chain.
#[derive(Debug, Clone)]
pub struct LodLevel {
    pub level: u32,
    pub target_ratio: f32,
    pub mesh: ImportedMesh,
}

/// A chain of LOD levels from highest to lowest detail.
#[derive(Debug, Clone)]
pub struct LodChain {
    pub levels: Vec<LodLevel>,
}

impl LodChain {
    /// Generate an LOD chain from a source mesh using the given optimizer and ratios.
    ///
    /// Ratios should be in descending order (e.g., [1.0, 0.5, 0.25]).
    /// The first ratio is typically 1.0 for the original mesh.
    pub fn generate(mesh: &ImportedMesh, ratios: &[f32], optimizer: &dyn MeshOptimizer) -> Self {
        let levels = ratios
            .iter()
            .enumerate()
            .map(|(i, &ratio)| {
                let simplified = if (ratio - 1.0).abs() < f32::EPSILON {
                    mesh.clone()
                } else {
                    optimizer.simplify(mesh, ratio)
                };
                LodLevel {
                    level: i as u32,
                    target_ratio: ratio,
                    mesh: simplified,
                }
            })
            .collect();

        Self { levels }
    }
}

/// Built-in simple mesh optimizer that performs uniform triangle decimation.
///
/// This is a basic implementation for testing. Production use should
/// swap in meshoptimizer or a quadric error metric implementation.
pub struct SimpleMeshOptimizer;

impl MeshOptimizer for SimpleMeshOptimizer {
    fn simplify(&self, mesh: &ImportedMesh, target_ratio: f32) -> ImportedMesh {
        let target_ratio = target_ratio.clamp(0.0, 1.0);

        if mesh.indices.len() < 3 {
            return mesh.clone();
        }

        let triangle_count = mesh.indices.len() / 3;
        let target_triangles = ((triangle_count as f32 * target_ratio).ceil() as usize).max(1);

        // Keep every Nth triangle to reach the target count
        let step = if target_triangles >= triangle_count {
            1
        } else {
            triangle_count / target_triangles
        };

        let mut new_indices = Vec::new();
        for i in (0..mesh.indices.len()).step_by(3 * step) {
            if i + 2 < mesh.indices.len() {
                new_indices.push(mesh.indices[i]);
                new_indices.push(mesh.indices[i + 1]);
                new_indices.push(mesh.indices[i + 2]);
            }
        }

        // Remap vertices: only include referenced vertices
        let mut used_vertices = std::collections::HashSet::new();
        for &idx in &new_indices {
            used_vertices.insert(idx);
        }

        let mut vertex_map = std::collections::HashMap::new();
        let mut new_vertices = Vec::new();
        for &old_idx in &new_indices {
            if let std::collections::hash_map::Entry::Vacant(e) = vertex_map.entry(old_idx) {
                let new_idx = new_vertices.len() as u32;
                e.insert(new_idx);
                new_vertices.push(mesh.vertices[old_idx as usize].clone());
            }
        }

        let remapped_indices: Vec<u32> = new_indices.iter().map(|idx| vertex_map[idx]).collect();

        ImportedMesh {
            name: format!("{}_lod", mesh.name),
            vertices: new_vertices,
            indices: remapped_indices,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_triangle() -> ImportedMesh {
        ImportedMesh {
            name: "triangle".to_string(),
            vertices: vec![
                Vertex::new([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0]),
                Vertex::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0]),
                Vertex::new([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0]),
            ],
            indices: vec![0, 1, 2],
        }
    }

    fn make_quad_mesh() -> ImportedMesh {
        ImportedMesh {
            name: "quad".to_string(),
            vertices: vec![
                Vertex::new([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0]),
                Vertex::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0]),
                Vertex::new([1.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 1.0]),
                Vertex::new([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0]),
            ],
            indices: vec![0, 1, 2, 0, 2, 3],
        }
    }

    fn make_large_mesh(triangle_count: usize) -> ImportedMesh {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        for i in 0..triangle_count {
            let base = (i * 3) as u32;
            let x = i as f32;
            vertices.push(Vertex::new([x, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0]));
            vertices.push(Vertex::new(
                [x + 1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [1.0, 0.0],
            ));
            vertices.push(Vertex::new(
                [x + 0.5, 1.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.5, 1.0],
            ));
            indices.push(base);
            indices.push(base + 1);
            indices.push(base + 2);
        }
        ImportedMesh {
            name: "large_mesh".to_string(),
            vertices,
            indices,
        }
    }

    #[test]
    fn vertex_creation() {
        let v = Vertex::new([1.0, 2.0, 3.0], [0.0, 1.0, 0.0], [0.5, 0.5]);
        assert_eq!(v.position, [1.0, 2.0, 3.0]);
        assert_eq!(v.normal, [0.0, 1.0, 0.0]);
        assert_eq!(v.uv, [0.5, 0.5]);
    }

    #[test]
    fn vertex_equality() {
        let v1 = Vertex::new([1.0, 2.0, 3.0], [0.0, 1.0, 0.0], [0.5, 0.5]);
        let v2 = Vertex::new([1.0, 2.0, 3.0], [0.0, 1.0, 0.0], [0.5, 0.5]);
        assert_eq!(v1, v2);
    }

    #[test]
    fn triangle_count_single() {
        let mesh = make_triangle();
        assert_eq!(mesh.triangle_count(), 1);
    }

    #[test]
    fn triangle_count_quad() {
        let mesh = make_quad_mesh();
        assert_eq!(mesh.triangle_count(), 2);
    }

    #[test]
    fn mesh_size_bytes_nonzero() {
        let mesh = make_triangle();
        assert!(mesh.size_bytes() > 0);
    }

    #[test]
    fn simplify_ratio_one_preserves_mesh() {
        let mesh = make_quad_mesh();
        let optimizer = SimpleMeshOptimizer;
        let result = optimizer.simplify(&mesh, 1.0);
        assert_eq!(result.triangle_count(), mesh.triangle_count());
    }

    #[test]
    fn simplify_reduces_triangle_count() {
        let mesh = make_large_mesh(100);
        assert_eq!(mesh.triangle_count(), 100);
        let optimizer = SimpleMeshOptimizer;
        let result = optimizer.simplify(&mesh, 0.5);
        assert!(result.triangle_count() < mesh.triangle_count());
        assert!(result.triangle_count() > 0);
    }

    #[test]
    fn simplify_very_low_ratio_still_produces_mesh() {
        let mesh = make_large_mesh(100);
        let optimizer = SimpleMeshOptimizer;
        let result = optimizer.simplify(&mesh, 0.01);
        assert!(result.triangle_count() >= 1);
        assert!(result.indices.len() % 3 == 0);
    }

    #[test]
    fn simplify_clamps_ratio_above_one() {
        let mesh = make_large_mesh(10);
        let optimizer = SimpleMeshOptimizer;
        let result = optimizer.simplify(&mesh, 2.0);
        assert_eq!(result.triangle_count(), mesh.triangle_count());
    }

    #[test]
    fn simplify_clamps_ratio_below_zero() {
        let mesh = make_large_mesh(10);
        let optimizer = SimpleMeshOptimizer;
        let result = optimizer.simplify(&mesh, -1.0);
        assert!(result.triangle_count() >= 1);
    }

    #[test]
    fn simplify_preserves_valid_indices() {
        let mesh = make_large_mesh(50);
        let optimizer = SimpleMeshOptimizer;
        let result = optimizer.simplify(&mesh, 0.5);
        for &idx in &result.indices {
            assert!((idx as usize) < result.vertices.len());
        }
    }

    #[test]
    fn simplify_single_triangle_mesh() {
        let mesh = make_triangle();
        let optimizer = SimpleMeshOptimizer;
        let result = optimizer.simplify(&mesh, 0.5);
        assert!(result.triangle_count() >= 1);
    }

    #[test]
    fn lod_chain_generation() {
        let mesh = make_large_mesh(100);
        let optimizer = SimpleMeshOptimizer;
        let ratios = vec![1.0, 0.5, 0.25];
        let chain = LodChain::generate(&mesh, &ratios, &optimizer);
        assert_eq!(chain.levels.len(), 3);
    }

    #[test]
    fn lod_chain_decreasing_detail() {
        let mesh = make_large_mesh(100);
        let optimizer = SimpleMeshOptimizer;
        let ratios = vec![1.0, 0.5, 0.25];
        let chain = LodChain::generate(&mesh, &ratios, &optimizer);

        for i in 1..chain.levels.len() {
            assert!(
                chain.levels[i].mesh.triangle_count() <= chain.levels[i - 1].mesh.triangle_count(),
                "LOD level {} should have fewer or equal triangles than level {}",
                i,
                i - 1
            );
        }
    }

    #[test]
    fn lod_chain_level_indices() {
        let mesh = make_large_mesh(20);
        let optimizer = SimpleMeshOptimizer;
        let ratios = vec![1.0, 0.5];
        let chain = LodChain::generate(&mesh, &ratios, &optimizer);
        assert_eq!(chain.levels[0].level, 0);
        assert_eq!(chain.levels[1].level, 1);
    }

    #[test]
    fn lod_chain_first_level_is_original() {
        let mesh = make_large_mesh(20);
        let optimizer = SimpleMeshOptimizer;
        let ratios = vec![1.0, 0.5];
        let chain = LodChain::generate(&mesh, &ratios, &optimizer);
        assert_eq!(chain.levels[0].mesh.triangle_count(), mesh.triangle_count());
    }

    #[test]
    fn lod_level_target_ratio_stored() {
        let mesh = make_large_mesh(20);
        let optimizer = SimpleMeshOptimizer;
        let ratios = vec![1.0, 0.75, 0.25];
        let chain = LodChain::generate(&mesh, &ratios, &optimizer);
        assert!((chain.levels[0].target_ratio - 1.0).abs() < f32::EPSILON);
        assert!((chain.levels[1].target_ratio - 0.75).abs() < f32::EPSILON);
        assert!((chain.levels[2].target_ratio - 0.25).abs() < f32::EPSILON);
    }

    #[test]
    fn empty_mesh_simplify() {
        let mesh = ImportedMesh {
            name: "empty".to_string(),
            vertices: vec![],
            indices: vec![],
        };
        let optimizer = SimpleMeshOptimizer;
        let result = optimizer.simplify(&mesh, 0.5);
        assert_eq!(result.triangle_count(), 0);
    }

    #[test]
    fn simplified_mesh_name_has_lod_suffix() {
        let mesh = make_quad_mesh();
        let optimizer = SimpleMeshOptimizer;
        let result = optimizer.simplify(&mesh, 0.5);
        assert!(result.name.contains("lod"));
    }
}
