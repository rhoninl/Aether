//! World runtime contracts for lifecycle, streaming, and world resource boot.
//!
//! Also provides multiplayer primitives: tick scheduling, input buffering,
//! entity prediction/interpolation, state synchronization, RPC, sessions,
//! and event distribution.

pub mod chunking;
pub mod events;
pub mod input_buffer;
pub mod lifecycle;
pub mod manifest;
pub mod prediction;
pub mod props;
pub mod rpc;
pub mod runtime;
pub mod session;
pub mod spawn;
pub mod state_sync;
pub mod tick;

pub use chunking::{ChunkDescriptor, ChunkKind, ChunkStreamingPolicy};
pub use events::{EventDispatcher, EventScope, GameEvent, EventDelivery, EntityPosition};
pub use input_buffer::{InputAction, InputBuffer, InputBufferError, PlayerId, PlayerInput};
pub use lifecycle::{LifecycleEvent, RuntimeState};
pub use manifest::{WorldManifestError, WorldRuntimeManifest};
pub use prediction::{
    EntityState, InterpolationBuffer, CorrectionDelta, compute_correction, lerp_entity_state,
};
pub use props::{LightingSetup, PropInstance, SpawnPoint, TerrainChunk, TileLayer};
pub use rpc::{RpcDirection, RpcDispatcher, RpcError, RpcRequest, RpcResponse};
pub use runtime::{
    PerformanceSample, WorldRuntime, WorldRuntimeCommand, WorldRuntimeConfig, WorldRuntimeInput,
    WorldRuntimeOutput, WorldRuntimeState,
};
pub use session::{PlayerSession, SessionError, SessionEvent, SessionManager, SessionState};
pub use spawn::{RuntimeSettings, RuntimeSettingsError};
pub use spawn::{WorldBootError, WorldLifecycle, WorldLifecycleEvent};
pub use state_sync::{
    FullStateSnapshot, StateSyncManager, StateSyncMessage, SyncChannel,
};
pub use tick::{ServerTick, TickScheduler};
