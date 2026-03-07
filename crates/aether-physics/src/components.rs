use aether_ecs::component::{Component, ReplicationMode};

use crate::layers::CollisionLayers;

/// Type of rigid body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyType {
    /// Affected by forces and collisions.
    Dynamic,
    /// Moved by user code, not affected by forces but affects dynamic bodies.
    Kinematic,
    /// Immovable, infinite mass.
    Static,
}

/// ECS component wrapping a Rapier rigid body handle.
#[derive(Debug, Clone, Copy)]
pub struct RigidBodyComponent {
    pub body_type: BodyType,
    pub(crate) handle: Option<rapier3d::dynamics::RigidBodyHandle>,
    pub mass: f32,
    pub linear_damping: f32,
    pub angular_damping: f32,
    pub gravity_scale: f32,
    pub ccd_enabled: bool,
}

impl RigidBodyComponent {
    pub fn dynamic(mass: f32) -> Self {
        Self {
            body_type: BodyType::Dynamic,
            handle: None,
            mass,
            linear_damping: 0.0,
            angular_damping: 0.05,
            gravity_scale: 1.0,
            ccd_enabled: false,
        }
    }

    pub fn kinematic() -> Self {
        Self {
            body_type: BodyType::Kinematic,
            handle: None,
            mass: 0.0,
            linear_damping: 0.0,
            angular_damping: 0.0,
            gravity_scale: 0.0,
            ccd_enabled: false,
        }
    }

    pub fn fixed() -> Self {
        Self {
            body_type: BodyType::Static,
            handle: None,
            mass: 0.0,
            linear_damping: 0.0,
            angular_damping: 0.0,
            gravity_scale: 0.0,
            ccd_enabled: false,
        }
    }

    pub fn handle(&self) -> Option<rapier3d::dynamics::RigidBodyHandle> {
        self.handle
    }
}

impl Component for RigidBodyComponent {
    fn replication_mode() -> ReplicationMode {
        ReplicationMode::ServerOnly
    }
}

/// Shape type for colliders.
#[derive(Debug, Clone)]
pub enum ColliderShape {
    Sphere { radius: f32 },
    Box { half_extents: [f32; 3] },
    Capsule { half_height: f32, radius: f32 },
    Cylinder { half_height: f32, radius: f32 },
}

/// ECS component wrapping a Rapier collider handle.
#[derive(Debug, Clone)]
pub struct ColliderComponent {
    pub shape: ColliderShape,
    pub(crate) handle: Option<rapier3d::geometry::ColliderHandle>,
    pub is_sensor: bool,
    pub friction: f32,
    pub restitution: f32,
    pub layers: CollisionLayers,
    pub density: f32,
}

impl ColliderComponent {
    pub fn sphere(radius: f32) -> Self {
        Self {
            shape: ColliderShape::Sphere { radius },
            handle: None,
            is_sensor: false,
            friction: 0.5,
            restitution: 0.0,
            layers: CollisionLayers::default(),
            density: 1.0,
        }
    }

    pub fn cuboid(half_x: f32, half_y: f32, half_z: f32) -> Self {
        Self {
            shape: ColliderShape::Box {
                half_extents: [half_x, half_y, half_z],
            },
            handle: None,
            is_sensor: false,
            friction: 0.5,
            restitution: 0.0,
            layers: CollisionLayers::default(),
            density: 1.0,
        }
    }

    pub fn capsule(half_height: f32, radius: f32) -> Self {
        Self {
            shape: ColliderShape::Capsule {
                half_height,
                radius,
            },
            handle: None,
            is_sensor: false,
            friction: 0.5,
            restitution: 0.0,
            layers: CollisionLayers::default(),
            density: 1.0,
        }
    }

    pub fn sensor(mut self) -> Self {
        self.is_sensor = true;
        self
    }

    pub fn with_layers(mut self, layers: CollisionLayers) -> Self {
        self.layers = layers;
        self
    }

    pub fn with_friction(mut self, friction: f32) -> Self {
        self.friction = friction;
        self
    }

    pub fn with_restitution(mut self, restitution: f32) -> Self {
        self.restitution = restitution;
        self
    }

    pub fn handle(&self) -> Option<rapier3d::geometry::ColliderHandle> {
        self.handle
    }
}

impl Component for ColliderComponent {
    fn replication_mode() -> ReplicationMode {
        ReplicationMode::ServerOnly
    }
}

/// Linear and angular velocity, replicated to clients for interpolation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Velocity {
    pub linear: [f32; 3],
    pub angular: [f32; 3],
}

