//! Ghost entity management for cross-zone boundary rendering.
//!
//! Ghost entities are lightweight proxies of entities near zone boundaries,
//! allowing adjacent zones to render and optionally collide with them.

use crate::authority::NetworkIdentity;
use crate::partition::KdBoundary;

/// Default margin (in world units) around zone boundaries where ghosts are created.
const DEFAULT_BOUNDARY_MARGIN: f32 = 10.0;

#[derive(Debug, Clone)]
pub struct GhostEntity {
    pub source_entity: u64,
    pub local_entity: u64,
    pub source_zone: String,
    pub remote_zone: String,
    pub ttl_ms: u64,
    pub collision_enabled: bool,
    pub render_only: bool,
}

#[derive(Debug, Clone)]
pub enum GhostVisibilityScope {
    Always,
    DistanceCapped { max_distance_m: f32 },
}

#[derive(Debug)]
pub struct GhostPolicy {
    pub ttl_ms: u64,
    pub max_ghosts_per_connection: usize,
    pub visibility: GhostVisibilityScope,
}

#[derive(Debug, Default)]
pub struct GhostCache {
    ghosts: Vec<GhostEntity>,
}

impl GhostCache {
    pub fn new(policy: GhostPolicy) -> Self {
        let _ = policy.ttl_ms;
        let _ = policy.max_ghosts_per_connection;
        Self::default()
    }

    pub fn add(&mut self, ghost: GhostEntity) {
        self.ghosts.push(ghost);
    }

    pub fn remove_by_local_entity(&mut self, local_entity: u64) -> Option<GhostEntity> {
        let idx = self
            .ghosts
            .iter()
            .position(|entry| entry.local_entity == local_entity)?;
        Some(self.ghosts.swap_remove(idx))
    }

    pub fn cull_expired(&mut self, now_ms: u64) {
        self.ghosts
            .retain(|ghost| ghost.ttl_ms > now_ms.saturating_sub(ghost.ttl_ms));
    }

    pub fn as_identities(&self) -> Vec<NetworkIdentity> {
        self.ghosts
            .iter()
            .map(|ghost| NetworkIdentity::new(ghost.local_entity, ghost.remote_zone.clone()))
            .collect()
    }

    pub fn ghosts(&self) -> &[GhostEntity] {
        &self.ghosts
    }

    pub fn count(&self) -> usize {
        self.ghosts.len()
    }

    pub fn remove_by_source_entity(&mut self, source_entity: u64) -> Vec<GhostEntity> {
        let mut removed = Vec::new();
        let mut i = 0;
        while i < self.ghosts.len() {
            if self.ghosts[i].source_entity == source_entity {
                removed.push(self.ghosts.swap_remove(i));
            } else {
                i += 1;
            }
        }
        removed
    }
}

// ---------------------------------------------------------------------------
// GhostManager -- boundary-aware ghost lifecycle
// ---------------------------------------------------------------------------

/// Position in 3D space.
#[derive(Debug, Clone, Copy)]
pub struct Position {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// An entity near a zone boundary that may need ghosting.
#[derive(Debug, Clone)]
pub struct BoundaryEntity {
    pub entity_id: u64,
    pub zone_id: String,
    pub position: Position,
}

/// Adjacent zone information for ghost creation.
#[derive(Debug, Clone)]
pub struct AdjacentZone {
    pub zone_id: String,
    pub boundary: KdBoundary,
}

/// Manages ghost entity creation and deletion at zone boundaries.
#[derive(Debug)]
pub struct GhostManager {
    cache: GhostCache,
    boundary_margin: f32,
    ttl_ms: u64,
    next_local_id: u64,
}

impl GhostManager {
    pub fn new(ttl_ms: u64) -> Self {
        Self {
            cache: GhostCache::default(),
            boundary_margin: DEFAULT_BOUNDARY_MARGIN,
            ttl_ms,
            next_local_id: 1_000_000,
        }
    }

    pub fn with_boundary_margin(mut self, margin: f32) -> Self {
        self.boundary_margin = margin;
        self
    }

    pub fn cache(&self) -> &GhostCache {
        &self.cache
    }

    /// Check if a position is within the boundary margin of a zone edge.
    pub fn is_near_boundary(&self, pos: &Position, zone_bounds: &KdBoundary) -> bool {
        let margin = self.boundary_margin;
        (pos.x - zone_bounds.min.x).abs() < margin
            || (pos.x - zone_bounds.max.x).abs() < margin
            || (pos.y - zone_bounds.min.y).abs() < margin
            || (pos.y - zone_bounds.max.y).abs() < margin
            || (pos.z - zone_bounds.min.z).abs() < margin
            || (pos.z - zone_bounds.max.z).abs() < margin
    }

