use std::collections::HashMap;

use uuid::Uuid;

use crate::manifest::WorldCategory;

/// Maximum length for world name.
const MAX_NAME_LENGTH: usize = 128;
/// Maximum length for world description.
const MAX_DESCRIPTION_LENGTH: usize = 4096;
/// Maximum number of tags per world.
const MAX_TAGS_PER_WORLD: usize = 20;

/// Status of a world in the registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryStatus {
    Active,
    Inactive,
    Deleted,
}

/// A world entry stored in the registry.
#[derive(Debug, Clone)]
pub struct WorldEntry {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub creator_id: Uuid,
    pub tags: Vec<String>,
    pub category: WorldCategory,
    pub max_players: u32,
    pub current_players: u32,
    pub rating: f32,
    pub total_ratings: u32,
    pub visit_count: u64,
    pub featured: bool,
    pub status: EntryStatus,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Partial update to apply to a world entry.
#[derive(Debug, Default)]
pub struct WorldUpdate {
    pub name: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub category: Option<WorldCategory>,
    pub max_players: Option<u32>,
    pub featured: Option<bool>,
}

/// Errors that can occur during registry operations.
#[derive(Debug, PartialEq, Eq)]
pub enum RegistryError {
    NotFound,
    DuplicateName,
    NameTooLong,
    DescriptionTooLong,
    TooManyTags,
    EmptyName,
    InvalidMaxPlayers,
}

/// In-memory world registry with CRUD operations.
pub struct WorldRegistry {
    worlds: HashMap<Uuid, WorldEntry>,
}

impl WorldRegistry {
    pub fn new() -> Self {
        Self {
            worlds: HashMap::new(),
        }
    }

    /// Register a new world entry. Returns the assigned UUID.
    pub fn register(&mut self, entry: WorldEntry) -> Result<Uuid, RegistryError> {
        self.validate_entry(&entry)?;

        let has_duplicate = self
            .worlds
            .values()
            .any(|w| w.name == entry.name && w.status != EntryStatus::Deleted);
        if has_duplicate {
            return Err(RegistryError::DuplicateName);
        }

        let id = entry.id;
        self.worlds.insert(id, entry);
        Ok(id)
    }

    /// Get a world entry by ID.
    pub fn get(&self, id: Uuid) -> Option<&WorldEntry> {
        self.worlds
            .get(&id)
            .filter(|w| w.status != EntryStatus::Deleted)
    }

    /// Update a world entry with partial fields.
    pub fn update(&mut self, id: Uuid, update: WorldUpdate) -> Result<(), RegistryError> {
        // Verify the entry exists and isn't deleted
        let exists = self
            .worlds
            .get(&id)
            .filter(|w| w.status != EntryStatus::Deleted)
            .is_some();
        if !exists {
            return Err(RegistryError::NotFound);
        }

        if let Some(ref name) = update.name {
            if name.trim().is_empty() {
                return Err(RegistryError::EmptyName);
            }
            if name.len() > MAX_NAME_LENGTH {
                return Err(RegistryError::NameTooLong);
            }
            // Check duplicate excluding self
            let has_dup = self.worlds.values().any(|w| {
                w.id != id && w.name == *name && w.status != EntryStatus::Deleted
            });
            if has_dup {
                return Err(RegistryError::DuplicateName);
            }
        }

        if let Some(ref desc) = update.description {
            if desc.len() > MAX_DESCRIPTION_LENGTH {
                return Err(RegistryError::DescriptionTooLong);
            }
        }

        if let Some(ref tags) = update.tags {
            if tags.len() > MAX_TAGS_PER_WORLD {
                return Err(RegistryError::TooManyTags);
            }
        }

        if let Some(max) = update.max_players {
            if max == 0 {
                return Err(RegistryError::InvalidMaxPlayers);
            }
        }

        // Re-borrow mutably after validation (duplicate check used immutable iteration above)
        let entry = self.worlds.get_mut(&id).unwrap();

        if let Some(name) = update.name {
            entry.name = name;
        }
        if let Some(description) = update.description {
            entry.description = description;
        }
        if let Some(tags) = update.tags {
            entry.tags = tags;
        }
        if let Some(category) = update.category {
            entry.category = category;
        }
        if let Some(max_players) = update.max_players {
            entry.max_players = max_players;
        }
        if let Some(featured) = update.featured {
            entry.featured = featured;
        }

        Ok(())
    }

