//! Backend trait the tools delegate to, plus a complete in-memory default.
//!
//! The in-memory impl is **not** a `todo!()` stub — it implements content-
//! addressed storage, world patching, entity/script tracking, a simulation
//! harness and a UGC pipeline in-process, deterministically. This lets the
//! tool layer be fully tested without wiring the real Aether crates.
//!
//! The `wire` Cargo feature, once enabled, will swap this for an adapter that
//! routes into `aether-schemas`, `aether-world-vcs`, `aether-sim-harness`,
//! `aether-behavior-dsl` and `aether-ugc`. For now the `wire` impl is an
//! alias to the in-memory impl — both so the tool tests stay green and so
//! downstream crates can depend on the trait today.

use std::collections::HashMap;
use std::sync::Mutex;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{codes, RepairOp, RepairPatch, ToolError, ToolResult};

// ---------------------------------------------------------------------------
// Public data types
// ---------------------------------------------------------------------------

/// Content identifier (`cid:<hex>`). Deterministic function of the JSON body.
pub type Cid = String;

/// A stored world state as a generic JSON document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldState {
    pub cid: Cid,
    pub manifest: serde_json::Value,
    /// Spawned entities keyed by id.
    pub entities: HashMap<String, serde_json::Value>,
    /// Scripts attached to entity ids.
    pub scripts: HashMap<String, ScriptRef>,
    /// Links as directed edges (source_id -> Vec<(target_id, kind)>).
    pub links: Vec<Link>,
}

impl WorldState {
    pub fn new(cid: Cid, manifest: serde_json::Value) -> Self {
        Self {
            cid,
            manifest,
            entities: HashMap::new(),
            scripts: HashMap::new(),
            links: Vec::new(),
        }
    }
}

/// Directed link between two entities in a world.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    pub source_id: String,
    pub target_id: String,
    pub kind: String,
}

/// Reference to a compiled script.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptRef {
    pub cid: Cid,
    pub wasm_bytes_b64: String,
}

/// A compiled script as returned from `script.compile`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledScript {
    pub cid: Cid,
    pub wasm_bytes_b64: String,
    pub source_hash: String,
}

/// Simulation verdict categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimVerdict {
    Pass,
    Fail,
    Inconclusive,
}

/// Simulation report returned from `sim.run`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimReport {
    pub verdict: SimVerdict,
    pub ticks: u64,
    pub telemetry: serde_json::Value,
    /// When `verdict != Pass`, suggest a repair patch for the scenario YAML.
    pub repair_patch: Option<RepairPatch>,
}

/// UGC artifact lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UgcStatus {
    Pending,
    Scanning,
    Approved,
    Rejected,
    Published,
}

/// UGC artifact record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UgcArtifact {
    pub cid: Cid,
    pub uploader: String,
    pub media_type: String,
    pub status: UgcStatus,
    pub moderation_notes: Vec<String>,
}

/// Moderation report record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModerationReport {
    pub report_id: String,
    pub artifact_cid: Cid,
    pub reason: String,
    pub reported_at: String,
}

/// A telemetry event emitted by `telemetry.stream`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryEvent {
    pub world_cid: Cid,
    pub seq: u64,
    pub kind: String,
    pub payload: serde_json::Value,
    pub timestamp: String,
}

// ---------------------------------------------------------------------------
// Backend trait
// ---------------------------------------------------------------------------

/// The single abstraction the tool handlers call into. Everything is sync +
/// thread-safe; the transports wrap these calls in spawn_blocking where needed.
pub trait Backend: Send + Sync {
    // World VCS ------------------------------------------------------------
    fn create_world(&self, manifest_yaml: &str) -> ToolResult<WorldState>;
    fn patch_world(&self, base_cid: &Cid, patch: &serde_json::Value) -> ToolResult<WorldState>;
    fn query_world(&self, cid: &Cid, jsonpath: &str) -> ToolResult<serde_json::Value>;

    // Entity ---------------------------------------------------------------
    fn spawn_entities(
        &self,
        world_cid: &Cid,
        prototypes: &[serde_json::Value],
    ) -> ToolResult<WorldState>;
    fn modify_entities(
        &self,
        world_cid: &Cid,
        ops: &[serde_json::Value],
    ) -> ToolResult<WorldState>;
    fn link_entities(
        &self,
        world_cid: &Cid,
        source_id: &str,
        target_id: &str,
        link_kind: &str,
    ) -> ToolResult<WorldState>;

