//! OpenXR swapchain management for stereo rendering.
//!
//! Manages per-eye GLES swapchain images provided by the OpenXR runtime.

use crate::OpenXrError;

/// Recommended eye resolution from the runtime.
#[derive(Debug, Clone, Copy)]
pub struct EyeResolution {
    pub width: u32,
    pub height: u32,
}

/// Swapchain configuration.
#[derive(Debug, Clone)]
pub struct SwapchainConfig {
    pub eye_resolution: EyeResolution,
    pub sample_count: u32,
}

impl Default for SwapchainConfig {
    fn default() -> Self {
        Self {
            eye_resolution: EyeResolution {
                width: 2064,
                height: 2208,
            },
            sample_count: 1,
        }
    }
}

/// Represents an OpenXR swapchain for one eye.
///
/// Placeholder: will wrap `openxr::Swapchain<OpenGL>` with GL texture IDs.
pub struct XrSwapchain {
    config: SwapchainConfig,
    image_count: u32,
    current_index: u32,
    acquired: bool,
}

impl XrSwapchain {
    /// Create a new swapchain with the given config.
    pub fn new(config: SwapchainConfig) -> Result<Self, OpenXrError> {
        if config.eye_resolution.width == 0 || config.eye_resolution.height == 0 {
            return Err(OpenXrError::SwapchainCreation(
                "resolution cannot be zero".to_string(),
            ));
        }

        let image_count = 3; // typical triple-buffering

        log::info!(
            "Created swapchain: {}x{} ({} images)",
            config.eye_resolution.width,
            config.eye_resolution.height,
            image_count
        );

        Ok(Self {
            config,
            image_count,
            current_index: 0,
            acquired: false,
        })
    }

    /// Acquire the next swapchain image. Returns the image index.
    pub fn acquire(&mut self) -> Result<u32, OpenXrError> {
        if self.acquired {
            return Err(OpenXrError::FrameError(
                "swapchain already acquired".to_string(),
            ));
        }
        self.current_index = (self.current_index + 1) % self.image_count;
        self.acquired = true;
        Ok(self.current_index)
    }

    /// Release the current swapchain image.
    pub fn release(&mut self) -> Result<(), OpenXrError> {
        if !self.acquired {
            return Err(OpenXrError::FrameError(
                "swapchain not acquired".to_string(),
            ));
        }
        self.acquired = false;
        Ok(())
    }

    pub fn width(&self) -> u32 {
        self.config.eye_resolution.width
    }

    pub fn height(&self) -> u32 {
        self.config.eye_resolution.height
    }

    pub fn image_count(&self) -> u32 {
        self.image_count
    }

    pub fn is_acquired(&self) -> bool {
        self.acquired
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_swapchain_default() {
        let sc = XrSwapchain::new(SwapchainConfig::default()).unwrap();
        assert_eq!(sc.width(), 2064);
        assert_eq!(sc.height(), 2208);
        assert_eq!(sc.image_count(), 3);
    }

    #[test]
    fn create_swapchain_zero_resolution_fails() {
        let config = SwapchainConfig {
            eye_resolution: EyeResolution {
                width: 0,
                height: 100,
            },
            sample_count: 1,
        };
        assert!(XrSwapchain::new(config).is_err());
    }

    #[test]
    fn acquire_and_release() {
        let mut sc = XrSwapchain::new(SwapchainConfig::default()).unwrap();
        assert!(!sc.is_acquired());

        let idx = sc.acquire().unwrap();
        assert!(sc.is_acquired());
        assert!(idx < sc.image_count());

        sc.release().unwrap();
        assert!(!sc.is_acquired());
    }

    #[test]
    fn double_acquire_fails() {
        let mut sc = XrSwapchain::new(SwapchainConfig::default()).unwrap();
        sc.acquire().unwrap();
        assert!(sc.acquire().is_err());
    }

    #[test]
    fn release_without_acquire_fails() {
        let mut sc = XrSwapchain::new(SwapchainConfig::default()).unwrap();
        assert!(sc.release().is_err());
    }

    #[test]
    fn acquire_cycles_through_images() {
        let mut sc = XrSwapchain::new(SwapchainConfig::default()).unwrap();
        let mut indices = Vec::new();
        for _ in 0..6 {
            let idx = sc.acquire().unwrap();
            indices.push(idx);
            sc.release().unwrap();
        }
        // Should cycle through 3 images
        assert_eq!(indices[0], indices[3]);
    }

    #[test]
    fn default_config_quest3_resolution() {
        let config = SwapchainConfig::default();
        assert_eq!(config.eye_resolution.width, 2064);
        assert_eq!(config.eye_resolution.height, 2208);
    }
}
