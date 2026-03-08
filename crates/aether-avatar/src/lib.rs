//! Avatar model primitives for animation, tracking, IK solving, and rating.

pub mod animation;
pub mod calibration;
pub mod constraints;
pub mod fabrik;
pub mod foot_placement;
pub mod formats;
pub mod ik;
pub mod lipsync;
pub mod lod;
pub mod rating;
pub mod skeleton;
pub mod state_machine;
pub mod tracking;
pub mod viseme;

pub use animation::{
    BlendCurve, BlendPoint, BlendStateInput, BlendTransition, BlendTransitionKind,
    LocomotionIntent, ProceduralGesture, ProceduralStateMachine,
};
pub use calibration::{CalibrationData, CalibrationError, calibrate_from_tpose};
pub use constraints::{ConstraintSet, JointConstraint};
pub use fabrik::{FabrikConfig, FabrikResult, FabrikSolver};
pub use foot_placement::{FootIkResult, FootPlacement, foot_ik};
pub use formats::{
    AvatarAssetId, AvatarFormat, AvatarFormatError, AvatarImportDecision, AvatarMetadata,
};
pub use ik::{BodyProportions, FullBodyPose, IkResult, solve_six_point, solve_three_point};
pub use lipsync::{LipSyncConfig, LipSyncFrame, VisemeCurve};
pub use lod::{AvatarLodProfile, LodDistanceBand, LodLevel};
pub use rating::{
    AvatarBudget, AvatarPerformanceRating, AvatarRatingBucket, BudgetConstraint,
    PerformanceOverride,
};
pub use skeleton::{Bone, IkTarget, Skeleton};
pub use state_machine::{AnimationOutput, AnimationStateMachine};
pub use tracking::{IkConfiguration, IkPoint, TrackingFrame, TrackingSource};
pub use viseme::{VisemeEvaluator, VisemeWeights};
