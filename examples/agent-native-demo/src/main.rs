//! `agent-native-demo` — thin-slice end-to-end proof that the Aether engine
//! can be driven, from first byte to promoted merge, by an AI agent speaking
//! MCP.
//!
//! See `docs/design/agent-native-demo.md` for the full narrative. The binary
//! prints one JSON-lines record per step and a final `done` record with the
//! merge CID, then exits 0 on success.
//!
//! Configuration (all env vars, no hardcoded paths):
//! * `AETHER_DEMO_WORLD_FIXTURE_PATH`      — YAML world manifest. Default:
//!   `examples/agent-native-demo/fixtures/hello.world.yaml`.
//! * `AETHER_DEMO_BEHAVIOR_FIXTURE_PATH`   — DSL source. Default:
//!   `examples/agent-native-demo/fixtures/patrol.beh`.
//! * `AETHER_DEMO_SCENARIO_FIXTURE_PATH`   — YAML scenario. Default:
//!   `examples/agent-native-demo/fixtures/patrol.scenario.yaml`.
//! * `AETHER_DEMO_AGENT_CP_ADDR`           — ignored by the stub transport,
//!   kept for parity with the real MCP client.
//! * `AETHER_DEMO_SIGNER_ID`               — agent id recorded on the signed
//!   diff. Default: `agent:demo`.
//! * `AETHER_DEMO_TARGET_BRANCH`           — branch to merge into. Default:
//!   `main`.

use std::io::{self, Write};
use std::path::PathBuf;
use std::time::Instant;

use serde::Serialize;

#[cfg(feature = "stubs")]
mod stubs;
#[cfg(feature = "stubs")]
use stubs::{
    cid_of, load_behavior_source, load_scenario, load_world_manifest, world_vcs, AgentCpClient,
    DiffSpec,
};

#[cfg(not(feature = "stubs"))]
compile_error!(
    "agent-native-demo: the `real` feature wiring is still a post-batch follow-up; build with the default `stubs` feature for now"
);

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

