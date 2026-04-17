//! Local stubs for the five dependency units (U03, U05, U07, U08, U09).
//!
//! These types mirror — in name, shape, and return values — the public APIs we
//! expect the real crates to expose. They deliberately return canned success
//! values so the thin-slice demo can exercise the full end-to-end flow today,
//! while the real crates are still being built out in parallel worktrees.
//!
//! When all five sibling units land on `main`, a follow-up change will:
//!   1. Flip `default = ["stubs"]` to `default = ["real"]` in `Cargo.toml`.
//!   2. Uncomment the workspace-path dependencies at the bottom of `Cargo.toml`.
//!   3. Delete this file.
//!   4. Replace `use crate::stubs::*;` in `main.rs` with imports from the real
//!      crates (`aether_schemas`, `aether_sim_harness`, `aether_behavior_dsl`,
//!      `aether_agent_cp`, `aether_world_vcs`).
//!
//! The API shapes here were cross-checked against the sister worktrees that
//! were visible at authoring time (see `docs/design/agent-native-demo.md` for
//! the exact provenance).

use std::path::Path;

use sha2::{Digest, Sha256};

/// Hex-encoded SHA-256 digest, used as a content identifier in every stub
/// surface. The real `aether-schemas::Cid` is richer (carries schema version);
/// the demo only ever needs the string form.
pub fn cid_of(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("cid:v1:{}", hex::encode(h.finalize()))
}

// ---------------------------------------------------------------------------
// U03 — aether-schemas (mirror): WorldManifest + Cid + ContentAddress.
// ---------------------------------------------------------------------------

pub mod schemas {
    use super::cid_of;
    use serde::{Deserialize, Serialize};

    /// Minimal mirror of `aether_schemas::WorldManifest`.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct WorldManifest {
        pub schema_version: u32,
        pub name: String,
        #[serde(default)]
        pub spawn: Option<SpawnPoint>,
        #[serde(default)]
        pub chunks: Vec<ChunkRef>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct SpawnPoint {
        pub position: [f32; 3],
        #[serde(default)]
        pub yaw_deg: f32,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ChunkRef {
        pub coord: [i32; 3],
        pub kind: String,
    }

    impl WorldManifest {
        /// Canonical-ish encoding: sorted JSON. Enough for a deterministic CID
        /// in the stub world.
        pub fn canonical_bytes(&self) -> Vec<u8> {
            let value = serde_json::to_value(self).expect("serialize");
            let sorted = super::sort_json(value);
            serde_json::to_vec(&sorted).expect("serialize sorted")
        }

        pub fn cid(&self) -> String {
            cid_of(&self.canonical_bytes())
        }
    }
}

// ---------------------------------------------------------------------------
// U07 — aether-behavior-dsl (mirror): parse + compile to WASM.
// ---------------------------------------------------------------------------

pub mod behavior_dsl {
    use super::cid_of;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct CompiledScript {
        pub script_cid: String,
        /// In the real crate this is a real WASM blob produced by the 5-verb
        /// compiler. Here we use a canned placeholder that still has the
        /// WebAssembly magic prefix so consumers can sanity-check it.
        pub wasm: Vec<u8>,
        pub verb_count: usize,
    }

    /// Mirror of `aether_behavior_dsl::parse_and_compile(source) -> CompiledScript`.
    pub fn parse_and_compile(source: &str) -> Result<CompiledScript, String> {
        if source.trim().is_empty() {
            return Err("empty behavior source".to_string());
        }
        // Count "verbs" by scanning for known 5-verb keywords; the real parser
        // produces an AST first.
        let verbs = ["move", "wait", "sense", "branch", "signal"];
        let verb_count = verbs
            .iter()
            .map(|v| source.matches(v).count())
            .sum::<usize>()
            .max(1);

        // Canned wasm: the 4-byte magic + 4-byte version + a payload derived
        // from the source so the CID is deterministic per source.
        let mut wasm = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
        wasm.extend_from_slice(source.as_bytes());
        let script_cid = cid_of(&wasm);
        Ok(CompiledScript {
            script_cid,
            wasm,
            verb_count,
        })
    }
}

// ---------------------------------------------------------------------------
// U05 — aether-sim-harness (mirror): Harness::run, SimReport, Verdict.
// ---------------------------------------------------------------------------

pub mod sim_harness {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(tag = "verdict", rename_all = "snake_case")]
    pub enum Verdict {
        Pass,
        PassWithWarnings { warnings: Vec<String> },
        Fail { reasons: Vec<String> },
    }

