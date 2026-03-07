use crate::archetype::{Archetype, ArchetypeStorage};
use crate::component::ComponentId;

/// Describes what a system reads and writes.
#[derive(Clone, Debug, Default)]
pub struct AccessDescriptor {
    pub reads: Vec<ComponentId>,
    pub writes: Vec<ComponentId>,
}

impl AccessDescriptor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn read(mut self, id: ComponentId) -> Self {
        if !self.reads.contains(&id) && !self.writes.contains(&id) {
            self.reads.push(id);
        }
        self
    }

    pub fn write(mut self, id: ComponentId) -> Self {
        self.reads.retain(|&r| r != id);
        if !self.writes.contains(&id) {
            self.writes.push(id);
        }
        self
    }

    /// All component IDs this descriptor requires to be present.
    pub fn required_components(&self) -> Vec<ComponentId> {
        let mut all = self.reads.clone();
        all.extend(&self.writes);
        all.sort();
        all.dedup();
        all
    }

    /// Check if this access conflicts with another (write-write or read-write on same component).
    pub fn conflicts_with(&self, other: &AccessDescriptor) -> bool {
        for &w in &self.writes {
            if other.writes.contains(&w) || other.reads.contains(&w) {
                return true;
            }
        }
        for &w in &other.writes {
            if self.reads.contains(&w) {
                return true;
            }
        }
        false
    }
}

/// Result of a query: references to matching archetypes.
pub struct QueryResult<'a> {
    pub archetype_indices: Vec<usize>,
    pub storage: &'a ArchetypeStorage,
}

impl<'a> QueryResult<'a> {
    pub fn iter_archetypes(&self) -> impl Iterator<Item = &'a Archetype> + use<'a, '_> {
        self.archetype_indices
            .iter()
            .filter_map(|&idx| self.storage.get_archetype(idx))
    }

    pub fn entity_count(&self) -> usize {
        self.iter_archetypes().map(|a| a.len()).sum()
    }
}

