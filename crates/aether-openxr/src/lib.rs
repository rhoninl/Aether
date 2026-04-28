pub mod error;
pub mod frame_loop;
pub mod input_actions;
pub mod instance;
pub mod session;
pub mod swapchain;

// HAL trait implementations against the real OpenXR loader. Gated on both
// the feature flag and the host target — `openxr` only links on Linux and
// Windows. See docs/design/xr-hal-refactor.md §11 (open questions).
#[cfg(all(
    feature = "openxr-runtime",
    any(target_os = "linux", target_os = "windows")
))]
pub mod hal;

#[cfg(all(
    feature = "openxr-runtime",
    any(target_os = "linux", target_os = "windows")
))]
pub use hal::{
    OpenXrActionSet, OpenXrHalFrame, OpenXrHalSession, OpenXrHaptics, OpenXrInstance,
    OpenXrPlatform, OpenXrSwapchain,
};

pub use error::OpenXrError;

// Re-export the raw `openxr` binding crate when the `openxr-runtime` feature
// is enabled on a supported host (Linux/Windows). The dep is target-gated in
// Cargo.toml because the `openxr` loader is not available on macOS. Lets
// downstream consumers confirm the dep is wired and use the loader types
// directly during the HAL refactor migration. See
// docs/design/xr-hal-refactor.md §9.
#[cfg(all(
    feature = "openxr-runtime",
    any(target_os = "linux", target_os = "windows")
))]
pub use openxr;
