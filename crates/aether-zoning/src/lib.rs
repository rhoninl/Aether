//! Spatial zoning, cross-zone authority primitives, and portal system.

pub mod aether_url;
pub mod authority;
pub mod config;
pub mod fence;
pub mod ghost;
pub mod handoff;
pub mod partition;
pub mod portal;
pub mod portal_renderer;
pub mod prefetch;
pub mod protocol;
pub mod runtime;
pub mod session_handoff;
pub mod split_merge;

pub use aether_url::{AetherUrl, AetherUrlError};
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
pub use portal::{ActivationMode, Portal, PortalShape, PortalState};
pub use portal_renderer::{PortalRenderState, PortalRenderer};
pub use prefetch::{AssetKind, PrefetchHint, PrefetchPriority, PrefetchQueue};
pub use protocol::{
    CrossZoneCombatDecision, CrossZonePhysicsDecision, HandoffDecision, HandoffFailureMode,
    HandoffResult, SequenceFence,
};
pub use runtime::{
    ZoningRuntime, ZoningRuntimeConfig, ZoningRuntimeInput, ZoningRuntimeOutput,
    ZoningRuntimeState,
};
pub use session_handoff::{
    PlayerStateSnapshot, SessionHandoffEnvelope, SessionHandoffError, SessionToken,
};
pub use split_merge::{
    AdjacentZonePair, SplitMergeConfig, SplitMergeDecision, SplitMergeManager,
};
