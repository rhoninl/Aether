pub mod archetype;
pub mod component;
pub mod entity;
pub mod network;
pub mod query;
pub mod schedule;
pub mod stage;
pub mod system;
pub mod world;

#[cfg(any(test, feature = "test-harness"))]
pub mod test_harness;

pub use archetype::{ArchetypeId, ArchetypeStorage};
pub use component::{Component, ComponentId, ComponentRegistry, ReplicationMode};
pub use entity::Entity;
pub use network::{Authority, NetworkIdentity};
pub use query::AccessDescriptor;
pub use schedule::{
    AlertSeverity, RuntimeAlert, Schedule, ScheduleDiagnostics, ScheduleMetrics, StageMetrics,
    SystemMetrics,
};
pub use stage::Stage;
pub use system::{System, SystemBuilder};
pub use world::World;

#[cfg(any(test, feature = "test-harness"))]
pub use test_harness::{TestWorld, TestWorldBuilder};
