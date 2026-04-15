pub mod components;
pub mod config;
pub mod events;
pub mod joints;
pub mod layers;
pub mod query;
pub mod shape_query_2d;
pub mod trigger;
pub mod vr;
pub mod world;

pub use components::{
    BodyType, ColliderComponent, ColliderShape, PhysicsAuthority, RigidBodyComponent, Transform,
    Velocity,
};
pub use config::WorldPhysicsConfig;
pub use events::PhysicsCollisionEvent;
pub use joints::JointType;
pub use layers::CollisionLayers;
pub use query::{QueryFilter, RaycastHit};
pub use shape_query_2d::{
    circle_overlap_2d, cone_contains_point_2d, rect_overlap_2d, Circle2, Rect2, Vec2,
};
pub use trigger::{TriggerEvent, TriggerEventKind, TriggerEventQueue};
pub use world::PhysicsWorld;
