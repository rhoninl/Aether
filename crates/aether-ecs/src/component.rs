use std::any::TypeId;
use std::collections::HashMap;

/// Unique identifier for a component type, derived from TypeId.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ComponentId(pub(crate) u64);

impl ComponentId {
    pub fn of<T: Component>() -> Self {
        // Use a deterministic hash of TypeId
        let type_id = TypeId::of::<T>();
        // We just use the bits from the TypeId hash
        let hash = fxhash_type_id(type_id);
        ComponentId(hash)
    }
}

fn fxhash_type_id(type_id: TypeId) -> u64 {
    // Safe: TypeId is a unique opaque identifier, we just need a u64
    // This is the same approach used by many ECS implementations
    use std::hash::{Hash, Hasher};
    let mut hasher = FxHasher::default();
    type_id.hash(&mut hasher);
    hasher.finish()
}

/// Minimal FxHash implementation for deterministic TypeId hashing.
#[derive(Default)]
struct FxHasher {
    hash: u64,
}

const SEED: u64 = 0x517cc1b727220a95;

impl std::hash::Hasher for FxHasher {
    fn finish(&self) -> u64 {
        self.hash
    }

    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.hash = (self.hash.rotate_left(5) ^ byte as u64).wrapping_mul(SEED);
        }
    }
}

/// Controls whether a component is replicated to clients or server-only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplicationMode {
    /// Component data is synced to clients.
    Replicated,
    /// Component data stays on the server only.
    ServerOnly,
}

/// Trait that all ECS components must implement.
///
/// Components are pure data types with no behavior. They must be `'static + Send + Sync`
/// to support parallel system execution.
pub trait Component: 'static + Send + Sync {
    fn replication_mode() -> ReplicationMode {
        ReplicationMode::ServerOnly
    }
}

/// Runtime metadata about a component type, stored in the registry.
#[derive(Clone, Debug)]
pub struct ComponentInfo {
    pub id: ComponentId,
    pub name: &'static str,
    pub size: usize,
    pub align: usize,
    pub drop_fn: Option<unsafe fn(*mut u8)>,
    pub replication_mode: ReplicationMode,
}

/// Registry that maps component types to their runtime metadata.
pub struct ComponentRegistry {
    infos: HashMap<ComponentId, ComponentInfo>,
}

impl ComponentRegistry {
    pub fn new() -> Self {
        Self {
            infos: HashMap::new(),
        }
    }

    pub fn register<T: Component>(&mut self) -> ComponentId {
        let id = ComponentId::of::<T>();
        self.infos.entry(id).or_insert_with(|| {
            let drop_fn: Option<unsafe fn(*mut u8)> = if std::mem::needs_drop::<T>() {
                Some(|ptr: *mut u8| unsafe {
                    std::ptr::drop_in_place(ptr as *mut T);
                })
            } else {
                None
            };
            ComponentInfo {
                id,
                name: std::any::type_name::<T>(),
                size: std::mem::size_of::<T>(),
                align: std::mem::align_of::<T>(),
                drop_fn,
                replication_mode: T::replication_mode(),
            }
        });
        id
    }

    pub fn get(&self, id: ComponentId) -> Option<&ComponentInfo> {
        self.infos.get(&id)
    }

    pub fn contains(&self, id: ComponentId) -> bool {
        self.infos.contains_key(&id)
    }
}

impl Default for ComponentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    struct Velocity {
        x: f32,
        y: f32,
        z: f32,
    }
    impl Component for Velocity {}

    struct Health(u32);
    impl Component for Health {
        fn replication_mode() -> ReplicationMode {
            ReplicationMode::Replicated
        }
    }

    #[test]
    fn component_id_is_deterministic() {
        let id1 = ComponentId::of::<Position>();
        let id2 = ComponentId::of::<Position>();
        assert_eq!(id1, id2);
    }

    #[test]
    fn different_types_get_different_ids() {
        let id1 = ComponentId::of::<Position>();
        let id2 = ComponentId::of::<Velocity>();
        assert_ne!(id1, id2);
    }

    #[test]
    fn registry_register_and_lookup() {
        let mut registry = ComponentRegistry::new();
        let id = registry.register::<Position>();
        assert!(registry.contains(id));
        let info = registry.get(id).unwrap();
        assert_eq!(info.size, std::mem::size_of::<Position>());
        assert_eq!(info.replication_mode, ReplicationMode::Replicated);
    }

    #[test]
    fn registry_default_replication_is_server_only() {
        let mut registry = ComponentRegistry::new();
        let id = registry.register::<Velocity>();
        let info = registry.get(id).unwrap();
        assert_eq!(info.replication_mode, ReplicationMode::ServerOnly);
    }

    #[test]
    fn registry_duplicate_register_is_idempotent() {
        let mut registry = ComponentRegistry::new();
        let id1 = registry.register::<Position>();
        let id2 = registry.register::<Position>();
        assert_eq!(id1, id2);
    }

    #[test]
    fn registry_unknown_id_returns_none() {
        let registry = ComponentRegistry::new();
        let id = ComponentId::of::<Position>();
        assert!(registry.get(id).is_none());
        assert!(!registry.contains(id));
    }

    #[test]
    fn component_info_has_correct_alignment() {
        let mut registry = ComponentRegistry::new();
        registry.register::<Position>();
        let info = registry.get(ComponentId::of::<Position>()).unwrap();
        assert_eq!(info.align, std::mem::align_of::<Position>());
    }

    #[test]
    fn drop_fn_set_for_types_with_drop() {
        let mut registry = ComponentRegistry::new();
        // String has a Drop impl
        struct WithDrop(String);
        impl Component for WithDrop {}
        registry.register::<WithDrop>();
        let info = registry.get(ComponentId::of::<WithDrop>()).unwrap();
        assert!(info.drop_fn.is_some());
    }

    #[test]
    fn no_drop_fn_for_copy_types() {
        let mut registry = ComponentRegistry::new();
        registry.register::<Health>();
        let info = registry.get(ComponentId::of::<Health>()).unwrap();
        assert!(info.drop_fn.is_none());
    }
}
