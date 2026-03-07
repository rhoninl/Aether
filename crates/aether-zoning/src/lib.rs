//! Spatial zoning and cross-zone authority primitives.

pub mod authority;
pub mod config;
pub mod ghost;
pub mod partition;
pub mod protocol;

pub use authority::{AuthorityZoneId, NetworkIdentity, SingleWriterMode};
pub use config::{
    AxisChoice, LoadMetrics, MergeThreshold, SplitPolicy, SplitResult, ZoneSplitPolicy, ZoneSpec,
};
pub use ghost::{GhostEntity, GhostPolicy, GhostVisibilityScope};
pub use partition::{
    EntitySample, KdAxis, KdBoundary, KdPoint, KdTree, KdTreeNode, KdTreeSplitResult, MAX_ZONE_DEPTH,
};
pub use protocol::{
    CrossZoneCombatDecision, CrossZonePhysicsDecision, HandoffDecision, HandoffFailureMode, HandoffResult, SequenceFence,
};

