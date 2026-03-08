//! Spatial zoning and cross-zone authority primitives.

pub mod authority;
pub mod config;
pub mod fence;
pub mod ghost;
pub mod handoff;
pub mod partition;
pub mod protocol;
pub mod runtime;
pub mod split_merge;

pub use authority::{
    AuthorityTransferManager, AuthorityTransition, AuthorityZoneId, NetworkIdentity,
    PendingTransfer, SingleWriterMode, TransferResult, TransferState,
};
pub use config::{
    AxisChoice, LoadMetrics, MergeThreshold, SplitPolicy, SplitResult, ZoneSplitPolicy, ZoneSpec,
};
pub use fence::{FenceResult, PendingMessage, SequenceFenceTracker};
pub use ghost::{
    AdjacentZone, BoundaryEntity, GhostEntity, GhostManager, GhostPolicy, GhostVisibilityScope,
    Position,
};
pub use handoff::{
    ActiveHandoff, HandoffCoordinator, HandoffOutcome, HandoffPhase, HandoffRequest,
};
pub use partition::{
    EntitySample, KdAxis, KdBoundary, KdPoint, KdTree, KdTreeNode, KdTreeSplitResult,
    MAX_ZONE_DEPTH,
};
pub use protocol::{
    CrossZoneCombatDecision, CrossZonePhysicsDecision, HandoffDecision, HandoffFailureMode,
    HandoffResult, SequenceFence,
};
pub use runtime::{
    ZoningRuntime, ZoningRuntimeConfig, ZoningRuntimeInput, ZoningRuntimeOutput,
    ZoningRuntimeState,
};
pub use split_merge::{
    AdjacentZonePair, SplitMergeConfig, SplitMergeDecision, SplitMergeManager,
};
