//! Swapchain trait + value types (design doc §5.5, §8, P1-C/P2-C).
//!
//! `XrSwapchain` mirrors the OpenXR swapchain image-rotation contract:
//! `xrEnumerateSwapchainImages` / `xrAcquireSwapchainImage` /
//! `xrWaitSwapchainImage` / `xrReleaseSwapchainImage`. The image type is left
//! as an associated type so this crate avoids a `wgpu` dependency in V1; backends
//! bind `Image = wgpu::Texture` once renderer integration lands.

pub const MAX_SWAPCHAIN_IMAGES: u32 = 4;
pub const DEFAULT_WIDTH: u32 = 1440;
pub const DEFAULT_HEIGHT: u32 = 1600;
pub const DEFAULT_SAMPLE_COUNT: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SwapchainFormat {
    Rgba8Srgb,
    Rgba8Unorm,
    Bgra8Srgb,
    Bgra8Unorm,
    Rgba16Float,
    Rgb10A2Unorm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SwapchainUsage {
    ColorAttachment,
    Sampled,
    ColorAttachmentAndSampled,
}

#[derive(Debug, Clone)]
pub struct SwapchainConfig {
    pub width: u32,
    pub height: u32,
    pub format: SwapchainFormat,
    pub sample_count: u32,
    pub image_count: u32,
    pub usage: SwapchainUsage,
}

impl Default for SwapchainConfig {
    fn default() -> Self {
        Self {
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            format: SwapchainFormat::Rgba8Srgb,
            sample_count: DEFAULT_SAMPLE_COUNT,
            image_count: 3,
            usage: SwapchainUsage::ColorAttachment,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwapchainError {
    AlreadyAcquired,
    NotAcquired,
    NotWaited,
    NoImageToRelease,
    NotCreated,
    InvalidConfig(String),
    ImageIndexOutOfRange { index: u32, count: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwapchainState {
    Idle,
    Acquired,
    Ready,
}

/// Index returned by `xrAcquireSwapchainImage`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SwapchainImageIndex(pub u32);

impl SwapchainImageIndex {
    pub fn get(self) -> u32 {
        self.0
    }
}

/// Per-eye swapchain handle. Backends own image rotation; consumers acquire →
/// wait → render → release per frame, exactly as the OpenXR spec requires.
pub trait XrSwapchain {
    /// Backend-specific image handle. In V1 both backends will set this to
    /// `wgpu::Texture` (see design doc §8).
    type Image;
    type Error: std::error::Error + Send + Sync + 'static;

    fn images(&self) -> &[Self::Image];
    fn acquire(&mut self) -> Result<SwapchainImageIndex, Self::Error>;
    fn wait(&mut self, timeout_ns: u64) -> Result<(), Self::Error>;
    fn release(&mut self) -> Result<(), Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swapchain_image_index_round_trips() {
        let i = SwapchainImageIndex(7);
        assert_eq!(i.get(), 7);
    }

    #[test]
    fn default_config_has_safe_dimensions() {
        let c = SwapchainConfig::default();
        assert_eq!(c.width, DEFAULT_WIDTH);
        assert_eq!(c.height, DEFAULT_HEIGHT);
        assert_eq!(c.format, SwapchainFormat::Rgba8Srgb);
        assert!(c.image_count > 0 && c.image_count <= MAX_SWAPCHAIN_IMAGES);
    }
}
