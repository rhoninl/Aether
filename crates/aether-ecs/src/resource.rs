use std::any::{Any, TypeId};
use std::collections::HashMap;

/// A typed container for shared global state accessible by systems.
///
/// Resources are stored as `Box<dyn Any + Send + Sync>` keyed by `TypeId`.
/// Each type can have at most one resource instance.
pub struct Resources {
    data: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl Resources {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    /// Insert a resource, replacing any existing resource of the same type.
    /// Returns the previous value if one existed.
    pub fn insert<T: 'static + Send + Sync>(&mut self, value: T) -> Option<T> {
        let prev = self.data.insert(TypeId::of::<T>(), Box::new(value));
        prev.map(|boxed| {
            *boxed
                .downcast::<T>()
                .expect("type mismatch in resource map")
        })
    }

    /// Get an immutable reference to a resource.
    pub fn get<T: 'static + Send + Sync>(&self) -> Option<&T> {
        self.data
            .get(&TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast_ref::<T>())
    }

    /// Get a mutable reference to a resource.
    pub fn get_mut<T: 'static + Send + Sync>(&mut self) -> Option<&mut T> {
        self.data
            .get_mut(&TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast_mut::<T>())
    }

    /// Remove a resource and return it.
    pub fn remove<T: 'static + Send + Sync>(&mut self) -> Option<T> {
        self.data.remove(&TypeId::of::<T>()).map(|boxed| {
            *boxed
                .downcast::<T>()
                .expect("type mismatch in resource map")
        })
    }

    /// Check whether a resource of type T exists.
    pub fn contains<T: 'static + Send + Sync>(&self) -> bool {
        self.data.contains_key(&TypeId::of::<T>())
    }
}

impl Default for Resources {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct DeltaTime(f64);

    #[derive(Debug, PartialEq)]
    struct Gravity(f32);

    #[derive(Debug, PartialEq)]
    struct PlayerName(String);

    #[test]
    fn insert_and_get_resource() {
        let mut resources = Resources::new();
        resources.insert(DeltaTime(1.0 / 60.0));
        let dt = resources.get::<DeltaTime>().unwrap();
        assert_eq!(dt.0, 1.0 / 60.0);
    }

    #[test]
    fn get_returns_none_for_missing_resource() {
        let resources = Resources::new();
        assert!(resources.get::<DeltaTime>().is_none());
    }

    #[test]
    fn overwrite_existing_resource() {
        let mut resources = Resources::new();
        resources.insert(DeltaTime(1.0 / 60.0));
        let prev = resources.insert(DeltaTime(1.0 / 30.0));
        assert_eq!(prev, Some(DeltaTime(1.0 / 60.0)));
        assert_eq!(resources.get::<DeltaTime>().unwrap().0, 1.0 / 30.0);
    }

    #[test]
    fn remove_resource() {
        let mut resources = Resources::new();
        resources.insert(DeltaTime(1.0 / 60.0));
        let removed = resources.remove::<DeltaTime>();
        assert_eq!(removed, Some(DeltaTime(1.0 / 60.0)));
        assert!(resources.get::<DeltaTime>().is_none());
    }

    #[test]
    fn remove_missing_resource_returns_none() {
        let mut resources = Resources::new();
        assert!(resources.remove::<DeltaTime>().is_none());
    }

    #[test]
    fn multiple_distinct_resource_types() {
        let mut resources = Resources::new();
        resources.insert(DeltaTime(1.0 / 60.0));
        resources.insert(Gravity(9.81));
        assert_eq!(resources.get::<DeltaTime>().unwrap().0, 1.0 / 60.0);
        assert_eq!(resources.get::<Gravity>().unwrap().0, 9.81);
    }

    #[test]
    fn contains_check() {
        let mut resources = Resources::new();
        assert!(!resources.contains::<DeltaTime>());
        resources.insert(DeltaTime(0.016));
        assert!(resources.contains::<DeltaTime>());
        resources.remove::<DeltaTime>();
        assert!(!resources.contains::<DeltaTime>());
    }

    #[test]
    fn get_mut_allows_modification() {
        let mut resources = Resources::new();
        resources.insert(DeltaTime(1.0 / 60.0));
        {
            let dt = resources.get_mut::<DeltaTime>().unwrap();
            dt.0 = 1.0 / 30.0;
        }
        assert_eq!(resources.get::<DeltaTime>().unwrap().0, 1.0 / 30.0);
    }

    #[test]
    fn get_mut_returns_none_for_missing() {
        let mut resources = Resources::new();
        assert!(resources.get_mut::<DeltaTime>().is_none());
    }

    #[test]
    fn resource_with_heap_data() {
        let mut resources = Resources::new();
        resources.insert(PlayerName("Alice".to_string()));
        assert_eq!(
            resources.get::<PlayerName>().unwrap().0,
            "Alice".to_string()
        );
    }

    #[test]
    fn insert_returns_none_for_first_insert() {
        let mut resources = Resources::new();
        let prev = resources.insert(DeltaTime(0.016));
        assert!(prev.is_none());
    }
}
