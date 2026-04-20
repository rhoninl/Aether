//! `agent-native-demo` — thin-slice end-to-end proof that the Aether engine
//! can be driven, from first byte to promoted merge, by an AI agent speaking
//! the MCP tool surface.
//!
//! The demo now wires the real crates:
//! * steps 2–6 dispatch through `aether_agent_cp::ToolRegistry` against an
//!   `InMemoryBackend` — the same code path services/agent-cp serves over
//!   stdio / WebSocket / gRPC, just plumbed in-process so `cargo run` works
//!   without a separate server.
//! * step 7 signs a diff with an Ed25519 key from `aether_world_vcs::sig` and
//!   moves the `main` branch head via `MemoryBranchStore`.
//!
//! The binary prints one JSON-lines record per step and a final `done` record
//! with the merge CID, then exits 0 on success.
//!
//! Configuration (all env vars, no hardcoded paths):
//! * `AETHER_DEMO_WORLD_FIXTURE_PATH`      — YAML world manifest. Default:
//!   `examples/agent-native-demo/fixtures/hello.world.yaml`.
//! * `AETHER_DEMO_BEHAVIOR_FIXTURE_PATH`   — DSL source. Default:
//!   `examples/agent-native-demo/fixtures/patrol.beh`.
//! * `AETHER_DEMO_SCENARIO_FIXTURE_PATH`   — YAML scenario. Default:
//!   `examples/agent-native-demo/fixtures/patrol.scenario.yaml`.
//! * `AETHER_DEMO_AGENT_CP_ADDR`           — informational; the in-process
//!   ToolRegistry is always used so this env var is surfaced for parity with
//!   the services/agent-cp remote transport only.
//! * `AETHER_DEMO_SIGNER_ID`               — agent id recorded on the signed
//!   diff. Default: `agent:demo`.
//! * `AETHER_DEMO_TARGET_BRANCH`           — branch to merge into. Default:
//!   `main`.

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use aether_agent_cp::{
    backend::InMemoryBackend, tools::build_default_registry, registry::ToolRegistry,
};
use aether_world_vcs::{
    branch::{BranchStore, MemoryBranchStore, DEFAULT_BRANCH},
    diff::{canonical_cbor, cid_of, cid_to_hex, AgentRef, Cid, Diff},
    sig::{generate_keypair, sign_diff, verify_signed_diff},
};
use serde::Serialize;

// ---------------------------------------------------------------------------
// Config constants (top of file, per coding standard).
// ---------------------------------------------------------------------------

const ENV_WORLD_FIXTURE: &str = "AETHER_DEMO_WORLD_FIXTURE_PATH";
const ENV_BEHAVIOR_FIXTURE: &str = "AETHER_DEMO_BEHAVIOR_FIXTURE_PATH";
const ENV_SCENARIO_FIXTURE: &str = "AETHER_DEMO_SCENARIO_FIXTURE_PATH";
const ENV_AGENT_CP_ADDR: &str = "AETHER_DEMO_AGENT_CP_ADDR";
const ENV_SIGNER_ID: &str = "AETHER_DEMO_SIGNER_ID";
const ENV_TARGET_BRANCH: &str = "AETHER_DEMO_TARGET_BRANCH";

const DEFAULT_WORLD_FIXTURE: &str = "examples/agent-native-demo/fixtures/hello.world.yaml";
const DEFAULT_BEHAVIOR_FIXTURE: &str = "examples/agent-native-demo/fixtures/patrol.beh";
const DEFAULT_SCENARIO_FIXTURE: &str = "examples/agent-native-demo/fixtures/patrol.scenario.yaml";
const DEFAULT_SIGNER_ID: &str = "agent:demo";
const DEFAULT_TARGET_BRANCH: &str = "main";
const DEFAULT_AGENT_CP_ADDR: &str = "in-process://tool-registry";

