use aether_ecs::Entity;

/// A physics collision event translated from Rapier collider handles to ECS entities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicsCollisionEvent {
    /// Two entities started colliding.
    Started {
        entity1: Entity,
        entity2: Entity,
        is_sensor: bool,
    },
    /// Two entities stopped colliding.
    Stopped {
        entity1: Entity,
        entity2: Entity,
        is_sensor: bool,
    },
}

impl PhysicsCollisionEvent {
    /// Returns the two entities involved in this collision event.
    pub fn entities(&self) -> (Entity, Entity) {
        match *self {
            PhysicsCollisionEvent::Started {
                entity1, entity2, ..
            } => (entity1, entity2),
            PhysicsCollisionEvent::Stopped {
                entity1, entity2, ..
            } => (entity1, entity2),
        }
    }

    /// Returns true if this is a sensor (trigger) collision.
    pub fn is_sensor(&self) -> bool {
        match *self {
            PhysicsCollisionEvent::Started { is_sensor, .. } => is_sensor,
            PhysicsCollisionEvent::Stopped { is_sensor, .. } => is_sensor,
        }
    }

    /// Returns true if this is a Started event.
    pub fn is_started(&self) -> bool {
        matches!(self, PhysicsCollisionEvent::Started { .. })
    }

    /// Returns true if this is a Stopped event.
    pub fn is_stopped(&self) -> bool {
        matches!(self, PhysicsCollisionEvent::Stopped { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(index: u32) -> Entity {
        unsafe { std::mem::transmute::<(u32, u32), Entity>((index, 0)) }
    }

    #[test]
    fn started_event_accessors() {
        let event = PhysicsCollisionEvent::Started {
            entity1: entity(1),
            entity2: entity(2),
            is_sensor: false,
        };
        assert!(event.is_started());
        assert!(!event.is_stopped());
        assert!(!event.is_sensor());
        let (e1, e2) = event.entities();
        assert_eq!(e1.index(), 1);
        assert_eq!(e2.index(), 2);
    }

    #[test]
    fn stopped_event_accessors() {
        let event = PhysicsCollisionEvent::Stopped {
            entity1: entity(3),
            entity2: entity(4),
            is_sensor: true,
        };
        assert!(!event.is_started());
        assert!(event.is_stopped());
        assert!(event.is_sensor());
        let (e1, e2) = event.entities();
        assert_eq!(e1.index(), 3);
        assert_eq!(e2.index(), 4);
    }

    #[test]
    fn sensor_started_event() {
        let event = PhysicsCollisionEvent::Started {
            entity1: entity(0),
            entity2: entity(1),
            is_sensor: true,
        };
        assert!(event.is_sensor());
        assert!(event.is_started());
    }

    #[test]
    fn event_equality() {
        let a = PhysicsCollisionEvent::Started {
            entity1: entity(1),
            entity2: entity(2),
            is_sensor: false,
        };
        let b = PhysicsCollisionEvent::Started {
            entity1: entity(1),
            entity2: entity(2),
            is_sensor: false,
        };
        assert_eq!(a, b);
    }

    #[test]
    fn different_events_not_equal() {
        let started = PhysicsCollisionEvent::Started {
            entity1: entity(1),
            entity2: entity(2),
            is_sensor: false,
        };
        let stopped = PhysicsCollisionEvent::Stopped {
            entity1: entity(1),
            entity2: entity(2),
            is_sensor: false,
        };
        assert_ne!(started, stopped);
    }
}
