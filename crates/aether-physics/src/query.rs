use aether_ecs::Entity;

/// Result of a raycast against the physics world.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RaycastHit {
    /// The entity that was hit.
    pub entity: Entity,
    /// World-space point of intersection.
    pub point: [f32; 3],
    /// Surface normal at the hit point.
    pub normal: [f32; 3],
    /// Distance from ray origin to the hit point.
    pub distance: f32,
}

/// Filter options for physics queries.
#[derive(Debug, Clone, Default)]
pub struct QueryFilter {
    /// If set, exclude this entity from results.
    pub exclude_entity: Option<Entity>,
    /// If true, include sensor colliders in results.
    pub include_sensors: bool,
}

impl QueryFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn excluding(mut self, entity: Entity) -> Self {
        self.exclude_entity = Some(entity);
        self
    }

    pub fn with_sensors(mut self) -> Self {
        self.include_sensors = true;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(index: u32) -> Entity {
        unsafe { std::mem::transmute::<(u32, u32), Entity>((index, 0)) }
    }

    #[test]
    fn raycast_hit_fields() {
        let hit = RaycastHit {
            entity: entity(42),
            point: [1.0, 2.0, 3.0],
            normal: [0.0, 1.0, 0.0],
            distance: 5.0,
        };
        assert_eq!(hit.entity.index(), 42);
        assert_eq!(hit.point, [1.0, 2.0, 3.0]);
        assert_eq!(hit.normal, [0.0, 1.0, 0.0]);
        assert_eq!(hit.distance, 5.0);
    }

    #[test]
    fn query_filter_default() {
        let filter = QueryFilter::default();
        assert!(filter.exclude_entity.is_none());
        assert!(!filter.include_sensors);
    }

    #[test]
    fn query_filter_builder() {
        let e = entity(5);
        let filter = QueryFilter::new().excluding(e).with_sensors();
        assert_eq!(filter.exclude_entity.unwrap().index(), 5);
        assert!(filter.include_sensors);
    }

    #[test]
    fn raycast_hit_equality() {
        let a = RaycastHit {
            entity: entity(1),
            point: [0.0, 0.0, 0.0],
            normal: [0.0, 1.0, 0.0],
            distance: 1.0,
        };
        let b = RaycastHit {
            entity: entity(1),
            point: [0.0, 0.0, 0.0],
            normal: [0.0, 1.0, 0.0],
            distance: 1.0,
        };
        assert_eq!(a, b);
    }
}
