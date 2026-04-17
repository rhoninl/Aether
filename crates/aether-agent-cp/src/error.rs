//! Structured tool errors and repair-patch envelope.
//!
//! Every tool handler returns [`ToolResult<T>`]. On the wire, the error form
//! serialises into a stable JSON envelope (see [`crate::envelope`]) that
//! includes a machine-readable error `code`, a human-readable `message`, an
//! optional source location (a JSON Pointer into the original request body)
//! and an optional [`RepairPatch`] the agent should apply before retrying.
//!
//! The goal is that a well-behaved agent can loop
//!     `tool_call -> error -> apply(repair_patch) -> retry`
//! until it commits, without a human in the loop.

use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use thiserror::Error;

/// Canonical error codes. Every tool error MUST use one of these (or add a new
/// entry here). Codes are `TOOL-E####` to keep them grep-friendly.
pub mod codes {
    /// Request body failed schema validation.
    pub const SCHEMA_VALIDATION: &str = "TOOL-E0001";
    /// Referenced entity / world / artifact was not found.
    pub const NOT_FOUND: &str = "TOOL-E0002";
    /// The tool's backend rejected the call with a structured reason.
    pub const BACKEND_REJECTED: &str = "TOOL-E0003";
    /// Compilation (DSL -> WASM) failed; the error carries the compile diagnostics.
    pub const COMPILE_FAILED: &str = "TOOL-E0004";
    /// Simulation verdict was FAIL; the repair patch proposes what to tweak.
    pub const SIMULATION_FAILED: &str = "TOOL-E0005";
    /// Content moderation / UGC scan blocked the artifact.
    pub const MODERATION_BLOCKED: &str = "TOOL-E0006";
    /// Conflict (e.g. patch against a stale base CID).
    pub const CONFLICT: &str = "TOOL-E0007";
    /// Unknown tool name or unknown RPC method.
    pub const UNKNOWN_METHOD: &str = "TOOL-E0008";
    /// Internal server error (should be rare; included for completeness).
    pub const INTERNAL: &str = "TOOL-E0009";
    /// Missing or invalid bearer token.
    pub const UNAUTHORIZED: &str = "TOOL-E4010";
}

/// A single JSON Patch–style repair operation (RFC 6902 subset) the agent
/// should apply to the original request body before retrying.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum RepairOp {
    /// Replace the value at `path` with the given JSON value.
    Replace {
        path: String,
        value: serde_json::Value,
    },
    /// Add the given JSON value at `path`.
    Add {
        path: String,
        value: serde_json::Value,
    },
    /// Remove the value at `path`.
    Remove { path: String },
    /// Advisory hint: no automatic patch, but try the given hint string.
    Hint { path: String, hint: String },
}

/// A sequence of repair ops carried by an error envelope.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepairPatch {
    /// Ordered list of operations. Apply in order.
    pub ops: Vec<RepairOp>,
    /// Human-readable explanation of why the patch is needed.
    pub rationale: String,
}

impl RepairPatch {
    pub fn new(rationale: impl Into<String>) -> Self {
        Self {
            ops: Vec::new(),
            rationale: rationale.into(),
        }
    }

    pub fn with_op(mut self, op: RepairOp) -> Self {
        self.ops.push(op);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }
}

/// The error variant returned by every tool handler.
#[derive(Debug, Clone, Error)]
#[error("[{code}] {message}")]
pub struct ToolError {
    /// Machine-readable error code, e.g. `TOOL-E0001`.
    pub code: &'static str,
    /// Human-readable message. Keep short; structural detail belongs in `repair_patch`.
    pub message: String,
    /// Optional RFC 6901 JSON Pointer to the offending field in the request body.
    pub source_location: Option<String>,
    /// Optional one-line hint for a developer; agents should prefer `repair_patch`.
    pub suggested_fix: Option<String>,
    /// Structured repair patch the agent should apply before retrying.
    pub repair_patch: Option<RepairPatch>,
}

impl ToolError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            source_location: None,
            suggested_fix: None,
            repair_patch: None,
        }
    }

    pub fn at(mut self, pointer: impl Into<String>) -> Self {
        self.source_location = Some(pointer.into());
        self
    }

    pub fn suggest(mut self, hint: impl Into<String>) -> Self {
        self.suggested_fix = Some(hint.into());
        self
    }

    pub fn with_patch(mut self, patch: RepairPatch) -> Self {
        self.repair_patch = Some(patch);
        self
    }

    /// Convenience: schema-validation error pointing at `pointer` with a
    /// single `replace` repair op suggesting `value`.
    pub fn schema(message: impl Into<String>, pointer: impl Into<String>) -> Self {
        Self::new(codes::SCHEMA_VALIDATION, message).at(pointer)
    }

    /// Convenience: not-found error.
    pub fn not_found(kind: &str, id: impl Into<Cow<'static, str>>) -> Self {
        Self::new(
            codes::NOT_FOUND,
            format!("{} `{}` not found", kind, id.into()),
        )
    }
}

/// Canonical result alias used by every tool handler.
pub type ToolResult<T> = Result<T, ToolError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_chains_fields() {
        let err = ToolError::new(codes::SCHEMA_VALIDATION, "bad input")
            .at("/manifest_yaml")
            .suggest("expected YAML with a `name:` field")
            .with_patch(
                RepairPatch::new("manifest must declare a name").with_op(RepairOp::Replace {
                    path: "/manifest_yaml".into(),
                    value: serde_json::Value::String("name: default\n".into()),
                }),
            );
        assert_eq!(err.code, codes::SCHEMA_VALIDATION);
        assert_eq!(err.source_location.as_deref(), Some("/manifest_yaml"));
        assert!(err.suggested_fix.is_some());
        let patch = err.repair_patch.unwrap();
        assert_eq!(patch.ops.len(), 1);
    }

    #[test]
    fn repair_patch_is_serialisable() {
        let patch = RepairPatch::new("fix path")
            .with_op(RepairOp::Replace {
                path: "/x".into(),
                value: serde_json::json!(42),
            })
            .with_op(RepairOp::Remove {
                path: "/y".into(),
            })
            .with_op(RepairOp::Add {
                path: "/z".into(),
                value: serde_json::json!("hello"),
            })
            .with_op(RepairOp::Hint {
                path: "/w".into(),
                hint: "consider splitting".into(),
            });
        let json = serde_json::to_string(&patch).unwrap();
        let round: RepairPatch = serde_json::from_str(&json).unwrap();
        assert_eq!(round, patch);
    }

    #[test]
    fn display_includes_code() {
        let err = ToolError::new(codes::UNAUTHORIZED, "missing bearer");
        assert!(err.to_string().contains("TOOL-E4010"));
        assert!(err.to_string().contains("missing bearer"));
    }

    #[test]
    fn not_found_formats_identifier() {
        let err = ToolError::not_found("world", Cow::Borrowed("cid:xyz"));
        assert_eq!(err.code, codes::NOT_FOUND);
        assert!(err.message.contains("cid:xyz"));
    }
}
