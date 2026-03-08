//! World chunk manifest: chunk references, portal definitions, and boundary stitching.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::coord::{ChunkCoord, ChunkId};

/// Maximum number of LOD levels supported.
const MAX_LOD_LEVELS: u8 = 8;

/// A reference to a chunk within the world manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkReference {
    pub id: ChunkId,
    pub coord: ChunkCoord,
    pub asset_path: String,
    /// Available LOD levels (0 = highest detail).
    pub available_lods: Vec<u8>,
    /// Estimated size in bytes per LOD level.
    pub size_per_lod: Vec<u64>,
    /// Human-readable label (optional).
    pub label: String,
}

/// The axis-aligned face between two adjacent chunks where a portal can exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PortalFace {
    PositiveX,
    NegativeX,
    PositiveY,
    NegativeY,
    PositiveZ,
    NegativeZ,
}

impl PortalFace {
    /// Get the opposite face.
    pub fn opposite(&self) -> Self {
        match self {
            PortalFace::PositiveX => PortalFace::NegativeX,
            PortalFace::NegativeX => PortalFace::PositiveX,
            PortalFace::PositiveY => PortalFace::NegativeY,
            PortalFace::NegativeY => PortalFace::PositiveY,
            PortalFace::PositiveZ => PortalFace::NegativeZ,
            PortalFace::NegativeZ => PortalFace::PositiveZ,
        }
    }
}

/// A portal connecting two chunks, used for occlusion-based streaming decisions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortalDefinition {
    pub from_chunk: ChunkId,
    pub to_chunk: ChunkId,
    pub face: PortalFace,
    /// Whether this portal is open (visible) by default.
    pub default_open: bool,
}

/// Metadata about how two adjacent chunk boundaries should be stitched.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundaryStitch {
    pub chunk_a: ChunkId,
    pub chunk_b: ChunkId,
    pub face: PortalFace,
    /// Vertex indices from chunk_a that connect to chunk_b.
    pub edge_vertex_count: u32,
    /// Whether the stitch requires LOD matching between the two chunks.
    pub requires_lod_match: bool,
}

/// Errors from manifest validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChunkManifestError {
    /// No chunks defined in the manifest.
    EmptyManifest,
    /// Duplicate chunk ID found.
    DuplicateChunkId(ChunkId),
    /// Portal references a chunk that is not in the manifest.
    PortalReferencesUnknownChunk(ChunkId),
    /// Stitch references a chunk that is not in the manifest.
    StitchReferencesUnknownChunk(ChunkId),
    /// LOD level exceeds maximum.
    LodLevelExceedsMax { chunk_id: ChunkId, lod: u8 },
    /// Mismatched size_per_lod and available_lods lengths.
    LodSizeMismatch { chunk_id: ChunkId },
}

/// The world chunk manifest describing all chunks, portals, and stitching info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkManifest {
    pub world_id: String,
    pub chunk_size: f32,
    pub chunks: Vec<ChunkReference>,
    pub portals: Vec<PortalDefinition>,
    pub stitches: Vec<BoundaryStitch>,
}

impl ChunkManifest {
    pub fn new(world_id: String, chunk_size: f32) -> Self {
        Self {
            world_id,
            chunk_size: if chunk_size <= 0.0 { super::coord::DEFAULT_CHUNK_SIZE } else { chunk_size },
            chunks: Vec::new(),
            portals: Vec::new(),
            stitches: Vec::new(),
        }
    }

    /// Add a chunk reference to the manifest.
    pub fn add_chunk(&mut self, reference: ChunkReference) {
        self.chunks.push(reference);
    }

    /// Add a portal definition.
    pub fn add_portal(&mut self, portal: PortalDefinition) {
        self.portals.push(portal);
    }

    /// Add a boundary stitch definition.
    pub fn add_stitch(&mut self, stitch: BoundaryStitch) {
        self.stitches.push(stitch);
    }

    /// Build a lookup map from ChunkId to ChunkReference.
    pub fn chunk_map(&self) -> HashMap<ChunkId, &ChunkReference> {
        self.chunks.iter().map(|c| (c.id, c)).collect()
    }

    /// Build a lookup map from ChunkCoord to ChunkReference.
    pub fn coord_map(&self) -> HashMap<ChunkCoord, &ChunkReference> {
        self.chunks.iter().map(|c| (c.coord, c)).collect()
    }

    /// Get portals originating from a specific chunk.
    pub fn portals_from(&self, chunk_id: ChunkId) -> Vec<&PortalDefinition> {
        self.portals
            .iter()
            .filter(|p| p.from_chunk == chunk_id)
            .collect()
    }

