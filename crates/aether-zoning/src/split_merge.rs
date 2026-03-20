//! Dynamic zone split/merge manager.
//!
//! Monitors zone load metrics and triggers zone splits when player density
//! exceeds thresholds, or merges when adjacent zones are under-populated.
//! Hysteresis is enforced via configurable hold periods.

use std::collections::HashMap;

use crate::config::{AxisChoice, ZoneSpec};

/// Default hold period (ms) before a split is triggered after threshold is exceeded.
const DEFAULT_SPLIT_HOLD_MS: u64 = 5_000;
/// Default hold period (ms) before a merge is triggered after threshold is met.
const DEFAULT_MERGE_HOLD_MS: u64 = 10_000;

/// Tracks when a zone first crossed the split/merge threshold.
#[derive(Debug, Clone)]
struct ThresholdCrossing {
    first_crossed_ms: u64,
    last_sample_players: u32,
}

/// Configuration for the split/merge manager.
#[derive(Debug, Clone)]
pub struct SplitMergeConfig {
    pub split_threshold: u32,
    pub merge_threshold: u32,
    pub split_hold_ms: u64,
    pub merge_hold_ms: u64,
    pub preferred_split_axis: AxisChoice,
    pub min_zone_size: u32,
}

impl Default for SplitMergeConfig {
    fn default() -> Self {
        Self {
            split_threshold: 50,
            merge_threshold: 10,
            split_hold_ms: DEFAULT_SPLIT_HOLD_MS,
            merge_hold_ms: DEFAULT_MERGE_HOLD_MS,
            preferred_split_axis: AxisChoice::X,
            min_zone_size: 4,
        }
    }
}

/// A decision produced by the manager.
#[derive(Debug, Clone, PartialEq)]
pub enum SplitMergeDecision {
    /// Zone should be split into two child zones.
    Split {
        zone_id: String,
        left: ZoneSpec,
        right: ZoneSpec,
        axis: AxisChoice,
    },
    /// Two zones should be merged into one.
    Merge {
        zone_a: String,
        zone_b: String,
        merged: ZoneSpec,
    },
    /// No action needed.
    NoAction,
}

/// Pair of adjacent zones that may be merge candidates.
#[derive(Debug, Clone)]
pub struct AdjacentZonePair {
    pub zone_a: String,
    pub zone_b: String,
    pub shared_axis: AxisChoice,
}

/// Manages dynamic zone splitting and merging based on player density.
#[derive(Debug)]
pub struct SplitMergeManager {
    config: SplitMergeConfig,
    /// zone_id -> threshold crossing record for split candidates
    split_candidates: HashMap<String, ThresholdCrossing>,
    /// (zone_a, zone_b) -> threshold crossing record for merge candidates
    merge_candidates: HashMap<(String, String), ThresholdCrossing>,
    /// Tracks the current load per zone
    zone_load: HashMap<String, u32>,
}

impl SplitMergeManager {
    pub fn new(config: SplitMergeConfig) -> Self {
        Self {
            config,
            split_candidates: HashMap::new(),
            merge_candidates: HashMap::new(),
            zone_load: HashMap::new(),
        }
    }

    /// Update load metrics for a zone. Call this every tick with fresh samples.
    pub fn update_load(&mut self, zone_id: &str, players: u32) {
        self.zone_load.insert(zone_id.to_string(), players);
    }

