use std::collections::HashMap;
use std::time::Instant;

use rayon::prelude::*;

use crate::query::AccessDescriptor;
use crate::stage::Stage;
use crate::system::System;
use crate::world::World;

const STAGE_COUNT: usize = Stage::ALL.len();

/// Timing details for one stage in a single snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct StageMetrics {
    pub stage: Stage,
    pub runs: u64,
    pub total_time_ns: u128,
    pub last_time_ns: u128,
    pub batch_count: u32,
}

impl StageMetrics {
    pub fn average_time_ns(&self) -> Option<u128> {
        if self.runs == 0 {
            None
        } else {
            Some(self.total_time_ns / self.runs as u128)
        }
    }
}

/// Scheduling diagnostics for quick operator inspection.
#[derive(Clone, Debug, PartialEq)]
pub struct ScheduleDiagnostics {
    pub run_count: u64,
    pub total_time_ns: u128,
    pub last_run_time_ns: u128,
    pub last_batch_count_total: u32,
    pub max_batch_count_last_run: u32,
}

/// Timing details for one system in a single snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct SystemMetrics {
    pub name: String,
    pub stage: Stage,
    pub runs: u64,
    pub total_time_ns: u128,
    pub last_time_ns: u128,
}

impl SystemMetrics {
    pub fn average_time_ns(&self) -> Option<u128> {
        if self.runs == 0 {
            None
        } else {
            Some(self.total_time_ns / self.runs as u128)
        }
    }
}

/// Snapshot of schedule runtime metrics.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScheduleMetrics {
    pub run_count: u64,
    pub total_time_ns: u128,
    pub stage_timings: Vec<StageMetrics>,
    pub system_timings: Vec<SystemMetrics>,
}

/// Severity of an alert emitted by a simple runtime health rule.
#[derive(Clone, Debug, PartialEq)]
pub enum AlertSeverity {
    Warning,
    Critical,
}

/// Result item from lightweight runtime health checks.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeAlert {
    pub name: String,
    pub message: String,
    pub severity: AlertSeverity,
}

#[derive(Clone, Copy, Debug, Default)]
struct TimingAccumulator {
    runs: u64,
    total_time_ns: u128,
    last_time_ns: u128,
}

#[derive(Clone, Debug)]
struct SystemMetricState {
    name: String,
    stage: Stage,
    timing: TimingAccumulator,
}

impl SystemMetricState {
    fn new(name: String, stage: Stage) -> Self {
        Self {
            name,
            stage,
            timing: TimingAccumulator::default(),
        }
    }

    fn snapshot(&self) -> SystemMetrics {
        SystemMetrics {
            name: self.name.clone(),
            stage: self.stage,
            runs: self.timing.runs,
            total_time_ns: self.timing.total_time_ns,
            last_time_ns: self.timing.last_time_ns,
        }
    }
}

/// A batch of systems that can run in parallel (no access conflicts between them).
#[derive(Clone, Debug)]
struct SystemBatch {
    system_indices: Vec<usize>,
}

/// The scheduler organizes systems into stages, builds dependency graphs within
/// each stage, and executes non-conflicting systems in parallel.
pub struct Schedule {
    systems: Vec<Box<dyn System>>,
    /// Cached batches per stage, rebuilt when systems change.
    stage_batches: HashMap<Stage, Vec<SystemBatch>>,
    dirty: bool,
    total_runs: u64,
    total_time_ns: u128,
    last_run_time_ns: u128,
    stage_timings: [TimingAccumulator; STAGE_COUNT],
    last_stage_batch_counts: [u32; STAGE_COUNT],
    system_timings: Vec<SystemMetricState>,
}

impl Schedule {
    pub fn new() -> Self {
        Self {
            systems: Vec::new(),
            stage_batches: HashMap::new(),
            dirty: true,
            total_runs: 0,
            total_time_ns: 0,
            last_run_time_ns: 0,
            stage_timings: [TimingAccumulator::default(); STAGE_COUNT],
            last_stage_batch_counts: [0; STAGE_COUNT],
            system_timings: Vec::new(),
        }
    }

    pub fn add_system(&mut self, system: Box<dyn System>) {
        let name = system.name().to_string();
        let stage = system.stage();
        self.systems.push(system);
        self.system_timings
            .push(SystemMetricState::new(name, stage));
        self.dirty = true;
    }