    /// Get stitches involving a specific chunk.
    pub fn stitches_for(&self, chunk_id: ChunkId) -> Vec<&BoundaryStitch> {
        self.stitches
            .iter()
            .filter(|s| s.chunk_a == chunk_id || s.chunk_b == chunk_id)
            .collect()
    }

    /// Validate the manifest for consistency.
    pub fn validate(&self) -> Result<(), ChunkManifestError> {
        if self.chunks.is_empty() {
            return Err(ChunkManifestError::EmptyManifest);
        }

        let mut seen_ids = HashMap::new();
        for chunk in &self.chunks {
            if seen_ids.contains_key(&chunk.id) {
                return Err(ChunkManifestError::DuplicateChunkId(chunk.id));
            }
            seen_ids.insert(chunk.id, true);

            // Validate LOD levels
            for &lod in &chunk.available_lods {
                if lod >= MAX_LOD_LEVELS {
                    return Err(ChunkManifestError::LodLevelExceedsMax {
                        chunk_id: chunk.id,
                        lod,
                    });
                }
            }

            // Validate size_per_lod matches available_lods
            if !chunk.size_per_lod.is_empty()
                && chunk.size_per_lod.len() != chunk.available_lods.len()
            {
                return Err(ChunkManifestError::LodSizeMismatch {
                    chunk_id: chunk.id,
                });
            }
        }

        // Validate portal references
        for portal in &self.portals {
            if !seen_ids.contains_key(&portal.from_chunk) {
                return Err(ChunkManifestError::PortalReferencesUnknownChunk(
                    portal.from_chunk,
                ));
            }
            if !seen_ids.contains_key(&portal.to_chunk) {
                return Err(ChunkManifestError::PortalReferencesUnknownChunk(
                    portal.to_chunk,
                ));
            }
        }

        // Validate stitch references
        for stitch in &self.stitches {
            if !seen_ids.contains_key(&stitch.chunk_a) {
                return Err(ChunkManifestError::StitchReferencesUnknownChunk(
                    stitch.chunk_a,
                ));
            }
            if !seen_ids.contains_key(&stitch.chunk_b) {
                return Err(ChunkManifestError::StitchReferencesUnknownChunk(
                    stitch.chunk_b,
                ));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_chunk_ref(id: u64, x: i32, y: i32, z: i32) -> ChunkReference {
        ChunkReference {
            id: ChunkId(id),
            coord: ChunkCoord::new(x, y, z),
            asset_path: format!("terrain/chunk_{x}_{y}_{z}.bin"),
            available_lods: vec![0, 1, 2],
            size_per_lod: vec![1024, 512, 256],
            label: String::new(),
        }
    }

    fn make_valid_manifest() -> ChunkManifest {
        let mut manifest = ChunkManifest::new("test-world".to_string(), 64.0);
        manifest.add_chunk(make_chunk_ref(1, 0, 0, 0));
        manifest.add_chunk(make_chunk_ref(2, 1, 0, 0));
        manifest.add_chunk(make_chunk_ref(3, 0, 1, 0));
        manifest
    }

    #[test]
    fn test_new_manifest() {
        let manifest = ChunkManifest::new("world-1".to_string(), 64.0);
        assert_eq!(manifest.world_id, "world-1");
        assert_eq!(manifest.chunk_size, 64.0);
        assert!(manifest.chunks.is_empty());
        assert!(manifest.portals.is_empty());
        assert!(manifest.stitches.is_empty());
    }

    #[test]
    fn test_new_manifest_zero_size_uses_default() {
        let manifest = ChunkManifest::new("world".to_string(), 0.0);
        assert_eq!(manifest.chunk_size, super::super::coord::DEFAULT_CHUNK_SIZE);
    }

    #[test]
    fn test_new_manifest_negative_size_uses_default() {
        let manifest = ChunkManifest::new("world".to_string(), -10.0);
        assert_eq!(manifest.chunk_size, super::super::coord::DEFAULT_CHUNK_SIZE);
    }

    #[test]
    fn test_add_chunk() {
        let mut manifest = ChunkManifest::new("world".to_string(), 64.0);
        manifest.add_chunk(make_chunk_ref(1, 0, 0, 0));
        assert_eq!(manifest.chunks.len(), 1);
        assert_eq!(manifest.chunks[0].id, ChunkId(1));
    }

    #[test]
    fn test_add_portal() {
        let mut manifest = make_valid_manifest();
        manifest.add_portal(PortalDefinition {
            from_chunk: ChunkId(1),
            to_chunk: ChunkId(2),
            face: PortalFace::PositiveX,
            default_open: true,
        });
        assert_eq!(manifest.portals.len(), 1);
    }

    #[test]
    fn test_add_stitch() {
        let mut manifest = make_valid_manifest();
        manifest.add_stitch(BoundaryStitch {
            chunk_a: ChunkId(1),
            chunk_b: ChunkId(2),
            face: PortalFace::PositiveX,
            edge_vertex_count: 16,
            requires_lod_match: true,
        });
        assert_eq!(manifest.stitches.len(), 1);
    }

    #[test]
    fn test_chunk_map() {
        let manifest = make_valid_manifest();
        let map = manifest.chunk_map();
        assert_eq!(map.len(), 3);
        assert!(map.contains_key(&ChunkId(1)));
        assert!(map.contains_key(&ChunkId(2)));
        assert!(map.contains_key(&ChunkId(3)));
    }

    #[test]
    fn test_coord_map() {
        let manifest = make_valid_manifest();
        let map = manifest.coord_map();
        assert_eq!(map.len(), 3);
        assert!(map.contains_key(&ChunkCoord::new(0, 0, 0)));
        assert!(map.contains_key(&ChunkCoord::new(1, 0, 0)));
        assert!(map.contains_key(&ChunkCoord::new(0, 1, 0)));
    }

    #[test]
    fn test_portals_from() {
        let mut manifest = make_valid_manifest();
        manifest.add_portal(PortalDefinition {
            from_chunk: ChunkId(1),
            to_chunk: ChunkId(2),
            face: PortalFace::PositiveX,
            default_open: true,
        });
        manifest.add_portal(PortalDefinition {
            from_chunk: ChunkId(1),
            to_chunk: ChunkId(3),
            face: PortalFace::PositiveY,
            default_open: false,
        });
        manifest.add_portal(PortalDefinition {
            from_chunk: ChunkId(2),
            to_chunk: ChunkId(3),
            face: PortalFace::PositiveY,
            default_open: true,
        });

        let from_1 = manifest.portals_from(ChunkId(1));
        assert_eq!(from_1.len(), 2);

        let from_2 = manifest.portals_from(ChunkId(2));
        assert_eq!(from_2.len(), 1);

        let from_3 = manifest.portals_from(ChunkId(3));
        assert_eq!(from_3.len(), 0);
    }

    #[test]
    fn test_stitches_for() {
        let mut manifest = make_valid_manifest();
        manifest.add_stitch(BoundaryStitch {
            chunk_a: ChunkId(1),
            chunk_b: ChunkId(2),
            face: PortalFace::PositiveX,
            edge_vertex_count: 16,
            requires_lod_match: true,
        });
        manifest.add_stitch(BoundaryStitch {
            chunk_a: ChunkId(2),
            chunk_b: ChunkId(3),
            face: PortalFace::PositiveY,
            edge_vertex_count: 8,
            requires_lod_match: false,
        });

        let for_1 = manifest.stitches_for(ChunkId(1));
        assert_eq!(for_1.len(), 1);

        let for_2 = manifest.stitches_for(ChunkId(2));
        assert_eq!(for_2.len(), 2);

        let for_3 = manifest.stitches_for(ChunkId(3));
        assert_eq!(for_3.len(), 1);
    }

    #[test]
    fn test_validate_valid_manifest() {
        let manifest = make_valid_manifest();
        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn test_validate_empty_manifest() {
        let manifest = ChunkManifest::new("world".to_string(), 64.0);
        assert_eq!(manifest.validate(), Err(ChunkManifestError::EmptyManifest));
    }

    #[test]
    fn test_validate_duplicate_chunk_id() {
        let mut manifest = ChunkManifest::new("world".to_string(), 64.0);
        manifest.add_chunk(make_chunk_ref(1, 0, 0, 0));
        manifest.add_chunk(make_chunk_ref(1, 1, 0, 0)); // Same ID, different coord
        assert_eq!(
            manifest.validate(),
            Err(ChunkManifestError::DuplicateChunkId(ChunkId(1)))
        );
    }

    #[test]
    fn test_validate_portal_unknown_from() {
        let mut manifest = make_valid_manifest();
        manifest.add_portal(PortalDefinition {
            from_chunk: ChunkId(999),
            to_chunk: ChunkId(1),
            face: PortalFace::PositiveX,
            default_open: true,
        });
        assert_eq!(
            manifest.validate(),
            Err(ChunkManifestError::PortalReferencesUnknownChunk(ChunkId(999)))
        );
    }

    #[test]
    fn test_validate_portal_unknown_to() {
        let mut manifest = make_valid_manifest();
        manifest.add_portal(PortalDefinition {
            from_chunk: ChunkId(1),
            to_chunk: ChunkId(999),
            face: PortalFace::PositiveX,
            default_open: true,
        });
        assert_eq!(
            manifest.validate(),
            Err(ChunkManifestError::PortalReferencesUnknownChunk(ChunkId(999)))
        );
    }

    #[test]
    fn test_validate_stitch_unknown_chunk() {
        let mut manifest = make_valid_manifest();
        manifest.add_stitch(BoundaryStitch {
            chunk_a: ChunkId(1),
            chunk_b: ChunkId(999),
            face: PortalFace::PositiveX,
            edge_vertex_count: 8,
            requires_lod_match: false,
        });
        assert_eq!(
            manifest.validate(),
            Err(ChunkManifestError::StitchReferencesUnknownChunk(ChunkId(999)))
        );
    }

    #[test]
    fn test_validate_lod_exceeds_max() {
        let mut manifest = ChunkManifest::new("world".to_string(), 64.0);
        manifest.add_chunk(ChunkReference {
            id: ChunkId(1),
            coord: ChunkCoord::new(0, 0, 0),
            asset_path: "test.bin".to_string(),
            available_lods: vec![0, 1, 10], // 10 exceeds MAX_LOD_LEVELS (8)
            size_per_lod: vec![1024, 512, 256],
            label: String::new(),
        });
        assert_eq!(
            manifest.validate(),
            Err(ChunkManifestError::LodLevelExceedsMax {
                chunk_id: ChunkId(1),
                lod: 10,
            })
        );
    }

    #[test]
    fn test_validate_lod_size_mismatch() {
        let mut manifest = ChunkManifest::new("world".to_string(), 64.0);
        manifest.add_chunk(ChunkReference {
            id: ChunkId(1),
            coord: ChunkCoord::new(0, 0, 0),
            asset_path: "test.bin".to_string(),
            available_lods: vec![0, 1, 2],
            size_per_lod: vec![1024, 512], // 2 sizes but 3 lods
            label: String::new(),
        });
        assert_eq!(
            manifest.validate(),
            Err(ChunkManifestError::LodSizeMismatch {
                chunk_id: ChunkId(1),
            })
        );
    }

    #[test]
    fn test_validate_empty_size_per_lod_is_ok() {
        let mut manifest = ChunkManifest::new("world".to_string(), 64.0);
        manifest.add_chunk(ChunkReference {
            id: ChunkId(1),
            coord: ChunkCoord::new(0, 0, 0),
            asset_path: "test.bin".to_string(),
            available_lods: vec![0, 1],
            size_per_lod: vec![], // empty is allowed (sizes not yet known)
            label: String::new(),
        });
        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn test_portal_face_opposite() {
        assert_eq!(PortalFace::PositiveX.opposite(), PortalFace::NegativeX);
        assert_eq!(PortalFace::NegativeX.opposite(), PortalFace::PositiveX);
        assert_eq!(PortalFace::PositiveY.opposite(), PortalFace::NegativeY);
        assert_eq!(PortalFace::NegativeY.opposite(), PortalFace::PositiveY);
        assert_eq!(PortalFace::PositiveZ.opposite(), PortalFace::NegativeZ);
        assert_eq!(PortalFace::NegativeZ.opposite(), PortalFace::PositiveZ);
    }

    #[test]
    fn test_portal_face_double_opposite_is_identity() {
        let faces = [
            PortalFace::PositiveX,
            PortalFace::NegativeX,
            PortalFace::PositiveY,
            PortalFace::NegativeY,
            PortalFace::PositiveZ,
            PortalFace::NegativeZ,
        ];
        for face in faces {
            assert_eq!(face.opposite().opposite(), face);
        }
    }

    #[test]
    fn test_manifest_with_portals_and_stitches_valid() {
        let mut manifest = make_valid_manifest();
        manifest.add_portal(PortalDefinition {
            from_chunk: ChunkId(1),
            to_chunk: ChunkId(2),
            face: PortalFace::PositiveX,
            default_open: true,
        });
        manifest.add_stitch(BoundaryStitch {
            chunk_a: ChunkId(1),
            chunk_b: ChunkId(2),
            face: PortalFace::PositiveX,
            edge_vertex_count: 16,
            requires_lod_match: true,
        });
        assert!(manifest.validate().is_ok());
    }
}
