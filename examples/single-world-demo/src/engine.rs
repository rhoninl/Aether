//! Engine configuration, procedural mesh generation, and scene setup
//! for the single-world integration demo.

use std::str::FromStr;

use aether_renderer::gpu::material::{MaterialId, PbrMaterial};
use aether_renderer::gpu::mesh::{MeshId, Vertex};
use aether_renderer::gpu::GpuRenderer;

/// Default window width in pixels.
pub const DEFAULT_WINDOW_WIDTH: u32 = 1280;
/// Default window height in pixels.
pub const DEFAULT_WINDOW_HEIGHT: u32 = 720;

/// Default server address for multiplayer.
const DEFAULT_SERVER_ADDR: &str = "127.0.0.1:7777";

/// Floor plane half-extent.
const FLOOR_SIZE: f32 = 20.0;
/// Floor plane subdivisions per axis.
const FLOOR_SUBDIVISIONS: u32 = 4;
/// Cube half-extent.
const CUBE_SIZE: f32 = 1.0;
/// Sphere radius.
const SPHERE_RADIUS: f32 = 1.0;
/// Sphere stacks.
const SPHERE_STACKS: u32 = 16;
/// Sphere sectors.
const SPHERE_SECTORS: u32 = 32;
/// Minimum subdivision count for plane generation.
const MIN_SUBDIVISIONS: u32 = 1;

/// Read an environment variable, parsing it into T, or return a default.
pub fn env_var_or<T: FromStr>(name: &str, default: T) -> T {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Engine configuration derived from environment variables.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub window_width: u32,
    pub window_height: u32,
    pub server_addr: String,
    pub offline_mode: bool,
}

impl EngineConfig {
    /// Build configuration from environment variables with defaults.
    pub fn from_env() -> Self {
        let window_width = env_var_or("AETHER_WINDOW_WIDTH", DEFAULT_WINDOW_WIDTH);
        let window_height = env_var_or("AETHER_WINDOW_HEIGHT", DEFAULT_WINDOW_HEIGHT);
        let server_addr = std::env::var("AETHER_SERVER_ADDR")
            .unwrap_or_else(|_| DEFAULT_SERVER_ADDR.to_string());
        let offline_mode = std::env::var("AETHER_OFFLINE")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(true);

        Self {
            window_width,
            window_height,
            server_addr,
            offline_mode,
        }
    }
}

/// Holds the mesh and material IDs created during scene setup.
#[derive(Debug, Clone)]
pub struct SceneSetup {
    pub floor_mesh: MeshId,
    pub floor_material: MaterialId,
    pub cube_mesh: MeshId,
    pub cube_material: MaterialId,
    pub sphere_mesh: MeshId,
    pub sphere_material: MaterialId,
}

/// Generate a flat plane on the XZ plane, centered at origin.
///
/// `size` is the half-extent (total width = size * 2).
/// `subdivisions` is the number of quads per axis (clamped to at least 1).
pub fn generate_plane_vertices(size: f32, subdivisions: u32) -> (Vec<Vertex>, Vec<u32>) {
    let subdivisions = subdivisions.max(MIN_SUBDIVISIONS);
    let step = (size * 2.0) / subdivisions as f32;
    let vert_count = (subdivisions + 1) as usize;

    let mut vertices = Vec::with_capacity(vert_count * vert_count);
    let mut indices = Vec::with_capacity((subdivisions * subdivisions * 6) as usize);

    for z in 0..=subdivisions {
        for x in 0..=subdivisions {
            let px = -size + x as f32 * step;
            let pz = -size + z as f32 * step;
            let u = x as f32 / subdivisions as f32;
            let v = z as f32 / subdivisions as f32;
            vertices.push(Vertex {
                position: [px, 0.0, pz],
                normal: [0.0, 1.0, 0.0],
                uv: [u, v],
            });
        }
    }

    for z in 0..subdivisions {
        for x in 0..subdivisions {
            let top_left = z * (subdivisions + 1) + x;
            let top_right = top_left + 1;
            let bottom_left = (z + 1) * (subdivisions + 1) + x;
            let bottom_right = bottom_left + 1;

            indices.push(top_left);
            indices.push(bottom_left);
            indices.push(top_right);

            indices.push(top_right);
            indices.push(bottom_left);
            indices.push(bottom_right);
        }
    }

    (vertices, indices)
}

