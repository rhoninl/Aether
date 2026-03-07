use std::alloc::{self, Layout};
use std::collections::HashMap;
use std::ptr;

use crate::component::{ComponentId, ComponentInfo, ComponentRegistry};
use crate::entity::Entity;

/// A sorted set of ComponentIds that uniquely identifies an archetype.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ArchetypeId(pub(crate) Vec<ComponentId>);

impl ArchetypeId {
    pub fn new(mut ids: Vec<ComponentId>) -> Self {
        ids.sort();
        ids.dedup();
        ArchetypeId(ids)
    }

    pub fn contains(&self, id: ComponentId) -> bool {
        self.0.binary_search(&id).is_ok()
    }

    pub fn components(&self) -> &[ComponentId] {
        &self.0
    }

    pub fn with_component(&self, id: ComponentId) -> Self {
        let mut ids = self.0.clone();
        if !self.contains(id) {
            ids.push(id);
            ids.sort();
        }
        ArchetypeId(ids)
    }

    pub fn without_component(&self, id: ComponentId) -> Self {
        let ids: Vec<_> = self.0.iter().copied().filter(|&c| c != id).collect();
        ArchetypeId(ids)
    }
}

/// A typed column of component data stored contiguously in memory.
pub(crate) struct Column {
    data: *mut u8,
    len: usize,
    capacity: usize,
    item_size: usize,
    item_align: usize,
    drop_fn: Option<unsafe fn(*mut u8)>,
}

// SAFETY: Column data is only accessed through World which ensures proper synchronization
unsafe impl Send for Column {}
unsafe impl Sync for Column {}

const INITIAL_CAPACITY: usize = 64;

impl Column {
    pub fn new(info: &ComponentInfo) -> Self {
        let item_size = info.size;
        let item_align = if info.align == 0 { 1 } else { info.align };

        let (data, capacity) = if item_size == 0 {
            (ptr::null_mut(), usize::MAX)
        } else {
            let capacity = INITIAL_CAPACITY;
            let layout =
                Layout::from_size_align(item_size * capacity, item_align).expect("invalid layout");
            let data = unsafe { alloc::alloc(layout) };
            if data.is_null() {
                alloc::handle_alloc_error(layout);
            }
            (data, capacity)
        };

        Column {
            data,
            len: 0,
            capacity,
            item_size,
            item_align,
            drop_fn: info.drop_fn,
        }
    }

    fn grow(&mut self) {
        if self.item_size == 0 {
            return;
        }
        let new_capacity = self.capacity * 2;
        let old_layout = Layout::from_size_align(self.item_size * self.capacity, self.item_align)
            .expect("invalid layout");
        let new_layout = Layout::from_size_align(self.item_size * new_capacity, self.item_align)
            .expect("invalid layout");
        let new_data = unsafe { alloc::realloc(self.data, old_layout, new_layout.size()) };
        if new_data.is_null() {
            alloc::handle_alloc_error(new_layout);
        }
        self.data = new_data;
        self.capacity = new_capacity;
    }

    /// Push raw bytes for one component. Caller must ensure `src` is valid for `item_size` bytes.
    pub unsafe fn push_raw(&mut self, src: *const u8) {
        if self.len == self.capacity {
            self.grow();
        }
        if self.item_size > 0 {
            let dst = self.data.add(self.len * self.item_size);
            ptr::copy_nonoverlapping(src, dst, self.item_size);
        }
        self.len += 1;
    }

    /// Get a pointer to the component at `row`.
    pub unsafe fn get_raw(&self, row: usize) -> *const u8 {
        debug_assert!(row < self.len);
        if self.item_size == 0 {
            // ZST: return a non-null aligned dangling pointer
            self.item_align as *const u8
        } else {
            self.data.add(row * self.item_size)
        }
    }

    /// Get a mutable pointer to the component at `row`.
    pub unsafe fn get_raw_mut(&mut self, row: usize) -> *mut u8 {
        debug_assert!(row < self.len);
        if self.item_size == 0 {
            self.item_align as *mut u8
        } else {
            self.data.add(row * self.item_size)
        }
    }

    /// Drop the element at `row` in place. Must be called before swap_remove
    /// when the element is being destroyed (despawn), NOT when it was already
    /// moved out via raw copy (migration).
    ///
    /// # Safety
    /// Caller must ensure `row` is valid and the element has not already been moved out.
    pub unsafe fn drop_at(&self, row: usize) {
        debug_assert!(row < self.len);
        if let Some(drop_fn) = self.drop_fn {
            if self.item_size > 0 {
                let ptr = self.data.add(row * self.item_size);
                drop_fn(ptr);
            }
        }
    }