const CUBE_SPAWN_POSITION: [f32; 3] = [0.0, 1.0, 0.0];
const CUBE_ENTITY_KIND: &str = "cube";

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
    let agent_cp_addr = std::env::var(ENV_AGENT_CP_ADDR).unwrap_or_else(|_| "stdio".into());

    // Step 1: connect over MCP.
    let mut client = AgentCpClient::stdio();
    emit(
        &mut out,
        &StepRecord {
            step: "mcp.connect",
            elapsed_ms: start.elapsed().as_millis(),
            ok: true,
            summary: serde_json::json!({ "transport": "stdio", "addr": agent_cp_addr }),
        },
    )?;

    // Step 2: world.create.
    let manifest = load_world_manifest(&world_path)
        .map_err(|e| DemoError::Fixture(format!("world manifest ({}): {}", world_path.display(), e)))?;
    let world = client
        .world_create(&manifest)
        .map_err(|e| DemoError::Mcp(format!("world.create: {}", e)))?;
    let genesis_world_cid = world.world_cid.clone();
    emit(
        &mut out,
        &StepRecord {
            step: "world.create",
            elapsed_ms: start.elapsed().as_millis(),
            ok: true,
            summary: serde_json::json!({
                "world_cid": world.world_cid,
                "name": manifest.name,
                "chunk_count": manifest.chunks.len(),
            }),
        },
    )?;

    // Step 3: entity.spawn (batch of 1 cube at (0,1,0)).
    let batch = vec![(CUBE_ENTITY_KIND.to_string(), CUBE_SPAWN_POSITION)];
    let spawned = client
        .entity_spawn(&batch)
        .map_err(|e| DemoError::Mcp(format!("entity.spawn: {}", e)))?;
    let cube_id = spawned
        .entity_ids
        .first()
        .cloned()
        .ok_or_else(|| DemoError::Mcp("entity.spawn returned no ids".into()))?;
    emit(
        &mut out,
        &StepRecord {
            step: "entity.spawn",
            elapsed_ms: start.elapsed().as_millis(),
            ok: true,
            summary: serde_json::json!({
                "entity_id": cube_id,
                "kind": CUBE_ENTITY_KIND,
                "position": CUBE_SPAWN_POSITION,
            }),
        },
    )?;

    // Step 4: script.compile.
    let source = load_behavior_source(&behavior_path).map_err(|e| {
        DemoError::Fixture(format!("behavior source ({}): {}", behavior_path.display(), e))
    })?;
    let compiled = client
        .script_compile(&source)
        .map_err(|e| DemoError::Mcp(format!("script.compile: {}", e)))?;
    emit(
        &mut out,
        &StepRecord {
            step: "script.compile",
            elapsed_ms: start.elapsed().as_millis(),
            ok: true,
            summary: serde_json::json!({
                "script_cid": compiled.script_cid,
                "wasm_len": compiled.wasm.len(),
                "verb_count": compiled.verb_count,
            }),
        },
    )?;

    // Step 5: script.deploy.
    let deploy = client
        .script_deploy(&cube_id, &compiled.script_cid)
        .map_err(|e| DemoError::Mcp(format!("script.deploy: {}", e)))?;
    emit(
        &mut out,
        &StepRecord {
            step: "script.deploy",
            elapsed_ms: start.elapsed().as_millis(),
            ok: true,
            summary: serde_json::json!({
                "entity_id": deploy.entity_id,
                "script_cid": deploy.script_cid,
            }),
        },
    )?;

    // Step 6: sim.run.
    let scenario = load_scenario(&scenario_path).map_err(|e| {
        DemoError::Fixture(format!("scenario ({}): {}", scenario_path.display(), e))
    })?;
    let report = client
        .sim_run(&scenario)
        .map_err(|e| DemoError::Mcp(format!("sim.run: {}", e)))?;
    if !report.verdict.is_pass() {
        return Err(DemoError::Verdict(format!(
            "simulation did not pass: {:?}",
            report.verdict
        )));
    }
    emit(
        &mut out,
        &StepRecord {
            step: "sim.run",
            elapsed_ms: start.elapsed().as_millis(),
            ok: true,
            summary: serde_json::json!({
                "scenario": scenario.name,
                "ticks_run": report.ticks_run,
                "wall_ms": report.wall_ms,
                "verdict": "pass",
            }),
        },
    )?;

    // Step 7: promote to main via world-vcs.
    // The genesis world has the plane + spawn point; the "head" world has the
    // cube + deployed script. In the real crate we'd walk the snapshot diff;
    // the stub derives a head CID from the genesis + mutations.
    let head_world_cid = cid_of(
        format!(
            "{}|entity={}|script={}",
            genesis_world_cid, cube_id, compiled.script_cid
        )
        .as_bytes(),
    );
    let diff = DiffSpec {
        base_world_cid: genesis_world_cid.clone(),
        head_world_cid,
        summary: format!(
            "spawn {} at {:?}, deploy patrol script {}",
            CUBE_ENTITY_KIND, CUBE_SPAWN_POSITION, compiled.script_cid
        ),
    };
    let signed = world_vcs::sign_diff(diff, &signer);
    let receipt = world_vcs::merge(&signed, &branch);
    emit(
        &mut out,
        &StepRecord {
            step: "vcs.merge",
            elapsed_ms: start.elapsed().as_millis(),
            ok: true,
            summary: serde_json::json!({
                "branch": receipt.branch,
                "base": signed.diff.base_world_cid,
                "head": signed.diff.head_world_cid,
                "signer": signed.signer,
                "merge_cid": receipt.merge_cid,
            }),
        },
    )?;

    // Step 8: done.
    emit(
        &mut out,
        &DoneRecord {
            step: "done",
            verdict: "pass",
            merge_cid: receipt.merge_cid.clone(),
            total_ms: start.elapsed().as_millis(),
        },
    )?;
    tracing::info!(merge_cid = %receipt.merge_cid, "agent-native-demo: pass");
    Ok(receipt.merge_cid)
}

fn env_path(var: &str, default: &str) -> PathBuf {
    std::env::var(var)
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(default))
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
