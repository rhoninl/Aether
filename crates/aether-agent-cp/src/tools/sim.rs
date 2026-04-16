//! Simulation tool: `sim.run`.
//!
//! Frogo task 93.

use std::sync::Arc;

use crate::backend::Backend;
use crate::error::ToolError;
use crate::registry::{ToolDescriptor, ToolFn, ToolRegistry};

use super::{ensure_object, required_str};

fn schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "required": ["world_cid", "scenario_yaml"],
        "properties": {
            "world_cid": { "type": "string" },
            "scenario_yaml": {
                "type": "string",
                "description": "Scenario script in YAML. Recognised keys: `ticks: u64`, `expect: pass|fail|inconclusive`."
            }
        }
    })
}

pub fn register_in<B: Backend + 'static>(registry: &mut ToolRegistry, backend: Arc<B>) {
    let b = backend;
    let call: ToolFn = Arc::new(move |params| {
        ensure_object(&params)?;
        let world_cid = required_str(&params, "world_cid")?.to_string();
        let scenario = required_str(&params, "scenario_yaml")?.to_string();
        let report = b.run_sim(&world_cid, &scenario)?;
        // If the verdict is not pass, bubble as a structured error with the repair patch.
        if !matches!(report.verdict, crate::backend::SimVerdict::Pass) {
            return Err(ToolError::new(
                crate::error::codes::SIMULATION_FAILED,
                format!("simulation verdict = {:?}", report.verdict),
            )
            .at("/scenario_yaml")
            .with_patch(report.repair_patch.unwrap_or_default()));
        }
        Ok(serde_json::to_value(report).map_err(|e| ToolError::new(
            crate::error::codes::INTERNAL,
            e.to_string(),
        ))?)
    });
    registry.register(
        ToolDescriptor {
            name: "sim.run".into(),
            description: "Run a scenario against a world and return a verdict with telemetry.".into(),
            input_schema: schema(),
            mutates: false,
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

    fn reg_with_world() -> (ToolRegistry, String) {
        let mut r = ToolRegistry::new();
        let b = Arc::new(InMemoryBackend::default());
        crate::tools::world::register_in(&mut r, b.clone());
        crate::tools::entity::register_in(&mut r, b.clone());
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
    fn sim_run_pass_with_entities() {
        let (r, cid) = reg_with_world();
        let spawned = r
            .call(
                "entity.spawn",
                serde_json::json!({"world_cid": cid, "prototypes": [{"id":"a"}]}),
            )
            .unwrap();
        let new_cid = spawned.get("cid").unwrap().as_str().unwrap().to_string();
        let out = r
            .call(
                "sim.run",
                serde_json::json!({"world_cid": new_cid, "scenario_yaml": "ticks: 5\n"}),
            )
            .unwrap();
        assert_eq!(out.get("verdict").unwrap(), "pass");
    }

    #[test]
    fn sim_run_inconclusive_bubbles_as_structured_error() {
        let (r, cid) = reg_with_world();
        let err = r
            .call(
                "sim.run",
                serde_json::json!({"world_cid": cid, "scenario_yaml": "ticks: 1\n"}),
            )
            .unwrap_err();
        assert_eq!(err.code, codes::SIMULATION_FAILED);
        assert!(err.repair_patch.is_some());
    }
}
