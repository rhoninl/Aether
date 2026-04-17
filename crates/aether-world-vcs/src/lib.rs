//! Aether World-as-Git: signed, content-addressed diffs with
//! branches, merges, rollbacks, federation remotes and a review
//! surface.
//!
//! See `docs/design/world-vcs-implementation.md` in the repo root for
//! the full design doc. Module-level docs cover the API contract.
//!
//! # Core flow
//!
//! ```ignore
//! use aether_world_vcs::{
//!     diff::{AgentRef, Diff, Op, cid_of},
//!     sig::{generate_keypair, sign_diff, verify_signed_diff},
//!     branch::{MemoryBranchStore, BranchStore, DEFAULT_BRANCH},
//!     review::{MemoryReviewStore, ReviewStore, ReviewerRef, ReviewStatus, MergePolicy},
//! };
//! ```
//!
//! # Feature flags
//!
//! - `shim` (default): enables a vendored shim for schema types
//!   normally supplied by `aether-schemas` (U03). Once that crate
//!   lands the coordinator will flip this off.
#![deny(missing_docs)]

pub mod branch;
pub mod diff;
pub mod error;
pub mod merge;
pub mod remote;
pub mod review;
pub mod rollback;
pub mod sig;
#[cfg(feature = "shim")]
pub mod shim;

pub use branch::{Branch, BranchStore, MemoryBranchStore, DEFAULT_BRANCH};
pub use diff::{
    canonical_cbor, cid_of, cid_to_hex, decode_cbor, AgentRef, Cid, Diff, Op, SignedDiff,
};
pub use error::{Result, VcsError};
pub use merge::{merge, Conflict, ConflictReport, MergeOutcome};
pub use remote::{FetchResult, NullTransport, PushResult, Remote, RemoteTransport};
pub use review::{
    Comment, MemoryReviewStore, MergeDecision, MergePolicy, Review, ReviewStatus, ReviewStore,
    Reviewer, ReviewerRef,
};
pub use rollback::{revert, rollback};
pub use sig::{generate_keypair, sign_diff, verify_signed_diff};
