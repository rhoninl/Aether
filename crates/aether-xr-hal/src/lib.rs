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
    ActionValue, XrAction, XrActionSet,
};
pub use event::XrEvent;
pub use frame::{XrFrame, XrTime};
pub use haptics::{
    clamp_amplitude, HapticAction, HapticPulse, HapticTarget, XrHaptics, MAX_HAPTIC_AMPLITUDE,
    MIN_HAPTIC_AMPLITUDE,
};
pub use instance::{
    ExtensionId, GraphicsRequirements, InstanceConfig, InstanceProperties, SystemProperties,
    ViewConfigType, XrInstance,
};
pub use layer::{LayerBuilder, LayerSubmission, ProjectionLayerView};
pub use platform::{RuntimeDescriptor, XrPlatform};
pub use profile::{BindingPath, InteractionProfile};
pub use session::{
    ReferenceSpace, ReferenceSpaceType, SessionConfig, SessionState, SessionTransitionError,
    XrSession, DEFAULT_PREDICTION_OFFSET_NS,
};
pub use swapchain::{
    SwapchainConfig, SwapchainError, SwapchainFormat, SwapchainImageIndex, SwapchainState,
    SwapchainUsage, XrSwapchain, DEFAULT_HEIGHT, DEFAULT_SAMPLE_COUNT, DEFAULT_WIDTH,
    MAX_SWAPCHAIN_IMAGES,
};
pub use tracking::{
    ControllerAnalog, ControllerButtons, ControllerState, Hand, HandJoint, HandJointSet, Pose3,
    TrackingConfidence, TrackingPipeline, TrackingSnapshot, DEFAULT_TRACKING_PREDICTION_NS,
    MAX_HAND_JOINTS,
};
pub use view::{Fov, View};
