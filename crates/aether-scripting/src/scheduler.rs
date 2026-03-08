use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::config::ScriptRuntimeLimits;
use crate::rate_limit::RateLimiter;

pub type ScriptId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptRuntime {
    Wasm,
    Lua,
}

impl Default for ScriptRuntime {
    fn default() -> Self {
        ScriptRuntime::Wasm
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptDescriptor {
    pub id: ScriptId,
    pub name: String,
    pub priority: u8,
    pub cpu_budget_per_tick: Duration,
    pub memory_bytes: u64,
    pub initial_entities: u32,
    pub runtime: ScriptRuntime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptState {
    Running,
    Suspended,
}

#[derive(Debug)]
pub struct RuntimeScript {
    pub id: ScriptId,
    pub name: String,
    pub base_priority: u8,
    pub age_bonus: u16,
    pub cpu_budget_per_tick: Duration,
    pub memory_bytes: u64,
    pub scripted_entities: u32,
    pub state: ScriptState,
    pub last_cpu_used: Duration,
    pub spawn_limit: RateLimiter,
    pub rpc_limit: RateLimiter,
    pub storage_write_limit: RateLimiter,
}

impl RuntimeScript {
    pub fn effective_priority(&self) -> u16 {
        self.base_priority as u16 + self.age_bonus
    }
}

#[derive(Debug)]
pub struct ScriptExecutionUsage {
    pub script_id: ScriptId,
    pub cpu_used: Duration,
}

#[derive(Debug)]
pub struct WorldTick {
    pub tick: u64,
    pub planned_at: Instant,
    pub selected: Vec<ScriptId>,
    pub deferred: Vec<ScriptId>,
    pub predicted_cpu: Duration,
    pub world_budget: Duration,
}

impl WorldTick {
    pub fn deferred_to_run_next(&self, limit: Option<usize>) -> Vec<ScriptId> {
        self.deferred.iter().take(limit.unwrap_or(self.deferred.len())).cloned().collect()
    }
}

#[derive(Debug, Default)]
pub struct TickUsageResult {
    pub tick: u64,
    pub total_cpu_used: Duration,
    pub over_budget_duration: Duration,
    pub force_suspended: Vec<ScriptId>,
}

#[derive(Debug)]
pub struct SchedulerLimitsError {
    pub reason: &'static str,
    pub used: u64,
    pub limit: u64,
}

#[derive(Debug)]
pub enum SchedulerError {
    NotFound(ScriptId),
    DuplicateScript(ScriptId),
    ScriptLimitExceeded { script_id: ScriptId, reason: &'static str },
    WorldLimitExceeded(SchedulerLimitsError),
    ScriptSuspended(ScriptId),
    WorldFull(String),
}

pub type Result<T> = std::result::Result<T, SchedulerError>;

#[derive(Debug)]
pub struct WorldScriptScheduler {
    limits: ScriptRuntimeLimits,
    scripts: HashMap<ScriptId, RuntimeScript>,
    running_world_entities: u32,
    running_world_memory: u64,
    overload_accumulator: Duration,
    tick_index: u64,
    last_tick_time: Option<Instant>,
}

impl Default for WorldScriptScheduler {
    fn default() -> Self {
        Self::new(ScriptRuntimeLimits::default())
    }
}

impl WorldScriptScheduler {
    pub fn new(limits: ScriptRuntimeLimits) -> Self {
        Self {
            limits,
            scripts: HashMap::new(),
            running_world_entities: 0,
            running_world_memory: 0,
            overload_accumulator: Duration::ZERO,
            tick_index: 0,
            last_tick_time: None,
        }
    }

    pub fn limits(&self) -> &ScriptRuntimeLimits {
        &self.limits
    }

    pub fn active_script_count(&self) -> usize {
        self.scripts.len()
    }

    pub fn register_script(&mut self, descriptor: ScriptDescriptor, now: Instant) -> Result<()> {
        if self.scripts.contains_key(&descriptor.id) {
            return Err(SchedulerError::DuplicateScript(descriptor.id));
        }

        let per_script = &self.limits.per_script;
        if descriptor.cpu_budget_per_tick > per_script.cpu_per_tick {
            return Err(SchedulerError::ScriptLimitExceeded {
                script_id: descriptor.id,
                reason: "CPU per tick exceeds configured per-script limit",
            });
        }
        if descriptor.memory_bytes > per_script.memory_bytes {
            return Err(SchedulerError::ScriptLimitExceeded {
                script_id: descriptor.id,
                reason: "script memory exceeds configured per-script memory limit",
            });
        }
        if self.running_world_memory + descriptor.memory_bytes > self.limits.per_world.max_script_memory_bytes {
            return Err(SchedulerError::WorldLimitExceeded(SchedulerLimitsError {
                reason: "script memory",
                used: self.running_world_memory + descriptor.memory_bytes,
                limit: self.limits.per_world.max_script_memory_bytes,
            }));
        }
        if self.running_world_entities + descriptor.initial_entities > self.limits.per_world.max_scripted_entities {
            return Err(SchedulerError::WorldLimitExceeded(SchedulerLimitsError {
                reason: "script entities",
                used: (self.running_world_entities + descriptor.initial_entities) as u64,
                limit: self.limits.per_world.max_scripted_entities as u64,
            }));
        }

        let script = RuntimeScript {
            id: descriptor.id,
            name: descriptor.name,
            base_priority: descriptor.priority,
            age_bonus: 0,
            cpu_budget_per_tick: descriptor.cpu_budget_per_tick,
            memory_bytes: descriptor.memory_bytes,
            scripted_entities: descriptor.initial_entities,
            state: ScriptState::Running,
            last_cpu_used: Duration::ZERO,
            spawn_limit: RateLimiter::new(per_script.entity_spawns_per_sec, now),
            rpc_limit: RateLimiter::new(per_script.network_rpcs_per_sec, now),
            storage_write_limit: RateLimiter::new(per_script.storage_writes_per_sec, now),
        };

        self.running_world_memory += descriptor.memory_bytes;
        self.running_world_entities += descriptor.initial_entities;
        self.scripts.insert(descriptor.id, script);
        Ok(())
    }

    pub fn remove_script(&mut self, id: ScriptId) -> Option<RuntimeScript> {
        if let Some(script) = self.scripts.remove(&id) {
            self.running_world_memory = self.running_world_memory.saturating_sub(script.memory_bytes);
            self.running_world_entities = self.running_world_entities.saturating_sub(script.scripted_entities);
            return Some(script);
        }
        None
    }

    pub fn script_state(&self, id: ScriptId) -> Option<ScriptState> {
        self.scripts.get(&id).map(|s| s.state)
    }

    pub fn set_script_state(&mut self, id: ScriptId, state: ScriptState) -> Result<()> {
        let script = self
            .scripts
            .get_mut(&id)
            .ok_or(SchedulerError::NotFound(id))?;
        script.state = state;
        if matches!(state, ScriptState::Suspended) {
            script.age_bonus = 0;
        }
        Ok(())
    }

    pub fn plan_tick(&mut self, now: Instant) -> WorldTick {
        self.tick_index += 1;
        let mut candidates: Vec<&mut RuntimeScript> = self
            .scripts
            .values_mut()
            .filter(|script| matches!(script.state, ScriptState::Running))
            .collect();

        candidates.sort_by(|a, b| b.effective_priority().cmp(&a.effective_priority()));

        let mut selected = Vec::new();
        let mut deferred = Vec::new();
        let mut predicted_cpu = Duration::ZERO;

        for script in candidates {
            if predicted_cpu + script.cpu_budget_per_tick <= self.limits.per_world.cpu_budget_per_tick {
                predicted_cpu += script.cpu_budget_per_tick;
                selected.push(script.id);
                script.age_bonus = 0;
            } else {
                deferred.push(script.id);
                script.age_bonus = script.age_bonus.saturating_add(1);
            }
        }

        WorldTick {
            tick: self.tick_index,
            planned_at: now,
            selected,
            deferred,
            predicted_cpu,
            world_budget: self.limits.per_world.cpu_budget_per_tick,
        }
    }

    pub fn record_usage(
        &mut self,
        now: Instant,
        report: &[ScriptExecutionUsage],
    ) -> TickUsageResult {
        let total_cpu = report.iter().map(|entry| entry.cpu_used).sum();

        if let Some(script) = report.iter().find_map(|entry| self.scripts.get(&entry.script_id)) {
            let _ = script;
        }
        for entry in report {
            if let Some(script) = self.scripts.get_mut(&entry.script_id) {
                script.last_cpu_used = entry.cpu_used;
            }
        }

        let dt = self
            .last_tick_time
            .map(|last| now.saturating_duration_since(last))
            .unwrap_or(Duration::ZERO);
        self.last_tick_time = Some(now);

        if total_cpu > self.limits.per_world.cpu_budget_per_tick {
            self.overload_accumulator += dt;
        } else {
            self.overload_accumulator = Duration::ZERO;
        }

        let mut force_suspended = Vec::new();
        if self.overload_accumulator >= self.limits.per_world.overload_window {
            if let Some(suspended_id) = self.lowest_priority_running_script() {
                if let Some(script) = self.scripts.get_mut(&suspended_id) {
                    script.state = ScriptState::Suspended;
                }
                force_suspended.push(suspended_id);
            }
            self.overload_accumulator = Duration::ZERO;
        }

        TickUsageResult {
            tick: self.tick_index,
            total_cpu_used: total_cpu,
            over_budget_duration: self.overload_accumulator,
            force_suspended,
        }
    }

    fn lowest_priority_running_script(&self) -> Option<ScriptId> {
        self.scripts
            .iter()
            .filter_map(|(id, script)| {
                if matches!(script.state, ScriptState::Running) {
                    Some((*id, script.effective_priority(), script.base_priority))
                } else {
                    None
                }
            })
            .min_by(|a, b| {
                a.1
                    .cmp(&b.1)
                    .then_with(|| a.2.cmp(&b.2))
                    .then_with(|| a.0.cmp(&b.0))
            })
            .map(|(id, _, _)| id)
    }

    pub fn try_spawn_entities(
        &mut self,
        now: Instant,
        script_id: ScriptId,
        count: u32,
    ) -> Result<()> {
        let script = self
            .scripts
            .get_mut(&script_id)
            .ok_or(SchedulerError::NotFound(script_id))?;
        if script.scripted_entities + count > u32::MAX {
            return Err(SchedulerError::WorldFull("entity request overflow".to_string()));
        }
        if matches!(script.state, ScriptState::Suspended) {
            return Err(SchedulerError::ScriptSuspended(script_id));
        }
        if !script.spawn_limit.try_take(now, count) {
            return Err(SchedulerError::ScriptLimitExceeded {
                script_id,
                reason: "entity spawn rate limit exceeded",
            });
        }
        if self.running_world_entities + count > self.limits.per_world.max_scripted_entities {
            return Err(SchedulerError::WorldLimitExceeded(SchedulerLimitsError {
                reason: "global scripted entity cap",
                used: (self.running_world_entities + count) as u64,
                limit: self.limits.per_world.max_scripted_entities as u64,
            }));
        }

        script.scripted_entities += count;
        self.running_world_entities += count;
        Ok(())
    }

    pub fn try_emit_network_rpc(
        &mut self,
        now: Instant,
        script_id: ScriptId,
        count: u32,
    ) -> Result<()> {
        let script = self
            .scripts
            .get_mut(&script_id)
            .ok_or(SchedulerError::NotFound(script_id))?;
        if matches!(script.state, ScriptState::Suspended) {
            return Err(SchedulerError::ScriptSuspended(script_id));
        }
        if !script.rpc_limit.try_take(now, count) {
            return Err(SchedulerError::ScriptLimitExceeded {
                script_id,
                reason: "network RPC rate limit exceeded",
            });
        }
        Ok(())
    }

    pub fn script_cpu_budget(&self, id: ScriptId) -> Duration {
        self.scripts
            .get(&id)
            .map(|s| s.cpu_budget_per_tick)
            .unwrap_or(Duration::from_millis(5))
    }

    pub fn try_write_storage(
        &mut self,
        now: Instant,
        script_id: ScriptId,
        count: u32,
    ) -> Result<()> {
        let script = self
            .scripts
            .get_mut(&script_id)
            .ok_or(SchedulerError::NotFound(script_id))?;
        if matches!(script.state, ScriptState::Suspended) {
            return Err(SchedulerError::ScriptSuspended(script_id));
        }
        if !script.storage_write_limit.try_take(now, count) {
            return Err(SchedulerError::ScriptLimitExceeded {
                script_id,
                reason: "storage write rate limit exceeded",
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_scheduler() -> (WorldScriptScheduler, Instant) {
        (
            WorldScriptScheduler::new(ScriptRuntimeLimits::default()),
            Instant::now(),
        )
    }

    #[test]
    fn scheduler_prioritizes_by_priority_and_ages_starved_scripts() {
        let (mut scheduler, now) = default_scheduler();
        let p1 = ScriptDescriptor {
            id: 1,
            name: "high".to_string(),
            priority: 200,
            cpu_budget_per_tick: Duration::from_millis(3),
            memory_bytes: 1024,
            initial_entities: 0,
            runtime: ScriptRuntime::Wasm,
        };
        let p2 = ScriptDescriptor {
            id: 2,
            name: "mid".to_string(),
            priority: 100,
            cpu_budget_per_tick: Duration::from_millis(3),
            memory_bytes: 1024,
            initial_entities: 0,
            runtime: ScriptRuntime::Wasm,
        };
        let p3 = ScriptDescriptor {
            id: 3,
            name: "low".to_string(),
            priority: 50,
            cpu_budget_per_tick: Duration::from_millis(3),
            memory_bytes: 1024,
            initial_entities: 0,
            runtime: ScriptRuntime::Wasm,
        };
        scheduler.register_script(p1, now).unwrap();
        scheduler.register_script(p2, now).unwrap();
        scheduler.register_script(p3, now).unwrap();

        let tick = scheduler.plan_tick(now);
        assert_eq!(tick.selected, vec![1, 2]);
        assert_eq!(tick.deferred, vec![3]);

        let tick2 = scheduler.plan_tick(now);
        assert_eq!(tick2.deferred, vec![3]);
        let script3 = scheduler.scripts.get(&3).expect("script 3 exists");
        assert_eq!(script3.age_bonus, 2);
        assert_eq!(script3.effective_priority(), 52);
    }

    #[test]
    fn overloaded_world_suspends_lowest_priority_script() {
        let (mut scheduler, now) = default_scheduler();
        let descriptor = ScriptDescriptor {
            id: 1,
            name: "busy".to_string(),
            priority: 200,
            cpu_budget_per_tick: Duration::from_millis(3),
            memory_bytes: 1024,
            initial_entities: 0,
            runtime: ScriptRuntime::Wasm,
        };
        scheduler.register_script(descriptor, now).unwrap();

        let mut report_time = now;
        for _ in 0..11 {
            report_time += Duration::from_millis(1000);
            let usage = [ScriptExecutionUsage {
                script_id: 1,
                cpu_used: Duration::from_millis(12),
            }];
            let result = scheduler.record_usage(report_time, &usage);
            if result.force_suspended.len() == 1 {
                assert_eq!(result.force_suspended[0], 1);
                break;
            }
            assert!(report_time - now <= Duration::from_secs(12));
        }

        assert_eq!(scheduler.script_state(1), Some(ScriptState::Suspended));
    }

    #[test]
    fn per_script_rate_limits_are_enforced() {
        let (mut scheduler, now) = default_scheduler();
        let descriptor = ScriptDescriptor {
            id: 7,
            name: "rpc-heavy".to_string(),
            priority: 128,
            cpu_budget_per_tick: Duration::from_millis(3),
            memory_bytes: 1024,
            initial_entities: 0,
            runtime: ScriptRuntime::Wasm,
        };
        scheduler.register_script(descriptor, now).unwrap();

        for _ in 0..50 {
            assert!(scheduler
                .try_emit_network_rpc(now, 7, 1)
                .is_ok());
        }
        assert!(scheduler.try_emit_network_rpc(now, 7, 1).is_err());

        let after_one_second = now + Duration::from_secs(1);
        assert!(scheduler
            .try_emit_network_rpc(after_one_second, 7, 1)
            .is_ok());
    }
}