impl Default for Velocity {
    fn default() -> Self {
        Self {
            linear: [0.0; 3],
            angular: [0.0; 3],
        }
    }
}

impl Component for Velocity {
    fn replication_mode() -> ReplicationMode {
        ReplicationMode::Replicated
    }
}

/// Transform component (position + rotation) for physics entities.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub position: [f32; 3],
    pub rotation: [f32; 4], // quaternion (x, y, z, w)
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0], // identity quaternion
        }
    }
}

impl Transform {
    pub fn from_position(x: f32, y: f32, z: f32) -> Self {
        Self {
            position: [x, y, z],
            ..Default::default()
        }
    }
}

impl Component for Transform {
    fn replication_mode() -> ReplicationMode {
        ReplicationMode::Replicated
    }
}

/// Who has physics authority over this body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicsAuthority {
    Server,
    Client(u64),
}

impl Default for PhysicsAuthority {
    fn default() -> Self {
        Self::Server
    }
}

impl Component for PhysicsAuthority {
    fn replication_mode() -> ReplicationMode {
        ReplicationMode::ServerOnly
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rigid_body_dynamic() {
        let rb = RigidBodyComponent::dynamic(10.0);
        assert_eq!(rb.body_type, BodyType::Dynamic);
        assert_eq!(rb.mass, 10.0);
        assert!(rb.handle.is_none());
        assert_eq!(rb.gravity_scale, 1.0);
    }

    #[test]
    fn rigid_body_kinematic() {
        let rb = RigidBodyComponent::kinematic();
        assert_eq!(rb.body_type, BodyType::Kinematic);
        assert_eq!(rb.gravity_scale, 0.0);
    }

    #[test]
    fn rigid_body_static() {
        let rb = RigidBodyComponent::fixed();
        assert_eq!(rb.body_type, BodyType::Static);
    }

    #[test]
    fn collider_sphere() {
        let col = ColliderComponent::sphere(1.0);
        assert!(matches!(col.shape, ColliderShape::Sphere { radius } if radius == 1.0));
        assert!(!col.is_sensor);
        assert_eq!(col.friction, 0.5);
    }

    #[test]
    fn collider_cuboid() {
        let col = ColliderComponent::cuboid(1.0, 2.0, 3.0);
        assert!(
            matches!(col.shape, ColliderShape::Box { half_extents } if half_extents == [1.0, 2.0, 3.0])
        );
    }

    #[test]
    fn collider_capsule() {
        let col = ColliderComponent::capsule(0.5, 0.3);
        assert!(
            matches!(col.shape, ColliderShape::Capsule { half_height, radius } if half_height == 0.5 && radius == 0.3)
        );
    }

    #[test]
    fn collider_sensor_builder() {
        let col = ColliderComponent::sphere(1.0).sensor();
        assert!(col.is_sensor);
    }

    #[test]
    fn collider_with_layers() {
        let layers = CollisionLayers::player();
        let col = ColliderComponent::sphere(1.0).with_layers(layers);
        assert_eq!(col.layers, layers);
    }

    #[test]
    fn collider_with_friction_and_restitution() {
        let col = ColliderComponent::sphere(1.0)
            .with_friction(0.8)
            .with_restitution(0.5);
        assert_eq!(col.friction, 0.8);
        assert_eq!(col.restitution, 0.5);
    }

    #[test]
    fn velocity_default() {
        let v = Velocity::default();
        assert_eq!(v.linear, [0.0; 3]);
        assert_eq!(v.angular, [0.0; 3]);
    }

    #[test]
    fn transform_default() {
        let t = Transform::default();
        assert_eq!(t.position, [0.0; 3]);
        assert_eq!(t.rotation, [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn transform_from_position() {
        let t = Transform::from_position(1.0, 2.0, 3.0);
        assert_eq!(t.position, [1.0, 2.0, 3.0]);
        assert_eq!(t.rotation[3], 1.0);
    }

    #[test]
    fn physics_authority_default_is_server() {
        assert_eq!(PhysicsAuthority::default(), PhysicsAuthority::Server);
    }

    #[test]
    fn replication_modes() {
        assert_eq!(
            RigidBodyComponent::replication_mode(),
            ReplicationMode::ServerOnly
        );
        assert_eq!(
            ColliderComponent::replication_mode(),
            ReplicationMode::ServerOnly
        );
        assert_eq!(Velocity::replication_mode(), ReplicationMode::Replicated);
        assert_eq!(Transform::replication_mode(), ReplicationMode::Replicated);
    }
}
