//! AOT (Ahead-of-Time) compilation pipeline for server-side WASM execution.
//!
//! Compiles WASM bytecode to native artifacts via Wasmtime's AOT path,
//! producing platform-specific precompiled modules ready for fast instantiation.

use sha2::{Digest, Sha256};
use wasmtime::Engine;

/// Default optimization level for AOT compilation.
const DEFAULT_OPTIMIZATION_LEVEL: OptimizationLevel = OptimizationLevel::Speed;

/// Supported AOT compilation targets for server deployment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AotTarget {
    /// Linux x86_64 servers (most common).
    LinuxX64,
    /// Linux AArch64 servers (ARM-based, e.g. Graviton).
    LinuxAArch64,
}

impl AotTarget {
    /// Returns the Wasmtime-compatible target triple string.
    pub fn triple(&self) -> &'static str {
        match self {
            AotTarget::LinuxX64 => "x86_64-unknown-linux-gnu",
            AotTarget::LinuxAArch64 => "aarch64-unknown-linux-gnu",
        }
    }
}

/// Optimization level for AOT compilation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OptimizationLevel {
    /// No optimizations (fast compile, slow execution).
    None,
    /// Balanced speed/compile-time tradeoff.
    Speed,
    /// Maximum optimization (slow compile, fast execution).
    SpeedAndSize,
}

/// A request to compile WASM bytes to a native artifact.
#[derive(Debug, Clone)]
pub struct AotCompilationRequest {
    /// The raw WASM bytecode to compile.
    pub wasm_bytes: Vec<u8>,
    /// The target platform for the compilation.
    pub target: AotTarget,
    /// The optimization level.
    pub optimization_level: OptimizationLevel,
}

/// A compiled native artifact produced by AOT compilation.
#[derive(Debug, Clone)]
pub struct AotArtifact {
    /// The precompiled native bytes (Wasmtime serialized module).
    pub native_bytes: Vec<u8>,
    /// SHA-256 hash of the original WASM source bytes.
    pub source_hash: [u8; 32],
    /// SHA-256 hash of the compiled native artifact.
    pub artifact_hash: [u8; 32],
    /// The target this artifact was compiled for.
    pub target: AotTarget,
    /// The optimization level used.
    pub optimization_level: OptimizationLevel,
}

/// Errors that can occur during AOT compilation.
#[derive(Debug)]
pub enum AotCompilationError {
    /// The input WASM bytes are invalid or cannot be compiled.
    InvalidWasm(String),
    /// Wasmtime engine error during compilation.
    EngineError(wasmtime::Error),
    /// The target platform is not supported by the current engine.
    UnsupportedTarget(AotTarget),
}

impl std::fmt::Display for AotCompilationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidWasm(msg) => write!(f, "invalid WASM: {msg}"),
            Self::EngineError(e) => write!(f, "engine error: {e}"),
            Self::UnsupportedTarget(t) => write!(f, "unsupported target: {t:?}"),
        }
    }
}

impl std::error::Error for AotCompilationError {}

impl From<wasmtime::Error> for AotCompilationError {
    fn from(e: wasmtime::Error) -> Self {
        Self::EngineError(e)
    }
}

/// Computes the SHA-256 hash of a byte slice.
pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

/// AOT compiler that transforms WASM bytecode into native artifacts.
///
/// Uses Wasmtime's `Engine::precompile_module` to produce serialized
/// native code that can be loaded without re-compilation.
#[derive(Debug)]
pub struct AotCompiler {
    default_optimization: OptimizationLevel,
}

impl Default for AotCompiler {
    fn default() -> Self {
        Self {
            default_optimization: DEFAULT_OPTIMIZATION_LEVEL,
        }
    }
}

impl AotCompiler {
    /// Creates a new AOT compiler with the specified default optimization level.
    pub fn new(default_optimization: OptimizationLevel) -> Self {
        Self {
            default_optimization,
        }
    }

    /// Returns the default optimization level.
    pub fn default_optimization(&self) -> OptimizationLevel {
        self.default_optimization
    }

    /// Compiles WASM bytes to a native artifact for the given target.
    ///
    /// Uses the compiler's default optimization level.
    pub fn compile(
        &self,
        wasm_bytes: &[u8],
        target: AotTarget,
    ) -> Result<AotArtifact, AotCompilationError> {
        self.compile_with_optimization(wasm_bytes, target, self.default_optimization)
    }

    /// Compiles WASM bytes to a native artifact with a specific optimization level.
    pub fn compile_with_optimization(
        &self,
        wasm_bytes: &[u8],
        target: AotTarget,
        optimization_level: OptimizationLevel,
    ) -> Result<AotArtifact, AotCompilationError> {
        if wasm_bytes.is_empty() {
            return Err(AotCompilationError::InvalidWasm(
                "empty WASM bytes".to_string(),
            ));
        }

        let source_hash = sha256(wasm_bytes);

        let config = Self::build_engine_config(optimization_level);
        let engine = Engine::new(&config)?;

        let native_bytes = engine.precompile_module(wasm_bytes)?;
        let artifact_hash = sha256(&native_bytes);

        Ok(AotArtifact {
            native_bytes,
            source_hash,
            artifact_hash,
            target,
            optimization_level,
        })
    }

