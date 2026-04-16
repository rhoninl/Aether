//! Script tools: `script.compile`, `script.deploy`.
//!
//! Frogo task 92.

use std::sync::Arc;

use crate::backend::Backend;
use crate::error::ToolError;
use crate::registry::{ToolDescriptor, ToolFn, ToolRegistry};

use super::{ensure_object, required_str};

fn schema_compile() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "required": ["dsl_source"],
        "properties": {
            "dsl_source": {
                "type": "string",
                "description": "Behavior DSL source; must contain at least one `on <event> do ...` handler."
            }
        }
    })
}

fn schema_deploy() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "required": ["world_cid", "entity_ref", "script_cid"],
        "properties": {
            "world_cid": { "type": "string" },
            "entity_ref": { "type": "string" },
            "script_cid": { "type": "string" }
        }
    })
}

pub fn register_in<B: Backend + 'static>(registry: &mut ToolRegistry, backend: Arc<B>) {
    {
        let b = backend.clone();
        let call: ToolFn = Arc::new(move |params| {
            ensure_object(&params)?;
            let dsl = required_str(&params, "dsl_source")?;
            let compiled = b.compile_script(dsl)?;
            Ok(serde_json::to_value(compiled).map_err(|e| ToolError::new(
                crate::error::codes::INTERNAL,
                e.to_string(),
            ))?)
        });
        registry.register(
            ToolDescriptor {
                name: "script.compile".into(),
                description: "Compile behavior-DSL source to WASM and return the artifact cid.".into(),
                input_schema: schema_compile(),
                mutates: false,
                streaming: false,
            },
            call,
        );
    }

    {
        let b = backend;
        let call: ToolFn = Arc::new(move |params| {
            ensure_object(&params)?;
            let world_cid = required_str(&params, "world_cid")?.to_string();
            let entity_ref = required_str(&params, "entity_ref")?.to_string();
            let script_cid = required_str(&params, "script_cid")?.to_string();
            let w = b.deploy_script(&world_cid, &entity_ref, &script_cid)?;
            Ok(serde_json::to_value(w).map_err(|e| ToolError::new(
                crate::error::codes::INTERNAL,
                e.to_string(),
            ))?)
        });
        registry.register(
            ToolDescriptor {
                name: "script.deploy".into(),
                description: "Attach a compiled script to an entity in a world.".into(),
                input_schema: schema_deploy(),
                mutates: true,
                streaming: false,
            },
            call,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::InMemoryBackend;
    use crate::error::codes;

    fn reg() -> ToolRegistry {
        let mut r = ToolRegistry::new();
        register_in(&mut r, Arc::new(InMemoryBackend::default()));
        r
    }

    #[test]
    fn compile_rejects_source_without_handler() {
        let r = reg();
        let err = r
            .call(
                "script.compile",
                serde_json::json!({"dsl_source": "noop"}),
            )
            .unwrap_err();
        assert_eq!(err.code, codes::COMPILE_FAILED);
        assert!(err.repair_patch.is_some());
    }

    #[test]
    fn compile_returns_cid_and_wasm() {
        let r = reg();
        let out = r
            .call(
                "script.compile",
                serde_json::json!({"dsl_source": "on tick do log \"x\""}),
            )
            .unwrap();
        assert!(out.get("cid").unwrap().as_str().unwrap().starts_with("cid:"));
        assert!(!out.get("wasm_bytes_b64").unwrap().as_str().unwrap().is_empty());
    }

    #[test]
    fn deploy_requires_known_script() {
        let r = reg();
        let err = r
            .call(
                "script.deploy",
                serde_json::json!({
                    "world_cid": "cid:fake",
                    "entity_ref": "e",
                    "script_cid": "cid:nope"
                }),
            )
            .unwrap_err();
        assert_eq!(err.code, codes::NOT_FOUND);
    }
}
