//! Entity tools: `entity.spawn`, `entity.modify`, `entity.link`.
//!
//! Frogo task 91.

use std::sync::Arc;

use crate::backend::Backend;
use crate::error::ToolError;
use crate::registry::{ToolDescriptor, ToolFn, ToolRegistry};

use super::{ensure_object, required_str};

fn schema_spawn() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "required": ["world_cid", "prototypes"],
        "properties": {
            "world_cid": { "type": "string" },
            "prototypes": {
                "type": "array",
                "minItems": 1,
                "items": {
                    "type": "object",
                    "required": ["id"],
                    "properties": { "id": { "type": "string" } }
                }
            }
        }
    })
}

fn schema_modify() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "required": ["world_cid", "ops"],
        "properties": {
            "world_cid": { "type": "string" },
            "ops": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["op", "id"],
                    "properties": {
                        "op": { "enum": ["set", "remove"] },
                        "id": { "type": "string" },
                        "value": {}
                    }
                }
            }
        }
    })
}

fn schema_link() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "required": ["world_cid", "source_id", "target_id", "link_kind"],
        "properties": {
            "world_cid": { "type": "string" },
            "source_id": { "type": "string" },
            "target_id": { "type": "string" },
            "link_kind": { "type": "string" }
        }
    })
}

pub fn register_in<B: Backend + 'static>(registry: &mut ToolRegistry, backend: Arc<B>) {
    {
        let b = backend.clone();
        let call: ToolFn = Arc::new(move |params| {
            ensure_object(&params)?;
            let world_cid = required_str(&params, "world_cid")?.to_string();
            let protos = params
                .get("prototypes")
                .and_then(|v| v.as_array())
                .ok_or_else(|| {
                    ToolError::schema(
                        "missing required array `prototypes`",
                        "/prototypes",
                    )
                })?
                .clone();
            let w = b.spawn_entities(&world_cid, &protos)?;
            Ok(serde_json::to_value(w).map_err(|e| ToolError::new(
                crate::error::codes::INTERNAL,
                e.to_string(),
            ))?)
        });
        registry.register(
            ToolDescriptor {
                name: "entity.spawn".into(),
                description: "Spawn a batch of entity prototypes into a world.".into(),
                input_schema: schema_spawn(),
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
            let world_cid = required_str(&params, "world_cid")?.to_string();
            let ops = params
                .get("ops")
                .and_then(|v| v.as_array())
                .ok_or_else(|| {
                    ToolError::schema("missing required array `ops`", "/ops")
                })?
                .clone();
            let w = b.modify_entities(&world_cid, &ops)?;
            Ok(serde_json::to_value(w).map_err(|e| ToolError::new(
                crate::error::codes::INTERNAL,
                e.to_string(),
            ))?)
        });
        registry.register(
            ToolDescriptor {
                name: "entity.modify".into(),
                description: "Apply a batch of set/remove ops against entities in a world.".into(),
                input_schema: schema_modify(),
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
            let world_cid = required_str(&params, "world_cid")?.to_string();
            let source_id = required_str(&params, "source_id")?.to_string();
            let target_id = required_str(&params, "target_id")?.to_string();
            let link_kind = required_str(&params, "link_kind")?.to_string();
            let w = b.link_entities(&world_cid, &source_id, &target_id, &link_kind)?;
            Ok(serde_json::to_value(w).map_err(|e| ToolError::new(
                crate::error::codes::INTERNAL,
                e.to_string(),
            ))?)
        });
        registry.register(
            ToolDescriptor {
                name: "entity.link".into(),
                description: "Create a directed link between two entities.".into(),
                input_schema: schema_link(),
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

    fn reg_with_world() -> (ToolRegistry, String) {
        let mut r = ToolRegistry::new();
        let b = Arc::new(InMemoryBackend::default());
        crate::tools::world::register_in(&mut r, b.clone());
        register_in(&mut r, b);
        let created = r
            .call(
                "world.create",
                serde_json::json!({"manifest_yaml": "name: w\n"}),
            )
            .unwrap();
        let cid = created.get("cid").unwrap().as_str().unwrap().to_string();
        (r, cid)
    }

    #[test]
    fn entity_spawn_success() {
        let (r, cid) = reg_with_world();
        let out = r
            .call(
                "entity.spawn",
                serde_json::json!({
                    "world_cid": cid,
                    "prototypes": [{"id":"a"}, {"id":"b"}]
                }),
            )
            .unwrap();
        assert_eq!(out.get("entities").unwrap().as_object().unwrap().len(), 2);
    }

    #[test]
    fn entity_spawn_rejects_empty_batch() {
        let (r, cid) = reg_with_world();
        let err = r
            .call(
                "entity.spawn",
                serde_json::json!({ "world_cid": cid, "prototypes": [] }),
            )
            .unwrap_err();
        assert_eq!(err.code, codes::SCHEMA_VALIDATION);
    }

    #[test]
    fn entity_modify_unknown_op_is_structured_error() {
        let (r, cid) = reg_with_world();
        let spawned = r
            .call(
                "entity.spawn",
                serde_json::json!({"world_cid": cid, "prototypes": [{"id":"a"}]}),
            )
            .unwrap();
        let new_cid = spawned.get("cid").unwrap().as_str().unwrap().to_string();
        let err = r
            .call(
                "entity.modify",
                serde_json::json!({
                    "world_cid": new_cid,
                    "ops": [{"op":"fly","id":"a"}]
                }),
            )
            .unwrap_err();
        assert_eq!(err.code, codes::SCHEMA_VALIDATION);
        assert!(err.repair_patch.is_some());
    }

    #[test]
    fn entity_link_requires_existing_entities() {
        let (r, cid) = reg_with_world();
        let err = r
            .call(
                "entity.link",
                serde_json::json!({
                    "world_cid": cid, "source_id": "x",
                    "target_id": "y", "link_kind": "follows"
                }),
            )
            .unwrap_err();
        assert_eq!(err.code, codes::NOT_FOUND);
    }
}
