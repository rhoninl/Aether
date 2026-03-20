use std::env;

/// Default MSAA sample count.
const DEFAULT_MSAA_SAMPLES: u32 = 4;
/// Default surface width when not configured.
const DEFAULT_SURFACE_WIDTH: u32 = 1920;
/// Default surface height when not configured.
const DEFAULT_SURFACE_HEIGHT: u32 = 1080;

/// Errors that can occur during GPU context creation.
#[derive(Debug)]
pub enum GpuError {
    /// No suitable GPU adapter found.
    NoAdapter,
    /// Device request failed.
    DeviceRequestFailed(String),
    /// Surface configuration failed.
    SurfaceConfigFailed(String),
}

impl std::fmt::Display for GpuError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GpuError::NoAdapter => write!(f, "no suitable GPU adapter found"),
            GpuError::DeviceRequestFailed(msg) => write!(f, "device request failed: {msg}"),
            GpuError::SurfaceConfigFailed(msg) => write!(f, "surface config failed: {msg}"),
        }
    }
}

impl std::error::Error for GpuError {}

/// Holds core wgpu objects: Instance, Adapter, Device, Queue, and optional Surface.
pub struct GpuContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: Option<wgpu::Surface<'static>>,
    pub surface_config: Option<wgpu::SurfaceConfiguration>,
    pub msaa_samples: u32,
    pub surface_format: wgpu::TextureFormat,
    pub depth_format: wgpu::TextureFormat,
}

/// Read the desired wgpu backend from the `AETHER_GPU_BACKEND` env var.
fn backend_from_env() -> wgpu::Backends {
    match env::var("AETHER_GPU_BACKEND")
        .unwrap_or_default()
        .to_lowercase()
        .as_str()
    {
        "vulkan" => wgpu::Backends::VULKAN,
        "metal" => wgpu::Backends::METAL,
        "dx12" => wgpu::Backends::DX12,
        "gl" => wgpu::Backends::GL,
        _ => wgpu::Backends::all(),
    }
}

/// Read the MSAA sample count from the `AETHER_MSAA_SAMPLES` env var.
fn msaa_samples_from_env() -> u32 {
    env::var("AETHER_MSAA_SAMPLES")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(DEFAULT_MSAA_SAMPLES)
}

impl GpuContext {
    /// Create a new headless GPU context (no surface).
    ///
    /// Returns `Err(GpuError::NoAdapter)` when no GPU is available (e.g. CI).
    pub async fn new_headless() -> Result<Self, GpuError> {
        let backends = backend_from_env();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends,
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
            .ok_or(GpuError::NoAdapter)?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("aether-gpu-device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    ..Default::default()
                },
                None,
            )
            .await
            .map_err(|e| GpuError::DeviceRequestFailed(e.to_string()))?;

        let msaa_samples = msaa_samples_from_env();

        Ok(Self {
            instance,
            adapter,
            device,
            queue,
            surface: None,
            surface_config: None,
            msaa_samples,
            surface_format: wgpu::TextureFormat::Bgra8UnormSrgb,
            depth_format: wgpu::TextureFormat::Depth32Float,
        })
    }

    /// Create a GPU context with a surface from a window target.
    ///
    /// The `instance` must be the same one used to create the `surface`.
    /// The `window` must live at least as long as the returned `GpuContext`.
    pub async fn new_with_surface(
        instance: wgpu::Instance,
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
    ) -> Result<Self, GpuError> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .ok_or(GpuError::NoAdapter)?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("aether-gpu-device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    ..Default::default()
                },
                None,
            )
            .await
            .map_err(|e| GpuError::DeviceRequestFailed(e.to_string()))?;

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let w = if width > 0 {
            width
        } else {
            DEFAULT_SURFACE_WIDTH
        };
        let h = if height > 0 {
            height
        } else {
            DEFAULT_SURFACE_HEIGHT
        };

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: w,
            height: h,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        let msaa_samples = msaa_samples_from_env();

        Ok(Self {
            instance,
            adapter,
            device,
            queue,
            surface: Some(surface),
            surface_config: Some(surface_config),
            msaa_samples,
            surface_format,
            depth_format: wgpu::TextureFormat::Depth32Float,
        })
    }

    /// Resize the surface. No-op if headless.
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        if let Some(config) = &mut self.surface_config {
            config.width = width;
            config.height = height;
            if let Some(surface) = &self.surface {
                surface.configure(&self.device, config);
            }
        }
    }

    /// Current surface dimensions, or defaults if headless.
    pub fn surface_size(&self) -> (u32, u32) {
        self.surface_config
            .as_ref()
            .map(|c| (c.width, c.height))
            .unwrap_or((DEFAULT_SURFACE_WIDTH, DEFAULT_SURFACE_HEIGHT))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_error_display_messages() {
        let e1 = GpuError::NoAdapter;
        assert_eq!(format!("{e1}"), "no suitable GPU adapter found");

        let e2 = GpuError::DeviceRequestFailed("timeout".into());
        assert!(format!("{e2}").contains("timeout"));

        let e3 = GpuError::SurfaceConfigFailed("bad format".into());
        assert!(format!("{e3}").contains("bad format"));
    }

    #[test]
    fn backend_from_env_defaults_to_all() {
        // When env var is not set, should return all backends.
        let backends = backend_from_env();
        assert_eq!(backends, wgpu::Backends::all());
    }

    #[test]
    fn msaa_samples_defaults_to_4() {
        // When env var is not set, should return 4.
        let samples = msaa_samples_from_env();
        assert_eq!(samples, DEFAULT_MSAA_SAMPLES);
    }

    #[test]
    fn default_surface_size_constants() {
        assert_eq!(DEFAULT_SURFACE_WIDTH, 1920);
        assert_eq!(DEFAULT_SURFACE_HEIGHT, 1080);
    }

    #[cfg(feature = "gpu-tests")]
    #[test]
    fn headless_context_creation() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let result = rt.block_on(GpuContext::new_headless());
        // May fail in CI without GPU -- that is acceptable.
        if let Ok(ctx) = result {
            assert!(ctx.surface.is_none());
            assert!(ctx.surface_config.is_none());
            assert!(ctx.msaa_samples >= 1);
        }
    }
}
