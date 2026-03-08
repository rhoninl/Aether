//! Teleport/position-jump detection.
//!
//! Detects impossible position changes between server ticks by tracking
//! each entity's last known position and comparing against physically
//! possible displacement.

use std::collections::HashMap;
use std::fmt;

use crate::movement_validator::Vec3;

/// Default maximum allowed displacement per tick in meters.
/// A position change exceeding this distance in a single tick is flagged
/// as a teleport violation.
const DEFAULT_MAX_TELEPORT_DISTANCE: f32 = 100.0;

/// Default minimum time between position updates in seconds.
/// Updates arriving faster than this are suspicious.
const DEFAULT_MIN_UPDATE_INTERVAL_SECS: f32 = 0.001;

/// Configuration for teleport detection.
#[derive(Debug, Clone)]
pub struct TeleportConfig {
    /// Maximum allowed position change in a single tick (meters).
    pub max_teleport_distance: f32,
    /// Minimum interval between position updates (seconds).
    pub min_update_interval_secs: f32,
}

impl Default for TeleportConfig {
    fn default() -> Self {
        Self {
            max_teleport_distance: DEFAULT_MAX_TELEPORT_DISTANCE,
            min_update_interval_secs: DEFAULT_MIN_UPDATE_INTERVAL_SECS,
        }
    }
}

/// Tracked state for a single entity.
#[derive(Debug, Clone)]
struct EntityState {
    position: Vec3,
    timestamp_secs: f64,
}

/// Result of a teleport check.
#[derive(Debug, Clone, PartialEq)]
pub enum TeleportResult {
    /// Position change is within acceptable limits.
    Valid {
        distance: f32,
    },
    /// First position report for this entity (no previous state to compare).
    FirstReport,
    /// Position change exceeds maximum teleport distance.
    TeleportViolation {
        distance: f32,
        max_allowed: f32,
    },
    /// Update arrived too soon after the previous one.
    UpdateTooFast {
        interval_secs: f64,
        min_interval_secs: f32,
    },
    /// Invalid input.
    InvalidInput {
        reason: String,
    },
}

impl fmt::Display for TeleportResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TeleportResult::Valid { distance } => {
                write!(f, "valid position update, distance={:.2}m", distance)
            }
            TeleportResult::FirstReport => {
                write!(f, "first position report accepted")
            }
            TeleportResult::TeleportViolation {
                distance,
                max_allowed,
            } => {
                write!(
                    f,
                    "teleport violation: {:.2}m exceeds max {:.2}m",
                    distance, max_allowed
                )
            }
            TeleportResult::UpdateTooFast {
                interval_secs,
                min_interval_secs,
            } => {
                write!(
                    f,
                    "update too fast: {:.4}s < min {:.4}s",
                    interval_secs, min_interval_secs
                )
            }
            TeleportResult::InvalidInput { reason } => {
                write!(f, "invalid input: {}", reason)
            }
        }
    }
}

/// Stateful teleport detector tracking per-entity positions.
#[derive(Debug)]
pub struct TeleportDetector {
    config: TeleportConfig,
    entities: HashMap<u64, EntityState>,
}

impl TeleportDetector {
    /// Creates a new detector with the given configuration.
    pub fn new(config: TeleportConfig) -> Self {
        Self {
            config,
            entities: HashMap::new(),
        }
    }

