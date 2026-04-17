//! Canonical wire boundary for `aether-world-runtime`.
//!
//! Rust in-memory types (`WorldRuntimeManifest`, `ChunkDescriptor`, portal
//! defs) are unchanged. Anything crossing a crate boundary — disk, network,
//! another crate — flows as canonical bytes via [`CanonicalCodec`].
//!
//! After U03 lands, swap `aether_canonical_shim` for `aether_schemas` here
//! and delete the `convert::*` helpers that only exist for the shim types.

use aether_canonical_shim::{
    CanonicalCodec, ChunkManifest as WireChunkManifest, ChunkRef, Cid, ContentAddress, PortalDef,
    PortalScheme as WirePortalScheme, SchemaError, SpawnPointDef, WorldManifest as WireWorldManifest,
    WorldStatus as WireWorldStatus,
};
use tracing::trace;

use crate::chunking::{ChunkDescriptor, ChunkKind};
use crate::manifest::WorldRuntimeManifest;
use crate::props::SpawnPoint;

// Re-export the canonical trait & types used at the boundary.
pub use aether_canonical_shim::CanonicalCodec as BoundaryCodec;

/// Encode a `WorldRuntimeManifest` as canonical bytes for wire/disk.
pub fn encode_world_runtime_manifest(
    manifest: &WorldRuntimeManifest,
    spawn_points: &[SpawnPoint],
) -> Result<Vec<u8>, SchemaError> {
    let span = tracing::trace_span!("encode_world_runtime_manifest", world_id = %manifest.world_id);
    let _enter = span.enter();
    let wire = runtime_to_wire(manifest, spawn_points);
    wire.to_canonical_bytes()
}

/// Decode canonical bytes back into the in-memory runtime types.
pub fn decode_world_runtime_manifest(
    bytes: &[u8],
) -> Result<(WorldRuntimeManifest, Vec<SpawnPoint>), SchemaError> {
    let span = tracing::trace_span!("decode_world_runtime_manifest", bytes = bytes.len());
    let _enter = span.enter();
    let wire = WireWorldManifest::from_canonical_bytes(bytes)?;
    Ok(wire_to_runtime(&wire))
}

/// CID for a runtime manifest — stable across encodes of the same in-memory value.
pub fn world_runtime_manifest_cid(
    manifest: &WorldRuntimeManifest,
    spawn_points: &[SpawnPoint],
) -> Result<Cid, SchemaError> {
    let wire = runtime_to_wire(manifest, spawn_points);
    Ok(wire.cid())
}

/// Encode a chunk manifest (list of `ChunkDescriptor`s) as canonical bytes.
pub fn encode_chunk_manifest(
    world_id: &str,
    chunks: &[ChunkDescriptor],
) -> Result<Vec<u8>, SchemaError> {
    let span = tracing::trace_span!("encode_chunk_manifest", world_id, count = chunks.len());
    let _enter = span.enter();
    let wire = WireChunkManifest {
        world_id: world_id.to_string(),
        chunks: chunks.iter().map(chunk_to_wire).collect(),
    };
    wire.to_canonical_bytes()
}

/// Decode canonical chunk-manifest bytes into `ChunkDescriptor`s.
pub fn decode_chunk_manifest(bytes: &[u8]) -> Result<(String, Vec<ChunkDescriptor>), SchemaError> {
    let span = tracing::trace_span!("decode_chunk_manifest", bytes = bytes.len());
    let _enter = span.enter();
    let wire = WireChunkManifest::from_canonical_bytes(bytes)?;
    let chunks = wire
        .chunks
        .iter()
        .map(|c| wire_to_chunk(&wire.world_id, c))
        .collect();
    Ok((wire.world_id, chunks))
}

// Portal defs are a direct passthrough — we don't own the in-memory portal
// type here (the registry does), but world-runtime needs to serialize them
// in world manifests. Expose the shim type at the boundary.
pub use aether_canonical_shim::PortalDef as CanonicalPortalDef;

// ---- conversions ------------------------------------------------------------

fn runtime_to_wire(m: &WorldRuntimeManifest, spawn_points: &[SpawnPoint]) -> WireWorldManifest {
    WireWorldManifest {
        world_id: m.world_id.clone(),
        slug: m.world_id.clone(),
        name: m.display_name.clone(),
        owner_id: 0,
        version: 1,
        status: WireWorldStatus::Published,
        max_players: m.max_players,
        gravity: m.gravity,
        tick_rate_hz: m.tick_rate_hz,
        environment_path: m.environment_path.clone(),
        terrain_manifest: m.terrain_manifest.clone(),
        props_manifest: m.props_manifest.clone(),
        spawn_points: spawn_points.iter().map(spawn_to_wire).collect(),
        portals: Vec::new(),
        region_preference: Vec::new(),
    }
}

fn wire_to_runtime(w: &WireWorldManifest) -> (WorldRuntimeManifest, Vec<SpawnPoint>) {
    let manifest = WorldRuntimeManifest {
        world_id: w.world_id.clone(),
        display_name: w.name.clone(),
        gravity: w.gravity,
        tick_rate_hz: w.tick_rate_hz,
        max_players: w.max_players,
        environment_path: w.environment_path.clone(),
        terrain_manifest: w.terrain_manifest.clone(),
        props_manifest: w.props_manifest.clone(),
        spawn_points: w.spawn_points.len() as u32,
    };
    let spawn_points = w.spawn_points.iter().map(wire_to_spawn).collect();
    trace!(world_id = %manifest.world_id, spawn_count = manifest.spawn_points, "wire→runtime");
    (manifest, spawn_points)
}

