//! Instance + system properties (design doc §5.2, P2-A).

use crate::event::XrEvent;
use crate::session::SessionConfig;

/// OpenXR `xrEnumerateInstanceExtensionProperties` returns extension *names* —
/// we keep them as opaque strings so unknown extensions discovered at runtime
/// can still be reported to the application without enum churn.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExtensionId(pub String);

impl ExtensionId {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ExtensionId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// `xrEnumerateViewConfigurations` discriminator. Only the two values
/// applications actually use are surfaced for now (mono for emulator, stereo
/// for HMDs). New entries can be added without breaking the trait surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ViewConfigType {
    Mono,
    Stereo,
}

#[derive(Debug, Clone, Default)]
pub struct InstanceProperties {
    pub runtime_name: String,
    pub runtime_version: u64,
}

#[derive(Debug, Clone, Default)]
pub struct SystemProperties {
    pub system_name: String,
    pub vendor_id: u32,
    pub max_swapchain_image_width: u32,
    pub max_swapchain_image_height: u32,
    pub max_layer_count: u32,
}

/// Caller-supplied configuration to `xrCreateInstance`.
#[derive(Debug, Clone)]
pub struct InstanceConfig {
    pub application_name: String,
    pub application_version: u32,
    pub engine_name: String,
    pub engine_version: u32,
    pub required_extensions: Vec<ExtensionId>,
    pub optional_extensions: Vec<ExtensionId>,
}

impl Default for InstanceConfig {
    fn default() -> Self {
        Self {
            application_name: "Aether".to_string(),
            application_version: 1,
            engine_name: "Aether".to_string(),
            engine_version: 1,
            required_extensions: Vec::new(),
            optional_extensions: Vec::new(),
        }
    }
}

/// Backend hint for `xrCreateSession`'s graphics binding. V1 is Vulkan-only on
/// the OpenXR backend (design doc §8); the emulator backend ignores this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphicsRequirements {
    Vulkan,
    Headless,
}

/// `XrInstance` owns extensions, system, and the event queue. Sessions are
/// created from instances; one instance can create one active session at a time.
pub trait XrInstance {
    type Session;
    type Error: std::error::Error + Send + Sync + 'static;

    fn properties(&self) -> InstanceProperties;
    fn system_properties(&self) -> SystemProperties;
    fn enabled_extensions(&self) -> &[ExtensionId];
    fn view_configurations(&self) -> &[ViewConfigType];

    /// Pump the runtime event queue (`xrPollEvent`); returns events accumulated
    /// since the last poll.
    fn poll_events(&mut self) -> Vec<XrEvent>;

    fn create_session(
        &self,
        config: SessionConfig,
        graphics: GraphicsRequirements,
    ) -> Result<Self::Session, Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_id_from_str() {
        let e: ExtensionId = "XR_EXT_hand_tracking".into();
        assert_eq!(e.as_str(), "XR_EXT_hand_tracking");
    }

    #[test]
    fn instance_config_default_is_aether() {
        let c = InstanceConfig::default();
        assert_eq!(c.application_name, "Aether");
        assert!(c.required_extensions.is_empty());
    }
}
