use aether_agent_cp::backend::InMemoryBackend;
use aether_agent_cp::error::codes;
use aether_agent_cp::tools::build_default_registry;
use std::sync::Arc;

#[test]
fn sim_inconclusive_on_empty_world_surfaces_repair() {
    let r = build_default_registry(Arc::new(InMemoryBackend::default()));
    let w = r
        .call(
            "world.create",
            serde_json::json!({"manifest_yaml": "name: w\n"}),
        )
        .unwrap();
    let cid = w.get("cid").unwrap().as_str().unwrap().to_string();
    let err = r
        .call(
            "sim.run",
            serde_json::json!({"world_cid": cid, "scenario_yaml": "ticks: 3\n"}),
        )
        .unwrap_err();
    assert_eq!(err.code, codes::SIMULATION_FAILED);
    let patch = err.repair_patch.expect("expected repair_patch");
    assert!(!patch.rationale.is_empty());
}

#[test]
fn sim_pass_with_entities() {
    let r = build_default_registry(Arc::new(InMemoryBackend::default()));
    let w = r
        .call(
            "world.create",
            serde_json::json!({"manifest_yaml": "name: w\n"}),
        )
        .unwrap();
    let cid = w.get("cid").unwrap().as_str().unwrap().to_string();
    let w = r
        .call(
            "entity.spawn",
            serde_json::json!({"world_cid": cid, "prototypes": [{"id":"a"}]}),
        )
        .unwrap();
    let cid = w.get("cid").unwrap().as_str().unwrap().to_string();
    let out = r
        .call(
            "sim.run",
            serde_json::json!({"world_cid": cid, "scenario_yaml": "ticks: 2\nexpect: pass\n"}),
        )
        .unwrap();
    assert_eq!(out.get("verdict").unwrap(), "pass");
    assert!(out.get("telemetry").is_some());
}
