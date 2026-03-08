//! Sandbox configuration for WASM script execution.
//!
//! Defines resource limits enforced via Wasmtime's fuel metering and
//! memory limiter. These caps prevent untrusted scripts from consuming
//! excessive CPU or memory.

const DEFAULT_MAX_MEMORY_BYTES: u64 = 16 * 1024 * 1024; // 16 MB
const DEFAULT_FUEL_LIMIT: u64 = 1_000_000;
const DEFAULT_MAX_EXECUTION_TIME_MS: u64 = 16; // one VR frame

/// Resource limits for a sandboxed WASM instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxConfig {
    /// Maximum linear memory in bytes the WASM module may allocate.
    pub max_memory_bytes: u64,
    /// Maximum fuel (instruction budget) before execution traps.
    pub fuel_limit: u64,
    /// Advisory maximum execution time in milliseconds.
    /// Enforcement is primarily via fuel; this value is informational.
    pub max_execution_time_ms: u64,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            max_memory_bytes: DEFAULT_MAX_MEMORY_BYTES,
            fuel_limit: DEFAULT_FUEL_LIMIT,
            max_execution_time_ms: DEFAULT_MAX_EXECUTION_TIME_MS,
        }
    }
}

impl SandboxConfig {
    /// Creates a sandbox config with custom limits.
    pub fn new(max_memory_bytes: u64, fuel_limit: u64, max_execution_time_ms: u64) -> Self {
        Self {
            max_memory_bytes,
            fuel_limit,
            max_execution_time_ms,
        }
    }

    /// Creates a minimal sandbox for testing (small memory, low fuel).
    pub fn minimal() -> Self {
        Self {
            max_memory_bytes: 1024 * 1024, // 1 MB
            fuel_limit: 10_000,
            max_execution_time_ms: 5,
        }
    }

    /// Configures a Wasmtime `StoreLimitsBuilder` with these limits.
    pub fn to_store_limits(&self) -> wasmtime::StoreLimitsBuilder {
        wasmtime::StoreLimitsBuilder::new()
            .memory_size(self.max_memory_bytes as usize)
            .memories(1)
            .tables(4)
            .instances(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_values() {
        let config = SandboxConfig::default();
        assert_eq!(config.max_memory_bytes, 16 * 1024 * 1024);
        assert_eq!(config.fuel_limit, 1_000_000);
        assert_eq!(config.max_execution_time_ms, 16);
    }

    #[test]
    fn minimal_config_values() {
        let config = SandboxConfig::minimal();
        assert_eq!(config.max_memory_bytes, 1024 * 1024);
        assert_eq!(config.fuel_limit, 10_000);
        assert_eq!(config.max_execution_time_ms, 5);
    }

    #[test]
    fn custom_config() {
        let config = SandboxConfig::new(32 * 1024 * 1024, 500_000, 8);
        assert_eq!(config.max_memory_bytes, 32 * 1024 * 1024);
        assert_eq!(config.fuel_limit, 500_000);
        assert_eq!(config.max_execution_time_ms, 8);
    }

    #[test]
    fn store_limits_builder_does_not_panic() {
        let config = SandboxConfig::default();
        let _limits = config.to_store_limits();
    }

    #[test]
    fn config_is_clone_and_eq() {
        let a = SandboxConfig::default();
        let b = a.clone();
        assert_eq!(a, b);
    }
}
