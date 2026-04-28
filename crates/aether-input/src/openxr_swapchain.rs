//! OpenXR swapchain manager. Value types re-exported from
//! `aether_xr_hal::swapchain` (P1-C); the [`SwapchainManager`] state machine
//! stays here until P5 lands a real wgpu-backed swapchain.

pub use aether_xr_hal::swapchain::{
    SwapchainConfig, SwapchainError, SwapchainFormat, SwapchainImageIndex, SwapchainState,
    SwapchainUsage, DEFAULT_HEIGHT, DEFAULT_SAMPLE_COUNT, DEFAULT_WIDTH, MAX_SWAPCHAIN_IMAGES,
};

/// Manages the swapchain lifecycle: acquire -> wait -> render -> release.
#[derive(Debug)]
pub struct SwapchainManager {
    config: SwapchainConfig,
    state: SwapchainState,
    current_image: Option<SwapchainImageIndex>,
    created: bool,
    acquire_count: u64,
    release_count: u64,
    next_image_index: u32,
}

impl SwapchainManager {
    /// Create a new swapchain manager with the given configuration.
    ///
    /// The swapchain is not created until `create()` is called.
    pub fn new(config: SwapchainConfig) -> Self {
        Self {
            config,
            state: SwapchainState::Idle,
            current_image: None,
            created: false,
            acquire_count: 0,
            release_count: 0,
            next_image_index: 0,
        }
    }

    /// Validate the configuration.
    pub fn validate_config(config: &SwapchainConfig) -> Result<(), SwapchainError> {
        if config.width == 0 || config.height == 0 {
            return Err(SwapchainError::InvalidConfig(
                "width and height must be > 0".to_string(),
            ));
        }
        if config.image_count == 0 {
            return Err(SwapchainError::InvalidConfig(
                "image_count must be > 0".to_string(),
            ));
        }
        if config.image_count > MAX_SWAPCHAIN_IMAGES {
            return Err(SwapchainError::InvalidConfig(format!(
                "image_count {} exceeds maximum {}",
                config.image_count, MAX_SWAPCHAIN_IMAGES
            )));
        }
        if config.sample_count == 0 {
            return Err(SwapchainError::InvalidConfig(
                "sample_count must be > 0".to_string(),
            ));
        }
        Ok(())
    }

    /// Create the swapchain (validate config and mark as created).
    pub fn create(&mut self) -> Result<(), SwapchainError> {
        Self::validate_config(&self.config)?;
        self.created = true;
        self.state = SwapchainState::Idle;
        self.next_image_index = 0;
        Ok(())
    }

    /// Get the swapchain configuration.
    pub fn config(&self) -> &SwapchainConfig {
        &self.config
    }

    /// Get the current swapchain state.
    pub fn state(&self) -> SwapchainState {
        self.state
    }

    /// Whether the swapchain has been created.
    pub fn is_created(&self) -> bool {
        self.created
    }

    /// Get the currently acquired image index, if any.
    pub fn current_image(&self) -> Option<SwapchainImageIndex> {
        self.current_image
    }

    /// Get the number of images acquired.
    pub fn acquire_count(&self) -> u64 {
        self.acquire_count
    }

    /// Get the number of images released.
    pub fn release_count(&self) -> u64 {
        self.release_count
    }

    /// Acquire the next swapchain image.
    ///
    /// Transitions: Idle -> Acquired.
    pub fn acquire_image(&mut self) -> Result<SwapchainImageIndex, SwapchainError> {
        if !self.created {
            return Err(SwapchainError::NotCreated);
        }
        if self.state != SwapchainState::Idle {
            return Err(SwapchainError::AlreadyAcquired);
        }

        let index = SwapchainImageIndex(self.next_image_index);
        self.next_image_index = (self.next_image_index + 1) % self.config.image_count;
        self.current_image = Some(index);
        self.state = SwapchainState::Acquired;
        self.acquire_count = self.acquire_count.saturating_add(1);

        Ok(index)
    }

