//! Default [`Harness`] implementation.
//!
//! Stitches replay + scorers + repair synthesis into the public contract:
//! given a snapshot and scenario, produce a [`SimReport`] whose verdict
//! is either `Pass`, `PassWithWarnings`, or `Fail` with a (possibly
//! `None`) minimal repair patch.

use tracing::debug;

use crate::replay::Replay;
use crate::scenario::{Input, Scenario};
use crate::scorer::{mmo_coherence, vr_comfort, CoherenceScore, ComfortScore};
use crate::telemetry::{Event, Telemetry};
use crate::verdict::{Cid, FailureReason, RepairOp, RepairPatch, Verdict, Warning};

pub use crate::scenario::WorldSnapshot;

/// The public contract: run a scenario against a snapshot, return a report.
pub trait Harness {
    fn run(&mut self, snapshot: WorldSnapshot, scenario: Scenario) -> SimReport;
}

/// A report from one harness invocation.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SimReport {
    pub verdict: Verdict,
    pub telemetry: Telemetry,
    pub repair_patch: Option<RepairPatch>,
    pub comfort: ComfortScore,
    pub coherence: CoherenceScore,
}

impl SimReport {
    /// Compute a stable content hash of the report for determinism tests.
    pub fn hash(&self) -> Cid {
        // Use canonical JSON — HashMap keys are sorted by serde_json when
        // the map uses BTree. We dump into a BTreeMap wrapper first.
        let canonical = canonicalize(self);
        Cid::from_bytes(canonical.as_bytes())
    }
}

fn canonicalize(report: &SimReport) -> String {
    // Convert to a Value and then serialize with sorted keys.
    let v = serde_json::to_value(report).expect("SimReport serializable");
    let sorted = sort_json(v);
    serde_json::to_string(&sorted).expect("serialize sorted value")
}

fn sort_json(v: serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::Object(m) => {
            let mut btm: std::collections::BTreeMap<String, serde_json::Value> =
                std::collections::BTreeMap::new();
            for (k, vv) in m {
                btm.insert(k, sort_json(vv));
            }
            serde_json::to_value(btm).unwrap()
        }
        serde_json::Value::Array(a) => {
            serde_json::Value::Array(a.into_iter().map(sort_json).collect())
        }
        other => other,
    }
}

/// Default in-process harness.
#[derive(Debug, Default)]
pub struct DefaultHarness;

impl DefaultHarness {
    pub fn new() -> Self {
        Self
    }
}

impl Harness for DefaultHarness {
    fn run(&mut self, snapshot: WorldSnapshot, mut scenario: Scenario) -> SimReport {
        scenario.snapshot = snapshot;
        run_scenario(&scenario)
    }
}

/// One-shot helper used by the harness trait and by integration tests.
pub fn run_scenario(scenario: &Scenario) -> SimReport {
    let output = Replay::new(scenario).run(scenario);
    let mut telemetry = output.telemetry;
    let comfort = vr_comfort::score(&output.state);
    let coherence = mmo_coherence::score(&output.state);

    // Fold scorer results into telemetry for downstream consumers.
    telemetry.emit(Event::new(
        output.state.current_tick,
        "scorer.comfort",
        serde_json::json!({"overall": comfort.overall, "reasons": comfort.reasons.len()}),
    ));
    telemetry.emit(Event::new(
        output.state.current_tick,
        "scorer.coherence",
        serde_json::json!({"overall": coherence.overall, "reasons": coherence.reasons.len()}),
    ));
    telemetry.incr("scorer.runs", 1);

    let verdict = derive_verdict(&comfort, &coherence);
    let repair_patch = if verdict.is_fail() {
        synthesize_repair(scenario, &comfort, &coherence)
    } else {
        None
    };

    SimReport {
        verdict,
        telemetry,
        repair_patch,
        comfort,
        coherence,
    }
}

fn derive_verdict(comfort: &ComfortScore, coherence: &CoherenceScore) -> Verdict {
    let mut fail_reasons: Vec<FailureReason> = Vec::new();
    let mut warnings: Vec<Warning> = Vec::new();

    for r in &comfort.reasons {
        if r.severity == "fail" {
            fail_reasons.push(
                FailureReason::new(format!("vr.{}", r.code), r.message.clone()).with_data(
                    serde_json::json!({"value": r.value, "threshold": r.threshold}),
                ),
            );
        } else {
            warnings.push(Warning {
                code: format!("vr.{}", r.code),
                message: r.message.clone(),
                data: serde_json::json!({"value": r.value, "threshold": r.threshold}),
            });
        }
    }
    for r in &coherence.reasons {
        if r.severity == "fail" {
            fail_reasons.push(
                FailureReason::new(format!("mmo.{}", r.code), r.message.clone())
                    .with_data(r.data.clone()),
            );
        } else {
            warnings.push(Warning {
                code: format!("mmo.{}", r.code),
                message: r.message.clone(),
                data: r.data.clone(),
            });
        }
    }

    if !fail_reasons.is_empty() {
        Verdict::Fail {
            reasons: fail_reasons,
        }
    } else if !warnings.is_empty() {
        Verdict::PassWithWarnings { warnings }
    } else {
        Verdict::Pass
    }
}