    /// Creates a new detector with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(TeleportConfig::default())
    }

    /// Checks a position update for teleport violations.
    ///
    /// # Arguments
    /// - `entity_id`: Unique entity identifier.
    /// - `new_pos`: Client-claimed new position.
    /// - `timestamp_secs`: Server timestamp of this update in seconds.
    pub fn check(
        &mut self,
        entity_id: u64,
        new_pos: Vec3,
        timestamp_secs: f64,
    ) -> TeleportResult {
        if timestamp_secs < 0.0 {
            return TeleportResult::InvalidInput {
                reason: "timestamp must be non-negative".to_string(),
            };
        }

        let prev = match self.entities.get(&entity_id) {
            Some(state) => state.clone(),
            None => {
                self.entities.insert(
                    entity_id,
                    EntityState {
                        position: new_pos,
                        timestamp_secs,
                    },
                );
                return TeleportResult::FirstReport;
            }
        };

        let interval = timestamp_secs - prev.timestamp_secs;
        if interval < self.config.min_update_interval_secs as f64 {
            return TeleportResult::UpdateTooFast {
                interval_secs: interval,
                min_interval_secs: self.config.min_update_interval_secs,
            };
        }

        let distance = prev.position.distance_to(&new_pos);

        if distance > self.config.max_teleport_distance {
            return TeleportResult::TeleportViolation {
                distance,
                max_allowed: self.config.max_teleport_distance,
            };
        }

        // Update stored state on valid check
        self.entities.insert(
            entity_id,
            EntityState {
                position: new_pos,
                timestamp_secs,
            },
        );

        TeleportResult::Valid { distance }
    }

    /// Removes tracking state for an entity (e.g., on disconnect).
    pub fn remove_entity(&mut self, entity_id: u64) {
        self.entities.remove(&entity_id);
    }

    /// Returns the number of currently tracked entities.
    pub fn tracked_count(&self) -> usize {
        self.entities.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_detector() -> TeleportDetector {
        TeleportDetector::with_defaults()
    }

    // --- First report ---

    #[test]
    fn test_first_report() {
        let mut det = default_detector();
        let result = det.check(1, Vec3::new(10.0, 0.0, 10.0), 0.0);
        assert!(matches!(result, TeleportResult::FirstReport));
    }

    #[test]
    fn test_first_report_tracks_entity() {
        let mut det = default_detector();
        det.check(1, Vec3::zero(), 0.0);
        assert_eq!(det.tracked_count(), 1);
    }

    // --- Valid movement ---

    #[test]
    fn test_valid_small_movement() {
        let mut det = default_detector();
        det.check(1, Vec3::zero(), 0.0);
        let result = det.check(1, Vec3::new(5.0, 0.0, 0.0), 1.0);
        match result {
            TeleportResult::Valid { distance } => {
                assert!((distance - 5.0).abs() < 1e-5);
            }
            other => panic!("expected Valid, got {:?}", other),
        }
    }

    #[test]
    fn test_valid_at_max_distance() {
        let mut det = default_detector();
        det.check(1, Vec3::zero(), 0.0);
        // Exactly at limit
        let result = det.check(1, Vec3::new(100.0, 0.0, 0.0), 1.0);
        assert!(matches!(result, TeleportResult::Valid { .. }));
    }

    #[test]
    fn test_valid_sequential_updates() {
        let mut det = default_detector();
        det.check(1, Vec3::zero(), 0.0);
        let r1 = det.check(1, Vec3::new(10.0, 0.0, 0.0), 1.0);
        assert!(matches!(r1, TeleportResult::Valid { .. }));
        let r2 = det.check(1, Vec3::new(20.0, 0.0, 0.0), 2.0);
        assert!(matches!(r2, TeleportResult::Valid { .. }));
    }

    // --- Teleport violations ---

    #[test]
    fn test_teleport_violation() {
        let mut det = default_detector();
        det.check(1, Vec3::zero(), 0.0);
        // 200 meters in one tick -> violation (limit = 100)
        let result = det.check(1, Vec3::new(200.0, 0.0, 0.0), 1.0);
        match result {
            TeleportResult::TeleportViolation {
                distance,
                max_allowed,
            } => {
                assert!((distance - 200.0).abs() < 1e-5);
                assert!((max_allowed - 100.0).abs() < 1e-5);
            }
            other => panic!("expected TeleportViolation, got {:?}", other),
        }
    }

    #[test]
    fn test_teleport_violation_just_over() {
        let mut det = default_detector();
        det.check(1, Vec3::zero(), 0.0);
        let result = det.check(1, Vec3::new(100.01, 0.0, 0.0), 1.0);
        assert!(matches!(result, TeleportResult::TeleportViolation { .. }));
    }

    #[test]
    fn test_teleport_violation_does_not_update_state() {
        let mut det = default_detector();
        det.check(1, Vec3::zero(), 0.0);
        // Violation: entity stays tracked at origin
        det.check(1, Vec3::new(500.0, 0.0, 0.0), 1.0);
        // Subsequent valid move from origin should still work
        let result = det.check(1, Vec3::new(5.0, 0.0, 0.0), 2.0);
        assert!(matches!(result, TeleportResult::Valid { .. }));
    }

    // --- Update too fast ---

    #[test]
    fn test_update_too_fast() {
        let mut det = default_detector();
        det.check(1, Vec3::zero(), 1.0);
        // Very small interval
        let result = det.check(1, Vec3::new(1.0, 0.0, 0.0), 1.0005);
        assert!(matches!(result, TeleportResult::UpdateTooFast { .. }));
    }

    #[test]
    fn test_update_same_timestamp() {
        let mut det = default_detector();
        det.check(1, Vec3::zero(), 1.0);
        let result = det.check(1, Vec3::new(1.0, 0.0, 0.0), 1.0);
        assert!(matches!(result, TeleportResult::UpdateTooFast { .. }));
    }

    // --- Invalid input ---

    #[test]
    fn test_negative_timestamp() {
        let mut det = default_detector();
        let result = det.check(1, Vec3::zero(), -1.0);
        assert!(matches!(result, TeleportResult::InvalidInput { .. }));
    }

    // --- Multiple entities ---

    #[test]
    fn test_independent_entities() {
        let mut det = default_detector();
        det.check(1, Vec3::zero(), 0.0);
        det.check(2, Vec3::new(50.0, 0.0, 0.0), 0.0);
        assert_eq!(det.tracked_count(), 2);

        // Entity 1 moves 10m -> valid
        let r1 = det.check(1, Vec3::new(10.0, 0.0, 0.0), 1.0);
        assert!(matches!(r1, TeleportResult::Valid { .. }));

        // Entity 2 moves 10m -> valid (from 50, not from 0)
        let r2 = det.check(2, Vec3::new(60.0, 0.0, 0.0), 1.0);
        assert!(matches!(r2, TeleportResult::Valid { .. }));
    }

    // --- Remove entity ---

    #[test]
    fn test_remove_entity() {
        let mut det = default_detector();
        det.check(1, Vec3::zero(), 0.0);
        assert_eq!(det.tracked_count(), 1);
        det.remove_entity(1);
        assert_eq!(det.tracked_count(), 0);
    }

    #[test]
    fn test_remove_then_first_report() {
        let mut det = default_detector();
        det.check(1, Vec3::zero(), 0.0);
        det.remove_entity(1);
        let result = det.check(1, Vec3::new(500.0, 0.0, 0.0), 1.0);
        // After removal, next check is a first report again
        assert!(matches!(result, TeleportResult::FirstReport));
    }

    // --- Custom config ---

    #[test]
    fn test_custom_max_distance() {
        let config = TeleportConfig {
            max_teleport_distance: 10.0,
            ..Default::default()
        };
        let mut det = TeleportDetector::new(config);
        det.check(1, Vec3::zero(), 0.0);
        let result = det.check(1, Vec3::new(11.0, 0.0, 0.0), 1.0);
        assert!(matches!(result, TeleportResult::TeleportViolation { .. }));
    }

    // --- Display ---

    #[test]
    fn test_display_valid() {
        let r = TeleportResult::Valid { distance: 5.0 };
        assert!(r.to_string().contains("valid"));
    }

    #[test]
    fn test_display_first_report() {
        let r = TeleportResult::FirstReport;
        assert!(r.to_string().contains("first"));
    }

    #[test]
    fn test_display_violation() {
        let r = TeleportResult::TeleportViolation {
            distance: 200.0,
            max_allowed: 100.0,
        };
        let s = r.to_string();
        assert!(s.contains("teleport violation"));
        assert!(s.contains("200.00"));
    }

    #[test]
    fn test_display_too_fast() {
        let r = TeleportResult::UpdateTooFast {
            interval_secs: 0.0001,
            min_interval_secs: 0.001,
        };
        assert!(r.to_string().contains("too fast"));
    }

    #[test]
    fn test_display_invalid() {
        let r = TeleportResult::InvalidInput {
            reason: "bad ts".to_string(),
        };
        assert!(r.to_string().contains("bad ts"));
    }
}
