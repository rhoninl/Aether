//! Transport layer.
//!
//! Three transports ship in-tree:
//!
//! * [`mcp_stdio`]: line-delimited JSON-RPC over stdin/stdout. Primary, used
//!   by Claude-in-a-box.
//! * [`mcp_ws`]: line-delimited JSON-RPC over a raw TCP socket, framed as
//!   one JSON object per line. We don't implement the RFC 6455 WebSocket
//!   handshake in-tree (it pulls in a heavy dep); instead we expose a TCP
//!   transport at the configured port so the same envelope can be bridged
//!   by a thin reverse proxy (e.g. `websocat --text`). The WS framing lives
//!   behind the `transport-ws-tcp` feature; toggled on by default.
//! * [`grpc`]: length-delimited JSON envelope over TCP. This is the gRPC
//!   sibling channel used by high-volume batch agents. A proto schema lives
//!   alongside the transport as a doc artifact.
//!
//! Every transport accepts a shared [`ToolRegistry`](crate::ToolRegistry)
//! and an [`AuthVerifier`](crate::AuthVerifier). A single request/response
//! function — [`handle_envelope`] — is used by all three. That keeps the
//! semantics identical regardless of wire.

pub mod grpc;
pub mod mcp_stdio;
pub mod mcp_ws;

use std::sync::Arc;

use crate::auth::AuthVerifier;
use crate::envelope::{
    methods, rpc_codes, rpc_error_from_tool_error, rpc_success, JsonRpcError, JsonRpcRequest,
    JsonRpcResponse,
};
use crate::error::{codes, ToolError};
use crate::registry::ToolRegistry;

/// Default retry count tools do NOT do — this constant is exposed so the
/// binary can advertise it in the banner. (Tools are idempotent; retries are
/// the agent's responsibility.)
pub const ADVERTISED_TOOL_RETRIES: u8 = 0;

/// Handle one decoded JSON-RPC request. Returns a JSON-RPC response. This is
/// the single hot path every transport calls.
pub fn handle_envelope(
    req: JsonRpcRequest,
    registry: &ToolRegistry,
    auth: &AuthVerifier,
) -> JsonRpcResponse {
    if req.jsonrpc != "2.0" {
        return JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: req.id.clone(),
            result: None,
            error: Some(JsonRpcError {
                code: rpc_codes::INVALID_REQUEST,
                message: "jsonrpc version must be \"2.0\"".into(),
                data: None,
            }),
        };
    }

    match req.method.as_str() {
        methods::PING => rpc_success(req.id, serde_json::json!({"ok": true})),
        methods::TOOLS_LIST => {
            let descriptors = registry.describe_all();
            rpc_success(
                req.id,
                serde_json::json!({
                    "tools": descriptors,
                    "count": descriptors.len()
                }),
            )
        }
        tool_name => {
            // Every other method requires auth.
            if let Err(err) = authenticate(&req, auth) {
                return rpc_error_from_tool_error(req.id, err);
            }
            let params = req.params.unwrap_or(serde_json::Value::Null);
            match registry.call(tool_name, params) {
                Ok(value) => rpc_success(req.id, value),
                Err(err) => rpc_error_from_tool_error(req.id, err),
            }
        }
    }
}

fn authenticate(req: &JsonRpcRequest, auth: &AuthVerifier) -> Result<(), ToolError> {
    let token = req
        .auth
        .as_deref()
        .or_else(|| {
            req.params
                .as_ref()
                .and_then(|p| p.get("_auth"))
                .and_then(|v| v.as_str())
        })
        .ok_or_else(|| {
            ToolError::new(codes::UNAUTHORIZED, "missing Authorization bearer token")
                .suggest(format!(
                    "include an `auth` field with `Bearer <jwt>` minted by the identity service at {}",
                    auth.jwks_url()
                ))
        })?;
    let token = AuthVerifier::parse_bearer(token).unwrap_or(token);
    auth.validate(token)?;
    Ok(())
}

