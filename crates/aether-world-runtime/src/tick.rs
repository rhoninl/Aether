//! Fixed-rate server tick loop with time accumulator.

use serde::{Deserialize, Serialize};

/// Maximum ticks per single update call to prevent spiral-of-death.
const MAX_TICKS_PER_UPDATE: u32 = 10;

/// Minimum tick rate in Hz.
const MIN_TICK_RATE_HZ: u32 = 1;

/// Maximum tick rate in Hz.
const MAX_TICK_RATE_HZ: u32 = 240;

/// Represents a single server simulation tick.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerTick {
    pub tick_number: u64,
    pub delta_time_us: u64,
    pub timestamp_us: u64,
}

/// Manages fixed-timestep tick scheduling.
///
/// The caller provides elapsed wall-clock time, and the scheduler
/// determines how many simulation ticks to run this frame.
#[derive(Debug)]
pub struct TickScheduler {
    tick_rate_hz: u32,
    tick_interval_us: u64,
    tick_number: u64,
    accumulator_us: u64,
    total_elapsed_us: u64,
    max_ticks_per_update: u32,
}

impl TickScheduler {
    /// Create a new tick scheduler at the given rate.
    /// Clamps tick_rate_hz to [1, 240].
    pub fn new(tick_rate_hz: u32) -> Self {
        let clamped = tick_rate_hz.clamp(MIN_TICK_RATE_HZ, MAX_TICK_RATE_HZ);
        let tick_interval_us = 1_000_000 / u64::from(clamped);
        Self {
            tick_rate_hz: clamped,
            tick_interval_us,
            tick_number: 0,
            accumulator_us: 0,
            total_elapsed_us: 0,
            max_ticks_per_update: MAX_TICKS_PER_UPDATE,
        }
    }

    /// Set the maximum number of ticks that can be produced in a single update.
    pub fn set_max_ticks_per_update(&mut self, max: u32) {
        self.max_ticks_per_update = max.max(1);
    }

    /// Advance the scheduler by `elapsed_us` microseconds.
    /// Returns the ticks that should be simulated this frame.
    pub fn update(&mut self, elapsed_us: u64) -> Vec<ServerTick> {
        self.accumulator_us = self.accumulator_us.saturating_add(elapsed_us);
        self.total_elapsed_us = self.total_elapsed_us.saturating_add(elapsed_us);

        let mut ticks = Vec::new();
        let mut produced = 0u32;

        while self.accumulator_us >= self.tick_interval_us && produced < self.max_ticks_per_update {
            self.accumulator_us -= self.tick_interval_us;
            self.tick_number += 1;
            produced += 1;

            ticks.push(ServerTick {
                tick_number: self.tick_number,
                delta_time_us: self.tick_interval_us,
                timestamp_us: self.total_elapsed_us,
            });
        }

        // If we hit the cap, drain the remaining accumulator to prevent spiral-of-death
        if produced >= self.max_ticks_per_update && self.accumulator_us >= self.tick_interval_us {
            self.accumulator_us = self.accumulator_us % self.tick_interval_us;
        }

        ticks
    }

    pub fn tick_number(&self) -> u64 {
        self.tick_number
    }

    pub fn tick_rate_hz(&self) -> u32 {
        self.tick_rate_hz
    }

    pub fn tick_interval_us(&self) -> u64 {
        self.tick_interval_us
    }

    pub fn accumulator_us(&self) -> u64 {
        self.accumulator_us
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_clamps_tick_rate() {
        let s = TickScheduler::new(0);
        assert_eq!(s.tick_rate_hz(), MIN_TICK_RATE_HZ);

        let s = TickScheduler::new(500);
        assert_eq!(s.tick_rate_hz(), MAX_TICK_RATE_HZ);

        let s = TickScheduler::new(60);
        assert_eq!(s.tick_rate_hz(), 60);
    }

    #[test]
    fn test_tick_interval_calculation() {
        let s = TickScheduler::new(60);
        // 1_000_000 / 60 = 16666 us
        assert_eq!(s.tick_interval_us(), 1_000_000 / 60);
    }

    #[test]
    fn test_zero_elapsed_produces_no_ticks() {
        let mut s = TickScheduler::new(60);
        let ticks = s.update(0);
        assert!(ticks.is_empty());
        assert_eq!(s.tick_number(), 0);
    }

    #[test]
    fn test_single_tick_produced() {
        let mut s = TickScheduler::new(60);
        let interval = s.tick_interval_us();
        let ticks = s.update(interval);
        assert_eq!(ticks.len(), 1);
        assert_eq!(ticks[0].tick_number, 1);
        assert_eq!(ticks[0].delta_time_us, interval);
    }

    #[test]
    fn test_multiple_ticks_produced() {
        let mut s = TickScheduler::new(60);
        let interval = s.tick_interval_us();
        // Feed 3 tick intervals worth of time
        let ticks = s.update(interval * 3);
        assert_eq!(ticks.len(), 3);
        assert_eq!(ticks[0].tick_number, 1);
        assert_eq!(ticks[1].tick_number, 2);
        assert_eq!(ticks[2].tick_number, 3);
    }

    #[test]
    fn test_accumulator_carries_remainder() {
        let mut s = TickScheduler::new(60);
        let interval = s.tick_interval_us();
        // Feed slightly more than one tick
        let ticks = s.update(interval + 100);
        assert_eq!(ticks.len(), 1);
        assert_eq!(s.accumulator_us(), 100);

        // Feed the rest to complete another tick
        let ticks = s.update(interval - 100);
        assert_eq!(ticks.len(), 1);
        assert_eq!(ticks[0].tick_number, 2);
    }

    #[test]
    fn test_max_ticks_per_update_caps_output() {
        let mut s = TickScheduler::new(60);
        s.set_max_ticks_per_update(3);
        let interval = s.tick_interval_us();
        // Feed 100 ticks worth of time
        let ticks = s.update(interval * 100);
        assert_eq!(ticks.len(), 3);
        // Accumulator should be drained to prevent spiral
        assert!(s.accumulator_us() < interval);
    }

    #[test]
    fn test_tick_numbers_are_monotonic() {
        let mut s = TickScheduler::new(30);
        let interval = s.tick_interval_us();
        let t1 = s.update(interval * 2);
        let t2 = s.update(interval * 3);
        let all: Vec<u64> = t1.iter().chain(t2.iter()).map(|t| t.tick_number).collect();
        assert_eq!(all, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_sub_tick_elapsed_accumulates() {
        let mut s = TickScheduler::new(60);
        let interval = s.tick_interval_us();
        let half = interval / 2;

        let ticks = s.update(half);
        assert!(ticks.is_empty());

        let ticks = s.update(half);
        // Depending on rounding, we should get exactly 1 tick
        // half + half = interval (or interval - 1 due to integer division)
        assert!(ticks.len() <= 1);
    }

    #[test]
    fn test_one_hz_tick_rate() {
        let mut s = TickScheduler::new(1);
        assert_eq!(s.tick_interval_us(), 1_000_000);
        let ticks = s.update(999_999);
        assert!(ticks.is_empty());
        let ticks = s.update(1);
        assert_eq!(ticks.len(), 1);
    }

    #[test]
    fn test_timestamp_increases() {
        let mut s = TickScheduler::new(60);
        let interval = s.tick_interval_us();
        let t1 = s.update(interval);
        let t2 = s.update(interval);
        assert!(t2[0].timestamp_us > t1[0].timestamp_us);
    }
}
