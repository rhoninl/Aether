use bytemuck::{Pod, Zeroable};

/// Camera uniform data (must match WGSL CameraUniforms).
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct CameraUniforms {
    pub view: [[f32; 4]; 4],
    pub projection: [[f32; 4]; 4],
    /// xyz = view position, w = padding.
    pub view_position: [f32; 4],
}

impl Default for CameraUniforms {
    fn default() -> Self {
        let identity = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        Self {
            view: identity,
            projection: identity,
            view_position: [0.0, 0.0, 0.0, 0.0],
        }
    }
}

/// Per-instance model uniform data (must match WGSL ModelUniforms).
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct ModelUniforms {
    pub model: [[f32; 4]; 4],
    pub normal_matrix: [[f32; 4]; 4],
}

impl Default for ModelUniforms {
    fn default() -> Self {
        let identity = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        Self {
            model: identity,
            normal_matrix: identity,
        }
    }
}

/// A draw command for instanced rendering.
#[derive(Debug, Clone)]
pub struct DrawCommand {
    /// Mesh to draw.
    pub mesh_id: crate::gpu::mesh::MeshId,
    /// Material to use.
    pub material_id: crate::gpu::material::MaterialId,
    /// Model bind group for this instance.
    pub model_bind_group_index: usize,
    /// Number of instances (for instanced drawing).
    pub instance_count: u32,
}

/// Holds the GPU resources for a single frame's render passes.
pub struct FrameResources {
    pub camera_buffer: wgpu::Buffer,
    pub camera_bind_group: wgpu::BindGroup,
    pub model_buffers: Vec<wgpu::Buffer>,
    pub model_bind_groups: Vec<wgpu::BindGroup>,
}

impl FrameResources {
    /// Create frame resources with camera + preallocated model slots.
    pub fn new(
        device: &wgpu::Device,
        camera_layout: &wgpu::BindGroupLayout,
        model_layout: &wgpu::BindGroupLayout,
        max_objects: usize,
    ) -> Self {
        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera-uniform-buffer"),
            size: std::mem::size_of::<CameraUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera-bind-group"),
            layout: camera_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        let mut model_buffers = Vec::with_capacity(max_objects);
        let mut model_bind_groups = Vec::with_capacity(max_objects);

        for i in 0..max_objects {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("model-uniform-{i}")),
                size: std::mem::size_of::<ModelUniforms>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("model-bind-group-{i}")),
                layout: model_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            });

            model_buffers.push(buffer);
            model_bind_groups.push(bind_group);
        }

        Self {
            camera_buffer,
            camera_bind_group,
            model_buffers,
            model_bind_groups,
        }
    }

    /// Update camera uniforms.
    pub fn update_camera(&self, queue: &wgpu::Queue, uniforms: &CameraUniforms) {
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(uniforms));
    }

    /// Update a model's uniforms by index.
    pub fn update_model(&self, queue: &wgpu::Queue, index: usize, uniforms: &ModelUniforms) {
        if index < self.model_buffers.len() {
            queue.write_buffer(&self.model_buffers[index], 0, bytemuck::bytes_of(uniforms));
        }
    }

    /// Number of preallocated model slots.
    pub fn model_slot_count(&self) -> usize {
        self.model_buffers.len()
    }
}

/// Create the light bind group for the forward pass (bind group 3).
pub fn create_light_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    light_buffer: &wgpu::Buffer,
    shadow_view: &wgpu::TextureView,
    shadow_sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("light-bind-group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: light_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(shadow_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(shadow_sampler),
            },
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_uniforms_default_is_identity() {
        let cam = CameraUniforms::default();
        // Check diagonal of view matrix
        assert_eq!(cam.view[0][0], 1.0);
        assert_eq!(cam.view[1][1], 1.0);
        assert_eq!(cam.view[2][2], 1.0);
        assert_eq!(cam.view[3][3], 1.0);
        // Check diagonal of projection matrix
        assert_eq!(cam.projection[0][0], 1.0);
        assert_eq!(cam.projection[1][1], 1.0);
        assert_eq!(cam.projection[2][2], 1.0);
        assert_eq!(cam.projection[3][3], 1.0);
        // Check view position is origin
        assert_eq!(cam.view_position, [0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn camera_uniforms_size() {
        // 2 * mat4(64) + vec4(16) = 144
        assert_eq!(std::mem::size_of::<CameraUniforms>(), 144);
    }

    #[test]
    fn camera_uniforms_is_pod() {
        let cam = CameraUniforms::default();
        let bytes: &[u8] = bytemuck::bytes_of(&cam);
        assert_eq!(bytes.len(), 144);
    }

    #[test]
    fn model_uniforms_default_is_identity() {
        let model = ModelUniforms::default();
        assert_eq!(model.model[0][0], 1.0);
        assert_eq!(model.model[1][1], 1.0);
        assert_eq!(model.model[2][2], 1.0);
        assert_eq!(model.model[3][3], 1.0);

        assert_eq!(model.normal_matrix[0][0], 1.0);
        assert_eq!(model.normal_matrix[1][1], 1.0);
        assert_eq!(model.normal_matrix[2][2], 1.0);
        assert_eq!(model.normal_matrix[3][3], 1.0);
    }

    #[test]
    fn model_uniforms_size() {
        // 2 * mat4(64) = 128
        assert_eq!(std::mem::size_of::<ModelUniforms>(), 128);
    }

    #[test]
    fn model_uniforms_is_pod() {
        let model = ModelUniforms::default();
        let bytes: &[u8] = bytemuck::bytes_of(&model);
        assert_eq!(bytes.len(), 128);
    }

    #[test]
    fn draw_command_construction() {
        let cmd = DrawCommand {
            mesh_id: crate::gpu::mesh::MeshId(1),
            material_id: crate::gpu::material::MaterialId(2),
            model_bind_group_index: 0,
            instance_count: 3,
        };
        assert_eq!(cmd.mesh_id, crate::gpu::mesh::MeshId(1));
        assert_eq!(cmd.material_id, crate::gpu::material::MaterialId(2));
        assert_eq!(cmd.instance_count, 3);
    }
}
