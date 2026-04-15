pub mod archetype;
pub mod component;
pub mod entity;
pub mod event;
pub mod game_loop;
pub mod network;
pub mod query;
pub mod resource;
pub mod schedule;
pub mod stage;
pub mod system;
pub mod tick;
pub mod world;

#[cfg(any(test, feature = "test-harness"))]
pub mod test_harness;

#[cfg(feature = "serde")]
pub mod save;

pub use archetype::{ArchetypeId, ArchetypeStorage};
pub use component::{Component, ComponentId, ComponentRegistry, ReplicationMode};
pub use entity::Entity;
pub use event::Events;
pub use game_loop::{FixedTimestepRunner, DEFAULT_MAX_SUBSTEPS, DEFAULT_TICK_HZ};
pub use network::{Authority, NetworkIdentity};
pub use query::AccessDescriptor;
pub use resource::Resources;
pub use schedule::{
    AlertSeverity, RuntimeAlert, Schedule, ScheduleDiagnostics, ScheduleMetrics, StageMetrics,
    SystemMetrics,
};
pub use stage::Stage;
pub use system::{System, SystemBuilder};
pub use tick::{DeltaTime, TickRunner, Time};
pub use world::World;

#[cfg(any(test, feature = "test-harness"))]
pub use test_harness::{TestWorld, TestWorldBuilder};

#[cfg(feature = "serde")]
pub use save::{
    restore_world_manual, snapshot_world_manual, SaveError, SaveLoad, WorldSnapshot,
    SNAPSHOT_VERSION,
};
