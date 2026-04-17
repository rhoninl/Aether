//! Golden scenario runner.
//!
//! Each `.scenario` file in `tests/golden/` has a sibling `.expected.json`
//! declaring the expected verdict shape and key telemetry counters. This
//! runner loads every pair and asserts the harness produces the expected
//! report.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use aether_sim_harness::{run_scenario, Scenario, Verdict};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Expected {
    /// One of `"pass"`, `"pass_with_warnings"`, `"fail"`.
    verdict: String,
    #[serde(default)]
    min_counters: HashMap<String, u64>,
    #[serde(default)]
    max_counters: HashMap<String, u64>,
    #[serde(default)]
    expect_repair_patch: Option<bool>,
    #[serde(default)]
    min_comfort: Option<f32>,
    #[serde(default)]
    min_coherence: Option<f32>,
}

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
}

fn discover_pairs() -> Vec<(PathBuf, PathBuf)> {
    let dir = golden_dir();
    let mut pairs = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("read golden dir") {
        let entry = entry.expect("read entry");
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("scenario") {
            continue;
        }
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        let expected = dir.join(format!("{stem}.expected.json"));
        assert!(
            expected.exists(),
            "missing expected file for {}: {}",
            path.display(),
            expected.display()
        );
        pairs.push((path, expected));
    }
    pairs.sort();
    pairs
}

fn verdict_tag(v: &Verdict) -> &'static str {
    match v {
        Verdict::Pass => "pass",
        Verdict::PassWithWarnings { .. } => "pass_with_warnings",
        Verdict::Fail { .. } => "fail",
    }
}

fn load_expected(path: &Path) -> Expected {
    let bytes = std::fs::read(path).expect("read expected");
    serde_json::from_slice(&bytes).expect("parse expected")
}

#[test]
fn all_golden_scenarios_match_expected() {
    let pairs = discover_pairs();
    assert!(
        pairs.len() >= 10,
        "expected >=10 golden scenarios, found {}",
        pairs.len()
    );

    for (scenario_path, expected_path) in pairs {
        let scenario = Scenario::load(&scenario_path).unwrap_or_else(|e| {
            panic!("load {}: {e}", scenario_path.display());
        });
        let expected = load_expected(&expected_path);
        let report = run_scenario(&scenario);

        let got_tag = verdict_tag(&report.verdict);
        assert_eq!(
            got_tag,
            expected.verdict,
            "scenario {} expected verdict {} got {} ({:?})",
            scenario_path.display(),
            expected.verdict,
            got_tag,
            report.verdict
        );

        for (k, min) in &expected.min_counters {
            let got = report.telemetry.counter(k);
            assert!(
                got >= *min,
                "scenario {} counter {k} expected >= {min}, got {got}",
                scenario_path.display()
            );
        }
        for (k, max) in &expected.max_counters {
            let got = report.telemetry.counter(k);
            assert!(
                got <= *max,
                "scenario {} counter {k} expected <= {max}, got {got}",
                scenario_path.display()
            );
        }
        if let Some(expect_repair) = expected.expect_repair_patch {
            assert_eq!(
                report.repair_patch.is_some(),
                expect_repair,
                "scenario {} repair_patch presence mismatch",
                scenario_path.display()
            );
        }
        if let Some(min) = expected.min_comfort {
            assert!(
                report.comfort.overall >= min,
                "scenario {} comfort {} < {min}",
                scenario_path.display(),
                report.comfort.overall
            );
        }
        if let Some(min) = expected.min_coherence {
            assert!(
                report.coherence.overall >= min,
                "scenario {} coherence {} < {min}",
                scenario_path.display(),
                report.coherence.overall
            );
        }
    }
}

#[test]
fn g10_replay_is_bit_identical_across_runs() {
    let dir = golden_dir();
    let path = dir.join("g10_deterministic_replay_bitidentical.scenario");
    let scenario = Scenario::load(&path).expect("load g10");

    let a = run_scenario(&scenario);
    let b = run_scenario(&scenario);

    let a_bytes = serde_json::to_vec(&a).unwrap();
    let b_bytes = serde_json::to_vec(&b).unwrap();
    assert_eq!(
        a_bytes, b_bytes,
        "g10 replays must be byte-identical across runs"
    );
    assert_eq!(a.hash(), b.hash());
}
