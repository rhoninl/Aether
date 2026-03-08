//! Chunk lifecycle state machine and chunk entry tracking.

use serde::{Deserialize, Serialize};
use std::fmt;

use super::coord::{ChunkCoord, ChunkId};

/// The lifecycle state of a chunk in the streaming pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChunkState {
    /// Not in memory; needs to be loaded from storage.
    Unloaded,
    /// Load has been requested but not yet started (queued).
    Requested,
    /// I/O is in progress; data is being fetched.
    Loading,
    /// Data is in memory but not rendered/active.
    Cached,
    /// Data is in memory and actively rendered.
    Active,
    /// Being removed from memory (cleanup in progress).
    Evicting,
}

impl ChunkState {
    /// Check whether transitioning from `self` to `next` is a valid state machine transition.
    pub fn can_transition_to(&self, next: &ChunkState) -> bool {
        matches!(
            (self, next),
            (ChunkState::Unloaded, ChunkState::Requested)
                | (ChunkState::Requested, ChunkState::Loading)
                | (ChunkState::Loading, ChunkState::Cached)
                | (ChunkState::Loading, ChunkState::Unloaded)  // load_failed
                | (ChunkState::Cached, ChunkState::Active)
                | (ChunkState::Active, ChunkState::Cached)
                | (ChunkState::Cached, ChunkState::Evicting)
                | (ChunkState::Active, ChunkState::Evicting)   // force_evict
                | (ChunkState::Evicting, ChunkState::Unloaded)
        )
    }
}

impl fmt::Display for ChunkState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChunkState::Unloaded => write!(f, "Unloaded"),
            ChunkState::Requested => write!(f, "Requested"),
            ChunkState::Loading => write!(f, "Loading"),
            ChunkState::Cached => write!(f, "Cached"),
            ChunkState::Active => write!(f, "Active"),
            ChunkState::Evicting => write!(f, "Evicting"),
        }
    }
}

/// Error returned when a chunk state transition is invalid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidTransition {
    pub chunk_id: ChunkId,
    pub from: ChunkState,
    pub to: ChunkState,
}

impl fmt::Display for InvalidTransition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid chunk transition for {}: {} -> {}",
            self.chunk_id, self.from, self.to
        )
    }
}

/// A tracked chunk in the streaming pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkEntry {
    pub id: ChunkId,
    pub coord: ChunkCoord,
    pub state: ChunkState,
    /// Current LOD level (0 = highest detail).
    pub lod: u8,
    /// Size of chunk data in bytes (0 if not yet known).
    pub size_bytes: u64,
    /// Timestamp (ms) when this chunk was last accessed or activated.
    pub last_access_ms: u64,
    /// Timestamp (ms) when loading was requested.
    pub requested_at_ms: u64,
    /// Asset path or reference for loading.
    pub asset_path: String,
}

impl ChunkEntry {
    pub fn new(id: ChunkId, coord: ChunkCoord, asset_path: String) -> Self {
        Self {
            id,
            coord,
            state: ChunkState::Unloaded,
            lod: 0,
            size_bytes: 0,
            requested_at_ms: 0,
            last_access_ms: 0,
            asset_path,
        }
    }

    /// Attempt to transition to a new state. Returns an error if the transition is invalid.
    pub fn transition(&mut self, next: ChunkState, now_ms: u64) -> Result<ChunkState, InvalidTransition> {
        if self.state.can_transition_to(&next) {
            let prev = self.state;
            self.state = next;
            self.last_access_ms = now_ms;
            if next == ChunkState::Requested {
                self.requested_at_ms = now_ms;
            }
            Ok(prev)
        } else {
            Err(InvalidTransition {
                chunk_id: self.id,
                from: self.state,
                to: next,
            })
        }
    }

    /// Whether this chunk is occupying memory (Cached, Active, or Evicting).
    pub fn is_in_memory(&self) -> bool {
        matches!(
            self.state,
            ChunkState::Cached | ChunkState::Active | ChunkState::Evicting
        )
    }

