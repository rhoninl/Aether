//! Creator Studio manifest types.
//!
//! The in-memory Rust representation is unchanged (`WorldManifestPatch`,
//! `PhysicsSettingsPatch`, etc.). Any manifest that crosses into Creator
//! Studio from another crate — or leaves it toward `aether-ugc` /
//! `aether-world-runtime` — arrives/departs as canonical bytes via
//! [`CanonicalCodec`]. `WorldManifest::apply_patch` accepts canonical
//! bytes and returns canonical bytes; the Rust struct API (`apply_patch_in_memory`)
//! is preserved for intra-crate use.

use aether_canonical_shim::{
    CanonicalCodec, SchemaError, SpawnPointDef, WorldManifest as WireWorldManifest, WorldStatus,
};
use tracing::trace;

#[derive(Debug, Clone)]
pub struct WorldManifestPatch {
    pub world_id: String,
    pub physics: Option<PhysicsSettingsPatch>,
    pub spawn_points: Vec<SpawnPointEdit>,
    pub props: Vec<PropEdit>,
    pub scripts: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PhysicsSettingsPatch {
    pub gravity: Option<f32>,
    pub tick_rate: Option<u32>,
    pub max_players: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpawnPointEdit {
    pub id: u64,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw_deg: f32,
}

#[derive(Debug, Clone)]
pub struct TerrainEdit {
    pub chunk_id: u64,
    pub height_delta: f32,
    pub texture_weight: f32,
}

#[derive(Debug, Clone)]
pub struct ManifestEdit {
    pub world_id: String,
    pub physics: PhysicsSettingsPatch,
    pub terrain: Vec<TerrainEdit>,
}

#[derive(Debug, Clone)]
pub enum ScriptEdit {
    VisualNode {
        world_id: String,
        node_id: String,
        payload: Vec<u8>,
    },
    Text {
        world_id: String,
        filename: String,
        source: String,
    },
}

#[derive(Debug, Clone)]
pub struct PropEdit {
    pub prop_id: String,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw_deg: f32,
}

/// Creator Studio's in-memory view of a world manifest.
///
/// This mirrors the shape of the canonical `WorldManifest` but stays a
/// plain Rust struct. Crossing the crate boundary goes through
/// [`CanonicalCodec`] on the shim type; the `From` impls keep the
/// conversion surgical.
#[derive(Debug, Clone, PartialEq)]
pub struct WorldManifest {
    pub world_id: String,
    pub slug: String,
    pub name: String,
    pub owner_id: u64,
    pub version: u16,
    pub max_players: u32,
    pub gravity: f32,
    pub tick_rate_hz: u32,
    pub environment_path: String,
    pub terrain_manifest: String,
    pub props_manifest: String,
    pub spawn_points: Vec<SpawnPointEdit>,
}

impl WorldManifest {
    /// Apply a patch to manifest **canonical bytes**, returning fresh
    /// canonical bytes. This is the boundary-facing API per task 74.
    pub fn apply_patch(
        canonical_bytes: &[u8],
        patch: &WorldManifestPatch,
    ) -> Result<Vec<u8>, SchemaError> {
        let span = tracing::trace_span!(
            "WorldManifest::apply_patch",
            world_id = %patch.world_id,
            bytes = canonical_bytes.len()
        );
        let _enter = span.enter();

        let mut wire = WireWorldManifest::from_canonical_bytes(canonical_bytes)?;
        apply_patch_to_wire(&mut wire, patch);
        let out = wire.to_canonical_bytes()?;
        trace!(world_id = %wire.world_id, out_bytes = out.len(), "patched");
        Ok(out)
    }

    /// In-memory patch application for intra-crate use. Keeps the old
    /// struct-based ergonomics for callers that aren't at a boundary yet.
    pub fn apply_patch_in_memory(&mut self, patch: &WorldManifestPatch) {
        if let Some(phys) = &patch.physics {
            if let Some(g) = phys.gravity {
                self.gravity = g;
            }
            if let Some(t) = phys.tick_rate {
                self.tick_rate_hz = t;
            }
            if let Some(m) = phys.max_players {
                self.max_players = m;
            }
        }
        for edit in &patch.spawn_points {
            if let Some(existing) = self.spawn_points.iter_mut().find(|p| p.id == edit.id) {
                *existing = edit.clone();
            } else {
                self.spawn_points.push(edit.clone());
            }
        }
    }
}

impl CanonicalCodec for WorldManifest {
    fn to_canonical_bytes(&self) -> Result<Vec<u8>, SchemaError> {
        let wire = studio_to_wire(self);
        wire.to_canonical_bytes()
    }
    fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, SchemaError> {
        let wire = WireWorldManifest::from_canonical_bytes(bytes)?;
        Ok(wire_to_studio(&wire))
    }
}

fn studio_to_wire(m: &WorldManifest) -> WireWorldManifest {
    WireWorldManifest {
        world_id: m.world_id.clone(),
        slug: m.slug.clone(),
        name: m.name.clone(),
        owner_id: m.owner_id,
        version: m.version,
        status: WorldStatus::Draft,
        max_players: m.max_players,
        gravity: m.gravity,
        tick_rate_hz: m.tick_rate_hz,
        environment_path: m.environment_path.clone(),
        terrain_manifest: m.terrain_manifest.clone(),
        props_manifest: m.props_manifest.clone(),
        spawn_points: m.spawn_points.iter().map(spawn_edit_to_wire).collect(),
        portals: Vec::new(),
        region_preference: Vec::new(),
    }
}

fn wire_to_studio(w: &WireWorldManifest) -> WorldManifest {
    WorldManifest {
        world_id: w.world_id.clone(),
        slug: w.slug.clone(),
        name: w.name.clone(),
        owner_id: w.owner_id,
        version: w.version,
        max_players: w.max_players,
        gravity: w.gravity,
        tick_rate_hz: w.tick_rate_hz,
        environment_path: w.environment_path.clone(),
        terrain_manifest: w.terrain_manifest.clone(),
        props_manifest: w.props_manifest.clone(),
        spawn_points: w.spawn_points.iter().map(spawn_wire_to_edit).collect(),
    }
}

fn spawn_edit_to_wire(e: &SpawnPointEdit) -> SpawnPointDef {
    SpawnPointDef {
        id: e.id,
        x: e.x,
        y: e.y,
        z: e.z,
        yaw_deg: e.yaw_deg,
        is_default: false,
    }
}

fn spawn_wire_to_edit(p: &SpawnPointDef) -> SpawnPointEdit {
    SpawnPointEdit {
        id: p.id,
        x: p.x,
        y: p.y,
        z: p.z,
        yaw_deg: p.yaw_deg,
    }
}

fn apply_patch_to_wire(wire: &mut WireWorldManifest, patch: &WorldManifestPatch) {
    if let Some(phys) = &patch.physics {
        if let Some(g) = phys.gravity {
            wire.gravity = g;
        }
        if let Some(t) = phys.tick_rate {
            wire.tick_rate_hz = t;
        }
        if let Some(m) = phys.max_players {
            wire.max_players = m;
        }
    }
    for edit in &patch.spawn_points {
        if let Some(existing) = wire.spawn_points.iter_mut().find(|p| p.id == edit.id) {
            existing.x = edit.x;
            existing.y = edit.y;
            existing.z = edit.z;
            existing.yaw_deg = edit.yaw_deg;
        } else {
            wire.spawn_points.push(spawn_edit_to_wire(edit));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> WorldManifest {
        WorldManifest {
            world_id: "w-1".into(),
            slug: "hello".into(),
            name: "Hello".into(),
            owner_id: 42,
            version: 1,
            max_players: 16,
            gravity: -9.8,
            tick_rate_hz: 60,
            environment_path: "env".into(),
            terrain_manifest: "t".into(),
            props_manifest: "p".into(),
            spawn_points: vec![SpawnPointEdit {
                id: 1,
                x: 0.0,
                y: 0.0,
                z: 0.0,
                yaw_deg: 0.0,
            }],
        }
    }

    #[test]
    fn canonical_roundtrip() {
        let m = sample();
        let bytes = m.to_canonical_bytes().unwrap();
        let back = WorldManifest::from_canonical_bytes(&bytes).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn apply_patch_on_canonical_bytes() {
        let m = sample();
        let bytes = m.to_canonical_bytes().unwrap();
        let patch = WorldManifestPatch {
            world_id: "w-1".into(),
            physics: Some(PhysicsSettingsPatch {
                gravity: Some(-5.0),
                tick_rate: Some(30),
                max_players: Some(64),
            }),
            spawn_points: vec![SpawnPointEdit {
                id: 2,
                x: 1.0,
                y: 2.0,
                z: 3.0,
                yaw_deg: 90.0,
            }],
            props: vec![],
            scripts: vec![],
        };
        let out = WorldManifest::apply_patch(&bytes, &patch).unwrap();
        let m2 = WorldManifest::from_canonical_bytes(&out).unwrap();
        assert_eq!(m2.gravity, -5.0);
        assert_eq!(m2.tick_rate_hz, 30);
        assert_eq!(m2.max_players, 64);
        assert_eq!(m2.spawn_points.len(), 2);
    }

    #[test]
    fn apply_patch_in_memory_matches_canonical_path() {
        let mut m = sample();
        let patch = WorldManifestPatch {
            world_id: "w-1".into(),
            physics: Some(PhysicsSettingsPatch {
                gravity: Some(-1.0),
                tick_rate: None,
                max_players: None,
            }),
            spawn_points: vec![],
            props: vec![],
            scripts: vec![],
        };
        let bytes = m.to_canonical_bytes().unwrap();
        let out = WorldManifest::apply_patch(&bytes, &patch).unwrap();
        let via_canonical = WorldManifest::from_canonical_bytes(&out).unwrap();
        m.apply_patch_in_memory(&patch);
        assert_eq!(m, via_canonical);
    }
}