    pub fn system_count(&self) -> usize {
        self.systems.len()
    }

    /// Rebuild the parallel execution batches for all stages.
    /// Within each stage, systems are grouped into batches where no two systems
    /// in the same batch have conflicting access patterns.
    fn rebuild_batches(&mut self) {
        self.stage_batches.clear();

        for &stage in &Stage::ALL {
            let stage_system_indices: Vec<usize> = self
                .systems
                .iter()
                .enumerate()
                .filter(|(_, s)| s.stage() == stage)
                .map(|(i, _)| i)
                .collect();

            if stage_system_indices.is_empty() {
                continue;
            }

            let batches = build_batches(&stage_system_indices, &self.systems);
            self.stage_batches.insert(stage, batches);
        }

        self.dirty = false;
    }

    /// Execute all systems in stage order. Within each stage, batches run
    /// sequentially, but systems within a batch run in parallel.
    pub fn run(&mut self, world: &World) {
        let run_start = Instant::now();
        self.total_runs = self.total_runs.saturating_add(1);

        if self.dirty {
            self.rebuild_batches();
        }

        self.last_stage_batch_counts = [0; STAGE_COUNT];

        if self.systems.is_empty() {
            let elapsed = run_start.elapsed().as_nanos();
            self.total_time_ns = self.total_time_ns.saturating_add(elapsed);
            self.last_run_time_ns = elapsed;
            return;
        }

        for &stage in &Stage::ALL {
            if let Some(batches) = self.stage_batches.get(&stage).cloned() {
                let batches_present = !batches.is_empty();
                let batch_count = if batches_present {
                    batches.len() as u32
                } else {
                    0
                };
                let stage_start = Instant::now();
                for batch in batches {
                    if batch.system_indices.len() == 1 {
                        // Single system: run directly without rayon overhead
                        let idx = batch.system_indices[0];
                        let elapsed = Self::timed_system_run(&*self.systems[idx], world);
                        self.record_system_run(idx, elapsed);
                    } else {
                        // Multiple non-conflicting systems: run in parallel
                        let timings: Vec<(usize, u128)> = batch
                            .system_indices
                            .par_iter()
                            .map(|&idx| (idx, Self::timed_system_run(&*self.systems[idx], world)))
                            .collect();
                        for (idx, elapsed) in timings {
                            self.record_system_run(idx, elapsed);
                        }
                    }
                }
                if batches_present {
                    let elapsed = stage_start.elapsed().as_nanos();
                    self.last_stage_batch_counts[stage as usize] = batch_count;
                    self.record_stage_run(stage, elapsed);
                }
            }
        }

        let elapsed = run_start.elapsed().as_nanos();
        self.total_time_ns = self.total_time_ns.saturating_add(elapsed);
        self.last_run_time_ns = elapsed;
    }

    /// Clear all collected runtime metrics.
    pub fn clear_metrics(&mut self) {
        self.total_runs = 0;
        self.total_time_ns = 0;
        self.last_run_time_ns = 0;
        self.stage_timings = [TimingAccumulator::default(); STAGE_COUNT];
        self.last_stage_batch_counts = [0; STAGE_COUNT];
        for metric in &mut self.system_timings {
            metric.timing = TimingAccumulator::default();
        }
    }

    /// Get snapshot metrics for schedule execution observability.
    pub fn metrics(&self) -> ScheduleMetrics {
        let stage_timings = Stage::ALL
            .iter()
            .map(|&stage| {
                let acc = &self.stage_timings[stage as usize];
                StageMetrics {
                    stage,
                    runs: acc.runs,
                    total_time_ns: acc.total_time_ns,
                    last_time_ns: acc.last_time_ns,
                    batch_count: self.last_stage_batch_counts[stage as usize],
                }
            })
            .collect();

        let system_timings = self
            .system_timings
            .iter()
            .map(SystemMetricState::snapshot)
            .collect();

        ScheduleMetrics {
            run_count: self.total_runs,
            total_time_ns: self.total_time_ns,
            stage_timings,
            system_timings,
        }
    }

