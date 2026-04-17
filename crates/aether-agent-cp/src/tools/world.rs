//! World-level tools: `world.create`, `world.patch`, `world.query`.
//!
//! Frogo task 90.

use std::sync::Arc;

use crate::backend::Backend;
use crate::error::ToolError;
use crate::registry::{ToolDescriptor, ToolFn, ToolRegistry};

use super::{ensure_object, required_str};

/// JSON Schema for `world.create`.
fn schema_world_create() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "required": ["manifest_yaml"],
        "properties": {
            "manifest_yaml": {
                "type": "string",
                "description": "World manifest as YAML text; must contain a `name` key."
            }
        }
    })
}

fn schema_world_patch() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "required": ["base_cid", "patch"],
        "properties": {
            "base_cid": { "type": "string" },
            "patch": {
                "type": "object",
                "description": "Structured patch: { manifest_merge: {..}, entities: { add: [..], remove: [..] } }"
            }
        }
    })
}

fn schema_world_query() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "required": ["cid", "jsonpath"],
        "properties": {
            "cid": { "type": "string" },
            "jsonpath": {
                "type": "string",
                "description": "Simplified JSON pointer into the world document (e.g. `/manifest/name`)"
            }
        }
    })
}

pub fn register_in<B: Backend + 'static>(registry: &mut ToolRegistry, backend: Arc<B>) {
    {
        let b = backend.clone();
        let call: ToolFn = Arc::new(move |params| {
            ensure_object(&params)?;
            let yaml = required_str(&params, "manifest_yaml")?;
            let w = b.create_world(yaml)?;
            Ok(serde_json::to_value(w).map_err(|e| ToolError::new(
                crate::error::codes::INTERNAL,
                e.to_string(),
            ))?)
        });
        registry.register(
            ToolDescriptor {
                name: "world.create".into(),
                description: "Create a new world from a YAML manifest. Returns the committed world state with its content-addressed cid.".into(),
                input_schema: schema_world_create(),
                mutates: true,
                streaming: false,
            },
            call,
        );
    }

    {
        let b = backend.clone();
        let call: ToolFn = Arc::new(move |params| {
            ensure_object(&params)?;
            let base_cid = required_str(&params, "base_cid")?.to_string();
            let patch = params.get("patch").cloned().ok_or_else(|| {
                ToolError::schema("missing required object `patch`", "/patch")
            })?;
            let w = b.patch_world(&base_cid, &patch)?;
            Ok(serde_json::to_value(w).map_err(|e| ToolError::new(
                crate::error::codes::INTERNAL,
                e.to_string(),
            ))?)
        });
        registry.register(
            ToolDescriptor {
                name: "world.patch".into(),
                description: "Apply a structured patch to an existing world, producing a new cid.".into(),
                input_schema: schema_world_patch(),
                mutates: true,
                streaming: false,
            },
            call,
        );
    }

    {
        let b = backend;
        let call: ToolFn = Arc::new(move |params| {
            ensure_object(&params)?;
            let cid = required_str(&params, "cid")?.to_string();
            let path = required_str(&params, "jsonpath")?.to_string();
            b.query_world(&cid, &path)
        });
        registry.register(
            ToolDescriptor {
                name: "world.query".into(),
                description: "Read a fragment of a world via JSON pointer.".into(),
                input_schema: schema_world_query(),
                mutates: false,
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
    fn world_create_success() {
        let r = reg();
        let out = r
            .call(
                "world.create",
                serde_json::json!({"manifest_yaml": "name: my-world\n"}),
            )
            .unwrap();
        assert!(out.get("cid").unwrap().as_str().unwrap().starts_with("cid:"));
    }

    #[test]
    fn world_create_missing_field() {
        let r = reg();
        let err = r.call("world.create", serde_json::json!({})).unwrap_err();
        assert_eq!(err.code, codes::SCHEMA_VALIDATION);
    }

    #[test]
    fn world_create_non_object_params() {
        let r = reg();
        let err = r.call("world.create", serde_json::json!("oops")).unwrap_err();
        assert_eq!(err.code, codes::SCHEMA_VALIDATION);
    }

    #[test]
    fn world_patch_success() {
        let r = reg();
        let created = r
            .call(
                "world.create",
                serde_json::json!({"manifest_yaml": "name: w1\n"}),
            )
            .unwrap();
        let cid = created.get("cid").unwrap().as_str().unwrap();
        let out = r
            .call(
                "world.patch",
                serde_json::json!({
                    "base_cid": cid,
                    "patch": { "entities": { "add": [{"id": "e1"}] } }
                }),
            )
            .unwrap();
        assert_ne!(out.get("cid").unwrap().as_str().unwrap(), cid);
    }

    #[test]
    fn world_patch_unknown_base() {
        let r = reg();
        let err = r
            .call(
                "world.patch",
                serde_json::json!({"base_cid": "cid:missing", "patch": {}}),
            )
            .unwrap_err();
        assert_eq!(err.code, codes::NOT_FOUND);
    }

    #[test]
    fn world_query_success() {
        let r = reg();
        let created = r
            .call(
                "world.create",
                serde_json::json!({"manifest_yaml": "name: hello\n"}),
            )
            .unwrap();
        let cid = created.get("cid").unwrap().as_str().unwrap();
        let out = r
            .call(
                "world.query",
                serde_json::json!({"cid": cid, "jsonpath": "/manifest/name"}),
            )
            .unwrap();
        assert_eq!(out, serde_json::json!("hello"));
    }
}
