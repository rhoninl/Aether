//! Render system: queries entities with Renderable + Transform and submits
//! draw commands to the GpuRenderer.

use aether_renderer::gpu::pass::{CameraUniforms, DrawCommand, ModelUniforms};
use aether_renderer::gpu::shadow::LightUniforms;
use aether_renderer::gpu::GpuRenderer;

use crate::components::{CameraState, Renderable, Transform};

/// Default sunlight direction (normalized downward-angled).
const DEFAULT_SUN_DIRECTION: [f32; 4] = [0.0, -1.0, 0.0, 0.0];
/// Default sunlight color (white, full intensity).
const DEFAULT_SUN_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

/// Bridge between the ECS render data and the GPU renderer.
pub struct RenderBridge<'a> {
    pub renderer: &'a GpuRenderer,
}

impl<'a> RenderBridge<'a> {
    /// Submit a full frame: update camera, models, lights, and issue draw commands.
    pub fn submit_frame(&self, camera: &CameraState, renderables: &[(Renderable, Transform)]) {
        let camera_uniforms = update_camera_uniforms(camera);
        self.renderer.update_camera(&camera_uniforms);

        let draw_commands = build_draw_commands(renderables);

        for (renderable, transform) in renderables {
            let model_uniforms = build_model_uniforms(transform);
            self.renderer
                .update_model(renderable.model_index, &model_uniforms);
        }

        let light_uniforms = build_light_uniforms();
        self.renderer.update_light(&light_uniforms);

        let _ = self.renderer.render(&draw_commands);
    }
}

/// Convert a flat column-major `[f32; 16]` array into `[[f32; 4]; 4]` columns.
fn mat4_to_columns(m: &[f32; 16]) -> [[f32; 4]; 4] {
    [
        [m[0], m[1], m[2], m[3]],
        [m[4], m[5], m[6], m[7]],
        [m[8], m[9], m[10], m[11]],
        [m[12], m[13], m[14], m[15]],
    ]
}

/// Convert the ECS `CameraState` into GPU `CameraUniforms`.
pub fn update_camera_uniforms(camera: &CameraState) -> CameraUniforms {
    let view = camera.view_matrix();
    let projection = camera.projection_matrix();

    CameraUniforms {
        view: mat4_to_columns(&view),
        projection: mat4_to_columns(&projection),
        view_position: [
            camera.position[0],
            camera.position[1],
            camera.position[2],
            0.0,
        ],
    }
}

/// Convert an ECS `Transform` into GPU `ModelUniforms`.
pub fn build_model_uniforms(transform: &Transform) -> ModelUniforms {
    let model = transform.model_matrix();
    let normal = transform.normal_matrix();

    ModelUniforms {
        model: mat4_to_columns(&model),
        normal_matrix: mat4_to_columns(&normal),
    }
}

/// Build a list of draw commands from renderable entities.
pub fn build_draw_commands(renderables: &[(Renderable, Transform)]) -> Vec<DrawCommand> {
    renderables
        .iter()
        .map(|(renderable, _)| DrawCommand {
            mesh_id: renderable.mesh_id,
            material_id: renderable.material_id,
            model_bind_group_index: renderable.model_index,
            instance_count: 1,
        })
        .collect()
}

