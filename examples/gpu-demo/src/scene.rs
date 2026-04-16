use aether_renderer::gpu::material::{MaterialId, PbrMaterial};
use aether_renderer::gpu::mesh::MeshId;
use aether_renderer::gpu::pass::{DrawCommand, ModelUniforms};
use aether_renderer::gpu::shadow::LightUniforms;
use aether_renderer::gpu::GpuRenderer;
use aether_renderer::primitives;

use crate::camera::{mat4_mul, normal_matrix, scale_matrix, translation_matrix};

/// Number of scene objects in the demo.
#[cfg(test)]
const SCENE_OBJECT_COUNT: usize = 5;
/// Floor size (half-extent).
const FLOOR_SIZE: f32 = 20.0;
/// Floor subdivisions.
const FLOOR_SUBDIVISIONS: u32 = 8;
/// Unit cube half-extent.
const CUBE_SIZE: f32 = 1.0;
/// Unit sphere radius.
const SPHERE_RADIUS: f32 = 1.0;
/// Default UV sphere stacks.
const SPHERE_STACKS: u32 = 16;
/// Default UV sphere sectors.
const SPHERE_SECTORS: u32 = 32;

/// Description of a scene object for setting up transforms and draw commands.
pub struct SceneObject {
    pub mesh_id: MeshId,
    pub material_id: MaterialId,
    pub model_index: usize,
}

/// Holds all scene data: objects, light uniforms, draw commands.
pub struct Scene {
    pub objects: Vec<SceneObject>,
    pub light: LightUniforms,
}

/// Scene material definitions (CPU-side, before upload).
pub struct SceneMaterials {
    pub floor: PbrMaterial,
    pub red_cube: PbrMaterial,
    pub blue_cube: PbrMaterial,
    pub metal_sphere: PbrMaterial,
    pub green_sphere: PbrMaterial,
}

impl SceneMaterials {
    /// Create the demo scene's material definitions.
    pub fn new(
        floor_id: MaterialId,
        red_id: MaterialId,
        blue_id: MaterialId,
        metal_id: MaterialId,
        green_id: MaterialId,
    ) -> Self {
        Self {
            floor: PbrMaterial::new(floor_id)
                .with_albedo(0.4, 0.4, 0.4, 1.0)
                .with_roughness(0.9)
                .with_metallic(0.0),
            red_cube: PbrMaterial::new(red_id)
                .with_albedo(0.9, 0.15, 0.1, 1.0)
                .with_roughness(0.4)
                .with_metallic(0.1),
            blue_cube: PbrMaterial::new(blue_id)
                .with_albedo(0.1, 0.2, 0.9, 1.0)
                .with_roughness(0.3)
                .with_metallic(0.2),
            metal_sphere: PbrMaterial::new(metal_id)
                .with_albedo(0.95, 0.93, 0.88, 1.0)
                .with_roughness(0.1)
                .with_metallic(1.0),
            green_sphere: PbrMaterial::new(green_id)
                .with_albedo(0.2, 0.8, 0.3, 1.0)
                .with_roughness(0.6)
                .with_metallic(0.0),
        }
    }
}

/// Set up the scene: upload meshes and materials, return scene description.
///
/// Objects:
///   0: Floor plane
///   1: Red cube (left)
///   2: Blue cube (right)
///   3: Metal sphere (center)
///   4: Green sphere (back)
pub fn setup_scene(renderer: &mut GpuRenderer) -> Scene {
    // Generate meshes
    let (plane_v, plane_i) = primitives::generate_plane(FLOOR_SIZE, FLOOR_SUBDIVISIONS);
    let (cube_v, cube_i) = primitives::generate_cube(CUBE_SIZE);
    let (sphere_v, sphere_i) =
        primitives::generate_sphere(SPHERE_RADIUS, SPHERE_STACKS, SPHERE_SECTORS);

    // Upload meshes
    let plane_mesh = renderer.upload_mesh(&plane_v, &plane_i);
    let cube_mesh = renderer.upload_mesh(&cube_v, &cube_i);
    let sphere_mesh = renderer.upload_mesh(&sphere_v, &sphere_i);

    // Allocate material IDs
    let floor_id = renderer.material_manager.allocate_id();
    let red_id = renderer.material_manager.allocate_id();
    let blue_id = renderer.material_manager.allocate_id();
    let metal_id = renderer.material_manager.allocate_id();
    let green_id = renderer.material_manager.allocate_id();

    let materials = SceneMaterials::new(floor_id, red_id, blue_id, metal_id, green_id);

    // Upload materials
    let floor_mat = renderer.upload_material(materials.floor);
    let red_mat = renderer.upload_material(materials.red_cube);
    let blue_mat = renderer.upload_material(materials.blue_cube);
    let metal_mat = renderer.upload_material(materials.metal_sphere);
    let green_mat = renderer.upload_material(materials.green_sphere);

    // Define scene objects with transforms
    let objects = vec![
        SceneObject {
            mesh_id: plane_mesh,
            material_id: floor_mat,
            model_index: 0,
        },
        SceneObject {
            mesh_id: cube_mesh,
            material_id: red_mat,
            model_index: 1,
        },
        SceneObject {
            mesh_id: cube_mesh,
            material_id: blue_mat,
            model_index: 2,
        },
        SceneObject {
            mesh_id: sphere_mesh,
            material_id: metal_mat,
            model_index: 3,
        },
        SceneObject {
            mesh_id: sphere_mesh,
            material_id: green_mat,
            model_index: 4,
        },
    ];

    // Set up directional light
    let light = create_light_uniforms();

    Scene { objects, light }
}

