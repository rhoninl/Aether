//! Game event distribution with interest-based filtering.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::input_buffer::PlayerId;

/// The scope of a game event, determining which clients receive it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EventScope {
    /// Broadcast to all connected clients.
    Global,
    /// Broadcast to clients near a specific entity.
    NearEntity { entity_id: u64, radius: f32 },
    /// Send to a specific player only.
    Player { player_id: PlayerId },
    /// Send to a group of players.
    Group { player_ids: Vec<PlayerId> },
}

/// A game event to be distributed to clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameEvent {
    pub event_id: u64,
    pub event_type: String,
    pub scope: EventScope,
    pub payload: Vec<u8>,
    pub tick: u64,
}

/// An event delivery targeted at a specific player.
#[derive(Debug, Clone)]
pub struct EventDelivery {
    pub target_player: PlayerId,
    pub event: GameEvent,
}

/// Position information for proximity-based event filtering.
#[derive(Debug, Clone)]
pub struct EntityPosition {
    pub entity_id: u64,
    pub position: [f32; 3],
    pub owner_player_id: Option<PlayerId>,
}

/// Manages game event queuing and distribution.
#[derive(Debug)]
pub struct EventDispatcher {
    pending_events: Vec<GameEvent>,
    next_event_id: u64,
}

impl EventDispatcher {
    pub fn new() -> Self {
        Self {
            pending_events: Vec::new(),
            next_event_id: 1,
        }
    }

    /// Queue a game event for distribution.
    pub fn emit(&mut self, event_type: &str, scope: EventScope, payload: Vec<u8>, tick: u64) -> u64 {
        let event_id = self.next_event_id;
        self.next_event_id += 1;

        self.pending_events.push(GameEvent {
            event_id,
            event_type: event_type.to_string(),
            scope,
            payload,
            tick,
        });

        event_id
    }

