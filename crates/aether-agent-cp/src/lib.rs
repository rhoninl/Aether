//! Agent Control Plane library.
//!
//! `aether-agent-cp` exposes a first-class MCP + gRPC-compatible surface that
//! lets AI agents author, mutate and simulate Aether worlds. Every tool call
//! returns either a committed result or a structured error with an attached
//! repair patch the agent is expected to apply before retrying.
//!
//! The crate is split into three concerns:
//!
//! 1. **Transports** (`transport`): MCP over stdio, MCP over WebSocket, and a
//!    gRPC-style length-delimited batch transport. All three dispatch into the
//!    same [`ToolRegistry`].
//! 2. **Tool registry + tools** (`registry`, `tools`): the named JSON-in /
//!    JSON-out handlers that actually do the work.
//! 3. **Backend** (`backend`): a thin trait the tools call into. Ships with a
//!    default in-memory implementation; the `wire` feature routes it through
//!    the real Aether crates.
//!
//! Auth lives in `auth` and wraps the JWT validation primitives from
//! [`aether_security`].
//!
//! # Quick start
//!
//! ```no_run
//! use aether_agent_cp::{build_default_registry, backend::InMemoryBackend};
//! use std::sync::Arc;
//!
//! let backend = Arc::new(InMemoryBackend::default());
//! let registry = build_default_registry(backend);
//! let names = registry.tool_names();
//! assert_eq!(names.len(), 9);
//! ```

pub mod auth;
pub mod backend;
pub mod envelope;
pub mod error;
pub mod registry;
pub mod tools;
pub mod transport;

pub use auth::{AuthConfig, AuthVerifier};
pub use backend::{Backend, InMemoryBackend};
pub use envelope::{
    JsonRpcEnvelope, JsonRpcError, JsonRpcId, JsonRpcRequest, JsonRpcResponse, ToolErrorEnvelope,
    ToolSuccessEnvelope,
};
pub use error::{RepairOp, RepairPatch, ToolError, ToolResult};
pub use registry::{ToolDescriptor, ToolRegistry};
pub use tools::build_default_registry;
