//! Canonical, content-addressed world diff format.
//!
//! A [`Diff`] is a typed sequence of [`Op`]s going from a `base` CID
//! to a `target` CID. Every diff is itself content-addressed: its CID
//! is the SHA-256 of its canonical CBOR encoding (see
//! [`canonical_cbor`]). Agents can therefore reference, compose, and
//! archive diffs by CID the same way they reference worlds.
//!
//! ## Byte ordering / wire format
//!
//! 1. The in-memory [`Diff`] is serialized to CBOR via `ciborium`.
//! 2. Map keys (from struct field names) are emitted in declaration
//!    order; we keep declaration order alphabetical so the output is
//!    canonical.
//! 3. Any embedded `BTreeMap<String, ...>` is ordered by key; `Vec`s
//!    keep caller order (callers must put ops in a stable order).
//! 4. The resulting bytes are hashed with SHA-256; the 32-byte digest
//!    is the diff's [`Cid`].
//!
//! Determinism is verified by the `diff_roundtrip::cid_is_stable` test.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{Result, VcsError};

#[cfg(feature = "shim")]
pub use crate::shim::Cid;

/// Reference to an agent or human who authored a diff.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AgentRef {
    /// An agent identified by a service-account string.
    Agent {
        /// Opaque service-account identifier.
        service_account: String,
    },
    /// A human identified by an opaque user id.
    Human {
        /// Opaque user identifier.
        user_id: String,
    },
}

/// A typed mutation within a diff.
///
/// Granularity is intentionally coarse (whole component / whole
/// entity / whole chunk) so that conflict detection has a meaningful
/// "subject key" (see [`crate::merge`]). Finer-grained, byte-level
/// delta replication already lives in
/// `crates/aether-network/src/delta.rs`; this format is complementary.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    /// Add a brand-new entity. Errors at apply time if the entity id
    /// already exists.
    AddEntity {
        /// Entity id.
        entity: u64,
        /// Opaque entity payload (component blob, manifest, etc.).
        payload: Vec<u8>,
    },
    /// Remove an existing entity.
    RemoveEntity {
        /// Entity id.
        entity: u64,
        /// Prior payload snapshot — required so the op is reversible.
        /// Ignored on apply; used by [`crate::rollback::revert`].
        #[serde(default, skip_serializing_if = "Option::is_none")]
        prior_payload: Option<Vec<u8>>,
    },
    /// Replace an existing entity's payload wholesale.
    ReplaceEntity {
        /// Entity id.
        entity: u64,
        /// New payload.
        payload: Vec<u8>,
        /// Prior payload snapshot for reversibility.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        prior_payload: Option<Vec<u8>>,
    },
    /// Modify a single named component on an entity.
    ModifyComponent {
        /// Entity id.
        entity: u64,
        /// Component name.
        component_name: String,
        /// New value bytes.
        value: Vec<u8>,
        /// Prior value for reversibility.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        prior_value: Option<Vec<u8>>,
    },
    /// Swap the script attached to an entity.
    RetargetScript {
        /// Entity id.
        entity: u64,
        /// Previous script CID — used both by reversal and by
        /// optimistic concurrency checks at apply time.
        old_script_cid: Cid,
        /// New script CID.
        new_script_cid: Cid,
    },
    /// Add a new world chunk.
    AddChunk {
        /// Chunk id.
        chunk: u64,
        /// Chunk bytes.
        payload: Vec<u8>,
    },
    /// Remove an existing chunk.
    RemoveChunk {
        /// Chunk id.
        chunk: u64,
        /// Prior payload for reversibility.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        prior_payload: Option<Vec<u8>>,
    },
    /// Replace an existing chunk's payload wholesale.
    ReplaceChunk {
        /// Chunk id.
        chunk: u64,
        /// New payload.
        payload: Vec<u8>,
        /// Prior payload for reversibility.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        prior_payload: Option<Vec<u8>>,
    },
}

impl Op {
    /// The *subject key* used by the merge algorithm to detect
    /// conflicts. Two ops conflict if they share a subject key and
    /// are not byte-identical.
    pub fn subject_key(&self) -> String {
        match self {
            Op::AddEntity { entity, .. }
            | Op::RemoveEntity { entity, .. }
            | Op::ReplaceEntity { entity, .. } => format!("entity:{entity}"),
            Op::ModifyComponent {
                entity,
                component_name,
                ..
            } => format!("component:{entity}:{component_name}"),
            Op::RetargetScript { entity, .. } => format!("script:{entity}"),
            Op::AddChunk { chunk, .. }
            | Op::RemoveChunk { chunk, .. }
            | Op::ReplaceChunk { chunk, .. } => format!("chunk:{chunk}"),
        }
    }
}

/// A world diff going from `base` to `target`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Diff {
    /// Ancestor world CID.
    pub base: Cid,
    /// Resulting world CID.
    pub target: Cid,
    /// Typed ops, applied in order.
    pub ops: Vec<Op>,
    /// Author of this diff.
    pub author: AgentRef,
    /// Wall-clock authoring time in unix milliseconds.
    pub timestamp_unix_ms: u64,
}

/// A diff plus its signature and the public key that signed it.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignedDiff {
    /// The diff itself.
    pub diff: Diff,
    /// Ed25519 signature over `canonical_cbor(&diff)`.
    pub signature: Vec<u8>,
    /// Public key that produced the signature (raw 32-byte ed25519).
    pub public_key: Vec<u8>,
}

/// Encode a diff to its canonical CBOR byte representation.
///
/// This is the exact byte sequence hashed to produce the diff's CID
/// and signed by its author.
pub fn canonical_cbor(diff: &Diff) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    ciborium::ser::into_writer(diff, &mut out).map_err(|e| VcsError::Encode(e.to_string()))?;
    Ok(out)
}

/// Decode a diff from its canonical CBOR byte representation.
pub fn decode_cbor(bytes: &[u8]) -> Result<Diff> {
    ciborium::de::from_reader(bytes).map_err(|e| VcsError::Decode(e.to_string()))
}

/// Compute the content-address (CID) of a diff: SHA-256 of its
/// canonical CBOR encoding.
pub fn cid_of(diff: &Diff) -> Result<Cid> {
    let bytes = canonical_cbor(diff)?;
    let digest = Sha256::digest(&bytes);
    let mut cid = [0u8; 32];
    cid.copy_from_slice(&digest);
    Ok(cid)
}

/// Format a CID as a lowercase hex string (64 chars).
pub fn cid_to_hex(cid: &Cid) -> String {
    let mut s = String::with_capacity(64);
    for b in cid.iter() {
        use std::fmt::Write;
        let _ = write!(s, "{:02x}", b);
    }
    s
}
