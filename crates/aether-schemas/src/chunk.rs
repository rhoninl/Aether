//! Chunk streaming unit (task 69).
//!
//! Aligns with `aether_world_runtime::ChunkDescriptor` but is the declarative
//! source of truth. Chunks are the unit of streaming, lighting bake, and LOD
//! selection, so they must be content-addressed and deterministically ordered.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{SchemaError, SchemaResult};

/// Integer grid coordinate identifying a chunk.
///
/// A world is laid out on a 3-dimensional integer grid with configurable
/// cell size (see [`ChunkManifest::cell_size_meters`]). Two chunks with the
/// same `ChunkCoord` collide; the manifest must enforce uniqueness.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct ChunkCoord {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl ChunkCoord {
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        ChunkCoord { x, y, z }
    }
}

/// LOD level; 0 is highest fidelity, higher values are coarser meshes.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct LodLevel(pub u8);

impl LodLevel {
    pub const MAX: u8 = 4;

    pub fn validate(&self, pointer_base: &str) -> SchemaResult<()> {
        if self.0 > Self::MAX {
            return Err(SchemaError::validation(
                format!("{pointer_base}/lod"),
                format!("LOD must be in [0, {}]", Self::MAX),
                format!("set LOD to an integer in [0, {}]", Self::MAX),
            ));
        }
        Ok(())
    }
}

impl Default for LodLevel {
    fn default() -> Self {
        LodLevel(0)
    }
}

/// Declarative chunk manifest. A world's chunk set is the list of these
/// manifests. Each manifest points to a content-addressed asset blob via
/// `asset_cid`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChunkManifest {
    pub coord: ChunkCoord,

    /// Semantic classification; controls which runtime pipeline consumes the
    /// chunk.
    pub kind: ChunkKind,

    /// LOD level represented by this manifest. Multiple manifests may share a
    /// `coord` if they differ in `lod`.
    #[serde(default)]
    pub lod: LodLevel,

    /// Content address of the backing asset blob (mesh, lightmap, etc.).
    pub asset_cid: String,

    /// Byte size of the backing asset (hint for streaming scheduler).
    pub size_bytes: u64,

    /// Minimum world-space distance at which the chunk can be prefetched.
    #[serde(default)]
    pub prefetch_distance_meters: f32,

    /// Free-form tags for authoring and agent filtering.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

/// Streaming classification. Mirrors `aether_world_runtime::ChunkKind`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ChunkKind {
    Terrain,
    PropMesh,
    Lighting,
    Audio,
    Navigation,
}

impl ChunkManifest {
    /// Validate schema-level invariants on a chunk manifest.
    pub fn validate(&self, pointer_base: &str) -> SchemaResult<()> {
        self.lod.validate(pointer_base)?;
        if self.asset_cid.trim().is_empty() {
            return Err(SchemaError::validation(
                format!("{pointer_base}/asset_cid"),
                "asset_cid must be non-empty",
                "produce the asset via the pipeline, record its CID here",
            ));
        }
        if self.size_bytes == 0 {
            return Err(SchemaError::validation(
                format!("{pointer_base}/size_bytes"),
                "size_bytes must be > 0",
                "set to the byte size of the blob at asset_cid",
            ));
        }
        if !self.prefetch_distance_meters.is_finite() || self.prefetch_distance_meters < 0.0 {
            return Err(SchemaError::validation(
                format!("{pointer_base}/prefetch_distance_meters"),
                "prefetch_distance_meters must be a finite, non-negative float",
                "set to a value like 128.0",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_coord_is_total_ordered() {
        let a = ChunkCoord::new(0, 0, 0);
        let b = ChunkCoord::new(0, 0, 1);
        let c = ChunkCoord::new(0, 1, 0);
        let d = ChunkCoord::new(1, 0, 0);
        let mut v = vec![d, c, b, a];
        v.sort();
        assert_eq!(v, vec![a, b, c, d]);
    }

    #[test]
    fn lod_validates_upper_bound() {
        assert!(LodLevel(0).validate("/c").is_ok());
        assert!(LodLevel(LodLevel::MAX).validate("/c").is_ok());
        assert!(LodLevel(99).validate("/c").is_err());
    }

    #[test]
    fn chunk_manifest_validate_happy_path() {
        let m = ChunkManifest {
            coord: ChunkCoord::new(0, 0, 0),
            kind: ChunkKind::Terrain,
            lod: LodLevel(0),
            asset_cid: "cid:v1:abc".into(),
            size_bytes: 4096,
            prefetch_distance_meters: 128.0,
            tags: vec![],
        };
        m.validate("/chunks/0").unwrap();
    }

    #[test]
    fn chunk_manifest_rejects_zero_size() {
        let m = ChunkManifest {
            coord: ChunkCoord::new(0, 0, 0),
            kind: ChunkKind::Terrain,
            lod: LodLevel(0),
            asset_cid: "cid:v1:abc".into(),
            size_bytes: 0,
            prefetch_distance_meters: 0.0,
            tags: vec![],
        };
        let err = m.validate("/chunks/0").unwrap_err();
        assert_eq!(err.pointer(), "/chunks/0/size_bytes");
    }

    #[test]
    fn chunk_manifest_roundtrip() {
        let m = ChunkManifest {
            coord: ChunkCoord::new(1, 2, 3),
            kind: ChunkKind::PropMesh,
            lod: LodLevel(2),
            asset_cid: "cid:v1:abc".into(),
            size_bytes: 1024,
            prefetch_distance_meters: 64.0,
            tags: vec!["forest".into()],
        };
        let yaml = serde_yaml::to_string(&m).unwrap();
        let back: ChunkManifest = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(m, back);
    }
}