    /// Remove the element at `row` by swap-removing with the last element.
    /// Does NOT drop any elements — caller must handle drops explicitly via
    /// `drop_at` before calling this if the element is being destroyed.
    /// Returns true if a swap occurred (i.e., row was not the last element).
    pub unsafe fn swap_remove(&mut self, row: usize) -> bool {
        debug_assert!(row < self.len);
        let last = self.len - 1;
        let swapped = row != last;
        if swapped && self.item_size > 0 {
            let row_ptr = self.data.add(row * self.item_size);
            let last_ptr = self.data.add(last * self.item_size);
            ptr::copy_nonoverlapping(last_ptr, row_ptr, self.item_size);
        }
        self.len -= 1;
        swapped
    }

    pub fn len(&self) -> usize {
        self.len
    }
}

impl Drop for Column {
    fn drop(&mut self) {
        if self.item_size > 0 {
            // Drop all remaining elements
            if let Some(drop_fn) = self.drop_fn {
                for i in 0..self.len {
                    unsafe {
                        let ptr = self.data.add(i * self.item_size);
                        drop_fn(ptr);
                    }
                }
            }
            // Free the allocation
            if self.capacity > 0 {
                let layout =
                    Layout::from_size_align(self.item_size * self.capacity, self.item_align)
                        .expect("invalid layout");
                unsafe {
                    alloc::dealloc(self.data, layout);
                }
            }
        }
    }
}

/// Location of an entity within an archetype.
#[derive(Clone, Copy, Debug)]
pub struct EntityLocation {
    pub archetype_index: usize,
    pub row: usize,
}

/// An archetype stores all entities that share the same set of component types.
pub struct Archetype {
    pub(crate) id: ArchetypeId,
    pub(crate) columns: HashMap<ComponentId, Column>,
    pub(crate) entities: Vec<Entity>,
}

impl Archetype {
    pub fn new(id: ArchetypeId, registry: &ComponentRegistry) -> Self {
        let mut columns = HashMap::new();
        for &comp_id in id.components() {
            let info = registry
                .get(comp_id)
                .expect("component not registered in registry");
            columns.insert(comp_id, Column::new(info));
        }
        Archetype {
            id,
            columns,
            entities: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    pub fn has_component(&self, id: ComponentId) -> bool {
        self.columns.contains_key(&id)
    }

    pub fn entities(&self) -> &[Entity] {
        &self.entities
    }

    pub fn archetype_id(&self) -> &ArchetypeId {
        &self.id
    }

    /// Add an entity to this archetype. Component data must be pushed to columns separately.
    pub(crate) fn push_entity(&mut self, entity: Entity) -> usize {
        let row = self.entities.len();
        self.entities.push(entity);
        row
    }

    /// Remove an entity by swap-remove, dropping all component data at the row.
    /// Used when despawning an entity (destroying it).
    /// Returns the entity that was swapped into this position (if any).
    pub(crate) fn swap_remove_entity_drop(&mut self, row: usize) -> Option<Entity> {
        let last = self.entities.len() - 1;
        let swapped = if row != last {
            Some(self.entities[last])
        } else {
            None
        };

        // Drop all component data at this row before overwriting
        for column in self.columns.values() {
            unsafe {
                column.drop_at(row);
            }
        }

        self.entities.swap_remove(row);
        for column in self.columns.values_mut() {
            unsafe {
                column.swap_remove(row);
            }
        }

        swapped
    }

    /// Remove an entity by swap-remove WITHOUT dropping component data.
    /// Used during archetype migration where data has already been copied out.
    /// Returns the entity that was swapped into this position (if any).
    pub(crate) fn swap_remove_entity_move(&mut self, row: usize) -> Option<Entity> {
        let last = self.entities.len() - 1;
        let swapped = if row != last {
            Some(self.entities[last])
        } else {
            None
        };
        self.entities.swap_remove(row);

        for column in self.columns.values_mut() {
            unsafe {
                column.swap_remove(row);
            }
        }

        swapped
    }

    /// Get a typed reference to a component for an entity at `row`.
    ///
    /// # Safety
    /// Caller must ensure T matches the actual component type at this ComponentId.
    pub unsafe fn get_component<T: 'static>(&self, comp_id: ComponentId, row: usize) -> &T {
        let column = self
            .columns
            .get(&comp_id)
            .expect("component not in archetype");
        &*(column.get_raw(row) as *const T)
    }

    /// Get a typed mutable reference to a component for an entity at `row`.
    ///
    /// # Safety
    /// Caller must ensure T matches the actual component type at this ComponentId.
    pub unsafe fn get_component_mut<T: 'static>(
        &mut self,
        comp_id: ComponentId,
        row: usize,
    ) -> &mut T {
        let column = self
            .columns
            .get_mut(&comp_id)
            .expect("component not in archetype");
        &mut *(column.get_raw_mut(row) as *mut T)
    }