/// Create default sunlight uniforms.
pub fn build_light_uniforms() -> LightUniforms {
    LightUniforms {
        direction: DEFAULT_SUN_DIRECTION,
        color: DEFAULT_SUN_COLOR,
        ..LightUniforms::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_renderer::gpu::material::MaterialId;
    use aether_renderer::gpu::mesh::MeshId;

    #[test]
    fn mat4_to_columns_identity() {
        let flat = [
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ];
        let cols = mat4_to_columns(&flat);
        assert_eq!(cols[0], [1.0, 0.0, 0.0, 0.0]);
        assert_eq!(cols[1], [0.0, 1.0, 0.0, 0.0]);
        assert_eq!(cols[2], [0.0, 0.0, 1.0, 0.0]);
        assert_eq!(cols[3], [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn mat4_to_columns_preserves_column_order() {
        let flat = [
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0,
        ];
        let cols = mat4_to_columns(&flat);
        assert_eq!(cols[0], [1.0, 2.0, 3.0, 4.0]);
        assert_eq!(cols[1], [5.0, 6.0, 7.0, 8.0]);
        assert_eq!(cols[2], [9.0, 10.0, 11.0, 12.0]);
        assert_eq!(cols[3], [13.0, 14.0, 15.0, 16.0]);
    }

    #[test]
    fn camera_uniforms_default_camera() {
        let camera = CameraState::default();
        let uniforms = update_camera_uniforms(&camera);

        // View position should match camera position
        assert!((uniforms.view_position[0] - camera.position[0]).abs() < 1e-6);
        assert!((uniforms.view_position[1] - camera.position[1]).abs() < 1e-6);
        assert!((uniforms.view_position[2] - camera.position[2]).abs() < 1e-6);
        assert!((uniforms.view_position[3]).abs() < 1e-6);
    }

    #[test]
    fn camera_uniforms_view_is_4x4() {
        let camera = CameraState::default();
        let uniforms = update_camera_uniforms(&camera);
        assert_eq!(uniforms.view.len(), 4);
        assert_eq!(uniforms.view[0].len(), 4);
    }

    #[test]
    fn camera_uniforms_projection_has_perspective() {
        let camera = CameraState::default();
        let uniforms = update_camera_uniforms(&camera);
        // Perspective projection: element [2][3] should be -1.0
        assert!((uniforms.projection[2][3] - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn camera_uniforms_custom_position() {
        let camera = CameraState {
            position: [10.0, 20.0, 30.0],
            ..CameraState::default()
        };
        let uniforms = update_camera_uniforms(&camera);
        assert!((uniforms.view_position[0] - 10.0).abs() < 1e-6);
        assert!((uniforms.view_position[1] - 20.0).abs() < 1e-6);
        assert!((uniforms.view_position[2] - 30.0).abs() < 1e-6);
    }

    #[test]
    fn model_uniforms_identity_transform() {
        let transform = Transform::default();
        let uniforms = build_model_uniforms(&transform);

        // Identity model matrix
        assert!((uniforms.model[0][0] - 1.0).abs() < 1e-6);
        assert!((uniforms.model[1][1] - 1.0).abs() < 1e-6);
        assert!((uniforms.model[2][2] - 1.0).abs() < 1e-6);
        assert!((uniforms.model[3][3] - 1.0).abs() < 1e-6);

        // Identity normal matrix
        assert!((uniforms.normal_matrix[0][0] - 1.0).abs() < 1e-6);
        assert!((uniforms.normal_matrix[1][1] - 1.0).abs() < 1e-6);
        assert!((uniforms.normal_matrix[2][2] - 1.0).abs() < 1e-6);
        assert!((uniforms.normal_matrix[3][3] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn model_uniforms_translated_transform() {
        let transform = Transform::at(3.0, 4.0, 5.0);
        let uniforms = build_model_uniforms(&transform);

        // Translation in column 3
        assert!((uniforms.model[3][0] - 3.0).abs() < 1e-6);
        assert!((uniforms.model[3][1] - 4.0).abs() < 1e-6);
        assert!((uniforms.model[3][2] - 5.0).abs() < 1e-6);
    }

    #[test]
    fn model_uniforms_scaled_transform() {
        let transform = Transform::default().with_scale(2.0, 3.0, 4.0);
        let uniforms = build_model_uniforms(&transform);

        assert!((uniforms.model[0][0] - 2.0).abs() < 1e-6);
        assert!((uniforms.model[1][1] - 3.0).abs() < 1e-6);
        assert!((uniforms.model[2][2] - 4.0).abs() < 1e-6);
    }

    #[test]
    fn model_uniforms_normal_matrix_compensates_scale() {
        let transform = Transform::default().with_scale(2.0, 3.0, 4.0);
        let uniforms = build_model_uniforms(&transform);

        // Normal matrix should have 1/scale on diagonal (inverse transpose)
        assert!((uniforms.normal_matrix[0][0] - 1.0).abs() < 1e-6);
        assert!((uniforms.normal_matrix[1][1] - 1.0).abs() < 1e-6);
        assert!((uniforms.normal_matrix[2][2] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn build_draw_commands_empty() {
        let commands = build_draw_commands(&[]);
        assert!(commands.is_empty());
    }

    #[test]
    fn build_draw_commands_single_entity() {
        let renderable = Renderable {
            mesh_id: MeshId(1),
            material_id: MaterialId(2),
            model_index: 0,
        };
        let transform = Transform::default();
        let commands = build_draw_commands(&[(renderable, transform)]);

        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].mesh_id, MeshId(1));
        assert_eq!(commands[0].material_id, MaterialId(2));
        assert_eq!(commands[0].model_bind_group_index, 0);
        assert_eq!(commands[0].instance_count, 1);
    }

    #[test]
    fn build_draw_commands_multiple_entities() {
        let renderables = vec![
            (
                Renderable {
                    mesh_id: MeshId(1),
                    material_id: MaterialId(10),
                    model_index: 0,
                },
                Transform::at(0.0, 0.0, 0.0),
            ),
            (
                Renderable {
                    mesh_id: MeshId(2),
                    material_id: MaterialId(20),
                    model_index: 1,
                },
                Transform::at(1.0, 2.0, 3.0),
            ),
            (
                Renderable {
                    mesh_id: MeshId(3),
                    material_id: MaterialId(30),
                    model_index: 2,
                },
                Transform::at(4.0, 5.0, 6.0),
            ),
        ];
        let commands = build_draw_commands(&renderables);

        assert_eq!(commands.len(), 3);
        assert_eq!(commands[0].mesh_id, MeshId(1));
        assert_eq!(commands[1].mesh_id, MeshId(2));
        assert_eq!(commands[2].mesh_id, MeshId(3));
        assert_eq!(commands[0].model_bind_group_index, 0);
        assert_eq!(commands[1].model_bind_group_index, 1);
        assert_eq!(commands[2].model_bind_group_index, 2);
    }

    #[test]
    fn build_draw_commands_preserves_model_index() {
        let renderable = Renderable {
            mesh_id: MeshId(5),
            material_id: MaterialId(7),
            model_index: 42,
        };
        let transform = Transform::default();
        let commands = build_draw_commands(&[(renderable, transform)]);

        assert_eq!(commands[0].model_bind_group_index, 42);
    }

    #[test]
    fn build_light_uniforms_default_direction() {
        let uniforms = build_light_uniforms();
        assert_eq!(uniforms.direction, DEFAULT_SUN_DIRECTION);
    }

    #[test]
    fn build_light_uniforms_default_color() {
        let uniforms = build_light_uniforms();
        assert_eq!(uniforms.color, DEFAULT_SUN_COLOR);
    }

    #[test]
    fn build_light_uniforms_has_cascade_splits() {
        let uniforms = build_light_uniforms();
        // Should have the same cascade splits as LightUniforms::default()
        let default = LightUniforms::default();
        assert_eq!(uniforms.cascade_splits, default.cascade_splits);
    }

    #[test]
    fn camera_uniforms_zero_yaw_pitch() {
        let camera = CameraState {
            position: [0.0, 0.0, 0.0],
            yaw: 0.0,
            pitch: 0.0,
            ..CameraState::default()
        };
        let uniforms = update_camera_uniforms(&camera);
        // With zero position/yaw/pitch, view matrix column 3 should be ~zero
        assert!((uniforms.view[3][0]).abs() < 1e-6);
        assert!((uniforms.view[3][1]).abs() < 1e-6);
        assert!((uniforms.view[3][2]).abs() < 1e-6);
    }

    #[test]
    fn draw_command_instance_count_always_one() {
        let renderables = vec![
            (
                Renderable {
                    mesh_id: MeshId(1),
                    material_id: MaterialId(1),
                    model_index: 0,
                },
                Transform::default(),
            ),
            (
                Renderable {
                    mesh_id: MeshId(2),
                    material_id: MaterialId(2),
                    model_index: 1,
                },
                Transform::default(),
            ),
        ];
        let commands = build_draw_commands(&renderables);
        for cmd in &commands {
            assert_eq!(cmd.instance_count, 1);
        }
    }
}
