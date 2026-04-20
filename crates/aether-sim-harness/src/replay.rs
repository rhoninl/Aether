//! Deterministic replay engine.
//!
//! Drives a scenario input-by-input against a fresh [`aether_ecs::World`]
//! at a fixed tick rate. A seeded [`ChaCha8Rng`] is threaded through the
//! simulation; the engine is pure-CPU and targets 10x–100x wall-clock
//! throughput on a 1000-tick scenario.
//!
//! The engine is intentionally bookkeeping-only: it records observable
//! state transitions into [`SimState`] and emits telemetry events.
//! Scorers then read that state to produce VR/MMO scores.

use std::collections::BTreeMap;
use std::time::Instant;

use aether_ecs::World;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use crate::scenario::{AgentAction, Input, NetEvent, Scenario, WorldSnapshot};
use crate::telemetry::{Event, Telemetry};

/// Per-entity live state tracked by the replay engine.
#[derive(Debug, Clone)]
pub struct EntityState {
    pub tag: String,
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub spawn_tick: u64,
    pub zone: String,
    /// Client IDs that have seen this entity.
    pub seen_by: Vec<u32>,
}

/// Per-tick VR state sample (used by the comfort scorer).
#[derive(Debug, Clone)]
pub struct VrSample {
    pub tick: u64,
    pub angular_velocity_deg_s: [f32; 3],
    pub fov_deg: f32,
    pub locomotion_accel_m_s2: [f32; 3],
    pub frame_time_ms: f32,
}

/// Networking-level state sample (used by the coherence scorer).
#[derive(Debug, Clone, Default)]
pub struct NetState {
    pub clients_connected: Vec<u32>,
    pub jitter_ms: u32,
    /// Per-client spawn-observation count, for double-spawn detection.
    pub spawns_seen: BTreeMap<(u32, String), u32>,
}

/// The aggregate state the engine builds up over a run; scorers read it.
#[derive(Debug, Clone, Default)]
pub struct SimState {
    pub current_tick: u64,
    pub entities: BTreeMap<String, EntityState>,
    pub despawned: Vec<String>,
    pub vr_samples: Vec<VrSample>,
    /// Current FOV; defaults to 90.
    pub fov_deg: f32,
    /// Current frame time; defaults to 11ms (~90Hz).
    pub frame_time_ms: f32,
    pub net: NetState,
}

impl SimState {
    fn new() -> Self {
        Self {
            current_tick: 0,
            entities: BTreeMap::new(),
            despawned: Vec::new(),
            vr_samples: Vec::new(),
            fov_deg: 90.0,
            frame_time_ms: 11.0,
            net: NetState::default(),
        }
    }
}

/// Output of one full scenario replay.
pub struct ReplayOutput {
    pub state: SimState,
    pub telemetry: Telemetry,
    pub wall_clock_ns: u128,
    pub sim_duration_ns: u128,
}

/// The deterministic replay engine.
///
/// `seed` is threaded into a [`ChaCha8Rng`]. `rng` is deliberately
/// exposed to callers (e.g. scorers) so they can produce deterministic
/// stochastic decisions if ever needed.
pub struct Replay {
    pub world: World,
    pub state: SimState,
    pub telemetry: Telemetry,
    pub rng: ChaCha8Rng,
    tick_hz: u32,
    tick_interval_ns: u128,
    /// Per-tick angular delta accumulated from `RotateHead` actions.
    pending_rotation: [f32; 3],
    /// Per-tick locomotion accel.
    pending_locomotion: [f32; 3],
}

impl Replay {
    pub fn new(scenario: &Scenario) -> Self {
        let mut state = SimState::new();
        apply_snapshot(&mut state, &scenario.snapshot);
        let tick_interval_ns = if scenario.tick_hz == 0 {
            0
        } else {
            1_000_000_000u128 / scenario.tick_hz as u128
        };
        Self {
            world: World::new(),
            state,
            telemetry: Telemetry::new(),
            rng: ChaCha8Rng::seed_from_u64(scenario.seed),
            tick_hz: scenario.tick_hz,
            tick_interval_ns,
            pending_rotation: [0.0; 3],
            pending_locomotion: [0.0; 3],
        }
    }

