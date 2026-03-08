//! Server-side resource enforcement for WASM script execution.
//!
//! Defines per-script resource policies (CPU time, memory caps, fuel budgets,
//! syscall restrictions) and a metering system to track usage and enforce limits
//! during execution.

use std::time::Duration;

/// Default fuel budget per script execution on server.
const DEFAULT_SERVER_FUEL: u64 = 1_000_000;

/// Default memory cap per script on server (64 MB).
const DEFAULT_SERVER_MEMORY_BYTES: u64 = 64 * 1024 * 1024;

/// Default CPU time limit per script per tick (5 ms).
const DEFAULT_SERVER_CPU_LIMIT_MS: u64 = 5;

/// Default maximum number of WASM table entries.
const DEFAULT_MAX_TABLE_ENTRIES: u32 = 10_000;

/// Default maximum number of WASM instances per module.
const DEFAULT_MAX_INSTANCES: u32 = 1;

/// Categories of syscall-like operations that scripts may attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyscallCategory {
    /// Filesystem access (read/write files).
    Filesystem,
    /// Network access (sockets, HTTP).
    Network,
    /// System clock / time queries.
    Clock,
    /// Random number generation.
    Random,
    /// Environment variable access.
    Environment,
}

/// Policy for syscall access. By default, all syscalls are denied on the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyscallPolicy {
    allowed: Vec<SyscallCategory>,
}

impl Default for SyscallPolicy {
    fn default() -> Self {
        Self {
            allowed: Vec::new(),
        }
    }
}

impl SyscallPolicy {
    /// Creates a policy that allows the specified syscall categories.
    pub fn allow(categories: Vec<SyscallCategory>) -> Self {
        Self {
            allowed: categories,
        }
    }

    /// Creates a fully restrictive policy (no syscalls allowed).
    pub fn deny_all() -> Self {
        Self::default()
    }

    /// Checks whether a syscall category is allowed.
    pub fn is_allowed(&self, category: SyscallCategory) -> bool {
        self.allowed.contains(&category)
    }

    /// Returns the list of allowed syscall categories.
    pub fn allowed_categories(&self) -> &[SyscallCategory] {
        &self.allowed
    }
}

/// Per-script resource policy enforced on the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerResourcePolicy {
    /// Maximum CPU time per tick for this script.
    pub cpu_limit: Duration,
    /// Maximum linear memory in bytes.
    pub memory_bytes: u64,
    /// Fuel budget (instruction count) per execution.
    pub fuel_budget: u64,
    /// Maximum WASM table entries.
    pub max_table_entries: u32,
    /// Maximum WASM instances per module.
    pub max_instances: u32,
    /// Syscall access policy.
    pub syscall_policy: SyscallPolicy,
}

impl Default for ServerResourcePolicy {
    fn default() -> Self {
        Self {
            cpu_limit: Duration::from_millis(DEFAULT_SERVER_CPU_LIMIT_MS),
            memory_bytes: DEFAULT_SERVER_MEMORY_BYTES,
            fuel_budget: DEFAULT_SERVER_FUEL,
            max_table_entries: DEFAULT_MAX_TABLE_ENTRIES,
            max_instances: DEFAULT_MAX_INSTANCES,
            syscall_policy: SyscallPolicy::deny_all(),
        }
    }
}

impl ServerResourcePolicy {
    /// Creates a policy with custom CPU and memory limits.
    pub fn with_cpu_and_memory(cpu_limit: Duration, memory_bytes: u64) -> Self {
        Self {
            cpu_limit,
            memory_bytes,
            ..Self::default()
        }
    }

    /// Creates a minimal policy suitable for testing.
    pub fn minimal() -> Self {
        Self {
            cpu_limit: Duration::from_millis(1),
            memory_bytes: 1024 * 1024,
            fuel_budget: 10_000,
            max_table_entries: 100,
            max_instances: 1,
            syscall_policy: SyscallPolicy::deny_all(),
        }
    }

    /// Creates a policy from environment variables, falling back to defaults.
    pub fn from_env() -> Self {
        let fuel = std::env::var("AETHER_SERVER_DEFAULT_FUEL")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_SERVER_FUEL);

        let memory_mb = std::env::var("AETHER_SERVER_DEFAULT_MEMORY_MB")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(DEFAULT_SERVER_MEMORY_BYTES / (1024 * 1024));

