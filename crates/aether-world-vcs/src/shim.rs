//! TEMPORARY SHIM — replace with `aether-schemas` (Bet 1 / U03) post-merge.
//!
//! Tracking: frogo task 74 (wiring retrofit). When the schemas crate
//! lands, the coordinator will:
//!   1. Remove `default-features = ["shim"]` from `Cargo.toml`.
//!   2. Add `aether-schemas = { workspace = true }`.
//!   3. Delete this file and swap `use crate::shim::X` for
//!      `use aether_schemas::X`.
//!
//! The public surface here intentionally mirrors the vocabulary we
//! expect from `aether-schemas`; types are opaque byte blobs so the
//! swap is mechanical.
#![cfg(feature = "shim")]

use serde::{Deserialize, Serialize};

/// Content-addressed identifier. A 32-byte SHA-256 digest.
///
/// Shared vocabulary across the engine; `aether-schemas` will own
/// this type once it lands.
pub type Cid = [u8; 32];

/// A world manifest — opaque to this crate.
///
/// `aether-world-vcs` only cares about CIDs, so we model the manifest
/// as a content-addressable wrapper around arbitrary bytes.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorldManifest {
    /// Opaque serialized manifest payload (CBOR, JSON, whatever).
    pub bytes: Vec<u8>,
}

/// An entity — opaque identifier + opaque payload.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entity {
    pub id: u64,
    pub payload: Vec<u8>,
}

/// A component — a named, typed value attached to an entity.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Component {
    pub name: String,
    pub value: Vec<u8>,
}

/// A world chunk — spatially-tiled segment of the world.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Chunk {
    pub id: u64,
    pub payload: Vec<u8>,
}

/// A script artifact — a WASM blob or source reference addressed by CID.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScriptArtifact {
    pub cid: Cid,
}