    /// Create ghost entities on adjacent zones for an entity near a boundary.
    /// Returns the list of newly created ghost entities.
    pub fn create_ghosts_for_boundary(
        &mut self,
        entity: &BoundaryEntity,
        zone_bounds: &KdBoundary,
        adjacent_zones: &[AdjacentZone],
    ) -> Vec<GhostEntity> {
        if !self.is_near_boundary(&entity.position, zone_bounds) {
            return vec![];
        }

        let mut created = Vec::new();
        for adj in adjacent_zones {
            // Check if ghost already exists for this source/remote pair
            let already_exists = self
                .cache
                .ghosts()
                .iter()
                .any(|g| g.source_entity == entity.entity_id && g.remote_zone == adj.zone_id);

            if already_exists {
                continue;
            }

            let local_id = self.next_local_id;
            self.next_local_id += 1;

            let ghost = GhostEntity {
                source_entity: entity.entity_id,
                local_entity: local_id,
                source_zone: entity.zone_id.clone(),
                remote_zone: adj.zone_id.clone(),
                ttl_ms: self.ttl_ms,
                collision_enabled: false,
                render_only: true,
            };
            self.cache.add(ghost.clone());
            created.push(ghost);
        }
        created
    }

    /// Remove all ghosts for an entity that is no longer near any boundary.
    pub fn remove_ghosts_for_entity(&mut self, entity_id: u64) -> Vec<GhostEntity> {
        self.cache.remove_by_source_entity(entity_id)
    }

    /// Cull expired ghosts.
    pub fn cull_expired(&mut self, now_ms: u64) {
        self.cache.cull_expired(now_ms);
    }

