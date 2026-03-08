use bytemuck::{Pod, Zeroable};

use crate::gpu::texture::TextureId;

/// Default albedo color (white).
const DEFAULT_ALBEDO: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
/// Default metallic factor.
const DEFAULT_METALLIC: f32 = 0.0;
/// Default roughness factor.
const DEFAULT_ROUGHNESS: f32 = 0.5;
/// Default emissive color (black, no emission).
const DEFAULT_EMISSIVE: [f32; 3] = [0.0, 0.0, 0.0];

/// Identifier for a PBR material.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MaterialId(pub u64);

/// GPU-side material uniform data (must match WGSL MaterialUniforms).
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct MaterialUniformData {
    pub albedo: [f32; 4],
    /// x = metallic, y = roughness, z = padding, w = padding.
    pub metallic_roughness: [f32; 4],
    /// xyz = emissive, w = padding.
    pub emissive: [f32; 4],
}

impl Default for MaterialUniformData {
    fn default() -> Self {
        Self {
            albedo: DEFAULT_ALBEDO,
            metallic_roughness: [DEFAULT_METALLIC, DEFAULT_ROUGHNESS, 0.0, 0.0],
            emissive: [DEFAULT_EMISSIVE[0], DEFAULT_EMISSIVE[1], DEFAULT_EMISSIVE[2], 0.0],
        }
    }
}

/// CPU-side PBR material description.
#[derive(Debug, Clone)]
pub struct PbrMaterial {
    pub id: MaterialId,
    pub albedo_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub emissive: [f32; 3],
    pub albedo_texture: Option<TextureId>,
    pub normal_texture: Option<TextureId>,
}

impl PbrMaterial {
    /// Create a new PBR material with default values.
    pub fn new(id: MaterialId) -> Self {
        Self {
            id,
            albedo_color: DEFAULT_ALBEDO,
            metallic: DEFAULT_METALLIC,
            roughness: DEFAULT_ROUGHNESS,
            emissive: DEFAULT_EMISSIVE,
            albedo_texture: None,
            normal_texture: None,
        }
    }

    /// Convert to GPU uniform data.
    pub fn to_uniform_data(&self) -> MaterialUniformData {
        MaterialUniformData {
            albedo: self.albedo_color,
            metallic_roughness: [self.metallic, self.roughness, 0.0, 0.0],
            emissive: [self.emissive[0], self.emissive[1], self.emissive[2], 0.0],
        }
    }

    /// Set albedo color. Returns self for chaining.
    pub fn with_albedo(mut self, r: f32, g: f32, b: f32, a: f32) -> Self {
        self.albedo_color = [r, g, b, a];
        self
    }

    /// Set metallic factor (0.0 = dielectric, 1.0 = metal).
    pub fn with_metallic(mut self, metallic: f32) -> Self {
        self.metallic = metallic.clamp(0.0, 1.0);
        self
    }

    /// Set roughness factor (0.0 = smooth, 1.0 = rough).
    pub fn with_roughness(mut self, roughness: f32) -> Self {
        self.roughness = roughness.clamp(0.0, 1.0);
        self
    }

    /// Set emissive color.
    pub fn with_emissive(mut self, r: f32, g: f32, b: f32) -> Self {
        self.emissive = [r, g, b];
        self
    }

    /// Set albedo texture.
    pub fn with_albedo_texture(mut self, tex: TextureId) -> Self {
        self.albedo_texture = Some(tex);
        self
    }
}

/// A material uploaded to the GPU with its uniform buffer and bind group.
pub struct GpuMaterial {
    pub id: MaterialId,
    pub uniform_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    pub material: PbrMaterial,
}

/// Manages PBR materials and their GPU resources.
pub struct MaterialManager {
    materials: std::collections::HashMap<MaterialId, GpuMaterial>,
    next_id: u64,
}

impl MaterialManager {
    pub fn new() -> Self {
        Self {
            materials: std::collections::HashMap::new(),
            next_id: 1,
        }
    }

