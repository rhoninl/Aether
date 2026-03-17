//! Runtime execution engine for compiled visual scripts.
//!
//! Provides a register-based virtual machine that interprets `IrInstruction`
//! sequences produced by the IR compiler. The VM is sandboxed with configurable
//! execution limits and dispatches engine API calls through a trait interface.

pub mod engine_api;
pub mod error;
pub mod vm;

pub use engine_api::{EngineApi, NoOpApi, RecordingApi};
pub use error::RuntimeError;
pub use vm::{ScriptVm, VmConfig};
