use crate::archetype::{ArchetypeId, ArchetypeStorage, EntityLocation};
use crate::component::{Component, ComponentId, ComponentRegistry};
use crate::entity::{Entity, EntityAllocator};
use crate::network::{replicated_component_ids, server_only_component_ids};
use crate::query::{self, AccessDescriptor, QueryResult};
use crate::schedule::{RuntimeAlert, Schedule, ScheduleDiagnostics, ScheduleMetrics};
use crate::system::System;

/// The World is the central data structure of the ECS.
/// It owns all entities, components, archetypes, and the system schedule.
pub struct World {
    entities: EntityAllocator,
    pub(crate) storage: ArchetypeStorage,
    pub(crate) registry: ComponentRegistry,
    schedule: Schedule,
}

impl World {
    pub fn new() -> Self {
        Self {
            entities: EntityAllocator::new(),
            storage: ArchetypeStorage::new(),
            registry: ComponentRegistry::new(),
            schedule: Schedule::new(),
        }
    }

    /// Register a component type. Must be called before spawning entities with this component.
    pub fn register_component<T: Component>(&mut self) -> ComponentId {
        self.registry.register::<T>()
    }

    /// Spawn a new empty entity (no components).
    pub fn spawn_empty(&mut self) -> Entity {
        self.entities.allocate()
    }

    /// Spawn an entity with a single component.
    pub fn spawn_with_1<A: Component>(&mut self, a: A) -> Entity {
        self.registry.register::<A>();
        let entity = self.entities.allocate();
        let arch_id = ArchetypeId::new(vec![ComponentId::of::<A>()]);
        let arch_idx = self
            .storage
            .get_or_create_archetype(arch_id, &self.registry);
        let arch = self.storage.get_archetype_mut(arch_idx).unwrap();
        unsafe {
            arch.push_component_raw(ComponentId::of::<A>(), &a as *const A as *const u8);
        }
        let row = arch.push_entity(entity);
        self.storage.set_entity_location(
            entity,
            EntityLocation {
                archetype_index: arch_idx,
                row,
            },
        );
        std::mem::forget(a);
        entity
    }

    /// Spawn an entity with two components.
    pub fn spawn_with_2<A: Component, B: Component>(&mut self, a: A, b: B) -> Entity {
        self.registry.register::<A>();
        self.registry.register::<B>();
        let entity = self.entities.allocate();
        let arch_id = ArchetypeId::new(vec![ComponentId::of::<A>(), ComponentId::of::<B>()]);
        let arch_idx = self
            .storage
            .get_or_create_archetype(arch_id, &self.registry);
        let arch = self.storage.get_archetype_mut(arch_idx).unwrap();
        unsafe {
            arch.push_component_raw(ComponentId::of::<A>(), &a as *const A as *const u8);
            arch.push_component_raw(ComponentId::of::<B>(), &b as *const B as *const u8);
        }
        let row = arch.push_entity(entity);
        self.storage.set_entity_location(
            entity,
            EntityLocation {
                archetype_index: arch_idx,
                row,
            },
        );
        std::mem::forget(a);
        std::mem::forget(b);
        entity
    }

    /// Spawn an entity with three components.
    pub fn spawn_with_3<A: Component, B: Component, C: Component>(
        &mut self,
        a: A,
        b: B,
        c: C,
    ) -> Entity {
        self.registry.register::<A>();
        self.registry.register::<B>();
        self.registry.register::<C>();
        let entity = self.entities.allocate();
        let arch_id = ArchetypeId::new(vec![
            ComponentId::of::<A>(),
            ComponentId::of::<B>(),
            ComponentId::of::<C>(),
        ]);
        let arch_idx = self
            .storage
            .get_or_create_archetype(arch_id, &self.registry);
        let arch = self.storage.get_archetype_mut(arch_idx).unwrap();
        unsafe {
            arch.push_component_raw(ComponentId::of::<A>(), &a as *const A as *const u8);
            arch.push_component_raw(ComponentId::of::<B>(), &b as *const B as *const u8);
            arch.push_component_raw(ComponentId::of::<C>(), &c as *const C as *const u8);
        }
        let row = arch.push_entity(entity);
        self.storage.set_entity_location(
            entity,
            EntityLocation {
                archetype_index: arch_idx,
                row,
            },
        );
        std::mem::forget(a);
        std::mem::forget(b);
        std::mem::forget(c);
        entity
    }

