//! v0→v1 migration tests (task 73).

use aether_schemas::migration::apply_chain;
use aether_schemas::{from_yaml_str, Migrator, MigratorV0ToV1, SchemaVersion, WorldManifest};

#[test]
fn v0_document_migrates_to_v1() {
    let v0_doc = serde_json::json!({
        "world_id": "legacy.world",
        "physics": {
            "gravity": -9.81,
            "tick_rate_hz": 60,
            "max_players": 16,
        },
        "lighting": {
            "sun_intensity": 1.0,
            "ambient_intensity": 0.1,
        },
        "spawn_points": [
            {"id": "origin", "position": [0.0, 0.0, 0.0], "yaw_deg": 0.0, "is_default": true}
        ],
        "legacy_flag": true
    });

    let migrated = apply_chain(
        &[&MigratorV0ToV1 as &dyn Migrator],
        v0_doc,
        SchemaVersion::V1,
    )
    .unwrap();

    // schema_version must be injected.
    assert_eq!(migrated["schema_version"], serde_json::json!(1));
    // `physics` must now live under `runtime_settings`.
    assert_eq!(
        migrated["runtime_settings"]["gravity"],
        serde_json::json!(-9.81)
    );
    // Unknowns preserved under migration_notes.
    assert_eq!(
        migrated["migration_notes"]["legacy_flag"],
        serde_json::json!(true)
    );

    // The migrated document must deserialize into the real struct.
    let yaml = serde_yaml::to_string(&migrated).unwrap();
    let manifest: WorldManifest = from_yaml_str(&yaml).unwrap();
    manifest.validate().unwrap();
    assert_eq!(manifest.world_id, "legacy.world");
    assert_eq!(manifest.runtime_settings.gravity, -9.81);
}

#[test]
fn v1_document_passes_through_unchanged() {
    let v1_doc = serde_json::json!({
        "schema_version": 1,
        "world_id": "w",
        "spawn_points": [
            {"id": "origin", "position": [0.0, 0.0, 0.0], "yaw_deg": 0.0, "is_default": true}
        ]
    });
    let out = apply_chain(&[], v1_doc.clone(), SchemaVersion::V1).unwrap();
    assert_eq!(out, v1_doc);
}

#[test]
fn unsupported_version_returns_error() {
    let bad = serde_json::json!({"schema_version": 42, "world_id": "w"});
    let err = apply_chain(&[], bad, SchemaVersion::V1).unwrap_err();
    assert!(err.to_string().contains("unsupported schema version"));
}