    /// Evaluate all zones and return decisions. Should be called each tick after load updates.
    pub fn evaluate(
        &mut self,
        now_ms: u64,
        adjacent_pairs: &[AdjacentZonePair],
    ) -> Vec<SplitMergeDecision> {
        let mut decisions = Vec::new();

        // Evaluate splits
        let zone_ids: Vec<String> = self.zone_load.keys().cloned().collect();
        for zone_id in &zone_ids {
            let players = self.zone_load.get(zone_id).copied().unwrap_or(0);
            if players >= self.config.split_threshold {
                let crossing = self
                    .split_candidates
                    .entry(zone_id.clone())
                    .or_insert_with(|| ThresholdCrossing {
                        first_crossed_ms: now_ms,
                        last_sample_players: players,
                    });
                crossing.last_sample_players = players;

                if now_ms.saturating_sub(crossing.first_crossed_ms) >= self.config.split_hold_ms {
                    decisions.push(SplitMergeDecision::Split {
                        zone_id: zone_id.clone(),
                        left: ZoneSpec {
                            world_id: "world".to_string(),
                            shard_key: format!("{}-A", zone_id),
                            zone_index: format!("{}-L", zone_id),
                        },
                        right: ZoneSpec {
                            world_id: "world".to_string(),
                            shard_key: format!("{}-B", zone_id),
                            zone_index: format!("{}-R", zone_id),
                        },
                        axis: self.config.preferred_split_axis.clone(),
                    });
                    self.split_candidates.remove(zone_id);
                }
            } else {
                self.split_candidates.remove(zone_id);
            }
        }

        // Evaluate merges
        for pair in adjacent_pairs {
            let players_a = self.zone_load.get(&pair.zone_a).copied().unwrap_or(0);
            let players_b = self.zone_load.get(&pair.zone_b).copied().unwrap_or(0);

            let merge_key = normalize_pair(&pair.zone_a, &pair.zone_b);

            if players_a < self.config.merge_threshold && players_b < self.config.merge_threshold {
                let crossing = self
                    .merge_candidates
                    .entry(merge_key.clone())
                    .or_insert_with(|| ThresholdCrossing {
                        first_crossed_ms: now_ms,
                        last_sample_players: players_a + players_b,
                    });
                crossing.last_sample_players = players_a + players_b;

                if now_ms.saturating_sub(crossing.first_crossed_ms) >= self.config.merge_hold_ms {
                    decisions.push(SplitMergeDecision::Merge {
                        zone_a: pair.zone_a.clone(),
                        zone_b: pair.zone_b.clone(),
                        merged: ZoneSpec {
                            world_id: "world".to_string(),
                            shard_key: format!("{}-merged", pair.zone_a),
                            zone_index: format!("{}+{}", pair.zone_a, pair.zone_b),
                        },
                    });
                    self.merge_candidates.remove(&merge_key);
                }
            } else {
                self.merge_candidates.remove(&merge_key);
            }
        }

        decisions
    }

    /// Clear all tracked state (e.g., after a zone topology change).
    pub fn clear(&mut self) {
        self.split_candidates.clear();
        self.merge_candidates.clear();
        self.zone_load.clear();
    }

    /// Number of zones currently being tracked for split.
    pub fn split_candidate_count(&self) -> usize {
        self.split_candidates.len()
    }

    /// Number of zone pairs currently being tracked for merge.
    pub fn merge_candidate_count(&self) -> usize {
        self.merge_candidates.len()
    }
}

impl Default for SplitMergeManager {
    fn default() -> Self {
        Self::new(SplitMergeConfig::default())
    }
}

