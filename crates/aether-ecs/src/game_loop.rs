//! Fixed-timestep game loop helper for headless, deterministic ticking.
//!
//! `FixedTimestepRunner` owns an accumulator and drives a user-supplied closure
//! at a fixed simulation rate regardless of the wall-clock cadence used to feed
//! elapsed time into it. Non-VR games (servers, tests, tools) can reuse this
//! instead of reinventing the accumulator loop around [`crate::World::run_systems`].
//!
//! The closure is intentionally parameterless so callers can close over any
//! state they want (a `World`, a schedule, multiple worlds, etc.) without the
//! helper needing to borrow or own anything.

/// Default simulation frequency used when callers do not specify one.
pub const DEFAULT_TICK_HZ: f32 = 60.0;

/// Default cap on how many substeps a single `advance` call may execute.
///
/// Clamping prevents the classic "spiral of death" where a long frame
/// accumulates enough debt to trigger hundreds of catch-up ticks, which in
/// turn take even longer and dig the hole deeper.
pub const DEFAULT_MAX_SUBSTEPS: u32 = 8;

/// Fixed-timestep runner that drives a closure at a constant simulation rate.
///
/// See the module documentation for the design rationale.
#[derive(Debug, Clone)]
pub struct FixedTimestepRunner {
    tick_hz: f32,
    tick_dt: f32,
    accumulator: f32,
    max_substeps: u32,
    total_ticks: u64,
}

impl FixedTimestepRunner {
    /// Creates a runner ticking at `tick_hz` simulation steps per second.
    ///
    /// `tick_hz` must be strictly positive; non-positive values fall back to
    /// [`DEFAULT_TICK_HZ`] so callers cannot accidentally divide by zero.
    pub fn new(tick_hz: f32) -> Self {
        let hz = if tick_hz > 0.0 {
            tick_hz
        } else {
            DEFAULT_TICK_HZ
        };
        Self {
            tick_hz: hz,
            tick_dt: 1.0 / hz,
            accumulator: 0.0,
            max_substeps: DEFAULT_MAX_SUBSTEPS,
            total_ticks: 0,
        }
    }

    /// Overrides the maximum number of substeps executed per `advance` call.
    ///
    /// A value of `0` is treated as `1` to guarantee forward progress.
    pub fn with_max_substeps(mut self, n: u32) -> Self {
        self.max_substeps = n.max(1);
        self
    }

    /// Returns the simulation tick length in seconds (`1 / tick_hz`).
    pub fn tick_dt(&self) -> f32 {
        self.tick_dt
    }

    /// Returns the configured simulation frequency in hertz.
    pub fn tick_hz(&self) -> f32 {
        self.tick_hz
    }

    /// Returns the configured substep cap.
    pub fn max_substeps(&self) -> u32 {
        self.max_substeps
    }

    /// Returns the total number of substeps executed across all `advance` calls.
    pub fn total_ticks(&self) -> u64 {
        self.total_ticks
    }

    /// Advances the accumulator by `elapsed_seconds` and invokes `tick_fn`
    /// zero or more times (once per full simulation step that fits).
    ///
    /// Returns the number of substeps actually executed. Negative or NaN
    /// inputs are ignored. Huge inputs are clamped to at most
    /// [`FixedTimestepRunner::max_substeps`] substeps per call so a stalled
    /// host cannot trigger a spiral of death.
    pub fn advance<F>(&mut self, elapsed_seconds: f32, mut tick_fn: F) -> u32
    where
        F: FnMut(),
    {
        if !elapsed_seconds.is_finite() || elapsed_seconds <= 0.0 {
            return 0;
        }

        self.accumulator += elapsed_seconds;

        let max_accumulated = self.tick_dt * self.max_substeps as f32;
        if self.accumulator > max_accumulated {
            self.accumulator = max_accumulated;
        }

        let mut substeps: u32 = 0;
        while self.accumulator >= self.tick_dt && substeps < self.max_substeps {
            tick_fn();
            self.accumulator -= self.tick_dt;
            substeps += 1;
        }

        self.total_ticks += substeps as u64;
        substeps
    }
}

