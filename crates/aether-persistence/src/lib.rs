//! Persistence primitives for Aether worlds.
//!
//! This crate models both ephemeral checkpointing and durable WAL-backed state in a single
//! policy-driven API that higher-level runtime services can use without binding to a concrete DB.
//!
//! ## Backend modules
//!
//! The crate also provides trait-based abstractions for database, cache, and event bus backends:
//!
//! - [`postgres`] - PostgreSQL via `sqlx` (feature: `postgres`)
//! - [`redis_client`] - Redis cache (feature: `redis-backend`)
//! - [`nats`] - NATS JetStream event bus (feature: `nats`)
//! - [`pool`] - Connection configuration from environment variables
//! - [`migration`] - Database migration framework
//! - [`health`] - Health check utilities

pub mod config;
pub mod error;
pub mod health;
pub mod migration;
pub mod nats;
pub mod placement;
pub mod pool;
pub mod postgres;
pub mod redis_client;
pub mod runtime;
pub mod snapshot;
pub mod transactions;
pub mod wal;

pub use config::{
    WorldPersistenceClass, WorldPersistenceProfile, DEFAULT_EPHEMERAL_SNAPSHOT_INTERVAL,
};
pub use error::PersistenceError;
pub use health::{BackendStatus, HealthReport};
pub use migration::Migration;
pub use nats::{EventBus, EventMessage, EventSubscription};
pub use placement::{PodPlacementHint, PodRuntimeClass, PodTopologyHint, WorldManifest};
pub use pool::ConnectionConfig;
pub use postgres::DatabaseClient;
pub use redis_client::CacheClient;
pub use runtime::{
    PersistenceRuntime, PersistenceRuntimeConfig, PersistenceRuntimeOutput,
    PersistenceRuntimeState, RuntimeTickInput, WorldRecovery, WorldRuntimeInput,
};
pub use snapshot::{Snapshot, SnapshotKind, SnapshotPolicy, SnapshotRecorder, SnapshotWindow};
pub use transactions::{
    CriticalStateError, CriticalStateKey, CriticalStateMutationRecord, CriticalStatePriority,
    CriticalWriteResult, SyncStateMutation,
};
pub use wal::{
    WalAppendError, WalAppendResult, WalDurability, WalEntry, WalReplayRecord, WalSegment,
    WalWriteCoordinator,
};