    /// Whether this chunk has a pending load operation (Requested or Loading).
    pub fn is_inflight(&self) -> bool {
        matches!(self.state, ChunkState::Requested | ChunkState::Loading)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::coord::ChunkCoord;

    fn make_entry() -> ChunkEntry {
        ChunkEntry::new(
            ChunkId(1),
            ChunkCoord::new(0, 0, 0),
            "terrain/chunk_0_0_0.bin".to_string(),
        )
    }

    // --- ChunkState transition tests ---

    #[test]
    fn test_valid_transition_unloaded_to_requested() {
        assert!(ChunkState::Unloaded.can_transition_to(&ChunkState::Requested));
    }

    #[test]
    fn test_valid_transition_requested_to_loading() {
        assert!(ChunkState::Requested.can_transition_to(&ChunkState::Loading));
    }

    #[test]
    fn test_valid_transition_loading_to_cached() {
        assert!(ChunkState::Loading.can_transition_to(&ChunkState::Cached));
    }

    #[test]
    fn test_valid_transition_loading_to_unloaded_on_failure() {
        assert!(ChunkState::Loading.can_transition_to(&ChunkState::Unloaded));
    }

    #[test]
    fn test_valid_transition_cached_to_active() {
        assert!(ChunkState::Cached.can_transition_to(&ChunkState::Active));
    }

    #[test]
    fn test_valid_transition_active_to_cached() {
        assert!(ChunkState::Active.can_transition_to(&ChunkState::Cached));
    }

    #[test]
    fn test_valid_transition_cached_to_evicting() {
        assert!(ChunkState::Cached.can_transition_to(&ChunkState::Evicting));
    }

    #[test]
    fn test_valid_transition_active_to_evicting_force() {
        assert!(ChunkState::Active.can_transition_to(&ChunkState::Evicting));
    }

    #[test]
    fn test_valid_transition_evicting_to_unloaded() {
        assert!(ChunkState::Evicting.can_transition_to(&ChunkState::Unloaded));
    }

    #[test]
    fn test_invalid_transition_unloaded_to_active() {
        assert!(!ChunkState::Unloaded.can_transition_to(&ChunkState::Active));
    }

    #[test]
    fn test_invalid_transition_unloaded_to_loading() {
        assert!(!ChunkState::Unloaded.can_transition_to(&ChunkState::Loading));
    }

    #[test]
    fn test_invalid_transition_cached_to_requested() {
        assert!(!ChunkState::Cached.can_transition_to(&ChunkState::Requested));
    }

    #[test]
    fn test_invalid_transition_active_to_requested() {
        assert!(!ChunkState::Active.can_transition_to(&ChunkState::Requested));
    }

    #[test]
    fn test_invalid_transition_evicting_to_active() {
        assert!(!ChunkState::Evicting.can_transition_to(&ChunkState::Active));
    }

    #[test]
    fn test_invalid_transition_requested_to_active() {
        assert!(!ChunkState::Requested.can_transition_to(&ChunkState::Active));
    }

    #[test]
    fn test_invalid_transition_same_state() {
        assert!(!ChunkState::Active.can_transition_to(&ChunkState::Active));
        assert!(!ChunkState::Unloaded.can_transition_to(&ChunkState::Unloaded));
        assert!(!ChunkState::Cached.can_transition_to(&ChunkState::Cached));
    }

    // --- ChunkEntry tests ---

    #[test]
    fn test_new_entry_is_unloaded() {
        let entry = make_entry();
        assert_eq!(entry.state, ChunkState::Unloaded);
        assert_eq!(entry.lod, 0);
        assert_eq!(entry.size_bytes, 0);
    }

    #[test]
    fn test_entry_full_lifecycle() {
        let mut entry = make_entry();
        let now = 1000;

        // Unloaded -> Requested
        assert!(entry.transition(ChunkState::Requested, now).is_ok());
        assert_eq!(entry.state, ChunkState::Requested);
        assert_eq!(entry.requested_at_ms, now);

        // Requested -> Loading
        assert!(entry.transition(ChunkState::Loading, now + 10).is_ok());
        assert_eq!(entry.state, ChunkState::Loading);

        // Loading -> Cached
        assert!(entry.transition(ChunkState::Cached, now + 50).is_ok());
        assert_eq!(entry.state, ChunkState::Cached);

        // Cached -> Active
        assert!(entry.transition(ChunkState::Active, now + 60).is_ok());
        assert_eq!(entry.state, ChunkState::Active);

        // Active -> Cached (deactivate)
        assert!(entry.transition(ChunkState::Cached, now + 100).is_ok());
        assert_eq!(entry.state, ChunkState::Cached);

        // Cached -> Evicting
        assert!(entry.transition(ChunkState::Evicting, now + 110).is_ok());
        assert_eq!(entry.state, ChunkState::Evicting);

        // Evicting -> Unloaded
        assert!(entry.transition(ChunkState::Unloaded, now + 120).is_ok());
        assert_eq!(entry.state, ChunkState::Unloaded);
    }

    #[test]
    fn test_entry_load_failure_path() {
        let mut entry = make_entry();

        entry.transition(ChunkState::Requested, 100).unwrap();
        entry.transition(ChunkState::Loading, 110).unwrap();
        // Load failed -> back to Unloaded
        assert!(entry.transition(ChunkState::Unloaded, 120).is_ok());
        assert_eq!(entry.state, ChunkState::Unloaded);
    }

    #[test]
    fn test_entry_force_evict_from_active() {
        let mut entry = make_entry();

        entry.transition(ChunkState::Requested, 100).unwrap();
        entry.transition(ChunkState::Loading, 110).unwrap();
        entry.transition(ChunkState::Cached, 120).unwrap();
        entry.transition(ChunkState::Active, 130).unwrap();

        // Force evict directly from Active
        assert!(entry.transition(ChunkState::Evicting, 140).is_ok());
        assert_eq!(entry.state, ChunkState::Evicting);
    }

    #[test]
    fn test_entry_invalid_transition_returns_error() {
        let mut entry = make_entry();
        let result = entry.transition(ChunkState::Active, 100);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.from, ChunkState::Unloaded);
        assert_eq!(err.to, ChunkState::Active);
        assert_eq!(err.chunk_id, ChunkId(1));
    }

