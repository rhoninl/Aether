//! Input and interaction primitives for VR clients and runtimes.

pub mod actions;
pub mod adapter;
pub mod capabilities;
pub mod deadzone;
pub mod desktop;
pub mod graph;
pub mod haptics;
pub mod locomotion;
pub mod mapping;
pub mod movement;
pub mod openxr;
pub mod processing;
pub mod runtime;

pub use actions::{ActionPhase, GrabState, InteractionEvent, InteractionTarget, Pose3, XRButton};
pub use adapter::{InputFrame, InputFrameError, RuntimeAdapter};
pub use capabilities::{
    Capability, CapabilityError, ControllerType, HeadsetProfile, InputActionPath, InputBackend,
    InputFrameHint,
};
pub use deadzone::{apply_dead_zone, apply_sensitivity, DeadZoneConfig, DeadZoneShape, SensitivityCurve};
pub use desktop::{DesktopAdapter, DesktopAdapterConfig, DesktopInputState, KeyCode, MouseAxis, MouseButton};
pub use graph::{ActionEvent, ActionEventPhase, GestureDetector, InputGesture};
pub use haptics::{HapticChannel, HapticEffect, HapticRequest, HapticWave};
pub use locomotion::{ComfortProfile, ComfortStyle, LocomotionMode, LocomotionProfile, TeleportAnchor};
pub use mapping::{ActionBinding, ActionMap, InputSource};
pub use movement::{
    compute_smooth_move, compute_smooth_turn, compute_snap_turn, compute_teleport,
    direction_from_keys, rotate_direction_by_yaw, TeleportResult,
};
pub use openxr::OpenXrAdapter;
pub use processing::{InputPipeline, ProcessedAxes, RawInputState};
pub use runtime::{
    InputRuntime, InputRuntimeConfig, InputRuntimeInput, InputRuntimeOutput, PlayerInputFrame,
    SimulationIntent, SimulationRuntimeState,
};
