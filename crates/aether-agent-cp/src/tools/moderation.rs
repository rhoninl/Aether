//! Moderation tool: `moderation.report`.
//!
//! Part of frogo task 94 — filed in its own module because the shape of a
//! moderation report is distinct from the UGC lifecycle tools.

use std::sync::Arc;

use crate::backend::Backend;
use crate::error::ToolError;
use crate::registry::{ToolDescriptor, ToolFn, ToolRegistry};

use super::{ensure_object, required_str};

fn schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "required": ["artifact_cid", "reason"],
        "properties": {
            "artifact_cid": { "type": "string" },
            "reason": { "type": "string", "minLength": 1 }
        }
    })
}

pub fn register_in<B: Backend + 'static>(registry: &mut ToolRegistry, backend: Arc<B>) {
    let b = backend;
    let call: ToolFn = Arc::new(move |params| {
        ensure_object(&params)?;
        let cid = required_str(&params, "artifact_cid")?.to_string();
        let reason = required_str(&params, "reason")?.to_string();
        let report = b.report_moderation(&cid, &reason)?;
        Ok(serde_json::to_value(report).map_err(|e| ToolError::new(
            crate::error::codes::INTERNAL,
            e.to_string(),
        ))?)
    });
    registry.register(
        ToolDescriptor {
            name: "moderation.report".into(),
            description: "File a moderation report against a UGC artifact. Also moves the artifact to `rejected`.".into(),
            input_schema: schema(),
            mutates: true,
            streaming: false,
        },
        call,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::InMemoryBackend;
    use crate::error::codes;
    use base64::engine::general_purpose::STANDARD as BASE64;
    use base64::Engine as _;

    fn full_reg() -> ToolRegistry {
        let mut r = ToolRegistry::new();
        let b = Arc::new(InMemoryBackend::default());
        crate::tools::ugc::register_in(&mut r, b.clone());
        register_in(&mut r, b);
        r
    }

    #[test]
    fn report_requires_nonempty_reason() {
        let r = full_reg();
        let err = r
            .call(
                "moderation.report",
                serde_json::json!({"artifact_cid": "cid:x", "reason": ""}),
            )
            .unwrap_err();
        assert_eq!(err.code, codes::SCHEMA_VALIDATION);
    }

    #[test]
    fn report_rejects_unknown_artifact() {
        let r = full_reg();
        let err = r
            .call(
                "moderation.report",
                serde_json::json!({"artifact_cid": "cid:missing", "reason": "bad"}),
            )
            .unwrap_err();
        assert_eq!(err.code, codes::NOT_FOUND);
    }

    #[test]
    fn report_succeeds_and_returns_report_id() {
        let r = full_reg();
        let uploaded = r
            .call(
                "ugc.upload",
                serde_json::json!({
                    "uploader": "a", "media_type": "x/y",
                    "payload_b64": BASE64.encode(b"hi")
                }),
            )
            .unwrap();
        let cid = uploaded.get("cid").unwrap().as_str().unwrap().to_string();
        let out = r
            .call(
                "moderation.report",
                serde_json::json!({"artifact_cid": cid, "reason": "contains slurs"}),
            )
            .unwrap();
        assert!(out.get("report_id").unwrap().as_str().unwrap().starts_with("mr_"));
    }
}