    /// Wait for the acquired image to be ready for rendering.
    ///
    /// Transitions: Acquired -> Ready.
    pub fn wait_image(&mut self) -> Result<SwapchainImageIndex, SwapchainError> {
        if !self.created {
            return Err(SwapchainError::NotCreated);
        }
        if self.state != SwapchainState::Acquired {
            return Err(SwapchainError::NotAcquired);
        }

        self.state = SwapchainState::Ready;
        Ok(self.current_image.unwrap())
    }

    /// Release the current image back to the swapchain.
    ///
    /// Transitions: Ready -> Idle.
    pub fn release_image(&mut self) -> Result<(), SwapchainError> {
        if !self.created {
            return Err(SwapchainError::NotCreated);
        }
        match self.state {
            SwapchainState::Idle => Err(SwapchainError::NoImageToRelease),
            SwapchainState::Acquired => Err(SwapchainError::NotWaited),
            SwapchainState::Ready => {
                self.current_image = None;
                self.state = SwapchainState::Idle;
                self.release_count = self.release_count.saturating_add(1);
                Ok(())
            }
        }
    }

    /// Perform a complete acquire -> wait -> release cycle (for testing).
    pub fn present_frame(&mut self) -> Result<SwapchainImageIndex, SwapchainError> {
        let index = self.acquire_image()?;
        self.wait_image()?;
        self.release_image()?;
        Ok(index)
    }

