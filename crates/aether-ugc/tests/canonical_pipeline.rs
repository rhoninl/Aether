//! Task 74 acceptance criterion:
//! Upload → Scan → Approve → Publish operates over content-addressed
//! canonical artifacts end-to-end, and the CID is stable across the
//! pipeline.

use aether_canonical_shim::{
    ArtifactEnvelope, ArtifactKind, CanonicalCodec, Cid, PortalDef, PortalScheme, SpawnPointDef,
    WorldManifest, WorldStatus,
};
use aether_ugc::{CanonicalPipelineState, CanonicalUgcPipeline};

fn fixture_world_manifest() -> WorldManifest {
    WorldManifest {
        world_id: "world-canonical-fixture".into(),
        slug: "canonical-fixture".into(),
        name: "Canonical Fixture".into(),
        owner_id: 7,
        version: 1,
        status: WorldStatus::Draft,
        max_players: 24,
        gravity: -9.8,
        tick_rate_hz: 60,
        environment_path: "env/day.env".into(),
        terrain_manifest: "terrain.man".into(),
        props_manifest: "props.man".into(),
        spawn_points: vec![SpawnPointDef {
            id: 1,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            yaw_deg: 0.0,
            is_default: true,
        }],
        portals: vec![PortalDef {
            scheme: PortalScheme::Aether,
            target: "other-world".into(),
            region: "us-west".into(),
            fallback: None,
        }],
        region_preference: vec!["us-west".into()],
    }
}

#[test]
fn upload_scan_approve_publish_stable_cid() {
    let manifest = fixture_world_manifest();
    let envelope = ArtifactEnvelope::wrap(ArtifactKind::WorldManifest, &manifest).unwrap();
    let envelope_bytes = envelope.to_canonical_bytes().unwrap();

    // Independent CID computed up-front.
    let expected_cid = Cid::sha256_of(&envelope_bytes);

    let mut pipe = CanonicalUgcPipeline::new();

    // Upload
    let cid = pipe.upload(&envelope_bytes).unwrap();
    assert_eq!(cid, expected_cid, "CID assigned on upload");
    assert_eq!(
        pipe.get(&cid).unwrap().state,
        CanonicalPipelineState::Uploaded
    );

    // Scan
    pipe.scan(&cid).unwrap();
    assert_eq!(
        pipe.get(&cid).unwrap().state,
        CanonicalPipelineState::Scanning
    );
    assert_eq!(&pipe.get(&cid).unwrap().cid, &expected_cid, "CID stable after scan");

    // Approve
    pipe.approve(&cid).unwrap();
    assert_eq!(
        pipe.get(&cid).unwrap().state,
        CanonicalPipelineState::Approved
    );
    assert_eq!(
        &pipe.get(&cid).unwrap().cid,
        &expected_cid,
        "CID stable after approve"
    );

    // Publish — returns the ContentAddress keyed by CID; that's what the
    // registry will pivot on (see `aether-registry::discovery`).
    let addr = pipe.publish(&cid).unwrap();
    assert_eq!(addr.cid, expected_cid, "CID stable after publish");
    assert_eq!(
        pipe.get(&cid).unwrap().state,
        CanonicalPipelineState::Published
    );

    // Round-trip the body: decoding from the stored canonical bytes must
    // reconstruct the exact original manifest.
    let body = pipe.body(&cid).expect("pipeline keeps canonical body");
    let envelope_back = ArtifactEnvelope::from_canonical_bytes(body).unwrap();
    assert!(matches!(envelope_back.kind, ArtifactKind::WorldManifest));
    let manifest_back = WorldManifest::from_canonical_bytes(&envelope_back.body).unwrap();
    assert_eq!(manifest_back, manifest);
}

#[test]
fn upload_rejects_non_canonical_bytes() {
    let mut pipe = CanonicalUgcPipeline::new();
    let err = pipe.upload(b"not canonical at all").unwrap_err();
    // Upload failures surface as SchemaError, NOT an internal pipeline error.
    match err {
        aether_ugc::CanonicalPipelineError::Schema(_) => {}
        other => panic!("expected SchemaError, got {other:?}"),
    }
}

#[test]
fn identical_manifests_share_cid() {
    let m = fixture_world_manifest();
    let env1 = ArtifactEnvelope::wrap(ArtifactKind::WorldManifest, &m)
        .unwrap()
        .to_canonical_bytes()
        .unwrap();
    let env2 = ArtifactEnvelope::wrap(ArtifactKind::WorldManifest, &m.clone())
        .unwrap()
        .to_canonical_bytes()
        .unwrap();
    assert_eq!(Cid::sha256_of(&env1), Cid::sha256_of(&env2));
}