        let cpu_ms = std::env::var("AETHER_SERVER_CPU_LIMIT_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_SERVER_CPU_LIMIT_MS);

        Self {
            cpu_limit: Duration::from_millis(cpu_ms),
            memory_bytes: memory_mb * 1024 * 1024,
            fuel_budget: fuel,
            ..Self::default()
        }
    }
}

/// Outcome of a metered execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MeteringOutcome {
    /// Execution completed within all resource budgets.
    Completed {
        fuel_consumed: u64,
        peak_memory_bytes: u64,
    },
    /// Execution was terminated because a resource budget was exceeded.
    Terminated {
        reason: MeteringTerminationReason,
        fuel_consumed: u64,
        peak_memory_bytes: u64,
    },
}

/// Reason why a metered execution was terminated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MeteringTerminationReason {
    /// Fuel (instruction budget) was exhausted.
    FuelExhausted,
    /// CPU time limit was exceeded.
    CpuTimeExceeded,
    /// Memory limit was exceeded.
    MemoryLimitExceeded,
    /// A denied syscall was attempted.
    SyscallDenied(SyscallCategory),
}

impl std::fmt::Display for MeteringTerminationReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FuelExhausted => write!(f, "fuel exhausted"),
            Self::CpuTimeExceeded => write!(f, "CPU time exceeded"),
            Self::MemoryLimitExceeded => write!(f, "memory limit exceeded"),
            Self::SyscallDenied(cat) => write!(f, "syscall denied: {cat:?}"),
        }
    }
}

/// Tracks resource consumption during a script execution.
#[derive(Debug)]
pub struct ResourceMeter {
    policy: ServerResourcePolicy,
    fuel_consumed: u64,
    peak_memory_bytes: u64,
    cpu_elapsed: Duration,
    terminated: Option<MeteringTerminationReason>,
}

impl ResourceMeter {
    /// Creates a new resource meter with the given policy.
    pub fn new(policy: ServerResourcePolicy) -> Self {
        Self {
            policy,
            fuel_consumed: 0,
            peak_memory_bytes: 0,
            cpu_elapsed: Duration::ZERO,
            terminated: None,
        }
    }

    /// Returns the resource policy being enforced.
    pub fn policy(&self) -> &ServerResourcePolicy {
        &self.policy
    }

    /// Returns the fuel consumed so far.
    pub fn fuel_consumed(&self) -> u64 {
        self.fuel_consumed
    }

    /// Returns the peak memory usage so far.
    pub fn peak_memory_bytes(&self) -> u64 {
        self.peak_memory_bytes
    }

    /// Returns whether execution has been terminated.
    pub fn is_terminated(&self) -> bool {
        self.terminated.is_some()
    }

    /// Records fuel consumption. Returns `Err` with termination reason if budget exceeded.
    pub fn record_fuel(&mut self, amount: u64) -> Result<(), MeteringTerminationReason> {
        if self.terminated.is_some() {
            return Err(self.terminated.clone().unwrap());
        }

        self.fuel_consumed += amount;
        if self.fuel_consumed > self.policy.fuel_budget {
            let reason = MeteringTerminationReason::FuelExhausted;
            self.terminated = Some(reason.clone());
            return Err(reason);
        }
        Ok(())
    }

    /// Records memory usage. Returns `Err` with termination reason if budget exceeded.
    pub fn record_memory(&mut self, current_bytes: u64) -> Result<(), MeteringTerminationReason> {
        if self.terminated.is_some() {
            return Err(self.terminated.clone().unwrap());
        }

        if current_bytes > self.peak_memory_bytes {
            self.peak_memory_bytes = current_bytes;
        }
        if current_bytes > self.policy.memory_bytes {
            let reason = MeteringTerminationReason::MemoryLimitExceeded;
            self.terminated = Some(reason.clone());
            return Err(reason);
        }
        Ok(())
    }

    /// Records CPU elapsed time. Returns `Err` with termination reason if budget exceeded.
    pub fn record_cpu_time(
        &mut self,
        elapsed: Duration,
    ) -> Result<(), MeteringTerminationReason> {
        if self.terminated.is_some() {
            return Err(self.terminated.clone().unwrap());
        }

        self.cpu_elapsed = elapsed;
        if self.cpu_elapsed > self.policy.cpu_limit {
            let reason = MeteringTerminationReason::CpuTimeExceeded;
            self.terminated = Some(reason.clone());
            return Err(reason);
        }
        Ok(())
    }