    /// Destroy the swapchain, releasing all resources.
    pub fn destroy(&mut self) {
        self.created = false;
        self.state = SwapchainState::Idle;
        self.current_image = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_manager() -> SwapchainManager {
        let mut mgr = SwapchainManager::new(SwapchainConfig::default());
        mgr.create().unwrap();
        mgr
    }

    // ---- Config validation ----

    #[test]
    fn default_config_is_valid() {
        let result = SwapchainManager::validate_config(&SwapchainConfig::default());
        assert!(result.is_ok());
    }

    #[test]
    fn zero_width_is_invalid() {
        let config = SwapchainConfig {
            width: 0,
            ..SwapchainConfig::default()
        };
        let err = SwapchainManager::validate_config(&config).unwrap_err();
        assert!(matches!(err, SwapchainError::InvalidConfig(_)));
    }

    #[test]
    fn zero_height_is_invalid() {
        let config = SwapchainConfig {
            height: 0,
            ..SwapchainConfig::default()
        };
        let err = SwapchainManager::validate_config(&config).unwrap_err();
        assert!(matches!(err, SwapchainError::InvalidConfig(_)));
    }

    #[test]
    fn zero_image_count_is_invalid() {
        let config = SwapchainConfig {
            image_count: 0,
            ..SwapchainConfig::default()
        };
        let err = SwapchainManager::validate_config(&config).unwrap_err();
        assert!(matches!(err, SwapchainError::InvalidConfig(_)));
    }

    #[test]
    fn excessive_image_count_is_invalid() {
        let config = SwapchainConfig {
            image_count: MAX_SWAPCHAIN_IMAGES + 1,
            ..SwapchainConfig::default()
        };
        let err = SwapchainManager::validate_config(&config).unwrap_err();
        assert!(matches!(err, SwapchainError::InvalidConfig(_)));
    }

    #[test]
    fn max_image_count_is_valid() {
        let config = SwapchainConfig {
            image_count: MAX_SWAPCHAIN_IMAGES,
            ..SwapchainConfig::default()
        };
        assert!(SwapchainManager::validate_config(&config).is_ok());
    }

    #[test]
    fn zero_sample_count_is_invalid() {
        let config = SwapchainConfig {
            sample_count: 0,
            ..SwapchainConfig::default()
        };
        let err = SwapchainManager::validate_config(&config).unwrap_err();
        assert!(matches!(err, SwapchainError::InvalidConfig(_)));
    }

    // ---- Initial state ----

    #[test]
    fn new_manager_is_not_created() {
        let mgr = SwapchainManager::new(SwapchainConfig::default());
        assert!(!mgr.is_created());
    }

    #[test]
    fn new_manager_is_idle() {
        let mgr = SwapchainManager::new(SwapchainConfig::default());
        assert_eq!(mgr.state(), SwapchainState::Idle);
    }

    #[test]
    fn created_manager_is_created() {
        let mgr = make_manager();
        assert!(mgr.is_created());
    }

    #[test]
    fn created_manager_is_idle() {
        let mgr = make_manager();
        assert_eq!(mgr.state(), SwapchainState::Idle);
    }

    #[test]
    fn initial_counts_are_zero() {
        let mgr = make_manager();
        assert_eq!(mgr.acquire_count(), 0);
        assert_eq!(mgr.release_count(), 0);
    }

    // ---- Acquire ----

    #[test]
    fn acquire_image_transitions_to_acquired() {
        let mut mgr = make_manager();
        let index = mgr.acquire_image().unwrap();
        assert_eq!(index, SwapchainImageIndex(0));
        assert_eq!(mgr.state(), SwapchainState::Acquired);
        assert_eq!(mgr.current_image(), Some(SwapchainImageIndex(0)));
    }

    #[test]
    fn acquire_increments_count() {
        let mut mgr = make_manager();
        mgr.acquire_image().unwrap();
        assert_eq!(mgr.acquire_count(), 1);
    }

    #[test]
    fn double_acquire_fails() {
        let mut mgr = make_manager();
        mgr.acquire_image().unwrap();
        let err = mgr.acquire_image().unwrap_err();
        assert_eq!(err, SwapchainError::AlreadyAcquired);
    }

    #[test]
    fn acquire_without_create_fails() {
        let mut mgr = SwapchainManager::new(SwapchainConfig::default());
        let err = mgr.acquire_image().unwrap_err();
        assert_eq!(err, SwapchainError::NotCreated);
    }

    // ---- Wait ----

    #[test]
    fn wait_image_transitions_to_ready() {
        let mut mgr = make_manager();
        mgr.acquire_image().unwrap();
        let index = mgr.wait_image().unwrap();
        assert_eq!(index, SwapchainImageIndex(0));
        assert_eq!(mgr.state(), SwapchainState::Ready);
    }

    #[test]
    fn wait_without_acquire_fails() {
        let mut mgr = make_manager();
        let err = mgr.wait_image().unwrap_err();
        assert_eq!(err, SwapchainError::NotAcquired);
    }

    #[test]
    fn wait_without_create_fails() {
        let mut mgr = SwapchainManager::new(SwapchainConfig::default());
        let err = mgr.wait_image().unwrap_err();
        assert_eq!(err, SwapchainError::NotCreated);
    }

    // ---- Release ----

    #[test]
    fn release_image_transitions_to_idle() {
        let mut mgr = make_manager();
        mgr.acquire_image().unwrap();
        mgr.wait_image().unwrap();
        mgr.release_image().unwrap();
        assert_eq!(mgr.state(), SwapchainState::Idle);
        assert!(mgr.current_image().is_none());
    }

    #[test]
    fn release_increments_count() {
        let mut mgr = make_manager();
        mgr.acquire_image().unwrap();
        mgr.wait_image().unwrap();
        mgr.release_image().unwrap();
        assert_eq!(mgr.release_count(), 1);
    }

    #[test]
    fn release_without_wait_fails() {
        let mut mgr = make_manager();
        mgr.acquire_image().unwrap();
        let err = mgr.release_image().unwrap_err();
        assert_eq!(err, SwapchainError::NotWaited);
    }

    #[test]
    fn release_without_acquire_fails() {
        let mut mgr = make_manager();
        let err = mgr.release_image().unwrap_err();
        assert_eq!(err, SwapchainError::NoImageToRelease);
    }

    #[test]
    fn release_without_create_fails() {
        let mut mgr = SwapchainManager::new(SwapchainConfig::default());
        let err = mgr.release_image().unwrap_err();
        assert_eq!(err, SwapchainError::NotCreated);
    }

    // ---- Full lifecycle ----

    #[test]
    fn full_acquire_wait_release_cycle() {
        let mut mgr = make_manager();

        let idx1 = mgr.acquire_image().unwrap();
        assert_eq!(idx1, SwapchainImageIndex(0));

        let idx2 = mgr.wait_image().unwrap();
        assert_eq!(idx2, SwapchainImageIndex(0));

        mgr.release_image().unwrap();
        assert_eq!(mgr.state(), SwapchainState::Idle);
    }

    #[test]
    fn sequential_frames_cycle_indices() {
        let mut mgr = make_manager(); // default image_count = 3

        let idx0 = mgr.present_frame().unwrap();
        assert_eq!(idx0, SwapchainImageIndex(0));

        let idx1 = mgr.present_frame().unwrap();
        assert_eq!(idx1, SwapchainImageIndex(1));

        let idx2 = mgr.present_frame().unwrap();
        assert_eq!(idx2, SwapchainImageIndex(2));

        // Wraps around
        let idx3 = mgr.present_frame().unwrap();
        assert_eq!(idx3, SwapchainImageIndex(0));
    }

    #[test]
    fn present_frame_increments_counts() {
        let mut mgr = make_manager();
        mgr.present_frame().unwrap();
        mgr.present_frame().unwrap();
        assert_eq!(mgr.acquire_count(), 2);
        assert_eq!(mgr.release_count(), 2);
    }

    // ---- Destroy ----

    #[test]
    fn destroy_resets_state() {
        let mut mgr = make_manager();
        mgr.acquire_image().unwrap();
        mgr.destroy();

        assert!(!mgr.is_created());
        assert_eq!(mgr.state(), SwapchainState::Idle);
        assert!(mgr.current_image().is_none());
    }

    #[test]
    fn acquire_after_destroy_fails() {
        let mut mgr = make_manager();
        mgr.destroy();
        let err = mgr.acquire_image().unwrap_err();
        assert_eq!(err, SwapchainError::NotCreated);
    }

    #[test]
    fn recreate_after_destroy_works() {
        let mut mgr = make_manager();
        mgr.destroy();
        mgr.create().unwrap();
        assert!(mgr.is_created());

        let idx = mgr.acquire_image().unwrap();
        // After recreate, index resets to 0
        assert_eq!(idx, SwapchainImageIndex(0));
    }

    // ---- Config access ----

    #[test]
    fn config_accessible() {
        let config = SwapchainConfig {
            width: 2048,
            height: 2048,
            format: SwapchainFormat::Rgba16Float,
            sample_count: 4,
            image_count: 2,
            usage: SwapchainUsage::ColorAttachmentAndSampled,
        };
        let mgr = SwapchainManager::new(config);
        assert_eq!(mgr.config().width, 2048);
        assert_eq!(mgr.config().height, 2048);
        assert_eq!(mgr.config().format, SwapchainFormat::Rgba16Float);
        assert_eq!(mgr.config().sample_count, 4);
        assert_eq!(mgr.config().image_count, 2);
        assert_eq!(
            mgr.config().usage,
            SwapchainUsage::ColorAttachmentAndSampled
        );
    }

    #[test]
    fn default_config_values() {
        let config = SwapchainConfig::default();
        assert_eq!(config.width, DEFAULT_WIDTH);
        assert_eq!(config.height, DEFAULT_HEIGHT);
        assert_eq!(config.format, SwapchainFormat::Rgba8Srgb);
        assert_eq!(config.sample_count, DEFAULT_SAMPLE_COUNT);
        assert_eq!(config.image_count, 3);
        assert_eq!(config.usage, SwapchainUsage::ColorAttachment);
    }

    // ---- Create with invalid config ----

    #[test]
    fn create_with_invalid_config_fails() {
        let config = SwapchainConfig {
            width: 0,
            ..SwapchainConfig::default()
        };
        let mut mgr = SwapchainManager::new(config);
        let err = mgr.create().unwrap_err();
        assert!(matches!(err, SwapchainError::InvalidConfig(_)));
        assert!(!mgr.is_created());
    }

    // ---- Single image swapchain ----

    #[test]
    fn single_image_swapchain_always_returns_index_zero() {
        let config = SwapchainConfig {
            image_count: 1,
            ..SwapchainConfig::default()
        };
        let mut mgr = SwapchainManager::new(config);
        mgr.create().unwrap();

        let idx1 = mgr.present_frame().unwrap();
        assert_eq!(idx1, SwapchainImageIndex(0));

        let idx2 = mgr.present_frame().unwrap();
        assert_eq!(idx2, SwapchainImageIndex(0));
    }
}
