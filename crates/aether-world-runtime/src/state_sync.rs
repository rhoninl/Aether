//! Entity state broadcast and delta synchronization.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::input_buffer::PlayerId;
use crate::prediction::EntityState;

/// Channel type for state sync messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncChannel {
    /// Guaranteed delivery, ordered. Used for critical state changes.
    Reliable,
    /// Best-effort, latest-wins. Used for frequently updated state.
    Unreliable,
}

/// A state update message destined for a client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSyncMessage {
    pub target_player: PlayerId,
    pub channel: SyncChannel,
    pub entities: Vec<EntityState>,
    pub server_tick: u64,
}

/// A full state snapshot for a newly connected client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullStateSnapshot {
    pub target_player: PlayerId,
    pub entities: Vec<EntityState>,
    pub server_tick: u64,
}

/// Manages entity state tracking and delta generation for sync.
#[derive(Debug)]
pub struct StateSyncManager {
    /// Current authoritative state of all entities.
    entities: HashMap<u64, EntityState>,
    /// Per-client, per-entity: the last tick at which we sent this entity's state.
    client_acks: HashMap<PlayerId, HashMap<u64, u64>>,
    /// Entities whose state changed this tick (dirty set).
    dirty_entities: Vec<u64>,
}

impl StateSyncManager {
    pub fn new() -> Self {
        Self {
            entities: HashMap::new(),
            client_acks: HashMap::new(),
            dirty_entities: Vec::new(),
        }
    }

    /// Update or insert an entity's authoritative state.
    pub fn update_entity(&mut self, state: EntityState) {
        let entity_id = state.entity_id;
        self.entities.insert(entity_id, state);
        if !self.dirty_entities.contains(&entity_id) {
            self.dirty_entities.push(entity_id);
        }
    }

    /// Remove an entity from tracking.
    pub fn remove_entity(&mut self, entity_id: u64) {
        self.entities.remove(&entity_id);
        for acks in self.client_acks.values_mut() {
            acks.remove(&entity_id);
        }
    }

    /// Register a client for state sync.
    pub fn add_client(&mut self, player_id: PlayerId) {
        self.client_acks.entry(player_id).or_default();
    }

    /// Remove a client from tracking.
    pub fn remove_client(&mut self, player_id: &PlayerId) {
        self.client_acks.remove(player_id);
    }

    /// Generate a full state snapshot for a client (e.g., on initial join).
    pub fn full_snapshot(&self, player_id: PlayerId, server_tick: u64) -> FullStateSnapshot {
        let entities: Vec<EntityState> = self.entities.values().cloned().collect();
        FullStateSnapshot {
            target_player: player_id,
            entities,
            server_tick,
        }
    }

    /// Generate delta sync messages for all registered clients.
    /// Only includes entities that changed since the client's last ack.
    /// After calling this, the dirty set is cleared and acks are updated.
    pub fn generate_sync_messages(&mut self, server_tick: u64) -> Vec<StateSyncMessage> {
        let mut messages = Vec::new();
        let client_ids: Vec<PlayerId> = self.client_acks.keys().copied().collect();

        for player_id in client_ids {
            let mut reliable_entities = Vec::new();
            let mut unreliable_entities = Vec::new();

            for &entity_id in &self.dirty_entities {
                if let Some(state) = self.entities.get(&entity_id) {
                    let acks = self.client_acks.entry(player_id).or_default();
                    let last_acked_tick = acks.get(&entity_id).copied().unwrap_or(0);

                    if state.tick > last_acked_tick {
                        // Classify channel: position changes are unreliable (frequent),
                        // but new entities or large corrections are reliable
                        if last_acked_tick == 0 {
                            reliable_entities.push(state.clone());
                        } else {
                            unreliable_entities.push(state.clone());
                        }

                        acks.insert(entity_id, server_tick);
                    }
                }
            }

            if !reliable_entities.is_empty() {
                messages.push(StateSyncMessage {
                    target_player: player_id,
                    channel: SyncChannel::Reliable,
                    entities: reliable_entities,
                    server_tick,
                });
            }

            if !unreliable_entities.is_empty() {
                messages.push(StateSyncMessage {
                    target_player: player_id,
                    channel: SyncChannel::Unreliable,
                    entities: unreliable_entities,
                    server_tick,
                });
            }
        }

        self.dirty_entities.clear();
        messages
    }

