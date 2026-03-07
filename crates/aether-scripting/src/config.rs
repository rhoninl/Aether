use std::time::Duration;

pub const MB: u64 = 1024 * 1024;

pub const DEFAULT_PER_SCRIPT_CPU_LIMIT: Duration = Duration::from_millis(5);
pub const DEFAULT_PER_SCRIPT_MEMORY_BYTES: u64 = 64 * MB;
pub const DEFAULT_PER_SCRIPT_ENTITY_SPAWNS_PER_SECOND: u32 = 100;
pub const DEFAULT_PER_SCRIPT_NETWORK_RPCS_PER_SECOND: u32 = 50;
pub const DEFAULT_PER_SCRIPT_STORAGE_WRITES_PER_SECOND: u32 = 10;

pub const DEFAULT_WORLD_CPU_BUDGET_MS: u64 = 8;
pub const DEFAULT_WORLD_MAX_SCRIPT_MEMORY_BYTES: u64 = 512 * MB;
pub const DEFAULT_WORLD_MAX_SCRIPTED_ENTITIES: u32 = 10_000;
pub const DEFAULT_WORLD_OVERLOAD_WINDOW: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptResourceLimits {
    pub cpu_per_tick: Duration,
    pub memory_bytes: u64,
    pub entity_spawns_per_sec: u32,
    pub network_rpcs_per_sec: u32,
    pub storage_writes_per_sec: u32,
}

impl Default for ScriptResourceLimits {
    fn default() -> Self {
        Self {
            cpu_per_tick: DEFAULT_PER_SCRIPT_CPU_LIMIT,
            memory_bytes: DEFAULT_PER_SCRIPT_MEMORY_BYTES,
            entity_spawns_per_sec: DEFAULT_PER_SCRIPT_ENTITY_SPAWNS_PER_SECOND,
            network_rpcs_per_sec: DEFAULT_PER_SCRIPT_NETWORK_RPCS_PER_SECOND,
            storage_writes_per_sec: DEFAULT_PER_SCRIPT_STORAGE_WRITES_PER_SECOND,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldScriptLimits {
    pub cpu_budget_per_tick: Duration,
    pub max_script_memory_bytes: u64,
    pub max_scripted_entities: u32,
    pub overload_window: Duration,
}

impl Default for WorldScriptLimits {
    fn default() -> Self {
        Self {
            cpu_budget_per_tick: Duration::from_millis(DEFAULT_WORLD_CPU_BUDGET_MS),
            max_script_memory_bytes: DEFAULT_WORLD_MAX_SCRIPT_MEMORY_BYTES,
            max_scripted_entities: DEFAULT_WORLD_MAX_SCRIPTED_ENTITIES,
            overload_window: DEFAULT_WORLD_OVERLOAD_WINDOW,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptRuntimeLimits {
    pub per_script: ScriptResourceLimits,
    pub per_world: WorldScriptLimits,
}

impl ScriptRuntimeLimits {
    pub fn with_cpu_and_memory(cpu_per_tick: Duration, max_memory: u64) -> Self {
        Self {
            per_script: ScriptResourceLimits {
                cpu_per_tick,
                memory_bytes: max_memory,
                ..ScriptResourceLimits::default()
            },
            per_world: WorldScriptLimits {
                ..WorldScriptLimits::default()
            },
        }
    }
}

impl Default for ScriptRuntimeLimits {
    fn default() -> Self {
        Self {
            per_script: ScriptResourceLimits::default(),
            per_world: WorldScriptLimits::default(),
        }
    }
}
