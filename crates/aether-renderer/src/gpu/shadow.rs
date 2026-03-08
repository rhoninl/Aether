use std::env;

use bytemuck::{Pod, Zeroable};

/// Default shadow map resolution per cascade.
const DEFAULT_SHADOW_MAP_SIZE: u32 = 2048;
/// Number of shadow cascades.
const NUM_CASCADES: u32 = 4;
/// Depth format for shadow maps.
const SHADOW_DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Read the shadow map size from the `AETHER_SHADOW_MAP_SIZE` env var.
fn shadow_map_size_from_env() -> u32 {
    env::var("AETHER_SHADOW_MAP_SIZE")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(DEFAULT_SHADOW_MAP_SIZE)
}

/// GPU-side light VP matrix for a single cascade.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct LightVpUniform {
    pub view_proj: [[f32; 4]; 4],
}

/// GPU-side light uniforms for the fragment shader (direction, color, cascade VPs, splits).
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct LightUniforms {
    pub direction: [f32; 4],
    pub color: [f32; 4],
    pub cascade_vp_0: [[f32; 4]; 4],
    pub cascade_vp_1: [[f32; 4]; 4],
    pub cascade_vp_2: [[f32; 4]; 4],
    pub cascade_vp_3: [[f32; 4]; 4],
    pub cascade_splits: [f32; 4],
}

impl Default for LightUniforms {
    fn default() -> Self {
        let identity = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        Self {
            direction: [0.0, -1.0, 0.0, 0.0],
            color: [1.0, 1.0, 1.0, 1.0],
            cascade_vp_0: identity,
            cascade_vp_1: identity,
            cascade_vp_2: identity,
            cascade_vp_3: identity,
            cascade_splits: [10.0, 30.0, 70.0, 150.0],
        }
    }
}

/// Manages the cascaded shadow map textures and resources.
pub struct ShadowPass {
    pub depth_texture: wgpu::Texture,
    pub depth_view: wgpu::TextureView,
    pub cascade_views: [wgpu::TextureView; 4],
    pub comparison_sampler: wgpu::Sampler,
    pub cascade_size: u32,
    pub light_uniform_buffer: wgpu::Buffer,
    pub cascade_vp_buffers: [wgpu::Buffer; 4],
    pub cascade_bind_groups: [wgpu::BindGroup; 4],
}

impl ShadowPass {
    /// Create the shadow pass resources.
    pub fn new(
        device: &wgpu::Device,
        light_vp_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let cascade_size = shadow_map_size_from_env();

        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shadow-cascade-depth"),
            size: wgpu::Extent3d {
                width: cascade_size,
                height: cascade_size,
                depth_or_array_layers: NUM_CASCADES,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SHADOW_DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        // Full array view for sampling in the forward pass
        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("shadow-cascade-depth-array-view"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });

        // Per-cascade views for rendering
        let cascade_views = std::array::from_fn(|i| {
            depth_texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some(&format!("shadow-cascade-{i}-view")),
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_array_layer: i as u32,
                array_layer_count: Some(1),
                ..Default::default()
            })
        });

        let comparison_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shadow-comparison-sampler"),
            compare: Some(wgpu::CompareFunction::LessEqual),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Light uniform buffer (used in forward pass, bind group 3)
        let light_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("light-uniform-buffer"),
            size: std::mem::size_of::<LightUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Per-cascade VP buffers and bind groups for the shadow pass
        let cascade_vp_buffers: [wgpu::Buffer; 4] = std::array::from_fn(|i| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("shadow-cascade-{i}-vp")),
                size: std::mem::size_of::<LightVpUniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        });

        let cascade_bind_groups: [wgpu::BindGroup; 4] = std::array::from_fn(|i| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("shadow-cascade-{i}-bind-group")),
                layout: light_vp_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: cascade_vp_buffers[i].as_entire_binding(),
                }],
            })
        });

        Self {
            depth_texture,
            depth_view,
            cascade_views,
            comparison_sampler,
            cascade_size,
            light_uniform_buffer,
            cascade_vp_buffers,
            cascade_bind_groups,
        }
    }

    /// Create the bind group layout for the light VP uniform (shadow pass group 0).
    pub fn create_light_vp_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow-light-vp-layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        })
    }

    /// Update the light uniforms for the forward pass.
    pub fn update_light_uniforms(&self, queue: &wgpu::Queue, uniforms: &LightUniforms) {
        queue.write_buffer(&self.light_uniform_buffer, 0, bytemuck::bytes_of(uniforms));
    }

    /// Update a single cascade's view-projection matrix.
    pub fn update_cascade_vp(&self, queue: &wgpu::Queue, cascade: usize, vp: &LightVpUniform) {
        if cascade < NUM_CASCADES as usize {
            queue.write_buffer(&self.cascade_vp_buffers[cascade], 0, bytemuck::bytes_of(vp));
        }
    }

    /// Depth format used by the shadow pass.
    pub fn depth_format() -> wgpu::TextureFormat {
        SHADOW_DEPTH_FORMAT
    }

    /// Number of cascades.
    pub fn num_cascades() -> u32 {
        NUM_CASCADES
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_uniforms_default() {
        let lu = LightUniforms::default();
        assert_eq!(lu.direction, [0.0, -1.0, 0.0, 0.0]);
        assert_eq!(lu.color, [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(lu.cascade_splits, [10.0, 30.0, 70.0, 150.0]);
    }

    #[test]
    fn light_uniforms_size() {
        // direction(16) + color(16) + 4 * mat4(64) + splits(16) = 304
        assert_eq!(std::mem::size_of::<LightUniforms>(), 304);
    }

    #[test]
    fn light_uniforms_is_pod() {
        let lu = LightUniforms::default();
        let bytes: &[u8] = bytemuck::bytes_of(&lu);
        assert_eq!(bytes.len(), 304);
    }

    #[test]
    fn light_vp_uniform_size() {
        // mat4x4 = 64 bytes
        assert_eq!(std::mem::size_of::<LightVpUniform>(), 64);
    }

    #[test]
    fn shadow_depth_format_is_depth32float() {
        assert_eq!(ShadowPass::depth_format(), wgpu::TextureFormat::Depth32Float);
    }

    #[test]
    fn shadow_num_cascades_is_4() {
        assert_eq!(ShadowPass::num_cascades(), 4);
    }

    #[test]
    fn shadow_map_size_default() {
        assert_eq!(DEFAULT_SHADOW_MAP_SIZE, 2048);
    }

    #[test]
    fn light_vp_uniform_is_pod() {
        let vp = LightVpUniform {
            view_proj: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        };
        let bytes: &[u8] = bytemuck::bytes_of(&vp);
        assert_eq!(bytes.len(), 64);
    }
}
