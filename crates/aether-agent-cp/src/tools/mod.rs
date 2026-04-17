//! Tool implementations.
//!
//! Every submodule exposes at least one `register_in(&mut ToolRegistry,
//! backend)` function. [`build_default_registry`] wires them all into a single
//! ready-to-serve registry.

pub mod entity;
pub mod moderation;
pub mod script;
pub mod sim;
pub mod telemetry;
pub mod ugc;
pub mod world;

use std::sync::Arc;

use crate::backend::Backend;
use crate::registry::ToolRegistry;

/// Wire all built-in tools against the given backend.
pub fn build_default_registry<B: Backend + 'static>(backend: Arc<B>) -> ToolRegistry {
    let mut r = ToolRegistry::new();
    world::register_in(&mut r, backend.clone());
    entity::register_in(&mut r, backend.clone());
    script::register_in(&mut r, backend.clone());
    sim::register_in(&mut r, backend.clone());
    ugc::register_in(&mut r, backend.clone());
    moderation::register_in(&mut r, backend.clone());
    telemetry::register_in(&mut r, backend);
    r
}

/// Helper: pull a required string field from a params object, with a helpful
/// schema-validation error when missing.
pub(crate) fn required_str<'a>(
    params: &'a serde_json::Value,
    field: &'static str,
) -> Result<&'a str, crate::error::ToolError> {
    params
        .get(field)
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            crate::error::ToolError::schema(
                format!("missing required string field `{}`", field),
                format!("/{}", field),
            )
        })
}

/// Helper: ensure params is a JSON object, with a clear error otherwise.
pub(crate) fn ensure_object(params: &serde_json::Value) -> Result<(), crate::error::ToolError> {
    if params.is_object() {
        Ok(())
    } else {
        Err(crate::error::ToolError::schema(
            "params must be a JSON object",
            "",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::InMemoryBackend;

    #[test]
    fn default_registry_has_all_tools() {
        let b = Arc::new(InMemoryBackend::default());
        let r = build_default_registry(b);
        let names = r.tool_names();
        // world (3) + entity (3) + script (2) + sim (1) + ugc (4) + moderation (1) + telemetry (1) = 15.
        assert_eq!(names.len(), 15, "got {:?}", names);
        for expected in [
            "world.create",
            "world.patch",
            "world.query",
            "entity.spawn",
            "entity.modify",
            "entity.link",
            "script.compile",
            "script.deploy",
            "sim.run",
            "ugc.upload",
            "ugc.scan_status",
            "ugc.approve",
            "ugc.publish",
            "moderation.report",
            "telemetry.stream",
        ] {
            assert!(
                names.iter().any(|n| n == expected),
                "missing tool `{}`",
                expected
            );
        }
    }
}