    pub fn tick_hz(&self) -> u32 {
        self.tick_hz
    }

    /// Run every input in `scenario` once, in order. Returns the output.
    pub fn run(mut self, scenario: &Scenario) -> ReplayOutput {
        let wall_start = Instant::now();

        self.telemetry.emit(Event::new(
            0,
            "sim.begin",
            serde_json::json!({"scenario": scenario.name, "inputs": scenario.inputs.len()}),
        ));

        for input in &scenario.inputs {
            self.apply_input(input);
        }

        self.telemetry.emit(Event::new(
            self.state.current_tick,
            "sim.end",
            serde_json::json!({"ticks": self.state.current_tick}),
        ));

        let wall_clock_ns = wall_start.elapsed().as_nanos();
        let sim_duration_ns = self.tick_interval_ns * self.state.current_tick as u128;

        self.telemetry.incr("sim.ticks", self.state.current_tick);
        // Intentionally do NOT fold wall-clock into `telemetry.timings` — wall
        // time varies across runs and would poison deterministic report
        // hashing. `wall_clock_ns` is returned out-of-band on `ReplayOutput`
        // for callers that want it.

        ReplayOutput {
            state: self.state,
            telemetry: self.telemetry,
            wall_clock_ns,
            sim_duration_ns,
        }
    }

    fn apply_input(&mut self, input: &Input) {
        match input {
            Input::Tick => self.finalize_tick(),
            Input::AgentAction { agent, action } => {
                self.apply_agent_action(agent, action);
            }
            Input::NetEvent { client, event } => {
                self.apply_net_event(*client, event);
            }
        }
    }

    fn apply_agent_action(&mut self, agent: &str, action: &AgentAction) {
        let tick = self.state.current_tick;
        match action {
            AgentAction::Spawn {
                entity_tag,
                position,
            } => {
                if self.state.entities.contains_key(entity_tag) {
                    self.telemetry.emit(Event::new(
                        tick,
                        "mmo.coherence.double_spawn",
                        serde_json::json!({"tag": entity_tag, "agent": agent}),
                    ));
                    self.telemetry.incr("mmo.double_spawn", 1);
                } else {
                    self.state.entities.insert(
                        entity_tag.clone(),
                        EntityState {
                            tag: entity_tag.clone(),
                            position: *position,
                            velocity: [0.0; 3],
                            spawn_tick: tick,
                            zone: "default".to_string(),
                            seen_by: Vec::new(),
                        },
                    );
                    let _ = self.world.spawn_empty();
                    self.telemetry.emit(Event::new(
                        tick,
                        "sim.spawn",
                        serde_json::json!({"tag": entity_tag, "pos": position}),
                    ));
                    self.telemetry.incr("sim.spawn", 1);
                }
            }
            AgentAction::Despawn { entity_tag } => {
                if self.state.entities.remove(entity_tag).is_some() {
                    self.state.despawned.push(entity_tag.clone());
                    self.telemetry.emit(Event::new(
                        tick,
                        "sim.despawn",
                        serde_json::json!({"tag": entity_tag}),
                    ));
                    self.telemetry.incr("sim.despawn", 1);
                }
            }
            AgentAction::Move {
                entity_tag,
                velocity,
            } => {
                if let Some(e) = self.state.entities.get_mut(entity_tag) {
                    e.velocity = *velocity;
                }
            }
            AgentAction::RotateHead {
                yaw_deg,
                pitch_deg,
                roll_deg,
            } => {
                self.pending_rotation[0] += yaw_deg;
                self.pending_rotation[1] += pitch_deg;
                self.pending_rotation[2] += roll_deg;
            }
            AgentAction::SmoothLocomotion { accel } => {
                self.pending_locomotion[0] += accel[0];
                self.pending_locomotion[1] += accel[1];
                self.pending_locomotion[2] += accel[2];
            }
            AgentAction::Teleport { to, .. } => {
                self.telemetry.emit(Event::new(
                    tick,
                    "vr.teleport",
                    serde_json::json!({"to": to}),
                ));
                self.telemetry.incr("vr.teleport", 1);
            }
            AgentAction::SetFov { fov_deg } => {
                self.state.fov_deg = *fov_deg;
            }
            AgentAction::FrameTimeMs { ms } => {
                self.state.frame_time_ms = *ms;
            }
            AgentAction::Patrol {
                entity_tag,
                from,
                to,
                steps,
            } => {
                // Deterministic patrol: queue up steps worth of moves over subsequent ticks.
                // We model this as a single registered intent — the engine won't advance here.
                if let Some(e) = self.state.entities.get_mut(entity_tag) {
                    let n = (*steps).max(1) as f32;
                    e.velocity = [
                        (to[0] - from[0]) / n,
                        (to[1] - from[1]) / n,
                        (to[2] - from[2]) / n,
                    ];
                    self.telemetry.emit(Event::new(
                        tick,
                        "sim.patrol.begin",
                        serde_json::json!({
                            "tag": entity_tag,
                            "from": from,
                            "to": to,
                            "steps": steps
                        }),
                    ));
                }
            }
        }
    }