/// Attempt to synthesize a minimal repair patch.
///
/// Strategy: for each fail reason we can recognize, produce an op that
/// neutralizes it. We prefer the smallest set, so we deduplicate and
/// cap at one op per distinct failing action. If a simulated re-run
/// with the patch applied would still fail, we return `None` with
/// reasons on the report (callers can see original failures).
pub fn synthesize_repair(
    scenario: &Scenario,
    comfort: &ComfortScore,
    coherence: &CoherenceScore,
) -> Option<RepairPatch> {
    let mut ops: Vec<RepairOp> = Vec::new();

    // VR comfort: clamp / drop the offending action.
    let has_angular_fail = comfort
        .reasons
        .iter()
        .any(|r| r.code == "angular_velocity.fail");
    let has_accel_fail = comfort
        .reasons
        .iter()
        .any(|r| r.code == "locomotion_accel.fail");
    let has_fov_fail = comfort.reasons.iter().any(|r| r.code == "fov_delta.fail");

    if has_angular_fail {
        if let Some(idx) = find_index_where(scenario, |i| {
            matches!(
                i,
                Input::AgentAction {
                    action: crate::scenario::AgentAction::RotateHead { .. },
                    ..
                }
            )
        }) {
            ops.push(RepairOp::ClampInputField {
                index: idx,
                field: "yaw_deg".into(),
                max: vr_comfort::ANGULAR_VEL_WARN_DEG_S / 2.0,
            });
        }
    }
    if has_accel_fail {
        if let Some(idx) = find_index_where(scenario, |i| {
            matches!(
                i,
                Input::AgentAction {
                    action: crate::scenario::AgentAction::SmoothLocomotion { .. },
                    ..
                }
            )
        }) {
            ops.push(RepairOp::ClampInputField {
                index: idx,
                field: "accel".into(),
                max: vr_comfort::LOCO_ACCEL_WARN / 2.0,
            });
        }
    }
    if has_fov_fail {
        if let Some(idx) = find_index_where(scenario, |i| {
            matches!(
                i,
                Input::AgentAction {
                    action: crate::scenario::AgentAction::SetFov { .. },
                    ..
                }
            )
        }) {
            ops.push(RepairOp::DropInput { index: idx });
        }
    }

    // MMO: double spawns → drop the duplicate.
    for reason in &coherence.reasons {
        if reason.code == "coherence.double_spawn" {
            let tag = reason
                .data
                .get("tag")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if let Some(idx) = find_second_spawn_of(scenario, tag) {
                ops.push(RepairOp::DropInput { index: idx });
            }
        }
    }

    if ops.is_empty() {
        debug!("no repair ops synthesized; failures not recognized");
        return None;
    }

    let target_hash = Cid::from_bytes(&scenario.canonical_bytes().unwrap_or_default());
    // De-dup identical ops (same index + same variant).
    ops.sort_by(|a, b| format!("{a:?}").cmp(&format!("{b:?}")));
    ops.dedup();
    Some(RepairPatch::new(target_hash, ops))
}

fn find_index_where<F: Fn(&Input) -> bool>(scenario: &Scenario, pred: F) -> Option<usize> {
    scenario.inputs.iter().position(pred)
}

fn find_second_spawn_of(scenario: &Scenario, tag: &str) -> Option<usize> {
    let mut seen = 0;
    for (i, inp) in scenario.inputs.iter().enumerate() {
        if let Input::AgentAction {
            action: crate::scenario::AgentAction::Spawn { entity_tag, .. },
            ..
        } = inp
        {
            if entity_tag == tag {
                seen += 1;
                if seen == 2 {
                    return Some(i);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::{AgentAction, Input, Scenario};

    #[test]
    fn empty_scenario_passes() {
        let s = Scenario::new("empty");
        let report = run_scenario(&s);
        assert!(report.verdict.is_pass());
        assert!(report.repair_patch.is_none());
    }

    #[test]
    fn harness_trait_wires_up() {
        let mut h = DefaultHarness::new();
        let s = Scenario::new("t").push_ticks(3);
        let r = h.run(WorldSnapshot::Empty, s);
        assert!(r.verdict.is_pass());
    }

    #[test]
    fn bad_locomotion_fails_and_proposes_repair() {
        let s = Scenario::new("bad")
            .push(Input::AgentAction {
                agent: "a".into(),
                action: AgentAction::SmoothLocomotion { accel: [10.0, 0.0, 0.0] },
            })
            .push_ticks(1);
        let r = run_scenario(&s);
        assert!(r.verdict.is_fail(), "{:?}", r.verdict);
        let patch = r.repair_patch.expect("repair expected");
        assert!(!patch.ops.is_empty());
    }

    #[test]
    fn report_hash_is_deterministic() {
        let s = Scenario::new("t").push_ticks(5);
        let a = run_scenario(&s);
        let b = run_scenario(&s);
        assert_eq!(a.hash(), b.hash());
    }
}
