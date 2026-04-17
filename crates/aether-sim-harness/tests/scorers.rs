//! Scorer integration tests against end-to-end scenarios.

use aether_sim_harness::{
    run_scenario, AgentAction, Input, NetEvent, Scenario,
};

#[test]
fn smooth_vr_session_passes_comfort() {
    let s = Scenario::new("vr.smooth")
        .push(Input::AgentAction {
            agent: "user".into(),
            action: AgentAction::FrameTimeMs { ms: 11.0 },
        })
        .push(Input::AgentAction {
            agent: "user".into(),
            action: AgentAction::RotateHead {
                yaw_deg: 0.5,
                pitch_deg: 0.0,
                roll_deg: 0.0,
            },
        })
        .push_ticks(60);
    let r = run_scenario(&s);
    assert!(r.comfort.is_pass(), "{:?}", r.comfort);
}

#[test]
fn violent_rotation_triggers_comfort_fail() {
    let s = Scenario::new("vr.bad")
        .push(Input::AgentAction {
            agent: "user".into(),
            action: AgentAction::RotateHead {
                yaw_deg: 30.0,
                pitch_deg: 0.0,
                roll_deg: 0.0,
            },
        })
        .push_ticks(1);
    let r = run_scenario(&s);
    assert!(!r.comfort.is_pass());
}

#[test]
fn two_clients_see_spawn_coherent() {
    let s = Scenario::new("mmo.ok")
        .push(Input::NetEvent {
            client: 1,
            event: NetEvent::ClientConnect,
        })
        .push(Input::NetEvent {
            client: 2,
            event: NetEvent::ClientConnect,
        })
        .push(Input::AgentAction {
            agent: "server".into(),
            action: AgentAction::Spawn {
                entity_tag: "chest".into(),
                position: [0.0; 3],
            },
        })
        .push(Input::NetEvent {
            client: 1,
            event: NetEvent::ReplicateSpawn {
                entity_tag: "chest".into(),
            },
        })
        .push(Input::NetEvent {
            client: 2,
            event: NetEvent::ReplicateSpawn {
                entity_tag: "chest".into(),
            },
        })
        .push_ticks(3);
    let r = run_scenario(&s);
    assert!(r.coherence.is_pass(), "{:?}", r.coherence);
}
