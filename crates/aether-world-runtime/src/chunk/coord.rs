//! Chunk coordinate system and spatial utilities.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Default chunk size in world units (meters).
pub const DEFAULT_CHUNK_SIZE: f32 = 64.0;

/// A unique chunk identifier derived from manifest assignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChunkId(pub u64);

impl fmt::Display for ChunkId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "chunk:{}", self.0)
    }
}

/// A 3D grid coordinate identifying a chunk's position in the world grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChunkCoord {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl ChunkCoord {
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    /// Compute a stable ChunkId from this coordinate using a spatial hash.
    pub fn to_chunk_id(&self) -> ChunkId {
        // Use a simple spatial hash that avoids collisions for reasonable coordinate ranges.
        let hx = (self.x as i64).wrapping_mul(73856093);
        let hy = (self.y as i64).wrapping_mul(19349669);
        let hz = (self.z as i64).wrapping_mul(83492791);
        let combined = hx ^ hy ^ hz;
        ChunkId(combined as u64)
    }

    /// Convert a world-space position to the chunk coordinate containing it.
    pub fn from_world_position(x: f32, y: f32, z: f32, chunk_size: f32) -> Self {
        let size = if chunk_size <= 0.0 {
            DEFAULT_CHUNK_SIZE
        } else {
            chunk_size
        };
        Self {
            x: (x / size).floor() as i32,
            y: (y / size).floor() as i32,
            z: (z / size).floor() as i32,
        }
    }

    /// Get the world-space center position of this chunk.
    pub fn world_center(&self, chunk_size: f32) -> [f32; 3] {
        let size = if chunk_size <= 0.0 {
            DEFAULT_CHUNK_SIZE
        } else {
            chunk_size
        };
        [
            (self.x as f32 + 0.5) * size,
            (self.y as f32 + 0.5) * size,
            (self.z as f32 + 0.5) * size,
        ]
    }

    /// Chebyshev distance (max of absolute coordinate differences).
    pub fn chebyshev_distance(&self, other: &ChunkCoord) -> u32 {
        let dx = (self.x - other.x).unsigned_abs();
        let dy = (self.y - other.y).unsigned_abs();
        let dz = (self.z - other.z).unsigned_abs();
        dx.max(dy).max(dz)
    }

    /// Manhattan distance (sum of absolute coordinate differences).
    pub fn manhattan_distance(&self, other: &ChunkCoord) -> u32 {
        let dx = (self.x - other.x).unsigned_abs();
        let dy = (self.y - other.y).unsigned_abs();
        let dz = (self.z - other.z).unsigned_abs();
        dx + dy + dz
    }

    /// Enumerate all neighbors within Chebyshev distance `radius` (excluding self).
    pub fn neighbors_within(&self, radius: u32) -> Vec<ChunkCoord> {
        let r = radius as i32;
        let mut result = Vec::new();
        for dx in -r..=r {
            for dy in -r..=r {
                for dz in -r..=r {
                    if dx == 0 && dy == 0 && dz == 0 {
                        continue;
                    }
                    result.push(ChunkCoord::new(self.x + dx, self.y + dy, self.z + dz));
                }
            }
        }
        result
    }

    /// Enumerate all coordinates within Chebyshev distance `radius` (including self).
    pub fn coords_within(&self, radius: u32) -> Vec<ChunkCoord> {
        let r = radius as i32;
        let mut result = Vec::new();
        for dx in -r..=r {
            for dy in -r..=r {
                for dz in -r..=r {
                    result.push(ChunkCoord::new(self.x + dx, self.y + dy, self.z + dz));
                }
            }
        }
        result
    }
}

