//! Platform profile and fidelity adaptation contracts.

pub mod capabilities;
pub mod compliance;
pub mod builds;
pub mod controls;
pub mod runtime;

// New modules: platform detection, quality profiles, feature toggles,
// performance budgets, and hardware capabilities.
pub mod detection;
pub mod profiles;
pub mod features;
pub mod budgets;
pub mod platform_capabilities;

pub use capabilities::{InputBackend, PlatformKind, PlatformProfile, QualityClass};
pub use compliance::{StoreCompliance, StoreRegion};
pub use builds::{WasmExecutionMode, WasmProfile};
pub use controls::{FidelityMode, SceneScaleMode, VisualMode};
pub use runtime::{
    PlatformRuntime, PlatformRuntimeConfig, PlatformRuntimeInput, PlatformRuntimeOutput, PlatformSessionIntent,
};

pub use detection::{Platform, detect_platform, all_platforms};
pub use profiles::QualityProfile;
pub use features::{Feature, FeatureFlags};
pub use budgets::{PerformanceBudget, BudgetUsage, BudgetReport};
pub use platform_capabilities::{GpuTier, PlatformCapabilities};
