use std::collections::HashMap;

use uuid::Uuid;

/// Minimum allowed rating value.
const MIN_RATING: f32 = 0.0;
/// Maximum allowed rating value.
const MAX_RATING: f32 = 5.0;

/// Per-world analytics data.
#[derive(Debug, Clone)]
pub struct WorldAnalytics {
    pub world_id: Uuid,
    pub visit_count: u64,
    pub concurrent_players: u32,
    pub peak_concurrent: u32,
    pub total_ratings: u32,
    pub rating_sum: f64,
}

impl WorldAnalytics {
    fn new(world_id: Uuid) -> Self {
        Self {
            world_id,
            visit_count: 0,
            concurrent_players: 0,
            peak_concurrent: 0,
            total_ratings: 0,
            rating_sum: 0.0,
        }
    }

    /// Average rating, or 0.0 if no ratings.
    pub fn average_rating(&self) -> f32 {
        if self.total_ratings == 0 {
            0.0
        } else {
            (self.rating_sum / self.total_ratings as f64) as f32
        }
    }
}

/// Analytics event types.
#[derive(Debug, Clone)]
pub enum AnalyticsEvent {
    Visit { world_id: Uuid },
    Leave { world_id: Uuid },
    Rate { world_id: Uuid, score: f32 },
}

/// Error returned when an analytics operation fails.
#[derive(Debug, PartialEq)]
pub enum AnalyticsError {
    RatingOutOfRange,
    NoConcurrentPlayers,
}

/// In-memory analytics tracker for world stats.
pub struct AnalyticsTracker {
    data: HashMap<Uuid, WorldAnalytics>,
}

impl AnalyticsTracker {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    /// Process an analytics event.
    pub fn record(&mut self, event: AnalyticsEvent) -> Result<(), AnalyticsError> {
        match event {
            AnalyticsEvent::Visit { world_id } => {
                let analytics = self
                    .data
                    .entry(world_id)
                    .or_insert_with(|| WorldAnalytics::new(world_id));
                analytics.visit_count += 1;
                analytics.concurrent_players += 1;
                if analytics.concurrent_players > analytics.peak_concurrent {
                    analytics.peak_concurrent = analytics.concurrent_players;
                }
                Ok(())
            }
            AnalyticsEvent::Leave { world_id } => {
                let analytics = self
                    .data
                    .entry(world_id)
                    .or_insert_with(|| WorldAnalytics::new(world_id));
                if analytics.concurrent_players == 0 {
                    return Err(AnalyticsError::NoConcurrentPlayers);
                }
                analytics.concurrent_players -= 1;
                Ok(())
            }
            AnalyticsEvent::Rate { world_id, score } => {
                if score < MIN_RATING || score > MAX_RATING {
                    return Err(AnalyticsError::RatingOutOfRange);
                }
                let analytics = self
                    .data
                    .entry(world_id)
                    .or_insert_with(|| WorldAnalytics::new(world_id));
                analytics.total_ratings += 1;
                analytics.rating_sum += score as f64;
                Ok(())
            }
        }
    }

    /// Get analytics for a specific world.
    pub fn get(&self, world_id: Uuid) -> Option<&WorldAnalytics> {
        self.data.get(&world_id)
    }

    /// Get visit count for velocity calculation.
    pub fn visit_count(&self, world_id: Uuid) -> u64 {
        self.data.get(&world_id).map_or(0, |a| a.visit_count)
    }

    /// Get concurrent player count.
    pub fn concurrent_players(&self, world_id: Uuid) -> u32 {
        self.data
            .get(&world_id)
            .map_or(0, |a| a.concurrent_players)
    }
}