    /// Despawn an entity, removing it from its archetype.
    pub fn despawn(&mut self, entity: Entity) -> bool {
        if !self.entities.is_alive(entity) {
            return false;
        }

        if let Some(location) = self.storage.entity_location(entity) {
            let swapped = {
                let arch = self
                    .storage
                    .get_archetype_mut(location.archetype_index)
                    .unwrap();
                arch.swap_remove_entity_drop(location.row)
            };

            // If an entity was swapped into this position, update its location
            if let Some(swapped_entity) = swapped {
                self.storage.set_entity_location(
                    swapped_entity,
                    EntityLocation {
                        archetype_index: location.archetype_index,
                        row: location.row,
                    },
                );
            }

            self.storage.remove_entity_location(entity);
        }

        self.entities.deallocate(entity);
        true
    }

    /// Check if an entity is alive.
    pub fn is_alive(&self, entity: Entity) -> bool {
        self.entities.is_alive(entity)
    }

    /// Get a component reference for an entity.
    pub fn get_component<T: Component>(&self, entity: Entity) -> Option<&T> {
        if !self.entities.is_alive(entity) {
            return None;
        }
        let location = self.storage.entity_location(entity)?;
        let arch = self.storage.get_archetype(location.archetype_index)?;
        if !arch.has_component(ComponentId::of::<T>()) {
            return None;
        }
        unsafe { Some(arch.get_component::<T>(ComponentId::of::<T>(), location.row)) }
    }

    /// Get a mutable component reference for an entity.
    pub fn get_component_mut<T: Component>(&mut self, entity: Entity) -> Option<&mut T> {
        if !self.entities.is_alive(entity) {
            return None;
        }
        let location = self.storage.entity_location(entity)?;
        let arch = self.storage.get_archetype_mut(location.archetype_index)?;
        if !arch.has_component(ComponentId::of::<T>()) {
            return None;
        }
        unsafe { Some(arch.get_component_mut::<T>(ComponentId::of::<T>(), location.row)) }
    }

    /// Add a component to an existing entity (triggers archetype migration).
    pub fn add_component<T: Component>(&mut self, entity: Entity, component: T) -> bool {
        if !self.entities.is_alive(entity) {
            return false;
        }
        self.registry.register::<T>();
        let comp_id = ComponentId::of::<T>();

        let location = match self.storage.entity_location(entity) {
            Some(loc) => loc,
            None => {
                // Entity has no components yet, create archetype with just this one
                let arch_id = ArchetypeId::new(vec![comp_id]);
                let arch_idx = self
                    .storage
                    .get_or_create_archetype(arch_id, &self.registry);
                let arch = self.storage.get_archetype_mut(arch_idx).unwrap();
                unsafe {
                    arch.push_component_raw(comp_id, &component as *const T as *const u8);
                }
                let row = arch.push_entity(entity);
                self.storage.set_entity_location(
                    entity,
                    EntityLocation {
                        archetype_index: arch_idx,
                        row,
                    },
                );
                std::mem::forget(component);
                return true;
            }
        };

        let old_arch = self
            .storage
            .get_archetype(location.archetype_index)
            .unwrap();
        if old_arch.has_component(comp_id) {
            return false; // Already has this component
        }

        let new_arch_id = old_arch.archetype_id().with_component(comp_id);
        let new_arch_idx = self
            .storage
            .get_or_create_archetype(new_arch_id.clone(), &self.registry);

        // Copy existing component data to new archetype
        let old_arch_idx = location.archetype_index;
        let old_row = location.row;

        // Get shared component IDs
        let old_comp_ids: Vec<ComponentId> = {
            let old_arch = self.storage.get_archetype(old_arch_idx).unwrap();
            old_arch.archetype_id().components().to_vec()
        };

        // Copy shared components to new archetype
        for &shared_id in &old_comp_ids {
            let src_ptr = {
                let old_arch = self.storage.get_archetype(old_arch_idx).unwrap();
                let col = old_arch.columns.get(&shared_id).unwrap();
                unsafe { col.get_raw(old_row) as *const u8 }
            };
            let info = self.registry.get(shared_id).unwrap();
            // Copy data
            let new_arch = self.storage.get_archetype_mut(new_arch_idx).unwrap();
            unsafe {
                // We need to copy item_size bytes
                let mut buf = vec![0u8; info.size];
                std::ptr::copy_nonoverlapping(src_ptr, buf.as_mut_ptr(), info.size);
                new_arch.push_component_raw(shared_id, buf.as_ptr());
            }
        }

        // Push the new component
        let new_arch = self.storage.get_archetype_mut(new_arch_idx).unwrap();
        unsafe {
            new_arch.push_component_raw(comp_id, &component as *const T as *const u8);
        }
        let new_row = new_arch.push_entity(entity);

        // Remove from old archetype
        let swapped = {
            let old_arch = self.storage.get_archetype_mut(old_arch_idx).unwrap();
            old_arch.swap_remove_entity_move(old_row)
        };
        if let Some(swapped_entity) = swapped {
            self.storage.set_entity_location(
                swapped_entity,
                EntityLocation {
                    archetype_index: old_arch_idx,
                    row: old_row,
                },
            );
        }

        self.storage.set_entity_location(
            entity,
            EntityLocation {
                archetype_index: new_arch_idx,
                row: new_row,
            },
        );

        std::mem::forget(component);
        true
    }

