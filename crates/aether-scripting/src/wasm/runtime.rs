//! Wasmtime runtime wrapper for loading, compiling, and executing
//! WASM modules with sandboxing and integrity verification.

use std::fmt;
use std::path::PathBuf;

use wasmtime::{Engine, Linker, Module, Store};

use super::cache::ModuleCache;
use super::host_api::{self, ScriptState};
use super::sandbox::SandboxConfig;
use super::verify::{self, IntegrityVerifier};

/// A verified, compiled WASM module ready for instantiation.
#[derive(Debug)]
pub struct WasmModule {
    module: Module,
    content_hash: [u8; 32],
}

impl WasmModule {
    /// Returns the SHA-256 hash of the original WASM bytes.
    pub fn content_hash(&self) -> &[u8; 32] {
        &self.content_hash
    }

    /// Returns a reference to the inner Wasmtime module.
    pub fn inner(&self) -> &Module {
        &self.module
    }
}

/// Errors that can occur during WASM runtime operations.
#[derive(Debug)]
pub enum WasmRuntimeError {
    /// The module failed integrity verification.
    IntegrityFailed(verify::IntegrityError),
    /// Wasmtime compilation or execution error.
    Wasmtime(wasmtime::Error),
    /// I/O error (cache directory, etc.).
    Io(std::io::Error),
    /// The requested export was not found.
    ExportNotFound(String),
}

impl fmt::Display for WasmRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IntegrityFailed(e) => write!(f, "integrity verification failed: {e}"),
            Self::Wasmtime(e) => write!(f, "wasmtime error: {e}"),
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::ExportNotFound(name) => write!(f, "export not found: {name}"),
        }
    }
}

impl std::error::Error for WasmRuntimeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::IntegrityFailed(e) => Some(e),
            Self::Wasmtime(_) => None,
            Self::Io(e) => Some(e),
            Self::ExportNotFound(_) => None,
        }
    }
}

impl From<verify::IntegrityError> for WasmRuntimeError {
    fn from(e: verify::IntegrityError) -> Self {
        Self::IntegrityFailed(e)
    }
}

impl From<wasmtime::Error> for WasmRuntimeError {
    fn from(e: wasmtime::Error) -> Self {
        Self::Wasmtime(e)
    }
}

impl From<std::io::Error> for WasmRuntimeError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// The main WASM runtime. Manages the Wasmtime engine, linker,
/// module cache, and integrity verifier.
pub struct WasmRuntime {
    engine: Engine,
    linker: Linker<ScriptState>,
    cache: ModuleCache,
    verifier: IntegrityVerifier,
    sandbox: SandboxConfig,
}

impl fmt::Debug for WasmRuntime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WasmRuntime")
            .field("cache_dir", &self.cache.cache_dir())
            .field("sandbox", &self.sandbox)
            .field("approved_hashes", &self.verifier.approved_count())
            .finish()
    }
}

impl WasmRuntime {
    /// Creates a new WASM runtime.
    ///
    /// - `cache_dir`: directory for precompiled module storage
    /// - `sandbox`: resource limits for script instances
    /// - `verifier`: integrity verifier with approved hashes
    pub fn new(
        cache_dir: impl Into<PathBuf>,
        sandbox: SandboxConfig,
        verifier: IntegrityVerifier,
    ) -> Result<Self, WasmRuntimeError> {
        let mut config = wasmtime::Config::new();
        config.consume_fuel(true);
        let engine = Engine::new(&config)?;

        let mut linker = Linker::new(&engine);
        host_api::register_host_functions(&mut linker)?;

        let cache = ModuleCache::new(cache_dir)?;

        Ok(Self {
            engine,
            linker,
            cache,
            verifier,
            sandbox,
        })
    }

    /// Returns a reference to the sandbox configuration.
    pub fn sandbox_config(&self) -> &SandboxConfig {
        &self.sandbox
    }

    /// Returns a reference to the integrity verifier.
    pub fn verifier(&self) -> &IntegrityVerifier {
        &self.verifier
    }

