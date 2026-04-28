//! OpenXR-aligned HAL for Aether's XR subsystem.
//!
//! This crate is the trait + value-type layer of the XR HAL refactor
//! (see `docs/design/xr-hal-refactor.md`). Backends — `aether-openxr` for real
//! hardware and `aether-vr-emulator` for desktop development — implement these
//! traits.

pub mod action;
pub mod event;
pub mod frame;
pub mod haptics;
pub mod instance;
pub mod layer;
pub mod platform;
pub mod profile;
pub mod session;
pub mod swapchain;
pub mod tracking;
pub mod view;

pub use action::{
    ActionBuilder, ActionDecl, ActionKind, ActionManifest, ActionSetHandle, ActionState,
    ActionValue, Pose3, XrAction, XrActionSet,
};
pub use frame::XrFrame;
pub use haptics::{HapticEffect, HapticTarget, XrHaptics};
pub use profile::{BindingPath, InteractionProfile};
pub use swapchain::{
    SwapchainConfig, SwapchainFormat, SwapchainImageIndex, SwapchainUsage, XrSwapchain,
};
