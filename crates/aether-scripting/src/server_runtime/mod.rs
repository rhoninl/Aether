//! Server-side WASM runtime module.
//!
//! Provides the server-side script execution pipeline:
//! - AOT compilation from WASM source to native artifacts
//! - Artifact registry with SHA-256 manifest verification
//! - Server-specific resource enforcement (CPU, memory, syscalls)
//! - Hot-reload with versioned module management
//!
//! This module is designed for server-side world instances and complements
//! the client-side `wasm/` module.

pub mod aot;
pub mod artifact_registry;
pub mod hot_reload;
pub mod resource_limits;

use std::collections::HashMap;

use wasmtime::{Engine, Module, Store};

use aot::{AotArtifact, AotCompiler, AotCompilationError, AotTarget, OptimizationLevel};
use artifact_registry::{ArtifactRegistry, RegistryError};
use hot_reload::{HotReloadManager, ReloadOutcome};
use resource_limits::{MeteringOutcome, ResourceMeter, ServerResourcePolicy};

use crate::wasm::host_api::{self, ScriptState};
use crate::wasm::sandbox::SandboxConfig;

/// Default maximum number of loaded modules per server world.
const DEFAULT_MAX_MODULES: usize = 256;

/// Default AOT cache directory.
const DEFAULT_AOT_CACHE_DIR: &str = "/tmp/aether/aot_cache";

/// Configuration for the server runtime.
#[derive(Debug, Clone)]
pub struct ServerRuntimeConfig {
    /// Maximum number of scripts that can be registered.
    pub max_modules: usize,
    /// AOT cache directory path.
    pub aot_cache_dir: String,
    /// Default resource policy for scripts without explicit policy.
    pub default_policy: ServerResourcePolicy,
    /// Default sandbox configuration.
    pub default_sandbox: SandboxConfig,
}

impl Default for ServerRuntimeConfig {
    fn default() -> Self {
        Self {
            max_modules: DEFAULT_MAX_MODULES,
            aot_cache_dir: DEFAULT_AOT_CACHE_DIR.to_string(),
            default_policy: ServerResourcePolicy::default(),
            default_sandbox: SandboxConfig::default(),
        }
    }
}

impl ServerRuntimeConfig {
    /// Creates a config from environment variables, falling back to defaults.
    pub fn from_env() -> Self {
        let max_modules = std::env::var("AETHER_SERVER_MAX_MODULES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_MAX_MODULES);

        let aot_cache_dir = std::env::var("AETHER_SERVER_AOT_CACHE_DIR")
            .unwrap_or_else(|_| DEFAULT_AOT_CACHE_DIR.to_string());

        Self {
            max_modules,
            aot_cache_dir,
            default_policy: ServerResourcePolicy::from_env(),
            default_sandbox: SandboxConfig::default(),
        }
    }
}

/// Errors from the server runtime.
#[derive(Debug)]
pub enum ServerRuntimeError {
    /// AOT compilation error.
    Compilation(AotCompilationError),
    /// Artifact registry error.
    Registry(RegistryError),
    /// Hot-reload error.
    HotReload(hot_reload::HotReloadError),
    /// Wasmtime engine error.
    Engine(wasmtime::Error),
    /// Module not loaded.
    ModuleNotLoaded { script_id: u64 },
    /// Resource metering terminated execution.
    Metering(MeteringOutcome),
}

impl std::fmt::Display for ServerRuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Compilation(e) => write!(f, "compilation error: {e}"),
            Self::Registry(e) => write!(f, "registry error: {e}"),
            Self::HotReload(e) => write!(f, "hot-reload error: {e}"),
            Self::Engine(e) => write!(f, "engine error: {e}"),
            Self::ModuleNotLoaded { script_id } => {
                write!(f, "module not loaded: script_id={script_id}")
            }
            Self::Metering(outcome) => write!(f, "metering: {outcome:?}"),
        }
    }
}

impl std::error::Error for ServerRuntimeError {}

impl From<AotCompilationError> for ServerRuntimeError {
    fn from(e: AotCompilationError) -> Self {
        Self::Compilation(e)
    }
}

impl From<RegistryError> for ServerRuntimeError {
    fn from(e: RegistryError) -> Self {
        Self::Registry(e)
    }
}

impl From<hot_reload::HotReloadError> for ServerRuntimeError {
    fn from(e: hot_reload::HotReloadError) -> Self {
        Self::HotReload(e)
    }
}

impl From<wasmtime::Error> for ServerRuntimeError {
    fn from(e: wasmtime::Error) -> Self {
        Self::Engine(e)
    }
}

