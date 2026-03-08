//! World registry primitives, discovery, ranking, analytics, and matchmaking.

pub mod analytics;
pub mod discovery;
pub mod manifest;
pub mod portal;
pub mod ranking;
pub mod registry;
pub mod search;
pub mod session;

pub use analytics::{AnalyticsEvent, AnalyticsTracker, WorldAnalytics};
pub use discovery::{DiscoveryFilter, DiscoveryResult, DiscoverySort, MatchCriteria};
pub use manifest::{
    WorldCategory, WorldManifest, WorldManifestError, WorldStatus, validate_manifest,
};
pub use portal::{PortalError, PortalResolver, PortalRoute, PortalScheme, PortalUrl};
pub use ranking::{RankingEngine, WorldScore};
pub use registry::{RegistryError, WorldEntry, WorldRegistry, WorldUpdate};
pub use search::{SearchQuery, SearchResult, SortField};
pub use session::{
    MatchOutcome, RegionPolicy, ServerInstance, SessionManager, SessionManagerPolicy, SessionState,
};