    /// Distribute all pending events to their intended recipients.
    ///
    /// `connected_players` is the list of currently active player IDs.
    /// `entity_positions` maps entity IDs to their positions and owner players,
    /// used for proximity-based event filtering.
    /// `player_positions` maps player IDs to their current position.
    pub fn distribute(
        &mut self,
        connected_players: &[PlayerId],
        entity_positions: &[EntityPosition],
        player_positions: &HashMap<PlayerId, [f32; 3]>,
    ) -> Vec<EventDelivery> {
        let mut deliveries = Vec::new();
        let events: Vec<GameEvent> = self.pending_events.drain(..).collect();

        for event in events {
            match &event.scope {
                EventScope::Global => {
                    for &player_id in connected_players {
                        deliveries.push(EventDelivery {
                            target_player: player_id,
                            event: event.clone(),
                        });
                    }
                }
                EventScope::Player { player_id } => {
                    if connected_players.contains(player_id) {
                        deliveries.push(EventDelivery {
                            target_player: *player_id,
                            event: event.clone(),
                        });
                    }
                }
                EventScope::Group { player_ids } => {
                    for pid in player_ids {
                        if connected_players.contains(pid) {
                            deliveries.push(EventDelivery {
                                target_player: *pid,
                                event: event.clone(),
                            });
                        }
                    }
                }
                EventScope::NearEntity { entity_id, radius } => {
                    // Find the entity's position
                    if let Some(entity_pos) = entity_positions.iter().find(|e| e.entity_id == *entity_id) {
                        let radius_sq = radius * radius;

                        for &player_id in connected_players {
                            if let Some(player_pos) = player_positions.get(&player_id) {
                                let dx = player_pos[0] - entity_pos.position[0];
                                let dy = player_pos[1] - entity_pos.position[1];
                                let dz = player_pos[2] - entity_pos.position[2];
                                let dist_sq = dx * dx + dy * dy + dz * dz;

                                if dist_sq <= radius_sq {
                                    deliveries.push(EventDelivery {
                                        target_player: player_id,
                                        event: event.clone(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        deliveries
    }

    /// Get the number of pending events.
    pub fn pending_count(&self) -> usize {
        self.pending_events.len()
    }

    /// Clear all pending events without distributing.
    pub fn clear(&mut self) {
        self.pending_events.clear();
    }
}

impl Default for EventDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn make_players(n: usize) -> Vec<PlayerId> {
        (0..n).map(|_| Uuid::new_v4()).collect()
    }

    #[test]
    fn test_emit_assigns_unique_ids() {
        let mut dispatcher = EventDispatcher::new();
        let id1 = dispatcher.emit("test", EventScope::Global, vec![], 1);
        let id2 = dispatcher.emit("test", EventScope::Global, vec![], 1);
        assert_ne!(id1, id2);
        assert_eq!(dispatcher.pending_count(), 2);
    }

    #[test]
    fn test_global_broadcast() {
        let mut dispatcher = EventDispatcher::new();
        let players = make_players(3);

        dispatcher.emit("chat", EventScope::Global, b"hello".to_vec(), 1);

        let deliveries = dispatcher.distribute(&players, &[], &HashMap::new());
        assert_eq!(deliveries.len(), 3);

        for delivery in &deliveries {
            assert!(players.contains(&delivery.target_player));
            assert_eq!(delivery.event.event_type, "chat");
        }
    }

    #[test]
    fn test_player_specific_event() {
        let mut dispatcher = EventDispatcher::new();
        let players = make_players(3);
        let target = players[1];

        dispatcher.emit(
            "reward",
            EventScope::Player { player_id: target },
            vec![],
            1,
        );

        let deliveries = dispatcher.distribute(&players, &[], &HashMap::new());
        assert_eq!(deliveries.len(), 1);
        assert_eq!(deliveries[0].target_player, target);
    }

    #[test]
    fn test_player_specific_disconnected_player_skipped() {
        let mut dispatcher = EventDispatcher::new();
        let connected = make_players(2);
        let disconnected = Uuid::new_v4();

        dispatcher.emit(
            "reward",
            EventScope::Player {
                player_id: disconnected,
            },
            vec![],
            1,
        );

        let deliveries = dispatcher.distribute(&connected, &[], &HashMap::new());
        assert!(deliveries.is_empty());
    }

    #[test]
    fn test_group_event() {
        let mut dispatcher = EventDispatcher::new();
        let players = make_players(5);
        let group = vec![players[0], players[2], players[4]];

        dispatcher.emit(
            "party_buff",
            EventScope::Group {
                player_ids: group.clone(),
            },
            vec![],
            1,
        );

        let deliveries = dispatcher.distribute(&players, &[], &HashMap::new());
        assert_eq!(deliveries.len(), 3);
        let targets: Vec<PlayerId> = deliveries.iter().map(|d| d.target_player).collect();
        assert!(targets.contains(&players[0]));
        assert!(targets.contains(&players[2]));
        assert!(targets.contains(&players[4]));
    }

    #[test]
    fn test_group_event_partial_connected() {
        let mut dispatcher = EventDispatcher::new();
        let connected = make_players(2);
        let disconnected = Uuid::new_v4();

        let group = vec![connected[0], disconnected, connected[1]];

        dispatcher.emit(
            "party_buff",
            EventScope::Group {
                player_ids: group,
            },
            vec![],
            1,
        );

        let deliveries = dispatcher.distribute(&connected, &[], &HashMap::new());
        assert_eq!(deliveries.len(), 2);
    }

    #[test]
    fn test_near_entity_event() {
        let mut dispatcher = EventDispatcher::new();
        let players = make_players(3);

        let entity_positions = vec![EntityPosition {
            entity_id: 100,
            position: [0.0, 0.0, 0.0],
            owner_player_id: None,
        }];

        let mut player_positions = HashMap::new();
        // Player 0 is close (distance 1)
        player_positions.insert(players[0], [1.0, 0.0, 0.0]);
        // Player 1 is far (distance 100)
        player_positions.insert(players[1], [100.0, 0.0, 0.0]);
        // Player 2 is close (distance ~1.7)
        player_positions.insert(players[2], [1.0, 1.0, 1.0]);

        dispatcher.emit(
            "explosion",
            EventScope::NearEntity {
                entity_id: 100,
                radius: 5.0,
            },
            vec![],
            1,
        );

        let deliveries = dispatcher.distribute(&players, &entity_positions, &player_positions);
        assert_eq!(deliveries.len(), 2);
        let targets: Vec<PlayerId> = deliveries.iter().map(|d| d.target_player).collect();
        assert!(targets.contains(&players[0]));
        assert!(targets.contains(&players[2]));
        assert!(!targets.contains(&players[1]));
    }

    #[test]
    fn test_near_entity_missing_entity() {
        let mut dispatcher = EventDispatcher::new();
        let players = make_players(2);

        dispatcher.emit(
            "explosion",
            EventScope::NearEntity {
                entity_id: 999,
                radius: 5.0,
            },
            vec![],
            1,
        );

        // Entity 999 doesn't exist, so no deliveries
        let deliveries = dispatcher.distribute(&players, &[], &HashMap::new());
        assert!(deliveries.is_empty());
    }

    #[test]
    fn test_near_entity_player_without_position() {
        let mut dispatcher = EventDispatcher::new();
        let players = make_players(2);

        let entity_positions = vec![EntityPosition {
            entity_id: 100,
            position: [0.0, 0.0, 0.0],
            owner_player_id: None,
        }];

        // Only one player has a position
        let mut player_positions = HashMap::new();
        player_positions.insert(players[0], [1.0, 0.0, 0.0]);
        // players[1] has no position

        dispatcher.emit(
            "explosion",
            EventScope::NearEntity {
                entity_id: 100,
                radius: 5.0,
            },
            vec![],
            1,
        );

        let deliveries = dispatcher.distribute(&players, &entity_positions, &player_positions);
        assert_eq!(deliveries.len(), 1);
        assert_eq!(deliveries[0].target_player, players[0]);
    }

    #[test]
    fn test_pending_cleared_after_distribute() {
        let mut dispatcher = EventDispatcher::new();
        dispatcher.emit("test", EventScope::Global, vec![], 1);
        assert_eq!(dispatcher.pending_count(), 1);

        let _ = dispatcher.distribute(&[], &[], &HashMap::new());
        assert_eq!(dispatcher.pending_count(), 0);
    }

    #[test]
    fn test_clear_events() {
        let mut dispatcher = EventDispatcher::new();
        dispatcher.emit("a", EventScope::Global, vec![], 1);
        dispatcher.emit("b", EventScope::Global, vec![], 1);
        assert_eq!(dispatcher.pending_count(), 2);

        dispatcher.clear();
        assert_eq!(dispatcher.pending_count(), 0);
    }

    #[test]
    fn test_multiple_events_distributed_together() {
        let mut dispatcher = EventDispatcher::new();
        let players = make_players(1);

        dispatcher.emit("chat", EventScope::Global, b"hello".to_vec(), 1);
        dispatcher.emit("system", EventScope::Global, b"welcome".to_vec(), 1);

        let deliveries = dispatcher.distribute(&players, &[], &HashMap::new());
        assert_eq!(deliveries.len(), 2);

        let types: Vec<&str> = deliveries.iter().map(|d| d.event.event_type.as_str()).collect();
        assert!(types.contains(&"chat"));
        assert!(types.contains(&"system"));
    }

    #[test]
    fn test_event_preserves_payload() {
        let mut dispatcher = EventDispatcher::new();
        let players = make_players(1);

        let payload = b"important data".to_vec();
        dispatcher.emit("test", EventScope::Global, payload.clone(), 42);

        let deliveries = dispatcher.distribute(&players, &[], &HashMap::new());
        assert_eq!(deliveries[0].event.payload, payload);
        assert_eq!(deliveries[0].event.tick, 42);
    }

    #[test]
    fn test_no_players_no_deliveries() {
        let mut dispatcher = EventDispatcher::new();
        dispatcher.emit("test", EventScope::Global, vec![], 1);

        let deliveries = dispatcher.distribute(&[], &[], &HashMap::new());
        assert!(deliveries.is_empty());
    }
}
