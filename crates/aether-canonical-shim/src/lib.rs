//! TEMPORARY SHIM — replace with `aether-schemas` (Bet 1 unit U03) once merged.
//! Tracking issue: frogo task 74.
//!
//! This crate vendors the minimum surface of `aether-schemas` that Aether's
//! boundary crates (world-runtime, ugc, registry, creator-studio) need to
//! treat canonical artifacts as the only wire/disk format.
//!
//! Public API mirrors the expected `aether-schemas` API:
//!
//! * [`Cid`] / [`ContentAddress`] — content-addressed identifiers.
//! * [`CanonicalCodec`] — serialize/deserialize trait enforced at crate boundaries.
//! * [`SchemaError`] — unified error surface.
//! * Wire types: [`WorldManifest`], [`ChunkManifest`], [`PortalDef`],
//!   [`WorldDiscoveryRecord`], [`ArtifactEnvelope`].
//!
//! Content format: a deterministic byte encoding (keys sorted, lengths
//! length-prefixed) with SHA-256 computed over those bytes. The real
//! `aether-schemas` uses deterministic CBOR; this shim uses a simpler
//! deterministic encoder with the same invariants (stable bytes → stable CID).
//!
//! When `aether-schemas` lands, re-point each boundary crate's dependency to
//! it and delete this crate. The trait shape and `Cid::sha256_of` semantics
//! match, so call sites should migrate unchanged.

mod canonical;
mod content_address;
mod error;
mod wire;

pub use canonical::CanonicalCodec;
pub use content_address::{Cid, ContentAddress};
pub use error::SchemaError;
pub use wire::{
    ArtifactEnvelope, ArtifactKind, ChunkManifest, ChunkRef, PortalDef, PortalScheme,
    SpawnPointDef, WorldDiscoveryRecord, WorldManifest, WorldStatus,
};
