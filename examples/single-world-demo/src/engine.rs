//! Engine configuration, procedural mesh generation, and scene setup
//! for the single-world integration demo.

use std::str::FromStr;

use aether_renderer::gpu::material::{MaterialId, PbrMaterial};
use aether_renderer::gpu::mesh::MeshId;
use aether_renderer::gpu::GpuRenderer;
use aether_renderer::primitives;

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
        let server_addr =
            std::env::var("AETHER_SERVER_ADDR").unwrap_or_else(|_| DEFAULT_SERVER_ADDR.to_string());
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

/// Upload floor, cube, and sphere meshes + materials to the renderer.
pub fn setup_scene(renderer: &mut GpuRenderer) -> SceneSetup {
    // Floor
    let (floor_verts, floor_idx) = primitives::generate_plane(FLOOR_SIZE, FLOOR_SUBDIVISIONS);
    let floor_mesh = renderer.upload_mesh(&floor_verts, &floor_idx);
    let floor_mat_id = renderer.material_manager.allocate_id();
    let floor_material = renderer.upload_material(
        PbrMaterial::new(floor_mat_id)
            .with_albedo(0.4, 0.4, 0.4, 1.0)
            .with_roughness(0.9)
            .with_metallic(0.0),
    );

    // Cube
    let (cube_verts, cube_idx) = primitives::generate_cube(CUBE_SIZE);
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
        primitives::generate_sphere(SPHERE_RADIUS, SPHERE_STACKS, SPHERE_SECTORS);
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
        let val: String = env_var_or("AETHER_TEST_NONEXISTENT_STR_12345", "fallback".to_string());
        assert_eq!(val, "fallback");
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
