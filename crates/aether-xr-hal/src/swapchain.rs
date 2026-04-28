//! Swapchain trait and supporting value types (design doc §5.5, §8).
//!
//! `XrSwapchain` mirrors the OpenXR swapchain image-rotation contract:
//! `xrEnumerateSwapchainImages` / `xrAcquireSwapchainImage` /
//! `xrWaitSwapchainImage` / `xrReleaseSwapchainImage`. The image type is left
//! as an associated type so this crate avoids a `wgpu` dependency in V1; backends
//! bind `Image = wgpu::Texture` once the renderer integration lands.

/// Index returned by `xrAcquireSwapchainImage`. Wraps a `u32` (OpenXR's
/// `XrSwapchainImageIndex`) so the API never exposes a raw counter that can be
/// mistaken for an image count or a frame index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SwapchainImageIndex(pub u32);

impl SwapchainImageIndex {
    pub fn get(self) -> u32 {
        self.0
    }
}

// TODO(P1-C): the design doc §8 moves `SwapchainConfig` / `SwapchainFormat` /
// `SwapchainUsage` from `aether-input::openxr_swapchain` into this crate as
// wgpu-aligned value types. P1-C owns the canonical definitions; the
// placeholders below let P2-C (the trait surface) compile in isolation. When
// P1-C lands, replace these with the merged value types and update
// `XrSwapchain::create_*` call sites accordingly.

/// Placeholder for the swapchain creation descriptor. See P1-C for the canonical
/// definition (extent, sample count, array size, format, usage, mip count).
#[derive(Debug, Clone)]
pub struct SwapchainConfig {
    pub width: u32,
    pub height: u32,
    pub sample_count: u32,
    pub array_size: u32,
    pub format: SwapchainFormat,
    pub usage: SwapchainUsage,
}

/// Placeholder for the wgpu-aligned color/depth format enum (see P1-C).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SwapchainFormat {
    /// 8-bit-per-channel sRGB color, 4 channels.
    Rgba8UnormSrgb,
    /// 32-bit float depth.
    Depth32Float,
}

/// Placeholder for the bitflag set describing how the swapchain images will be
/// used (color attachment, depth attachment, sampled, etc.). See P1-C.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwapchainUsage(pub u32);

impl SwapchainUsage {
    pub const COLOR_ATTACHMENT: Self = Self(1 << 0);
    pub const DEPTH_STENCIL_ATTACHMENT: Self = Self(1 << 1);
    pub const SAMPLED: Self = Self(1 << 2);
    pub const TRANSFER_SRC: Self = Self(1 << 3);
    pub const TRANSFER_DST: Self = Self(1 << 4);
}

/// Per-eye swapchain handle.
///
/// Backends own image rotation; consumers acquire → wait → render → release per
/// frame, exactly as the OpenXR spec requires. `Image` is an associated type to
/// keep `aether-xr-hal` free of a `wgpu` dependency in V1; both the OpenXR and
/// emulator backends bind it to `wgpu::Texture` so render code is identical.
pub trait XrSwapchain {
    /// Backend-specific image handle. In V1 both backends will set this to
    /// `wgpu::Texture` (see design doc §8).
    type Image;
    type Error: std::error::Error + Send + Sync + 'static;

    /// Static slice of all images owned by this swapchain
    /// (`xrEnumerateSwapchainImages`).
    fn images(&self) -> &[Self::Image];

    /// Acquire the next image from the swapchain. Returns the index into
    /// `images()` (`xrAcquireSwapchainImage`).
    fn acquire(&mut self) -> Result<SwapchainImageIndex, Self::Error>;

    /// Wait until the acquired image is ready for rendering, with a nanosecond
    /// timeout (`xrWaitSwapchainImage`).
    fn wait(&mut self, timeout_ns: u64) -> Result<(), Self::Error>;

    /// Release the most-recently-acquired image back to the runtime
    /// (`xrReleaseSwapchainImage`).
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
    fn swapchain_usage_flags_are_distinct() {
        // Sanity: bit positions don't collide.
        assert_ne!(
            SwapchainUsage::COLOR_ATTACHMENT,
            SwapchainUsage::DEPTH_STENCIL_ATTACHMENT
        );
        assert_ne!(SwapchainUsage::COLOR_ATTACHMENT, SwapchainUsage::SAMPLED);
    }
}