/// Parse a JSON-RPC request from raw bytes. Returns a canonical error response
/// on parse failure.
pub fn decode_or_error(raw: &[u8]) -> Result<JsonRpcRequest, JsonRpcResponse> {
    serde_json::from_slice::<JsonRpcRequest>(raw).map_err(|e| JsonRpcResponse {
        jsonrpc: "2.0".into(),
        id: None,
        result: None,
        error: Some(JsonRpcError {
            code: rpc_codes::PARSE_ERROR,
            message: format!("invalid JSON: {}", e),
            data: None,
        }),
    })
}

/// Shared handle + auth verifier used by long-running transports.
#[derive(Clone)]
pub struct SharedState {
    pub registry: Arc<ToolRegistry>,
    pub auth: AuthVerifier,
}

impl SharedState {
    pub fn new(registry: Arc<ToolRegistry>, auth: AuthVerifier) -> Self {
        Self { registry, auth }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{test_support, AuthConfig};
    use crate::backend::InMemoryBackend;
    use crate::envelope::JsonRpcId;
    use crate::tools::build_default_registry;

    const TEST_SECRET: &str = "transport-unit-test";

    fn shared_state() -> SharedState {
        let backend = Arc::new(InMemoryBackend::default());
        let registry = Arc::new(build_default_registry(backend));
        let auth = AuthVerifier::new(AuthConfig {
            identity_jwks_url: "http://identity".into(),
            required_role: None,
            hs256_secret: TEST_SECRET.into(),
        });
        SharedState::new(registry, auth)
    }

    fn fresh_token() -> String {
        test_support::mint_token(TEST_SECRET, "user", 60)
    }

    #[test]
    fn tools_list_does_not_require_auth() {
        let st = shared_state();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(JsonRpcId::Num(1)),
            method: "tools/list".into(),
            params: None,
            auth: None,
        };
        let resp = handle_envelope(req, &st.registry, &st.auth);
        let r = resp.result.unwrap();
        assert_eq!(r.get("count").unwrap().as_u64().unwrap(), 15);
    }

    #[test]
    fn ping_does_not_require_auth() {
        let st = shared_state();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(JsonRpcId::Num(2)),
            method: "ping".into(),
            params: None,
            auth: None,
        };
        let resp = handle_envelope(req, &st.registry, &st.auth);
        assert_eq!(resp.result.unwrap(), serde_json::json!({"ok": true}));
    }

    #[test]
    fn tool_call_without_auth_returns_4010() {
        let st = shared_state();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(JsonRpcId::Num(3)),
            method: "world.create".into(),
            params: Some(serde_json::json!({"manifest_yaml": "name: x\n"})),
            auth: None,
        };
        let resp = handle_envelope(req, &st.registry, &st.auth);
        let err = resp.error.unwrap();
        let data = err.data.unwrap();
        assert_eq!(data.get("code").unwrap(), codes::UNAUTHORIZED);
    }

    #[test]
    fn tool_call_with_auth_succeeds() {
        let st = shared_state();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(JsonRpcId::Num(4)),
            method: "world.create".into(),
            params: Some(serde_json::json!({"manifest_yaml": "name: x\n"})),
            auth: Some(format!("Bearer {}", fresh_token())),
        };
        let resp = handle_envelope(req, &st.registry, &st.auth);
        assert!(resp.error.is_none());
        assert!(resp.result.unwrap().get("cid").is_some());
    }

    #[test]
    fn invalid_jsonrpc_version_rejected() {
        let st = shared_state();
        let req = JsonRpcRequest {
            jsonrpc: "1.0".into(),
            id: None,
            method: "ping".into(),
            params: None,
            auth: None,
        };
        let resp = handle_envelope(req, &st.registry, &st.auth);
        assert_eq!(resp.error.unwrap().code, rpc_codes::INVALID_REQUEST);
    }
}
