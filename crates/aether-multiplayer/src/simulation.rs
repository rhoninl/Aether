//! Server-side simulation: validates inputs and updates authoritative state.

use std::collections::HashMap;

use crate::avatar::AvatarState;
use crate::protocol::PlayerId;

/// Server-side authoritative world state.
#[derive(Debug)]
pub struct WorldState {
    avatars: HashMap<PlayerId, AvatarState>,
}

impl WorldState {
    pub fn new() -> Self {
        Self {
            avatars: HashMap::new(),
        }
    }

    /// Add a new player with default avatar state.
    pub fn add_player(&mut self, player_id: PlayerId) {
        self.avatars.insert(player_id, AvatarState::default());
    }

    /// Remove a player from the world.
    pub fn remove_player(&mut self, player_id: &PlayerId) {
        self.avatars.remove(player_id);
    }

    /// Apply a validated input update for a player.
    /// Returns true if the input was accepted (possibly clamped).
    pub fn apply_input(&mut self, player_id: &PlayerId, mut avatar: AvatarState) -> bool {
        let previous = match self.avatars.get(player_id) {
            Some(prev) => prev.clone(),
            None => return false,
        };

        // Validate and clamp the incoming state
        avatar.validate_and_clamp();

        // Check speed limit; if exceeded, use previous position
        if avatar.exceeds_speed_limit(&previous) {
            tracing::warn!(
                player_id = %player_id,
                "input exceeds speed limit, rejecting position update"
            );
            // Keep rotations but reject position change
            let clamped = AvatarState {
                head_position: previous.head_position,
                head_rotation: avatar.head_rotation,
                left_hand_position: previous.left_hand_position,
                left_hand_rotation: avatar.left_hand_rotation,
                right_hand_position: previous.right_hand_position,
                right_hand_rotation: avatar.right_hand_rotation,
            };
            self.avatars.insert(*player_id, clamped);
            return true;
        }

        self.avatars.insert(*player_id, avatar);
        true
    }

    /// Get the current avatar state for a player.
    pub fn get_avatar(&self, player_id: &PlayerId) -> Option<&AvatarState> {
        self.avatars.get(player_id)
    }

    /// Get all avatar states as a vec of (player_id, avatar) pairs.
    pub fn all_avatars(&self) -> Vec<(PlayerId, AvatarState)> {
        self.avatars
            .iter()
            .map(|(id, avatar)| (*id, avatar.clone()))
            .collect()
    }

    /// Get the number of players in the world.
    pub fn player_count(&self) -> usize {
        self.avatars.len()
    }

    /// Check if a player exists in the world.
    pub fn has_player(&self, player_id: &PlayerId) -> bool {
        self.avatars.contains_key(player_id)
    }
}

impl Default for WorldState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn new_world_is_empty() {
        let world = WorldState::new();
        assert_eq!(world.player_count(), 0);
    }

    #[test]
    fn add_player_creates_default_avatar() {
        let mut world = WorldState::new();
        let pid = Uuid::new_v4();
        world.add_player(pid);
        assert_eq!(world.player_count(), 1);
        assert!(world.has_player(&pid));
        let avatar = world.get_avatar(&pid).unwrap();
        assert_eq!(*avatar, AvatarState::default());
    }

    #[test]
    fn remove_player_decreases_count() {
        let mut world = WorldState::new();
        let pid = Uuid::new_v4();
        world.add_player(pid);
        world.remove_player(&pid);
        assert_eq!(world.player_count(), 0);
        assert!(!world.has_player(&pid));
    }

    #[test]
    fn apply_input_updates_avatar() {
        let mut world = WorldState::new();
        let pid = Uuid::new_v4();
        world.add_player(pid);

        let mut new_avatar = AvatarState::default();
        new_avatar.head_position = [1.0, 1.7, 2.0];
        assert!(world.apply_input(&pid, new_avatar.clone()));

        let avatar = world.get_avatar(&pid).unwrap();
        assert_eq!(avatar.head_position, [1.0, 1.7, 2.0]);
    }

    #[test]
    fn apply_input_to_nonexistent_player_returns_false() {
        let mut world = WorldState::new();
        let pid = Uuid::new_v4();
        assert!(!world.apply_input(&pid, AvatarState::default()));
    }

    #[test]
    fn apply_input_clamps_nan_values() {
        let mut world = WorldState::new();
        let pid = Uuid::new_v4();
        world.add_player(pid);

        let mut bad_avatar = AvatarState::default();
        bad_avatar.head_position = [f32::NAN, 1.7, 0.0];
        assert!(world.apply_input(&pid, bad_avatar));

        let avatar = world.get_avatar(&pid).unwrap();
        assert!(!avatar.head_position[0].is_nan());
    }

    #[test]
    fn apply_input_rejects_teleport_position() {
        let mut world = WorldState::new();
        let pid = Uuid::new_v4();
        world.add_player(pid);

        let mut teleport = AvatarState::default();
        teleport.head_position = [1000.0, 1.7, 0.0]; // far from default
        assert!(world.apply_input(&pid, teleport));

        // Position should stay at default (speed limit exceeded)
        let avatar = world.get_avatar(&pid).unwrap();
        assert_eq!(avatar.head_position, AvatarState::default().head_position);
    }

    #[test]
    fn apply_input_keeps_rotation_on_speed_reject() {
        let mut world = WorldState::new();
        let pid = Uuid::new_v4();
        world.add_player(pid);

        let mut teleport = AvatarState::default();
        teleport.head_position = [1000.0, 1.7, 0.0];
        teleport.head_rotation = [0.0, 0.707, 0.0, 0.707];
        assert!(world.apply_input(&pid, teleport));

        let avatar = world.get_avatar(&pid).unwrap();
        // Rotation should be updated even though position was rejected
        assert!((avatar.head_rotation[1] - 0.707).abs() < 0.01);
    }

    #[test]
    fn all_avatars_returns_all_players() {
        let mut world = WorldState::new();
        let p1 = Uuid::new_v4();
        let p2 = Uuid::new_v4();
        world.add_player(p1);
        world.add_player(p2);

        let all = world.all_avatars();
        assert_eq!(all.len(), 2);
        let ids: Vec<PlayerId> = all.iter().map(|(id, _)| *id).collect();
        assert!(ids.contains(&p1));
        assert!(ids.contains(&p2));
    }

    #[test]
    fn normal_movement_accepted() {
        let mut world = WorldState::new();
        let pid = Uuid::new_v4();
        world.add_player(pid);

        let mut moved = AvatarState::default();
        moved.head_position[0] += 0.5; // small step
        assert!(world.apply_input(&pid, moved.clone()));

        let avatar = world.get_avatar(&pid).unwrap();
        assert!((avatar.head_position[0] - moved.head_position[0]).abs() < 0.01);
    }

    #[test]
    fn multiple_sequential_moves() {
        let mut world = WorldState::new();
        let pid = Uuid::new_v4();
        world.add_player(pid);

        for i in 1..=10 {
            let mut avatar = world.get_avatar(&pid).unwrap().clone();
            avatar.head_position[0] += 0.1;
            world.apply_input(&pid, avatar);
            let current = world.get_avatar(&pid).unwrap();
            assert!((current.head_position[0] - 0.1 * i as f32).abs() < 0.02);
        }
    }

    #[test]
    fn remove_nonexistent_player_is_noop() {
        let mut world = WorldState::new();
        let pid = Uuid::new_v4();
        world.remove_player(&pid); // should not panic
        assert_eq!(world.player_count(), 0);
    }
}