    /// Acknowledge that a client received state up to a given tick.
    pub fn acknowledge(&mut self, player_id: &PlayerId, entity_id: u64, tick: u64) {
        if let Some(acks) = self.client_acks.get_mut(player_id) {
            let current = acks.get(&entity_id).copied().unwrap_or(0);
            if tick > current {
                acks.insert(entity_id, tick);
            }
        }
    }

    /// Get the current authoritative state of an entity.
    pub fn get_entity(&self, entity_id: u64) -> Option<&EntityState> {
        self.entities.get(&entity_id)
    }

    /// Get the number of tracked entities.
    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }

    /// Get the number of registered clients.
    pub fn client_count(&self) -> usize {
        self.client_acks.len()
    }

    /// Get the number of dirty (changed) entities pending sync.
    pub fn dirty_count(&self) -> usize {
        self.dirty_entities.len()
    }
}

impl Default for StateSyncManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn make_entity(id: u64, tick: u64, x: f32) -> EntityState {
        EntityState {
            entity_id: id,
            position: [x, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            velocity: [0.0, 0.0, 0.0],
            tick,
        }
    }

    #[test]
    fn test_update_and_get_entity() {
        let mut mgr = StateSyncManager::new();
        let state = make_entity(1, 1, 5.0);
        mgr.update_entity(state);

        let retrieved = mgr.get_entity(1).unwrap();
        assert_eq!(retrieved.position[0], 5.0);
        assert_eq!(mgr.entity_count(), 1);
    }

    #[test]
    fn test_remove_entity() {
        let mut mgr = StateSyncManager::new();
        mgr.update_entity(make_entity(1, 1, 0.0));
        mgr.update_entity(make_entity(2, 1, 0.0));
        assert_eq!(mgr.entity_count(), 2);

        mgr.remove_entity(1);
        assert_eq!(mgr.entity_count(), 1);
        assert!(mgr.get_entity(1).is_none());
        assert!(mgr.get_entity(2).is_some());
    }

    #[test]
    fn test_add_and_remove_client() {
        let mut mgr = StateSyncManager::new();
        let p1 = Uuid::new_v4();
        let p2 = Uuid::new_v4();

        mgr.add_client(p1);
        mgr.add_client(p2);
        assert_eq!(mgr.client_count(), 2);

        mgr.remove_client(&p1);
        assert_eq!(mgr.client_count(), 1);
    }

    #[test]
    fn test_full_snapshot() {
        let mut mgr = StateSyncManager::new();
        let pid = Uuid::new_v4();

        mgr.update_entity(make_entity(1, 1, 1.0));
        mgr.update_entity(make_entity(2, 1, 2.0));

        let snapshot = mgr.full_snapshot(pid, 1);
        assert_eq!(snapshot.target_player, pid);
        assert_eq!(snapshot.entities.len(), 2);
        assert_eq!(snapshot.server_tick, 1);
    }

