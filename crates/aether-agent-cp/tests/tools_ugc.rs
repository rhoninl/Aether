use aether_agent_cp::backend::InMemoryBackend;
use aether_agent_cp::error::codes;
use aether_agent_cp::tools::build_default_registry;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use std::sync::Arc;

#[test]
fn full_lifecycle_then_moderation_reject() {
    let r = build_default_registry(Arc::new(InMemoryBackend::default()));
    let up = r
        .call(
            "ugc.upload",
            serde_json::json!({
                "uploader": "agent-alice",
                "media_type": "model/gltf",
                "payload_b64": BASE64.encode(b"ok-payload")
            }),
        )
        .unwrap();
    let cid = up.get("cid").unwrap().as_str().unwrap().to_string();

    let status = r
        .call("ugc.scan_status", serde_json::json!({"cid": cid}))
        .unwrap();
    assert_eq!(status.get("status").unwrap(), "scanning");

    let approved = r
        .call("ugc.approve", serde_json::json!({"cid": cid}))
        .unwrap();
    assert_eq!(approved.get("status").unwrap(), "approved");

    let pub_ = r
        .call("ugc.publish", serde_json::json!({"cid": cid}))
        .unwrap();
    assert_eq!(pub_.get("status").unwrap(), "published");
}

#[test]
fn publish_before_approve_is_conflict() {
    let r = build_default_registry(Arc::new(InMemoryBackend::default()));
    let up = r
        .call(
            "ugc.upload",
            serde_json::json!({
                "uploader": "bob",
                "media_type": "model/gltf",
                "payload_b64": BASE64.encode(b"x")
            }),
        )
        .unwrap();
    let cid = up.get("cid").unwrap().as_str().unwrap().to_string();
    let err = r
        .call("ugc.publish", serde_json::json!({"cid": cid}))
        .unwrap_err();
    assert_eq!(err.code, codes::CONFLICT);
    assert!(err.repair_patch.is_some());
}

#[test]
fn moderation_report_blocks_subsequent_approval() {
    let r = build_default_registry(Arc::new(InMemoryBackend::default()));
    let up = r
        .call(
            "ugc.upload",
            serde_json::json!({
                "uploader": "mallory",
                "media_type": "model/gltf",
                "payload_b64": BASE64.encode(b"bad")
            }),
        )
        .unwrap();
    let cid = up.get("cid").unwrap().as_str().unwrap().to_string();
    let _ = r
        .call(
            "moderation.report",
            serde_json::json!({"artifact_cid": cid, "reason": "disallowed content"}),
        )
        .unwrap();
    let err = r
        .call("ugc.approve", serde_json::json!({"cid": cid}))
        .unwrap_err();
    assert_eq!(err.code, codes::MODERATION_BLOCKED);
}