    /// Checks whether a syscall is allowed by the policy.
    pub fn check_syscall(
        &mut self,
        category: SyscallCategory,
    ) -> Result<(), MeteringTerminationReason> {
        if self.terminated.is_some() {
            return Err(self.terminated.clone().unwrap());
        }

        if !self.policy.syscall_policy.is_allowed(category) {
            let reason = MeteringTerminationReason::SyscallDenied(category);
            self.terminated = Some(reason.clone());
            return Err(reason);
        }
        Ok(())
    }

    /// Produces the final metering outcome.
    pub fn outcome(&self) -> MeteringOutcome {
        match &self.terminated {
            Some(reason) => MeteringOutcome::Terminated {
                reason: reason.clone(),
                fuel_consumed: self.fuel_consumed,
                peak_memory_bytes: self.peak_memory_bytes,
            },
            None => MeteringOutcome::Completed {
                fuel_consumed: self.fuel_consumed,
                peak_memory_bytes: self.peak_memory_bytes,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_values() {
        let policy = ServerResourcePolicy::default();
        assert_eq!(policy.cpu_limit, Duration::from_millis(5));
        assert_eq!(policy.memory_bytes, 64 * 1024 * 1024);
        assert_eq!(policy.fuel_budget, 1_000_000);
        assert_eq!(policy.max_table_entries, 10_000);
        assert_eq!(policy.max_instances, 1);
        assert!(policy.syscall_policy.allowed_categories().is_empty());
    }

    #[test]
    fn minimal_policy_values() {
        let policy = ServerResourcePolicy::minimal();
        assert_eq!(policy.cpu_limit, Duration::from_millis(1));
        assert_eq!(policy.memory_bytes, 1024 * 1024);
        assert_eq!(policy.fuel_budget, 10_000);
    }

    #[test]
    fn policy_with_cpu_and_memory() {
        let policy = ServerResourcePolicy::with_cpu_and_memory(
            Duration::from_millis(10),
            128 * 1024 * 1024,
        );
        assert_eq!(policy.cpu_limit, Duration::from_millis(10));
        assert_eq!(policy.memory_bytes, 128 * 1024 * 1024);
        // Other fields should be defaults
        assert_eq!(policy.fuel_budget, DEFAULT_SERVER_FUEL);
    }

    #[test]
    fn policy_from_env_uses_defaults() {
        // Without env vars set, should use defaults
        let policy = ServerResourcePolicy::from_env();
        assert_eq!(policy.fuel_budget, DEFAULT_SERVER_FUEL);
    }

    #[test]
    fn syscall_policy_deny_all() {
        let policy = SyscallPolicy::deny_all();
        assert!(!policy.is_allowed(SyscallCategory::Filesystem));
        assert!(!policy.is_allowed(SyscallCategory::Network));
        assert!(!policy.is_allowed(SyscallCategory::Clock));
        assert!(!policy.is_allowed(SyscallCategory::Random));
        assert!(!policy.is_allowed(SyscallCategory::Environment));
    }

    #[test]
    fn syscall_policy_allow_specific() {
        let policy =
            SyscallPolicy::allow(vec![SyscallCategory::Clock, SyscallCategory::Random]);
        assert!(policy.is_allowed(SyscallCategory::Clock));
        assert!(policy.is_allowed(SyscallCategory::Random));
        assert!(!policy.is_allowed(SyscallCategory::Filesystem));
        assert!(!policy.is_allowed(SyscallCategory::Network));
    }

    #[test]
    fn meter_tracks_fuel_within_budget() {
        let policy = ServerResourcePolicy {
            fuel_budget: 100,
            ..ServerResourcePolicy::default()
        };
        let mut meter = ResourceMeter::new(policy);

        assert!(meter.record_fuel(50).is_ok());
        assert_eq!(meter.fuel_consumed(), 50);
        assert!(!meter.is_terminated());
    }

    #[test]
    fn meter_terminates_on_fuel_exhaustion() {
        let policy = ServerResourcePolicy {
            fuel_budget: 100,
            ..ServerResourcePolicy::default()
        };
        let mut meter = ResourceMeter::new(policy);

        assert!(meter.record_fuel(50).is_ok());
        let result = meter.record_fuel(60);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), MeteringTerminationReason::FuelExhausted);
        assert!(meter.is_terminated());
    }

    #[test]
    fn meter_tracks_memory_within_budget() {
        let policy = ServerResourcePolicy {
            memory_bytes: 1024,
            ..ServerResourcePolicy::default()
        };
        let mut meter = ResourceMeter::new(policy);

        assert!(meter.record_memory(512).is_ok());
        assert_eq!(meter.peak_memory_bytes(), 512);

        assert!(meter.record_memory(1024).is_ok());
        assert_eq!(meter.peak_memory_bytes(), 1024);
    }

    #[test]
    fn meter_terminates_on_memory_exceeded() {
        let policy = ServerResourcePolicy {
            memory_bytes: 1024,
            ..ServerResourcePolicy::default()
        };
        let mut meter = ResourceMeter::new(policy);

        let result = meter.record_memory(2048);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            MeteringTerminationReason::MemoryLimitExceeded
        );
    }

