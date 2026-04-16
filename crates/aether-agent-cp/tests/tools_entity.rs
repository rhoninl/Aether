use aether_agent_cp::backend::InMemoryBackend;
use aether_agent_cp::error::codes;
use aether_agent_cp::tools::build_default_registry;
use std::sync::Arc;

fn reg_with_world() -> (aether_agent_cp::ToolRegistry, String) {
    let r = build_default_registry(Arc::new(InMemoryBackend::default()));
    let w = r
        .call(
            "world.create",
            serde_json::json!({"manifest_yaml": "name: w\n"}),
        )
        .unwrap();
    let cid = w.get("cid").unwrap().as_str().unwrap().to_string();
    (r, cid)
}

#[test]
fn spawn_modify_link_happy_path() {
    let (r, cid) = reg_with_world();
    let spawned = r
        .call(
            "entity.spawn",
            serde_json::json!({
                "world_cid": cid,
                "prototypes": [{"id":"a","kind":"npc"}, {"id":"b","kind":"npc"}]
            }),
        )
        .unwrap();
    let cid = spawned.get("cid").unwrap().as_str().unwrap().to_string();

    let modified = r
        .call(
            "entity.modify",
            serde_json::json!({
                "world_cid": cid,
                "ops": [{"op":"set","id":"a","value":{"id":"a","hp":99}}]
            }),
        )
        .unwrap();
    let cid = modified.get("cid").unwrap().as_str().unwrap().to_string();

    let linked = r
        .call(
            "entity.link",
            serde_json::json!({
                "world_cid": cid,
                "source_id": "a",
                "target_id": "b",
                "link_kind": "guards"
            }),
        )
        .unwrap();
    assert_eq!(linked.get("links").unwrap().as_array().unwrap().len(), 1);
}

#[test]
fn spawn_rejects_prototype_without_id_with_pointer() {
    let (r, cid) = reg_with_world();
    let err = r
        .call(
            "entity.spawn",
            serde_json::json!({
                "world_cid": cid,
                "prototypes": [{"id":"a"}, {"kind":"noid"}]
            }),
        )
        .unwrap_err();
    assert_eq!(err.code, codes::SCHEMA_VALIDATION);
    assert_eq!(err.source_location.as_deref(), Some("/prototypes/1"));
}
