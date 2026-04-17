//! End-to-end integration test for `agent-native-demo`.
//!
//! Drives the binary's `run` function in-process against the bundled fixtures
//! and asserts that the JSON-lines output contains every expected step plus a
//! `done` record with `verdict: "pass"`.
//!
//! Must finish in well under 5 seconds; see
//! `docs/design/agent-native-demo.md` for the wall-clock budget.

use std::path::PathBuf;
use std::time::{Duration, Instant};

// We compile the binary's `main.rs` as a module here by re-including it with
// the `path` attribute. This keeps the test against the real `run` function
// without duplicating code.
#[path = "../src/main.rs"]
#[allow(dead_code)]
mod demo_bin;

const EXPECTED_STEPS: &[&str] = &[
    "mcp.connect",
    "world.create",
    "entity.spawn",
    "script.compile",
    "script.deploy",
    "sim.run",
    "vcs.merge",
];

fn fixture(name: &str) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("fixtures");
    p.push(name);
    p
}

#[test]
fn full_thin_slice_end_to_end() {
    // Point the binary at the bundled fixtures without depending on the shell
    // cwd — the integration test may run from the target dir.
    std::env::set_var(
        "AETHER_DEMO_WORLD_FIXTURE_PATH",
        fixture("hello.world.yaml"),
    );
    std::env::set_var(
        "AETHER_DEMO_BEHAVIOR_FIXTURE_PATH",
        fixture("patrol.beh"),
    );
    std::env::set_var(
        "AETHER_DEMO_SCENARIO_FIXTURE_PATH",
        fixture("patrol.scenario.yaml"),
    );

    let mut buf: Vec<u8> = Vec::new();
    let started = Instant::now();
    let merge_cid = demo_bin::run(&mut buf).expect("demo run must succeed");
    let elapsed = started.elapsed();

    assert!(
        elapsed < Duration::from_secs(5),
        "demo must complete in <5s (took {:?})",
        elapsed
    );

    let text = String::from_utf8(buf).expect("stdout is utf-8");
    let lines: Vec<&str> = text.lines().collect();
    assert!(
        lines.len() >= EXPECTED_STEPS.len() + 1,
        "expected at least {} JSON-lines records, got {}: {}",
        EXPECTED_STEPS.len() + 1,
        lines.len(),
        text
    );

    // Every expected step appears, in order, with ok: true.
    let mut cursor = 0usize;
    for expected in EXPECTED_STEPS {
        let record: serde_json::Value = serde_json::from_str(lines[cursor])
            .unwrap_or_else(|e| panic!("line {} is not JSON: {} ({})", cursor, lines[cursor], e));
        assert_eq!(
            record.get("step").and_then(|v| v.as_str()),
            Some(*expected),
            "step {} mismatch: {}",
            cursor,
            lines[cursor]
        );
        assert_eq!(
            record.get("ok").and_then(|v| v.as_bool()),
            Some(true),
            "step {} ok mismatch: {}",
            cursor,
            lines[cursor]
        );
        cursor += 1;
    }

    // Final record: done + pass + merge_cid.
    let done: serde_json::Value = serde_json::from_str(lines[cursor])
        .expect("final record must be JSON");
    assert_eq!(done.get("step").and_then(|v| v.as_str()), Some("done"));
    assert_eq!(done.get("verdict").and_then(|v| v.as_str()), Some("pass"));
    let merge_from_line = done
        .get("merge_cid")
        .and_then(|v| v.as_str())
        .expect("merge_cid on done line");
    assert_eq!(merge_from_line, merge_cid);
    assert!(
        merge_from_line.starts_with("cid:v1:"),
        "merge_cid has expected shape, got {}",
        merge_from_line
    );
}