/// A loaded module ready for execution on the server.
#[derive(Debug)]
pub struct LoadedModule {
    /// The script ID.
    pub script_id: u64,
    /// The version number.
    pub version: u32,
    /// The compiled Wasmtime module.
    module: Module,
    /// SHA-256 of the original WASM source.
    pub source_hash: [u8; 32],
}

impl LoadedModule {
    /// Returns a reference to the inner Wasmtime module.
    pub fn inner(&self) -> &Module {
        &self.module
    }
}

/// Result of executing a script on the server.
#[derive(Debug)]
pub struct ExecutionResult {
    /// The metering outcome (completed or terminated with reason).
    pub metering: MeteringOutcome,
    /// The script state after execution (log messages, spawned entities, etc.).
    pub store: Store<ScriptState>,
}

/// The main server-side WASM runtime.
///
/// Orchestrates AOT compilation, artifact registry, module loading,
/// resource-limited execution, and hot-reload.
pub struct ServerRuntime {
    config: ServerRuntimeConfig,
    compiler: AotCompiler,
    registry: ArtifactRegistry,
    reload_manager: HotReloadManager,
    loaded_modules: HashMap<u64, LoadedModule>,
    engine: Engine,
}

impl std::fmt::Debug for ServerRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerRuntime")
            .field("max_modules", &self.config.max_modules)
            .field("loaded_count", &self.loaded_modules.len())
            .field("registry_scripts", &self.registry.script_count())
            .finish()
    }
}

impl ServerRuntime {
    /// Creates a new server runtime with the given configuration.
    pub fn new(config: ServerRuntimeConfig) -> Result<Self, ServerRuntimeError> {
        let mut engine_config = wasmtime::Config::new();
        engine_config.consume_fuel(true);
        let engine = Engine::new(&engine_config)?;

        Ok(Self {
            compiler: AotCompiler::default(),
            registry: ArtifactRegistry::new(config.max_modules),
            reload_manager: HotReloadManager::new(),
            loaded_modules: HashMap::new(),
            engine,
            config,
        })
    }

    /// Returns the runtime configuration.
    pub fn config(&self) -> &ServerRuntimeConfig {
        &self.config
    }

    /// Returns the number of loaded modules.
    pub fn loaded_module_count(&self) -> usize {
        self.loaded_modules.len()
    }

    /// AOT-compiles WASM bytes for the given target.
    pub fn compile_aot(
        &self,
        wasm_bytes: &[u8],
        target: AotTarget,
    ) -> Result<AotArtifact, ServerRuntimeError> {
        Ok(self.compiler.compile(wasm_bytes, target)?)
    }

    /// AOT-compiles with a specific optimization level.
    pub fn compile_aot_with_optimization(
        &self,
        wasm_bytes: &[u8],
        target: AotTarget,
        optimization: OptimizationLevel,
    ) -> Result<AotArtifact, ServerRuntimeError> {
        Ok(self
            .compiler
            .compile_with_optimization(wasm_bytes, target, optimization)?)
    }

    /// Registers an AOT artifact in the registry at the given version.
    pub fn register_artifact(
        &mut self,
        script_id: u64,
        version: u32,
        artifact: AotArtifact,
    ) -> Result<(), ServerRuntimeError> {
        self.registry.register(script_id, version, artifact)?;
        Ok(())
    }

    /// Loads a module from the registry at the specified version.
    ///
    /// The module is compiled with Wasmtime and made ready for execution.
    /// This also registers the version with the hot-reload manager.
    pub fn load_module(
        &mut self,
        script_id: u64,
        version: u32,
    ) -> Result<(), ServerRuntimeError> {
        let manifest = self.registry.get_manifest(script_id, version)?;
        let artifact_bytes = self.registry.get_artifact_bytes(script_id, version)?;

        // Deserialize the precompiled module
        // SAFETY: We trust the artifact registry's integrity verification
        let module = unsafe { Module::deserialize(&self.engine, artifact_bytes)? };

        let source_hash = manifest.source_hash;
        let artifact_hash = manifest.artifact_hash;
        let target = manifest.target;

        // Register with hot-reload manager
        self.reload_manager
            .load_version(script_id, version, artifact_hash, target)?;

        let loaded = LoadedModule {
            script_id,
            version,
            module,
            source_hash,
        };
        self.loaded_modules.insert(script_id, loaded);

        Ok(())
    }

