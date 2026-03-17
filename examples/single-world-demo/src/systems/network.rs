//! Network system: bridges aether-multiplayer with ECS, syncing remote player avatars.

use std::collections::HashMap;

use uuid::Uuid;

use aether_multiplayer::avatar::AvatarState;
use aether_multiplayer::protocol::{PlayerId, ServerMessage};

use crate::components::Transform;

/// Default scale for spawned avatar entities.
const DEFAULT_AVATAR_SCALE: [f32; 3] = [1.0, 1.0, 1.0];

/// Registry that maps remote player IDs to local entity data.
///
/// Tracks which remote players have entities spawned in the local world,
/// so the network system can create/update/remove avatar entities as
/// players join and leave.
#[derive(Debug, Default)]
pub struct AvatarRegistry {
    entries: HashMap<PlayerId, Transform>,
}

impl AvatarRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Track a new remote player. Returns `true` if the player was newly
    /// inserted, `false` if they were already registered.
    pub fn register(&mut self, player_id: PlayerId, transform: Transform) -> bool {
        if self.entries.contains_key(&player_id) {
            return false;
        }
        self.entries.insert(player_id, transform);
        true
    }

    /// Remove a remote player from tracking. Returns `true` if the player
    /// was found and removed, `false` if they were not registered.
    pub fn unregister(&mut self, player_id: PlayerId) -> bool {
        self.entries.remove(&player_id).is_some()
    }

    /// Get the current transform for a remote player.
    pub fn get_transform(&self, player_id: PlayerId) -> Option<&Transform> {
        self.entries.get(&player_id)
    }

    /// Update a remote player's position/rotation. Does nothing if the
    /// player is not registered.
    pub fn update_transform(&mut self, player_id: PlayerId, transform: Transform) {
        if let Some(entry) = self.entries.get_mut(&player_id) {
            *entry = transform;
        }
    }

    /// List all tracked player IDs.
    pub fn player_ids(&self) -> Vec<Uuid> {
        self.entries.keys().copied().collect()
    }

    /// Number of tracked remote players.
    pub fn count(&self) -> usize {
        self.entries.len()
    }
}

/// Convert a network `AvatarState` to a local `Transform`.
///
/// Uses the head position and head rotation as the entity's spatial data,
/// since the avatar's head is the canonical reference point.
pub fn avatar_state_to_transform(avatar: &AvatarState) -> Transform {
    Transform {
        position: avatar.head_position,
        rotation: avatar.head_rotation,
        scale: DEFAULT_AVATAR_SCALE,
    }
}

