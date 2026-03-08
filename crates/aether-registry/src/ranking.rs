use crate::registry::WorldEntry;

/// Weight for visit velocity in trending score.
const VELOCITY_WEIGHT: f64 = 0.4;
/// Weight for rating in trending score.
const RATING_WEIGHT: f64 = 0.3;
/// Weight for concurrent player ratio in trending score.
const CONCURRENT_WEIGHT: f64 = 0.2;
/// Weight for recency bonus in trending score.
const RECENCY_WEIGHT: f64 = 0.1;
/// Featured worlds get a multiplier boost.
const FEATURED_BOOST: f64 = 1.5;
/// Maximum number of trending results returned.
const MAX_TRENDING_RESULTS: usize = 50;
/// Age in seconds beyond which the recency bonus drops to zero.
const RECENCY_CUTOFF_SECS: i64 = 86400 * 7; // 7 days

/// A world with its computed trending score.
#[derive(Debug, Clone)]
pub struct WorldScore {
    pub world_id: uuid::Uuid,
    pub score: f64,
}

/// Engine for computing trending and featured world rankings.
pub struct RankingEngine;

impl RankingEngine {
    /// Calculate the trending score for a single world entry.
    ///
    /// - `visit_velocity`: visits per hour in the recent window
    /// - `now_timestamp`: current Unix timestamp for recency calculation
    pub fn score(entry: &WorldEntry, visit_velocity: f64, now_timestamp: i64) -> f64 {
        let rating_norm = entry.rating as f64 / 5.0;

        let concurrent_ratio = if entry.max_players > 0 {
            entry.current_players as f64 / entry.max_players as f64
        } else {
            0.0
        };

        let age_secs = (now_timestamp - entry.created_at).max(0);
        let recency = if age_secs < RECENCY_CUTOFF_SECS {
            1.0 - (age_secs as f64 / RECENCY_CUTOFF_SECS as f64)
        } else {
            0.0
        };

        let base = (visit_velocity * VELOCITY_WEIGHT)
            + (rating_norm * RATING_WEIGHT)
            + (concurrent_ratio * CONCURRENT_WEIGHT)
            + (recency * RECENCY_WEIGHT);

        if entry.featured {
            base * FEATURED_BOOST
        } else {
            base
        }
    }