impl Default for AnalyticsTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world_id() -> Uuid {
        Uuid::new_v4()
    }

    #[test]
    fn record_visit_increments_count() {
        let mut tracker = AnalyticsTracker::new();
        let wid = world_id();

        tracker
            .record(AnalyticsEvent::Visit { world_id: wid })
            .unwrap();
        tracker
            .record(AnalyticsEvent::Visit { world_id: wid })
            .unwrap();

        let analytics = tracker.get(wid).unwrap();
        assert_eq!(analytics.visit_count, 2);
        assert_eq!(analytics.concurrent_players, 2);
    }

    #[test]
    fn record_leave_decrements_concurrent() {
        let mut tracker = AnalyticsTracker::new();
        let wid = world_id();

        tracker
            .record(AnalyticsEvent::Visit { world_id: wid })
            .unwrap();
        tracker
            .record(AnalyticsEvent::Visit { world_id: wid })
            .unwrap();
        tracker
            .record(AnalyticsEvent::Leave { world_id: wid })
            .unwrap();

        let analytics = tracker.get(wid).unwrap();
        assert_eq!(analytics.concurrent_players, 1);
        assert_eq!(analytics.visit_count, 2); // visits unchanged
    }

    #[test]
    fn record_leave_below_zero_errors() {
        let mut tracker = AnalyticsTracker::new();
        let wid = world_id();

        let result = tracker.record(AnalyticsEvent::Leave { world_id: wid });
        assert_eq!(result, Err(AnalyticsError::NoConcurrentPlayers));
    }

    #[test]
    fn peak_concurrent_tracks_maximum() {
        let mut tracker = AnalyticsTracker::new();
        let wid = world_id();

        for _ in 0..5 {
            tracker
                .record(AnalyticsEvent::Visit { world_id: wid })
                .unwrap();
        }
        for _ in 0..3 {
            tracker
                .record(AnalyticsEvent::Leave { world_id: wid })
                .unwrap();
        }

        let analytics = tracker.get(wid).unwrap();
        assert_eq!(analytics.peak_concurrent, 5);
        assert_eq!(analytics.concurrent_players, 2);
    }

    #[test]
    fn record_rating_updates_average() {
        let mut tracker = AnalyticsTracker::new();
        let wid = world_id();

        tracker
            .record(AnalyticsEvent::Rate {
                world_id: wid,
                score: 4.0,
            })
            .unwrap();
        tracker
            .record(AnalyticsEvent::Rate {
                world_id: wid,
                score: 2.0,
            })
            .unwrap();

        let analytics = tracker.get(wid).unwrap();
        assert_eq!(analytics.total_ratings, 2);
        let avg = analytics.average_rating();
        assert!((avg - 3.0).abs() < 0.01);
    }

    #[test]
    fn record_rating_out_of_range() {
        let mut tracker = AnalyticsTracker::new();
        let wid = world_id();

        let result = tracker.record(AnalyticsEvent::Rate {
            world_id: wid,
            score: 6.0,
        });
        assert_eq!(result, Err(AnalyticsError::RatingOutOfRange));

        let result = tracker.record(AnalyticsEvent::Rate {
            world_id: wid,
            score: -1.0,
        });
        assert_eq!(result, Err(AnalyticsError::RatingOutOfRange));
    }

    #[test]
    fn record_rating_boundary_values() {
        let mut tracker = AnalyticsTracker::new();
        let wid = world_id();

        tracker
            .record(AnalyticsEvent::Rate {
                world_id: wid,
                score: MIN_RATING,
            })
            .unwrap();
        tracker
            .record(AnalyticsEvent::Rate {
                world_id: wid,
                score: MAX_RATING,
            })
            .unwrap();

        let analytics = tracker.get(wid).unwrap();
        assert_eq!(analytics.total_ratings, 2);
    }

    #[test]
    fn average_rating_no_ratings_returns_zero() {
        let analytics = WorldAnalytics::new(world_id());
        assert_eq!(analytics.average_rating(), 0.0);
    }

    #[test]
    fn get_nonexistent_world_returns_none() {
        let tracker = AnalyticsTracker::new();
        assert!(tracker.get(world_id()).is_none());
    }

    #[test]
    fn visit_count_helper() {
        let mut tracker = AnalyticsTracker::new();
        let wid = world_id();

        assert_eq!(tracker.visit_count(wid), 0);

        tracker
            .record(AnalyticsEvent::Visit { world_id: wid })
            .unwrap();
        assert_eq!(tracker.visit_count(wid), 1);
    }

    #[test]
    fn concurrent_players_helper() {
        let mut tracker = AnalyticsTracker::new();
        let wid = world_id();

        assert_eq!(tracker.concurrent_players(wid), 0);

        tracker
            .record(AnalyticsEvent::Visit { world_id: wid })
            .unwrap();
        assert_eq!(tracker.concurrent_players(wid), 1);
    }

    #[test]
    fn multiple_worlds_independent() {
        let mut tracker = AnalyticsTracker::new();
        let w1 = world_id();
        let w2 = world_id();

        tracker
            .record(AnalyticsEvent::Visit { world_id: w1 })
            .unwrap();
        tracker
            .record(AnalyticsEvent::Visit { world_id: w1 })
            .unwrap();
        tracker
            .record(AnalyticsEvent::Visit { world_id: w2 })
            .unwrap();

        assert_eq!(tracker.visit_count(w1), 2);
        assert_eq!(tracker.visit_count(w2), 1);
    }
}
