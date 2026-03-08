//! Chunked world streaming pipeline.
//!
//! Provides spatial chunk management with lifecycle state machine,
//! progressive LOD loading, asset prefetch, occlusion portal gating,
//! and LRU-based eviction with distance weighting.

pub mod coord;
pub mod eviction;
pub mod manifest;
pub mod state;
pub mod streaming;

pub use coord::{ChunkCoord, ChunkId, DEFAULT_CHUNK_SIZE};
pub use eviction::{EvictionCandidate, EvictionPolicy, DEFAULT_MAX_CACHED_CHUNKS};
pub use manifest::{
    BoundaryStitch, ChunkManifest, ChunkManifestError, ChunkReference, PortalDefinition,
    PortalFace,
};
pub use state::{ChunkEntry, ChunkState, InvalidTransition};
pub use streaming::{
    PlayerView, StreamingConfig, StreamingEngine, StreamingEvent, DEFAULT_ACTIVE_RADIUS,
    DEFAULT_CACHE_RADIUS, DEFAULT_LOD_DISTANCES, DEFAULT_MAX_INFLIGHT_REQUESTS,
    DEFAULT_PREFETCH_TIME_SECS,
};