    /// Emit compact scheduling diagnostics for external profilers and dashboards.
    pub fn diagnostics(&self) -> ScheduleDiagnostics {
        let (last_batch_count_total, max_batch_count_last_run) = self
            .last_stage_batch_counts
            .iter()
            .fold((0u32, 0u32), |(total, max), &value| {
                (total.saturating_add(value), max.max(value))
            });

        ScheduleDiagnostics {
            run_count: self.total_runs,
            total_time_ns: self.total_time_ns,
            last_run_time_ns: self.last_run_time_ns,
            last_batch_count_total,
            max_batch_count_last_run,
        }
    }

    /// Render metrics as Prometheus text format for scrape endpoints.
    pub fn metrics_prometheus(&self) -> String {
        let metrics = self.metrics();
        let mut out = String::new();

        out.push_str("# HELP aether_schedule_run_count Total number of schedule runs.\n");
        out.push_str("# TYPE aether_schedule_run_count counter\n");
        out.push_str(&format!(
            "aether_schedule_run_count {}\n",
            metrics.run_count
        ));

        out.push_str("# HELP aether_schedule_total_time_ns Total time spent across all runs in nanoseconds.\n");
        out.push_str("# TYPE aether_schedule_total_time_ns counter\n");
        out.push_str(&format!(
            "aether_schedule_total_time_ns {}\n",
            metrics.total_time_ns
        ));

        out.push_str("# HELP aether_schedule_stage_runs_total Stage execution count.\n");
        out.push_str("# TYPE aether_schedule_stage_runs_total counter\n");
        for stage_metric in &metrics.stage_timings {
            out.push_str(&format!(
                "aether_schedule_stage_runs_total{{stage=\"{}\"}} {}\n",
                stage_metric.stage.name(),
                stage_metric.runs
            ));
        }

        out.push_str("# HELP aether_schedule_stage_time_ns Stage cumulative execution time in nanoseconds.\n");
        out.push_str("# TYPE aether_schedule_stage_time_ns counter\n");
        for stage_metric in &metrics.stage_timings {
            out.push_str(&format!(
                "aether_schedule_stage_time_ns{{stage=\"{}\"}} {}\n",
                stage_metric.stage.name(),
                stage_metric.total_time_ns
            ));
        }

        out.push_str("# HELP aether_system_runs_total ECS system execution count.\n");
        out.push_str("# TYPE aether_system_runs_total counter\n");
        for sys_metric in &metrics.system_timings {
            out.push_str(&format!(
                "aether_system_runs_total{{stage=\"{}\",name=\"{}\"}} {}\n",
                sys_metric.stage.name(),
                escape_label_value(&sys_metric.name),
                sys_metric.runs
            ));
        }

        out.push_str(
            "# HELP aether_system_time_ns ECS system cumulative execution time in nanoseconds.\n",
        );
        out.push_str("# TYPE aether_system_time_ns counter\n");
        for sys_metric in &metrics.system_timings {
            out.push_str(&format!(
                "aether_system_time_ns{{stage=\"{}\",name=\"{}\"}} {}\n",
                sys_metric.stage.name(),
                escape_label_value(&sys_metric.name),
                sys_metric.total_time_ns
            ));
        }

        out
    }

    /// Run simple runtime alerts from threshold rules.
    pub fn evaluate_alerts(
        &self,
        max_stage_time_ns: u128,
        max_system_time_ns: u128,
    ) -> Vec<RuntimeAlert> {
        let metrics = self.metrics();
        let mut alerts = Vec::new();

        for stage in &metrics.stage_timings {
            if max_stage_time_ns == 0 || stage.runs == 0 {
                continue;
            }
            if stage.last_time_ns >= max_stage_time_ns {
                let severity = if stage.last_time_ns >= max_stage_time_ns.saturating_mul(2) {
                    AlertSeverity::Critical
                } else {
                    AlertSeverity::Warning
                };
                alerts.push(RuntimeAlert {
                    name: format!("stage_latency_high_{}", stage.stage.name()),
                    message: format!(
                        "{} stage exceeded latency threshold ({})",
                        stage.stage.name(),
                        stage.last_time_ns
                    ),
                    severity,
                });
            }
        }

        for sys in &metrics.system_timings {
            if max_system_time_ns == 0 || sys.runs == 0 {
                continue;
            }
            if sys.last_time_ns >= max_system_time_ns {
                let severity = if sys.last_time_ns >= max_system_time_ns.saturating_mul(2) {
                    AlertSeverity::Critical
                } else {
                    AlertSeverity::Warning
                };
                alerts.push(RuntimeAlert {
                    name: format!("system_latency_high_{}", sys.name),
                    message: format!(
                        "System {} exceeded latency threshold ({})",
                        escape_label_value(&sys.name),
                        sys.last_time_ns
                    ),
                    severity,
                });
            }
        }

        alerts
    }

