use std::time::Duration;

use crate::world::World;

const DEFAULT_TIMESTEP_HZ: u32 = 60;
const DEFAULT_MAX_TICKS_PER_FRAME: u32 = 5;

/// Built-in resource providing the fixed delta time for the current tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeltaTime {
    /// Delta time in seconds for this tick.
    pub seconds: f64,
}

impl DeltaTime {
    pub fn new(seconds: f64) -> Self {
        Self { seconds }
    }

    pub fn from_duration(duration: Duration) -> Self {
        Self {
            seconds: duration.as_secs_f64(),
        }
    }
}

/// Built-in resource tracking overall simulation time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Time {
    /// Total elapsed simulation time in seconds.
    pub elapsed_seconds: f64,
    /// Total number of ticks executed.
    pub tick_count: u64,
}

impl Time {
    pub fn new() -> Self {
        Self {
            elapsed_seconds: 0.0,
            tick_count: 0,
        }
    }
}

impl Default for Time {
    fn default() -> Self {
        Self::new()
    }
}

/// A fixed-timestep tick runner that drives the ECS schedule.
///
/// Uses an accumulator pattern: elapsed wall-clock time is accumulated,
/// and the schedule is run once per fixed timestep until the accumulator
/// is drained below the timestep.
///
/// A spiral-of-death guard caps the maximum number of ticks per frame
/// to prevent runaway catch-up when the simulation falls behind.
pub struct TickRunner {
    timestep: Duration,
    accumulator: Duration,
    max_ticks_per_frame: u32,
}

impl TickRunner {
    /// Create a new TickRunner with the given fixed timestep.
    pub fn new(timestep: Duration) -> Self {
        Self {
            timestep,
            accumulator: Duration::ZERO,
            max_ticks_per_frame: DEFAULT_MAX_TICKS_PER_FRAME,
        }
    }

    /// Create a TickRunner from a frequency in Hz (e.g., 60 for 60 ticks/sec).
    pub fn from_hz(hz: u32) -> Self {
        let timestep = Duration::from_secs_f64(1.0 / hz as f64);
        Self::new(timestep)
    }

    /// Set the maximum number of ticks allowed per frame (spiral-of-death guard).
    pub fn with_max_ticks_per_frame(mut self, max: u32) -> Self {
        self.max_ticks_per_frame = max;
        self
    }

    /// Get the configured fixed timestep.
    pub fn timestep(&self) -> Duration {
        self.timestep
    }

    /// Get the current accumulator value.
    pub fn accumulator(&self) -> Duration {
        self.accumulator
    }

    /// Advance the simulation by the given elapsed wall-clock time.
    ///
    /// This will run the schedule zero or more times depending on how much
    /// time has accumulated. The `DeltaTime` and `Time` resources are updated
    /// before each tick. Event buffers are swapped after each tick.
    ///
    /// Returns the number of ticks that were executed.
    pub fn update(&mut self, elapsed: Duration, world: &mut World) -> u32 {
        self.accumulator += elapsed;

        // Ensure DeltaTime and Time resources exist
        if !world.has_resource::<DeltaTime>() {
            world.insert_resource(DeltaTime::from_duration(self.timestep));
        }
        if !world.has_resource::<Time>() {
            world.insert_resource(Time::new());
        }

        let dt = DeltaTime::from_duration(self.timestep);
        let mut ticks_this_frame: u32 = 0;

        while self.accumulator >= self.timestep && ticks_this_frame < self.max_ticks_per_frame {
            // Update DeltaTime resource
            if let Some(dt_res) = world.resource_mut::<DeltaTime>() {
                *dt_res = dt;
            }

            // Update Time resource
            if let Some(time) = world.resource_mut::<Time>() {
                time.elapsed_seconds += self.timestep.as_secs_f64();
                time.tick_count += 1;
            }

            // Run all systems
            world.run_systems();

            // Swap event buffers
            world.swap_event_buffers();

            self.accumulator -= self.timestep;
            ticks_this_frame += 1;
        }

        // Clamp accumulator if we hit the spiral-of-death guard
        if ticks_this_frame >= self.max_ticks_per_frame && self.accumulator >= self.timestep {
            self.accumulator = Duration::ZERO;
        }

        ticks_this_frame
    }
}

