//! Content-addressing (CID) tests (task 72).
//!
//! The invariant under test: given the same logical artifact, the CID must be
//! byte-for-byte stable across:
//! 1. Multiple calls in the same process.
//! 2. Serialization → deserialization → re-serialization.
//! 3. YAML round-trips.
//!
//! If a schema change intentionally breaks this contract, the author must
//! bump `schema_version` so the CID tag changes accordingly.

use aether_schemas::{
    from_canonical_bytes, from_yaml_str, to_canonical_bytes, to_yaml_string, Cid, ContentAddress,
    SchemaVersion, WorldManifest,
};

#[test]
fn cid_is_stable_within_process() {
    let m = WorldManifest::minimal_example();
    let a = m.cid().unwrap();
    let b = m.cid().unwrap();
    let c = m.cid().unwrap();
    assert_eq!(a, b);
    assert_eq!(b, c);
}

#[test]
fn cid_is_stable_after_yaml_roundtrip() {
    let m = WorldManifest::minimal_example();
    let cid1 = m.cid().unwrap();
    let yaml = to_yaml_string(&m).unwrap();
    let back: WorldManifest = from_yaml_str(&yaml).unwrap();
    let cid2 = back.cid().unwrap();
    assert_eq!(cid1, cid2);
}

#[test]
fn cid_is_stable_after_binary_roundtrip() {
    let m = WorldManifest::minimal_example();
    let cid1 = m.cid().unwrap();
    let bytes = to_canonical_bytes(&m).unwrap();
    let back: WorldManifest = from_canonical_bytes(&bytes).unwrap();
    let cid2 = back.cid().unwrap();
    assert_eq!(cid1, cid2);
}

#[test]
fn cid_matches_committed_fixture() {
    // The committed hello.cid fixture must match the freshly computed CID.
    // If the fixture is absent (first run), we regenerate it; if it exists
    // it must match byte-for-byte. If the assertion fails, the schema changed
    // in a way that affected the canonical bytes: inspect the diff and either
    // revert the change or regenerate the fixture intentionally.
    let m = WorldManifest::minimal_example();
    let fixture_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("hello.cid");
    let actual = m.cid().unwrap();

    if !fixture_path.exists() {
        std::fs::create_dir_all(fixture_path.parent().unwrap()).unwrap();
        std::fs::write(&fixture_path, actual.to_string().as_bytes()).unwrap();
        return;
    }
    let expected_cid_str = std::fs::read_to_string(&fixture_path).unwrap();
    let expected: Cid = expected_cid_str.trim().parse().unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn cid_changes_when_schema_version_changes() {
    // Tag a hand-built CID with a different schema version; the digest must
    // match bytes (same underlying data) but the string form must differ.
    let bytes = b"hello";
    let cid_v0 = Cid::from_bytes(SchemaVersion::V0, bytes);
    let cid_v1 = Cid::from_bytes(SchemaVersion::V1, bytes);
    assert_eq!(cid_v0.digest, cid_v1.digest);
    assert_ne!(cid_v0.to_string(), cid_v1.to_string());
}

#[test]
fn cid_verify_succeeds_on_match_and_fails_on_drift() {
    let mut m = WorldManifest::minimal_example();
    let cid = m.cid().unwrap();
    m.verify_cid(&cid).unwrap();

    // Mutate → verify must fail.
    m.world_id = "different.world".into();
    let err = m.verify_cid(&cid).unwrap_err();
    assert!(err.to_string().contains("content address mismatch"));
}