/// Process a `ServerMessage` and update the avatar registry accordingly.
///
/// - `PlayerJoined`: registers the new player with a default avatar transform.
/// - `PlayerLeft`: unregisters the player.
/// - `WorldState`: updates transforms for all avatars in the delta.
/// - `FullSync`: clears the registry and repopulates from the full state.
/// - `Pong`: ignored (latency measurement only).
pub fn apply_server_message(msg: &ServerMessage, registry: &mut AvatarRegistry) {
    match msg {
        ServerMessage::PlayerJoined { player_id } => {
            let default_avatar = AvatarState::default();
            let transform = avatar_state_to_transform(&default_avatar);
            registry.register(*player_id, transform);
        }
        ServerMessage::PlayerLeft { player_id } => {
            registry.unregister(*player_id);
        }
        ServerMessage::WorldState { avatars, .. } => {
            for (player_id, avatar) in avatars {
                let transform = avatar_state_to_transform(avatar);
                if registry.get_transform(*player_id).is_some() {
                    registry.update_transform(*player_id, transform);
                } else {
                    registry.register(*player_id, transform);
                }
            }
        }
        ServerMessage::FullSync { avatars, .. } => {
            // Clear existing entries and repopulate.
            let existing_ids = registry.player_ids();
            for id in existing_ids {
                registry.unregister(id);
            }
            for (player_id, avatar) in avatars {
                let transform = avatar_state_to_transform(avatar);
                registry.register(*player_id, transform);
            }
        }
        ServerMessage::Pong { .. } => {
            // Latency measurement; nothing to do for the avatar registry.
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- AvatarRegistry tests --

    #[test]
    fn registry_starts_empty() {
        let registry = AvatarRegistry::new();
        assert_eq!(registry.count(), 0);
        assert!(registry.player_ids().is_empty());
    }

    #[test]
    fn registry_default_is_empty() {
        let registry = AvatarRegistry::default();
        assert_eq!(registry.count(), 0);
    }

    #[test]
    fn register_new_player_returns_true() {
        let mut registry = AvatarRegistry::new();
        let id = Uuid::new_v4();
        assert!(registry.register(id, Transform::default()));
        assert_eq!(registry.count(), 1);
    }

    #[test]
    fn register_duplicate_player_returns_false() {
        let mut registry = AvatarRegistry::new();
        let id = Uuid::new_v4();
        assert!(registry.register(id, Transform::default()));
        assert!(!registry.register(id, Transform::at(1.0, 2.0, 3.0)));
        assert_eq!(registry.count(), 1);
    }

    #[test]
    fn unregister_existing_player_returns_true() {
        let mut registry = AvatarRegistry::new();
        let id = Uuid::new_v4();
        registry.register(id, Transform::default());
        assert!(registry.unregister(id));
        assert_eq!(registry.count(), 0);
    }

    #[test]
    fn unregister_missing_player_returns_false() {
        let mut registry = AvatarRegistry::new();
        assert!(!registry.unregister(Uuid::new_v4()));
    }

    #[test]
    fn get_transform_for_existing_player() {
        let mut registry = AvatarRegistry::new();
        let id = Uuid::new_v4();
        registry.register(id, Transform::at(5.0, 6.0, 7.0));
        let t = registry.get_transform(id).unwrap();
        assert_eq!(t.position, [5.0, 6.0, 7.0]);
    }

    #[test]
    fn get_transform_for_missing_player_returns_none() {
        let registry = AvatarRegistry::new();
        assert!(registry.get_transform(Uuid::new_v4()).is_none());
    }

    #[test]
    fn update_transform_changes_position() {
        let mut registry = AvatarRegistry::new();
        let id = Uuid::new_v4();
        registry.register(id, Transform::default());
        registry.update_transform(id, Transform::at(10.0, 20.0, 30.0));
        let t = registry.get_transform(id).unwrap();
        assert_eq!(t.position, [10.0, 20.0, 30.0]);
    }

    #[test]
    fn update_transform_for_missing_player_is_noop() {
        let mut registry = AvatarRegistry::new();
        // Should not panic.
        registry.update_transform(Uuid::new_v4(), Transform::default());
        assert_eq!(registry.count(), 0);
    }

    #[test]
    fn player_ids_returns_all_registered() {
        let mut registry = AvatarRegistry::new();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        registry.register(id1, Transform::default());
        registry.register(id2, Transform::default());
        let ids = registry.player_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&id1));
        assert!(ids.contains(&id2));
    }

    #[test]
    fn register_multiple_players() {
        let mut registry = AvatarRegistry::new();
        for _ in 0..10 {
            registry.register(Uuid::new_v4(), Transform::default());
        }
        assert_eq!(registry.count(), 10);
    }

    // -- avatar_state_to_transform tests --

    #[test]
    fn convert_default_avatar_to_transform() {
        let avatar = AvatarState::default();
        let t = avatar_state_to_transform(&avatar);
        assert_eq!(t.position, avatar.head_position);
        assert_eq!(t.rotation, avatar.head_rotation);
        assert_eq!(t.scale, [1.0, 1.0, 1.0]);
    }

    #[test]
    fn convert_custom_avatar_to_transform() {
        let mut avatar = AvatarState::default();
        avatar.head_position = [3.0, 4.0, 5.0];
        avatar.head_rotation = [0.1, 0.2, 0.3, 0.9];
        let t = avatar_state_to_transform(&avatar);
        assert_eq!(t.position, [3.0, 4.0, 5.0]);
        assert_eq!(t.rotation, [0.1, 0.2, 0.3, 0.9]);
    }

    #[test]
    fn avatar_transform_uses_head_not_hands() {
        let mut avatar = AvatarState::default();
        avatar.head_position = [1.0, 2.0, 3.0];
        avatar.left_hand_position = [10.0, 20.0, 30.0];
        avatar.right_hand_position = [100.0, 200.0, 300.0];
        let t = avatar_state_to_transform(&avatar);
        assert_eq!(t.position, [1.0, 2.0, 3.0]);
    }

    // -- apply_server_message tests --

    #[test]
    fn apply_player_joined_registers_player() {
        let mut registry = AvatarRegistry::new();
        let id = Uuid::new_v4();
        let msg = ServerMessage::PlayerJoined { player_id: id };
        apply_server_message(&msg, &mut registry);
        assert_eq!(registry.count(), 1);
        assert!(registry.get_transform(id).is_some());
    }

    #[test]
    fn apply_player_left_unregisters_player() {
        let mut registry = AvatarRegistry::new();
        let id = Uuid::new_v4();
        registry.register(id, Transform::default());
        let msg = ServerMessage::PlayerLeft { player_id: id };
        apply_server_message(&msg, &mut registry);
        assert_eq!(registry.count(), 0);
    }

    #[test]
    fn apply_player_left_for_unknown_player_is_noop() {
        let mut registry = AvatarRegistry::new();
        let msg = ServerMessage::PlayerLeft {
            player_id: Uuid::new_v4(),
        };
        apply_server_message(&msg, &mut registry);
        assert_eq!(registry.count(), 0);
    }

    #[test]
    fn apply_world_state_updates_existing_players() {
        let mut registry = AvatarRegistry::new();
        let id = Uuid::new_v4();
        registry.register(id, Transform::default());

        let mut avatar = AvatarState::default();
        avatar.head_position = [9.0, 8.0, 7.0];
        let msg = ServerMessage::WorldState {
            tick: 1,
            avatars: vec![(id, avatar)],
        };
        apply_server_message(&msg, &mut registry);

        let t = registry.get_transform(id).unwrap();
        assert_eq!(t.position, [9.0, 8.0, 7.0]);
    }

    #[test]
    fn apply_world_state_registers_new_players() {
        let mut registry = AvatarRegistry::new();
        let id = Uuid::new_v4();
        let msg = ServerMessage::WorldState {
            tick: 1,
            avatars: vec![(id, AvatarState::default())],
        };
        apply_server_message(&msg, &mut registry);
        assert_eq!(registry.count(), 1);
    }

    #[test]
    fn apply_full_sync_replaces_all_players() {
        let mut registry = AvatarRegistry::new();
        let old_id = Uuid::new_v4();
        registry.register(old_id, Transform::default());

        let new_id = Uuid::new_v4();
        let msg = ServerMessage::FullSync {
            tick: 100,
            avatars: vec![(new_id, AvatarState::default())],
        };
        apply_server_message(&msg, &mut registry);

        assert_eq!(registry.count(), 1);
        assert!(registry.get_transform(old_id).is_none());
        assert!(registry.get_transform(new_id).is_some());
    }

    #[test]
    fn apply_full_sync_with_empty_avatars_clears_registry() {
        let mut registry = AvatarRegistry::new();
        registry.register(Uuid::new_v4(), Transform::default());
        registry.register(Uuid::new_v4(), Transform::default());

        let msg = ServerMessage::FullSync {
            tick: 50,
            avatars: vec![],
        };
        apply_server_message(&msg, &mut registry);
        assert_eq!(registry.count(), 0);
    }

    #[test]
    fn apply_pong_does_nothing() {
        let mut registry = AvatarRegistry::new();
        let id = Uuid::new_v4();
        registry.register(id, Transform::default());

        let msg = ServerMessage::Pong {
            client_time_ms: 100,
            server_time_ms: 200,
        };
        apply_server_message(&msg, &mut registry);
        assert_eq!(registry.count(), 1);
    }

    #[test]
    fn apply_multiple_messages_in_sequence() {
        let mut registry = AvatarRegistry::new();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        apply_server_message(
            &ServerMessage::PlayerJoined { player_id: id1 },
            &mut registry,
        );
        apply_server_message(
            &ServerMessage::PlayerJoined { player_id: id2 },
            &mut registry,
        );
        assert_eq!(registry.count(), 2);

        apply_server_message(
            &ServerMessage::PlayerLeft { player_id: id1 },
            &mut registry,
        );
        assert_eq!(registry.count(), 1);
        assert!(registry.get_transform(id2).is_some());
    }

    #[test]
    fn apply_world_state_with_multiple_avatars() {
        let mut registry = AvatarRegistry::new();
        let ids: Vec<Uuid> = (0..5).map(|_| Uuid::new_v4()).collect();
        let avatars: Vec<(PlayerId, AvatarState)> = ids
            .iter()
            .map(|id| (*id, AvatarState::default()))
            .collect();
        let msg = ServerMessage::WorldState {
            tick: 42,
            avatars,
        };
        apply_server_message(&msg, &mut registry);
        assert_eq!(registry.count(), 5);
    }
}
