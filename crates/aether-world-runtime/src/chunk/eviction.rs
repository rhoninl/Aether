//! Chunk eviction policy: LRU with distance weighting.

use super::coord::{ChunkCoord, ChunkId};

/// Default maximum number of cached chunks before eviction triggers.
pub const DEFAULT_MAX_CACHED_CHUNKS: usize = 128;

/// Default distance weight exponent for eviction scoring.
/// Higher values penalize distant chunks more aggressively.
const DEFAULT_DISTANCE_WEIGHT_EXPONENT: f32 = 2.0;

/// A record used to compute eviction priority for a cached chunk.
#[derive(Debug, Clone)]
pub struct EvictionCandidate {
    pub id: ChunkId,
    pub coord: ChunkCoord,
    pub last_access_ms: u64,
    pub size_bytes: u64,
}

/// Eviction policy configuration.
#[derive(Debug, Clone)]
pub struct EvictionPolicy {
    /// Maximum number of chunks allowed in cache before eviction.
    pub max_cached_chunks: usize,
    /// Maximum total bytes allowed in cache before eviction.
    pub max_cache_bytes: u64,
    /// Distance weight exponent (higher = more aggressive distance penalty).
    pub distance_weight_exponent: f32,
}

impl Default for EvictionPolicy {
    fn default() -> Self {
        Self {
            max_cached_chunks: DEFAULT_MAX_CACHED_CHUNKS,
            max_cache_bytes: 512 * 1024 * 1024, // 512 MB
            distance_weight_exponent: DEFAULT_DISTANCE_WEIGHT_EXPONENT,
        }
    }
}

impl EvictionPolicy {
    pub fn new(max_cached_chunks: usize, max_cache_bytes: u64) -> Self {
        Self {
            max_cached_chunks: max_cached_chunks.max(1),
            max_cache_bytes: max_cache_bytes.max(1),
            distance_weight_exponent: DEFAULT_DISTANCE_WEIGHT_EXPONENT,
        }
    }

    /// Compute the eviction score for a candidate.
    /// Higher score = higher priority for eviction.
    ///
    /// Score = time_since_last_access * distance_weight
    /// where distance_weight = (1 + chebyshev_distance)^exponent
    pub fn eviction_score(
        &self,
        candidate: &EvictionCandidate,
        player_coord: &ChunkCoord,
        now_ms: u64,
    ) -> f64 {
        let time_since_access = now_ms.saturating_sub(candidate.last_access_ms) as f64;
        let distance = candidate.coord.chebyshev_distance(player_coord) as f64;
        let distance_weight = (1.0 + distance).powf(self.distance_weight_exponent as f64);
        time_since_access * distance_weight
    }

    /// Determine which chunks should be evicted given the current cache state.
    ///
    /// Returns chunk IDs ordered from highest to lowest eviction priority (first = evict first).
    pub fn select_evictions(
        &self,
        candidates: &[EvictionCandidate],
        player_coord: &ChunkCoord,
        now_ms: u64,
    ) -> Vec<ChunkId> {
        if candidates.len() <= self.max_cached_chunks {
            let total_bytes: u64 = candidates.iter().map(|c| c.size_bytes).sum();
            if total_bytes <= self.max_cache_bytes {
                return Vec::new();
            }
        }

        // Score all candidates
        let mut scored: Vec<(ChunkId, f64, u64)> = candidates
            .iter()
            .map(|c| {
                let score = self.eviction_score(c, player_coord, now_ms);
                (c.id, score, c.size_bytes)
            })
            .collect();

        // Sort by score descending (highest score = evict first)
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let mut evict_ids = Vec::new();
        let mut remaining_count = candidates.len();
        let mut remaining_bytes: u64 = candidates.iter().map(|c| c.size_bytes).sum();

        for (id, _score, size) in &scored {
            if remaining_count <= self.max_cached_chunks && remaining_bytes <= self.max_cache_bytes {
                break;
            }
            evict_ids.push(*id);
            remaining_count = remaining_count.saturating_sub(1);
            remaining_bytes = remaining_bytes.saturating_sub(*size);
        }

        evict_ids
    }

