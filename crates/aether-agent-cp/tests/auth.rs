//! Integration tests for the auth surface.

use aether_agent_cp::auth::{test_support, AuthConfig, AuthVerifier};
use aether_agent_cp::backend::InMemoryBackend;
use aether_agent_cp::envelope::{JsonRpcId, JsonRpcRequest};
use aether_agent_cp::error::codes;
use aether_agent_cp::tools::build_default_registry;
use aether_agent_cp::transport::{handle_envelope, SharedState};
use std::sync::Arc;

fn shared(secret: &str) -> SharedState {
    let backend = Arc::new(InMemoryBackend::default());
    let registry = Arc::new(build_default_registry(backend));
    let auth = AuthVerifier::new(AuthConfig {
        identity_jwks_url: "http://identity".into(),
        required_role: None,
        hs256_secret: secret.into(),
    });
    SharedState::new(registry, auth)
}

#[test]
fn tool_call_without_token_returns_4010() {
    let state = shared("auth-it-secret");
    let req = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(JsonRpcId::Num(1)),
        method: "world.create".into(),
        params: Some(serde_json::json!({"manifest_yaml": "name: x\n"})),
        auth: None,
    };
    let resp = handle_envelope(req, &state.registry, &state.auth);
    let data = resp.error.unwrap().data.unwrap();
    assert_eq!(data.get("code").unwrap(), codes::UNAUTHORIZED);
    assert!(data.get("suggested_fix").unwrap().as_str().unwrap().contains("identity"));
}

#[test]
fn tool_call_with_valid_token_succeeds() {
    let state = shared("auth-it-secret");
    let tok = test_support::mint_token("auth-it-secret", "user", 60);
    let req = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(JsonRpcId::Num(2)),
        method: "world.create".into(),
        params: Some(serde_json::json!({"manifest_yaml": "name: x\n"})),
        auth: Some(format!("Bearer {}", tok)),
    };
    let resp = handle_envelope(req, &state.registry, &state.auth);
    assert!(resp.error.is_none(), "expected success, got {:?}", resp.error);
    assert!(resp.result.unwrap().get("cid").is_some());
}

#[test]
fn tools_list_never_requires_auth() {
    let state = shared("auth-it-secret");
    let req = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(JsonRpcId::Num(3)),
        method: "tools/list".into(),
        params: None,
        auth: None,
    };
    let resp = handle_envelope(req, &state.registry, &state.auth);
    assert!(resp.error.is_none());
}

#[test]
fn wrong_secret_rejected() {
    let state = shared("auth-it-secret");
    let bad = test_support::mint_token("different-secret", "user", 60);
    let req = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(JsonRpcId::Num(4)),
        method: "world.create".into(),
        params: Some(serde_json::json!({"manifest_yaml": "name: x\n"})),
        auth: Some(format!("Bearer {}", bad)),
    };
    let resp = handle_envelope(req, &state.registry, &state.auth);
    let code = resp.error.unwrap().data.unwrap().get("code").unwrap().clone();
    assert_eq!(code, codes::UNAUTHORIZED);
}
