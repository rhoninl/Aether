//! Platform profile and fidelity adaptation contracts.

pub mod capabilities;
pub mod compliance;
pub mod builds;
pub mod controls;
pub mod runtime;

pub use capabilities::{InputBackend, PlatformKind, PlatformProfile, QualityClass};
pub use compliance::{StoreCompliance, StoreRegion};
pub use builds::{WasmExecutionMode, WasmProfile};
pub use controls::{FidelityMode, SceneScaleMode, VisualMode};
pub use runtime::{
    PlatformRuntime, PlatformRuntimeConfig, PlatformRuntimeInput, PlatformRuntimeOutput, PlatformSessionIntent,
};