    /// Count of active ghosts.
    pub fn ghost_count(&self) -> usize {
        self.cache.count()
    }
}

impl Default for GhostManager {
    fn default() -> Self {
        Self::new(1200)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::partition::KdPoint;

    fn make_zone_bounds() -> KdBoundary {
        KdBoundary {
            min: KdPoint {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            max: KdPoint {
                x: 100.0,
                y: 100.0,
                z: 100.0,
            },
        }
    }

    fn make_adjacent_zone(id: &str) -> AdjacentZone {
        AdjacentZone {
            zone_id: id.to_string(),
            boundary: KdBoundary {
                min: KdPoint {
                    x: 100.0,
                    y: 0.0,
                    z: 0.0,
                },
                max: KdPoint {
                    x: 200.0,
                    y: 100.0,
                    z: 100.0,
                },
            },
        }
    }

    // --- GhostCache tests ---

    #[test]
    fn cache_add_and_count() {
        let mut cache = GhostCache::default();
        assert_eq!(cache.count(), 0);

        cache.add(GhostEntity {
            source_entity: 1,
            local_entity: 100,
            source_zone: "a".to_string(),
            remote_zone: "b".to_string(),
            ttl_ms: 1000,
            collision_enabled: false,
            render_only: true,
        });
        assert_eq!(cache.count(), 1);
    }

    #[test]
    fn cache_remove_by_local_entity() {
        let mut cache = GhostCache::default();
        cache.add(GhostEntity {
            source_entity: 1,
            local_entity: 100,
            source_zone: "a".to_string(),
            remote_zone: "b".to_string(),
            ttl_ms: 1000,
            collision_enabled: false,
            render_only: true,
        });

        let removed = cache.remove_by_local_entity(100);
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().source_entity, 1);
        assert_eq!(cache.count(), 0);
    }

    #[test]
    fn cache_remove_by_source_entity() {
        let mut cache = GhostCache::default();
        cache.add(GhostEntity {
            source_entity: 1,
            local_entity: 100,
            source_zone: "a".to_string(),
            remote_zone: "b".to_string(),
            ttl_ms: 1000,
            collision_enabled: false,
            render_only: true,
        });
        cache.add(GhostEntity {
            source_entity: 1,
            local_entity: 101,
            source_zone: "a".to_string(),
            remote_zone: "c".to_string(),
            ttl_ms: 1000,
            collision_enabled: false,
            render_only: true,
        });
        cache.add(GhostEntity {
            source_entity: 2,
            local_entity: 200,
            source_zone: "a".to_string(),
            remote_zone: "b".to_string(),
            ttl_ms: 1000,
            collision_enabled: false,
            render_only: true,
        });

        let removed = cache.remove_by_source_entity(1);
        assert_eq!(removed.len(), 2);
        assert_eq!(cache.count(), 1);
        assert_eq!(cache.ghosts()[0].source_entity, 2);
    }

    // --- GhostManager tests ---

    #[test]
    fn near_boundary_detection() {
        let mgr = GhostManager::new(1000).with_boundary_margin(10.0);
        let bounds = make_zone_bounds();

        // Near the min-x edge
        let pos = Position {
            x: 5.0,
            y: 50.0,
            z: 50.0,
        };
        assert!(mgr.is_near_boundary(&pos, &bounds));

        // Near the max-x edge
        let pos = Position {
            x: 95.0,
            y: 50.0,
            z: 50.0,
        };
        assert!(mgr.is_near_boundary(&pos, &bounds));

        // Center of zone -- not near boundary
        let pos = Position {
            x: 50.0,
            y: 50.0,
            z: 50.0,
        };
        assert!(!mgr.is_near_boundary(&pos, &bounds));
    }

    #[test]
    fn create_ghosts_when_near_boundary() {
        let mut mgr = GhostManager::new(1000).with_boundary_margin(10.0);
        let bounds = make_zone_bounds();
        let adjacent = vec![make_adjacent_zone("zone-b")];

        let entity = BoundaryEntity {
            entity_id: 1,
            zone_id: "zone-a".to_string(),
            position: Position {
                x: 95.0,
                y: 50.0,
                z: 50.0,
            },
        };

        let created = mgr.create_ghosts_for_boundary(&entity, &bounds, &adjacent);
        assert_eq!(created.len(), 1);
        assert_eq!(created[0].source_entity, 1);
        assert_eq!(created[0].remote_zone, "zone-b");
        assert_eq!(mgr.ghost_count(), 1);
    }

    #[test]
    fn no_ghost_when_not_near_boundary() {
        let mut mgr = GhostManager::new(1000).with_boundary_margin(10.0);
        let bounds = make_zone_bounds();
        let adjacent = vec![make_adjacent_zone("zone-b")];

        let entity = BoundaryEntity {
            entity_id: 1,
            zone_id: "zone-a".to_string(),
            position: Position {
                x: 50.0,
                y: 50.0,
                z: 50.0,
            },
        };

        let created = mgr.create_ghosts_for_boundary(&entity, &bounds, &adjacent);
        assert!(created.is_empty());
        assert_eq!(mgr.ghost_count(), 0);
    }

    #[test]
    fn no_duplicate_ghosts() {
        let mut mgr = GhostManager::new(1000).with_boundary_margin(10.0);
        let bounds = make_zone_bounds();
        let adjacent = vec![make_adjacent_zone("zone-b")];

        let entity = BoundaryEntity {
            entity_id: 1,
            zone_id: "zone-a".to_string(),
            position: Position {
                x: 95.0,
                y: 50.0,
                z: 50.0,
            },
        };

        mgr.create_ghosts_for_boundary(&entity, &bounds, &adjacent);
        let created = mgr.create_ghosts_for_boundary(&entity, &bounds, &adjacent);
        assert!(created.is_empty());
        assert_eq!(mgr.ghost_count(), 1);
    }

    #[test]
    fn remove_ghosts_for_entity() {
        let mut mgr = GhostManager::new(1000).with_boundary_margin(10.0);
        let bounds = make_zone_bounds();
        let adjacent = vec![
            make_adjacent_zone("zone-b"),
            make_adjacent_zone("zone-c"),
        ];

        let entity = BoundaryEntity {
            entity_id: 1,
            zone_id: "zone-a".to_string(),
            position: Position {
                x: 95.0,
                y: 50.0,
                z: 50.0,
            },
        };

        mgr.create_ghosts_for_boundary(&entity, &bounds, &adjacent);
        assert_eq!(mgr.ghost_count(), 2);

        let removed = mgr.remove_ghosts_for_entity(1);
        assert_eq!(removed.len(), 2);
        assert_eq!(mgr.ghost_count(), 0);
    }

    #[test]
    fn ghosts_for_multiple_adjacent_zones() {
        let mut mgr = GhostManager::new(1000).with_boundary_margin(10.0);
        let bounds = make_zone_bounds();
        let adjacent = vec![
            make_adjacent_zone("zone-b"),
            make_adjacent_zone("zone-c"),
            make_adjacent_zone("zone-d"),
        ];

        let entity = BoundaryEntity {
            entity_id: 1,
            zone_id: "zone-a".to_string(),
            position: Position {
                x: 95.0,
                y: 50.0,
                z: 50.0,
            },
        };

        let created = mgr.create_ghosts_for_boundary(&entity, &bounds, &adjacent);
        assert_eq!(created.len(), 3);
        assert_eq!(mgr.ghost_count(), 3);
    }

    #[test]
    fn near_boundary_all_edges() {
        let mgr = GhostManager::new(1000).with_boundary_margin(10.0);
        let bounds = make_zone_bounds();

        // Near min-y
        assert!(mgr.is_near_boundary(
            &Position { x: 50.0, y: 5.0, z: 50.0 },
            &bounds
        ));
        // Near max-z
        assert!(mgr.is_near_boundary(
            &Position { x: 50.0, y: 50.0, z: 95.0 },
            &bounds
        ));
        // Near min-z
        assert!(mgr.is_near_boundary(
            &Position { x: 50.0, y: 50.0, z: 5.0 },
            &bounds
        ));
    }
}
