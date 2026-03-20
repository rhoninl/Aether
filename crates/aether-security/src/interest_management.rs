//! Interest management for entity visibility control.
//!
//! Determines which entities are visible to which players based on
//! spatial proximity and access-control permissions. Prevents information
//! leaks by ensuring hidden/unauthorized entities are never sent to clients.

use std::collections::{HashMap, HashSet};
use std::fmt;

use crate::movement_validator::Vec3;

/// Default interest radius in meters. Entities beyond this distance
/// from a player are not sent to that player's client.
const DEFAULT_INTEREST_RADIUS: f32 = 200.0;

/// Configuration for interest management.
#[derive(Debug, Clone)]
pub struct InterestConfig {
    /// Maximum distance at which entities are visible to a player.
    pub interest_radius: f32,
}

impl Default for InterestConfig {
    fn default() -> Self {
        Self {
            interest_radius: DEFAULT_INTEREST_RADIUS,
        }
    }
}

/// Visibility permission for an entity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Visibility {
    /// Visible to all players within interest radius.
    Public,
    /// Only visible to players in the allowed set.
    Restricted(HashSet<u64>),
    /// Hidden from all players (server-only entity).
    Hidden,
}

/// An entity tracked by the interest manager.
#[derive(Debug, Clone)]
pub struct TrackedEntity {
    pub entity_id: u64,
    pub position: Vec3,
    pub visibility: Visibility,
}

/// Result of querying visible entities for a player.
#[derive(Debug, Clone)]
pub struct VisibilityResult {
    /// Entity IDs that should be sent to this player.
    pub visible: Vec<u64>,
    /// Entity IDs that were filtered out (for logging/debugging).
    pub filtered_count: usize,
}

/// Reason an entity was filtered out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterReason {
    /// Entity is outside the player's interest radius.
    OutOfRange,
    /// Entity has restricted visibility and the player is not authorized.
    Unauthorized,
    /// Entity is hidden from all players.
    Hidden,
}

impl fmt::Display for FilterReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FilterReason::OutOfRange => write!(f, "out of interest range"),
            FilterReason::Unauthorized => write!(f, "unauthorized access"),
            FilterReason::Hidden => write!(f, "hidden entity"),
        }
    }
}

/// Manages entity visibility for all players.
#[derive(Debug)]
pub struct InterestManager {
    config: InterestConfig,
    entities: HashMap<u64, TrackedEntity>,
}

impl InterestManager {
    /// Creates a new interest manager with the given configuration.
    pub fn new(config: InterestConfig) -> Self {
        Self {
            config,
            entities: HashMap::new(),
        }
    }