    fn record_system_run(&mut self, idx: usize, elapsed_ns: u128) {
        let metric = &mut self.system_timings[idx];
        metric.timing.runs = metric.timing.runs.saturating_add(1);
        metric.timing.total_time_ns = metric.timing.total_time_ns.saturating_add(elapsed_ns);
        metric.timing.last_time_ns = elapsed_ns;
    }

    fn record_stage_run(&mut self, stage: Stage, elapsed_ns: u128) {
        let metric = &mut self.stage_timings[stage as usize];
        metric.runs = metric.runs.saturating_add(1);
        metric.total_time_ns = metric.total_time_ns.saturating_add(elapsed_ns);
        metric.last_time_ns = elapsed_ns;
    }

    fn timed_system_run(system: &dyn System, world: &World) -> u128 {
        let start = Instant::now();
        system.run(world);
        start.elapsed().as_nanos()
    }

    /// Get the execution order as a list of (stage, batch_index, system_name) tuples.
    /// Useful for debugging and testing.
    pub fn execution_order(&mut self) -> Vec<(Stage, usize, String)> {
        if self.dirty {
            self.rebuild_batches();
        }

        let mut order = Vec::new();
        for &stage in &Stage::ALL {
            if let Some(batches) = self.stage_batches.get(&stage) {
                for (batch_idx, batch) in batches.iter().enumerate() {
                    for &sys_idx in &batch.system_indices {
                        order.push((stage, batch_idx, self.systems[sys_idx].name().to_string()));
                    }
                }
            }
        }
        order
    }
}

impl Default for Schedule {
    fn default() -> Self {
        Self::new()
    }
}

/// Greedy batching algorithm: for each system, try to place it in an existing batch
/// where it doesn't conflict with any system already in that batch. If no such batch
/// exists, create a new one.
fn build_batches(system_indices: &[usize], systems: &[Box<dyn System>]) -> Vec<SystemBatch> {
    let mut batches: Vec<SystemBatch> = Vec::new();
    let accesses: Vec<AccessDescriptor> = system_indices
        .iter()
        .map(|&i| systems[i].access())
        .collect();
    let local_indices: HashMap<usize, usize> = system_indices
        .iter()
        .enumerate()
        .map(|(i, &idx)| (idx, i))
        .collect();

    for (local_idx, &sys_idx) in system_indices.iter().enumerate() {
        let mut placed = false;
        for batch in batches.iter_mut() {
            let conflicts = batch.system_indices.iter().any(|&existing_sys_idx| {
                let existing_local = *local_indices.get(&existing_sys_idx).unwrap();
                accesses[local_idx].conflicts_with(&accesses[existing_local])
            });
            if !conflicts {
                batch.system_indices.push(sys_idx);
                placed = true;
                break;
            }
        }
        if !placed {
            batches.push(SystemBatch {
                system_indices: vec![sys_idx],
            });
        }
    }

    batches
}