    #[test]
    fn test_delta_sync_new_entities_are_reliable() {
        let mut mgr = StateSyncManager::new();
        let pid = Uuid::new_v4();
        mgr.add_client(pid);

        mgr.update_entity(make_entity(1, 1, 1.0));
        let messages = mgr.generate_sync_messages(1);

        // New entity should be sent reliably
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].channel, SyncChannel::Reliable);
        assert_eq!(messages[0].entities.len(), 1);
    }

    #[test]
    fn test_delta_sync_subsequent_updates_are_unreliable() {
        let mut mgr = StateSyncManager::new();
        let pid = Uuid::new_v4();
        mgr.add_client(pid);

        // First update - reliable (new entity)
        mgr.update_entity(make_entity(1, 1, 1.0));
        let _ = mgr.generate_sync_messages(1);

        // Second update - unreliable (entity already known)
        mgr.update_entity(make_entity(1, 2, 2.0));
        let messages = mgr.generate_sync_messages(2);

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].channel, SyncChannel::Unreliable);
    }

    #[test]
    fn test_dirty_set_cleared_after_sync() {
        let mut mgr = StateSyncManager::new();
        let pid = Uuid::new_v4();
        mgr.add_client(pid);

        mgr.update_entity(make_entity(1, 1, 1.0));
        assert_eq!(mgr.dirty_count(), 1);

        let _ = mgr.generate_sync_messages(1);
        assert_eq!(mgr.dirty_count(), 0);
    }

    #[test]
    fn test_no_messages_when_no_dirty_entities() {
        let mut mgr = StateSyncManager::new();
        let pid = Uuid::new_v4();
        mgr.add_client(pid);

        mgr.update_entity(make_entity(1, 1, 1.0));
        let _ = mgr.generate_sync_messages(1);

        // No updates -> no messages
        let messages = mgr.generate_sync_messages(2);
        assert!(messages.is_empty());
    }

    #[test]
    fn test_multi_client_sync() {
        let mut mgr = StateSyncManager::new();
        let p1 = Uuid::new_v4();
        let p2 = Uuid::new_v4();
        mgr.add_client(p1);
        mgr.add_client(p2);

        mgr.update_entity(make_entity(1, 1, 1.0));
        let messages = mgr.generate_sync_messages(1);

        // Both clients should receive the entity
        assert_eq!(messages.len(), 2);
        let targets: Vec<PlayerId> = messages.iter().map(|m| m.target_player).collect();
        assert!(targets.contains(&p1));
        assert!(targets.contains(&p2));
    }

    #[test]
    fn test_acknowledge_prevents_resend() {
        let mut mgr = StateSyncManager::new();
        let pid = Uuid::new_v4();
        mgr.add_client(pid);

        mgr.update_entity(make_entity(1, 1, 1.0));
        let _ = mgr.generate_sync_messages(1);

        // Ack the entity at tick 5
        mgr.acknowledge(&pid, 1, 5);

        // Update at tick 3 (before ack) should not produce a message
        mgr.update_entity(make_entity(1, 3, 3.0));
        let messages = mgr.generate_sync_messages(3);
        assert!(messages.is_empty());

        // Update at tick 6 (after ack) should produce a message
        mgr.update_entity(make_entity(1, 6, 6.0));
        let messages = mgr.generate_sync_messages(6);
        assert_eq!(messages.len(), 1);
    }

    #[test]
    fn test_remove_entity_cleans_up_acks() {
        let mut mgr = StateSyncManager::new();
        let pid = Uuid::new_v4();
        mgr.add_client(pid);

        mgr.update_entity(make_entity(1, 1, 1.0));
        let _ = mgr.generate_sync_messages(1);

        mgr.remove_entity(1);

        // Entity should be gone from acks as well
        // Re-adding it should be treated as a new entity (reliable)
        mgr.update_entity(make_entity(1, 2, 2.0));
        let messages = mgr.generate_sync_messages(2);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].channel, SyncChannel::Reliable);
    }

    #[test]
    fn test_multiple_entity_updates_same_tick() {
        let mut mgr = StateSyncManager::new();
        let pid = Uuid::new_v4();
        mgr.add_client(pid);

        mgr.update_entity(make_entity(1, 1, 1.0));
        mgr.update_entity(make_entity(2, 1, 2.0));
        mgr.update_entity(make_entity(3, 1, 3.0));

        let messages = mgr.generate_sync_messages(1);
        // All new entities should be reliable, in one message
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].channel, SyncChannel::Reliable);
        assert_eq!(messages[0].entities.len(), 3);
    }
}