    #[test]
    fn meter_terminates_on_cpu_exceeded() {
        let policy = ServerResourcePolicy {
            cpu_limit: Duration::from_millis(5),
            ..ServerResourcePolicy::default()
        };
        let mut meter = ResourceMeter::new(policy);

        assert!(meter.record_cpu_time(Duration::from_millis(3)).is_ok());
        let result = meter.record_cpu_time(Duration::from_millis(10));
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            MeteringTerminationReason::CpuTimeExceeded
        );
    }

    #[test]
    fn meter_denies_syscall() {
        let policy = ServerResourcePolicy::default(); // deny all
        let mut meter = ResourceMeter::new(policy);

        let result = meter.check_syscall(SyscallCategory::Filesystem);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            MeteringTerminationReason::SyscallDenied(SyscallCategory::Filesystem)
        );
    }

    #[test]
    fn meter_allows_permitted_syscall() {
        let policy = ServerResourcePolicy {
            syscall_policy: SyscallPolicy::allow(vec![SyscallCategory::Clock]),
            ..ServerResourcePolicy::default()
        };
        let mut meter = ResourceMeter::new(policy);

        assert!(meter.check_syscall(SyscallCategory::Clock).is_ok());
        assert!(!meter.is_terminated());
    }

    #[test]
    fn meter_stays_terminated_after_first_violation() {
        let policy = ServerResourcePolicy {
            fuel_budget: 10,
            ..ServerResourcePolicy::default()
        };
        let mut meter = ResourceMeter::new(policy);

        let _ = meter.record_fuel(20); // exceeds budget
        assert!(meter.is_terminated());

        // Subsequent operations should also fail
        assert!(meter.record_fuel(1).is_err());
        assert!(meter.record_memory(1).is_err());
        assert!(meter.record_cpu_time(Duration::ZERO).is_err());
    }

    #[test]
    fn meter_outcome_completed() {
        let policy = ServerResourcePolicy::default();
        let mut meter = ResourceMeter::new(policy);
        meter.record_fuel(500).unwrap();
        meter.record_memory(2048).unwrap();

        match meter.outcome() {
            MeteringOutcome::Completed {
                fuel_consumed,
                peak_memory_bytes,
            } => {
                assert_eq!(fuel_consumed, 500);
                assert_eq!(peak_memory_bytes, 2048);
            }
            other => panic!("expected Completed, got: {other:?}"),
        }
    }

    #[test]
    fn meter_outcome_terminated() {
        let policy = ServerResourcePolicy {
            fuel_budget: 10,
            ..ServerResourcePolicy::default()
        };
        let mut meter = ResourceMeter::new(policy);
        let _ = meter.record_fuel(20);

        match meter.outcome() {
            MeteringOutcome::Terminated {
                reason,
                fuel_consumed,
                ..
            } => {
                assert_eq!(reason, MeteringTerminationReason::FuelExhausted);
                assert_eq!(fuel_consumed, 20);
            }
            other => panic!("expected Terminated, got: {other:?}"),
        }
    }

    #[test]
    fn peak_memory_tracks_highest_value() {
        let policy = ServerResourcePolicy::default();
        let mut meter = ResourceMeter::new(policy);

        meter.record_memory(1000).unwrap();
        meter.record_memory(500).unwrap(); // lower than peak
        meter.record_memory(2000).unwrap();
        meter.record_memory(1500).unwrap(); // lower than peak

        assert_eq!(meter.peak_memory_bytes(), 2000);
    }

    #[test]
    fn metering_termination_reason_display() {
        assert_eq!(
            format!("{}", MeteringTerminationReason::FuelExhausted),
            "fuel exhausted"
        );
        assert_eq!(
            format!("{}", MeteringTerminationReason::CpuTimeExceeded),
            "CPU time exceeded"
        );
        assert_eq!(
            format!("{}", MeteringTerminationReason::MemoryLimitExceeded),
            "memory limit exceeded"
        );
        let denied = MeteringTerminationReason::SyscallDenied(SyscallCategory::Network);
        assert!(format!("{denied}").contains("syscall denied"));
    }
}