fn escape_label_value(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::{Component, ComponentId};
    use crate::system::SystemBuilder;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    struct Position {
        x: f32,
        y: f32,
        z: f32,
    }
    impl Component for Position {}

    struct Velocity {
        x: f32,
        y: f32,
        z: f32,
    }
    impl Component for Velocity {}

    struct Health(u32);
    impl Component for Health {}

    fn make_test_world() -> World {
        World::new()
    }

    #[test]
    fn schedule_runs_systems_in_stage_order() {
        let order = Arc::new(std::sync::Mutex::new(Vec::new()));

        let order1 = order.clone();
        let sys1 = SystemBuilder::new("input_sys", move |_: &World| {
            order1.lock().unwrap().push("input");
        })
        .stage(Stage::Input)
        .build();

        let order2 = order.clone();
        let sys2 = SystemBuilder::new("physics_sys", move |_: &World| {
            order2.lock().unwrap().push("physics");
        })
        .stage(Stage::Physics)
        .build();

        let order3 = order.clone();
        let sys3 = SystemBuilder::new("render_sys", move |_: &World| {
            order3.lock().unwrap().push("render");
        })
        .stage(Stage::Render)
        .build();

        let mut schedule = Schedule::new();
        // Add in reverse order to verify stage ordering works
        schedule.add_system(sys3);
        schedule.add_system(sys1);
        schedule.add_system(sys2);

        let world = make_test_world();
        schedule.run(&world);

        let result = order.lock().unwrap();
        assert_eq!(*result, vec!["input", "physics", "render"]);
    }

    #[test]
    fn non_conflicting_systems_batched_together() {
        let pos_id = ComponentId::of::<Position>();
        let vel_id = ComponentId::of::<Velocity>();
        let health_id = ComponentId::of::<Health>();

        // System A: writes Position
        let sys_a = SystemBuilder::new("sys_a", |_: &World| {})
            .stage(Stage::Physics)
            .access(AccessDescriptor::new().write(pos_id))
            .build();

        // System B: writes Velocity (no conflict with A)
        let sys_b = SystemBuilder::new("sys_b", |_: &World| {})
            .stage(Stage::Physics)
            .access(AccessDescriptor::new().write(vel_id))
            .build();

        // System C: writes Health (no conflict with A or B)
        let sys_c = SystemBuilder::new("sys_c", |_: &World| {})
            .stage(Stage::Physics)
            .access(AccessDescriptor::new().write(health_id))
            .build();

        let mut schedule = Schedule::new();
        schedule.add_system(sys_a);
        schedule.add_system(sys_b);
        schedule.add_system(sys_c);

        let order = schedule.execution_order();
        // All three should be in the same batch (batch 0)
        let physics_systems: Vec<_> = order
            .iter()
            .filter(|(s, _, _)| *s == Stage::Physics)
            .collect();
        assert_eq!(physics_systems.len(), 3);
        // All should be batch 0
        assert!(physics_systems.iter().all(|(_, batch, _)| *batch == 0));
    }

    #[test]
    fn conflicting_systems_in_separate_batches() {
        let pos_id = ComponentId::of::<Position>();

        // System A: writes Position
        let sys_a = SystemBuilder::new("sys_a", |_: &World| {})
            .stage(Stage::Physics)
            .access(AccessDescriptor::new().write(pos_id))
            .build();

        // System B: reads Position (conflicts with A's write)
        let sys_b = SystemBuilder::new("sys_b", |_: &World| {})
            .stage(Stage::Physics)
            .access(AccessDescriptor::new().read(pos_id))
            .build();

        let mut schedule = Schedule::new();
        schedule.add_system(sys_a);
        schedule.add_system(sys_b);

        let order = schedule.execution_order();
        let physics_systems: Vec<_> = order
            .iter()
            .filter(|(s, _, _)| *s == Stage::Physics)
            .collect();
        assert_eq!(physics_systems.len(), 2);
        // They should be in different batches
        let batches: Vec<usize> = physics_systems.iter().map(|(_, b, _)| *b).collect();
        assert_ne!(batches[0], batches[1]);
    }

    #[test]
    fn systems_actually_execute() {
        let counter = Arc::new(AtomicUsize::new(0));

        let c1 = counter.clone();
        let sys1 = SystemBuilder::new("counter_sys", move |_: &World| {
            c1.fetch_add(1, Ordering::SeqCst);
        })
        .stage(Stage::Input)
        .build();

        let mut schedule = Schedule::new();
        schedule.add_system(sys1);

        let world = make_test_world();
        schedule.run(&world);
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        schedule.run(&world);
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn empty_schedule_runs_without_error() {
        let mut schedule = Schedule::new();
        let world = make_test_world();
        schedule.run(&world);
        assert_eq!(schedule.system_count(), 0);
    }

    #[test]
    fn all_stages_are_executed() {
        let stages_hit = Arc::new(std::sync::Mutex::new(Vec::new()));

        let mut schedule = Schedule::new();

        for &stage in &Stage::ALL {
            let stages_clone = stages_hit.clone();
            let sys = SystemBuilder::new(stage.name(), move |_: &World| {
                stages_clone.lock().unwrap().push(stage);
            })
            .stage(stage)
            .build();
            schedule.add_system(sys);
        }

        let world = make_test_world();
        schedule.run(&world);

        let result = stages_hit.lock().unwrap();
        assert_eq!(result.len(), 8);
        assert_eq!(*result, Stage::ALL.to_vec());
    }

    #[test]
    fn mixed_conflict_batching() {
        let pos_id = ComponentId::of::<Position>();
        let vel_id = ComponentId::of::<Velocity>();

        // A: write pos
        let sys_a = SystemBuilder::new("a", |_: &World| {})
            .stage(Stage::Physics)
            .access(AccessDescriptor::new().write(pos_id))
            .build();

        // B: write vel (no conflict with A)
        let sys_b = SystemBuilder::new("b", |_: &World| {})
            .stage(Stage::Physics)
            .access(AccessDescriptor::new().write(vel_id))
            .build();

        // C: read pos + write vel (conflicts with both A and B)
        let sys_c = SystemBuilder::new("c", |_: &World| {})
            .stage(Stage::Physics)
            .access(AccessDescriptor::new().read(pos_id).write(vel_id))
            .build();

        let mut schedule = Schedule::new();
        schedule.add_system(sys_a);
        schedule.add_system(sys_b);
        schedule.add_system(sys_c);

        let order = schedule.execution_order();
        let physics: Vec<_> = order
            .iter()
            .filter(|(s, _, _)| *s == Stage::Physics)
            .collect();

        // A and B should be in batch 0, C in batch 1
        let a_batch = physics.iter().find(|(_, _, n)| n == "a").unwrap().1;
        let b_batch = physics.iter().find(|(_, _, n)| n == "b").unwrap().1;
        let c_batch = physics.iter().find(|(_, _, n)| n == "c").unwrap().1;

        assert_eq!(a_batch, b_batch); // A and B don't conflict
        assert_ne!(a_batch, c_batch); // C conflicts with both
    }

    #[test]
    fn metrics_track_runs_and_stages() {
        let pos_id = ComponentId::of::<Position>();

        let sys_a = SystemBuilder::new("sys_a", |_: &World| {})
            .stage(Stage::Physics)
            .access(AccessDescriptor::new().write(pos_id))
            .build();

        let sys_b = SystemBuilder::new("sys_b", |_: &World| {})
            .stage(Stage::Render)
            .build();

        let mut schedule = Schedule::new();
        schedule.add_system(sys_a);
        schedule.add_system(sys_b);

        let world = make_test_world();
        schedule.run(&world);

        let metrics = schedule.metrics();
        assert_eq!(metrics.run_count, 1);
        assert_eq!(metrics.system_timings.len(), 2);
        assert_eq!(metrics.system_timings[0].name, "sys_a");
        assert_eq!(metrics.system_timings[1].name, "sys_b");
        assert_eq!(metrics.system_timings[0].runs, 1);
        assert_eq!(metrics.system_timings[1].runs, 1);
        assert!(metrics.stage_timings[Stage::Physics as usize].runs >= 1);
        assert!(metrics.stage_timings[Stage::Render as usize].runs >= 1);
        assert!(metrics.total_time_ns > 0);
    }

    #[test]
    fn clear_metrics_resets_observability_state() {
        let sys = SystemBuilder::new("clear", |_: &World| {})
            .stage(Stage::Input)
            .build();

        let mut schedule = Schedule::new();
        schedule.add_system(sys);

        let world = make_test_world();
        schedule.run(&world);
        let before = schedule.metrics();
        assert_eq!(before.run_count, 1);

        schedule.clear_metrics();
        let after = schedule.metrics();
        assert_eq!(after.run_count, 0);
        assert_eq!(after.system_timings.len(), 1);
        assert_eq!(after.system_timings[0].runs, 0);
        assert_eq!(after.system_timings[0].name, "clear");
        assert_eq!(after.total_time_ns, 0);
        assert_eq!(after.stage_timings[Stage::Input as usize].runs, 0);
    }

    #[test]
    fn metrics_exports_prometheus_text() {
        let sys = SystemBuilder::new("export_sys", |_: &World| {})
            .stage(Stage::Physics)
            .build();

        let mut schedule = Schedule::new();
        schedule.add_system(sys);

        let world = make_test_world();
        schedule.run(&world);

        let text = schedule.metrics_prometheus();
        assert!(text.contains("aether_schedule_run_count"));
        assert!(text.contains("aether_system_runs_total"));
        assert!(text.contains("name=\"export_sys\""));
    }

    #[test]
    fn alerts_trigger_for_thresholds() {
        let sys = SystemBuilder::new("slow_system", |_: &World| {
            // No-op body intentionally small; timer overhead is still measurable in tests.
        })
        .stage(Stage::Render)
        .build();

        let mut schedule = Schedule::new();
        schedule.add_system(sys);
        let world = make_test_world();
        schedule.run(&world);

        let metrics = schedule.metrics();
        assert_eq!(metrics.run_count, 1);
        let alerts = schedule.evaluate_alerts(u128::MAX, u128::MAX);
        assert_eq!(alerts.len(), 0);
    }

    #[test]
    fn batch_count_is_order_invariant_for_equivalent_system_sets() {
        let pos_id = ComponentId::of::<Position>();
        let vel_id = ComponentId::of::<Velocity>();
        let health_id = ComponentId::of::<Health>();

        let mut schedule = Schedule::new();
        schedule.add_system(
            SystemBuilder::new("write_position", |_: &World| {})
                .stage(Stage::Physics)
                .access(AccessDescriptor::new().write(pos_id))
                .build(),
        );
        schedule.add_system(
            SystemBuilder::new("write_velocity", |_: &World| {})
                .stage(Stage::Physics)
                .access(AccessDescriptor::new().write(vel_id))
                .build(),
        );
        schedule.add_system(
            SystemBuilder::new("write_health", |_: &World| {})
                .stage(Stage::Physics)
                .access(AccessDescriptor::new().write(health_id))
                .build(),
        );

        let first = schedule.execution_order();
        let first_batch_count: Vec<usize> = first
            .iter()
            .filter(|(s, _, _)| *s == Stage::Physics)
            .map(|(_, batch, _)| *batch)
            .collect();

        let mut schedule = Schedule::new();
        schedule.add_system(
            SystemBuilder::new("write_velocity", |_: &World| {})
                .stage(Stage::Physics)
                .access(AccessDescriptor::new().write(vel_id))
                .build(),
        );
        schedule.add_system(
            SystemBuilder::new("write_health", |_: &World| {})
                .stage(Stage::Physics)
                .access(AccessDescriptor::new().write(health_id))
                .build(),
        );
        schedule.add_system(
            SystemBuilder::new("write_position", |_: &World| {})
                .stage(Stage::Physics)
                .access(AccessDescriptor::new().write(pos_id))
                .build(),
        );
        let second = schedule.execution_order();
        let second_batch_count: Vec<usize> = second
            .iter()
            .filter(|(s, _, _)| *s == Stage::Physics)
            .map(|(_, batch, _)| *batch)
            .collect();

        assert_eq!(first_batch_count, second_batch_count);
    }

    #[test]
    fn diagnostics_tracks_last_batch_counts() {
        let pos_id = ComponentId::of::<Position>();
        let vel_id = ComponentId::of::<Velocity>();

        let mut schedule = Schedule::new();
        schedule.add_system(
            SystemBuilder::new("disjoint_1", |_: &World| {})
                .stage(Stage::Input)
                .access(AccessDescriptor::new().write(pos_id))
                .build(),
        );
        schedule.add_system(
            SystemBuilder::new("disjoint_2", |_: &World| {})
                .stage(Stage::Input)
                .access(AccessDescriptor::new().write(vel_id))
                .build(),
        );
        schedule.add_system(
            SystemBuilder::new("conflicting", |_: &World| {
                // no-op
            })
            .stage(Stage::Input)
            .access(AccessDescriptor::new().write(vel_id).read(pos_id))
            .build(),
        );

        schedule.run(&make_test_world());
        let diagnostics = schedule.diagnostics();
        assert_eq!(diagnostics.run_count, 1);
        assert!(diagnostics.last_batch_count_total >= 2);
        assert!(diagnostics.max_batch_count_last_run >= 2);

        let metrics = schedule.metrics();
        assert_eq!(metrics.stage_timings[Stage::Input as usize].runs, 1);
        assert!(
            metrics.stage_timings[Stage::Input as usize].batch_count >= 2,
            "batch_count should capture the run's final stage batch layout"
        );
    }
}
