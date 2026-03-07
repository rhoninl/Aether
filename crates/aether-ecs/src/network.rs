use crate::component::{Component, ComponentId, ComponentRegistry, ReplicationMode};

/// Marks an entity as having a network identity for replication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NetworkIdentity {
    /// Unique network ID for this entity (assigned by server).
    pub net_id: u64,
    /// Whether this entity is owned by the server or a specific client.
    pub authority: Authority,
}

impl Component for NetworkIdentity {
    fn replication_mode() -> ReplicationMode {
        ReplicationMode::Replicated
    }
}

/// Who has authority over an entity's state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Authority {
    Server,
    Client(u64),
}

/// Queries the registry to find all component IDs that are marked as Replicated.
pub fn replicated_component_ids(
    registry: &ComponentRegistry,
    ids: &[ComponentId],
) -> Vec<ComponentId> {
    ids.iter()
        .filter(|&&id| {
            registry
                .get(id)
                .map(|info| info.replication_mode == ReplicationMode::Replicated)
                .unwrap_or(false)
        })
        .copied()
        .collect()
}

/// Queries the registry to find all component IDs that are marked as ServerOnly.
pub fn server_only_component_ids(
    registry: &ComponentRegistry,
    ids: &[ComponentId],
) -> Vec<ComponentId> {
    ids.iter()
        .filter(|&&id| {
            registry
                .get(id)
                .map(|info| info.replication_mode == ReplicationMode::ServerOnly)
                .unwrap_or(false)
        })
        .copied()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Copy)]
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

    #[derive(Debug, Clone, Copy)]
    struct RigidBody {
        mass: f32,
    }
    impl Component for RigidBody {
        fn replication_mode() -> ReplicationMode {
            ReplicationMode::ServerOnly
        }
    }

    #[derive(Debug, Clone, Copy)]
    struct MeshRenderer {
        mesh_id: u32,
    }
    impl Component for MeshRenderer {
        fn replication_mode() -> ReplicationMode {
            ReplicationMode::Replicated
        }
    }

    #[derive(Debug, Clone, Copy)]
    struct Collider {
        radius: f32,
    }
    impl Component for Collider {
        fn replication_mode() -> ReplicationMode {
            ReplicationMode::ServerOnly
        }
    }

    #[test]
    fn network_identity_is_replicated() {
        assert_eq!(
            NetworkIdentity::replication_mode(),
            ReplicationMode::Replicated
        );
    }

    #[test]
    fn authority_variants() {
        let server = Authority::Server;
        let client = Authority::Client(42);
        assert_eq!(server, Authority::Server);
        assert_eq!(client, Authority::Client(42));
        assert_ne!(server, client);
    }

    #[test]
    fn network_identity_creation() {
        let ni = NetworkIdentity {
            net_id: 100,
            authority: Authority::Server,
        };
        assert_eq!(ni.net_id, 100);
        assert_eq!(ni.authority, Authority::Server);
    }

    #[test]
    fn filter_replicated_components() {
        let mut registry = ComponentRegistry::new();
        let transform_id = registry.register::<Transform>();
        let rigidbody_id = registry.register::<RigidBody>();
        let mesh_id = registry.register::<MeshRenderer>();
        let collider_id = registry.register::<Collider>();
        let net_id = registry.register::<NetworkIdentity>();

        let all_ids = vec![transform_id, rigidbody_id, mesh_id, collider_id, net_id];

        let replicated = replicated_component_ids(&registry, &all_ids);
        assert_eq!(replicated.len(), 3);
        assert!(replicated.contains(&transform_id));
        assert!(replicated.contains(&mesh_id));
        assert!(replicated.contains(&net_id));
    }

    #[test]
    fn filter_server_only_components() {
        let mut registry = ComponentRegistry::new();
        let transform_id = registry.register::<Transform>();
        let rigidbody_id = registry.register::<RigidBody>();
        let collider_id = registry.register::<Collider>();

        let all_ids = vec![transform_id, rigidbody_id, collider_id];

        let server_only = server_only_component_ids(&registry, &all_ids);
        assert_eq!(server_only.len(), 2);
        assert!(server_only.contains(&rigidbody_id));
        assert!(server_only.contains(&collider_id));
    }

    #[test]
    fn empty_ids_returns_empty() {
        let registry = ComponentRegistry::new();
        let replicated = replicated_component_ids(&registry, &[]);
        assert!(replicated.is_empty());
    }

    #[test]
    fn unregistered_ids_are_filtered_out() {
        let registry = ComponentRegistry::new();
        let fake_id = ComponentId::of::<Transform>();
        let replicated = replicated_component_ids(&registry, &[fake_id]);
        assert!(replicated.is_empty());
    }
}
