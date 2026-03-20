use std::env;

/// Default maximum texture dimension.
const DEFAULT_MAX_TEXTURE_SIZE: u32 = 4096;
/// Default texture format for albedo maps.
const DEFAULT_ALBEDO_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
/// Default texture format for normal maps.
const DEFAULT_NORMAL_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// Read the maximum texture size from the `AETHER_MAX_TEXTURE_SIZE` env var.
fn max_texture_size_from_env() -> u32 {
    env::var("AETHER_MAX_TEXTURE_SIZE")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(DEFAULT_MAX_TEXTURE_SIZE)
}

/// Identifier for a managed GPU texture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureId(pub u64);

/// Describes a texture to be uploaded.
#[derive(Debug, Clone)]
pub struct TextureDescriptor {
    pub width: u32,
    pub height: u32,
    pub format: wgpu::TextureFormat,
    pub label: String,
    /// If true, generate mipmaps.
    pub generate_mipmaps: bool,
}

/// A GPU texture with its view and sampler.
pub struct GpuTexture {
    pub id: TextureId,
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub width: u32,
    pub height: u32,
    pub format: wgpu::TextureFormat,
}

/// Manages texture uploads and GPU texture lifetimes.
pub struct TextureManager {
    textures: std::collections::HashMap<TextureId, GpuTexture>,
    next_id: u64,
    max_size: u32,
}

impl TextureManager {
    pub fn new() -> Self {
        Self {
            textures: std::collections::HashMap::new(),
            next_id: 1,
            max_size: max_texture_size_from_env(),
        }
    }

    /// Upload RGBA8 pixel data as a texture.
    #[allow(clippy::too_many_arguments)]
    pub fn upload_rgba8(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        data: &[u8],
        label: &str,
        srgb: bool,
    ) -> TextureId {
        let clamped_w = width.min(self.max_size);
        let clamped_h = height.min(self.max_size);
        let format = if srgb {
            DEFAULT_ALBEDO_FORMAT
        } else {
            DEFAULT_NORMAL_FORMAT
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: clamped_w,
                height: clamped_h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * clamped_w),
                rows_per_image: Some(clamped_h),
            },
            wgpu::Extent3d {
                width: clamped_w,
                height: clamped_h,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some(&format!("{label}-sampler")),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let id = TextureId(self.next_id);
        self.next_id += 1;

        self.textures.insert(
            id,
            GpuTexture {
                id,
                texture,
                view,
                sampler,
                width: clamped_w,
                height: clamped_h,
                format,
            },
        );

        id
    }

    /// Create a 1x1 default white texture (useful as fallback).
    pub fn create_default_white(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> TextureId {
        self.upload_rgba8(
            device,
            queue,
            1,
            1,
            &[255, 255, 255, 255],
            "default-white",
            true,
        )
    }

    /// Get a GPU texture by ID.
    pub fn get(&self, id: TextureId) -> Option<&GpuTexture> {
        self.textures.get(&id)
    }

    /// Remove a texture, dropping GPU resources.
    pub fn remove(&mut self, id: TextureId) -> bool {
        self.textures.remove(&id).is_some()
    }

    /// Number of textures managed.
    pub fn count(&self) -> usize {
        self.textures.len()
    }

    /// Maximum allowed texture dimension.
    pub fn max_size(&self) -> u32 {
        self.max_size
    }
}

impl Default for TextureManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a depth texture for the render pass.
pub fn create_depth_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    sample_count: u32,
    format: wgpu::TextureFormat,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("depth-texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

/// Create an MSAA resolve texture.
pub fn create_msaa_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    sample_count: u32,
    format: wgpu::TextureFormat,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("msaa-texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texture_id_equality() {
        assert_eq!(TextureId(1), TextureId(1));
        assert_ne!(TextureId(1), TextureId(2));
    }

    #[test]
    fn max_texture_size_default() {
        assert_eq!(DEFAULT_MAX_TEXTURE_SIZE, 4096);
    }

    #[test]
    fn texture_manager_starts_empty() {
        let mgr = TextureManager::new();
        assert_eq!(mgr.count(), 0);
        assert!(mgr.get(TextureId(1)).is_none());
    }

    #[test]
    fn texture_manager_default_is_empty() {
        let mgr = TextureManager::default();
        assert_eq!(mgr.count(), 0);
    }

    #[test]
    fn texture_manager_max_size() {
        let mgr = TextureManager::new();
        // Default when AETHER_MAX_TEXTURE_SIZE is not set
        assert!(mgr.max_size() > 0);
    }

    #[test]
    fn default_formats_are_correct() {
        assert_eq!(DEFAULT_ALBEDO_FORMAT, wgpu::TextureFormat::Rgba8UnormSrgb);
        assert_eq!(DEFAULT_NORMAL_FORMAT, wgpu::TextureFormat::Rgba8Unorm);
    }

    #[test]
    fn texture_descriptor_construction() {
        let desc = TextureDescriptor {
            width: 256,
            height: 256,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            label: "test".to_string(),
            generate_mipmaps: false,
        };
        assert_eq!(desc.width, 256);
        assert_eq!(desc.height, 256);
        assert!(!desc.generate_mipmaps);
    }
}
