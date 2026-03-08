//! VR hand interaction physics for the Aether VR engine.
//!
//! This module provides VR-specific physics primitives:
//! - **Grab system**: Joint-based grabbing (fixed, spring, hinge)
//! - **Hand collision**: CCD for fast hand movements
//! - **Haptic feedback**: Mapping collision forces to vibration
//! - **Throw detection**: Velocity tracking for natural throwing
//! - **Manipulation**: Rotate/scale objects while held
//! - **UI interaction**: Hand raycasts for UI pointing

pub mod grab;
pub mod hand_collision;
pub mod haptic;
pub mod manipulation;
pub mod math;
pub mod throw_detection;
pub mod ui_interaction;

pub use grab::{GrabConstraint, GrabJointKind, GrabState, GrabSystem, GrabUpdateResult};
pub use hand_collision::{
    CollisionSphere, HandColliderConfig, HandCollisionDetector, HandCollisionResult,
};
pub use haptic::{
    Hand, HapticCurve, HapticEvent, HapticFeedbackConfig, HapticFeedbackMapper,
};
pub use manipulation::{
    ManipulationConfig, ManipulationMode, ManipulationResult, ManipulationState,
};
pub use throw_detection::{ThrowDetector, ThrowDetectorConfig, ThrowResult, VelocitySample};
pub use ui_interaction::{
    UiHitResult, UiInteractionEvent, UiInteractionPhase, UiInteractionState, UiRaycastConfig,
};