const CUBE_ENTITY_ID: &str = "cube-0";
const CUBE_ENTITY_KIND: &str = "cube";
const CUBE_SPAWN_POSITION: [f32; 3] = [0.0, 1.0, 0.0];

const CID_DISPLAY_PREFIX: &str = "cid:v1:";

// ---------------------------------------------------------------------------
// JSON-lines emitter.
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct StepRecord<'a> {
    step: &'a str,
    elapsed_ms: u128,
    ok: bool,
    summary: serde_json::Value,
}

#[derive(Serialize)]
struct DoneRecord<'a> {
    step: &'a str,
    verdict: &'a str,
    merge_cid: String,
    total_ms: u128,
}

fn emit<W: Write, T: Serialize>(out: &mut W, record: &T) -> io::Result<()> {
    let s = serde_json::to_string(record).expect("serialize step record");
    writeln!(out, "{}", s)
}

// ---------------------------------------------------------------------------
// Run — separated from `main` so the integration test can drive it in-process.
// ---------------------------------------------------------------------------

/// Run the full demo. Emits one JSON-lines record per step to `out`. Returns
/// the merge CID on success.
pub fn run<W: Write>(mut out: W) -> Result<String, DemoError> {
    let start = Instant::now();
    tracing::info!("agent-native-demo: start");

    let world_path = env_path(ENV_WORLD_FIXTURE, DEFAULT_WORLD_FIXTURE);
    let behavior_path = env_path(ENV_BEHAVIOR_FIXTURE, DEFAULT_BEHAVIOR_FIXTURE);
    let scenario_path = env_path(ENV_SCENARIO_FIXTURE, DEFAULT_SCENARIO_FIXTURE);
    let signer = std::env::var(ENV_SIGNER_ID).unwrap_or_else(|_| DEFAULT_SIGNER_ID.into());
    let branch = std::env::var(ENV_TARGET_BRANCH).unwrap_or_else(|_| DEFAULT_TARGET_BRANCH.into());
    let agent_cp_addr =
        std::env::var(ENV_AGENT_CP_ADDR).unwrap_or_else(|_| DEFAULT_AGENT_CP_ADDR.into());

    // Build the real ToolRegistry against the InMemoryBackend. This is the
    // exact same registry services/agent-cp exposes — we just call it
    // in-process so the demo has no transport dependency.
    let backend = Arc::new(InMemoryBackend::default());
    let registry = build_default_registry(backend);

    // Step 1: "connect" — in-process ToolRegistry snapshot.
    emit(
        &mut out,
        &StepRecord {
            step: "mcp.connect",
            elapsed_ms: start.elapsed().as_millis(),
            ok: true,
            summary: serde_json::json!({
                "transport": "in-process",
                "addr": agent_cp_addr,
                "tools": registry.tool_names(),
            }),
        },
    )?;

    // Step 2: world.create.
    let manifest_yaml = read_to_string(&world_path, "world manifest")?;
    let created = call_tool(&registry, "world.create", serde_json::json!({ "manifest_yaml": manifest_yaml }))?;
    let genesis_world_cid = json_string(&created, "cid")?;
    let world_name = created
        .pointer("/manifest/name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    emit(
        &mut out,
        &StepRecord {
            step: "world.create",
            elapsed_ms: start.elapsed().as_millis(),
            ok: true,
            summary: serde_json::json!({
                "world_cid": genesis_world_cid,
                "name": world_name,
            }),
        },
    )?;

    // Step 3: entity.spawn (batch of 1 cube).
    let spawned = call_tool(
        &registry,
        "entity.spawn",
        serde_json::json!({
            "world_cid": genesis_world_cid,
            "prototypes": [{
                "id": CUBE_ENTITY_ID,
                "kind": CUBE_ENTITY_KIND,
                "position": CUBE_SPAWN_POSITION,
            }]
        }),
    )?;
    let world_cid_after_spawn = json_string(&spawned, "cid")?;
    emit(
        &mut out,
        &StepRecord {
            step: "entity.spawn",
            elapsed_ms: start.elapsed().as_millis(),
            ok: true,
            summary: serde_json::json!({
                "entity_id": CUBE_ENTITY_ID,
                "kind": CUBE_ENTITY_KIND,
                "position": CUBE_SPAWN_POSITION,
                "world_cid": world_cid_after_spawn,
            }),
        },
    )?;

    // Step 4: script.compile.
    let dsl_source = read_to_string(&behavior_path, "behavior source")?;
    let compiled = call_tool(
        &registry,
        "script.compile",
        serde_json::json!({ "dsl_source": dsl_source }),
    )?;
    let script_cid = json_string(&compiled, "cid")?;
    let wasm_len = compiled
        .get("wasm_bytes_b64")
        .and_then(|v| v.as_str())
        .map(|s| s.len())
        .unwrap_or(0);
    emit(
        &mut out,
        &StepRecord {
            step: "script.compile",
            elapsed_ms: start.elapsed().as_millis(),
            ok: true,
            summary: serde_json::json!({
                "script_cid": script_cid,
                "wasm_b64_len": wasm_len,
            }),
        },
    )?;

    // Step 5: script.deploy.
    let deployed = call_tool(
        &registry,
        "script.deploy",
        serde_json::json!({
            "world_cid": world_cid_after_spawn,
            "entity_ref": CUBE_ENTITY_ID,
            "script_cid": script_cid,
        }),
    )?;
    let world_cid_after_deploy = json_string(&deployed, "cid")?;
    emit(
        &mut out,
        &StepRecord {
            step: "script.deploy",
            elapsed_ms: start.elapsed().as_millis(),
            ok: true,
            summary: serde_json::json!({
                "entity_id": CUBE_ENTITY_ID,
                "script_cid": script_cid,
                "world_cid": world_cid_after_deploy,
            }),
        },
    )?;

    // Step 6: sim.run against the world that now has a scripted entity.
    let scenario_yaml = read_to_string(&scenario_path, "scenario")?;
    let report = call_tool(
        &registry,
        "sim.run",
        serde_json::json!({
            "world_cid": world_cid_after_deploy,
            "scenario_yaml": scenario_yaml,
        }),
    )?;
    let verdict = report
        .get("verdict")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if verdict != "pass" {
        return Err(DemoError::Verdict(format!(
            "sim.run returned verdict {:?}, expected pass",
            verdict
        )));
    }
    let ticks = report.get("ticks").and_then(|v| v.as_u64()).unwrap_or(0);
    emit(
        &mut out,
        &StepRecord {
            step: "sim.run",
            elapsed_ms: start.elapsed().as_millis(),
            ok: true,
            summary: serde_json::json!({
                "world_cid": world_cid_after_deploy,
                "ticks": ticks,
                "verdict": verdict,
            }),
        },
    )?;

    // Step 7: sign a diff and move the branch head via aether-world-vcs.
    let (sk, _vk) = generate_keypair();
    let diff = Diff {
        base: zero_cid(),
        target: target_cid_from_world_cid(&world_cid_after_deploy),
        ops: Vec::new(),
        author: AgentRef::Agent {
            service_account: signer.clone(),
        },
        timestamp_unix_ms: 0,
    };
    let signed = sign_diff(diff, &sk).map_err(|e| DemoError::Vcs(e.to_string()))?;
    verify_signed_diff(&signed).map_err(|e| DemoError::Vcs(format!("signature did not verify: {}", e)))?;
    let diff_cid =
        cid_of(&signed.diff).map_err(|e| DemoError::Vcs(format!("cid_of(diff): {}", e)))?;

    let mut store = MemoryBranchStore::with_default_main(AgentRef::Agent {
        service_account: signer.clone(),
    });
    store
        .set_head(&branch, signed.diff.target)
        .map_err(|e| DemoError::Vcs(format!("set_head: {}", e)))?;
    store.record_ancestry(signed.diff.target, signed.diff.base);

    let merge_cid_display = format!("{}{}", CID_DISPLAY_PREFIX, cid_to_hex(&diff_cid));
    let head_after_merge = store
        .head(&branch)
        .map_err(|e| DemoError::Vcs(format!("head: {}", e)))?;
    emit(
        &mut out,
        &StepRecord {
            step: "vcs.merge",
            elapsed_ms: start.elapsed().as_millis(),
            ok: true,
            summary: serde_json::json!({
                "branch": branch,
                "base": cid_to_hex(&signed.diff.base),
                "target": cid_to_hex(&signed.diff.target),
                "head_after_merge": cid_to_hex(&head_after_merge),
                "signer": signer,
                "canonical_bytes": canonical_cbor(&signed.diff)
                    .map_err(|e| DemoError::Vcs(format!("canonical_cbor: {}", e)))?
                    .len(),
                "merge_cid": merge_cid_display,
            }),
        },
    )?;

    // Step 8: done.
    emit(
        &mut out,
        &DoneRecord {
            step: "done",
            verdict: "pass",
            merge_cid: merge_cid_display.clone(),
            total_ms: start.elapsed().as_millis(),
        },
    )?;
    tracing::info!(merge_cid = %merge_cid_display, "agent-native-demo: pass");
    Ok(merge_cid_display)
}