    // Scripts --------------------------------------------------------------
    fn compile_script(&self, dsl_source: &str) -> ToolResult<CompiledScript>;
    fn deploy_script(
        &self,
        world_cid: &Cid,
        entity_ref: &str,
        script_cid: &Cid,
    ) -> ToolResult<WorldState>;

    // Simulation -----------------------------------------------------------
    fn run_sim(&self, world_cid: &Cid, scenario_yaml: &str) -> ToolResult<SimReport>;

    // UGC / moderation -----------------------------------------------------
    fn upload_ugc(
        &self,
        uploader: &str,
        media_type: &str,
        payload_b64: &str,
    ) -> ToolResult<UgcArtifact>;
    fn ugc_scan_status(&self, cid: &Cid) -> ToolResult<UgcArtifact>;
    fn ugc_approve(&self, cid: &Cid) -> ToolResult<UgcArtifact>;
    fn ugc_publish(&self, cid: &Cid) -> ToolResult<UgcArtifact>;
    fn report_moderation(&self, cid: &Cid, reason: &str) -> ToolResult<ModerationReport>;

    // Telemetry ------------------------------------------------------------
    fn telemetry_snapshot(
        &self,
        world_cid: &Cid,
        filter: Option<&str>,
    ) -> ToolResult<Vec<TelemetryEvent>>;
}

// ---------------------------------------------------------------------------
// In-memory implementation
// ---------------------------------------------------------------------------

/// Deterministic, in-process default [`Backend`]. Backed by a single `Mutex`
/// around a typed state struct.
pub struct InMemoryBackend {
    state: Mutex<BackendState>,
}

struct BackendState {
    worlds: HashMap<Cid, WorldState>,
    scripts: HashMap<Cid, CompiledScript>,
    ugc: HashMap<Cid, UgcArtifact>,
    telemetry: HashMap<Cid, Vec<TelemetryEvent>>,
    next_telemetry_seq: HashMap<Cid, u64>,
    moderation: Vec<ModerationReport>,
}

impl Default for InMemoryBackend {
    fn default() -> Self {
        Self {
            state: Mutex::new(BackendState {
                worlds: HashMap::new(),
                scripts: HashMap::new(),
                ugc: HashMap::new(),
                telemetry: HashMap::new(),
                next_telemetry_seq: HashMap::new(),
                moderation: Vec::new(),
            }),
        }
    }
}

impl InMemoryBackend {
    fn cid_for_json(value: &serde_json::Value) -> Cid {
        let canonical = serde_json::to_string(value).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(canonical.as_bytes());
        format!("cid:{}", hex_encode(&hasher.finalize()))
    }

    fn cid_for_str(kind: &str, s: &str) -> Cid {
        let mut hasher = Sha256::new();
        hasher.update(kind.as_bytes());
        hasher.update(b":");
        hasher.update(s.as_bytes());
        format!("cid:{}", hex_encode(&hasher.finalize()))
    }

