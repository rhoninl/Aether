//! UGC tools: `ugc.upload`, `ugc.scan_status`, `ugc.approve`, `ugc.publish`.
//!
//! Frogo task 94.

use std::sync::Arc;

use crate::backend::Backend;
use crate::error::ToolError;
use crate::registry::{ToolDescriptor, ToolFn, ToolRegistry};

use super::{ensure_object, required_str};

fn schema_upload() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "required": ["uploader", "media_type", "payload_b64"],
        "properties": {
            "uploader": { "type": "string" },
            "media_type": { "type": "string" },
            "payload_b64": { "type": "string", "description": "Artifact payload, base64-encoded." }
        }
    })
}

fn schema_cid_only() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "required": ["cid"],
        "properties": { "cid": { "type": "string" } }
    })
}

pub fn register_in<B: Backend + 'static>(registry: &mut ToolRegistry, backend: Arc<B>) {
    {
        let b = backend.clone();
        let call: ToolFn = Arc::new(move |params| {
            ensure_object(&params)?;
            let uploader = required_str(&params, "uploader")?.to_string();
            let media_type = required_str(&params, "media_type")?.to_string();
            let payload = required_str(&params, "payload_b64")?.to_string();
            let art = b.upload_ugc(&uploader, &media_type, &payload)?;
            Ok(serde_json::to_value(art).map_err(|e| ToolError::new(
                crate::error::codes::INTERNAL,
                e.to_string(),
            ))?)
        });
        registry.register(
            ToolDescriptor {
                name: "ugc.upload".into(),
                description: "Upload a UGC artifact (base64 payload); queues it for scanning.".into(),
                input_schema: schema_upload(),
                mutates: true,
                streaming: false,
            },
            call,
        );
    }

    for (name, description, mutates, op) in [
        (
            "ugc.scan_status",
            "Return the current lifecycle state + moderation notes for an artifact.",
            false,
            UgcOp::Status,
        ),
        (
            "ugc.approve",
            "Mark a scanned artifact as approved for publish.",
            true,
            UgcOp::Approve,
        ),
        (
            "ugc.publish",
            "Publish an approved artifact.",
            true,
            UgcOp::Publish,
        ),
    ] {
        let b = backend.clone();
        let call: ToolFn = Arc::new(move |params| {
            ensure_object(&params)?;
            let cid = required_str(&params, "cid")?.to_string();
            let art = match op {
                UgcOp::Status => b.ugc_scan_status(&cid),
                UgcOp::Approve => b.ugc_approve(&cid),
                UgcOp::Publish => b.ugc_publish(&cid),
            }?;
            Ok(serde_json::to_value(art).map_err(|e| ToolError::new(
                crate::error::codes::INTERNAL,
                e.to_string(),
            ))?)
        });
        registry.register(
            ToolDescriptor {
                name: name.into(),
                description: description.into(),
                input_schema: schema_cid_only(),
                mutates,
                streaming: false,
            },
            call,
        );
    }
}

#[derive(Copy, Clone)]
enum UgcOp {
    Status,
    Approve,
    Publish,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::InMemoryBackend;
    use crate::error::codes;
    use base64::engine::general_purpose::STANDARD as BASE64;
    use base64::Engine as _;

    fn reg() -> ToolRegistry {
        let mut r = ToolRegistry::new();
        register_in(&mut r, Arc::new(InMemoryBackend::default()));
        r
    }

    #[test]
    fn upload_then_approve_then_publish() {
        let r = reg();
        let uploaded = r
            .call(
                "ugc.upload",
                serde_json::json!({
                    "uploader": "alice",
                    "media_type": "model/gltf",
                    "payload_b64": BASE64.encode(b"hello")
                }),
            )
            .unwrap();
        let cid = uploaded.get("cid").unwrap().as_str().unwrap().to_string();
        let approved = r
            .call("ugc.approve", serde_json::json!({"cid": cid}))
            .unwrap();
        assert_eq!(approved.get("status").unwrap(), "approved");
        let published = r
            .call("ugc.publish", serde_json::json!({"cid": cid}))
            .unwrap();
        assert_eq!(published.get("status").unwrap(), "published");
    }

    #[test]
    fn publish_before_approve_conflicts() {
        let r = reg();
        let uploaded = r
            .call(
                "ugc.upload",
                serde_json::json!({
                    "uploader": "bob",
                    "media_type": "model/gltf",
                    "payload_b64": BASE64.encode(b"x")
                }),
            )
            .unwrap();
        let cid = uploaded.get("cid").unwrap().as_str().unwrap().to_string();
        let err = r
            .call("ugc.publish", serde_json::json!({"cid": cid}))
            .unwrap_err();
        assert_eq!(err.code, codes::CONFLICT);
        assert!(err.repair_patch.is_some());
    }

    #[test]
    fn upload_rejects_bad_base64() {
        let r = reg();
        let err = r
            .call(
                "ugc.upload",
                serde_json::json!({
                    "uploader": "alice",
                    "media_type": "model/gltf",
                    "payload_b64": "!!!not-base64"
                }),
            )
            .unwrap_err();
        assert_eq!(err.code, codes::SCHEMA_VALIDATION);
    }
}
