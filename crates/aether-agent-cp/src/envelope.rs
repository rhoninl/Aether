//! JSON-RPC 2.0 wire envelope used by the MCP-stdio, MCP-WS and gRPC-compatible
//! transports.
//!
//! We intentionally stick to a conservative JSON-RPC framing rather than
//! adopting a proto schema — it lets us share one request/response shape across
//! all three transports. The gRPC transport wraps the same envelope in a simple
//! `u32 length-prefixed JSON` framing.
//!
//! Tool errors serialise through [`ToolErrorEnvelope`] so that the `code`,
//! `message`, `source_location` and `repair_patch` fields land in well-known
//! slots inside JSON-RPC's `error.data`.

use serde::{Deserialize, Serialize};

use crate::error::{RepairPatch, ToolError};

/// Canonical MCP / JSON-RPC method names exposed by this service.
pub mod methods {
    /// `tools/list` returns the registered tool descriptors.
    pub const TOOLS_LIST: &str = "tools/list";
    /// `ping` returns `{ "ok": true }`.
    pub const PING: &str = "ping";
}

/// A JSON-RPC 2.0 id. We accept string or number; null is not used.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum JsonRpcId {
    Num(i64),
    Str(String),
}

/// A JSON-RPC 2.0 request frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<JsonRpcId>,
    pub method: String,
    #[serde(default)]
    pub params: Option<serde_json::Value>,
    /// Agent service-account bearer token. Carried in-band on transports
    /// where a transport-level header is inconvenient (e.g. stdio).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<String>,
}

/// A JSON-RPC 2.0 response frame (either success or error).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<JsonRpcId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// The JSON-RPC error object. `data` carries a [`ToolErrorEnvelope`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// The payload nested in `JsonRpcError::data` when a tool fails.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolErrorEnvelope {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_location: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested_fix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repair_patch: Option<RepairPatch>,
}

impl From<ToolError> for ToolErrorEnvelope {
    fn from(e: ToolError) -> Self {
        Self {
            code: e.code.to_string(),
            message: e.message,
            source_location: e.source_location,
            suggested_fix: e.suggested_fix,
            repair_patch: e.repair_patch,
        }
    }
}

/// The payload returned to a successful `tools/*` call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSuccessEnvelope {
    /// Tool-specific result as an opaque JSON value.
    pub result: serde_json::Value,
}

/// Convenience union: a request either carries a named tool invocation or one
/// of the two built-in MCP meta methods.
#[derive(Debug, Clone)]
pub enum JsonRpcEnvelope {
    Request(JsonRpcRequest),
    Response(JsonRpcResponse),
}

/// JSON-RPC standard error codes we map to.
pub mod rpc_codes {
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;
    pub const UNAUTHORIZED: i32 = -32001;
}

/// Build a canonical JSON-RPC error response from a `ToolError`.
pub fn rpc_error_from_tool_error(id: Option<JsonRpcId>, err: ToolError) -> JsonRpcResponse {
    let rpc_code = match err.code {
        crate::error::codes::SCHEMA_VALIDATION => rpc_codes::INVALID_PARAMS,
        crate::error::codes::UNKNOWN_METHOD => rpc_codes::METHOD_NOT_FOUND,
        crate::error::codes::UNAUTHORIZED => rpc_codes::UNAUTHORIZED,
        _ => rpc_codes::INTERNAL_ERROR,
    };
    let envelope = ToolErrorEnvelope::from(err);
    let data = serde_json::to_value(&envelope).ok();
    JsonRpcResponse {
        jsonrpc: "2.0".into(),
        id,
        result: None,
        error: Some(JsonRpcError {
            code: rpc_code,
            message: envelope.message.clone(),
            data,
        }),
    }
}

/// Build a canonical JSON-RPC success response from a `serde_json::Value`.
pub fn rpc_success(id: Option<JsonRpcId>, value: serde_json::Value) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".into(),
        id,
        result: Some(value),
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{codes, RepairOp, RepairPatch};

    #[test]
    fn parse_jsonrpc_request_numeric_id() {
        let raw = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
        let req: JsonRpcRequest = serde_json::from_str(raw).unwrap();
        assert_eq!(req.method, "tools/list");
        assert!(matches!(req.id, Some(JsonRpcId::Num(1))));
    }

    #[test]
    fn parse_jsonrpc_request_string_id() {
        let raw = r#"{"jsonrpc":"2.0","id":"abc","method":"ping"}"#;
        let req: JsonRpcRequest = serde_json::from_str(raw).unwrap();
        assert!(matches!(req.id, Some(JsonRpcId::Str(ref s)) if s == "abc"));
    }

    #[test]
    fn tool_error_round_trip_through_envelope() {
        let err = ToolError::new(codes::SCHEMA_VALIDATION, "bad")
            .at("/field")
            .with_patch(RepairPatch::new("fix").with_op(RepairOp::Replace {
                path: "/field".into(),
                value: serde_json::json!("x"),
            }));
        let resp = rpc_error_from_tool_error(Some(JsonRpcId::Num(7)), err);
        let data = resp.error.unwrap().data.unwrap();
        let env: ToolErrorEnvelope = serde_json::from_value(data).unwrap();
        assert_eq!(env.code, codes::SCHEMA_VALIDATION);
        assert_eq!(env.source_location.as_deref(), Some("/field"));
        assert!(env.repair_patch.is_some());
    }

    #[test]
    fn success_response_has_result() {
        let resp = rpc_success(
            Some(JsonRpcId::Num(1)),
            serde_json::json!({"ok": true}),
        );
        assert!(resp.error.is_none());
        assert_eq!(resp.result.unwrap(), serde_json::json!({"ok": true}));
    }

    #[test]
    fn unauthorized_maps_to_rpc_code() {
        let err = ToolError::new(codes::UNAUTHORIZED, "missing");
        let resp = rpc_error_from_tool_error(Some(JsonRpcId::Num(0)), err);
        assert_eq!(resp.error.unwrap().code, rpc_codes::UNAUTHORIZED);
    }
}