    /// Returns a mutable reference to the integrity verifier.
    pub fn verifier_mut(&mut self) -> &mut IntegrityVerifier {
        &mut self.verifier
    }

    /// Loads and compiles a WASM module from raw bytes.
    ///
    /// 1. Computes SHA-256 hash of the bytes
    /// 2. Checks hash against the integrity verifier
    /// 3. Tries to load a precompiled module from cache
    /// 4. If cache miss, compiles and caches the result
    pub fn load_module(&self, wasm_bytes: &[u8]) -> Result<WasmModule, WasmRuntimeError> {
        let hash = self.verifier.verify(wasm_bytes)?;

        // Try cache first
        if let Some(module) = self.cache.load(&self.engine, &hash)? {
            return Ok(WasmModule {
                module,
                content_hash: hash,
            });
        }

        // Compile and cache
        let module = Module::new(&self.engine, wasm_bytes)?;
        // Best-effort cache store; ignore errors
        let _ = self.cache.store(&module, &hash);

        Ok(WasmModule {
            module,
            content_hash: hash,
        })
    }

    /// Loads a module without integrity verification.
    ///
    /// This is intended for development/testing only. In production,
    /// use `load_module` which enforces integrity checks.
    pub fn load_module_unchecked(
        &self,
        wasm_bytes: &[u8],
    ) -> Result<WasmModule, WasmRuntimeError> {
        let hash = verify::sha256_hash(wasm_bytes);

        if let Some(module) = self.cache.load(&self.engine, &hash)? {
            return Ok(WasmModule {
                module,
                content_hash: hash,
            });
        }

        let module = Module::new(&self.engine, wasm_bytes)?;
        let _ = self.cache.store(&module, &hash);

        Ok(WasmModule {
            module,
            content_hash: hash,
        })
    }

    /// Creates a new sandboxed store and instantiates the module.
    ///
    /// Returns the store and instance. The store has fuel and memory
    /// limits applied according to the runtime's `SandboxConfig`.
    pub fn instantiate(
        &self,
        wasm_module: &WasmModule,
    ) -> Result<(Store<ScriptState>, wasmtime::Instance), WasmRuntimeError> {
        let limits = self.sandbox.to_store_limits().build();
        let state = ScriptState::new(limits);
        let mut store = Store::new(&self.engine, state);
        store.set_fuel(self.sandbox.fuel_limit)?;
        store.limiter(|s| &mut s.store_limits);

        let instance = self.linker.instantiate(&mut store, &wasm_module.module)?;
        Ok((store, instance))
    }

    /// Convenience: loads a module, instantiates it, and calls a named
    /// export function that takes no arguments and returns no value.
    pub fn call_void_export(
        &self,
        wasm_module: &WasmModule,
        export_name: &str,
    ) -> Result<Store<ScriptState>, WasmRuntimeError> {
        let (mut store, instance) = self.instantiate(wasm_module)?;
        let func = instance
            .get_typed_func::<(), ()>(&mut store, export_name)
            .map_err(|_| WasmRuntimeError::ExportNotFound(export_name.to_string()))?;
        func.call(&mut store, ())?;
        Ok(store)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nop_module_wat() -> &'static str {
        r#"(module (func (export "on_init") (nop)) (func (export "on_tick") (nop)))"#
    }

