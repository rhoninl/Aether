//! End-to-end coverage of the world tools through the registry.

use aether_agent_cp::backend::InMemoryBackend;
use aether_agent_cp::error::codes;
use aether_agent_cp::tools::build_default_registry;
use std::sync::Arc;

#[test]
fn create_then_patch_then_query() {
    let registry = build_default_registry(Arc::new(InMemoryBackend::default()));

    let created = registry
        .call(
            "world.create",
            serde_json::json!({"manifest_yaml": "name: rose-garden\nversion: 1\n"}),
        )
        .unwrap();
    let cid0 = created.get("cid").unwrap().as_str().unwrap().to_string();

    let patched = registry
        .call(
            "world.patch",
            serde_json::json!({
                "base_cid": cid0,
                "patch": {
                    "manifest_merge": {"biome": "garden"},
                    "entities": {"add": [{"id":"rose","kind":"flower"}]}
                }
            }),
        )
        .unwrap();
    let cid1 = patched.get("cid").unwrap().as_str().unwrap().to_string();
    assert_ne!(cid0, cid1);

    let q = registry
        .call(
            "world.query",
            serde_json::json!({"cid": cid1, "jsonpath": "/manifest/biome"}),
        )
        .unwrap();
    assert_eq!(q, serde_json::json!("garden"));
}

#[test]
fn world_create_structured_error_has_repair_patch() {
    let registry = build_default_registry(Arc::new(InMemoryBackend::default()));
    let err = registry
        .call(
            "world.create",
            serde_json::json!({"manifest_yaml": "version: 1\n"}),
        )
        .unwrap_err();
    assert_eq!(err.code, codes::SCHEMA_VALIDATION);
    let patch = err.repair_patch.expect("repair patch must be attached");
    assert!(!patch.ops.is_empty());
}