    /// Push raw component data for a given component ID.
    ///
    /// # Safety
    /// Caller must ensure `src` points to valid data of the correct component type.
    pub(crate) unsafe fn push_component_raw(&mut self, comp_id: ComponentId, src: *const u8) {
        let column = self
            .columns
            .get_mut(&comp_id)
            .expect("component not in archetype");
        column.push_raw(src);
    }
}

/// Storage that manages all archetypes and the entity-to-archetype mapping.
pub struct ArchetypeStorage {
    archetypes: Vec<Archetype>,
    archetype_index: HashMap<ArchetypeId, usize>,
    entity_locations: HashMap<Entity, EntityLocation>,
}

impl ArchetypeStorage {
    pub fn new() -> Self {
        Self {
            archetypes: Vec::new(),
            archetype_index: HashMap::new(),
            entity_locations: HashMap::new(),
        }
    }

    pub fn get_or_create_archetype(
        &mut self,
        id: ArchetypeId,
        registry: &ComponentRegistry,
    ) -> usize {
        if let Some(&index) = self.archetype_index.get(&id) {
            return index;
        }
        let index = self.archetypes.len();
        self.archetypes.push(Archetype::new(id.clone(), registry));
        self.archetype_index.insert(id, index);
        index
    }

    pub fn get_archetype(&self, index: usize) -> Option<&Archetype> {
        self.archetypes.get(index)
    }

    pub fn get_archetype_mut(&mut self, index: usize) -> Option<&mut Archetype> {
        self.archetypes.get_mut(index)
    }

    pub fn entity_location(&self, entity: Entity) -> Option<EntityLocation> {
        self.entity_locations.get(&entity).copied()
    }

    pub fn set_entity_location(&mut self, entity: Entity, location: EntityLocation) {
        self.entity_locations.insert(entity, location);
    }

    pub fn remove_entity_location(&mut self, entity: Entity) {
        self.entity_locations.remove(&entity);
    }

    pub fn archetypes(&self) -> &[Archetype] {
        &self.archetypes
    }

    pub fn archetype_count(&self) -> usize {
        self.archetypes.len()
    }

    /// Find all archetypes that contain ALL of the given component IDs.
    pub fn find_archetypes_with(&self, component_ids: &[ComponentId]) -> Vec<usize> {
        self.archetypes
            .iter()
            .enumerate()
            .filter(|(_, arch)| component_ids.iter().all(|id| arch.has_component(*id)))
            .map(|(i, _)| i)
            .collect()
    }
}

