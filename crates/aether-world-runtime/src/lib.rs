//! World runtime contracts for lifecycle, streaming, and world resource boot.

pub mod canonical;
pub mod chunking;
pub mod lifecycle;
pub mod manifest;
pub mod props;
pub mod spawn;
pub mod runtime;

pub use canonical::{
    decode_chunk_manifest, decode_world_runtime_manifest, encode_chunk_manifest,
    encode_world_runtime_manifest, portal_def, world_runtime_manifest_cid, BoundaryCodec,
    CanonicalPortalDef, CanonicalPortalScheme,
};
pub use chunking::{ChunkDescriptor, ChunkKind, ChunkStreamingPolicy};
pub use lifecycle::{LifecycleEvent, RuntimeState};
pub use manifest::{WorldManifestError, WorldRuntimeManifest};
pub use props::{LightingSetup, PropInstance, SpawnPoint, TerrainChunk, TileLayer};
pub use spawn::{RuntimeSettings, RuntimeSettingsError};
pub use spawn::{WorldBootError, WorldLifecycle, WorldLifecycleEvent};
pub use runtime::{
    PerformanceSample, WorldRuntime, WorldRuntimeCommand, WorldRuntimeInput, WorldRuntimeOutput, WorldRuntimeState,
    WorldRuntimeConfig,
};
