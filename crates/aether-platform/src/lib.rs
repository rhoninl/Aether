//! Platform profile and fidelity adaptation contracts.

pub mod builds;
pub mod capabilities;
pub mod compliance;
pub mod controls;
pub mod runtime;

// New modules: platform detection, quality profiles, feature toggles,
// performance budgets, and hardware capabilities.
pub mod budgets;
pub mod detection;
pub mod features;
pub mod platform_capabilities;
pub mod profiles;

pub use builds::{WasmExecutionMode, WasmProfile};
pub use capabilities::{InputBackend, PlatformKind, PlatformProfile, QualityClass};
pub use compliance::{StoreCompliance, StoreRegion};
pub use controls::{FidelityMode, SceneScaleMode, VisualMode};
pub use runtime::{
    PlatformRuntime, PlatformRuntimeConfig, PlatformRuntimeInput, PlatformRuntimeOutput,
    PlatformSessionIntent,
};

pub use budgets::{BudgetReport, BudgetUsage, PerformanceBudget};
pub use detection::{all_platforms, detect_platform, Platform};
pub use features::{Feature, FeatureFlags};
pub use platform_capabilities::{GpuTier, PlatformCapabilities};
pub use profiles::QualityProfile;