/// Generate a cube mesh with per-face normals (24 vertices, 36 indices).
///
/// `size` is the half-extent of the cube.
pub fn generate_cube_vertices(size: f32) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(36);

    // Face definitions: (normal, tangent_u, tangent_v)
    let faces: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
        // +Y (top)
        ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        // -Y (bottom)
        ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
        // +X (right)
        ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
        // -X (left)
        ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        // +Z (front)
        ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        // -Z (back)
        ([0.0, 0.0, -1.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
    ];

    for (face_idx, (normal, u_dir, v_dir)) in faces.iter().enumerate() {
        let base = (face_idx * 4) as u32;

        let offsets: [(f32, f32, f32, f32); 4] = [
            (-1.0, -1.0, 0.0, 0.0),
            (1.0, -1.0, 1.0, 0.0),
            (1.0, 1.0, 1.0, 1.0),
            (-1.0, 1.0, 0.0, 1.0),
        ];

        for (ou, ov, tex_u, tex_v) in &offsets {
            let position = [
                (normal[0] + u_dir[0] * ou + v_dir[0] * ov) * size,
                (normal[1] + u_dir[1] * ou + v_dir[1] * ov) * size,
                (normal[2] + u_dir[2] * ou + v_dir[2] * ov) * size,
            ];
            vertices.push(Vertex {
                position,
                normal: *normal,
                uv: [*tex_u, *tex_v],
            });
        }

        indices.push(base);
        indices.push(base + 1);
        indices.push(base + 2);

        indices.push(base);
        indices.push(base + 2);
        indices.push(base + 3);
    }

    (vertices, indices)
}

/// Generate a UV sphere mesh.
///
/// `radius` scales the sphere. `stacks` and `sectors` control tessellation
/// (clamped to minimums of 2 and 3 respectively).
pub fn generate_sphere_vertices(
    radius: f32,
    stacks: u32,
    sectors: u32,
) -> (Vec<Vertex>, Vec<u32>) {
    let stacks = stacks.max(2);
    let sectors = sectors.max(3);

    let mut vertices = Vec::with_capacity(((stacks + 1) * (sectors + 1)) as usize);
    let mut indices = Vec::new();

    let pi = std::f32::consts::PI;
    let two_pi = 2.0 * pi;

    for i in 0..=stacks {
        let stack_angle = pi / 2.0 - (i as f32) * pi / (stacks as f32);
        let xy = stack_angle.cos();
        let y = stack_angle.sin();

        for j in 0..=sectors {
            let sector_angle = (j as f32) * two_pi / (sectors as f32);
            let x = xy * sector_angle.cos();
            let z = xy * sector_angle.sin();

            let u = j as f32 / sectors as f32;
            let v = i as f32 / stacks as f32;

            vertices.push(Vertex {
                position: [x * radius, y * radius, z * radius],
                normal: [x, y, z],
                uv: [u, v],
            });
        }
    }

    for i in 0..stacks {
        for j in 0..sectors {
            let first = i * (sectors + 1) + j;
            let second = first + sectors + 1;

            if i != 0 {
                indices.push(first);
                indices.push(second);
                indices.push(first + 1);
            }
            if i != stacks - 1 {
                indices.push(first + 1);
                indices.push(second);
                indices.push(second + 1);
            }
        }
    }

    (vertices, indices)
}

