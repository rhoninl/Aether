//! Round-trip integration tests for the public harness contract.

use aether_sim_harness::{
    AgentAction, DefaultHarness, Harness, Input, NetEvent, Scenario, Verdict, WorldSnapshot,
};

#[test]
fn default_harness_runs_scenario() {
    let mut h = DefaultHarness::new();
    let s = Scenario::new("ok").push_ticks(5);
    let r = h.run(WorldSnapshot::Empty, s);
    assert_eq!(r.verdict, Verdict::Pass);
    assert!(r.repair_patch.is_none());
    assert_eq!(r.telemetry.counter("sim.ticks"), 5);
}

#[test]
fn failed_scenario_returns_repair_patch() {
    let s = Scenario::new("bad")
        .push(Input::AgentAction {
            agent: "a".into(),
            action: AgentAction::SmoothLocomotion { accel: [12.0, 0.0, 0.0] },
        })
        .push_ticks(1);
    let mut h = DefaultHarness::new();
    let r = h.run(WorldSnapshot::Empty, s);
    assert!(r.verdict.is_fail());
    assert!(r.repair_patch.is_some());
}

#[test]
fn double_spawn_is_coherence_failure() {
    let s = Scenario::new("dup")
        .push(Input::NetEvent {
            client: 1,
            event: NetEvent::ClientConnect,
        })
        .push(Input::NetEvent {
            client: 1,
            event: NetEvent::ReplicateSpawn {
                entity_tag: "tree".into(),
            },
        })
        .push(Input::NetEvent {
            client: 1,
            event: NetEvent::ReplicateSpawn {
                entity_tag: "tree".into(),
            },
        })
        .push_ticks(1);
    let r = DefaultHarness::new().run(WorldSnapshot::Empty, s);
    assert!(r.verdict.is_fail(), "{:?}", r.verdict);
}

#[test]
fn determinism_hash_matches_across_runs() {
    let s = Scenario::new("det")
        .push(Input::AgentAction {
            agent: "a".into(),
            action: AgentAction::Spawn {
                entity_tag: "x".into(),
                position: [1.0, 2.0, 3.0],
            },
        })
        .push_ticks(10);
    let a = DefaultHarness::new().run(WorldSnapshot::Empty, s.clone());
    let b = DefaultHarness::new().run(WorldSnapshot::Empty, s);
    assert_eq!(a.hash(), b.hash());
}