    /// Remove a component from an entity (triggers archetype migration).
    pub fn remove_component<T: Component>(&mut self, entity: Entity) -> bool {
        if !self.entities.is_alive(entity) {
            return false;
        }
        let comp_id = ComponentId::of::<T>();

        let location = match self.storage.entity_location(entity) {
            Some(loc) => loc,
            None => return false,
        };

        let old_arch = self
            .storage
            .get_archetype(location.archetype_index)
            .unwrap();
        if !old_arch.has_component(comp_id) {
            return false;
        }

        let new_arch_id = old_arch.archetype_id().without_component(comp_id);
        let old_arch_idx = location.archetype_index;
        let old_row = location.row;

        if new_arch_id.components().is_empty() {
            // Removing the last component - drop all data and remove from archetype
            let swapped = {
                let old_arch = self.storage.get_archetype_mut(old_arch_idx).unwrap();
                old_arch.swap_remove_entity_drop(old_row)
            };
            if let Some(swapped_entity) = swapped {
                self.storage.set_entity_location(
                    swapped_entity,
                    EntityLocation {
                        archetype_index: old_arch_idx,
                        row: old_row,
                    },
                );
            }
            self.storage.remove_entity_location(entity);
            return true;
        }

        let new_arch_idx = self
            .storage
            .get_or_create_archetype(new_arch_id.clone(), &self.registry);

        // Copy all components except the removed one
        let keep_ids: Vec<ComponentId> = new_arch_id.components().to_vec();

        for &keep_id in &keep_ids {
            let src_ptr = {
                let old_arch = self.storage.get_archetype(old_arch_idx).unwrap();
                let col = old_arch.columns.get(&keep_id).unwrap();
                unsafe { col.get_raw(old_row) as *const u8 }
            };
            let info = self.registry.get(keep_id).unwrap();
            let new_arch = self.storage.get_archetype_mut(new_arch_idx).unwrap();
            unsafe {
                let mut buf = vec![0u8; info.size];
                std::ptr::copy_nonoverlapping(src_ptr, buf.as_mut_ptr(), info.size);
                new_arch.push_component_raw(keep_id, buf.as_ptr());
            }
        }

        let new_arch = self.storage.get_archetype_mut(new_arch_idx).unwrap();
        let new_row = new_arch.push_entity(entity);

        // Drop the removed component's data before the swap_remove
        {
            let old_arch = self.storage.get_archetype(old_arch_idx).unwrap();
            if let Some(col) = old_arch.columns.get(&comp_id) {
                unsafe {
                    col.drop_at(old_row);
                }
            }
        }

        // Remove from old archetype (no drops — kept components were copied, removed was dropped above)
        let swapped = {
            let old_arch = self.storage.get_archetype_mut(old_arch_idx).unwrap();
            old_arch.swap_remove_entity_move(old_row)
        };
        if let Some(swapped_entity) = swapped {
            self.storage.set_entity_location(
                swapped_entity,
                EntityLocation {
                    archetype_index: old_arch_idx,
                    row: old_row,
                },
            );
        }

        self.storage.set_entity_location(
            entity,
            EntityLocation {
                archetype_index: new_arch_idx,
                row: new_row,
            },
        );

        true
    }

