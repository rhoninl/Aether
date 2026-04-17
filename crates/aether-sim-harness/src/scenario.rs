//! Scenario format: a deterministic, typed sequence of inputs.
//!
//! Scenarios serialize to YAML for human authoring, and to JSON for
//! machine pipelines. The loader accepts both; the saver writes YAML.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{HarnessError, HarnessResult};

/// Snapshot of a world at the start of a scenario.
///
/// The harness never peeks inside this payload — it's handed straight to
/// the world materializer. For the golden tests we use the `empty` form
/// and populate via inputs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorldSnapshot {
    /// An empty world.
    Empty,
    /// A world pre-populated with N transform-tagged entities.
    NEntities { count: u32 },
}

impl Default for WorldSnapshot {
    fn default() -> Self {
        Self::Empty
    }
}

/// One typed input delivered to the harness on a specific tick.
///
/// Inputs are applied in scenario order; the replay engine turns each one
/// into effects (spawn, despawn, component mutation, event) on the world.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Input {
    /// Advance the simulation by one fixed tick. No payload.
    Tick,
    /// An agent-initiated action.
    AgentAction {
        agent: String,
        action: AgentAction,
    },
    /// A simulated network event.
    NetEvent {
        client: u32,
        event: NetEvent,
    },
}

/// Agent action primitives. Rich enough to exercise the scorers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentAction {
    Spawn {
        entity_tag: String,
        position: [f32; 3],
    },
    Despawn {
        entity_tag: String,
    },
    Move {
        entity_tag: String,
        /// Linear velocity in m/s.
        velocity: [f32; 3],
    },
    /// VR head rotation — yaw/pitch/roll delta in degrees for this tick.
    RotateHead {
        yaw_deg: f32,
        pitch_deg: f32,
        roll_deg: f32,
    },
    /// VR smooth-locomotion step. `accel` is the acceleration applied to
    /// the player capsule in m/s^2.
    SmoothLocomotion {
        accel: [f32; 3],
    },
    /// VR teleport; teleports are comfort-neutral by default.
    Teleport {
        from: [f32; 3],
        to: [f32; 3],
    },
    /// Change the field of view (degrees) for this tick.
    SetFov {
        fov_deg: f32,
    },
    /// Declare the frame time in milliseconds for comfort analysis.
    FrameTimeMs {
        ms: f32,
    },
    /// Patrol behavior — move between two points over `steps` ticks.
    Patrol {
        entity_tag: String,
        from: [f32; 3],
        to: [f32; 3],
        steps: u32,
    },
}

/// Simulated network events visible in the loopback transport.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NetEvent {
    ClientConnect,
    ClientDisconnect,
    /// Replicate a spawn to a specific client (for coherence tests).
    ReplicateSpawn { entity_tag: String },
    /// Inject jitter in milliseconds for subsequent ticks.
    Jitter { ms: u32 },
    /// Entity hand-off between zones.
    ZoneHandoff {
        entity_tag: String,
        from_zone: String,
        to_zone: String,
    },
}

/// The scenario document: a typed, deterministic input program.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Scenario {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_seed")]
    pub seed: u64,
    #[serde(default = "default_hz")]
    pub tick_hz: u32,
    #[serde(default)]
    pub snapshot: WorldSnapshot,
    pub inputs: Vec<Input>,
}

fn default_seed() -> u64 {
    0xA37E_4221_C0DE_5EED
}

fn default_hz() -> u32 {
    60
}

impl Scenario {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            seed: default_seed(),
            tick_hz: default_hz(),
            snapshot: WorldSnapshot::Empty,
            inputs: Vec::new(),
        }
    }

    /// Append an input.
    pub fn push(mut self, input: Input) -> Self {
        self.inputs.push(input);
        self
    }

    /// Append N `Input::Tick` events (convenience).
    pub fn push_ticks(mut self, n: u32) -> Self {
        for _ in 0..n {
            self.inputs.push(Input::Tick);
        }
        self
    }

    /// Load a scenario from a path. Uses file extension to pick format:
    /// `.json` → JSON, anything else → YAML (`.scenario`, `.yaml`, `.yml`).
    pub fn load(path: impl AsRef<Path>) -> HarnessResult<Self> {
        let path = path.as_ref();
        let bytes = std::fs::read(path)?;
        let is_json = path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.eq_ignore_ascii_case("json"))
            .unwrap_or(false);
        if is_json {
            Ok(serde_json::from_slice(&bytes)?)
        } else {
            Ok(serde_yaml::from_slice(&bytes)?)
        }
    }

    /// Save the scenario as YAML.
    pub fn save_yaml(&self, path: impl AsRef<Path>) -> HarnessResult<()> {
        let s = serde_yaml::to_string(self)?;
        std::fs::write(path, s)?;
        Ok(())
    }

    /// Serialize to a stable canonical byte form. Used for hashing scenarios.
    pub fn canonical_bytes(&self) -> HarnessResult<Vec<u8>> {
        serde_json::to_vec(self).map_err(HarnessError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_and_roundtrip_yaml() {
        let s = Scenario::new("demo")
            .push(Input::AgentAction {
                agent: "alice".into(),
                action: AgentAction::Spawn {
                    entity_tag: "tree".into(),
                    position: [1.0, 2.0, 3.0],
                },
            })
            .push_ticks(3);

        let y = serde_yaml::to_string(&s).unwrap();
        let back: Scenario = serde_yaml::from_str(&y).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn defaults_are_stable() {
        let y = "name: demo\ninputs: []\n";
        let s: Scenario = serde_yaml::from_str(y).unwrap();
        assert_eq!(s.tick_hz, 60);
        assert_eq!(s.seed, default_seed());
        assert_eq!(s.snapshot, WorldSnapshot::Empty);
    }
}