    /// Rank a list of worlds by trending score, returning the top results.
    ///
    /// `velocity_fn` provides the visit velocity for each world.
    pub fn rank_trending<F>(
        worlds: &[&WorldEntry],
        now_timestamp: i64,
        velocity_fn: F,
    ) -> Vec<WorldScore>
    where
        F: Fn(uuid::Uuid) -> f64,
    {
        let mut scores: Vec<WorldScore> = worlds
            .iter()
            .map(|w| {
                let velocity = velocity_fn(w.id);
                WorldScore {
                    world_id: w.id,
                    score: Self::score(w, velocity, now_timestamp),
                }
            })
            .collect();

        scores.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        scores.truncate(MAX_TRENDING_RESULTS);
        scores
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::make_entry;
    use uuid::Uuid;

    fn make_world(
        name: &str,
        rating: f32,
        current: u32,
        max: u32,
        visits: u64,
        featured: bool,
        created_at: i64,
    ) -> WorldEntry {
        let mut w = make_entry(name, Uuid::new_v4());
        w.rating = rating;
        w.current_players = current;
        w.max_players = max;
        w.visit_count = visits;
        w.featured = featured;
        w.created_at = created_at;
        w
    }

    #[test]
    fn score_basic() {
        let w = make_world("Test", 4.0, 25, 50, 1000, false, 0);
        let score = RankingEngine::score(&w, 10.0, 100);
        assert!(score > 0.0);
    }

    #[test]
    fn higher_velocity_higher_score() {
        let w = make_world("Test", 4.0, 25, 50, 1000, false, 0);
        let low = RankingEngine::score(&w, 1.0, 100);
        let high = RankingEngine::score(&w, 100.0, 100);
        assert!(high > low);
    }

    #[test]
    fn higher_rating_higher_score() {
        let w_low = make_world("Low", 1.0, 25, 50, 1000, false, 0);
        let w_high = make_world("High", 5.0, 25, 50, 1000, false, 0);
        let s_low = RankingEngine::score(&w_low, 10.0, 100);
        let s_high = RankingEngine::score(&w_high, 10.0, 100);
        assert!(s_high > s_low);
    }

    #[test]
    fn higher_concurrent_ratio_higher_score() {
        let w_empty = make_world("Empty", 4.0, 0, 50, 1000, false, 0);
        let w_full = make_world("Full", 4.0, 50, 50, 1000, false, 0);
        let s_empty = RankingEngine::score(&w_empty, 10.0, 100);
        let s_full = RankingEngine::score(&w_full, 10.0, 100);
        assert!(s_full > s_empty);
    }

    #[test]
    fn featured_boost_applies() {
        let w_normal = make_world("Normal", 4.0, 25, 50, 1000, false, 0);
        let w_featured = make_world("Featured", 4.0, 25, 50, 1000, true, 0);
        let s_normal = RankingEngine::score(&w_normal, 10.0, 100);
        let s_featured = RankingEngine::score(&w_featured, 10.0, 100);
        assert!(s_featured > s_normal);
        // Featured boost should be exactly FEATURED_BOOST multiplier
        let ratio = s_featured / s_normal;
        assert!((ratio - FEATURED_BOOST).abs() < 0.001);
    }

    #[test]
    fn recency_bonus_decays() {
        let now = 1_000_000i64;
        let w_new = make_world("New", 4.0, 25, 50, 1000, false, now - 3600); // 1 hour old
        let w_old = make_world("Old", 4.0, 25, 50, 1000, false, now - RECENCY_CUTOFF_SECS - 1);
        let s_new = RankingEngine::score(&w_new, 10.0, now);
        let s_old = RankingEngine::score(&w_old, 10.0, now);
        assert!(s_new > s_old);
    }

    #[test]
    fn recency_bonus_zero_after_cutoff() {
        let now = 1_000_000i64;
        let w_ancient = make_world("Ancient", 4.0, 25, 50, 1000, false, 0);
        let w_recent = make_world("Recent", 4.0, 25, 50, 1000, false, now - 100);
        let s_ancient = RankingEngine::score(&w_ancient, 10.0, now);
        let s_recent = RankingEngine::score(&w_recent, 10.0, now);
        assert!(s_recent > s_ancient);
    }

    #[test]
    fn zero_max_players_no_panic() {
        let mut w = make_world("ZeroMax", 4.0, 0, 0, 1000, false, 0);
        w.max_players = 0;
        // Should not panic, concurrent_ratio = 0
        let score = RankingEngine::score(&w, 10.0, 100);
        assert!(score >= 0.0);
    }

    #[test]
    fn rank_trending_returns_sorted() {
        let w1 = make_world("Low", 1.0, 0, 50, 100, false, 0);
        let w2 = make_world("High", 5.0, 50, 50, 50000, true, 0);
        let w3 = make_world("Mid", 3.0, 25, 50, 5000, false, 0);

        let worlds: Vec<&WorldEntry> = vec![&w1, &w2, &w3];
        let results = RankingEngine::rank_trending(&worlds, 100, |_| 10.0);

        assert_eq!(results.len(), 3);
        assert!(results[0].score >= results[1].score);
        assert!(results[1].score >= results[2].score);
        assert_eq!(results[0].world_id, w2.id); // Featured + high rating
    }

    #[test]
    fn rank_trending_caps_at_max() {
        let worlds_owned: Vec<WorldEntry> = (0..60)
            .map(|i| make_world(&format!("World{i}"), 3.0, 10, 50, 100, false, 0))
            .collect();
        let worlds: Vec<&WorldEntry> = worlds_owned.iter().collect();
        let results = RankingEngine::rank_trending(&worlds, 100, |_| 1.0);
        assert_eq!(results.len(), MAX_TRENDING_RESULTS);
    }

    #[test]
    fn rank_trending_with_velocity_fn() {
        let w1 = make_world("Viral", 3.0, 10, 50, 100, false, 0);
        let w2 = make_world("Stale", 3.0, 10, 50, 100, false, 0);

        let w1_id = w1.id;
        let worlds: Vec<&WorldEntry> = vec![&w1, &w2];
        let results = RankingEngine::rank_trending(&worlds, 100, |id| {
            if id == w1_id {
                100.0
            } else {
                0.1
            }
        });

        assert_eq!(results[0].world_id, w1_id);
    }

    #[test]
    fn rank_trending_empty_input() {
        let worlds: Vec<&WorldEntry> = vec![];
        let results = RankingEngine::rank_trending(&worlds, 100, |_| 1.0);
        assert!(results.is_empty());
    }
}