/// Upload floor, cube, and sphere meshes + materials to the renderer.
pub fn setup_scene(renderer: &mut GpuRenderer) -> SceneSetup {
    // Floor
    let (floor_verts, floor_idx) = generate_plane_vertices(FLOOR_SIZE, FLOOR_SUBDIVISIONS);
    let floor_mesh = renderer.upload_mesh(&floor_verts, &floor_idx);
    let floor_mat_id = renderer.material_manager.allocate_id();
    let floor_material = renderer.upload_material(
        PbrMaterial::new(floor_mat_id)
            .with_albedo(0.4, 0.4, 0.4, 1.0)
            .with_roughness(0.9)
            .with_metallic(0.0),
    );

    // Cube
    let (cube_verts, cube_idx) = generate_cube_vertices(CUBE_SIZE);
    let cube_mesh = renderer.upload_mesh(&cube_verts, &cube_idx);
    let cube_mat_id = renderer.material_manager.allocate_id();
    let cube_material = renderer.upload_material(
        PbrMaterial::new(cube_mat_id)
            .with_albedo(0.8, 0.3, 0.2, 1.0)
            .with_roughness(0.5)
            .with_metallic(0.1),
    );

    // Sphere
    let (sphere_verts, sphere_idx) =
        generate_sphere_vertices(SPHERE_RADIUS, SPHERE_STACKS, SPHERE_SECTORS);
    let sphere_mesh = renderer.upload_mesh(&sphere_verts, &sphere_idx);
    let sphere_mat_id = renderer.material_manager.allocate_id();
    let sphere_material = renderer.upload_material(
        PbrMaterial::new(sphere_mat_id)
            .with_albedo(0.2, 0.5, 0.8, 1.0)
            .with_roughness(0.3)
            .with_metallic(0.6),
    );

    SceneSetup {
        floor_mesh,
        floor_material,
        cube_mesh,
        cube_material,
        sphere_mesh,
        sphere_material,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- EngineConfig tests ----

    #[test]
    fn engine_config_from_env_defaults() {
        let config = EngineConfig::from_env();
        assert!(config.window_width > 0);
        assert!(config.window_height > 0);
        assert!(!config.server_addr.is_empty());
    }

    #[test]
    fn engine_config_default_dimensions() {
        // Without env vars set, should use defaults
        let config = EngineConfig::from_env();
        // May differ if env vars are set, but width/height should be reasonable
        assert!(config.window_width >= 320);
        assert!(config.window_height >= 240);
    }

    #[test]
    fn engine_config_clone() {
        let config = EngineConfig::from_env();
        let cloned = config.clone();
        assert_eq!(cloned.window_width, config.window_width);
        assert_eq!(cloned.window_height, config.window_height);
        assert_eq!(cloned.server_addr, config.server_addr);
        assert_eq!(cloned.offline_mode, config.offline_mode);
    }

    // ---- env_var_or tests ----

    #[test]
    fn env_var_or_returns_default_when_unset() {
        let val: u32 = env_var_or("AETHER_TEST_NONEXISTENT_VAR_12345", 42);
        assert_eq!(val, 42);
    }

    #[test]
    fn env_var_or_returns_default_for_string() {
        let val: String = env_var_or(
            "AETHER_TEST_NONEXISTENT_STR_12345",
            "fallback".to_string(),
        );
        assert_eq!(val, "fallback");
    }

    // ---- Plane generation tests ----

    #[test]
    fn plane_vertex_count() {
        let (verts, _) = generate_plane_vertices(5.0, 4);
        // (4+1)*(4+1) = 25
        assert_eq!(verts.len(), 25);
    }

    #[test]
    fn plane_index_count() {
        let (_, indices) = generate_plane_vertices(5.0, 4);
        // 4*4*6 = 96
        assert_eq!(indices.len(), 96);
    }

    #[test]
    fn plane_normals_point_up() {
        let (verts, _) = generate_plane_vertices(1.0, 2);
        for v in &verts {
            assert_eq!(v.normal, [0.0, 1.0, 0.0]);
        }
    }

    #[test]
    fn plane_uvs_in_range() {
        let (verts, _) = generate_plane_vertices(10.0, 8);
        for v in &verts {
            assert!(v.uv[0] >= 0.0 && v.uv[0] <= 1.0);
            assert!(v.uv[1] >= 0.0 && v.uv[1] <= 1.0);
        }
    }

    #[test]
    fn plane_indices_in_bounds() {
        let (verts, indices) = generate_plane_vertices(5.0, 4);
        let max_idx = verts.len() as u32;
        for &idx in &indices {
            assert!(idx < max_idx, "index {idx} out of bounds (max {max_idx})");
        }
    }

    #[test]
    fn plane_min_subdivisions_clamped() {
        let (verts, indices) = generate_plane_vertices(1.0, 0);
        // Clamped to 1: (1+1)*(1+1) = 4, 1*1*6 = 6
        assert_eq!(verts.len(), 4);
        assert_eq!(indices.len(), 6);
    }

    #[test]
    fn plane_centered_at_origin() {
        let (verts, _) = generate_plane_vertices(5.0, 2);
        for v in &verts {
            assert!((v.position[1]).abs() < f32::EPSILON);
        }
        let min_x = verts.iter().map(|v| v.position[0]).fold(f32::MAX, f32::min);
        let max_x = verts.iter().map(|v| v.position[0]).fold(f32::MIN, f32::max);
        assert!((min_x + 5.0).abs() < f32::EPSILON);
        assert!((max_x - 5.0).abs() < f32::EPSILON);
    }

    // ---- Cube generation tests ----

    #[test]
    fn cube_vertex_count() {
        let (verts, _) = generate_cube_vertices(1.0);
        assert_eq!(verts.len(), 24);
    }

    #[test]
    fn cube_index_count() {
        let (_, indices) = generate_cube_vertices(1.0);
        assert_eq!(indices.len(), 36);
    }

    #[test]
    fn cube_normals_are_unit_vectors() {
        let (verts, _) = generate_cube_vertices(1.0);
        for v in &verts {
            let len = (v.normal[0].powi(2) + v.normal[1].powi(2) + v.normal[2].powi(2)).sqrt();
            assert!((len - 1.0).abs() < 1e-5, "normal not unit: len={len}");
        }
    }

    #[test]
    fn cube_has_six_distinct_normals() {
        let (verts, _) = generate_cube_vertices(1.0);
        let mut normals = std::collections::HashSet::new();
        for v in &verts {
            let key = (
                (v.normal[0] * 10.0) as i32,
                (v.normal[1] * 10.0) as i32,
                (v.normal[2] * 10.0) as i32,
            );
            normals.insert(key);
        }
        assert_eq!(normals.len(), 6);
    }

    #[test]
    fn cube_indices_in_bounds() {
        let (verts, indices) = generate_cube_vertices(1.0);
        let max_idx = verts.len() as u32;
        for &idx in &indices {
            assert!(idx < max_idx);
        }
    }

    #[test]
    fn cube_uvs_in_range() {
        let (verts, _) = generate_cube_vertices(1.0);
        for v in &verts {
            assert!(v.uv[0] >= 0.0 && v.uv[0] <= 1.0);
            assert!(v.uv[1] >= 0.0 && v.uv[1] <= 1.0);
        }
    }

    #[test]
    fn cube_scaled_positions() {
        let (verts, _) = generate_cube_vertices(2.0);
        for v in &verts {
            for p in &v.position {
                assert!(p.abs() <= 2.0 + 1e-5);
            }
        }
    }

    // ---- Sphere generation tests ----

    #[test]
    fn sphere_vertex_count() {
        let (verts, _) = generate_sphere_vertices(1.0, 8, 16);
        // (8+1)*(16+1) = 153
        assert_eq!(verts.len(), 153);
    }

    #[test]
    fn sphere_normals_are_unit_vectors() {
        let (verts, _) = generate_sphere_vertices(1.0, 8, 16);
        for v in &verts {
            let len = (v.normal[0].powi(2) + v.normal[1].powi(2) + v.normal[2].powi(2)).sqrt();
            assert!((len - 1.0).abs() < 1e-4, "normal not unit: len={len}");
        }
    }

    #[test]
    fn sphere_positions_on_surface() {
        let (verts, _) = generate_sphere_vertices(2.0, 8, 16);
        for v in &verts {
            let r = (v.position[0].powi(2) + v.position[1].powi(2) + v.position[2].powi(2)).sqrt();
            assert!((r - 2.0).abs() < 1e-4, "not on sphere surface: r={r}");
        }
    }

    #[test]
    fn sphere_indices_in_bounds() {
        let (verts, indices) = generate_sphere_vertices(1.0, 8, 16);
        let max_idx = verts.len() as u32;
        for &idx in &indices {
            assert!(idx < max_idx, "index {idx} out of bounds");
        }
    }

    #[test]
    fn sphere_uvs_in_range() {
        let (verts, _) = generate_sphere_vertices(1.0, 8, 16);
        for v in &verts {
            assert!(v.uv[0] >= 0.0 && v.uv[0] <= 1.0);
            assert!(v.uv[1] >= 0.0 && v.uv[1] <= 1.0);
        }
    }

    #[test]
    fn sphere_clamps_min_params() {
        let (verts, indices) = generate_sphere_vertices(1.0, 1, 2);
        // stacks clamped to 2, sectors clamped to 3
        assert_eq!(verts.len(), ((2 + 1) * (3 + 1)) as usize);
        assert!(!indices.is_empty());
    }

    #[test]
    fn sphere_has_poles() {
        let (verts, _) = generate_sphere_vertices(1.0, 8, 16);
        let has_top = verts.iter().any(|v| (v.position[1] - 1.0).abs() < 1e-4);
        let has_bottom = verts.iter().any(|v| (v.position[1] + 1.0).abs() < 1e-4);
        assert!(has_top, "sphere missing top pole");
        assert!(has_bottom, "sphere missing bottom pole");
    }

    // ---- SceneSetup type tests ----

    #[test]
    fn scene_setup_is_clone() {
        fn assert_clone<T: Clone>() {}
        assert_clone::<SceneSetup>();
    }

    #[test]
    fn scene_setup_is_debug() {
        fn assert_debug<T: std::fmt::Debug>() {}
        assert_debug::<SceneSetup>();
    }

    // ---- Constants tests ----

    #[test]
    fn default_window_dimensions() {
        assert_eq!(DEFAULT_WINDOW_WIDTH, 1280);
        assert_eq!(DEFAULT_WINDOW_HEIGHT, 720);
    }
}