    /// Query the world for entities matching an access descriptor.
    pub fn query(&self, access: &AccessDescriptor) -> QueryResult<'_> {
        query::query(&self.storage, access)
    }

    /// Add a system to the world's schedule.
    pub fn add_system(&mut self, system: Box<dyn System>) {
        self.schedule.add_system(system);
    }

    /// Run all systems in the schedule.
    pub fn run_systems(&mut self) {
        // Extract schedule to avoid aliasing &mut schedule with &World.
        let mut schedule = std::mem::take(&mut self.schedule);
        schedule.run(self);
        self.schedule = schedule;
    }

    /// Run all systems and return a metrics snapshot for observability tooling.
    pub fn run_systems_with_metrics(&mut self) -> ScheduleMetrics {
        let mut schedule = std::mem::take(&mut self.schedule);
        schedule.run(self);
        let metrics = schedule.metrics();
        self.schedule = schedule;
        metrics
    }

    /// Render the latest schedule metrics into Prometheus exposition format.
    pub fn metrics_prometheus(&self) -> String {
        self.schedule.metrics_prometheus()
    }

    /// Evaluate runtime alert rules and return alerts for schedule metrics.
    pub fn evaluate_alerts(
        &self,
        max_stage_time_ns: u128,
        max_system_time_ns: u128,
    ) -> Vec<RuntimeAlert> {
        self.schedule
            .evaluate_alerts(max_stage_time_ns, max_system_time_ns)
    }

    /// Get the latest collected schedule metrics.
    pub fn metrics(&self) -> ScheduleMetrics {
        self.schedule.metrics()
    }

    /// Get compact scheduling diagnostics for profiler-style consumers.
    pub fn diagnostics(&self) -> ScheduleDiagnostics {
        self.schedule.diagnostics()
    }

    /// Clear collected schedule metrics.
    pub fn clear_metrics(&mut self) {
        self.schedule.clear_metrics();
    }

    /// Get the number of alive entities.
    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }

    /// Get the component registry.
    pub fn registry(&self) -> &ComponentRegistry {
        &self.registry
    }

    /// Get the archetype storage.
    pub fn archetype_storage(&self) -> &ArchetypeStorage {
        &self.storage
    }

    /// Get all replicated component IDs from a set of component IDs.
    pub fn replicated_components(&self, ids: &[ComponentId]) -> Vec<ComponentId> {
        replicated_component_ids(&self.registry, ids)
    }

    /// Get all server-only component IDs from a set of component IDs.
    pub fn server_only_components(&self, ids: &[ComponentId]) -> Vec<ComponentId> {
        server_only_component_ids(&self.registry, ids)
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::ReplicationMode;
    use crate::network::Authority;
    use crate::network::NetworkIdentity;
    use crate::Stage;

    #[derive(Debug, Clone, Copy, PartialEq)]
    struct Transform {
        x: f32,
        y: f32,
        z: f32,
    }
    impl Component for Transform {
        fn replication_mode() -> ReplicationMode {
            ReplicationMode::Replicated
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    struct RigidBody {
        mass: f32,
    }
    impl Component for RigidBody {
        fn replication_mode() -> ReplicationMode {
            ReplicationMode::ServerOnly
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    struct MeshRenderer {
        mesh_id: u32,
    }
    impl Component for MeshRenderer {
        fn replication_mode() -> ReplicationMode {
            ReplicationMode::Replicated
        }
    }

    #[test]
    fn spawn_and_get_component() {
        let mut world = World::new();
        let pos = Transform {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        };
        let entity = world.spawn_with_1(pos);
        assert!(world.is_alive(entity));
        let got = world.get_component::<Transform>(entity).unwrap();
        assert_eq!(*got, pos);
    }

    #[test]
    fn spawn_with_two_components() {
        let mut world = World::new();
        let pos = Transform {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        };
        let rb = RigidBody { mass: 10.0 };
        let entity = world.spawn_with_2(pos, rb);

        assert_eq!(*world.get_component::<Transform>(entity).unwrap(), pos);
        assert_eq!(*world.get_component::<RigidBody>(entity).unwrap(), rb);
    }

    #[test]
    fn spawn_with_three_components() {
        let mut world = World::new();
        let pos = Transform {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        };
        let rb = RigidBody { mass: 5.0 };
        let mesh = MeshRenderer { mesh_id: 42 };
        let entity = world.spawn_with_3(pos, rb, mesh);

        assert_eq!(*world.get_component::<Transform>(entity).unwrap(), pos);
        assert_eq!(*world.get_component::<RigidBody>(entity).unwrap(), rb);
        assert_eq!(*world.get_component::<MeshRenderer>(entity).unwrap(), mesh);
    }

    #[test]
    fn despawn_entity() {
        let mut world = World::new();
        let entity = world.spawn_with_1(Transform {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        });
        assert!(world.is_alive(entity));
        assert!(world.despawn(entity));
        assert!(!world.is_alive(entity));
        assert!(world.get_component::<Transform>(entity).is_none());
    }

    #[test]
    fn despawn_stale_entity_fails() {
        let mut world = World::new();
        let entity = world.spawn_with_1(Transform {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        });
        world.despawn(entity);
        assert!(!world.despawn(entity));
    }

    #[test]
    fn get_component_on_dead_entity_returns_none() {
        let mut world = World::new();
        let entity = world.spawn_with_1(Transform {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        });
        world.despawn(entity);
        assert!(world.get_component::<Transform>(entity).is_none());
    }

    #[test]
    fn get_missing_component_returns_none() {
        let mut world = World::new();
        let entity = world.spawn_with_1(Transform {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        });
        assert!(world.get_component::<RigidBody>(entity).is_none());
    }

    #[test]
    fn mutate_component() {
        let mut world = World::new();
        let entity = world.spawn_with_1(Transform {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        });
        {
            let pos = world.get_component_mut::<Transform>(entity).unwrap();
            pos.x = 99.0;
        }
        let pos = world.get_component::<Transform>(entity).unwrap();
        assert_eq!(pos.x, 99.0);
    }

    #[test]
    fn add_component_to_entity() {
        let mut world = World::new();
        let entity = world.spawn_with_1(Transform {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        });
        assert!(world.get_component::<RigidBody>(entity).is_none());

        assert!(world.add_component(entity, RigidBody { mass: 5.0 }));

        let pos = world.get_component::<Transform>(entity).unwrap();
        assert_eq!(pos.x, 1.0);
        let rb = world.get_component::<RigidBody>(entity).unwrap();
        assert_eq!(rb.mass, 5.0);
    }

    #[test]
    fn add_duplicate_component_fails() {
        let mut world = World::new();
        let entity = world.spawn_with_1(Transform {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        });
        assert!(!world.add_component(
            entity,
            Transform {
                x: 9.0,
                y: 9.0,
                z: 9.0
            }
        ));
    }

    #[test]
    fn remove_component_from_entity() {
        let mut world = World::new();
        let entity = world.spawn_with_2(
            Transform {
                x: 1.0,
                y: 2.0,
                z: 3.0,
            },
            RigidBody { mass: 5.0 },
        );

        assert!(world.remove_component::<RigidBody>(entity));
        assert!(world.get_component::<RigidBody>(entity).is_none());

        // Transform should still be there
        let pos = world.get_component::<Transform>(entity).unwrap();
        assert_eq!(pos.x, 1.0);
    }

    #[test]
    fn remove_missing_component_fails() {
        let mut world = World::new();
        let entity = world.spawn_with_1(Transform {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        });
        assert!(!world.remove_component::<RigidBody>(entity));
    }

    #[test]
    fn entity_count_tracks_alive() {
        let mut world = World::new();
        assert_eq!(world.entity_count(), 0);
        let e1 = world.spawn_with_1(Transform {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        });
        let _e2 = world.spawn_with_1(Transform {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        });
        assert_eq!(world.entity_count(), 2);
        world.despawn(e1);
        assert_eq!(world.entity_count(), 1);
    }

    #[test]
    fn entities_with_same_components_share_archetype() {
        let mut world = World::new();
        let _e1 = world.spawn_with_2(
            Transform {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            RigidBody { mass: 1.0 },
        );
        let _e2 = world.spawn_with_2(
            Transform {
                x: 1.0,
                y: 1.0,
                z: 1.0,
            },
            RigidBody { mass: 2.0 },
        );
        // Both should be in the same archetype
        assert_eq!(world.archetype_storage().archetype_count(), 1);
    }

    #[test]
    fn different_component_sets_create_different_archetypes() {
        let mut world = World::new();
        let _e1 = world.spawn_with_1(Transform {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        });
        let _e2 = world.spawn_with_2(
            Transform {
                x: 1.0,
                y: 1.0,
                z: 1.0,
            },
            RigidBody { mass: 2.0 },
        );
        assert_eq!(world.archetype_storage().archetype_count(), 2);
    }

    #[test]
    fn archetype_migration_preserves_other_entities() {
        let mut world = World::new();
        let e1 = world.spawn_with_1(Transform {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        });
        let e2 = world.spawn_with_1(Transform {
            x: 2.0,
            y: 0.0,
            z: 0.0,
        });
        let e3 = world.spawn_with_1(Transform {
            x: 3.0,
            y: 0.0,
            z: 0.0,
        });

        // Migrate e1 to a new archetype
        world.add_component(e1, RigidBody { mass: 10.0 });

        // e2 and e3 should still be accessible
        assert_eq!(world.get_component::<Transform>(e2).unwrap().x, 2.0);
        assert_eq!(world.get_component::<Transform>(e3).unwrap().x, 3.0);
        // e1 should have both components
        assert_eq!(world.get_component::<Transform>(e1).unwrap().x, 1.0);
        assert_eq!(world.get_component::<RigidBody>(e1).unwrap().mass, 10.0);
    }

    #[test]
    fn query_world() {
        let mut world = World::new();
        world.register_component::<Transform>();
        world.register_component::<RigidBody>();

        let _e1 = world.spawn_with_2(
            Transform {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
            RigidBody { mass: 1.0 },
        );
        let _e2 = world.spawn_with_1(Transform {
            x: 2.0,
            y: 0.0,
            z: 0.0,
        });

        let access = AccessDescriptor::new()
            .read(ComponentId::of::<Transform>())
            .read(ComponentId::of::<RigidBody>());
        let result = world.query(&access);
        assert_eq!(result.entity_count(), 1);
    }

    #[test]
    fn network_aware_components() {
        let mut world = World::new();
        let transform_id = world.register_component::<Transform>();
        let rb_id = world.register_component::<RigidBody>();
        let mesh_id = world.register_component::<MeshRenderer>();
        let net_id = world.register_component::<NetworkIdentity>();

        let all_ids = vec![transform_id, rb_id, mesh_id, net_id];

        let replicated = world.replicated_components(&all_ids);
        assert_eq!(replicated.len(), 3); // Transform, MeshRenderer, NetworkIdentity
        assert!(replicated.contains(&transform_id));
        assert!(replicated.contains(&mesh_id));
        assert!(replicated.contains(&net_id));

        let server_only = world.server_only_components(&all_ids);
        assert_eq!(server_only.len(), 1); // RigidBody
        assert!(server_only.contains(&rb_id));
    }

    #[test]
    fn spawn_entity_with_network_identity() {
        let mut world = World::new();
        let entity = world.spawn_with_2(
            Transform {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            NetworkIdentity {
                net_id: 42,
                authority: Authority::Server,
            },
        );

        let ni = world.get_component::<NetworkIdentity>(entity).unwrap();
        assert_eq!(ni.net_id, 42);
        assert_eq!(ni.authority, Authority::Server);
    }

    #[test]
    fn despawn_with_swap_updates_locations() {
        let mut world = World::new();
        let e0 = world.spawn_with_1(Transform {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        });
        let e1 = world.spawn_with_1(Transform {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        });
        let e2 = world.spawn_with_1(Transform {
            x: 2.0,
            y: 0.0,
            z: 0.0,
        });

        // Despawn e0 (e2 should be swap-removed into its slot)
        world.despawn(e0);

        // e1 and e2 should still be accessible with correct data
        assert_eq!(world.get_component::<Transform>(e1).unwrap().x, 1.0);
        assert_eq!(world.get_component::<Transform>(e2).unwrap().x, 2.0);
    }

    #[test]
    fn add_component_to_empty_entity() {
        let mut world = World::new();
        let entity = world.spawn_empty();
        assert!(world.add_component(
            entity,
            Transform {
                x: 5.0,
                y: 6.0,
                z: 7.0
            }
        ));
        let pos = world.get_component::<Transform>(entity).unwrap();
        assert_eq!(pos.x, 5.0);
    }

    #[test]
    fn remove_last_component() {
        let mut world = World::new();
        let entity = world.spawn_with_1(Transform {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        });
        assert!(world.remove_component::<Transform>(entity));
        assert!(world.get_component::<Transform>(entity).is_none());
        // Entity should still be alive
        assert!(world.is_alive(entity));
    }

    // -- Drop correctness tests --

    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[derive(Clone)]
    struct DropCounter(Arc<AtomicUsize>);
    impl Drop for DropCounter {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    impl Component for DropCounter {}

    #[test]
    fn despawn_drops_components() {
        let counter = Arc::new(AtomicUsize::new(0));
        let mut world = World::new();
        let entity = world.spawn_with_1(DropCounter(counter.clone()));
        assert_eq!(counter.load(Ordering::SeqCst), 0);
        world.despawn(entity);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn despawn_last_entity_drops() {
        let counter = Arc::new(AtomicUsize::new(0));
        let mut world = World::new();
        // Only one entity — it IS the last row, so swap_remove has row == last
        let entity = world.spawn_with_1(DropCounter(counter.clone()));
        world.despawn(entity);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn despawn_multiple_entities_drops_all() {
        let counter = Arc::new(AtomicUsize::new(0));
        let mut world = World::new();
        let e0 = world.spawn_with_1(DropCounter(counter.clone()));
        let e1 = world.spawn_with_1(DropCounter(counter.clone()));
        let e2 = world.spawn_with_1(DropCounter(counter.clone()));
        world.despawn(e0);
        world.despawn(e1);
        world.despawn(e2);
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn add_component_migration_no_double_free() {
        let counter = Arc::new(AtomicUsize::new(0));
        let mut world = World::new();
        // Spawn with DropCounter, then migrate by adding Transform
        let entity = world.spawn_with_1(DropCounter(counter.clone()));
        // Migration should NOT drop the DropCounter (it's being moved)
        world.add_component(
            entity,
            Transform {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
        );
        assert_eq!(counter.load(Ordering::SeqCst), 0);
        // Despawn should drop exactly once
        world.despawn(entity);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn add_component_migration_last_entity_no_double_free() {
        let counter = Arc::new(AtomicUsize::new(0));
        let mut world = World::new();
        // Only entity in archetype — row == last during swap_remove
        let entity = world.spawn_with_1(DropCounter(counter.clone()));
        world.add_component(
            entity,
            Transform {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
        );
        assert_eq!(counter.load(Ordering::SeqCst), 0);
        world.despawn(entity);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn remove_component_drops_removed() {
        let counter = Arc::new(AtomicUsize::new(0));
        let mut world = World::new();
        let entity = world.spawn_with_2(
            DropCounter(counter.clone()),
            Transform {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
        );
        // Removing DropCounter should drop it
        world.remove_component::<DropCounter>(entity);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
        // Transform should still be accessible
        assert_eq!(world.get_component::<Transform>(entity).unwrap().x, 1.0);
    }

    #[test]
    fn remove_last_component_drops() {
        let counter = Arc::new(AtomicUsize::new(0));
        let mut world = World::new();
        let entity = world.spawn_with_1(DropCounter(counter.clone()));
        world.remove_component::<DropCounter>(entity);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn add_component_to_dead_entity_fails() {
        let mut world = World::new();
        let entity = world.spawn_with_1(Transform {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        });
        world.despawn(entity);
        assert!(!world.add_component(entity, RigidBody { mass: 1.0 }));
    }

    #[test]
    fn remove_component_from_dead_entity_fails() {
        let mut world = World::new();
        let entity = world.spawn_with_1(Transform {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        });
        world.despawn(entity);
        assert!(!world.remove_component::<Transform>(entity));
    }

    #[test]
    fn sequential_migrations() {
        let mut world = World::new();
        let entity = world.spawn_with_1(Transform {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        });
        world.add_component(entity, RigidBody { mass: 5.0 });
        world.add_component(entity, MeshRenderer { mesh_id: 42 });
        world.remove_component::<Transform>(entity);

        // Should have RigidBody + MeshRenderer, not Transform
        assert!(world.get_component::<Transform>(entity).is_none());
        assert_eq!(world.get_component::<RigidBody>(entity).unwrap().mass, 5.0);
        assert_eq!(
            world.get_component::<MeshRenderer>(entity).unwrap().mesh_id,
            42
        );
    }

    #[test]
    fn despawn_middle_entity() {
        let mut world = World::new();
        let e0 = world.spawn_with_1(Transform {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        });
        let e1 = world.spawn_with_1(Transform {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        });
        let e2 = world.spawn_with_1(Transform {
            x: 2.0,
            y: 0.0,
            z: 0.0,
        });
        world.despawn(e1);
        assert_eq!(world.get_component::<Transform>(e0).unwrap().x, 0.0);
        assert_eq!(world.get_component::<Transform>(e2).unwrap().x, 2.0);
        assert!(!world.is_alive(e1));
    }

    // -- ZST (zero-sized type) component tests --

    struct Marker;
    impl Component for Marker {}

    #[test]
    fn zst_component_spawn_and_query() {
        let mut world = World::new();
        let entity = world.spawn_with_2(
            Marker,
            Transform {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
        );
        // Can check the component exists
        assert!(world.get_component::<Marker>(entity).is_some());
        assert_eq!(world.get_component::<Transform>(entity).unwrap().x, 1.0);
    }

    #[test]
    fn zst_add_and_remove() {
        let mut world = World::new();
        let entity = world.spawn_with_1(Transform {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        });
        world.add_component(entity, Marker);
        assert!(world.get_component::<Marker>(entity).is_some());
        world.remove_component::<Marker>(entity);
        assert!(world.get_component::<Marker>(entity).is_none());
        assert_eq!(world.get_component::<Transform>(entity).unwrap().x, 1.0);
    }

    // -- Scale test (>64 entities to exercise Column::grow) --

    #[test]
    fn many_entities_exercises_column_grow() {
        let mut world = World::new();
        let mut entities = Vec::new();
        for i in 0..200u32 {
            let e = world.spawn_with_1(Transform {
                x: i as f32,
                y: 0.0,
                z: 0.0,
            });
            entities.push(e);
        }
        assert_eq!(world.entity_count(), 200);
        for (i, &e) in entities.iter().enumerate() {
            assert_eq!(world.get_component::<Transform>(e).unwrap().x, i as f32);
        }
        // Despawn half
        for &e in entities.iter().take(100) {
            world.despawn(e);
        }
        assert_eq!(world.entity_count(), 100);
        // Remaining should still be valid
        for &e in entities.iter().skip(100) {
            assert!(world.get_component::<Transform>(e).is_some());
        }
    }

    #[test]
    fn query_after_despawn_excludes_removed() {
        let mut world = World::new();
        world.register_component::<Transform>();
        world.register_component::<RigidBody>();

        let e1 = world.spawn_with_2(
            Transform {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
            RigidBody { mass: 1.0 },
        );
        let _e2 = world.spawn_with_2(
            Transform {
                x: 2.0,
                y: 0.0,
                z: 0.0,
            },
            RigidBody { mass: 2.0 },
        );

        let access = AccessDescriptor::new()
            .read(ComponentId::of::<Transform>())
            .read(ComponentId::of::<RigidBody>());

        assert_eq!(world.query(&access).entity_count(), 2);
        world.despawn(e1);
        assert_eq!(world.query(&access).entity_count(), 1);
    }

    #[test]
    fn run_systems_metrics_and_diagnostics_flow_through_world() {
        use crate::system::SystemBuilder;

        let mut world = World::new();
        world.add_system(
            SystemBuilder::new("input", |_: &World| {})
                .stage(Stage::Input)
                .build(),
        );
        world.add_system(
            SystemBuilder::new("physics", |_: &World| {})
                .stage(Stage::Physics)
                .build(),
        );

        let metrics = world.run_systems_with_metrics();
        assert_eq!(metrics.run_count, 1);
        assert_eq!(metrics.system_timings.len(), 2);

        let diagnostics = world.diagnostics();
        assert_eq!(diagnostics.run_count, 1);
        assert!(diagnostics.last_batch_count_total >= 1);
        assert_eq!(diagnostics.last_run_time_ns, metrics.total_time_ns);
    }
}
