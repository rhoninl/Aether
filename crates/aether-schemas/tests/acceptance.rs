//! Additional acceptance tests backing three frogo backlog tasks:
//!
//! - #68 Entity+Prop+Component — `box_on_plane_yaml_round_trips_with_stable_hash`
//! - #72 Content-addressing — `fuzz_10k_random_manifests_hash_stable`
//! - #75 JSON Schema + examples — `published_examples_parse_against_their_types`

use std::path::PathBuf;

use aether_schemas::{
    from_canonical_bytes, from_yaml_str, to_canonical_bytes, to_yaml_string, ChunkManifest,
    ContentAddress, Entity, Prop, ScriptArtifact, WorldManifest,
};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn workspace_root() -> PathBuf {
    crate_root().parent().unwrap().parent().unwrap().to_path_buf()
}

// --- #68 -------------------------------------------------------------------

#[test]
fn box_on_plane_yaml_round_trips_with_stable_hash() {
    let path = crate_root()
        .join("tests")
        .join("fixtures")
        .join("box_on_plane.yaml");
    let yaml = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!("could not read {}: {}", path.display(), e);
    });

    // YAML -> Rust struct
    let manifest: WorldManifest = from_yaml_str(&yaml)
        .unwrap_or_else(|e| panic!("box_on_plane.yaml failed to parse: {e}"));

    // Fixture exercises Entity + Transform + Component (Mesh + RigidBody).
    assert!(
        manifest.entities.iter().any(|e| e.id == "ground.plane"),
        "expected `ground.plane` entity in fixture"
    );
    assert!(
        manifest.entities.iter().any(|e| e.id == "falling.box"),
        "expected `falling.box` entity in fixture"
    );
    let box_entity = manifest
        .entities
        .iter()
        .find(|e| e.id == "falling.box")
        .unwrap();
    assert!(
        box_entity.components.iter().any(|c| c.ty == "render.mesh"),
        "expected render.mesh component"
    );
    assert!(
        box_entity
            .components
            .iter()
            .any(|c| c.ty == "physics.rigid_body"),
        "expected physics.rigid_body component"
    );

    // YAML -> struct -> YAML -> struct must be lossless.
    let reencoded = to_yaml_string(&manifest).unwrap();
    let re_parsed: WorldManifest = from_yaml_str(&reencoded).unwrap();
    assert_eq!(manifest, re_parsed);

    // CID must be deterministic across three independent computations.
    let cid_a = manifest.cid().unwrap();
    let cid_b = re_parsed.cid().unwrap();
    let from_bytes: WorldManifest = from_canonical_bytes(&to_canonical_bytes(&manifest).unwrap()).unwrap();
    let cid_c = from_bytes.cid().unwrap();
    assert_eq!(cid_a, cid_b, "YAML round-trip must preserve the CID");
    assert_eq!(cid_a, cid_c, "binary round-trip must preserve the CID");
}

// --- #72 -------------------------------------------------------------------

/// Number of random manifests to generate. 10k is the backlog-task bar and
/// still runs in well under a second on a laptop.
const FUZZ_SAMPLE_COUNT: usize = 10_000;

/// A cheap LCG — no external rand dep, fully deterministic per seed.
struct Lcg(u64);
impl Lcg {
    fn new(seed: u64) -> Self {
        Self(seed.wrapping_add(0x9E37_79B9_7F4A_7C15))
    }
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0
    }
    fn next_u32(&mut self) -> u32 {
        (self.next() >> 32) as u32
    }
}

fn synthesize_manifest(i: usize) -> WorldManifest {
    let mut rng = Lcg::new(i as u64);
    let mut m = WorldManifest::minimal_example();
    m.world_id = format!("fuzz.{:06}.aether", rng.next_u32());
    m.display_name = Some(format!("Fuzz World {}", rng.next_u32() % 1_000));
    m.runtime_settings.tick_rate_hz = 30 + (rng.next_u32() % 91); // 30..=120
    m.runtime_settings.max_players = 1 + (rng.next_u32() % 64); // 1..=64
    m.lighting.sun_intensity = (rng.next_u32() % 10_000) as f32 / 10_000.0;
    m.lighting.ambient_intensity = (rng.next_u32() % 10_000) as f32 / 10_000.0;
    m
}

#[test]
fn fuzz_10k_random_manifests_hash_stable() {
    let mut seen_cids = std::collections::HashSet::with_capacity(FUZZ_SAMPLE_COUNT);
    let mut cid_mismatches = Vec::new();
    let mut yaml_binary_drift = 0usize;

    for i in 0..FUZZ_SAMPLE_COUNT {
        let m = synthesize_manifest(i);

        // YAML -> binary -> YAML must preserve canonical bytes.
        let yaml = to_yaml_string(&m).unwrap();
        let reparsed: WorldManifest = from_yaml_str(&yaml).unwrap();
        let bytes_a = to_canonical_bytes(&m).unwrap();
        let bytes_b = to_canonical_bytes(&reparsed).unwrap();
        if bytes_a != bytes_b {
            yaml_binary_drift += 1;
            if cid_mismatches.len() < 3 {
                cid_mismatches.push((i, hex::encode(&bytes_a), hex::encode(&bytes_b)));
            }
            continue;
        }

        // CID must be stable across two independent computations.
        let cid_1 = m.cid().unwrap();
        let cid_2 = reparsed.cid().unwrap();
        assert_eq!(cid_1, cid_2, "cid drift for fuzz iteration {i}");

        seen_cids.insert(cid_1.to_string());
    }

    assert_eq!(
        yaml_binary_drift, 0,
        "YAML↔binary drift across {} samples: {:?}",
        FUZZ_SAMPLE_COUNT, cid_mismatches
    );

    // Sanity: fuzz domain large enough that we're not collapsing everything to
    // a single CID (would indicate our synthesis is degenerate).
    assert!(
        seen_cids.len() >= FUZZ_SAMPLE_COUNT / 2,
        "too few distinct CIDs ({}), fuzz synthesis may be degenerate",
        seen_cids.len()
    );
}

// --- #75 -------------------------------------------------------------------

#[test]
fn published_examples_parse_against_their_types() {
    let examples_dir = workspace_root()
        .join("docs")
        .join("schemas")
        .join("examples");

    let world_yaml = std::fs::read_to_string(examples_dir.join("world-manifest.example.yaml"))
        .expect("world-manifest.example.yaml missing");
    let _: WorldManifest = from_yaml_str(&world_yaml)
        .expect("world-manifest example must parse as WorldManifest");

    let entity_yaml = std::fs::read_to_string(examples_dir.join("entity.example.yaml"))
        .expect("entity.example.yaml missing");
    let _: Entity = serde_yaml::from_str(&entity_yaml)
        .expect("entity example must parse as Entity");

    let prop_yaml = std::fs::read_to_string(examples_dir.join("prop.example.yaml"))
        .expect("prop.example.yaml missing");
    let _: Prop = serde_yaml::from_str(&prop_yaml)
        .expect("prop example must parse as Prop");

    let chunk_yaml = std::fs::read_to_string(examples_dir.join("chunk-manifest.example.yaml"))
        .expect("chunk-manifest.example.yaml missing");
    let _: ChunkManifest = serde_yaml::from_str(&chunk_yaml)
        .expect("chunk-manifest example must parse as ChunkManifest");

    let script_yaml = std::fs::read_to_string(examples_dir.join("script-artifact.example.yaml"))
        .expect("script-artifact.example.yaml missing");
    let _: ScriptArtifact = serde_yaml::from_str(&script_yaml)
        .expect("script-artifact example must parse as ScriptArtifact");
}
