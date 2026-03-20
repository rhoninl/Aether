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
    select_lod_tier, AvatarLodConfig, AvatarLodRenderHints, AvatarLodTier, AvatarLodTransition,
};
pub use avatar_shader::{
    AvatarMaterialConfig, AvatarPbrProperties, AvatarShaderPermutation, EyeRefractionConfig,
    ShaderFeatureFlags, SssProfile, SubsurfaceScatteringConfig,
};
pub use blend_shapes::{
    BlendShapeDispatch, BlendShapeSet, BlendShapeTarget, BlendShapeVertexDelta, BlendShapeWeights,
    GpuBlendShapeConfig,
};
pub use calibration::{calibrate_from_tpose, CalibrationData, CalibrationError};
pub use constraints::{ConstraintSet, JointConstraint};
pub use fabrik::{FabrikConfig, FabrikResult, FabrikSolver};
pub use foot_placement::{foot_ik, FootIkResult, FootPlacement};
pub use formats::{
    AvatarAssetId, AvatarFormat, AvatarFormatError, AvatarImportDecision, AvatarMetadata,
};
pub use ik::{solve_six_point, solve_three_point, BodyProportions, FullBodyPose, IkResult};
pub use lipsync::{LipSyncConfig, LipSyncFrame, VisemeCurve};
pub use lod::{AvatarLodProfile, LodDistanceBand, LodLevel};
pub use performance_rating::{
    classify_avatar, validate_avatar, AvatarMeshStats, BudgetResource, BudgetViolation,
    PerformanceBudgetTable, ValidationResult,
};
pub use rating::{
    AvatarBudget, AvatarPerformanceRating, AvatarRatingBucket, BudgetConstraint,
    PerformanceOverride,
};
pub use skeleton::{Bone, IkTarget, Skeleton};
pub use skeleton_eval::{
    compute_bone_matrices, compute_skinning_matrices, compute_world_transforms, BoneTransform,
    SkeletonPose,
};
pub use skinning::{
    BoneMatrixPalette, GpuSkinningConfig, SkinVertex, SkinningDispatch, SkinningMethod,
};
pub use state_machine::{AnimationOutput, AnimationStateMachine};
pub use tracking::{IkConfiguration, IkPoint, TrackingFrame, TrackingSource};
pub use viseme::{VisemeEvaluator, VisemeWeights};
