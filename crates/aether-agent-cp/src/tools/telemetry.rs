//! Telemetry tool: `telemetry.stream`.
//!
//! Frogo task 95. MCP over stdio delivers streaming as a JSON array in the
//! response `result`; the WS and gRPC transports can later promote this to a
//! true streaming channel. The handler always returns the full snapshot; the
//! transport is free to chunk it.

use std::sync::Arc;

use crate::backend::Backend;
use crate::registry::{ToolDescriptor, ToolFn, ToolRegistry};

use super::{ensure_object, required_str};

fn schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "required": ["world_cid"],
        "properties": {
            "world_cid": { "type": "string" },
            "filter": {
                "type": "string",
                "description": "Optional substring to match against event `kind`."
            }
        }
    })
}

pub fn register_in<B: Backend + 'static>(registry: &mut ToolRegistry, backend: Arc<B>) {
    let b = backend;
    let call: ToolFn = Arc::new(move |params| {
        ensure_object(&params)?;
        let world_cid = required_str(&params, "world_cid")?.to_string();
        let filter = params
            .get("filter")
            .and_then(|v| v.as_str())
            .map(str::to_owned);
        let events = b.telemetry_snapshot(&world_cid, filter.as_deref())?;
        Ok(serde_json::json!({
            "events": events,
            "streaming": false,
            "count": events.len(),
        }))
    });
    registry.register(
        ToolDescriptor {
            name: "telemetry.stream".into(),
            description: "Stream (or snapshot) world telemetry events filtered by `kind` substring.".into(),
            input_schema: schema(),
            mutates: false,
            streaming: true,
        },
        call,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::InMemoryBackend;
    use crate::error::codes;

    fn full_reg() -> (ToolRegistry, String) {
        let mut r = ToolRegistry::new();
        let b = Arc::new(InMemoryBackend::default());
        crate::tools::world::register_in(&mut r, b.clone());
        register_in(&mut r, b);
        let out = r
            .call(
                "world.create",
                serde_json::json!({"manifest_yaml": "name: w\n"}),
            )
            .unwrap();
        (
            r,
            out.get("cid").unwrap().as_str().unwrap().to_string(),
        )
    }

    #[test]
    fn telemetry_stream_returns_events() {
        let (r, cid) = full_reg();
        let out = r
            .call(
                "telemetry.stream",
                serde_json::json!({"world_cid": cid}),
            )
            .unwrap();
        assert!(out.get("events").unwrap().as_array().unwrap().len() >= 1);
    }

    #[test]
    fn telemetry_stream_requires_world_cid() {
        let r = ToolRegistry::new();
        let mut r = r;
        register_in(&mut r, Arc::new(InMemoryBackend::default()));
        let err = r
            .call("telemetry.stream", serde_json::json!({}))
            .unwrap_err();
        assert_eq!(err.code, codes::SCHEMA_VALIDATION);
    }
}