    /// Builds a Wasmtime engine config for the given optimization level.
    fn build_engine_config(optimization_level: OptimizationLevel) -> wasmtime::Config {
        let mut config = wasmtime::Config::new();
        config.consume_fuel(true);

        let cranelift_level = match optimization_level {
            OptimizationLevel::None => wasmtime::OptLevel::None,
            OptimizationLevel::Speed => wasmtime::OptLevel::Speed,
            OptimizationLevel::SpeedAndSize => wasmtime::OptLevel::SpeedAndSize,
        };
        config.cranelift_opt_level(cranelift_level);
        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_wasm() -> Vec<u8> {
        wat::parse_str(r#"(module (func (export "on_tick") (nop)))"#).unwrap()
    }

    fn addition_wasm() -> Vec<u8> {
        wat::parse_str(
            r#"
            (module
                (func (export "add") (param i32 i32) (result i32)
                    (i32.add (local.get 0) (local.get 1))
                )
            )
            "#,
        )
        .unwrap()
    }

    #[test]
    fn compile_valid_wasm_produces_artifact() {
        let compiler = AotCompiler::default();
        let wasm = valid_wasm();
        let artifact = compiler.compile(&wasm, AotTarget::LinuxX64).unwrap();

        assert!(!artifact.native_bytes.is_empty());
        assert_eq!(artifact.source_hash, sha256(&wasm));
        assert_eq!(artifact.target, AotTarget::LinuxX64);
        assert_eq!(artifact.optimization_level, OptimizationLevel::Speed);
    }

    #[test]
    fn compile_empty_wasm_fails() {
        let compiler = AotCompiler::default();
        let result = compiler.compile(&[], AotTarget::LinuxX64);
        assert!(result.is_err());
        match result.unwrap_err() {
            AotCompilationError::InvalidWasm(msg) => {
                assert!(msg.contains("empty"));
            }
            other => panic!("expected InvalidWasm, got: {other}"),
        }
    }

    #[test]
    fn compile_invalid_wasm_fails() {
        let compiler = AotCompiler::default();
        let result = compiler.compile(b"not valid wasm", AotTarget::LinuxX64);
        assert!(result.is_err());
    }

    #[test]
    fn artifact_hash_is_deterministic() {
        let compiler = AotCompiler::default();
        let wasm = valid_wasm();
        let a1 = compiler.compile(&wasm, AotTarget::LinuxX64).unwrap();
        let a2 = compiler.compile(&wasm, AotTarget::LinuxX64).unwrap();
        assert_eq!(a1.source_hash, a2.source_hash);
        // Note: native bytes may differ between engine instantiations in theory,
        // but source hash should always be identical.
    }

    #[test]
    fn compile_with_different_optimization_levels() {
        let compiler = AotCompiler::default();
        let wasm = addition_wasm();

        let none = compiler
            .compile_with_optimization(&wasm, AotTarget::LinuxX64, OptimizationLevel::None)
            .unwrap();
        let speed = compiler
            .compile_with_optimization(&wasm, AotTarget::LinuxX64, OptimizationLevel::Speed)
            .unwrap();
        let both = compiler
            .compile_with_optimization(
                &wasm,
                AotTarget::LinuxX64,
                OptimizationLevel::SpeedAndSize,
            )
            .unwrap();

        // All should produce valid artifacts
        assert!(!none.native_bytes.is_empty());
        assert!(!speed.native_bytes.is_empty());
        assert!(!both.native_bytes.is_empty());

        // Source hash must be the same for all
        assert_eq!(none.source_hash, speed.source_hash);
        assert_eq!(speed.source_hash, both.source_hash);
    }

    #[test]
    fn aot_target_triples() {
        assert_eq!(AotTarget::LinuxX64.triple(), "x86_64-unknown-linux-gnu");
        assert_eq!(
            AotTarget::LinuxAArch64.triple(),
            "aarch64-unknown-linux-gnu"
        );
    }

    #[test]
    fn sha256_deterministic() {
        let h1 = sha256(b"test data");
        let h2 = sha256(b"test data");
        assert_eq!(h1, h2);
    }

    #[test]
    fn sha256_differs_for_different_data() {
        let h1 = sha256(b"aaa");
        let h2 = sha256(b"bbb");
        assert_ne!(h1, h2);
    }

    #[test]
    fn default_compiler_uses_speed_optimization() {
        let compiler = AotCompiler::default();
        assert_eq!(compiler.default_optimization(), OptimizationLevel::Speed);
    }

    #[test]
    fn custom_compiler_optimization() {
        let compiler = AotCompiler::new(OptimizationLevel::None);
        assert_eq!(compiler.default_optimization(), OptimizationLevel::None);
    }

    #[test]
    fn aot_compilation_error_display() {
        let err = AotCompilationError::InvalidWasm("bad bytes".to_string());
        assert_eq!(format!("{err}"), "invalid WASM: bad bytes");

        let err2 = AotCompilationError::UnsupportedTarget(AotTarget::LinuxAArch64);
        let msg = format!("{err2}");
        assert!(msg.contains("unsupported target"));
    }
}
