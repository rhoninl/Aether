//! OpenXR instance creation and system discovery.
//!
//! On Quest, this loads the Meta OpenXR runtime and finds the HMD system.
//! The actual `openxr` crate is not yet a dependency — this module provides
//! the high-level API that will wrap it once linked.

use crate::OpenXrError;

const DEFAULT_APP_NAME: &str = "Aether VR";
const ENGINE_NAME: &str = "Aether Engine";
const ENGINE_VERSION: u32 = 1;

/// Configuration for creating an OpenXR instance.
#[derive(Debug, Clone)]
pub struct InstanceConfig {
    pub app_name: String,
    pub app_version: u32,
}

impl Default for InstanceConfig {
    fn default() -> Self {
        Self {
            app_name: DEFAULT_APP_NAME.to_string(),
            app_version: 1,
        }
    }
}

/// Represents an OpenXR instance and discovered system.
///
/// This is currently a placeholder that validates configuration.
/// The real implementation will wrap `openxr::Instance` + `SystemId`.
#[derive(Debug)]
pub struct XrInstance {
    config: InstanceConfig,
    initialized: bool,
}

impl XrInstance {
    /// Create a new XR instance with the given config.
    ///
    /// In production this calls `xrCreateInstance` and `xrGetSystem`.
    /// Currently validates config and marks as initialized.
    pub fn new(config: InstanceConfig) -> Result<Self, OpenXrError> {
        if config.app_name.is_empty() {
            return Err(OpenXrError::InstanceCreation(
                "app_name cannot be empty".to_string(),
            ));
        }

        log::info!(
            "Creating OpenXR instance: app='{}' engine='{}'",
            config.app_name,
            ENGINE_NAME
        );

        Ok(Self {
            config,
            initialized: true,
        })
    }

    pub fn app_name(&self) -> &str {
        &self.config.app_name
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    pub fn engine_name(&self) -> &'static str {
        ENGINE_NAME
    }

    pub fn engine_version(&self) -> u32 {
        ENGINE_VERSION
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_instance_default_config() {
        let instance = XrInstance::new(InstanceConfig::default()).unwrap();
        assert!(instance.is_initialized());
        assert_eq!(instance.app_name(), DEFAULT_APP_NAME);
    }

    #[test]
    fn create_instance_custom_name() {
        let config = InstanceConfig {
            app_name: "My VR App".to_string(),
            app_version: 2,
        };
        let instance = XrInstance::new(config).unwrap();
        assert_eq!(instance.app_name(), "My VR App");
    }

    #[test]
    fn create_instance_empty_name_fails() {
        let config = InstanceConfig {
            app_name: String::new(),
            app_version: 1,
        };
        let err = XrInstance::new(config).unwrap_err();
        match err {
            OpenXrError::InstanceCreation(msg) => assert!(msg.contains("empty")),
            _ => panic!("expected InstanceCreation error"),
        }
    }

    #[test]
    fn engine_info() {
        let instance = XrInstance::new(InstanceConfig::default()).unwrap();
        assert_eq!(instance.engine_name(), "Aether Engine");
        assert_eq!(instance.engine_version(), 1);
    }

    #[test]
    fn default_config_values() {
        let config = InstanceConfig::default();
        assert_eq!(config.app_name, "Aether VR");
        assert_eq!(config.app_version, 1);
    }
}