    /// Check whether the cache is over capacity.
    pub fn is_over_capacity(&self, chunk_count: usize, total_bytes: u64) -> bool {
        chunk_count > self.max_cached_chunks || total_bytes > self.max_cache_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::coord::ChunkCoord;

    fn make_candidate(id: u64, x: i32, y: i32, z: i32, last_access: u64, size: u64) -> EvictionCandidate {
        EvictionCandidate {
            id: ChunkId(id),
            coord: ChunkCoord::new(x, y, z),
            last_access_ms: last_access,
            size_bytes: size,
        }
    }

    #[test]
    fn test_default_policy() {
        let policy = EvictionPolicy::default();
        assert_eq!(policy.max_cached_chunks, DEFAULT_MAX_CACHED_CHUNKS);
        assert_eq!(policy.max_cache_bytes, 512 * 1024 * 1024);
    }

    #[test]
    fn test_new_policy_clamps_min() {
        let policy = EvictionPolicy::new(0, 0);
        assert_eq!(policy.max_cached_chunks, 1);
        assert_eq!(policy.max_cache_bytes, 1);
    }

    #[test]
    fn test_eviction_score_recent_close_is_low() {
        let policy = EvictionPolicy::default();
        let candidate = make_candidate(1, 0, 0, 0, 990, 1024);
        let player = ChunkCoord::new(0, 0, 0);
        let score = policy.eviction_score(&candidate, &player, 1000);
        // time = 10ms, distance = 0, weight = (1+0)^2 = 1, score = 10
        assert!((score - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_eviction_score_old_far_is_high() {
        let policy = EvictionPolicy::default();
        let candidate = make_candidate(1, 10, 0, 0, 0, 1024);
        let player = ChunkCoord::new(0, 0, 0);
        let score = policy.eviction_score(&candidate, &player, 10000);
        // time = 10000ms, distance = 10, weight = (1+10)^2 = 121, score = 1210000
        assert!((score - 1_210_000.0).abs() < 1.0);
    }

    #[test]
    fn test_eviction_score_increases_with_distance() {
        let policy = EvictionPolicy::default();
        let player = ChunkCoord::new(0, 0, 0);
        let now = 1000;

        let close = make_candidate(1, 1, 0, 0, 500, 1024);
        let far = make_candidate(2, 5, 0, 0, 500, 1024);

        let score_close = policy.eviction_score(&close, &player, now);
        let score_far = policy.eviction_score(&far, &player, now);
        assert!(score_far > score_close);
    }

    #[test]
    fn test_eviction_score_increases_with_age() {
        let policy = EvictionPolicy::default();
        let player = ChunkCoord::new(0, 0, 0);
        let now = 10000;

        let recent = make_candidate(1, 1, 0, 0, 9000, 1024);
        let old = make_candidate(2, 1, 0, 0, 1000, 1024);

        let score_recent = policy.eviction_score(&recent, &player, now);
        let score_old = policy.eviction_score(&old, &player, now);
        assert!(score_old > score_recent);
    }

    #[test]
    fn test_select_evictions_under_capacity_returns_empty() {
        let policy = EvictionPolicy::new(10, 1024 * 1024);
        let player = ChunkCoord::new(0, 0, 0);
        let candidates = vec![
            make_candidate(1, 0, 0, 0, 100, 1024),
            make_candidate(2, 1, 0, 0, 100, 1024),
        ];
        let evictions = policy.select_evictions(&candidates, &player, 1000);
        assert!(evictions.is_empty());
    }

    #[test]
    fn test_select_evictions_over_count_capacity() {
        let policy = EvictionPolicy::new(2, u64::MAX);
        let player = ChunkCoord::new(0, 0, 0);
        let candidates = vec![
            make_candidate(1, 0, 0, 0, 900, 1024), // close, recent
            make_candidate(2, 5, 0, 0, 100, 1024),  // far, old (highest score)
            make_candidate(3, 1, 0, 0, 500, 1024),  // medium
        ];

        let evictions = policy.select_evictions(&candidates, &player, 1000);
        assert_eq!(evictions.len(), 1); // Need to evict 1 to get to capacity 2
        // The far-old chunk should be evicted first
        assert_eq!(evictions[0], ChunkId(2));
    }

    #[test]
    fn test_select_evictions_over_byte_capacity() {
        let policy = EvictionPolicy::new(100, 2000); // 2000 byte limit
        let player = ChunkCoord::new(0, 0, 0);
        let candidates = vec![
            make_candidate(1, 0, 0, 0, 900, 1000),
            make_candidate(2, 5, 0, 0, 100, 1000), // far, old
            make_candidate(3, 1, 0, 0, 500, 1000),
        ];
        // Total: 3000 bytes, limit 2000, need to evict at least 1

        let evictions = policy.select_evictions(&candidates, &player, 1000);
        assert!(!evictions.is_empty());
        // Should evict the highest-scored chunk
        assert_eq!(evictions[0], ChunkId(2));
    }

    #[test]
    fn test_select_evictions_evicts_multiple() {
        let policy = EvictionPolicy::new(1, u64::MAX);
        let player = ChunkCoord::new(0, 0, 0);
        let candidates = vec![
            make_candidate(1, 0, 0, 0, 900, 1024),
            make_candidate(2, 5, 0, 0, 100, 1024),
            make_candidate(3, 3, 0, 0, 500, 1024),
        ];

        let evictions = policy.select_evictions(&candidates, &player, 1000);
        assert_eq!(evictions.len(), 2); // 3 chunks - 1 capacity = 2 to evict
    }

    #[test]
    fn test_select_evictions_empty_candidates() {
        let policy = EvictionPolicy::new(10, u64::MAX);
        let player = ChunkCoord::new(0, 0, 0);
        let evictions = policy.select_evictions(&[], &player, 1000);
        assert!(evictions.is_empty());
    }

    #[test]
    fn test_is_over_capacity_by_count() {
        let policy = EvictionPolicy::new(5, u64::MAX);
        assert!(!policy.is_over_capacity(5, 0));
        assert!(policy.is_over_capacity(6, 0));
    }

    #[test]
    fn test_is_over_capacity_by_bytes() {
        let policy = EvictionPolicy::new(100, 1000);
        assert!(!policy.is_over_capacity(1, 1000));
        assert!(policy.is_over_capacity(1, 1001));
    }

    #[test]
    fn test_is_over_capacity_both() {
        let policy = EvictionPolicy::new(5, 1000);
        assert!(!policy.is_over_capacity(5, 1000));
        assert!(policy.is_over_capacity(6, 1001));
        assert!(policy.is_over_capacity(6, 500)); // over count
        assert!(policy.is_over_capacity(3, 1001)); // over bytes
    }

    #[test]
    fn test_eviction_score_zero_time_zero_distance() {
        let policy = EvictionPolicy::default();
        let candidate = make_candidate(1, 0, 0, 0, 1000, 1024);
        let player = ChunkCoord::new(0, 0, 0);
        let score = policy.eviction_score(&candidate, &player, 1000);
        // time = 0, so score should be 0 regardless of distance
        assert!((score - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_eviction_preserves_closest_most_recent() {
        let policy = EvictionPolicy::new(2, u64::MAX);
        let player = ChunkCoord::new(5, 5, 5);

        let candidates = vec![
            make_candidate(1, 5, 5, 5, 990, 1024), // same chunk as player, very recent
            make_candidate(2, 5, 5, 6, 950, 1024),  // adjacent, recent
            make_candidate(3, 0, 0, 0, 100, 1024),   // far away, old
            make_candidate(4, 10, 10, 10, 200, 1024), // far away, old
        ];

        let evictions = policy.select_evictions(&candidates, &player, 1000);
        assert_eq!(evictions.len(), 2);
        // The two far-old chunks should be evicted; close chunks should be kept
        let kept: Vec<ChunkId> = candidates
            .iter()
            .map(|c| c.id)
            .filter(|id| !evictions.contains(id))
            .collect();
        assert!(kept.contains(&ChunkId(1)));
        assert!(kept.contains(&ChunkId(2)));
    }
}