// ---------------------------------------------------------------------------
// Helpers.
// ---------------------------------------------------------------------------

fn env_path(var: &str, default: &str) -> PathBuf {
    std::env::var(var)
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(default))
}

fn read_to_string(path: &Path, what: &str) -> Result<String, DemoError> {
    std::fs::read_to_string(path)
        .map_err(|e| DemoError::Fixture(format!("{} ({}): {}", what, path.display(), e)))
}

fn call_tool(
    registry: &ToolRegistry,
    name: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, DemoError> {
    registry.call(name, params).map_err(|e| {
        DemoError::Mcp(format!(
            "{}: code={} msg={}",
            name, e.code, e.message
        ))
    })
}

fn json_string(value: &serde_json::Value, key: &str) -> Result<String, DemoError> {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| DemoError::Mcp(format!("tool response missing string `{}`", key)))
}

fn zero_cid() -> Cid {
    [0u8; 32]
}

/// Derive a stable 32-byte CID from the backend's string-shaped world CID so
/// we can thread it into aether-world-vcs (which uses `[u8; 32]`).
fn target_cid_from_world_cid(world_cid: &str) -> Cid {
    use sha2::{Digest, Sha256};
    let mut out = [0u8; 32];
    out.copy_from_slice(&Sha256::digest(world_cid.as_bytes()));
    out
}