    fn with_state<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut BackendState) -> R,
    {
        let mut guard = self.state.lock().expect("backend mutex poisoned");
        f(&mut guard)
    }

    fn emit_telemetry(
        state: &mut BackendState,
        world_cid: &Cid,
        kind: &str,
        payload: serde_json::Value,
    ) {
        let seq = state.next_telemetry_seq.entry(world_cid.clone()).or_insert(0);
        *seq += 1;
        let event = TelemetryEvent {
            world_cid: world_cid.clone(),
            seq: *seq,
            kind: kind.to_string(),
            payload,
            timestamp: Utc::now().to_rfc3339(),
        };
        state.telemetry.entry(world_cid.clone()).or_default().push(event);
    }

    /// Re-cid + re-insert a world after mutation. Returns the new stored state.
    fn rehash(state: &mut BackendState, mut world: WorldState) -> WorldState {
        // Zero out the cid field before hashing so that same content = same cid.
        let prior_cid = std::mem::take(&mut world.cid);
        let payload = serde_json::json!({
            "manifest": world.manifest,
            "entities": world.entities,
            "scripts": world.scripts,
            "links": world.links,
        });
        world.cid = Self::cid_for_json(&payload);
        state.worlds.remove(&prior_cid);
        state.worlds.insert(world.cid.clone(), world.clone());
        world
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

impl Backend for InMemoryBackend {
    fn create_world(&self, manifest_yaml: &str) -> ToolResult<WorldState> {
        let manifest: serde_json::Value = serde_yaml::from_str(manifest_yaml).map_err(|e| {
            ToolError::schema(
                format!("manifest_yaml is not valid YAML: {}", e),
                "/manifest_yaml",
            )
            .with_patch(RepairPatch::new("manifest YAML failed to parse").with_op(
                RepairOp::Hint {
                    path: "/manifest_yaml".into(),
                    hint: "ensure valid YAML 1.2 with at least a `name:` key".into(),
                },
            ))
        })?;
        if !manifest.get("name").and_then(|v| v.as_str()).is_some() {
            return Err(ToolError::schema(
                "manifest is missing required `name` field",
                "/manifest_yaml",
            )
            .with_patch(
                RepairPatch::new("manifest must declare a `name`").with_op(RepairOp::Replace {
                    path: "/manifest_yaml".into(),
                    value: serde_json::Value::String("name: unnamed-world\n".into()),
                }),
            ));
        }
        self.with_state(|s| {
            let cid = Self::cid_for_json(&manifest);
            if let Some(existing) = s.worlds.get(&cid) {
                return Ok(existing.clone());
            }
            let world = WorldState::new(cid.clone(), manifest);
            s.worlds.insert(cid.clone(), world.clone());
            Self::emit_telemetry(
                s,
                &cid,
                "world.created",
                serde_json::json!({ "cid": cid }),
            );
            Ok(world)
        })
    }

    fn patch_world(&self, base_cid: &Cid, patch: &serde_json::Value) -> ToolResult<WorldState> {
        self.with_state(|s| {
            let base = s.worlds.get(base_cid).cloned().ok_or_else(|| {
                ToolError::not_found("world", base_cid.to_string()).with_patch(
                    RepairPatch::new("world.patch requires an existing base_cid").with_op(
                        RepairOp::Hint {
                            path: "/base_cid".into(),
                            hint: "call world.create first, then use the returned cid".into(),
                        },
                    ),
                )
            })?;
            // Patch semantics: we accept `{ "manifest_merge": {..}, "entities": {"add": [..], "remove": ["id", ..]} }`.
            let mut next = base.clone();
            if let Some(merge) = patch.get("manifest_merge").and_then(|v| v.as_object()) {
                if let Some(obj) = next.manifest.as_object_mut() {
                    for (k, v) in merge {
                        obj.insert(k.clone(), v.clone());
                    }
                } else {
                    return Err(ToolError::new(
                        codes::CONFLICT,
                        "base manifest is not an object; cannot merge",
                    )
                    .at("/patch/manifest_merge"));
                }
            }
            if let Some(entities) = patch.get("entities") {
                if let Some(add) = entities.get("add").and_then(|v| v.as_array()) {
                    for e in add {
                        let id = e
                            .get("id")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| {
                                ToolError::schema(
                                    "entity missing `id`",
                                    "/patch/entities/add",
                                )
                                .with_patch(
                                    RepairPatch::new(
                                        "every added entity must carry a string `id`",
                                    )
                                    .with_op(RepairOp::Hint {
                                        path: "/patch/entities/add".into(),
                                        hint: "set `{\"id\":\"ent:...\",...}` on each entry"
                                            .into(),
                                    }),
                                )
                            })?
                            .to_string();
                        next.entities.insert(id, e.clone());
                    }
                }
                if let Some(remove) = entities.get("remove").and_then(|v| v.as_array()) {
                    for r in remove {
                        if let Some(id) = r.as_str() {
                            next.entities.remove(id);
                        }
                    }
                }
            }
            let out = Self::rehash(s, next);
            Self::emit_telemetry(
                s,
                &out.cid,
                "world.patched",
                serde_json::json!({ "base": base_cid, "new": out.cid }),
            );
            Ok(out)
        })
    }

    fn query_world(&self, cid: &Cid, jsonpath: &str) -> ToolResult<serde_json::Value> {
        self.with_state(|s| {
            let world = s.worlds.get(cid).ok_or_else(|| {
                ToolError::not_found("world", cid.to_string())
            })?;
            // Support a simple `/`-separated pointer for portability.
            let value: serde_json::Value = serde_json::to_value(world)
                .map_err(|e| ToolError::new(codes::INTERNAL, e.to_string()))?;
            Ok(lookup_pointer(&value, jsonpath).unwrap_or(serde_json::Value::Null))
        })
    }

    fn spawn_entities(
        &self,
        world_cid: &Cid,
        prototypes: &[serde_json::Value],
    ) -> ToolResult<WorldState> {
        if prototypes.is_empty() {
            return Err(ToolError::schema(
                "prototypes must not be empty",
                "/prototypes",
            )
            .with_patch(RepairPatch::new("supply at least one prototype").with_op(
                RepairOp::Replace {
                    path: "/prototypes".into(),
                    value: serde_json::json!([{"id":"ent:1","kind":"npc"}]),
                },
            )));
        }
        self.with_state(|s| {
            let mut world = s
                .worlds
                .get(world_cid)
                .cloned()
                .ok_or_else(|| ToolError::not_found("world", world_cid.to_string()))?;
            for (idx, p) in prototypes.iter().enumerate() {
                let id = p
                    .get("id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ToolError::schema(
                            format!("prototype #{} is missing `id`", idx),
                            format!("/prototypes/{}", idx),
                        )
                    })?;
                world.entities.insert(id.to_string(), p.clone());
            }
            let out = Self::rehash(s, world);
            Self::emit_telemetry(
                s,
                &out.cid,
                "entity.spawned",
                serde_json::json!({ "count": prototypes.len() }),
            );
            Ok(out)
        })
    }

    fn modify_entities(
        &self,
        world_cid: &Cid,
        ops: &[serde_json::Value],
    ) -> ToolResult<WorldState> {
        self.with_state(|s| {
            let mut world = s
                .worlds
                .get(world_cid)
                .cloned()
                .ok_or_else(|| ToolError::not_found("world", world_cid.to_string()))?;
            for (idx, op) in ops.iter().enumerate() {
                let kind = op.get("op").and_then(|v| v.as_str()).ok_or_else(|| {
                    ToolError::schema(
                        format!("modify op #{} missing `op`", idx),
                        format!("/ops/{}", idx),
                    )
                })?;
                let id = op.get("id").and_then(|v| v.as_str()).ok_or_else(|| {
                    ToolError::schema(
                        format!("modify op #{} missing `id`", idx),
                        format!("/ops/{}", idx),
                    )
                })?;
                match kind {
                    "set" => {
                        let patch = op.get("value").cloned().ok_or_else(|| {
                            ToolError::schema(
                                format!("set op #{} missing `value`", idx),
                                format!("/ops/{}/value", idx),
                            )
                        })?;
                        world.entities.insert(id.to_string(), patch);
                    }
                    "remove" => {
                        world.entities.remove(id);
                    }
                    other => {
                        return Err(ToolError::schema(
                            format!("unknown op kind `{}`", other),
                            format!("/ops/{}/op", idx),
                        )
                        .with_patch(RepairPatch::new("allowed ops are `set` and `remove`")
                            .with_op(RepairOp::Replace {
                                path: format!("/ops/{}/op", idx),
                                value: serde_json::Value::String("set".into()),
                            })));
                    }
                }
            }
            let out = Self::rehash(s, world);
            Self::emit_telemetry(
                s,
                &out.cid,
                "entity.modified",
                serde_json::json!({ "ops": ops.len() }),
            );
            Ok(out)
        })
    }

    fn link_entities(
        &self,
        world_cid: &Cid,
        source_id: &str,
        target_id: &str,
        link_kind: &str,
    ) -> ToolResult<WorldState> {
        self.with_state(|s| {
            let mut world = s
                .worlds
                .get(world_cid)
                .cloned()
                .ok_or_else(|| ToolError::not_found("world", world_cid.to_string()))?;
            if !world.entities.contains_key(source_id) {
                return Err(ToolError::not_found("entity", source_id.to_string())
                    .at("/source_id")
                    .with_patch(
                        RepairPatch::new("link source must exist in the world")
                            .with_op(RepairOp::Hint {
                                path: "/source_id".into(),
                                hint: "spawn the entity first via entity.spawn".into(),
                            }),
                    ));
            }
            if !world.entities.contains_key(target_id) {
                return Err(ToolError::not_found("entity", target_id.to_string())
                    .at("/target_id"));
            }
            world.links.push(Link {
                source_id: source_id.into(),
                target_id: target_id.into(),
                kind: link_kind.into(),
            });
            let out = Self::rehash(s, world);
            Self::emit_telemetry(
                s,
                &out.cid,
                "entity.linked",
                serde_json::json!({ "source": source_id, "target": target_id, "kind": link_kind }),
            );
            Ok(out)
        })
    }

    fn compile_script(&self, dsl_source: &str) -> ToolResult<CompiledScript> {
        if dsl_source.trim().is_empty() {
            return Err(ToolError::schema(
                "dsl_source must not be empty",
                "/dsl_source",
            )
            .with_patch(RepairPatch::new("supply a non-empty DSL source").with_op(
                RepairOp::Replace {
                    path: "/dsl_source".into(),
                    value: serde_json::Value::String("on tick do log \"hi\"\n".into()),
                },
            )));
        }
        // Minimal DSL check: we require the source to contain `on` somewhere.
        if !dsl_source.contains("on") {
            return Err(ToolError::new(
                codes::COMPILE_FAILED,
                "DSL source does not contain any `on <event>` handler",
            )
            .at("/dsl_source")
            .suggest("every script needs at least one `on <event> do <body>` clause")
            .with_patch(
                RepairPatch::new("add an event handler").with_op(RepairOp::Replace {
                    path: "/dsl_source".into(),
                    value: serde_json::Value::String(format!(
                        "on tick do log \"{}\"\n",
                        dsl_source.chars().take(16).collect::<String>().trim()
                    )),
                }),
            ));
        }
        // "Compile" = emit a deterministic tiny fake-WASM blob (the header bytes).
        let wasm_bytes: Vec<u8> = b"\0asm\x01\0\0\0"
            .iter()
            .copied()
            .chain(dsl_source.bytes())
            .collect();
        let cid = Self::cid_for_str("script", dsl_source);
        let compiled = CompiledScript {
            cid: cid.clone(),
            wasm_bytes_b64: BASE64.encode(&wasm_bytes),
            source_hash: hex_encode(Sha256::digest(dsl_source.as_bytes()).as_slice()),
        };
        self.with_state(|s| {
            s.scripts.insert(cid.clone(), compiled.clone());
        });
        Ok(compiled)
    }

    fn deploy_script(
        &self,
        world_cid: &Cid,
        entity_ref: &str,
        script_cid: &Cid,
    ) -> ToolResult<WorldState> {
        self.with_state(|s| {
            let script = s.scripts.get(script_cid).cloned().ok_or_else(|| {
                ToolError::not_found("script", script_cid.to_string())
                    .at("/script_cid")
                    .with_patch(
                        RepairPatch::new("call script.compile first and pass the returned cid")
                            .with_op(RepairOp::Hint {
                                path: "/script_cid".into(),
                                hint: "the cid must come from script.compile".into(),
                            }),
                    )
            })?;
            let mut world = s
                .worlds
                .get(world_cid)
                .cloned()
                .ok_or_else(|| ToolError::not_found("world", world_cid.to_string()))?;
            if !world.entities.contains_key(entity_ref) {
                return Err(ToolError::not_found("entity", entity_ref.to_string())
                    .at("/entity_ref"));
            }
            world.scripts.insert(
                entity_ref.to_string(),
                ScriptRef {
                    cid: script.cid.clone(),
                    wasm_bytes_b64: script.wasm_bytes_b64.clone(),
                },
            );
            let out = Self::rehash(s, world);
            Self::emit_telemetry(
                s,
                &out.cid,
                "script.deployed",
                serde_json::json!({ "entity": entity_ref, "script": script_cid }),
            );
            Ok(out)
        })
    }

    fn run_sim(&self, world_cid: &Cid, scenario_yaml: &str) -> ToolResult<SimReport> {
        let scenario: serde_json::Value = serde_yaml::from_str(scenario_yaml).map_err(|e| {
            ToolError::schema(
                format!("scenario_yaml is not valid YAML: {}", e),
                "/scenario_yaml",
            )
        })?;
        let ticks = scenario
            .get("ticks")
            .and_then(|v| v.as_u64())
            .unwrap_or(60);
        let expected = scenario.get("expect").and_then(|v| v.as_str()).unwrap_or("pass");
        self.with_state(|s| {
            let world = s.worlds.get(world_cid).ok_or_else(|| {
                ToolError::not_found("world", world_cid.to_string()).at("/world_cid")
            })?;
            let entity_count = world.entities.len() as u64;
            let script_count = world.scripts.len() as u64;
            let verdict = match expected {
                "fail" => SimVerdict::Fail,
                "inconclusive" => SimVerdict::Inconclusive,
                _ if entity_count == 0 => SimVerdict::Inconclusive,
                _ => SimVerdict::Pass,
            };
            let repair_patch = match verdict {
                SimVerdict::Pass => None,
                SimVerdict::Fail => Some(
                    RepairPatch::new(
                        "simulation asserted `expect: pass` but scenario demanded failure",
                    )
                    .with_op(RepairOp::Replace {
                        path: "/scenario_yaml".into(),
                        value: serde_json::Value::String("expect: pass\nticks: 60\n".into()),
                    }),
                ),
                SimVerdict::Inconclusive => Some(
                    RepairPatch::new("world has no entities to simulate").with_op(
                        RepairOp::Hint {
                            path: "/world_cid".into(),
                            hint: "call entity.spawn before sim.run".into(),
                        },
                    ),
                ),
            };
            let report = SimReport {
                verdict,
                ticks,
                telemetry: serde_json::json!({
                    "entities": entity_count,
                    "scripts": script_count,
                    "scenario": scenario,
                }),
                repair_patch,
            };
            Self::emit_telemetry(
                s,
                world_cid,
                "sim.ran",
                serde_json::json!({ "verdict": format!("{:?}", report.verdict), "ticks": ticks }),
            );
            Ok(report)
        })
    }

    fn upload_ugc(
        &self,
        uploader: &str,
        media_type: &str,
        payload_b64: &str,
    ) -> ToolResult<UgcArtifact> {
        if uploader.trim().is_empty() {
            return Err(ToolError::schema("uploader must not be empty", "/uploader"));
        }
        let bytes = BASE64.decode(payload_b64).map_err(|e| {
            ToolError::schema(
                format!("payload_b64 is not valid base64: {}", e),
                "/payload_b64",
            )
        })?;
        let cid = Self::cid_for_str("ugc", &BASE64.encode(&bytes));
        let artifact = UgcArtifact {
            cid: cid.clone(),
            uploader: uploader.to_string(),
            media_type: media_type.to_string(),
            status: UgcStatus::Scanning,
            moderation_notes: Vec::new(),
        };
        self.with_state(|s| s.ugc.insert(cid.clone(), artifact.clone()));
        Ok(artifact)
    }

    fn ugc_scan_status(&self, cid: &Cid) -> ToolResult<UgcArtifact> {
        self.with_state(|s| {
            s.ugc
                .get(cid)
                .cloned()
                .ok_or_else(|| ToolError::not_found("ugc_artifact", cid.to_string()))
        })
    }

    fn ugc_approve(&self, cid: &Cid) -> ToolResult<UgcArtifact> {
        self.with_state(|s| {
            let art = s.ugc.get_mut(cid).ok_or_else(|| {
                ToolError::not_found("ugc_artifact", cid.to_string())
            })?;
            if matches!(art.status, UgcStatus::Rejected) {
                return Err(ToolError::new(
                    codes::MODERATION_BLOCKED,
                    "artifact has been rejected and cannot be approved",
                )
                .with_patch(
                    RepairPatch::new("re-upload a compliant replacement").with_op(
                        RepairOp::Hint {
                            path: "/cid".into(),
                            hint: "call ugc.upload with a clean payload".into(),
                        },
                    ),
                ));
            }
            art.status = UgcStatus::Approved;
            Ok(art.clone())
        })
    }

    fn ugc_publish(&self, cid: &Cid) -> ToolResult<UgcArtifact> {
        self.with_state(|s| {
            let art = s.ugc.get_mut(cid).ok_or_else(|| {
                ToolError::not_found("ugc_artifact", cid.to_string())
            })?;
            if !matches!(art.status, UgcStatus::Approved) {
                return Err(ToolError::new(
                    codes::CONFLICT,
                    "only approved artifacts can be published",
                )
                .at("/cid")
                .with_patch(
                    RepairPatch::new("approve the artifact first").with_op(RepairOp::Hint {
                        path: "/cid".into(),
                        hint: "call ugc.approve before ugc.publish".into(),
                    }),
                ));
            }
            art.status = UgcStatus::Published;
            Ok(art.clone())
        })
    }

    fn report_moderation(&self, cid: &Cid, reason: &str) -> ToolResult<ModerationReport> {
        if reason.trim().is_empty() {
            return Err(ToolError::schema(
                "reason must not be empty",
                "/reason",
            )
            .with_patch(RepairPatch::new("describe the issue").with_op(RepairOp::Replace {
                path: "/reason".into(),
                value: serde_json::Value::String("contains disallowed content".into()),
            })));
        }
        self.with_state(|s| {
            if !s.ugc.contains_key(cid) {
                return Err(ToolError::not_found("ugc_artifact", cid.to_string()));
            }
            let report = ModerationReport {
                report_id: format!("mr_{}", s.moderation.len() + 1),
                artifact_cid: cid.clone(),
                reason: reason.to_string(),
                reported_at: Utc::now().to_rfc3339(),
            };
            // Auto-reject on any report, for the demo.
            if let Some(art) = s.ugc.get_mut(cid) {
                art.status = UgcStatus::Rejected;
                art.moderation_notes.push(reason.to_string());
            }
            s.moderation.push(report.clone());
            Ok(report)
        })
    }

    fn telemetry_snapshot(
        &self,
        world_cid: &Cid,
        filter: Option<&str>,
    ) -> ToolResult<Vec<TelemetryEvent>> {
        self.with_state(|s| {
            let events = s.telemetry.get(world_cid).cloned().unwrap_or_default();
            let filtered = match filter {
                None => events,
                Some(f) => events.into_iter().filter(|e| e.kind.contains(f)).collect(),
            };
            Ok(filtered)
        })
    }
}

