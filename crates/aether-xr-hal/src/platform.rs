//! Top-level platform entry point (design doc §5.1, P2-A).

use crate::instance::{ExtensionId, InstanceConfig, XrInstance};

/// Static description of an available runtime, returned from
/// `XrPlatform::available()`. The OpenXR backend produces one of these per
/// concrete runtime/loader pair; the emulator backend produces a single
/// hard-coded entry.
#[derive(Debug, Clone)]
pub struct RuntimeDescriptor {
    pub name: String,
    pub extensions: Vec<ExtensionId>,
}

/// Platform-level entry: discover runtimes and create an instance.
pub trait XrPlatform {
    type Instance: XrInstance;
    type Error: std::error::Error + Send + Sync + 'static;

    fn available(&self) -> Result<Vec<RuntimeDescriptor>, Self::Error>;
    fn create_instance(&self, config: InstanceConfig) -> Result<Self::Instance, Self::Error>;
}
