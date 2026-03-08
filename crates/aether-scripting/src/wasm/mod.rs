//! WASM client-side runtime module.
//!
//! Provides Wasmtime-based script execution with:
//! - JIT compilation and precompiled module caching
//! - SHA-256 integrity verification
//! - Fuel metering and memory-limit sandboxing
//! - Host API bindings for engine integration

pub mod cache;
pub mod host_api;
pub mod runtime;
pub mod sandbox;
pub mod verify;

pub use cache::ModuleCache;
pub use host_api::ScriptState;
pub use runtime::{WasmModule, WasmRuntime};
pub use sandbox::SandboxConfig;
pub use verify::IntegrityVerifier;
