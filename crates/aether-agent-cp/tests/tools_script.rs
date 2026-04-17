use aether_agent_cp::backend::InMemoryBackend;
use aether_agent_cp::error::codes;
use aether_agent_cp::tools::build_default_registry;
use std::sync::Arc;

#[test]
fn compile_success_returns_wasm_and_cid() {
    let r = build_default_registry(Arc::new(InMemoryBackend::default()));
    let out = r
        .call(
            "script.compile",
            serde_json::json!({"dsl_source": "on tick do log \"hi\""}),
        )
        .unwrap();
    assert!(out.get("cid").unwrap().as_str().unwrap().starts_with("cid:"));
    assert!(!out.get("wasm_bytes_b64").unwrap().as_str().unwrap().is_empty());
    assert!(out.get("source_hash").unwrap().as_str().unwrap().len() == 64);
}

#[test]
fn compile_without_handler_returns_structured_compile_error() {
    let r = build_default_registry(Arc::new(InMemoryBackend::default()));
    let err = r
        .call(
            "script.compile",
            serde_json::json!({"dsl_source": "let x = 1"}),
        )
        .unwrap_err();
    assert_eq!(err.code, codes::COMPILE_FAILED);
    let patch = err.repair_patch.unwrap();
    assert!(!patch.ops.is_empty());
}

#[test]
fn deploy_happy_path() {
    let r = build_default_registry(Arc::new(InMemoryBackend::default()));
    let w = r
        .call(
            "world.create",
            serde_json::json!({"manifest_yaml": "name: w\n"}),
        )
        .unwrap();
    let world_cid = w.get("cid").unwrap().as_str().unwrap().to_string();
    let w = r
        .call(
            "entity.spawn",
            serde_json::json!({"world_cid": world_cid, "prototypes": [{"id":"e1"}]}),
        )
        .unwrap();
    let world_cid = w.get("cid").unwrap().as_str().unwrap().to_string();
    let compiled = r
        .call(
            "script.compile",
            serde_json::json!({"dsl_source": "on tick do noop"}),
        )
        .unwrap();
    let script_cid = compiled.get("cid").unwrap().as_str().unwrap().to_string();
    let deployed = r
        .call(
            "script.deploy",
            serde_json::json!({
                "world_cid": world_cid,
                "entity_ref": "e1",
                "script_cid": script_cid
            }),
        )
        .unwrap();
    assert_eq!(
        deployed
            .get("scripts")
            .unwrap()
            .as_object()
            .unwrap()
            .len(),
        1
    );
}