    /// Soft-delete a world entry.
    pub fn delete(&mut self, id: Uuid) -> Result<(), RegistryError> {
        let entry = self
            .worlds
            .get_mut(&id)
            .filter(|w| w.status != EntryStatus::Deleted)
            .ok_or(RegistryError::NotFound)?;
        entry.status = EntryStatus::Deleted;
        Ok(())
    }

    /// List all active worlds by a specific creator.
    pub fn list_by_creator(&self, creator_id: Uuid) -> Vec<&WorldEntry> {
        self.worlds
            .values()
            .filter(|w| w.creator_id == creator_id && w.status != EntryStatus::Deleted)
            .collect()
    }

    /// Get all active worlds as a slice-compatible iterator.
    pub fn all_active(&self) -> Vec<&WorldEntry> {
        self.worlds
            .values()
            .filter(|w| w.status != EntryStatus::Deleted)
            .collect()
    }

    /// Get mutable reference to a world entry (for analytics updates).
    pub fn get_mut(&mut self, id: Uuid) -> Option<&mut WorldEntry> {
        self.worlds
            .get_mut(&id)
            .filter(|w| w.status != EntryStatus::Deleted)
    }

    fn validate_entry(&self, entry: &WorldEntry) -> Result<(), RegistryError> {
        if entry.name.trim().is_empty() {
            return Err(RegistryError::EmptyName);
        }
        if entry.name.len() > MAX_NAME_LENGTH {
            return Err(RegistryError::NameTooLong);
        }
        if entry.description.len() > MAX_DESCRIPTION_LENGTH {
            return Err(RegistryError::DescriptionTooLong);
        }
        if entry.tags.len() > MAX_TAGS_PER_WORLD {
            return Err(RegistryError::TooManyTags);
        }
        if entry.max_players == 0 {
            return Err(RegistryError::InvalidMaxPlayers);
        }
        Ok(())
    }
}

impl Default for WorldRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper to build a `WorldEntry` for tests.
pub fn make_entry(name: &str, creator_id: Uuid) -> WorldEntry {
    WorldEntry {
        id: Uuid::new_v4(),
        name: name.to_string(),
        description: String::new(),
        creator_id,
        tags: Vec::new(),
        category: WorldCategory::Social,
        max_players: 50,
        current_players: 0,
        rating: 0.0,
        total_ratings: 0,
        visit_count: 0,
        featured: false,
        status: EntryStatus::Active,
        created_at: 1000,
        updated_at: 1000,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn creator() -> Uuid {
        Uuid::new_v4()
    }

    #[test]
    fn register_and_get() {
        let mut reg = WorldRegistry::new();
        let cid = creator();
        let entry = make_entry("Test World", cid);
        let id = entry.id;
        reg.register(entry).unwrap();

        let w = reg.get(id).unwrap();
        assert_eq!(w.name, "Test World");
        assert_eq!(w.creator_id, cid);
    }

    #[test]
    fn register_duplicate_name_fails() {
        let mut reg = WorldRegistry::new();
        let cid = creator();
        reg.register(make_entry("Dup", cid)).unwrap();
        let result = reg.register(make_entry("Dup", cid));
        assert_eq!(result, Err(RegistryError::DuplicateName));
    }

    #[test]
    fn register_empty_name_fails() {
        let mut reg = WorldRegistry::new();
        let mut entry = make_entry("ok", creator());
        entry.name = "   ".to_string();
        assert_eq!(reg.register(entry), Err(RegistryError::EmptyName));
    }

    #[test]
    fn register_name_too_long_fails() {
        let mut reg = WorldRegistry::new();
        let mut entry = make_entry("ok", creator());
        entry.name = "a".repeat(MAX_NAME_LENGTH + 1);
        assert_eq!(reg.register(entry), Err(RegistryError::NameTooLong));
    }

    #[test]
    fn register_too_many_tags_fails() {
        let mut reg = WorldRegistry::new();
        let mut entry = make_entry("ok", creator());
        entry.tags = (0..MAX_TAGS_PER_WORLD + 1)
            .map(|i| format!("tag{i}"))
            .collect();
        assert_eq!(reg.register(entry), Err(RegistryError::TooManyTags));
    }

    #[test]
    fn register_zero_max_players_fails() {
        let mut reg = WorldRegistry::new();
        let mut entry = make_entry("ok", creator());
        entry.max_players = 0;
        assert_eq!(reg.register(entry), Err(RegistryError::InvalidMaxPlayers));
    }

    #[test]
    fn update_world() {
        let mut reg = WorldRegistry::new();
        let entry = make_entry("Old Name", creator());
        let id = entry.id;
        reg.register(entry).unwrap();

        reg.update(
            id,
            WorldUpdate {
                name: Some("New Name".to_string()),
                max_players: Some(100),
                ..Default::default()
            },
        )
        .unwrap();

        let w = reg.get(id).unwrap();
        assert_eq!(w.name, "New Name");
        assert_eq!(w.max_players, 100);
    }

    #[test]
    fn update_not_found() {
        let mut reg = WorldRegistry::new();
        let result = reg.update(Uuid::new_v4(), WorldUpdate::default());
        assert_eq!(result, Err(RegistryError::NotFound));
    }

    #[test]
    fn update_duplicate_name_fails() {
        let mut reg = WorldRegistry::new();
        let cid = creator();
        reg.register(make_entry("World A", cid)).unwrap();
        let entry_b = make_entry("World B", cid);
        let id_b = entry_b.id;
        reg.register(entry_b).unwrap();

        let result = reg.update(
            id_b,
            WorldUpdate {
                name: Some("World A".to_string()),
                ..Default::default()
            },
        );
        assert_eq!(result, Err(RegistryError::DuplicateName));
    }

    #[test]
    fn delete_world() {
        let mut reg = WorldRegistry::new();
        let entry = make_entry("Gone", creator());
        let id = entry.id;
        reg.register(entry).unwrap();

        reg.delete(id).unwrap();
        assert!(reg.get(id).is_none());
    }

    #[test]
    fn delete_not_found() {
        let mut reg = WorldRegistry::new();
        assert_eq!(reg.delete(Uuid::new_v4()), Err(RegistryError::NotFound));
    }

    #[test]
    fn delete_allows_reuse_of_name() {
        let mut reg = WorldRegistry::new();
        let cid = creator();
        let entry = make_entry("Reuse", cid);
        let id = entry.id;
        reg.register(entry).unwrap();
        reg.delete(id).unwrap();

        // Should be able to register the same name again
        reg.register(make_entry("Reuse", cid)).unwrap();
    }

    #[test]
    fn list_by_creator() {
        let mut reg = WorldRegistry::new();
        let alice = creator();
        let bob = creator();
        reg.register(make_entry("Alice1", alice)).unwrap();
        reg.register(make_entry("Alice2", alice)).unwrap();
        reg.register(make_entry("Bob1", bob)).unwrap();

        let alice_worlds = reg.list_by_creator(alice);
        assert_eq!(alice_worlds.len(), 2);

        let bob_worlds = reg.list_by_creator(bob);
        assert_eq!(bob_worlds.len(), 1);
    }

    #[test]
    fn list_by_creator_excludes_deleted() {
        let mut reg = WorldRegistry::new();
        let cid = creator();
        let entry = make_entry("Del", cid);
        let id = entry.id;
        reg.register(entry).unwrap();
        reg.delete(id).unwrap();

        assert_eq!(reg.list_by_creator(cid).len(), 0);
    }

    #[test]
    fn all_active_excludes_deleted() {
        let mut reg = WorldRegistry::new();
        let cid = creator();
        let e1 = make_entry("A", cid);
        let e2 = make_entry("B", cid);
        let id2 = e2.id;
        reg.register(e1).unwrap();
        reg.register(e2).unwrap();
        reg.delete(id2).unwrap();

        assert_eq!(reg.all_active().len(), 1);
    }
}