    fn apply_net_event(&mut self, client: u32, event: &NetEvent) {
        let tick = self.state.current_tick;
        match event {
            NetEvent::ClientConnect => {
                if !self.state.net.clients_connected.contains(&client) {
                    self.state.net.clients_connected.push(client);
                    self.telemetry.emit(Event::new(
                        tick,
                        "net.connect",
                        serde_json::json!({"client": client}),
                    ));
                }
            }
            NetEvent::ClientDisconnect => {
                self.state.net.clients_connected.retain(|c| *c != client);
                self.telemetry.emit(Event::new(
                    tick,
                    "net.disconnect",
                    serde_json::json!({"client": client}),
                ));
            }
            NetEvent::ReplicateSpawn { entity_tag } => {
                let key = (client, entity_tag.clone());
                let count = self.state.net.spawns_seen.entry(key).or_insert(0);
                *count += 1;
                if *count > 1 {
                    self.telemetry.emit(Event::new(
                        tick,
                        "mmo.coherence.double_spawn",
                        serde_json::json!({
                            "client": client, "tag": entity_tag, "count": *count
                        }),
                    ));
                    self.telemetry.incr("mmo.double_spawn", 1);
                }
                if let Some(e) = self.state.entities.get_mut(entity_tag) {
                    if !e.seen_by.contains(&client) {
                        e.seen_by.push(client);
                    }
                }
            }
            NetEvent::Jitter { ms } => {
                self.state.net.jitter_ms = *ms;
                self.telemetry.emit(Event::new(
                    tick,
                    "net.jitter",
                    serde_json::json!({"ms": ms}),
                ));
            }
            NetEvent::ZoneHandoff {
                entity_tag,
                from_zone,
                to_zone,
            } => {
                if let Some(e) = self.state.entities.get_mut(entity_tag) {
                    e.zone = to_zone.clone();
                    self.telemetry.emit(Event::new(
                        tick,
                        "mmo.handoff",
                        serde_json::json!({
                            "tag": entity_tag, "from": from_zone, "to": to_zone
                        }),
                    ));
                    self.telemetry.incr("mmo.handoff", 1);
                }
            }
        }
    }