    fn addition_module_wat() -> &'static str {
        r#"
        (module
            (func (export "add") (param i32 i32) (result i32)
                (i32.add (local.get 0) (local.get 1))
            )
        )
        "#
    }

    fn infinite_loop_wat() -> &'static str {
        r#"
        (module
            (func (export "run")
                (loop $inf
                    (br $inf)
                )
            )
        )
        "#
    }

    /// WAT module that tries to grow memory beyond limits.
    fn memory_hog_wat() -> &'static str {
        r#"
        (module
            (memory (export "memory") 1)
            (func (export "run") (result i32)
                ;; Try to grow memory by 1024 pages (64 MB)
                (memory.grow (i32.const 1024))
            )
        )
        "#
    }

    fn create_runtime(
        tmp: &tempfile::TempDir,
    ) -> WasmRuntime {
        WasmRuntime::new(
            tmp.path().join("cache"),
            SandboxConfig::default(),
            IntegrityVerifier::new(),
        )
        .expect("runtime creation")
    }

    fn create_runtime_with_verifier(
        tmp: &tempfile::TempDir,
        verifier: IntegrityVerifier,
    ) -> WasmRuntime {
        WasmRuntime::new(
            tmp.path().join("cache"),
            SandboxConfig::default(),
            verifier,
        )
        .expect("runtime creation")
    }

    #[test]
    fn load_and_call_nop_module() {
        let tmp = tempfile::tempdir().unwrap();
        let runtime = create_runtime(&tmp);

        let wasm = wat::parse_str(nop_module_wat()).unwrap();
        let module = runtime.load_module_unchecked(&wasm).unwrap();

        let store = runtime.call_void_export(&module, "on_init").unwrap();
        // Should have completed without error, fuel should be partially consumed
        let remaining_fuel = store.get_fuel().unwrap();
        assert!(remaining_fuel < 1_000_000);
    }

    #[test]
    fn load_and_call_addition() {
        let tmp = tempfile::tempdir().unwrap();
        let runtime = create_runtime(&tmp);

        let wasm = wat::parse_str(addition_module_wat()).unwrap();
        let module = runtime.load_module_unchecked(&wasm).unwrap();

        let (mut store, instance) = runtime.instantiate(&module).unwrap();
        let add = instance
            .get_typed_func::<(i32, i32), i32>(&mut store, "add")
            .unwrap();
        let result = add.call(&mut store, (3, 4)).unwrap();
        assert_eq!(result, 7);
    }

    #[test]
    fn fuel_exhaustion_traps_infinite_loop() {
        let tmp = tempfile::tempdir().unwrap();
        let sandbox = SandboxConfig::new(16 * 1024 * 1024, 100, 16);
        let runtime = WasmRuntime::new(
            tmp.path().join("cache"),
            sandbox,
            IntegrityVerifier::new(),
        )
        .unwrap();

        let wasm = wat::parse_str(infinite_loop_wat()).unwrap();
        let module = runtime.load_module_unchecked(&wasm).unwrap();

        let result = runtime.call_void_export(&module, "run");
        assert!(result.is_err(), "infinite loop should trap on fuel exhaustion");
        let err_msg = format!("{}", result.unwrap_err());
        // Wasmtime traps with various messages depending on version;
        // the key is that execution was terminated.
        assert!(
            err_msg.contains("fuel")
                || err_msg.contains("wasm trap")
                || err_msg.contains("wasm backtrace")
                || err_msg.contains("error while executing"),
            "error should indicate execution was stopped: {err_msg}"
        );
    }

    #[test]
    fn memory_limit_prevents_excessive_growth() {
        let tmp = tempfile::tempdir().unwrap();
        // 1 MB memory limit
        let sandbox = SandboxConfig::new(1024 * 1024, 1_000_000, 16);
        let runtime = WasmRuntime::new(
            tmp.path().join("cache"),
            sandbox,
            IntegrityVerifier::new(),
        )
        .unwrap();

        let wasm = wat::parse_str(memory_hog_wat()).unwrap();
        let module = runtime.load_module_unchecked(&wasm).unwrap();

        let (mut store, instance) = runtime.instantiate(&module).unwrap();
        let run = instance
            .get_typed_func::<(), i32>(&mut store, "run")
            .unwrap();
        let grow_result = run.call(&mut store, ()).unwrap();
        // memory.grow returns -1 on failure
        assert_eq!(grow_result, -1, "memory growth should be denied");
    }

    #[test]
    fn integrity_verified_module_loads() {
        let tmp = tempfile::tempdir().unwrap();
        let wasm = wat::parse_str(nop_module_wat()).unwrap();
        let hash = verify::sha256_hash(&wasm);

        let verifier = IntegrityVerifier::with_approved(vec![hash]);
        let runtime = create_runtime_with_verifier(&tmp, verifier);

        let module = runtime.load_module(&wasm);
        assert!(module.is_ok());
        assert_eq!(module.unwrap().content_hash(), &hash);
    }

    #[test]
    fn integrity_rejected_module_fails() {
        let tmp = tempfile::tempdir().unwrap();
        let wasm = wat::parse_str(nop_module_wat()).unwrap();
        // No hashes approved
        let runtime = create_runtime(&tmp);

        let result = runtime.load_module(&wasm);
        assert!(result.is_err());
        match result.unwrap_err() {
            WasmRuntimeError::IntegrityFailed(_) => {}
            other => panic!("expected IntegrityFailed, got: {other}"),
        }
    }

    #[test]
    fn cached_module_loads_on_second_call() {
        let tmp = tempfile::tempdir().unwrap();
        let runtime = create_runtime(&tmp);

        let wasm = wat::parse_str(nop_module_wat()).unwrap();

        // First load: compiles and caches
        let module1 = runtime.load_module_unchecked(&wasm).unwrap();
        let hash = *module1.content_hash();

        // Verify cache file exists
        assert!(runtime.cache.cache_path(&hash).exists());

        // Second load: should hit cache
        let module2 = runtime.load_module_unchecked(&wasm).unwrap();
        assert_eq!(module2.content_hash(), &hash);
    }

    #[test]
    fn export_not_found_error() {
        let tmp = tempfile::tempdir().unwrap();
        let runtime = create_runtime(&tmp);

        let wasm = wat::parse_str(nop_module_wat()).unwrap();
        let module = runtime.load_module_unchecked(&wasm).unwrap();

        let result = runtime.call_void_export(&module, "nonexistent_function");
        assert!(result.is_err());
        match result.unwrap_err() {
            WasmRuntimeError::ExportNotFound(name) => {
                assert_eq!(name, "nonexistent_function");
            }
            other => panic!("expected ExportNotFound, got: {other}"),
        }
    }

    #[test]
    fn runtime_debug_format() {
        let tmp = tempfile::tempdir().unwrap();
        let runtime = create_runtime(&tmp);
        let debug = format!("{runtime:?}");
        assert!(debug.contains("WasmRuntime"));
        assert!(debug.contains("cache_dir"));
    }

    #[test]
    fn module_content_hash_matches() {
        let tmp = tempfile::tempdir().unwrap();
        let runtime = create_runtime(&tmp);

        let wasm = wat::parse_str(nop_module_wat()).unwrap();
        let expected_hash = verify::sha256_hash(&wasm);
        let module = runtime.load_module_unchecked(&wasm).unwrap();

        assert_eq!(*module.content_hash(), expected_hash);
    }

    #[test]
    fn multiple_instantiations_are_isolated() {
        let tmp = tempfile::tempdir().unwrap();
        let runtime = create_runtime(&tmp);

        let wat = r#"
        (module
            (import "env" "host_log" (func $log (param i32 i32)))
            (memory (export "memory") 1)
            (data (i32.const 0) "msg_a")
            (func (export "run")
                (call $log (i32.const 0) (i32.const 5))
            )
        )
        "#;
        let wasm = wat::parse_str(wat).unwrap();
        let module = runtime.load_module_unchecked(&wasm).unwrap();

        let store1 = runtime.call_void_export(&module, "run").unwrap();
        let store2 = runtime.call_void_export(&module, "run").unwrap();

        // Each instance has its own state
        assert_eq!(store1.data().log_messages.len(), 1);
        assert_eq!(store2.data().log_messages.len(), 1);
    }

    #[test]
    fn wasm_runtime_error_display() {
        let err = WasmRuntimeError::ExportNotFound("foo".to_string());
        assert_eq!(format!("{err}"), "export not found: foo");

        let integrity_err = verify::IntegrityError {
            expected_any_of: 1,
            actual_hash: [0u8; 32],
        };
        let err2 = WasmRuntimeError::IntegrityFailed(integrity_err);
        let msg = format!("{err2}");
        assert!(msg.contains("integrity verification failed"));
    }
}
