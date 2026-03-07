//! World runtime contracts for lifecycle, streaming, and world resource boot.

pub mod chunking;
pub mod lifecycle;
pub mod manifest;
pub mod props;
pub mod spawn;

pub use chunking::{ChunkDescriptor, ChunkKind, ChunkStreamingPolicy};
pub use lifecycle::{LifecycleEvent, RuntimeState};
pub use manifest::{WorldManifestError, WorldRuntimeManifest};
pub use props::{LightingSetup, PropInstance, SpawnPoint, TerrainChunk, TileLayer};
pub use spawn::{RuntimeSettings, RuntimeSettingsError};
pub use spawn::{WorldBootError, WorldLifecycle, WorldLifecycleEvent};
