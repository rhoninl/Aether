//! Error types for the world VCS.

use thiserror::Error;

/// Result type alias using [`VcsError`].
pub type Result<T> = std::result::Result<T, VcsError>;

/// Errors raised by the world VCS.
#[derive(Debug, Error)]
pub enum VcsError {
    /// Encoding a diff to canonical CBOR failed.
    #[error("canonical CBOR encoding failed: {0}")]
    Encode(String),

    /// Decoding a diff from CBOR failed.
    #[error("CBOR decoding failed: {0}")]
    Decode(String),

    /// JSON serialization failed.
    #[error("JSON serialization failed: {0}")]
    Json(#[from] serde_json::Error),

    /// Signature verification failed.
    #[error("signature verification failed: {0}")]
    BadSignature(String),

    /// Signing failed.
    #[error("signing failed: {0}")]
    Signing(String),

    /// Referenced branch does not exist.
    #[error("unknown branch: {0}")]
    UnknownBranch(String),

    /// Branch already exists.
    #[error("branch already exists: {0}")]
    BranchExists(String),

    /// Referenced diff CID was not found in the store.
    #[error("unknown diff CID: {0}")]
    UnknownDiff(String),

    /// Review referenced an unknown reviewer.
    #[error("unknown reviewer for review {0}")]
    UnknownReviewer(String),

    /// Merge conflict — caller should inspect the accompanying
    /// [`crate::merge::ConflictReport`] produced by `merge`.
    #[error("merge has {0} conflicting op pair(s)")]
    MergeConflict(usize),

    /// Attempted to invert a `ModifyComponent` without a prior value.
    #[error("cannot invert ModifyComponent without prior_value")]
    InverseMissingPriorValue,

    /// Rollback target is not in the branch's ancestry.
    #[error("rollback target is not an ancestor of branch {branch}")]
    RollbackOutOfLineage {
        /// The branch that was attempted to roll back.
        branch: String,
    },
}