/// Create the default directional light.
fn create_light_uniforms() -> LightUniforms {
    // Normalize direction: roughly (-0.5, -1.0, -0.3)
    let dx: f32 = -0.5;
    let dy: f32 = -1.0;
    let dz: f32 = -0.3;
    let len = (dx * dx + dy * dy + dz * dz).sqrt();

    LightUniforms {
        direction: [dx / len, dy / len, dz / len, 0.0],
        color: [1.0, 0.95, 0.9, 1.5], // warm sunlight, w = intensity
        ..Default::default()
    }
}

/// Update model transforms for all scene objects.
pub fn update_transforms(renderer: &GpuRenderer, _scene: &Scene) {
    // Object 0: Floor at origin
    let floor_model = translation_matrix(0.0, 0.0, 0.0);
    renderer.update_model(
        0,
        &ModelUniforms {
            model: floor_model,
            normal_matrix: normal_matrix(floor_model),
        },
    );

    // Object 1: Red cube at (-4, 1.5, 0), scaled
    let red_model = mat4_mul(
        translation_matrix(-4.0, 1.5, 0.0),
        scale_matrix(1.5, 1.5, 1.5),
    );
    renderer.update_model(
        1,
        &ModelUniforms {
            model: red_model,
            normal_matrix: normal_matrix(red_model),
        },
    );

    // Object 2: Blue cube at (4, 1.0, -2)
    let blue_model = translation_matrix(4.0, 1.0, -2.0);
    renderer.update_model(
        2,
        &ModelUniforms {
            model: blue_model,
            normal_matrix: normal_matrix(blue_model),
        },
    );

    // Object 3: Metal sphere at (0, 2, 0), scaled 2x
    let metal_model = mat4_mul(
        translation_matrix(0.0, 2.0, 0.0),
        scale_matrix(2.0, 2.0, 2.0),
    );
    renderer.update_model(
        3,
        &ModelUniforms {
            model: metal_model,
            normal_matrix: normal_matrix(metal_model),
        },
    );

    // Object 4: Green sphere at (3, 1.5, 4)
    let green_model = mat4_mul(
        translation_matrix(3.0, 1.5, 4.0),
        scale_matrix(1.5, 1.5, 1.5),
    );
    renderer.update_model(
        4,
        &ModelUniforms {
            model: green_model,
            normal_matrix: normal_matrix(green_model),
        },
    );
}

/// Build draw commands from the scene.
pub fn build_draw_commands(scene: &Scene) -> Vec<DrawCommand> {
    scene
        .objects
        .iter()
        .map(|obj| DrawCommand {
            mesh_id: obj.mesh_id,
            material_id: obj.material_id,
            model_bind_group_index: obj.model_index,
            instance_count: 1,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_object_count() {
        assert_eq!(SCENE_OBJECT_COUNT, 5);
    }

    #[test]
    fn scene_materials_properties() {
        let mats = SceneMaterials::new(
            MaterialId(1),
            MaterialId(2),
            MaterialId(3),
            MaterialId(4),
            MaterialId(5),
        );

        // Floor is grey, rough, non-metallic
        assert_eq!(mats.floor.albedo_color, [0.4, 0.4, 0.4, 1.0]);
        assert!(mats.floor.roughness > 0.8);
        assert!(mats.floor.metallic < 0.1);

        // Red cube
        assert!(mats.red_cube.albedo_color[0] > 0.8);
        assert!(mats.red_cube.albedo_color[1] < 0.3);

        // Blue cube
        assert!(mats.blue_cube.albedo_color[2] > 0.8);
        assert!(mats.blue_cube.albedo_color[0] < 0.3);

        // Metal sphere is metallic
        assert!(mats.metal_sphere.metallic > 0.9);
        assert!(mats.metal_sphere.roughness < 0.2);

        // Green sphere
        assert!(mats.green_sphere.albedo_color[1] > 0.7);
        assert!(mats.green_sphere.metallic < 0.1);
    }

    #[test]
    fn light_uniforms_direction_normalized() {
        let light = create_light_uniforms();
        let len =
            (light.direction[0].powi(2) + light.direction[1].powi(2) + light.direction[2].powi(2))
                .sqrt();
        assert!(
            (len - 1.0).abs() < 1e-4,
            "light direction not normalized: len={len}"
        );
    }

    #[test]
    fn light_uniforms_points_downward() {
        let light = create_light_uniforms();
        assert!(light.direction[1] < 0.0, "light should point downward");
    }

    #[test]
    fn light_has_positive_color() {
        let light = create_light_uniforms();
        assert!(light.color[0] > 0.0);
        assert!(light.color[1] > 0.0);
        assert!(light.color[2] > 0.0);
    }

    #[test]
    fn floor_size_and_subdivisions() {
        assert!(FLOOR_SIZE > 0.0);
        assert!(FLOOR_SUBDIVISIONS > 0);
    }
}