    impl Verdict {
        pub fn is_pass(&self) -> bool {
            matches!(self, Verdict::Pass | Verdict::PassWithWarnings { .. })
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct SimReport {
        pub ticks_run: u32,
        pub wall_ms: u64,
        pub verdict: Verdict,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Scenario {
        pub name: String,
        pub ticks: u32,
    }

    /// Parse a minimal YAML scenario. The real `aether-sim-harness` has a
    /// richer grammar (inputs, assertions); the stub only needs name + ticks.
    pub fn parse_scenario_yaml(yaml: &str) -> Result<Scenario, String> {
        serde_yaml::from_str(yaml).map_err(|e| e.to_string())
    }

    /// Mirror of `aether_sim_harness::Harness::run(scenario, world_cid) -> SimReport`.
    pub fn run(scenario: &Scenario) -> SimReport {
        // In the real harness this steps the ghost-world ECS for `ticks` frames
        // and scores the mutations. The stub returns a pass immediately so the
        // e2e run is fast.
        SimReport {
            ticks_run: scenario.ticks,
            wall_ms: 1,
            verdict: Verdict::Pass,
        }
    }
}

// ---------------------------------------------------------------------------
// U08 — aether-agent-cp + services/agent-cp (mirror): MCP tool surface.
// ---------------------------------------------------------------------------

pub mod agent_cp {
    use super::behavior_dsl::CompiledScript;
    use super::schemas::WorldManifest;
    use super::sim_harness::{Scenario, SimReport};
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    /// Stub MCP client. The real crate speaks JSON-RPC over stdio; this one
    /// simply holds in-memory state so each "call" looks like an MCP round
    /// trip.
    pub struct AgentCpClient {
        _transport: Transport,
        pub last_world_cid: Option<String>,
        pub entities: HashMap<String, Entity>,
        pub scripts: HashMap<String, CompiledScript>,
    }

    #[derive(Debug, Clone)]
    pub enum Transport {
        Stdio,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Entity {
        pub entity_id: String,
        pub position: [f32; 3],
        pub kind: String,
        #[serde(default)]
        pub script_cid: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct WorldCreated {
        pub world_cid: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct SpawnResult {
        pub entity_ids: Vec<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct DeployResult {
        pub entity_id: String,
        pub script_cid: String,
    }

    #[derive(Debug, Clone, thiserror::Error)]
    pub enum McpError {
        #[error("no world created")]
        NoWorld,
        #[error("unknown entity: {0}")]
        UnknownEntity(String),
        #[error("unknown script: {0}")]
        UnknownScript(String),
    }

    impl AgentCpClient {
        pub fn stdio() -> Self {
            Self {
                _transport: Transport::Stdio,
                last_world_cid: None,
                entities: HashMap::new(),
                scripts: HashMap::new(),
            }
        }

        /// MCP tool: `world.create`. Input: `WorldManifest`. Output: new
        /// `world_cid`.
        pub fn world_create(&mut self, manifest: &WorldManifest) -> Result<WorldCreated, McpError> {
            let cid = manifest.cid();
            self.last_world_cid = Some(cid.clone());
            Ok(WorldCreated { world_cid: cid })
        }

        /// MCP tool: `entity.spawn` (batch). Input: a list of (kind, position)
        /// tuples. Output: the assigned entity IDs.
        pub fn entity_spawn(
            &mut self,
            batch: &[(String, [f32; 3])],
        ) -> Result<SpawnResult, McpError> {
            if self.last_world_cid.is_none() {
                return Err(McpError::NoWorld);
            }
            let mut ids = Vec::with_capacity(batch.len());
            for (i, (kind, pos)) in batch.iter().enumerate() {
                let id = format!("entity:{:08x}", (self.entities.len() + i) as u32);
                self.entities.insert(
                    id.clone(),
                    Entity {
                        entity_id: id.clone(),
                        position: *pos,
                        kind: kind.clone(),
                        script_cid: None,
                    },
                );
                ids.push(id);
            }
            Ok(SpawnResult { entity_ids: ids })
        }

        /// MCP tool: `script.compile`. Delegates to the DSL crate and records
        /// the compiled WASM for later deploy.
        pub fn script_compile(&mut self, source: &str) -> Result<CompiledScript, McpError> {
            let compiled =
                super::behavior_dsl::parse_and_compile(source).map_err(McpError::UnknownScript)?;
            self.scripts
                .insert(compiled.script_cid.clone(), compiled.clone());
            Ok(compiled)
        }

        /// MCP tool: `script.deploy`. Attach a previously-compiled script to
        /// an entity.
        pub fn script_deploy(
            &mut self,
            entity_id: &str,
            script_cid: &str,
        ) -> Result<DeployResult, McpError> {
            if !self.scripts.contains_key(script_cid) {
                return Err(McpError::UnknownScript(script_cid.to_string()));
            }
            let entity = self
                .entities
                .get_mut(entity_id)
                .ok_or_else(|| McpError::UnknownEntity(entity_id.to_string()))?;
            entity.script_cid = Some(script_cid.to_string());
            Ok(DeployResult {
                entity_id: entity_id.to_string(),
                script_cid: script_cid.to_string(),
            })
        }

        /// MCP tool: `sim.run`. Dispatches to the harness.
        pub fn sim_run(&self, scenario: &Scenario) -> Result<SimReport, McpError> {
            if self.last_world_cid.is_none() {
                return Err(McpError::NoWorld);
            }
            Ok(super::sim_harness::run(scenario))
        }
    }
}

// ---------------------------------------------------------------------------
// U09 — aether-world-vcs (mirror): Diff, Branch, merge, sign.
// ---------------------------------------------------------------------------

pub mod world_vcs {
    use super::cid_of;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct DiffSpec {
        pub base_world_cid: String,
        pub head_world_cid: String,
        pub summary: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct SignedDiff {
        pub diff: DiffSpec,
        pub signer: String,
        pub signature_hex: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MergeReceipt {
        pub merge_cid: String,
        pub branch: String,
    }

    /// Mirror of `aether_world_vcs::sign_diff(diff, keypair) -> SignedDiff`.
    pub fn sign_diff(diff: DiffSpec, signer: &str) -> SignedDiff {
        // In the real crate this is an ed25519 signature over canonical bytes.
        // The stub uses a deterministic sha256 so the test can assert a stable
        // value without needing key material.
        let payload = serde_json::to_vec(&diff).expect("serialize diff");
        let sig = cid_of(&payload);
        SignedDiff {
            diff,
            signer: signer.to_string(),
            signature_hex: sig,
        }
    }

    /// Mirror of `aether_world_vcs::merge(signed, branch) -> MergeReceipt`.
    pub fn merge(signed: &SignedDiff, branch: &str) -> MergeReceipt {
        let payload = format!(
            "{}::{}::{}",
            branch, signed.diff.head_world_cid, signed.signature_hex
        );
        MergeReceipt {
            merge_cid: cid_of(payload.as_bytes()),
            branch: branch.to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Shared helpers.
// ---------------------------------------------------------------------------

fn sort_json(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut entries: Vec<(String, serde_json::Value)> = map.into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            let mut out = serde_json::Map::with_capacity(entries.len());
            for (k, v) in entries {
                out.insert(k, sort_json(v));
            }
            serde_json::Value::Object(out)
        }
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.into_iter().map(sort_json).collect())
        }
        other => other,
    }
}

/// Parse a YAML world manifest from disk. Kept here so `main.rs` stays clean.
pub fn load_world_manifest(path: &Path) -> Result<schemas::WorldManifest, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_yaml::from_str(&text).map_err(|e| e.to_string())
}

/// Parse a YAML scenario from disk.
pub fn load_scenario(path: &Path) -> Result<sim_harness::Scenario, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    sim_harness::parse_scenario_yaml(&text)
}

/// Read a DSL source from disk.
pub fn load_behavior_source(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| e.to_string())
}

// Re-exports so `main.rs` / tests can do `use crate::stubs::*`.
pub use agent_cp::{AgentCpClient, DeployResult, SpawnResult, WorldCreated};
pub use behavior_dsl::CompiledScript;
pub use schemas::WorldManifest;
pub use sim_harness::{Scenario, SimReport, Verdict};
pub use world_vcs::{DiffSpec, MergeReceipt, SignedDiff};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cid_of_is_deterministic() {
        assert_eq!(cid_of(b"hello"), cid_of(b"hello"));
        assert_ne!(cid_of(b"hello"), cid_of(b"world"));
    }

    #[test]
    fn world_cid_stable_across_calls() {
        let m = WorldManifest {
            schema_version: 1,
            name: "x".into(),
            spawn: None,
            chunks: vec![],
        };
        assert_eq!(m.cid(), m.cid());
    }

    #[test]
    fn client_rejects_spawn_before_world_create() {
        let mut c = AgentCpClient::stdio();
        let err = c.entity_spawn(&[("cube".into(), [0.0, 1.0, 0.0])]);
        assert!(err.is_err());
    }

    #[test]
    fn happy_path_roundtrip() {
        let mut c = AgentCpClient::stdio();
        let m = WorldManifest {
            schema_version: 1,
            name: "hello".into(),
            spawn: None,
            chunks: vec![],
        };
        let w = c.world_create(&m).unwrap();
        let s = c
            .entity_spawn(&[("cube".into(), [0.0, 1.0, 0.0])])
            .unwrap();
        let script = c.script_compile("move; wait; sense;").unwrap();
        c.script_deploy(&s.entity_ids[0], &script.script_cid)
            .unwrap();
        let report = c.sim_run(&Scenario {
            name: "t".into(),
            ticks: 10,
        }).unwrap();
        assert!(report.verdict.is_pass());
        assert!(!w.world_cid.is_empty());
    }
}
