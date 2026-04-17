//! Tool registry.
//!
//! Maps tool names (e.g. `world.create`) to boxed handlers. Supports enumeration
//! (for `tools/list`) and synchronous dispatch.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::error::{codes, ToolError, ToolResult};

/// Machine-readable descriptor for a registered tool. Returned by `tools/list`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDescriptor {
    pub name: String,
    pub description: String,
    /// Minimal JSON Schema describing the `params` body. We inline it to avoid
    /// pulling in `schemars`; each tool module supplies its own schema literal.
    pub input_schema: serde_json::Value,
    /// Whether the tool mutates world state. Informational only.
    pub mutates: bool,
    /// Whether the tool streams results (e.g. telemetry).
    pub streaming: bool,
}

/// Synchronous tool invocation callback. Tools are cheap + stateless; shared
/// state lives on the backend.
pub type ToolFn =
    Arc<dyn Fn(serde_json::Value) -> ToolResult<serde_json::Value> + Send + Sync + 'static>;

/// Registered tool entry.
pub struct ToolEntry {
    pub descriptor: ToolDescriptor,
    pub call: ToolFn,
}

impl std::fmt::Debug for ToolEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolEntry").field("descriptor", &self.descriptor).finish()
    }
}

/// The registry itself. `BTreeMap` keeps the enumeration stable.
#[derive(Default, Debug)]
pub struct ToolRegistry {
    tools: BTreeMap<String, ToolEntry>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a tool. Panics if the name is already present (caller bug).
    pub fn register(&mut self, descriptor: ToolDescriptor, call: ToolFn) {
        let name = descriptor.name.clone();
        if self.tools.contains_key(&name) {
            panic!("tool `{}` already registered", name);
        }
        self.tools.insert(name, ToolEntry { descriptor, call });
    }

    /// Whether the registry has a handler for the given name.
    pub fn contains(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// Snapshot the currently registered tool descriptors.
    pub fn describe_all(&self) -> Vec<ToolDescriptor> {
        self.tools.values().map(|t| t.descriptor.clone()).collect()
    }

    /// Names only (cheap; no clone of the schemas).
    pub fn tool_names(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    /// Dispatch a tool call. Returns an `UNKNOWN_METHOD` error for unregistered
    /// names.
    pub fn call(&self, name: &str, params: serde_json::Value) -> ToolResult<serde_json::Value> {
        match self.tools.get(name) {
            Some(entry) => (entry.call)(params),
            None => Err(ToolError::new(
                codes::UNKNOWN_METHOD,
                format!("unknown tool `{}`", name),
            )
            .suggest("call `tools/list` to discover available tool names")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn echo_tool() -> (ToolDescriptor, ToolFn) {
        let desc = ToolDescriptor {
            name: "echo".into(),
            description: "echoes params".into(),
            input_schema: serde_json::json!({"type":"object"}),
            mutates: false,
            streaming: false,
        };
        let call: ToolFn = Arc::new(|v| Ok(v));
        (desc, call)
    }

    #[test]
    fn register_and_dispatch() {
        let mut r = ToolRegistry::new();
        let (d, c) = echo_tool();
        r.register(d, c);
        assert!(r.contains("echo"));
        let v = r.call("echo", serde_json::json!(42)).unwrap();
        assert_eq!(v, serde_json::json!(42));
    }

    #[test]
    fn unknown_method_error() {
        let r = ToolRegistry::new();
        let err = r.call("nope", serde_json::Value::Null).unwrap_err();
        assert_eq!(err.code, codes::UNKNOWN_METHOD);
    }

    #[test]
    fn describe_all_is_stable() {
        let mut r = ToolRegistry::new();
        let (d, c) = echo_tool();
        r.register(d, c);
        let desc = r.describe_all();
        assert_eq!(desc.len(), 1);
        assert_eq!(desc[0].name, "echo");
    }

    #[test]
    #[should_panic(expected = "already registered")]
    fn double_register_panics() {
        let mut r = ToolRegistry::new();
        let (d, c) = echo_tool();
        r.register(d.clone(), c.clone());
        r.register(d, c);
    }
}