impl Default for ArchetypeStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::{Component, ReplicationMode};

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

    fn setup_registry() -> ComponentRegistry {
        let mut reg = ComponentRegistry::new();
        reg.register::<Position>();
        reg.register::<Velocity>();
        reg.register::<Health>();
        reg
    }

    #[test]
    fn archetype_id_sorts_components() {
        let id_a = ComponentId::of::<Position>();
        let id_b = ComponentId::of::<Velocity>();
        let arch1 = ArchetypeId::new(vec![id_a, id_b]);
        let arch2 = ArchetypeId::new(vec![id_b, id_a]);
        assert_eq!(arch1, arch2);
    }

    #[test]
    fn archetype_id_deduplicates() {
        let id_a = ComponentId::of::<Position>();
        let arch = ArchetypeId::new(vec![id_a, id_a, id_a]);
        assert_eq!(arch.components().len(), 1);
    }

    #[test]
    fn archetype_id_with_component() {
        let id_a = ComponentId::of::<Position>();
        let id_b = ComponentId::of::<Velocity>();
        let arch = ArchetypeId::new(vec![id_a]);
        let arch2 = arch.with_component(id_b);
        assert!(arch2.contains(id_a));
        assert!(arch2.contains(id_b));
    }

    #[test]
    fn archetype_id_without_component() {
        let id_a = ComponentId::of::<Position>();
        let id_b = ComponentId::of::<Velocity>();
        let arch = ArchetypeId::new(vec![id_a, id_b]);
        let arch2 = arch.without_component(id_b);
        assert!(arch2.contains(id_a));
        assert!(!arch2.contains(id_b));
    }

    #[test]
    fn archetype_push_and_get_component() {
        let registry = setup_registry();
        let arch_id = ArchetypeId::new(vec![
            ComponentId::of::<Position>(),
            ComponentId::of::<Velocity>(),
        ]);
        let mut archetype = Archetype::new(arch_id, &registry);

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
            x: 4.0,
            y: 5.0,
            z: 6.0,
        };

        unsafe {
            archetype.push_component_raw(
                ComponentId::of::<Position>(),
                &pos as *const Position as *const u8,
            );
            archetype.push_component_raw(
                ComponentId::of::<Velocity>(),
                &vel as *const Velocity as *const u8,
            );
        }
        archetype.push_entity(entity);

        unsafe {
            let got_pos: &Position = archetype.get_component(ComponentId::of::<Position>(), 0);
            let got_vel: &Velocity = archetype.get_component(ComponentId::of::<Velocity>(), 0);
            assert_eq!(*got_pos, pos);
            assert_eq!(*got_vel, vel);
        }
    }

    #[test]
    fn archetype_swap_remove() {
        let registry = setup_registry();
        let arch_id = ArchetypeId::new(vec![ComponentId::of::<Position>()]);
        let mut archetype = Archetype::new(arch_id, &registry);

        let e0 = Entity {
            index: 0,
            generation: 0,
        };
        let e1 = Entity {
            index: 1,
            generation: 0,
        };
        let e2 = Entity {
            index: 2,
            generation: 0,
        };

        let positions = [
            Position {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            Position {
                x: 1.0,
                y: 1.0,
                z: 1.0,
            },
            Position {
                x: 2.0,
                y: 2.0,
                z: 2.0,
            },
        ];

        for (entity, pos) in [e0, e1, e2].iter().zip(positions.iter()) {
            unsafe {
                archetype.push_component_raw(
                    ComponentId::of::<Position>(),
                    pos as *const Position as *const u8,
                );
            }
            archetype.push_entity(*entity);
        }

        assert_eq!(archetype.len(), 3);

        // Remove middle entity (e1 at row 1), e2 should be swapped in
        let swapped = archetype.swap_remove_entity_drop(1);
        assert_eq!(swapped, Some(e2));
        assert_eq!(archetype.len(), 2);
        assert_eq!(archetype.entities()[0], e0);
        assert_eq!(archetype.entities()[1], e2);

        // Position at row 1 should now be e2's position
        unsafe {
            let pos: &Position = archetype.get_component(ComponentId::of::<Position>(), 1);
            assert_eq!(*pos, positions[2]);
        }
    }

    #[test]
    fn archetype_storage_get_or_create() {
        let registry = setup_registry();
        let mut storage = ArchetypeStorage::new();

        let arch_id = ArchetypeId::new(vec![
            ComponentId::of::<Position>(),
            ComponentId::of::<Velocity>(),
        ]);

        let idx1 = storage.get_or_create_archetype(arch_id.clone(), &registry);
        let idx2 = storage.get_or_create_archetype(arch_id, &registry);
        assert_eq!(idx1, idx2);
        assert_eq!(storage.archetype_count(), 1);
    }

    #[test]
    fn archetype_storage_find_archetypes_with() {
        let registry = setup_registry();
        let mut storage = ArchetypeStorage::new();

        let arch1 = ArchetypeId::new(vec![
            ComponentId::of::<Position>(),
            ComponentId::of::<Velocity>(),
        ]);
        let arch2 = ArchetypeId::new(vec![ComponentId::of::<Position>()]);
        let arch3 = ArchetypeId::new(vec![ComponentId::of::<Health>()]);

        storage.get_or_create_archetype(arch1, &registry);
        storage.get_or_create_archetype(arch2, &registry);
        storage.get_or_create_archetype(arch3, &registry);

        let with_pos = storage.find_archetypes_with(&[ComponentId::of::<Position>()]);
        assert_eq!(with_pos.len(), 2);

        let with_pos_vel = storage
            .find_archetypes_with(&[ComponentId::of::<Position>(), ComponentId::of::<Velocity>()]);
        assert_eq!(with_pos_vel.len(), 1);

        let with_health = storage.find_archetypes_with(&[ComponentId::of::<Health>()]);
        assert_eq!(with_health.len(), 1);
    }

    #[test]
    fn entity_location_tracking() {
        let mut storage = ArchetypeStorage::new();
        let entity = Entity {
            index: 0,
            generation: 0,
        };
        let loc = EntityLocation {
            archetype_index: 0,
            row: 0,
        };
        storage.set_entity_location(entity, loc);
        assert!(storage.entity_location(entity).is_some());
        storage.remove_entity_location(entity);
        assert!(storage.entity_location(entity).is_none());
    }

    #[test]
    fn column_basic_operations() {
        let mut registry = ComponentRegistry::new();
        registry.register::<Position>();
        let info = registry.get(ComponentId::of::<Position>()).unwrap();
        let mut column = Column::new(info);

        let pos = Position {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        };
        unsafe {
            column.push_raw(&pos as *const Position as *const u8);
        }
        assert_eq!(column.len(), 1);

        unsafe {
            let got = &*(column.get_raw(0) as *const Position);
            assert_eq!(*got, pos);
        }
    }
}