/// Normalize a zone pair key so (a, b) and (b, a) map to the same key.
fn normalize_pair(a: &str, b: &str) -> (String, String) {
    if a <= b {
        (a.to_string(), b.to_string())
    } else {
        (b.to_string(), a.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config(split: u32, merge: u32) -> SplitMergeConfig {
        SplitMergeConfig {
            split_threshold: split,
            merge_threshold: merge,
            split_hold_ms: 1000,
            merge_hold_ms: 2000,
            preferred_split_axis: AxisChoice::X,
            min_zone_size: 2,
        }
    }

    #[test]
    fn split_triggered_after_hold_period() {
        let mut mgr = SplitMergeManager::new(make_config(50, 10));

        // Update with high load
        mgr.update_load("zone-1", 60);

        // First evaluation -- threshold just crossed, hold not elapsed
        let decisions = mgr.evaluate(0, &[]);
        assert!(decisions
            .iter()
            .all(|d| matches!(d, SplitMergeDecision::NoAction)
                || !matches!(d, SplitMergeDecision::Split { .. })));
        assert_eq!(mgr.split_candidate_count(), 1);

        // Still within hold period
        let decisions = mgr.evaluate(500, &[]);
        let split_count = decisions
            .iter()
            .filter(|d| matches!(d, SplitMergeDecision::Split { .. }))
            .count();
        assert_eq!(split_count, 0);

        // Hold period elapsed
        let decisions = mgr.evaluate(1000, &[]);
        let splits: Vec<_> = decisions
            .iter()
            .filter(|d| matches!(d, SplitMergeDecision::Split { .. }))
            .collect();
        assert_eq!(splits.len(), 1);
        if let SplitMergeDecision::Split {
            zone_id,
            left,
            right,
            axis,
        } = &splits[0]
        {
            assert_eq!(zone_id, "zone-1");
            assert_eq!(left.zone_index, "zone-1-L");
            assert_eq!(right.zone_index, "zone-1-R");
            assert!(matches!(axis, AxisChoice::X));
        }
    }

    #[test]
    fn split_cancelled_when_load_drops() {
        let mut mgr = SplitMergeManager::new(make_config(50, 10));

        mgr.update_load("zone-1", 60);
        mgr.evaluate(0, &[]);
        assert_eq!(mgr.split_candidate_count(), 1);

        // Load drops below threshold
        mgr.update_load("zone-1", 30);
        mgr.evaluate(500, &[]);
        assert_eq!(mgr.split_candidate_count(), 0);
    }

    #[test]
    fn merge_triggered_after_hold_period() {
        let mut mgr = SplitMergeManager::new(make_config(50, 10));
        let pairs = vec![AdjacentZonePair {
            zone_a: "zone-1".to_string(),
            zone_b: "zone-2".to_string(),
            shared_axis: AxisChoice::X,
        }];

        mgr.update_load("zone-1", 5);
        mgr.update_load("zone-2", 3);

        // First eval -- under threshold but hold not elapsed
        let decisions = mgr.evaluate(0, &pairs);
        let merge_count = decisions
            .iter()
            .filter(|d| matches!(d, SplitMergeDecision::Merge { .. }))
            .count();
        assert_eq!(merge_count, 0);
        assert_eq!(mgr.merge_candidate_count(), 1);

        // Hold not yet elapsed
        let decisions = mgr.evaluate(1500, &pairs);
        let merge_count = decisions
            .iter()
            .filter(|d| matches!(d, SplitMergeDecision::Merge { .. }))
            .count();
        assert_eq!(merge_count, 0);

        // Hold elapsed
        let decisions = mgr.evaluate(2000, &pairs);
        let merges: Vec<_> = decisions
            .iter()
            .filter(|d| matches!(d, SplitMergeDecision::Merge { .. }))
            .collect();
        assert_eq!(merges.len(), 1);
    }

    #[test]
    fn merge_cancelled_when_load_rises() {
        let mut mgr = SplitMergeManager::new(make_config(50, 10));
        let pairs = vec![AdjacentZonePair {
            zone_a: "zone-1".to_string(),
            zone_b: "zone-2".to_string(),
            shared_axis: AxisChoice::X,
        }];

        mgr.update_load("zone-1", 5);
        mgr.update_load("zone-2", 3);
        mgr.evaluate(0, &pairs);
        assert_eq!(mgr.merge_candidate_count(), 1);

        // One zone rises above threshold
        mgr.update_load("zone-1", 15);
        mgr.evaluate(1000, &pairs);
        assert_eq!(mgr.merge_candidate_count(), 0);
    }

    #[test]
    fn no_action_when_load_normal() {
        let mut mgr = SplitMergeManager::new(make_config(50, 10));
        mgr.update_load("zone-1", 25);

        let decisions = mgr.evaluate(0, &[]);
        assert!(decisions.is_empty());
    }

    #[test]
    fn clear_resets_all_state() {
        let mut mgr = SplitMergeManager::new(make_config(50, 10));
        mgr.update_load("zone-1", 60);
        mgr.evaluate(0, &[]);
        assert_eq!(mgr.split_candidate_count(), 1);

        mgr.clear();
        assert_eq!(mgr.split_candidate_count(), 0);
        assert_eq!(mgr.merge_candidate_count(), 0);
    }

    #[test]
    fn normalize_pair_is_symmetric() {
        assert_eq!(normalize_pair("a", "b"), normalize_pair("b", "a"));
    }

    #[test]
    fn multiple_zones_split_independently() {
        let mut mgr = SplitMergeManager::new(make_config(50, 10));

        mgr.update_load("zone-1", 60);
        mgr.update_load("zone-2", 70);
        mgr.update_load("zone-3", 20); // normal

        // First tick
        mgr.evaluate(0, &[]);
        assert_eq!(mgr.split_candidate_count(), 2);

        // After hold period
        let decisions = mgr.evaluate(1000, &[]);
        let splits: Vec<_> = decisions
            .iter()
            .filter(|d| matches!(d, SplitMergeDecision::Split { .. }))
            .collect();
        assert_eq!(splits.len(), 2);
    }

    #[test]
    fn split_and_merge_can_coexist() {
        let mut mgr = SplitMergeManager::new(SplitMergeConfig {
            split_threshold: 50,
            merge_threshold: 10,
            split_hold_ms: 0, // immediate
            merge_hold_ms: 0, // immediate
            preferred_split_axis: AxisChoice::Z,
            min_zone_size: 2,
        });

        let pairs = vec![AdjacentZonePair {
            zone_a: "zone-low-a".to_string(),
            zone_b: "zone-low-b".to_string(),
            shared_axis: AxisChoice::X,
        }];

        mgr.update_load("zone-high", 60);
        mgr.update_load("zone-low-a", 3);
        mgr.update_load("zone-low-b", 2);

        let decisions = mgr.evaluate(0, &pairs);
        let split_count = decisions
            .iter()
            .filter(|d| matches!(d, SplitMergeDecision::Split { .. }))
            .count();
        let merge_count = decisions
            .iter()
            .filter(|d| matches!(d, SplitMergeDecision::Merge { .. }))
            .count();

        assert_eq!(split_count, 1);
        assert_eq!(merge_count, 1);
    }
}