#[derive(Debug, thiserror::Error)]
pub enum DemoError {
    #[error("io: {0}")]
    Io(#[from] io::Error),
    #[error("fixture: {0}")]
    Fixture(String),
    #[error("mcp: {0}")]
    Mcp(String),
    #[error("verdict: {0}")]
    Verdict(String),
    #[error("vcs: {0}")]
    Vcs(String),
}

// ---------------------------------------------------------------------------
// Entry point.
// ---------------------------------------------------------------------------

fn main() -> std::process::ExitCode {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with_writer(std::io::stderr)
        .try_init();

    let stdout = io::stdout();
    let mut handle = stdout.lock();
    match run(&mut handle) {
        Ok(_) => std::process::ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!(error = %err, "agent-native-demo failed");
            let _ = emit(
                &mut handle,
                &StepRecord {
                    step: "error",
                    elapsed_ms: 0,
                    ok: false,
                    summary: serde_json::json!({ "error": err.to_string() }),
                },
            );
            std::process::ExitCode::FAILURE
        }
    }
}

// Silence unused-import warnings on platforms where the transitive crate
// re-exports are platform-conditional. Keeping this list small and explicit so
// a missing import becomes a hard error.
#[allow(dead_code)]
const _USED_BRANCH_DEFAULT: &str = DEFAULT_BRANCH;