/// Execute a query against the archetype storage.
pub fn query<'a>(storage: &'a ArchetypeStorage, access: &AccessDescriptor) -> QueryResult<'a> {
    let required = access.required_components();
    let archetype_indices = storage.find_archetypes_with(&required);
    QueryResult {
        archetype_indices,
        storage,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archetype::ArchetypeId;
    use crate::component::{Component, ComponentRegistry, ReplicationMode};
    use crate::entity::Entity;

    #[derive(Debug, Clone, Copy, PartialEq)]
    struct Position {
        x: f32,
        y: f32,
        z: f32,
    }
    impl Component for Position {
        fn replication_mode() -> ReplicationMode {
            ReplicationMode::Replicated
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    struct Velocity {
        x: f32,
        y: f32,
        z: f32,
    }
    impl Component for Velocity {}

    #[derive(Debug, Clone, Copy, PartialEq)]
    struct Health(u32);
    impl Component for Health {}

    #[test]
    fn access_descriptor_read_write() {
        let pos_id = ComponentId::of::<Position>();
        let vel_id = ComponentId::of::<Velocity>();

        let access = AccessDescriptor::new().read(pos_id).write(vel_id);

        assert_eq!(access.reads, vec![pos_id]);
        assert_eq!(access.writes, vec![vel_id]);
    }

    #[test]
    fn access_descriptor_write_overrides_read() {
        let pos_id = ComponentId::of::<Position>();
        let access = AccessDescriptor::new().read(pos_id).write(pos_id);
        assert!(access.reads.is_empty());
        assert_eq!(access.writes, vec![pos_id]);
    }

    #[test]
    fn access_descriptor_no_duplicates() {
        let pos_id = ComponentId::of::<Position>();
        let access = AccessDescriptor::new().read(pos_id).read(pos_id);
        assert_eq!(access.reads.len(), 1);
    }

    #[test]
    fn conflict_detection_write_write() {
        let pos_id = ComponentId::of::<Position>();
        let a = AccessDescriptor::new().write(pos_id);
        let b = AccessDescriptor::new().write(pos_id);
        assert!(a.conflicts_with(&b));
    }

    #[test]
    fn conflict_detection_read_write() {
        let pos_id = ComponentId::of::<Position>();
        let a = AccessDescriptor::new().read(pos_id);
        let b = AccessDescriptor::new().write(pos_id);
        assert!(a.conflicts_with(&b));
        assert!(b.conflicts_with(&a));
    }

    #[test]
    fn no_conflict_read_read() {
        let pos_id = ComponentId::of::<Position>();
        let a = AccessDescriptor::new().read(pos_id);
        let b = AccessDescriptor::new().read(pos_id);
        assert!(!a.conflicts_with(&b));
    }

    #[test]
    fn no_conflict_disjoint_writes() {
        let pos_id = ComponentId::of::<Position>();
        let vel_id = ComponentId::of::<Velocity>();
        let a = AccessDescriptor::new().write(pos_id);
        let b = AccessDescriptor::new().write(vel_id);
        assert!(!a.conflicts_with(&b));
    }

    #[test]
    fn required_components_combines_reads_and_writes() {
        let pos_id = ComponentId::of::<Position>();
        let vel_id = ComponentId::of::<Velocity>();
        let access = AccessDescriptor::new().read(pos_id).write(vel_id);
        let required = access.required_components();
        assert!(required.contains(&pos_id));
        assert!(required.contains(&vel_id));
        assert_eq!(required.len(), 2);
    }

    #[test]
    fn query_finds_matching_archetypes() {
        let mut registry = ComponentRegistry::new();
        registry.register::<Position>();
        registry.register::<Velocity>();
        registry.register::<Health>();

        let mut storage = ArchetypeStorage::new();

        // Archetype with Position + Velocity
        let arch1 = ArchetypeId::new(vec![
            ComponentId::of::<Position>(),
            ComponentId::of::<Velocity>(),
        ]);
        let idx1 = storage.get_or_create_archetype(arch1, &registry);

        // Archetype with Position only
        let arch2 = ArchetypeId::new(vec![ComponentId::of::<Position>()]);
        storage.get_or_create_archetype(arch2, &registry);

        // Archetype with Health only
        let arch3 = ArchetypeId::new(vec![ComponentId::of::<Health>()]);
        storage.get_or_create_archetype(arch3, &registry);

        // Add an entity to archetype 1
        let entity = Entity {
            index: 0,
            generation: 0,
        };
        let pos = Position {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        };
        let vel = Velocity {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        {
            let arch = storage.get_archetype_mut(idx1).unwrap();
            unsafe {
                arch.push_component_raw(
                    ComponentId::of::<Position>(),
                    &pos as *const _ as *const u8,
                );
                arch.push_component_raw(
                    ComponentId::of::<Velocity>(),
                    &vel as *const _ as *const u8,
                );
            }
            arch.push_entity(entity);
        }

        // Query for Position + Velocity
        let access = AccessDescriptor::new()
            .read(ComponentId::of::<Position>())
            .write(ComponentId::of::<Velocity>());
        let result = query(&storage, &access);
        assert_eq!(result.archetype_indices.len(), 1);
        assert_eq!(result.entity_count(), 1);
    }

    #[test]
    fn query_returns_empty_for_no_match() {
        let mut registry = ComponentRegistry::new();
        registry.register::<Position>();
        registry.register::<Health>();

        let mut storage = ArchetypeStorage::new();
        let arch = ArchetypeId::new(vec![ComponentId::of::<Position>()]);
        storage.get_or_create_archetype(arch, &registry);

        let access = AccessDescriptor::new().read(ComponentId::of::<Health>());
        let result = query(&storage, &access);
        assert_eq!(result.archetype_indices.len(), 0);
        assert_eq!(result.entity_count(), 0);
    }
}
