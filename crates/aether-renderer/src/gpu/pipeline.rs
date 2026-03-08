use crate::gpu::material::MaterialManager;
use crate::gpu::mesh::Vertex;
use crate::gpu::shader;
use crate::gpu::shadow::ShadowPass;

/// Label for the forward render pipeline.
const FORWARD_PIPELINE_LABEL: &str = "aether-forward-pipeline";

/// Camera bind group layout (group 0).
pub fn create_camera_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("camera-bind-group-layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    })
}

/// Model bind group layout (group 1).
pub fn create_model_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("model-bind-group-layout"),
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

/// Light + shadow bind group layout (group 3).
pub fn create_light_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("light-bind-group-layout"),
        entries: &[
            // LightUniforms
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // Shadow depth texture array
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2Array,
                    multisampled: false,
                },
                count: None,
            },
            // Shadow comparison sampler
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                count: None,
            },
        ],
    })
}

/// All bind group layouts needed for the PBR forward pipeline.
pub struct PipelineLayouts {
    pub camera_layout: wgpu::BindGroupLayout,
    pub model_layout: wgpu::BindGroupLayout,
    pub material_layout: wgpu::BindGroupLayout,
    pub light_layout: wgpu::BindGroupLayout,
    pub pipeline_layout: wgpu::PipelineLayout,
}

impl PipelineLayouts {
    /// Create all bind group layouts and the pipeline layout.
    pub fn new(device: &wgpu::Device) -> Self {
        let camera_layout = create_camera_bind_group_layout(device);
        let model_layout = create_model_bind_group_layout(device);
        let material_layout = MaterialManager::create_bind_group_layout(device);
        let light_layout = create_light_bind_group_layout(device);

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("aether-pipeline-layout"),
            bind_group_layouts: &[
                &camera_layout,
                &model_layout,
                &material_layout,
                &light_layout,
            ],
            push_constant_ranges: &[],
        });

        Self {
            camera_layout,
            model_layout,
            material_layout,
            light_layout,
            pipeline_layout,
        }
    }
}

/// Create the forward render pipeline with MSAA and depth.
pub fn create_forward_pipeline(
    device: &wgpu::Device,
    layouts: &PipelineLayouts,
    surface_format: wgpu::TextureFormat,
    depth_format: wgpu::TextureFormat,
    msaa_samples: u32,
) -> wgpu::RenderPipeline {
    let pbr_shader = shader::create_shader_module(device, shader::PBR_SHADER_LABEL, shader::PBR_SHADER_SOURCE);

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(FORWARD_PIPELINE_LABEL),
        layout: Some(&layouts.pipeline_layout),
        vertex: wgpu::VertexState {
            module: &pbr_shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[Vertex::buffer_layout()],
        },
        fragment: Some(wgpu::FragmentState {
            module: &pbr_shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: surface_format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: depth_format,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: msaa_samples,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview: None,
        cache: None,
    })
}

/// Shadow pipeline layouts (subset: light VP + model).
pub struct ShadowPipelineLayouts {
    pub light_vp_layout: wgpu::BindGroupLayout,
    pub model_layout: wgpu::BindGroupLayout,
    pub pipeline_layout: wgpu::PipelineLayout,
}

impl ShadowPipelineLayouts {
    pub fn new(device: &wgpu::Device) -> Self {
        let light_vp_layout = ShadowPass::create_light_vp_bind_group_layout(device);
        let model_layout = create_model_bind_group_layout(device);

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("shadow-pipeline-layout"),
            bind_group_layouts: &[&light_vp_layout, &model_layout],
            push_constant_ranges: &[],
        });

        Self {
            light_vp_layout,
            model_layout,
            pipeline_layout,
        }
    }
}

/// Create the shadow render pipeline (depth-only, no fragment).
pub fn create_shadow_pipeline(
    device: &wgpu::Device,
    layouts: &ShadowPipelineLayouts,
    depth_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shadow_shader = shader::create_shader_module(
        device,
        shader::SHADOW_SHADER_LABEL,
        shader::SHADOW_SHADER_SOURCE,
    );

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("aether-shadow-pipeline"),
        layout: Some(&layouts.pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shadow_shader,
            entry_point: Some("vs_shadow"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[Vertex::buffer_layout()],
        },
        fragment: None,
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Front), // front-face culling for shadow pass
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: depth_format,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState {
                constant: 2,
                slope_scale: 2.0,
                clamp: 0.0,
            },
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_pipeline_label_constant() {
        assert_eq!(FORWARD_PIPELINE_LABEL, "aether-forward-pipeline");
    }
}