    #[test]
    fn test_entry_transition_updates_last_access() {
        let mut entry = make_entry();
        entry.transition(ChunkState::Requested, 500).unwrap();
        assert_eq!(entry.last_access_ms, 500);

        entry.transition(ChunkState::Loading, 600).unwrap();
        assert_eq!(entry.last_access_ms, 600);
    }

    #[test]
    fn test_entry_transition_returns_previous_state() {
        let mut entry = make_entry();
        let prev = entry.transition(ChunkState::Requested, 100).unwrap();
        assert_eq!(prev, ChunkState::Unloaded);

        let prev = entry.transition(ChunkState::Loading, 200).unwrap();
        assert_eq!(prev, ChunkState::Requested);
    }

    #[test]
    fn test_is_in_memory() {
        let mut entry = make_entry();
        assert!(!entry.is_in_memory());

        entry.state = ChunkState::Requested;
        assert!(!entry.is_in_memory());

        entry.state = ChunkState::Loading;
        assert!(!entry.is_in_memory());

        entry.state = ChunkState::Cached;
        assert!(entry.is_in_memory());

        entry.state = ChunkState::Active;
        assert!(entry.is_in_memory());

        entry.state = ChunkState::Evicting;
        assert!(entry.is_in_memory());
    }

    #[test]
    fn test_is_inflight() {
        let mut entry = make_entry();
        assert!(!entry.is_inflight());

        entry.state = ChunkState::Requested;
        assert!(entry.is_inflight());

        entry.state = ChunkState::Loading;
        assert!(entry.is_inflight());

        entry.state = ChunkState::Cached;
        assert!(!entry.is_inflight());

        entry.state = ChunkState::Active;
        assert!(!entry.is_inflight());
    }

    #[test]
    fn test_chunk_state_display() {
        assert_eq!(format!("{}", ChunkState::Unloaded), "Unloaded");
        assert_eq!(format!("{}", ChunkState::Requested), "Requested");
        assert_eq!(format!("{}", ChunkState::Loading), "Loading");
        assert_eq!(format!("{}", ChunkState::Cached), "Cached");
        assert_eq!(format!("{}", ChunkState::Active), "Active");
        assert_eq!(format!("{}", ChunkState::Evicting), "Evicting");
    }

    #[test]
    fn test_invalid_transition_display() {
        let err = InvalidTransition {
            chunk_id: ChunkId(42),
            from: ChunkState::Unloaded,
            to: ChunkState::Active,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("chunk:42"));
        assert!(msg.contains("Unloaded"));
        assert!(msg.contains("Active"));
    }
}