impl Default for FixedTimestepRunner {
    fn default() -> Self {
        Self::new(DEFAULT_TICK_HZ)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIXTY_HZ_DT: f32 = 1.0 / 60.0;
    const EPSILON: f32 = 1e-6;

    #[test]
    fn tick_dt_matches_requested_frequency() {
        let runner = FixedTimestepRunner::new(60.0);
        assert!((runner.tick_dt() - SIXTY_HZ_DT).abs() < EPSILON);
        assert_eq!(runner.total_ticks(), 0);
        assert_eq!(runner.max_substeps(), DEFAULT_MAX_SUBSTEPS);
    }

    #[test]
    fn default_uses_default_tick_hz() {
        let runner = FixedTimestepRunner::default();
        assert!((runner.tick_hz() - DEFAULT_TICK_HZ).abs() < EPSILON);
    }

    #[test]
    fn one_dt_runs_single_substep() {
        let mut runner = FixedTimestepRunner::new(60.0);
        let mut count = 0;
        let substeps = runner.advance(SIXTY_HZ_DT, || count += 1);
        assert_eq!(substeps, 1);
        assert_eq!(count, 1);
        assert_eq!(runner.total_ticks(), 1);
    }

    #[test]
    fn two_dts_runs_two_substeps() {
        let mut runner = FixedTimestepRunner::new(60.0);
        let mut count = 0;
        let substeps = runner.advance(2.0 * SIXTY_HZ_DT, || count += 1);
        assert_eq!(substeps, 2);
        assert_eq!(count, 2);
        assert_eq!(runner.total_ticks(), 2);
    }

    #[test]
    fn partial_dt_accumulates_across_calls() {
        let mut runner = FixedTimestepRunner::new(60.0);
        let mut count = 0;

        let first = runner.advance(0.5 * SIXTY_HZ_DT, || count += 1);
        assert_eq!(first, 0);
        assert_eq!(count, 0);
        assert_eq!(runner.total_ticks(), 0);

        let second = runner.advance(0.5 * SIXTY_HZ_DT, || count += 1);
        assert_eq!(second, 1);
        assert_eq!(count, 1);
        assert_eq!(runner.total_ticks(), 1);
    }

    #[test]
    fn huge_elapsed_clamps_to_max_substeps() {
        let mut runner = FixedTimestepRunner::new(60.0);
        let mut count = 0;
        let substeps = runner.advance(10.0, || count += 1);
        assert_eq!(substeps, DEFAULT_MAX_SUBSTEPS);
        assert_eq!(count, DEFAULT_MAX_SUBSTEPS as usize);
        assert_eq!(runner.total_ticks(), DEFAULT_MAX_SUBSTEPS as u64);
    }

    #[test]
    fn with_max_substeps_overrides_clamp() {
        let mut runner = FixedTimestepRunner::new(60.0).with_max_substeps(3);
        let mut count = 0;
        let substeps = runner.advance(10.0, || count += 1);
        assert_eq!(substeps, 3);
        assert_eq!(count, 3);
        assert_eq!(runner.max_substeps(), 3);
    }

    #[test]
    fn with_max_substeps_zero_is_promoted_to_one() {
        let mut runner = FixedTimestepRunner::new(60.0).with_max_substeps(0);
        assert_eq!(runner.max_substeps(), 1);
        let mut count = 0;
        let substeps = runner.advance(1.0, || count += 1);
        assert_eq!(substeps, 1);
        assert_eq!(count, 1);
    }

    #[test]
    fn total_ticks_accumulates_across_calls() {
        let mut runner = FixedTimestepRunner::new(60.0);
        let mut count = 0;
        runner.advance(SIXTY_HZ_DT, || count += 1);
        runner.advance(SIXTY_HZ_DT, || count += 1);
        runner.advance(2.0 * SIXTY_HZ_DT, || count += 1);
        assert_eq!(runner.total_ticks(), 4);
        assert_eq!(count, 4);
    }

    #[test]
    fn closure_runs_once_per_reported_substep() {
        let mut runner = FixedTimestepRunner::new(120.0);
        let dt = runner.tick_dt();
        let mut count = 0;
        let substeps = runner.advance(dt * 5.0, || count += 1);
        assert_eq!(substeps as usize, count);
    }

    #[test]
    fn negative_or_nan_elapsed_is_ignored() {
        let mut runner = FixedTimestepRunner::new(60.0);
        let mut count = 0;
        assert_eq!(runner.advance(-1.0, || count += 1), 0);
        assert_eq!(runner.advance(f32::NAN, || count += 1), 0);
        assert_eq!(runner.advance(0.0, || count += 1), 0);
        assert_eq!(count, 0);
        assert_eq!(runner.total_ticks(), 0);
    }

    #[test]
    fn non_positive_tick_hz_falls_back_to_default() {
        let runner = FixedTimestepRunner::new(0.0);
        assert!((runner.tick_hz() - DEFAULT_TICK_HZ).abs() < EPSILON);
        let runner_neg = FixedTimestepRunner::new(-5.0);
        assert!((runner_neg.tick_hz() - DEFAULT_TICK_HZ).abs() < EPSILON);
    }
}