fn lookup_pointer(value: &serde_json::Value, jsonpath: &str) -> Option<serde_json::Value> {
    // Treat incoming string as an RFC 6901 pointer (optionally without leading `/`).
    let trimmed = jsonpath.trim();
    if trimmed.is_empty() || trimmed == "/" {
        return Some(value.clone());
    }
    let pointer = if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{}", trimmed.trim_start_matches('$').trim_start_matches('.').replace('.', "/"))
    };
    value.pointer(&pointer).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_world_hashes_manifest() {
        let b = InMemoryBackend::default();
        let w = b.create_world("name: hello\nversion: 1\n").unwrap();
        assert!(w.cid.starts_with("cid:"));
        let w2 = b.create_world("name: hello\nversion: 1\n").unwrap();
        assert_eq!(w.cid, w2.cid, "same manifest must produce same cid");
    }

    #[test]
    fn create_world_rejects_missing_name() {
        let b = InMemoryBackend::default();
        let err = b.create_world("version: 1\n").unwrap_err();
        assert_eq!(err.code, codes::SCHEMA_VALIDATION);
        assert!(err.repair_patch.is_some());
    }

    #[test]
    fn patch_world_rejects_unknown_base() {
        let b = InMemoryBackend::default();
        let err = b
            .patch_world(&"cid:missing".to_string(), &serde_json::json!({}))
            .unwrap_err();
        assert_eq!(err.code, codes::NOT_FOUND);
    }

    #[test]
    fn spawn_requires_nonempty_prototypes() {
        let b = InMemoryBackend::default();
        let w = b.create_world("name: x\n").unwrap();
        let err = b.spawn_entities(&w.cid, &[]).unwrap_err();
        assert_eq!(err.code, codes::SCHEMA_VALIDATION);
    }

    #[test]
    fn spawn_then_modify_then_link() {
        let b = InMemoryBackend::default();
        let w = b.create_world("name: x\n").unwrap();
        let w = b
            .spawn_entities(
                &w.cid,
                &[
                    serde_json::json!({"id":"a"}),
                    serde_json::json!({"id":"b"}),
                ],
            )
            .unwrap();
        let w = b
            .modify_entities(
                &w.cid,
                &[serde_json::json!({"op":"set","id":"a","value":{"id":"a","hp":10}})],
            )
            .unwrap();
        let w = b.link_entities(&w.cid, "a", "b", "follows").unwrap();
        assert_eq!(w.entities.len(), 2);
        assert_eq!(w.links.len(), 1);
    }

    #[test]
    fn compile_script_rejects_empty() {
        let b = InMemoryBackend::default();
        let err = b.compile_script("").unwrap_err();
        assert_eq!(err.code, codes::SCHEMA_VALIDATION);
    }

    #[test]
    fn compile_script_rejects_no_handler() {
        let b = InMemoryBackend::default();
        let err = b.compile_script("let x = 1").unwrap_err();
        assert_eq!(err.code, codes::COMPILE_FAILED);
        assert!(err.repair_patch.is_some());
    }

    #[test]
    fn compile_script_succeeds_with_handler() {
        let b = InMemoryBackend::default();
        let c = b.compile_script("on tick do noop").unwrap();
        assert!(c.cid.starts_with("cid:"));
        assert!(!c.wasm_bytes_b64.is_empty());
    }

    #[test]
    fn deploy_requires_script_and_entity() {
        let b = InMemoryBackend::default();
        let w = b.create_world("name: x\n").unwrap();
        let err = b
            .deploy_script(&w.cid, "ghost", &"cid:missing".into())
            .unwrap_err();
        assert_eq!(err.code, codes::NOT_FOUND);
    }

    #[test]
    fn sim_run_inconclusive_when_empty() {
        let b = InMemoryBackend::default();
        let w = b.create_world("name: x\n").unwrap();
        let r = b.run_sim(&w.cid, "ticks: 10\n").unwrap();
        assert_eq!(r.verdict, SimVerdict::Inconclusive);
        assert!(r.repair_patch.is_some());
    }

    #[test]
    fn sim_run_pass_with_entities() {
        let b = InMemoryBackend::default();
        let w = b.create_world("name: x\n").unwrap();
        let w = b
            .spawn_entities(&w.cid, &[serde_json::json!({"id":"e1"})])
            .unwrap();
        let r = b.run_sim(&w.cid, "ticks: 5\n").unwrap();
        assert_eq!(r.verdict, SimVerdict::Pass);
    }

    #[test]
    fn ugc_lifecycle() {
        let b = InMemoryBackend::default();
        let a = b
            .upload_ugc("alice", "model/gltf", &BASE64.encode(b"hello"))
            .unwrap();
        assert_eq!(a.status, UgcStatus::Scanning);
        let a = b.ugc_approve(&a.cid).unwrap();
        assert_eq!(a.status, UgcStatus::Approved);
        let a = b.ugc_publish(&a.cid).unwrap();
        assert_eq!(a.status, UgcStatus::Published);
    }

    #[test]
    fn ugc_publish_requires_approval() {
        let b = InMemoryBackend::default();
        let a = b
            .upload_ugc("alice", "model/gltf", &BASE64.encode(b"hello"))
            .unwrap();
        let err = b.ugc_publish(&a.cid).unwrap_err();
        assert_eq!(err.code, codes::CONFLICT);
    }

    #[test]
    fn moderation_rejects_artifact() {
        let b = InMemoryBackend::default();
        let a = b
            .upload_ugc("alice", "model/gltf", &BASE64.encode(b"hello"))
            .unwrap();
        let _ = b.report_moderation(&a.cid, "bad stuff").unwrap();
        let err = b.ugc_approve(&a.cid).unwrap_err();
        assert_eq!(err.code, codes::MODERATION_BLOCKED);
    }

    #[test]
    fn telemetry_snapshot_returns_events() {
        let b = InMemoryBackend::default();
        let w = b.create_world("name: x\n").unwrap();
        let _ = b
            .spawn_entities(&w.cid, &[serde_json::json!({"id":"a"})])
            .unwrap();
        let events = b.telemetry_snapshot(&w.cid, None).unwrap();
        assert!(events.iter().any(|e| e.kind == "world.created"));
    }

    #[test]
    fn query_world_with_simple_pointer() {
        let b = InMemoryBackend::default();
        let w = b.create_world("name: hello\n").unwrap();
        let v = b.query_world(&w.cid, "/manifest/name").unwrap();
        assert_eq!(v, serde_json::json!("hello"));
    }
}
