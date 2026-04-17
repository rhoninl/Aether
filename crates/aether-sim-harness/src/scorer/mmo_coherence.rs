//! MMO coherence scorer.
//!
//! A scenario is "coherent" when:
//!
//! * Every spawned entity is observed by every connected client within
//!   the configured latency budget (in ticks).
//! * No double spawns on the same client for the same tag.
//! * No pair of entities closer than `SPATIAL_OVERLAP_THRESHOLD` at the
//!   end of the run (proxy for "no overlap").
//!
//! Thresholds are documented constants.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::replay::SimState;

/// Maximum ticks between spawn and all clients having seen it.
pub const LATENCY_BUDGET_TICKS: u64 = 10;
/// Minimum L2 distance (meters) between any two live entities.
pub const SPATIAL_OVERLAP_THRESHOLD_M: f32 = 0.1;

pub const COHERENCE_PASS_THRESHOLD: f32 = 0.75;
pub const COHERENCE_WARN_THRESHOLD: f32 = 0.5;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoherenceReason {
    pub code: String,
    pub message: String,
    /// `"warn"` or `"fail"`.
    pub severity: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoherenceScore {
    pub overall: f32,
    pub reasons: Vec<CoherenceReason>,
}

impl CoherenceScore {
    pub fn is_pass(&self) -> bool {
        self.overall >= COHERENCE_PASS_THRESHOLD
            && !self.reasons.iter().any(|r| r.severity == "fail")
    }

    pub fn is_warn(&self) -> bool {
        !self.is_pass() && self.overall >= COHERENCE_WARN_THRESHOLD
    }
}

pub fn score(state: &SimState) -> CoherenceScore {
    let mut reasons: Vec<CoherenceReason> = Vec::new();
    let mut deductions: f32 = 0.0;

    let connected: HashSet<u32> = state.net.clients_connected.iter().copied().collect();

    // 1) Every spawn observed by every connected client within budget.
    //    We operate per-entity: the harness tracks `seen_by`; if a client
    //    never saw it, that's a coherence hit.
    if !connected.is_empty() {
        for entity in state.entities.values() {
            let seen: HashSet<u32> = entity.seen_by.iter().copied().collect();
            let missing: Vec<u32> = connected.difference(&seen).copied().collect();
            let tick_age = state.current_tick.saturating_sub(entity.spawn_tick);
            if !missing.is_empty() && tick_age > LATENCY_BUDGET_TICKS {
                deductions += 0.3;
                reasons.push(CoherenceReason {
                    code: "coherence.missing_replication".into(),
                    message: "Entity not observed by all connected clients within budget".into(),
                    severity: "fail".into(),
                    data: serde_json::json!({
                        "tag": entity.tag,
                        "missing_clients": missing,
                        "tick_age": tick_age,
                        "budget": LATENCY_BUDGET_TICKS,
                    }),
                });
            }
        }
    }

    // 2) Double-spawn: any (client, tag) seen more than once.
    for ((client, tag), count) in &state.net.spawns_seen {
        if *count > 1 {
            deductions += 0.4;
            reasons.push(CoherenceReason {
                code: "coherence.double_spawn".into(),
                message: "Same entity spawned multiple times for the same client".into(),
                severity: "fail".into(),
                data: serde_json::json!({
                    "client": client,
                    "tag": tag,
                    "count": count,
                }),
            });
        }
    }

    // 3) Spatial overlap.
    let entities: Vec<_> = state.entities.values().collect();
    for i in 0..entities.len() {
        for j in (i + 1)..entities.len() {
            let d = distance(&entities[i].position, &entities[j].position);
            if d < SPATIAL_OVERLAP_THRESHOLD_M {
                deductions += 0.1;
                reasons.push(CoherenceReason {
                    code: "coherence.spatial_overlap".into(),
                    message: "Two entities occupy overlapping space".into(),
                    severity: "warn".into(),
                    data: serde_json::json!({
                        "a": entities[i].tag,
                        "b": entities[j].tag,
                        "distance_m": d,
                        "threshold_m": SPATIAL_OVERLAP_THRESHOLD_M,
                    }),
                });
            }
        }
    }

    let overall = (1.0 - deductions).clamp(0.0, 1.0);
    CoherenceScore { overall, reasons }
}

fn distance(a: &[f32; 3], b: &[f32; 3]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::{EntityState, NetState, SimState};

    #[test]
    fn empty_state_is_pass() {
        let s = score(&SimState::default());
        assert!(s.is_pass());
    }

    #[test]
    fn all_clients_see_spawn_is_pass() {
        let mut s = SimState::default();
        s.current_tick = 5;
        s.net = NetState {
            clients_connected: vec![1, 2],
            jitter_ms: 0,
            spawns_seen: Default::default(),
        };
        s.entities.insert(
            "tree".into(),
            EntityState {
                tag: "tree".into(),
                position: [0.0; 3],
                velocity: [0.0; 3],
                spawn_tick: 0,
                zone: "z".into(),
                seen_by: vec![1, 2],
            },
        );
        let score = score(&s);
        assert!(score.is_pass(), "score: {:?}", score);
    }

    #[test]
    fn missing_replication_fails_when_past_budget() {
        let mut s = SimState::default();
        s.current_tick = 50;
        s.net.clients_connected = vec![1, 2];
        s.entities.insert(
            "tree".into(),
            EntityState {
                tag: "tree".into(),
                position: [0.0; 3],
                velocity: [0.0; 3],
                spawn_tick: 0,
                zone: "z".into(),
                seen_by: vec![1],
            },
        );
        let sc = score(&s);
        assert!(!sc.is_pass());
        assert!(sc
            .reasons
            .iter()
            .any(|r| r.code == "coherence.missing_replication"));
    }

    #[test]
    fn double_spawn_fails() {
        let mut s = SimState::default();
        s.net.spawns_seen.insert((1, "tree".into()), 2);
        let sc = score(&s);
        assert!(!sc.is_pass());
    }

    #[test]
    fn spatial_overlap_warns() {
        let mut s = SimState::default();
        s.entities.insert(
            "a".into(),
            EntityState {
                tag: "a".into(),
                position: [0.0, 0.0, 0.0],
                velocity: [0.0; 3],
                spawn_tick: 0,
                zone: "z".into(),
                seen_by: vec![],
            },
        );
        s.entities.insert(
            "b".into(),
            EntityState {
                tag: "b".into(),
                position: [0.01, 0.0, 0.0],
                velocity: [0.0; 3],
                spawn_tick: 0,
                zone: "z".into(),
                seen_by: vec![],
            },
        );
        let sc = score(&s);
        assert!(sc
            .reasons
            .iter()
            .any(|r| r.code == "coherence.spatial_overlap"));
    }
}
