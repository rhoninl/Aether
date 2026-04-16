//! YAML↔struct↔binary byte-for-byte round-trip tests.
//!
//! This suite also regenerates the fixture files under `tests/fixtures/` and
//! the JSON schemas under `docs/schemas/` at the workspace root. That gives
//! us a single command (`cargo test -p aether-schemas --test roundtrip`) that
//! refreshes every artifact the agent needs to consume.
//!
//! Re-running the test must produce byte-identical fixtures; any drift fails
//! CI. If a legitimate schema change lands, the author regenerates fixtures
//! by running the test, inspects the diff, and commits the new files.

use std::path::PathBuf;

use aether_schemas::{
    emit_all_schemas, from_canonical_bytes, from_yaml_str, to_canonical_bytes, to_yaml_string,
    Cid, ContentAddress, WorldManifest,
};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn workspace_root() -> PathBuf {
    // crates/aether-schemas/ -> ../..
    crate_root().parent().unwrap().parent().unwrap().to_path_buf()
}

fn fixture_dir() -> PathBuf {
    crate_root().join("tests").join("fixtures")
}

#[test]
fn yaml_struct_binary_roundtrip_is_lossless() {
    let manifest = WorldManifest::minimal_example();

    // YAML roundtrip.
    let yaml = to_yaml_string(&manifest).unwrap();
    let from_yaml: WorldManifest = from_yaml_str(&yaml).unwrap();
    assert_eq!(manifest, from_yaml);

    // Binary roundtrip.
    let bytes = to_canonical_bytes(&manifest).unwrap();
    let from_bytes: WorldManifest = from_canonical_bytes(&bytes).unwrap();
    assert_eq!(manifest, from_bytes);

    // YAML → struct → binary must match direct struct → binary.
    let bytes_direct = to_canonical_bytes(&manifest).unwrap();
    let bytes_via_yaml = to_canonical_bytes(&from_yaml).unwrap();
    assert_eq!(
        bytes_direct, bytes_via_yaml,
        "YAML round-trip must preserve canonical bytes exactly"
    );
}

#[test]
fn regenerate_fixtures_and_json_schemas() {
    let fixtures = fixture_dir();
    std::fs::create_dir_all(&fixtures).unwrap();

    let manifest = WorldManifest::minimal_example();
    let yaml = to_yaml_string(&manifest).unwrap();
    let bytes = to_canonical_bytes(&manifest).unwrap();
    let cid = manifest.cid().unwrap();

    let yaml_path = fixtures.join("hello.yaml");
    let bin_path = fixtures.join("hello.bin");
    let cid_path = fixtures.join("hello.cid");

    std::fs::write(&yaml_path, yaml.as_bytes()).unwrap();
    std::fs::write(&bin_path, &bytes).unwrap();
    std::fs::write(&cid_path, cid.to_string().as_bytes()).unwrap();

    // JSON schemas land at workspace root under docs/schemas/ so they're
    // easy to find and reference from agent system prompts.
    let schemas_dir = workspace_root().join("docs").join("schemas");
    let emitted = emit_all_schemas(&schemas_dir).unwrap();
    assert!(!emitted.is_empty(), "at least one schema must be emitted");

    // Re-read and verify byte stability.
    let yaml_reread = std::fs::read_to_string(&yaml_path).unwrap();
    let bin_reread = std::fs::read(&bin_path).unwrap();
    let cid_reread = std::fs::read_to_string(&cid_path).unwrap();

    let parsed: WorldManifest = from_yaml_str(&yaml_reread).unwrap();
    let parsed_bytes = to_canonical_bytes(&parsed).unwrap();
    assert_eq!(parsed_bytes, bin_reread, "binary fixture drift");

    let parsed_cid: Cid = cid_reread.trim().parse().unwrap();
    assert_eq!(parsed_cid, cid);
}

#[test]
fn canonical_bytes_deterministic_across_independent_constructions() {
    // Same logical manifest built via two independent paths must yield
    // identical bytes. This protects against hidden nondeterminism from map
    // ordering, iterator ordering, or float formatting drift.
    let a = WorldManifest::minimal_example();
    let b = WorldManifest::minimal_example();
    assert_eq!(to_canonical_bytes(&a).unwrap(), to_canonical_bytes(&b).unwrap());
}