    fn finalize_tick(&mut self) {
        // Integrate velocities.
        let dt = if self.tick_hz == 0 {
            0.0
        } else {
            1.0 / self.tick_hz as f32
        };
        for e in self.state.entities.values_mut() {
            e.position[0] += e.velocity[0] * dt;
            e.position[1] += e.velocity[1] * dt;
            e.position[2] += e.velocity[2] * dt;
        }

        // Capture VR sample for this tick.
        let angular_velocity_deg_s = [
            self.pending_rotation[0] / dt.max(1e-6),
            self.pending_rotation[1] / dt.max(1e-6),
            self.pending_rotation[2] / dt.max(1e-6),
        ];
        self.state.vr_samples.push(VrSample {
            tick: self.state.current_tick,
            angular_velocity_deg_s,
            fov_deg: self.state.fov_deg,
            locomotion_accel_m_s2: self.pending_locomotion,
            frame_time_ms: self.state.frame_time_ms,
        });
        self.pending_rotation = [0.0; 3];
        self.pending_locomotion = [0.0; 3];

        self.telemetry.emit(Event::new(
            self.state.current_tick,
            "sim.tick",
            serde_json::json!({"tick": self.state.current_tick}),
        ));
        self.state.current_tick += 1;
    }
}

fn apply_snapshot(state: &mut SimState, snapshot: &WorldSnapshot) {
    match snapshot {
        WorldSnapshot::Empty => {}
        WorldSnapshot::NEntities { count } => {
            for i in 0..*count {
                let tag = format!("preset_{i}");
                state.entities.insert(
                    tag.clone(),
                    EntityState {
                        tag,
                        position: [i as f32, 0.0, 0.0],
                        velocity: [0.0; 3],
                        spawn_tick: 0,
                        zone: "default".to_string(),
                        seen_by: Vec::new(),
                    },
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::{AgentAction, Input, Scenario};

    #[test]
    fn empty_scenario_records_zero_ticks() {
        let s = Scenario::new("t");
        let out = Replay::new(&s).run(&s);
        assert_eq!(out.state.current_tick, 0);
    }

    #[test]
    fn ticks_advance_state() {
        let s = Scenario::new("t").push_ticks(5);
        let out = Replay::new(&s).run(&s);
        assert_eq!(out.state.current_tick, 5);
        // One sim.tick event per tick, plus sim.begin + sim.end.
        let tick_events = out
            .telemetry
            .events_with_kind("sim.tick")
            .count();
        assert_eq!(tick_events, 5);
    }

    #[test]
    fn spawn_and_despawn_cycles_cleanly() {
        let s = Scenario::new("t")
            .push(Input::AgentAction {
                agent: "a".into(),
                action: AgentAction::Spawn {
                    entity_tag: "goblin".into(),
                    position: [0.0; 3],
                },
            })
            .push_ticks(1)
            .push(Input::AgentAction {
                agent: "a".into(),
                action: AgentAction::Despawn {
                    entity_tag: "goblin".into(),
                },
            })
            .push_ticks(1);
        let out = Replay::new(&s).run(&s);
        assert_eq!(out.state.entities.len(), 0);
        assert_eq!(out.state.despawned, vec!["goblin".to_string()]);
    }

    #[test]
    fn double_spawn_records_coherence_event() {
        let s = Scenario::new("t")
            .push(Input::AgentAction {
                agent: "a".into(),
                action: AgentAction::Spawn {
                    entity_tag: "tree".into(),
                    position: [0.0; 3],
                },
            })
            .push(Input::AgentAction {
                agent: "a".into(),
                action: AgentAction::Spawn {
                    entity_tag: "tree".into(),
                    position: [0.0; 3],
                },
            });
        let out = Replay::new(&s).run(&s);
        assert_eq!(out.telemetry.counter("mmo.double_spawn"), 1);
    }

    #[test]
    fn determinism_same_seed_same_state() {
        let s = Scenario::new("t")
            .push(Input::AgentAction {
                agent: "a".into(),
                action: AgentAction::Spawn {
                    entity_tag: "x".into(),
                    position: [1.0, 2.0, 3.0],
                },
            })
            .push_ticks(10);
        let a = Replay::new(&s).run(&s);
        let b = Replay::new(&s).run(&s);
        assert_eq!(
            serde_json::to_string(&a.telemetry).unwrap(),
            serde_json::to_string(&b.telemetry).unwrap()
        );
    }
}