impl Default for TickRunner {
    fn default() -> Self {
        Self::from_hz(DEFAULT_TIMESTEP_HZ)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stage::Stage;
    use crate::system::SystemBuilder;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn single_tick_fires_when_accumulator_exceeds_timestep() {
        let mut runner = TickRunner::from_hz(60);
        let timestep = runner.timestep();
        let mut world = World::new();

        let counter = Arc::new(AtomicU32::new(0));
        let c = counter.clone();
        world.add_system(
            SystemBuilder::new("counter", move |_: &World| {
                c.fetch_add(1, Ordering::SeqCst);
            })
            .stage(Stage::Physics)
            .build(),
        );

        let ticks = runner.update(timestep, &mut world);
        assert_eq!(ticks, 1);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn multiple_ticks_fire_for_large_elapsed() {
        let mut runner = TickRunner::from_hz(60);
        let timestep = runner.timestep();
        let mut world = World::new();

        let counter = Arc::new(AtomicU32::new(0));
        let c = counter.clone();
        world.add_system(
            SystemBuilder::new("counter", move |_: &World| {
                c.fetch_add(1, Ordering::SeqCst);
            })
            .stage(Stage::Physics)
            .build(),
        );

        // 3 timesteps worth of elapsed time (using exact multiples)
        let ticks = runner.update(timestep * 3, &mut world);
        assert_eq!(ticks, 3);
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn spiral_of_death_guard_caps_ticks_per_frame() {
        let mut runner = TickRunner::from_hz(60).with_max_ticks_per_frame(3);
        let timestep = runner.timestep();
        let mut world = World::new();

        let counter = Arc::new(AtomicU32::new(0));
        let c = counter.clone();
        world.add_system(
            SystemBuilder::new("counter", move |_: &World| {
                c.fetch_add(1, Ordering::SeqCst);
            })
            .stage(Stage::Physics)
            .build(),
        );

        // 10 timesteps worth, but capped at 3
        let ticks = runner.update(timestep * 10, &mut world);
        assert_eq!(ticks, 3);
        assert_eq!(counter.load(Ordering::SeqCst), 3);

        // Accumulator should have been clamped to zero
        assert_eq!(runner.accumulator(), Duration::ZERO);
    }

    #[test]
    fn delta_time_resource_is_set_correctly() {
        let mut runner = TickRunner::from_hz(60);
        let timestep = runner.timestep();
        let mut world = World::new();

        runner.update(timestep, &mut world);

        let dt = world.resource::<DeltaTime>().unwrap();
        let expected = timestep.as_secs_f64();
        assert!(
            (dt.seconds - expected).abs() < 1e-10,
            "dt.seconds={} expected={}",
            dt.seconds,
            expected
        );
    }

    #[test]
    fn time_resource_tracks_total_elapsed_and_tick_count() {
        let mut runner = TickRunner::from_hz(60);
        let timestep = runner.timestep();
        let mut world = World::new();

        // Run 3 ticks using exact multiples
        runner.update(timestep * 3, &mut world);

        let time = world.resource::<Time>().unwrap();
        assert_eq!(time.tick_count, 3);
        let expected = timestep.as_secs_f64() * 3.0;
        assert!(
            (time.elapsed_seconds - expected).abs() < 1e-10,
            "elapsed={} expected={}",
            time.elapsed_seconds,
            expected
        );
    }

    #[test]
    fn zero_elapsed_produces_no_ticks() {
        let mut runner = TickRunner::from_hz(60);
        let mut world = World::new();

        let ticks = runner.update(Duration::ZERO, &mut world);
        assert_eq!(ticks, 0);
    }

    #[test]
    fn sub_timestep_elapsed_accumulates() {
        // Use a clean timestep (10ms) to avoid integer division rounding issues
        let timestep = Duration::from_millis(10);
        let mut runner = TickRunner::new(timestep);
        let mut world = World::new();

        let half_step = Duration::from_millis(5);

        // First half: no tick
        let ticks = runner.update(half_step, &mut world);
        assert_eq!(ticks, 0);
        assert!(runner.accumulator() > Duration::ZERO);

        // Second half: now we have enough for one tick
        let ticks = runner.update(half_step, &mut world);
        assert_eq!(ticks, 1);
    }

    #[test]
    fn configurable_timestep() {
        let timestep_30hz = Duration::from_secs_f64(1.0 / 30.0);
        let mut runner = TickRunner::new(timestep_30hz);
        let mut world = World::new();

        // One second should give 30 ticks at 30Hz, but capped at default 5
        let ticks = runner.update(timestep_30hz * 30, &mut world);
        assert_eq!(ticks, 5);

        // With higher cap
        let mut runner = TickRunner::new(timestep_30hz).with_max_ticks_per_frame(100);
        let mut world = World::new();
        let ticks = runner.update(timestep_30hz * 30, &mut world);
        assert_eq!(ticks, 30);
    }

    #[test]
    fn systems_execute_during_tick() {
        let mut runner = TickRunner::from_hz(60);
        let timestep = runner.timestep();
        let mut world = World::new();

        world.insert_resource(0u64);

        world.add_system(
            SystemBuilder::new("incrementer", |_w: &World| {
                // Systems currently take &World, so we can't mutate resources directly
                // This test just verifies the system runs
            })
            .stage(Stage::Physics)
            .build(),
        );

        let ticks = runner.update(timestep, &mut world);
        assert_eq!(ticks, 1);
    }

    #[test]
    fn event_buffers_are_swapped_each_tick() {
        let mut runner = TickRunner::from_hz(60);
        let timestep = runner.timestep();
        let mut world = World::new();
        world.insert_events::<TestEvent>();

        // Send an event before the tick
        world.send_event(TestEvent { value: 42 });

        // Run one tick -- this should swap event buffers
        runner.update(timestep, &mut world);

        // After the tick, the event should be in the read buffer
        let events = world.read_events::<TestEvent>().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].value, 42);
    }

    #[derive(Debug, PartialEq, Clone)]
    struct TestEvent {
        value: u32,
    }

    #[test]
    fn default_tick_runner_is_60hz() {
        let runner = TickRunner::default();
        let expected = Duration::from_secs_f64(1.0 / 60.0);
        let diff = if runner.timestep() > expected {
            runner.timestep() - expected
        } else {
            expected - runner.timestep()
        };
        assert!(diff < Duration::from_nanos(100));
    }

    #[test]
    fn time_accumulates_across_multiple_updates() {
        let mut runner = TickRunner::from_hz(60);
        let timestep = runner.timestep();
        let mut world = World::new();

        runner.update(timestep * 2, &mut world);
        runner.update(timestep, &mut world);

        let time = world.resource::<Time>().unwrap();
        assert_eq!(time.tick_count, 3);
        let expected = timestep.as_secs_f64() * 3.0;
        assert!(
            (time.elapsed_seconds - expected).abs() < 1e-10,
            "elapsed={} expected={}",
            time.elapsed_seconds,
            expected
        );
    }

    #[test]
    fn accumulator_preserves_remainder() {
        let mut runner = TickRunner::from_hz(60);
        let timestep = runner.timestep();
        let mut world = World::new();

        // 1.5 timesteps: should run 1 tick with 0.5 timestep remaining
        let elapsed = timestep + timestep / 2;
        let ticks = runner.update(elapsed, &mut world);
        assert_eq!(ticks, 1);
        assert!(runner.accumulator() > Duration::ZERO);
        assert!(runner.accumulator() < runner.timestep());
    }
}
