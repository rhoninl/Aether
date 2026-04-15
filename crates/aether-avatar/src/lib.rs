//! Avatar model primitives for animation, tracking, and rating.

pub mod animation;
pub mod formats;
pub mod lod;
pub mod lipsync;
pub mod rating;
pub mod tracking;
pub mod tween;

pub use animation::{
    BlendCurve, BlendPoint, BlendStateInput, BlendTransition, BlendTransitionKind, LocomotionIntent,
    ProceduralGesture, ProceduralStateMachine,
};
pub use formats::{AvatarAssetId, AvatarFormat, AvatarFormatError, AvatarImportDecision, AvatarMetadata};
pub use lod::{AvatarLodProfile, LodDistanceBand, LodLevel};
pub use lipsync::{LipSyncConfig, LipSyncFrame, VisemeCurve};
pub use rating::{
    AvatarBudget, AvatarPerformanceRating, AvatarRatingBucket, BudgetConstraint, PerformanceOverride,
};
pub use tracking::{IkConfiguration, IkPoint, TrackingFrame, TrackingSource};
pub use tween::{Easing, Lerp, Tween, TweenTrack};