    /// Allocate a new MaterialId.
    pub fn allocate_id(&mut self) -> MaterialId {
        let id = MaterialId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Upload a PBR material to the GPU.
    pub fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bind_group_layout: &wgpu::BindGroupLayout,
        material: PbrMaterial,
        albedo_view: &wgpu::TextureView,
        albedo_sampler: &wgpu::Sampler,
    ) -> MaterialId {
        let id = material.id;
        let uniform_data = material.to_uniform_data();

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("material-{}-uniform", id.0)),
            size: std::mem::size_of::<MaterialUniformData>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&uniform_buffer, 0, bytemuck::bytes_of(&uniform_data));

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("material-{}-bind-group", id.0)),
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(albedo_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(albedo_sampler),
                },
            ],
        });

        self.materials.insert(
            id,
            GpuMaterial {
                id,
                uniform_buffer,
                bind_group,
                material,
            },
        );

        id
    }

    /// Update the uniform data for an existing material.
    pub fn update_uniforms(
        &self,
        queue: &wgpu::Queue,
        id: MaterialId,
        data: &MaterialUniformData,
    ) -> bool {
        if let Some(gpu_mat) = self.materials.get(&id) {
            queue.write_buffer(&gpu_mat.uniform_buffer, 0, bytemuck::bytes_of(data));
            true
        } else {
            false
        }
    }

    /// Get a GPU material by ID.
    pub fn get(&self, id: MaterialId) -> Option<&GpuMaterial> {
        self.materials.get(&id)
    }

    /// Remove a material, dropping GPU resources.
    pub fn remove(&mut self, id: MaterialId) -> bool {
        self.materials.remove(&id).is_some()
    }

    /// Number of materials managed.
    pub fn count(&self) -> usize {
        self.materials.len()
    }

    /// Create the bind group layout for the material bind group (group 2).
    pub fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("material-bind-group-layout"),
            entries: &[
                // MaterialUniforms
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Albedo texture
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Albedo sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        })
    }
}

impl Default for MaterialManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_uniform_data_default() {
        let data = MaterialUniformData::default();
        assert_eq!(data.albedo, [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(data.metallic_roughness[0], 0.0); // metallic
        assert_eq!(data.metallic_roughness[1], 0.5); // roughness
        assert_eq!(data.emissive[0], 0.0);
        assert_eq!(data.emissive[1], 0.0);
        assert_eq!(data.emissive[2], 0.0);
    }

    #[test]
    fn material_uniform_data_size() {
        // 3 * vec4 = 48 bytes
        assert_eq!(std::mem::size_of::<MaterialUniformData>(), 48);
    }

    #[test]
    fn material_uniform_data_is_pod() {
        let data = MaterialUniformData::default();
        let bytes: &[u8] = bytemuck::bytes_of(&data);
        assert_eq!(bytes.len(), 48);
    }

    #[test]
    fn pbr_material_defaults() {
        let mat = PbrMaterial::new(MaterialId(1));
        assert_eq!(mat.albedo_color, [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(mat.metallic, 0.0);
        assert_eq!(mat.roughness, 0.5);
        assert_eq!(mat.emissive, [0.0, 0.0, 0.0]);
        assert!(mat.albedo_texture.is_none());
        assert!(mat.normal_texture.is_none());
    }

    #[test]
    fn pbr_material_builder_pattern() {
        let mat = PbrMaterial::new(MaterialId(1))
            .with_albedo(1.0, 0.0, 0.0, 1.0)
            .with_metallic(0.8)
            .with_roughness(0.2)
            .with_emissive(0.5, 0.5, 0.0)
            .with_albedo_texture(TextureId(42));

        assert_eq!(mat.albedo_color, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(mat.metallic, 0.8);
        assert_eq!(mat.roughness, 0.2);
        assert_eq!(mat.emissive, [0.5, 0.5, 0.0]);
        assert_eq!(mat.albedo_texture, Some(TextureId(42)));
    }

    #[test]
    fn pbr_material_clamps_metallic_roughness() {
        let mat = PbrMaterial::new(MaterialId(1))
            .with_metallic(2.0)
            .with_roughness(-0.5);
        assert_eq!(mat.metallic, 1.0);
        assert_eq!(mat.roughness, 0.0);
    }

    #[test]
    fn pbr_material_to_uniform_data() {
        let mat = PbrMaterial::new(MaterialId(1))
            .with_albedo(0.5, 0.6, 0.7, 1.0)
            .with_metallic(0.3)
            .with_roughness(0.9)
            .with_emissive(0.1, 0.2, 0.3);

        let data = mat.to_uniform_data();
        assert_eq!(data.albedo, [0.5, 0.6, 0.7, 1.0]);
        assert_eq!(data.metallic_roughness[0], 0.3);
        assert_eq!(data.metallic_roughness[1], 0.9);
        assert_eq!(data.emissive[0], 0.1);
        assert_eq!(data.emissive[1], 0.2);
        assert_eq!(data.emissive[2], 0.3);
    }

    #[test]
    fn material_id_equality() {
        assert_eq!(MaterialId(1), MaterialId(1));
        assert_ne!(MaterialId(1), MaterialId(2));
    }

    #[test]
    fn material_manager_starts_empty() {
        let mgr = MaterialManager::new();
        assert_eq!(mgr.count(), 0);
        assert!(mgr.get(MaterialId(1)).is_none());
    }

    #[test]
    fn material_manager_allocate_id() {
        let mut mgr = MaterialManager::new();
        let id1 = mgr.allocate_id();
        let id2 = mgr.allocate_id();
        assert_ne!(id1, id2);
    }

    #[test]
    fn material_manager_default_is_empty() {
        let mgr = MaterialManager::default();
        assert_eq!(mgr.count(), 0);
    }
}