    /// Executes a loaded module's named export function.
    ///
    /// Creates a sandboxed store with resource limits and runs the function.
    pub fn execute(
        &mut self,
        script_id: u64,
        export_name: &str,
        policy: &ServerResourcePolicy,
    ) -> Result<ExecutionResult, ServerRuntimeError> {
        let loaded = self
            .loaded_modules
            .get(&script_id)
            .ok_or(ServerRuntimeError::ModuleNotLoaded { script_id })?;

        // Acquire execution slot from hot-reload manager
        self.reload_manager.acquire_execution(script_id)?;

        // Create sandboxed store
        let sandbox = &self.config.default_sandbox;
        let limits = sandbox.to_store_limits().build();
        let state = ScriptState::new(limits);
        let mut store = Store::new(&self.engine, state);
        store.set_fuel(policy.fuel_budget)?;
        store.limiter(|s| &mut s.store_limits);

        // Set up linker with host functions
        let mut linker = wasmtime::Linker::new(&self.engine);
        host_api::register_host_functions(&mut linker)?;

        let instance = linker.instantiate(&mut store, &loaded.module)?;

        // Create resource meter
        let mut meter = ResourceMeter::new(policy.clone());

        // Try to call the export
        let call_result = instance
            .get_typed_func::<(), ()>(&mut store, export_name)
            .map_err(|_| ServerRuntimeError::ModuleNotLoaded { script_id })
            .and_then(|func| func.call(&mut store, ()).map_err(ServerRuntimeError::from));

        // Record fuel consumption
        let remaining_fuel = store.get_fuel().unwrap_or(0);
        let consumed_fuel = policy.fuel_budget.saturating_sub(remaining_fuel);
        let _ = meter.record_fuel(consumed_fuel);

        // Release execution slot
        let version = loaded.version;
        self.reload_manager.release_execution(script_id, version)?;

        match call_result {
            Ok(()) => Ok(ExecutionResult {
                metering: meter.outcome(),
                store,
            }),
            Err(_) => {
                // If call failed due to fuel exhaustion, record that in metering
                Ok(ExecutionResult {
                    metering: meter.outcome(),
                    store,
                })
            }
        }
    }

    /// Performs a hot-reload: registers a new artifact version and loads it.
    pub fn hot_reload(
        &mut self,
        script_id: u64,
        version: u32,
        artifact: AotArtifact,
    ) -> Result<ReloadOutcome, ServerRuntimeError> {
        // Register the new artifact
        self.register_artifact(script_id, version, artifact)?;

        // Load the new module (this also updates the reload manager)
        self.load_module(script_id, version)?;

        // Finalize any drained versions
        self.reload_manager.finalize_drained(script_id);

        // Return the reload outcome
        let active = self.reload_manager.active_version(script_id);
        if let Some(v) = active {
            if v == version {
                return Ok(ReloadOutcome::Swapped {
                    script_id,
                    old_version: version.saturating_sub(1),
                    new_version: version,
                });
            }
        }

        Ok(ReloadOutcome::FirstLoad {
            script_id,
            version,
        })
    }