    /// Creates a new interest manager with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(InterestConfig::default())
    }

    /// Registers or updates an entity in the interest manager.
    pub fn upsert_entity(&mut self, entity: TrackedEntity) {
        self.entities.insert(entity.entity_id, entity);
    }

    /// Removes an entity from tracking.
    pub fn remove_entity(&mut self, entity_id: u64) {
        self.entities.remove(&entity_id);
    }

    /// Updates an entity's position.
    pub fn update_position(&mut self, entity_id: u64, position: Vec3) {
        if let Some(entity) = self.entities.get_mut(&entity_id) {
            entity.position = position;
        }
    }

    /// Updates an entity's visibility.
    pub fn update_visibility(&mut self, entity_id: u64, visibility: Visibility) {
        if let Some(entity) = self.entities.get_mut(&entity_id) {
            entity.visibility = visibility;
        }
    }

    /// Returns the set of entity IDs that should be visible to a given player.
    ///
    /// # Arguments
    /// - `player_id`: The player querying visibility.
    /// - `player_pos`: The player's current position.
    ///
    /// # Returns
    /// A `VisibilityResult` containing visible entity IDs and the filtered count.
    pub fn visible_entities(&self, player_id: u64, player_pos: &Vec3) -> VisibilityResult {
        let mut visible = Vec::new();
        let mut filtered_count = 0;

        for entity in self.entities.values() {
            match self.check_visibility(player_id, player_pos, entity) {
                Ok(()) => visible.push(entity.entity_id),
                Err(_) => filtered_count += 1,
            }
        }

        VisibilityResult {
            visible,
            filtered_count,
        }
    }

    /// Checks whether a specific entity is visible to a specific player.
    ///
    /// Returns `Ok(())` if visible, `Err(FilterReason)` if not.
    pub fn check_visibility(
        &self,
        player_id: u64,
        player_pos: &Vec3,
        entity: &TrackedEntity,
    ) -> Result<(), FilterReason> {
        // Check visibility permissions first
        match &entity.visibility {
            Visibility::Hidden => return Err(FilterReason::Hidden),
            Visibility::Restricted(allowed) => {
                if !allowed.contains(&player_id) {
                    return Err(FilterReason::Unauthorized);
                }
            }
            Visibility::Public => {}
        }

        // Check spatial proximity
        let distance = player_pos.distance_to(&entity.position);
        if distance > self.config.interest_radius {
            return Err(FilterReason::OutOfRange);
        }

        Ok(())
    }

    /// Returns the number of tracked entities.
    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_public_entity(id: u64, pos: Vec3) -> TrackedEntity {
        TrackedEntity {
            entity_id: id,
            position: pos,
            visibility: Visibility::Public,
        }
    }

    fn make_hidden_entity(id: u64, pos: Vec3) -> TrackedEntity {
        TrackedEntity {
            entity_id: id,
            position: pos,
            visibility: Visibility::Hidden,
        }
    }

    fn make_restricted_entity(id: u64, pos: Vec3, allowed: HashSet<u64>) -> TrackedEntity {
        TrackedEntity {
            entity_id: id,
            position: pos,
            visibility: Visibility::Restricted(allowed),
        }
    }

    // --- Basic visibility ---

    #[test]
    fn test_empty_manager_returns_no_entities() {
        let mgr = InterestManager::with_defaults();
        let result = mgr.visible_entities(1, &Vec3::zero());
        assert!(result.visible.is_empty());
        assert_eq!(result.filtered_count, 0);
    }

    #[test]
    fn test_public_entity_in_range() {
        let mut mgr = InterestManager::with_defaults();
        mgr.upsert_entity(make_public_entity(10, Vec3::new(50.0, 0.0, 0.0)));
        let result = mgr.visible_entities(1, &Vec3::zero());
        assert_eq!(result.visible, vec![10]);
        assert_eq!(result.filtered_count, 0);
    }

    #[test]
    fn test_public_entity_out_of_range() {
        let mut mgr = InterestManager::with_defaults();
        mgr.upsert_entity(make_public_entity(10, Vec3::new(300.0, 0.0, 0.0)));
        let result = mgr.visible_entities(1, &Vec3::zero());
        assert!(result.visible.is_empty());
        assert_eq!(result.filtered_count, 1);
    }

    #[test]
    fn test_public_entity_at_exact_radius() {
        let mut mgr = InterestManager::with_defaults();
        mgr.upsert_entity(make_public_entity(10, Vec3::new(200.0, 0.0, 0.0)));
        let result = mgr.visible_entities(1, &Vec3::zero());
        assert_eq!(result.visible, vec![10]);
    }

    #[test]
    fn test_public_entity_just_beyond_radius() {
        let mut mgr = InterestManager::with_defaults();
        mgr.upsert_entity(make_public_entity(10, Vec3::new(200.01, 0.0, 0.0)));
        let result = mgr.visible_entities(1, &Vec3::zero());
        assert!(result.visible.is_empty());
    }

    // --- Hidden entities ---

    #[test]
    fn test_hidden_entity_never_visible() {
        let mut mgr = InterestManager::with_defaults();
        mgr.upsert_entity(make_hidden_entity(10, Vec3::zero()));
        let result = mgr.visible_entities(1, &Vec3::zero());
        assert!(result.visible.is_empty());
        assert_eq!(result.filtered_count, 1);
    }

    #[test]
    fn test_hidden_entity_even_at_same_position() {
        let mut mgr = InterestManager::with_defaults();
        mgr.upsert_entity(make_hidden_entity(10, Vec3::new(1.0, 0.0, 0.0)));
        let result = mgr.visible_entities(1, &Vec3::new(1.0, 0.0, 0.0));
        assert!(result.visible.is_empty());
    }

    // --- Restricted entities ---

    #[test]
    fn test_restricted_entity_authorized_player() {
        let mut mgr = InterestManager::with_defaults();
        let allowed: HashSet<u64> = [1, 2].into_iter().collect();
        mgr.upsert_entity(make_restricted_entity(
            10,
            Vec3::new(50.0, 0.0, 0.0),
            allowed,
        ));
        let result = mgr.visible_entities(1, &Vec3::zero());
        assert_eq!(result.visible, vec![10]);
    }

    #[test]
    fn test_restricted_entity_unauthorized_player() {
        let mut mgr = InterestManager::with_defaults();
        let allowed: HashSet<u64> = [2, 3].into_iter().collect();
        mgr.upsert_entity(make_restricted_entity(
            10,
            Vec3::new(50.0, 0.0, 0.0),
            allowed,
        ));
        let result = mgr.visible_entities(1, &Vec3::zero());
        assert!(result.visible.is_empty());
        assert_eq!(result.filtered_count, 1);
    }

    #[test]
    fn test_restricted_entity_authorized_but_out_of_range() {
        let mut mgr = InterestManager::with_defaults();
        let allowed: HashSet<u64> = [1].into_iter().collect();
        mgr.upsert_entity(make_restricted_entity(
            10,
            Vec3::new(500.0, 0.0, 0.0),
            allowed,
        ));
        let result = mgr.visible_entities(1, &Vec3::zero());
        assert!(result.visible.is_empty());
    }

    // --- Multiple entities mixed ---

    #[test]
    fn test_mixed_visibility() {
        let mut mgr = InterestManager::with_defaults();
        mgr.upsert_entity(make_public_entity(1, Vec3::new(10.0, 0.0, 0.0)));
        mgr.upsert_entity(make_hidden_entity(2, Vec3::new(10.0, 0.0, 0.0)));
        mgr.upsert_entity(make_public_entity(3, Vec3::new(500.0, 0.0, 0.0)));

        let allowed: HashSet<u64> = [100].into_iter().collect();
        mgr.upsert_entity(make_restricted_entity(
            4,
            Vec3::new(10.0, 0.0, 0.0),
            allowed,
        ));

        let result = mgr.visible_entities(100, &Vec3::zero());
        // Entities 1 (public, in range), 4 (restricted but player 100 is authorized)
        assert!(result.visible.contains(&1));
        assert!(result.visible.contains(&4));
        assert!(!result.visible.contains(&2)); // hidden
        assert!(!result.visible.contains(&3)); // out of range
        assert_eq!(result.filtered_count, 2);
    }

    // --- Entity management ---

    #[test]
    fn test_upsert_updates_position() {
        let mut mgr = InterestManager::with_defaults();
        mgr.upsert_entity(make_public_entity(10, Vec3::new(500.0, 0.0, 0.0)));
        let result = mgr.visible_entities(1, &Vec3::zero());
        assert!(result.visible.is_empty());

        // Update entity to be in range
        mgr.upsert_entity(make_public_entity(10, Vec3::new(50.0, 0.0, 0.0)));
        let result = mgr.visible_entities(1, &Vec3::zero());
        assert_eq!(result.visible, vec![10]);
    }

    #[test]
    fn test_remove_entity() {
        let mut mgr = InterestManager::with_defaults();
        mgr.upsert_entity(make_public_entity(10, Vec3::new(50.0, 0.0, 0.0)));
        assert_eq!(mgr.entity_count(), 1);
        mgr.remove_entity(10);
        assert_eq!(mgr.entity_count(), 0);
        let result = mgr.visible_entities(1, &Vec3::zero());
        assert!(result.visible.is_empty());
    }

    #[test]
    fn test_update_position() {
        let mut mgr = InterestManager::with_defaults();
        mgr.upsert_entity(make_public_entity(10, Vec3::new(500.0, 0.0, 0.0)));
        mgr.update_position(10, Vec3::new(50.0, 0.0, 0.0));
        let result = mgr.visible_entities(1, &Vec3::zero());
        assert_eq!(result.visible, vec![10]);
    }

    #[test]
    fn test_update_visibility() {
        let mut mgr = InterestManager::with_defaults();
        mgr.upsert_entity(make_public_entity(10, Vec3::new(50.0, 0.0, 0.0)));

        // Make it hidden
        mgr.update_visibility(10, Visibility::Hidden);
        let result = mgr.visible_entities(1, &Vec3::zero());
        assert!(result.visible.is_empty());

        // Make it public again
        mgr.update_visibility(10, Visibility::Public);
        let result = mgr.visible_entities(1, &Vec3::zero());
        assert_eq!(result.visible, vec![10]);
    }

    // --- check_visibility directly ---

    #[test]
    fn test_check_visibility_hidden() {
        let mgr = InterestManager::with_defaults();
        let entity = make_hidden_entity(10, Vec3::zero());
        let result = mgr.check_visibility(1, &Vec3::zero(), &entity);
        assert_eq!(result, Err(FilterReason::Hidden));
    }

    #[test]
    fn test_check_visibility_unauthorized() {
        let mgr = InterestManager::with_defaults();
        let allowed: HashSet<u64> = [2].into_iter().collect();
        let entity = make_restricted_entity(10, Vec3::zero(), allowed);
        let result = mgr.check_visibility(1, &Vec3::zero(), &entity);
        assert_eq!(result, Err(FilterReason::Unauthorized));
    }

    #[test]
    fn test_check_visibility_out_of_range() {
        let mgr = InterestManager::with_defaults();
        let entity = make_public_entity(10, Vec3::new(999.0, 0.0, 0.0));
        let result = mgr.check_visibility(1, &Vec3::zero(), &entity);
        assert_eq!(result, Err(FilterReason::OutOfRange));
    }

    // --- Custom config ---

    #[test]
    fn test_custom_small_radius() {
        let config = InterestConfig {
            interest_radius: 10.0,
        };
        let mut mgr = InterestManager::new(config);
        mgr.upsert_entity(make_public_entity(10, Vec3::new(15.0, 0.0, 0.0)));
        let result = mgr.visible_entities(1, &Vec3::zero());
        assert!(result.visible.is_empty());
    }

    // --- Display ---

    #[test]
    fn test_filter_reason_display() {
        assert!(FilterReason::OutOfRange.to_string().contains("range"));
        assert!(FilterReason::Unauthorized
            .to_string()
            .contains("unauthorized"));
        assert!(FilterReason::Hidden.to_string().contains("hidden"));
    }
}
