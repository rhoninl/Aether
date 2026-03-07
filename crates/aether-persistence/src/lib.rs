//! Persistence primitives for Aether worlds.
//!
//! This crate models both ephemeral checkpointing and durable WAL-backed state in a single
//! policy-driven API that higher-level runtime services can use without binding to a concrete DB.

pub mod config;
pub mod placement;
pub mod snapshot;
pub mod transactions;
pub mod wal;

pub use config::{WorldPersistenceClass, WorldPersistenceProfile, DEFAULT_EPHEMERAL_SNAPSHOT_INTERVAL};
pub use placement::{PodPlacementHint, PodRuntimeClass, PodTopologyHint, WorldManifest};
pub use snapshot::{Snapshot, SnapshotKind, SnapshotPolicy, SnapshotRecorder, SnapshotWindow};
pub use transactions::{
    CriticalStateError, CriticalStateKey, CriticalStateMutation, CriticalStatePriority, CriticalWriteResult,
    SyncStateMutation,
};
pub use wal::{
    WalAppendError, WalAppendResult, WalDurability, WalEntry, WalReplayRecord, WalSegment, WalWriteCoordinator,
};