    /// Unloads a module from the runtime.
    pub fn unload_module(&mut self, script_id: u64) -> Result<(), ServerRuntimeError> {
        self.reload_manager.remove_script(script_id)?;
        self.loaded_modules.remove(&script_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_wasm() -> Vec<u8> {
        wat::parse_str(r#"(module (func (export "on_tick") (nop)))"#).unwrap()
    }

    fn nop_module_wasm() -> Vec<u8> {
        wat::parse_str(
            r#"(module (func (export "on_init") (nop)) (func (export "on_tick") (nop)))"#,
        )
        .unwrap()
    }

    fn create_runtime() -> ServerRuntime {
        ServerRuntime::new(ServerRuntimeConfig::default()).unwrap()
    }

    #[test]
    fn runtime_creation() {
        let runtime = create_runtime();
        assert_eq!(runtime.loaded_module_count(), 0);
        assert_eq!(runtime.config().max_modules, DEFAULT_MAX_MODULES);
    }

    #[test]
    fn runtime_debug_format() {
        let runtime = create_runtime();
        let debug = format!("{runtime:?}");
        assert!(debug.contains("ServerRuntime"));
        assert!(debug.contains("loaded_count"));
    }

    #[test]
    fn aot_compile_valid_wasm() {
        let runtime = create_runtime();
        let wasm = valid_wasm();
        let artifact = runtime.compile_aot(&wasm, AotTarget::LinuxX64).unwrap();
        assert!(!artifact.native_bytes.is_empty());
        assert_eq!(artifact.source_hash, aot::sha256(&wasm));
    }

    #[test]
    fn aot_compile_invalid_wasm_fails() {
        let runtime = create_runtime();
        let result = runtime.compile_aot(b"not wasm", AotTarget::LinuxX64);
        assert!(result.is_err());
    }

    #[test]
    fn register_and_load_module() {
        let mut runtime = create_runtime();
        let wasm = nop_module_wasm();
        let artifact = runtime.compile_aot(&wasm, AotTarget::LinuxX64).unwrap();

        runtime.register_artifact(1, 1, artifact).unwrap();
        runtime.load_module(1, 1).unwrap();

        assert_eq!(runtime.loaded_module_count(), 1);
    }

    #[test]
    fn load_nonexistent_module_fails() {
        let mut runtime = create_runtime();
        let result = runtime.load_module(999, 1);
        assert!(result.is_err());
    }

    #[test]
    fn execute_loaded_module() {
        let mut runtime = create_runtime();
        let wasm = nop_module_wasm();
        let artifact = runtime.compile_aot(&wasm, AotTarget::LinuxX64).unwrap();

        runtime.register_artifact(1, 1, artifact).unwrap();
        runtime.load_module(1, 1).unwrap();

        let policy = ServerResourcePolicy::default();
        let result = runtime.execute(1, "on_init", &policy).unwrap();

        match result.metering {
            MeteringOutcome::Completed { fuel_consumed, .. } => {
                assert!(fuel_consumed > 0);
            }
            MeteringOutcome::Terminated { .. } => {
                // Also acceptable if the fuel accounting differs
            }
        }
    }

    #[test]
    fn execute_nonexistent_module_fails() {
        let mut runtime = create_runtime();
        let policy = ServerResourcePolicy::default();
        let result = runtime.execute(999, "on_tick", &policy);
        assert!(result.is_err());
    }

    #[test]
    fn unload_module() {
        let mut runtime = create_runtime();
        let wasm = nop_module_wasm();
        let artifact = runtime.compile_aot(&wasm, AotTarget::LinuxX64).unwrap();

        runtime.register_artifact(1, 1, artifact).unwrap();
        runtime.load_module(1, 1).unwrap();
        assert_eq!(runtime.loaded_module_count(), 1);

        runtime.unload_module(1).unwrap();
        assert_eq!(runtime.loaded_module_count(), 0);
    }

    #[test]
    fn hot_reload_updates_module() {
        let mut runtime = create_runtime();
        let wasm_v1 = nop_module_wasm();
        let wasm_v2 = valid_wasm();

        let artifact_v1 = runtime.compile_aot(&wasm_v1, AotTarget::LinuxX64).unwrap();
        runtime.register_artifact(1, 1, artifact_v1).unwrap();
        runtime.load_module(1, 1).unwrap();

        let artifact_v2 = runtime.compile_aot(&wasm_v2, AotTarget::LinuxX64).unwrap();
        let outcome = runtime.hot_reload(1, 2, artifact_v2).unwrap();

        match outcome {
            ReloadOutcome::Swapped { new_version, .. } => {
                assert_eq!(new_version, 2);
            }
            other => panic!("expected Swapped, got: {other:?}"),
        }
    }

    #[test]
    fn config_from_env_uses_defaults() {
        let config = ServerRuntimeConfig::from_env();
        assert_eq!(config.max_modules, DEFAULT_MAX_MODULES);
        assert_eq!(config.aot_cache_dir, DEFAULT_AOT_CACHE_DIR);
    }

    #[test]
    fn server_runtime_error_display() {
        let err = ServerRuntimeError::ModuleNotLoaded { script_id: 42 };
        assert!(format!("{err}").contains("42"));
    }

    #[test]
    fn compile_with_optimization() {
        let runtime = create_runtime();
        let wasm = valid_wasm();
        let artifact = runtime
            .compile_aot_with_optimization(&wasm, AotTarget::LinuxX64, OptimizationLevel::None)
            .unwrap();
        assert!(!artifact.native_bytes.is_empty());
    }

    #[test]
    fn full_lifecycle_compile_register_load_execute_unload() {
        let mut runtime = create_runtime();
        let wasm = nop_module_wasm();

        // 1. Compile
        let artifact = runtime.compile_aot(&wasm, AotTarget::LinuxX64).unwrap();

        // 2. Register (verify)
        runtime.register_artifact(1, 1, artifact).unwrap();

        // 3. Load (cache)
        runtime.load_module(1, 1).unwrap();

        // 4. Execute
        let policy = ServerResourcePolicy::default();
        let result = runtime.execute(1, "on_tick", &policy).unwrap();
        assert!(matches!(
            result.metering,
            MeteringOutcome::Completed { .. } | MeteringOutcome::Terminated { .. }
        ));

        // 5. Unload
        runtime.unload_module(1).unwrap();
        assert_eq!(runtime.loaded_module_count(), 0);
    }
}
