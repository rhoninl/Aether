//! Input and interaction primitives for VR clients and runtimes.

pub mod actions;
pub mod adapter;
pub mod capabilities;
pub mod haptics;
pub mod locomotion;
pub mod openxr;

pub use actions::{ActionPhase, GrabState, InteractionEvent, InteractionTarget, Pose3, XRButton};
pub use adapter::{InputFrame, InputFrameError, RuntimeAdapter};
pub use capabilities::{
    Capability, CapabilityError, HeadsetProfile, InputActionPath, InputBackend, InputFrameHint, ControllerType,
};
pub use haptics::{HapticChannel, HapticEffect, HapticRequest, HapticWave};
pub use locomotion::{ComfortProfile, ComfortStyle, LocomotionMode, LocomotionProfile, TeleportAnchor};
pub use openxr::OpenXrAdapter;
