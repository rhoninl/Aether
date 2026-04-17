//! World registry primitives and matchmaking policy types.

pub mod canonical;
pub mod discovery;
pub mod manifest;
pub mod portal;
pub mod session;

pub use canonical::{manifest_to_canonical_bytes, CanonicalWorldIndex};
pub use discovery::{DiscoveryFilter, DiscoveryResult, DiscoverySort, MatchCriteria};
pub use manifest::{
    WorldCategory, WorldManifest, WorldManifestError, WorldStatus, validate_manifest,
};
pub use portal::{PortalRoute, PortalResolver, PortalScheme};
pub use session::{MatchOutcome, RegionPolicy, ServerInstance, SessionManagerPolicy, SessionState};