fn spawn_to_wire(p: &SpawnPoint) -> SpawnPointDef {
    SpawnPointDef {
        id: p.id,
        x: p.x,
        y: p.y,
        z: p.z,
        yaw_deg: p.yaw_deg,
        is_default: p.is_default,
    }
}

fn wire_to_spawn(p: &SpawnPointDef) -> SpawnPoint {
    SpawnPoint {
        id: p.id,
        x: p.x,
        y: p.y,
        z: p.z,
        yaw_deg: p.yaw_deg,
        is_default: p.is_default,
    }
}

fn chunk_to_wire(c: &ChunkDescriptor) -> ChunkRef {
    let kind = match c.kind {
        ChunkKind::Terrain => 0,
        ChunkKind::PropMesh => 1,
        ChunkKind::Lighting => 2,
    };
    // Reuse the existing checksum string as the content CID payload. We
    // prefix it with "sha256:" if it looks like raw hex so the CID format
    // is consistent.
    let cid_str = if c.checksum_sha256.starts_with("sha256:") {
        c.checksum_sha256.clone()
    } else {
        format!("sha256:{}", c.checksum_sha256)
    };
    ChunkRef {
        chunk_id: c.chunk_id,
        kind,
        lod: c.lod,
        path: c.path.clone(),
        size_bytes: c.size_bytes,
        content: ContentAddress::new(Cid::from_string(cid_str), c.size_bytes),
    }
}

fn wire_to_chunk(world_id: &str, c: &ChunkRef) -> ChunkDescriptor {
    let kind = match c.kind {
        0 => ChunkKind::Terrain,
        1 => ChunkKind::PropMesh,
        _ => ChunkKind::Lighting,
    };
    ChunkDescriptor {
        world_id: world_id.to_string(),
        chunk_id: c.chunk_id,
        kind,
        lod: c.lod,
        path: c.path.clone(),
        size_bytes: c.size_bytes,
        checksum_sha256: c.content.cid.as_str().to_string(),
    }
}

/// Build a canonical portal definition from (scheme, target, region, fallback).
/// Kept here so world manifests can embed portals without creator-studio
/// or registry needing to know the shim type.
pub fn portal_def(
    scheme: CanonicalPortalScheme,
    target: impl Into<String>,
    region: impl Into<String>,
    fallback: Option<String>,
) -> PortalDef {
    PortalDef {
        scheme: scheme.into(),
        target: target.into(),
        region: region.into(),
        fallback,
    }
}

/// Public enum mirroring the shim's portal scheme, re-exported here so
/// downstream crates don't need to depend on the shim directly if all they
/// need is the wire boundary through world-runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanonicalPortalScheme {
    Aether,
    Https,
    StaticWorld,
}

impl From<CanonicalPortalScheme> for WirePortalScheme {
    fn from(v: CanonicalPortalScheme) -> Self {
        match v {
            CanonicalPortalScheme::Aether => WirePortalScheme::Aether,
            CanonicalPortalScheme::Https => WirePortalScheme::Https,
            CanonicalPortalScheme::StaticWorld => WirePortalScheme::StaticWorld,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunking::ChunkKind;

    fn sample_manifest() -> (WorldRuntimeManifest, Vec<SpawnPoint>) {
        (
            WorldRuntimeManifest {
                world_id: "w-1".into(),
                display_name: "W One".into(),
                gravity: -9.8,
                tick_rate_hz: 60,
                max_players: 16,
                environment_path: "env/day".into(),
                terrain_manifest: "terrain.man".into(),
                props_manifest: "props.man".into(),
                spawn_points: 1,
            },
            vec![SpawnPoint {
                id: 1,
                x: 0.0,
                y: 0.0,
                z: 0.0,
                yaw_deg: 0.0,
                is_default: true,
            }],
        )
    }

    #[test]
    fn world_runtime_manifest_roundtrips_through_canonical_bytes() {
        let (m, sp) = sample_manifest();
        let bytes = encode_world_runtime_manifest(&m, &sp).unwrap();
        let (m2, sp2) = decode_world_runtime_manifest(&bytes).unwrap();
        assert_eq!(m.world_id, m2.world_id);
        assert_eq!(m.tick_rate_hz, m2.tick_rate_hz);
        assert_eq!(m.max_players, m2.max_players);
        assert_eq!(sp.len(), sp2.len());
        assert_eq!(sp[0].id, sp2[0].id);
    }

    #[test]
    fn world_runtime_manifest_cid_stable() {
        let (m, sp) = sample_manifest();
        let c1 = world_runtime_manifest_cid(&m, &sp).unwrap();
        let c2 = world_runtime_manifest_cid(&m, &sp).unwrap();
        assert_eq!(c1, c2);
    }

    #[test]
    fn chunk_manifest_roundtrips() {
        let chunks = vec![ChunkDescriptor {
            world_id: "w-1".into(),
            chunk_id: 42,
            kind: ChunkKind::Terrain,
            lod: 2,
            path: "chunks/42.bin".into(),
            size_bytes: 1024,
            checksum_sha256: "deadbeef".into(),
        }];
        let bytes = encode_chunk_manifest("w-1", &chunks).unwrap();
        let (world_id, back) = decode_chunk_manifest(&bytes).unwrap();
        assert_eq!(world_id, "w-1");
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].chunk_id, 42);
        assert!(back[0].checksum_sha256.starts_with("sha256:"));
    }
}
