//! Avatar model primitives for animation, tracking, IK solving, rendering, and rating.

pub mod animation;
pub mod avatar_lod;
pub mod avatar_shader;
pub mod blend_shapes;
pub mod calibration;
pub mod constraints;
pub mod fabrik;
pub mod foot_placement;
pub mod formats;
pub mod ik;
pub mod lipsync;
pub mod lod;
pub mod performance_rating;
pub mod rating;
pub mod skeleton;
pub mod skeleton_eval;
pub mod skinning;
pub mod state_machine;
pub mod tracking;
pub mod viseme;

pub use animation::{
    BlendCurve, BlendPoint, BlendStateInput, BlendTransition, BlendTransitionKind,
    LocomotionIntent, ProceduralGesture, ProceduralStateMachine,
};
pub use avatar_lod::{
    AvatarLodConfig, AvatarLodRenderHints, AvatarLodTier, AvatarLodTransition, select_lod_tier,
};
pub use avatar_shader::{
    AvatarMaterialConfig, AvatarPbrProperties, AvatarShaderPermutation, EyeRefractionConfig,
    ShaderFeatureFlags, SssProfile, SubsurfaceScatteringConfig,
};
pub use blend_shapes::{
    BlendShapeDispatch, BlendShapeSet, BlendShapeTarget, BlendShapeVertexDelta,
    BlendShapeWeights, GpuBlendShapeConfig,
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
pub use performance_rating::{
    AvatarMeshStats, BudgetResource, BudgetViolation, PerformanceBudgetTable, ValidationResult,
    classify_avatar, validate_avatar,
};
pub use rating::{
    AvatarBudget, AvatarPerformanceRating, AvatarRatingBucket, BudgetConstraint,
    PerformanceOverride,
};
pub use skeleton::{Bone, IkTarget, Skeleton};
pub use skeleton_eval::{
    BoneTransform, SkeletonPose, compute_bone_matrices, compute_skinning_matrices,
    compute_world_transforms,
};
pub use skinning::{
    BoneMatrixPalette, GpuSkinningConfig, SkinVertex, SkinningDispatch, SkinningMethod,
};
pub use state_machine::{AnimationOutput, AnimationStateMachine};
pub use tracking::{IkConfiguration, IkPoint, TrackingFrame, TrackingSource};
pub use viseme::{VisemeEvaluator, VisemeWeights};
