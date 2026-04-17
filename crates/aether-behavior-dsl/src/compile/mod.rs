//! Compilation backend for the Behavior DSL.
//!
//! Entry points:
//! * [`compile_module`]: type-checked [`CheckedModule`] → WASM bytes.
//! * [`WasmSummary::from_bytes`]: inspect a compiled module (used by tests).

pub mod wasm;

pub use wasm::{compile_module, WasmSummary};