impl fmt::Display for ChunkCoord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {}, {})", self.x, self.y, self.z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_coord_new() {
        let coord = ChunkCoord::new(1, 2, 3);
        assert_eq!(coord.x, 1);
        assert_eq!(coord.y, 2);
        assert_eq!(coord.z, 3);
    }

    #[test]
    fn test_chunk_coord_equality() {
        let a = ChunkCoord::new(1, 2, 3);
        let b = ChunkCoord::new(1, 2, 3);
        let c = ChunkCoord::new(1, 2, 4);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_chunk_coord_hash_consistency() {
        let a = ChunkCoord::new(5, -3, 7);
        let b = ChunkCoord::new(5, -3, 7);
        assert_eq!(a.to_chunk_id(), b.to_chunk_id());
    }

    #[test]
    fn test_chunk_coord_hash_differs() {
        let a = ChunkCoord::new(0, 0, 0);
        let b = ChunkCoord::new(1, 0, 0);
        // While hash collisions are theoretically possible, these simple coords should differ
        assert_ne!(a.to_chunk_id(), b.to_chunk_id());
    }

    #[test]
    fn test_from_world_position_origin() {
        let coord = ChunkCoord::from_world_position(0.0, 0.0, 0.0, DEFAULT_CHUNK_SIZE);
        assert_eq!(coord, ChunkCoord::new(0, 0, 0));
    }

    #[test]
    fn test_from_world_position_positive() {
        let size = 64.0;
        let coord = ChunkCoord::from_world_position(100.0, 200.0, 50.0, size);
        assert_eq!(coord.x, 1); // 100/64 = 1.5625 -> floor = 1
        assert_eq!(coord.y, 3); // 200/64 = 3.125 -> floor = 3
        assert_eq!(coord.z, 0); // 50/64 = 0.78125 -> floor = 0
    }

    #[test]
    fn test_from_world_position_negative() {
        let size = 64.0;
        let coord = ChunkCoord::from_world_position(-10.0, -130.0, 0.0, size);
        assert_eq!(coord.x, -1); // -10/64 = -0.15625 -> floor = -1
        assert_eq!(coord.y, -3); // -130/64 = -2.03125 -> floor = -3
        assert_eq!(coord.z, 0);
    }

    #[test]
    fn test_from_world_position_zero_chunk_size_uses_default() {
        let coord = ChunkCoord::from_world_position(128.0, 0.0, 0.0, 0.0);
        // 128 / 64 = 2.0 -> floor = 2
        assert_eq!(coord.x, 2);
    }

    #[test]
    fn test_from_world_position_negative_chunk_size_uses_default() {
        let coord = ChunkCoord::from_world_position(128.0, 0.0, 0.0, -10.0);
        assert_eq!(coord.x, 2);
    }

    #[test]
    fn test_world_center() {
        let coord = ChunkCoord::new(0, 0, 0);
        let center = coord.world_center(64.0);
        assert!((center[0] - 32.0).abs() < f32::EPSILON);
        assert!((center[1] - 32.0).abs() < f32::EPSILON);
        assert!((center[2] - 32.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_world_center_offset_chunk() {
        let coord = ChunkCoord::new(2, -1, 3);
        let center = coord.world_center(64.0);
        assert!((center[0] - 160.0).abs() < f32::EPSILON); // (2+0.5)*64 = 160
        assert!((center[1] - -32.0).abs() < f32::EPSILON); // (-1+0.5)*64 = -32
        assert!((center[2] - 224.0).abs() < f32::EPSILON); // (3+0.5)*64 = 224
    }

    #[test]
    fn test_chebyshev_distance_same() {
        let a = ChunkCoord::new(1, 2, 3);
        assert_eq!(a.chebyshev_distance(&a), 0);
    }

    #[test]
    fn test_chebyshev_distance_axis_aligned() {
        let a = ChunkCoord::new(0, 0, 0);
        let b = ChunkCoord::new(5, 0, 0);
        assert_eq!(a.chebyshev_distance(&b), 5);
    }

    #[test]
    fn test_chebyshev_distance_diagonal() {
        let a = ChunkCoord::new(0, 0, 0);
        let b = ChunkCoord::new(3, 4, 2);
        assert_eq!(a.chebyshev_distance(&b), 4); // max(3, 4, 2) = 4
    }

    #[test]
    fn test_chebyshev_distance_negative_coords() {
        let a = ChunkCoord::new(-2, -3, 1);
        let b = ChunkCoord::new(2, 1, -1);
        // diffs: 4, 4, 2 -> max = 4
        assert_eq!(a.chebyshev_distance(&b), 4);
    }

    #[test]
    fn test_manhattan_distance() {
        let a = ChunkCoord::new(0, 0, 0);
        let b = ChunkCoord::new(3, 4, 2);
        assert_eq!(a.manhattan_distance(&b), 9); // 3 + 4 + 2
    }

    #[test]
    fn test_manhattan_distance_same() {
        let a = ChunkCoord::new(1, 2, 3);
        assert_eq!(a.manhattan_distance(&a), 0);
    }

    #[test]
    fn test_neighbors_within_radius_1() {
        let coord = ChunkCoord::new(0, 0, 0);
        let neighbors = coord.neighbors_within(1);
        // 3x3x3 = 27, minus self = 26
        assert_eq!(neighbors.len(), 26);
        assert!(!neighbors.contains(&coord));
    }

    #[test]
    fn test_neighbors_within_radius_0() {
        let coord = ChunkCoord::new(0, 0, 0);
        let neighbors = coord.neighbors_within(0);
        assert!(neighbors.is_empty());
    }

    #[test]
    fn test_coords_within_includes_self() {
        let coord = ChunkCoord::new(5, 5, 5);
        let coords = coord.coords_within(1);
        // 3x3x3 = 27
        assert_eq!(coords.len(), 27);
        assert!(coords.contains(&coord));
    }

    #[test]
    fn test_coords_within_radius_2() {
        let coord = ChunkCoord::new(0, 0, 0);
        let coords = coord.coords_within(2);
        // 5x5x5 = 125
        assert_eq!(coords.len(), 125);
    }

    #[test]
    fn test_chunk_id_display() {
        let id = ChunkId(42);
        assert_eq!(format!("{}", id), "chunk:42");
    }

    #[test]
    fn test_chunk_coord_display() {
        let coord = ChunkCoord::new(1, -2, 3);
        assert_eq!(format!("{}", coord), "(1, -2, 3)");
    }

    #[test]
    fn test_roundtrip_position_to_coord_and_back() {
        let size = 64.0;
        let pos = [100.0, 200.0, 300.0];
        let coord = ChunkCoord::from_world_position(pos[0], pos[1], pos[2], size);
        let center = coord.world_center(size);
        // Center should be within chunk_size/2 of the original position
        assert!((center[0] - pos[0]).abs() < size);
        assert!((center[1] - pos[1]).abs() < size);
        assert!((center[2] - pos[2]).abs() < size);
    }

    #[test]
    fn test_chunk_coord_on_boundary() {
        let size = 64.0;
        // Exactly on a boundary
        let coord = ChunkCoord::from_world_position(64.0, 0.0, 0.0, size);
        assert_eq!(coord.x, 1);
    }
}
