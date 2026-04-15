//! Headless test harness for driving a [`World`] from tests.
//!
//! [`TestWorld`] is a thin wrapper over [`World`] that makes it trivial to
//! advance the ECS by N ticks in just a few lines of code. It is not intended
//! for production use and is gated behind the `test-harness` cargo feature
//! (enabled automatically for unit and integration tests).
//!
//! ```
//! use aether_ecs::test_harness::TestWorld;
//!
//! let mut tw = TestWorld::new();
//! let ticks = tw.tick_n(3);
//! assert_eq!(ticks, 3);
//! ```

use crate::system::System;
use crate::world::World;

/// Thin wrapper over [`World`] that lets tests advance `N` ticks with
/// synthetic input in a few lines.
///
/// Not for production use — enable via the `test-harness` feature.
pub struct TestWorld {
    world: World,
    ticks: u64,
}

impl TestWorld {
    /// Construct a harness wrapping a fresh, empty [`World`].
    pub fn new() -> Self {
        Self {
            world: World::new(),
            ticks: 0,
        }
    }

    /// Begin constructing a [`TestWorld`] via the fluent builder API.
    pub fn builder() -> TestWorldBuilder {
        TestWorldBuilder::new()
    }

    /// Borrow the underlying [`World`] immutably.
    pub fn world(&self) -> &World {
        &self.world
    }

    /// Borrow the underlying [`World`] mutably so tests can spawn entities,
    /// register components, inject synthetic input, etc.
    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    /// The number of ticks this harness has advanced so far.
    pub fn ticks(&self) -> u64 {
        self.ticks
    }

    /// Run all registered systems once. Returns the tick count after.
    pub fn tick_once(&mut self) -> u64 {
        self.world.run_systems();
        self.ticks += 1;
        self.ticks
    }

    /// Run `n` ticks. Returns the tick count after.
    pub fn tick_n(&mut self, n: usize) -> u64 {
        for _ in 0..n {
            self.tick_once();
        }
        self.ticks
    }
}

impl Default for TestWorld {
    fn default() -> Self {
        Self::new()
    }
}

/// Fluent builder for [`TestWorld`]. Mirrors the subset of the [`World`] API
/// that tests commonly configure before their first tick.
pub struct TestWorldBuilder {
    systems: Vec<Box<dyn System>>,
    setups: Vec<Box<dyn FnOnce(&mut World)>>,
}

impl TestWorldBuilder {
    /// Create an empty builder.
    pub fn new() -> Self {
        Self {
            systems: Vec::new(),
            setups: Vec::new(),
        }
    }

    /// Register a system with the world under construction.
    pub fn with_system(mut self, system: Box<dyn System>) -> Self {
        self.systems.push(system);
        self
    }

    /// Run an arbitrary setup closure against the underlying world. Useful
    /// for registering components or spawning fixture entities in one line.
    /// Setups run during [`build`](Self::build), before systems are added.
    pub fn with_setup<F: FnOnce(&mut World) + 'static>(mut self, setup: F) -> Self {
        self.setups.push(Box::new(setup));
        self
    }

    /// Finalize and return the configured [`TestWorld`].
    pub fn build(self) -> TestWorld {
        let mut world = World::new();
        for setup in self.setups {
            setup(&mut world);
        }
        for system in self.systems {
            world.add_system(system);
        }
        TestWorld { world, ticks: 0 }
    }
}

impl Default for TestWorldBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::Component;
    use crate::system::SystemBuilder;

    struct Counter(u32);
    impl Component for Counter {}

    #[test]
    fn tick_once_returns_one() {
        let mut tw = TestWorld::new();
        assert_eq!(tw.tick_once(), 1);
        assert_eq!(tw.ticks(), 1);
    }

    #[test]
    fn tick_n_returns_n() {
        let mut tw = TestWorld::new();
        assert_eq!(tw.tick_n(10), 10);
        assert_eq!(tw.ticks(), 10);
    }

    #[test]
    fn tick_n_accumulates_across_calls() {
        let mut tw = TestWorld::new();
        tw.tick_n(4);
        assert_eq!(tw.tick_n(6), 10);
    }

    #[test]
    fn builder_roundtrips_to_usable_testworld() {
        let mut tw = TestWorld::builder().build();
        assert_eq!(tw.tick_once(), 1);
    }

    #[test]
    fn builder_with_system_registers_system() {
        let system = SystemBuilder::new("noop", |_w| {}).build();
        let mut tw = TestWorld::builder().with_system(system).build();
        assert_eq!(tw.tick_n(3), 3);
    }

    #[test]
    fn world_mut_spawn_observable_after_tick() {
        let mut tw = TestWorld::new();
        let entity = tw.world_mut().spawn_with_1(Counter(7));
        tw.tick_once();
        let counter = tw.world().get_component::<Counter>(entity).unwrap();
        assert_eq!(counter.0, 7);
    }

    #[test]
    fn default_testworld_is_empty() {
        let tw = TestWorld::default();
        assert_eq!(tw.ticks(), 0);
        assert_eq!(tw.world().entity_count(), 0);
    }

    #[test]
    fn builder_with_setup_runs_before_tick() {
        let mut tw = TestWorld::builder()
            .with_setup(|world| {
                world.spawn_with_1(Counter(42));
            })
            .build();
        assert_eq!(tw.world().entity_count(), 1);
        tw.tick_once();
        assert_eq!(tw.world().entity_count(), 1);
    }
}
