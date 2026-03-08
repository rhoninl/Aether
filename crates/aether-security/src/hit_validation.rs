//! Server-side hit validation for competitive worlds.
//!
//! Validates client-submitted hit claims against server-authoritative state
//! to prevent hit registration exploits.

use std::fmt;

use crate::movement_validator::Vec3;

/// Default maximum weapon range in meters.
const DEFAULT_MAX_HIT_RANGE: f32 = 100.0;

/// Default timing window for hit validation in milliseconds.
/// Hits claimed outside this window from the server tick are rejected.
const DEFAULT_HIT_TIMING_WINDOW_MS: u64 = 500;

/// Default maximum angle deviation (degrees) for line-of-sight plausibility.
const DEFAULT_MAX_LOS_ANGLE_DEG: f32 = 90.0;

/// Configuration for hit validation.
#[derive(Debug, Clone)]
pub struct HitValidationConfig {
    /// Maximum weapon range in meters.
    pub max_hit_range: f32,
    /// Maximum timing window for hit claims in milliseconds.
    pub hit_timing_window_ms: u64,
    /// Maximum angle deviation for line-of-sight in degrees.
    pub max_los_angle_deg: f32,
}

impl Default for HitValidationConfig {
    fn default() -> Self {
        Self {
            max_hit_range: DEFAULT_MAX_HIT_RANGE,
            hit_timing_window_ms: DEFAULT_HIT_TIMING_WINDOW_MS,
            max_los_angle_deg: DEFAULT_MAX_LOS_ANGLE_DEG,
        }
    }
}

/// A client-submitted hit claim.
#[derive(Debug, Clone)]
pub struct HitClaim {
    /// ID of the attacking entity.
    pub attacker_id: u64,
    /// ID of the target entity.
    pub target_id: u64,
    /// Claimed attacker position at time of hit.
    pub attacker_pos: Vec3,
    /// Claimed target position at time of hit.
    pub target_pos: Vec3,
    /// Client-reported timestamp of the hit in milliseconds.
    pub timestamp_ms: u64,
    /// Weapon/attack identifier.
    pub weapon_id: u32,
    /// Weapon range override (if weapon has custom range). None = use default.
    pub weapon_range: Option<f32>,
}

/// Server-side state snapshot for validating a hit.
#[derive(Debug, Clone)]
pub struct ServerHitState {
    /// Server-authoritative attacker position.
    pub attacker_pos: Vec3,
    /// Server-authoritative target position.
    pub target_pos: Vec3,
    /// Server tick timestamp in milliseconds.
    pub server_timestamp_ms: u64,
    /// Whether the target is alive/hittable.
    pub target_alive: bool,
    /// Attacker's facing direction (unit vector). None if not tracked.
    pub attacker_facing: Option<Vec3>,
}

/// Result of hit validation.
#[derive(Debug, Clone, PartialEq)]
pub enum HitResult {
    /// Hit is valid.
    Valid,
    /// Target is out of weapon range.
    OutOfRange {
        distance: f32,
        max_range: f32,
    },
    /// Hit timestamp is outside acceptable window.
    TimingViolation {
        time_diff_ms: u64,
        max_window_ms: u64,
    },
    /// Target is not alive or hittable.
    TargetNotAlive,
    /// Position mismatch between client claim and server state.
    PositionMismatch {
        attacker_discrepancy: f32,
        target_discrepancy: f32,
    },
    /// Attacker is not facing the target (line-of-sight check).
    LineOfSightViolation {
        angle_deg: f32,
        max_angle_deg: f32,
    },
}

impl fmt::Display for HitResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HitResult::Valid => write!(f, "hit validated"),
            HitResult::OutOfRange {
                distance,
                max_range,
            } => {
                write!(
                    f,
                    "hit out of range: {:.2}m exceeds {:.2}m",
                    distance, max_range
                )
            }
            HitResult::TimingViolation {
                time_diff_ms,
                max_window_ms,
            } => {
                write!(
                    f,
                    "hit timing violation: {}ms diff exceeds {}ms window",
                    time_diff_ms, max_window_ms
                )
            }
            HitResult::TargetNotAlive => write!(f, "target is not alive"),
            HitResult::PositionMismatch {
                attacker_discrepancy,
                target_discrepancy,
            } => {
                write!(
                    f,
                    "position mismatch: attacker={:.2}m, target={:.2}m",
                    attacker_discrepancy, target_discrepancy
                )
            }
            HitResult::LineOfSightViolation {
                angle_deg,
                max_angle_deg,
            } => {
                write!(
                    f,
                    "line-of-sight violation: {:.1} deg exceeds {:.1} deg",
                    angle_deg, max_angle_deg
                )
            }
        }
    }
}

/// Validates a hit claim against server-authoritative state.
///
/// Checks in order:
/// 1. Target is alive
/// 2. Timing window
/// 3. Range check (using server positions)
/// 4. Line-of-sight (if facing direction is available)
///
/// Position mismatch is reported as a separate check since small discrepancies
/// are expected due to latency.
pub fn validate_hit(
    claim: &HitClaim,
    server_state: &ServerHitState,
    config: &HitValidationConfig,
) -> HitResult {
    // 1. Target must be alive
    if !server_state.target_alive {
        return HitResult::TargetNotAlive;
    }

    // 2. Timing window check
    let time_diff = if claim.timestamp_ms > server_state.server_timestamp_ms {
        claim.timestamp_ms - server_state.server_timestamp_ms
    } else {
        server_state.server_timestamp_ms - claim.timestamp_ms
    };
    if time_diff > config.hit_timing_window_ms {
        return HitResult::TimingViolation {
            time_diff_ms: time_diff,
            max_window_ms: config.hit_timing_window_ms,
        };
    }

    // 3. Range check using server-authoritative positions
    let effective_range = claim.weapon_range.unwrap_or(config.max_hit_range);
    let distance = server_state
        .attacker_pos
        .distance_to(&server_state.target_pos);
    if distance > effective_range {
        return HitResult::OutOfRange {
            distance,
            max_range: effective_range,
        };
    }

    // 4. Line-of-sight check if facing direction is available
    if let Some(facing) = &server_state.attacker_facing {
        let to_target = Vec3::new(
            server_state.target_pos.x - server_state.attacker_pos.x,
            server_state.target_pos.y - server_state.attacker_pos.y,
            server_state.target_pos.z - server_state.attacker_pos.z,
        );
        let to_target_len = (to_target.x * to_target.x
            + to_target.y * to_target.y
            + to_target.z * to_target.z)
            .sqrt();
        let facing_len =
            (facing.x * facing.x + facing.y * facing.y + facing.z * facing.z).sqrt();

        if to_target_len > f32::EPSILON && facing_len > f32::EPSILON {
            let dot = facing.x * to_target.x + facing.y * to_target.y + facing.z * to_target.z;
            let cos_angle = (dot / (facing_len * to_target_len)).clamp(-1.0, 1.0);
            let angle_deg = cos_angle.acos().to_degrees();

            if angle_deg > config.max_los_angle_deg {
                return HitResult::LineOfSightViolation {
                    angle_deg,
                    max_angle_deg: config.max_los_angle_deg,
                };
            }
        }
    }

    HitResult::Valid
}

/// Computes the position discrepancy between client claim and server state.
/// This is a separate check because small discrepancies are normal due to
/// network latency.
pub fn compute_position_discrepancy(
    claim: &HitClaim,
    server_state: &ServerHitState,
) -> (f32, f32) {
    let attacker_disc = claim.attacker_pos.distance_to(&server_state.attacker_pos);
    let target_disc = claim.target_pos.distance_to(&server_state.target_pos);
    (attacker_disc, target_disc)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> HitValidationConfig {
        HitValidationConfig::default()
    }

    fn make_claim(attacker_pos: Vec3, target_pos: Vec3, timestamp_ms: u64) -> HitClaim {
        HitClaim {
            attacker_id: 1,
            target_id: 2,
            attacker_pos,
            target_pos,
            timestamp_ms,
            weapon_id: 1,
            weapon_range: None,
        }
    }

    fn make_server_state(
        attacker_pos: Vec3,
        target_pos: Vec3,
        server_timestamp_ms: u64,
    ) -> ServerHitState {
        ServerHitState {
            attacker_pos,
            target_pos,
            server_timestamp_ms,
            target_alive: true,
            attacker_facing: None,
        }
    }

    // --- Valid hits ---

    #[test]
    fn test_valid_hit_close_range() {
        let claim = make_claim(Vec3::zero(), Vec3::new(5.0, 0.0, 0.0), 1000);
        let state = make_server_state(Vec3::zero(), Vec3::new(5.0, 0.0, 0.0), 1000);
        let result = validate_hit(&claim, &state, &default_config());
        assert_eq!(result, HitResult::Valid);
    }

    #[test]
    fn test_valid_hit_at_max_range() {
        let claim = make_claim(Vec3::zero(), Vec3::new(100.0, 0.0, 0.0), 1000);
        let state = make_server_state(Vec3::zero(), Vec3::new(100.0, 0.0, 0.0), 1000);
        let result = validate_hit(&claim, &state, &default_config());
        assert_eq!(result, HitResult::Valid);
    }

    #[test]
    fn test_valid_hit_within_timing_window() {
        let claim = make_claim(Vec3::zero(), Vec3::new(5.0, 0.0, 0.0), 1200);
        let state = make_server_state(Vec3::zero(), Vec3::new(5.0, 0.0, 0.0), 1000);
        let result = validate_hit(&claim, &state, &default_config());
        assert_eq!(result, HitResult::Valid);
    }

    // --- Target not alive ---

    #[test]
    fn test_target_not_alive() {
        let claim = make_claim(Vec3::zero(), Vec3::new(5.0, 0.0, 0.0), 1000);
        let mut state = make_server_state(Vec3::zero(), Vec3::new(5.0, 0.0, 0.0), 1000);
        state.target_alive = false;
        let result = validate_hit(&claim, &state, &default_config());
        assert_eq!(result, HitResult::TargetNotAlive);
    }

    // --- Out of range ---

    #[test]
    fn test_out_of_range() {
        let claim = make_claim(Vec3::zero(), Vec3::new(150.0, 0.0, 0.0), 1000);
        let state = make_server_state(Vec3::zero(), Vec3::new(150.0, 0.0, 0.0), 1000);
        let result = validate_hit(&claim, &state, &default_config());
        match result {
            HitResult::OutOfRange {
                distance,
                max_range,
            } => {
                assert!((distance - 150.0).abs() < 1e-5);
                assert!((max_range - 100.0).abs() < 1e-5);
            }
            other => panic!("expected OutOfRange, got {:?}", other),
        }
    }

    #[test]
    fn test_out_of_range_just_over() {
        let claim = make_claim(Vec3::zero(), Vec3::new(100.01, 0.0, 0.0), 1000);
        let state = make_server_state(Vec3::zero(), Vec3::new(100.01, 0.0, 0.0), 1000);
        let result = validate_hit(&claim, &state, &default_config());
        assert!(matches!(result, HitResult::OutOfRange { .. }));
    }

    #[test]
    fn test_weapon_range_override() {
        let mut claim = make_claim(Vec3::zero(), Vec3::new(15.0, 0.0, 0.0), 1000);
        claim.weapon_range = Some(10.0);
        let state = make_server_state(Vec3::zero(), Vec3::new(15.0, 0.0, 0.0), 1000);
        let result = validate_hit(&claim, &state, &default_config());
        assert!(matches!(result, HitResult::OutOfRange { .. }));
    }

    #[test]
    fn test_weapon_range_override_valid() {
        let mut claim = make_claim(Vec3::zero(), Vec3::new(150.0, 0.0, 0.0), 1000);
        claim.weapon_range = Some(200.0);
        let state = make_server_state(Vec3::zero(), Vec3::new(150.0, 0.0, 0.0), 1000);
        let result = validate_hit(&claim, &state, &default_config());
        assert_eq!(result, HitResult::Valid);
    }

    // --- Timing violations ---

    #[test]
    fn test_timing_violation_too_early() {
        let claim = make_claim(Vec3::zero(), Vec3::new(5.0, 0.0, 0.0), 100);
        let state = make_server_state(Vec3::zero(), Vec3::new(5.0, 0.0, 0.0), 1000);
        let result = validate_hit(&claim, &state, &default_config());
        match result {
            HitResult::TimingViolation {
                time_diff_ms,
                max_window_ms,
            } => {
                assert_eq!(time_diff_ms, 900);
                assert_eq!(max_window_ms, 500);
            }
            other => panic!("expected TimingViolation, got {:?}", other),
        }
    }

    #[test]
    fn test_timing_violation_too_late() {
        let claim = make_claim(Vec3::zero(), Vec3::new(5.0, 0.0, 0.0), 2000);
        let state = make_server_state(Vec3::zero(), Vec3::new(5.0, 0.0, 0.0), 1000);
        let result = validate_hit(&claim, &state, &default_config());
        assert!(matches!(result, HitResult::TimingViolation { .. }));
    }

    #[test]
    fn test_timing_at_boundary() {
        let claim = make_claim(Vec3::zero(), Vec3::new(5.0, 0.0, 0.0), 1500);
        let state = make_server_state(Vec3::zero(), Vec3::new(5.0, 0.0, 0.0), 1000);
        let result = validate_hit(&claim, &state, &default_config());
        assert_eq!(result, HitResult::Valid);
    }

    #[test]
    fn test_timing_just_over_boundary() {
        let claim = make_claim(Vec3::zero(), Vec3::new(5.0, 0.0, 0.0), 1501);
        let state = make_server_state(Vec3::zero(), Vec3::new(5.0, 0.0, 0.0), 1000);
        let result = validate_hit(&claim, &state, &default_config());
        assert!(matches!(result, HitResult::TimingViolation { .. }));
    }

    // --- Line-of-sight ---

    #[test]
    fn test_los_facing_target() {
        let claim = make_claim(Vec3::zero(), Vec3::new(10.0, 0.0, 0.0), 1000);
        let mut state = make_server_state(Vec3::zero(), Vec3::new(10.0, 0.0, 0.0), 1000);
        state.attacker_facing = Some(Vec3::new(1.0, 0.0, 0.0)); // facing right
        let result = validate_hit(&claim, &state, &default_config());
        assert_eq!(result, HitResult::Valid);
    }

    #[test]
    fn test_los_facing_away() {
        let claim = make_claim(Vec3::zero(), Vec3::new(10.0, 0.0, 0.0), 1000);
        let mut state = make_server_state(Vec3::zero(), Vec3::new(10.0, 0.0, 0.0), 1000);
        state.attacker_facing = Some(Vec3::new(-1.0, 0.0, 0.0)); // facing opposite
        let result = validate_hit(&claim, &state, &default_config());
        assert!(matches!(result, HitResult::LineOfSightViolation { .. }));
    }

    #[test]
    fn test_los_at_angle_limit() {
        let claim = make_claim(Vec3::zero(), Vec3::new(10.0, 0.0, 0.0), 1000);
        let mut state = make_server_state(Vec3::zero(), Vec3::new(10.0, 0.0, 0.0), 1000);
        // Facing perpendicular = 90 degrees (at the limit)
        state.attacker_facing = Some(Vec3::new(0.0, 1.0, 0.0));
        let result = validate_hit(&claim, &state, &default_config());
        // 90 degrees is at the limit, should be valid (not strictly greater)
        assert_eq!(result, HitResult::Valid);
    }

    #[test]
    fn test_los_no_facing_direction() {
        let claim = make_claim(Vec3::zero(), Vec3::new(10.0, 0.0, 0.0), 1000);
        let state = make_server_state(Vec3::zero(), Vec3::new(10.0, 0.0, 0.0), 1000);
        // No facing direction -> LOS check is skipped
        let result = validate_hit(&claim, &state, &default_config());
        assert_eq!(result, HitResult::Valid);
    }

    // --- Position discrepancy ---

    #[test]
    fn test_position_discrepancy_none() {
        let claim = make_claim(Vec3::zero(), Vec3::new(10.0, 0.0, 0.0), 1000);
        let state = make_server_state(Vec3::zero(), Vec3::new(10.0, 0.0, 0.0), 1000);
        let (att, tgt) = compute_position_discrepancy(&claim, &state);
        assert!(att < f32::EPSILON);
        assert!(tgt < f32::EPSILON);
    }

    #[test]
    fn test_position_discrepancy_present() {
        let claim = make_claim(
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(10.0, 0.0, 0.0),
            1000,
        );
        let state = make_server_state(Vec3::zero(), Vec3::new(12.0, 0.0, 0.0), 1000);
        let (att, tgt) = compute_position_discrepancy(&claim, &state);
        assert!((att - 1.0).abs() < 1e-5);
        assert!((tgt - 2.0).abs() < 1e-5);
    }

    // --- Custom config ---

    #[test]
    fn test_custom_short_range() {
        let config = HitValidationConfig {
            max_hit_range: 5.0,
            ..Default::default()
        };
        let claim = make_claim(Vec3::zero(), Vec3::new(10.0, 0.0, 0.0), 1000);
        let state = make_server_state(Vec3::zero(), Vec3::new(10.0, 0.0, 0.0), 1000);
        let result = validate_hit(&claim, &state, &config);
        assert!(matches!(result, HitResult::OutOfRange { .. }));
    }

    #[test]
    fn test_custom_tight_timing() {
        let config = HitValidationConfig {
            hit_timing_window_ms: 50,
            ..Default::default()
        };
        let claim = make_claim(Vec3::zero(), Vec3::new(5.0, 0.0, 0.0), 1100);
        let state = make_server_state(Vec3::zero(), Vec3::new(5.0, 0.0, 0.0), 1000);
        let result = validate_hit(&claim, &state, &config);
        assert!(matches!(result, HitResult::TimingViolation { .. }));
    }

    // --- Display ---

    #[test]
    fn test_display_valid() {
        assert!(HitResult::Valid.to_string().contains("validated"));
    }

    #[test]
    fn test_display_out_of_range() {
        let r = HitResult::OutOfRange {
            distance: 150.0,
            max_range: 100.0,
        };
        assert!(r.to_string().contains("out of range"));
    }

    #[test]
    fn test_display_timing() {
        let r = HitResult::TimingViolation {
            time_diff_ms: 800,
            max_window_ms: 500,
        };
        assert!(r.to_string().contains("timing"));
    }

    #[test]
    fn test_display_not_alive() {
        assert!(HitResult::TargetNotAlive.to_string().contains("not alive"));
    }

    #[test]
    fn test_display_position_mismatch() {
        let r = HitResult::PositionMismatch {
            attacker_discrepancy: 5.0,
            target_discrepancy: 3.0,
        };
        assert!(r.to_string().contains("mismatch"));
    }

    #[test]
    fn test_display_los_violation() {
        let r = HitResult::LineOfSightViolation {
            angle_deg: 120.0,
            max_angle_deg: 90.0,
        };
        assert!(r.to_string().contains("line-of-sight"));
    }

    // --- Server uses its own positions for range ---

    #[test]
    fn test_server_positions_used_for_range() {
        // Client claims close range, but server says they're far apart
        let claim = make_claim(Vec3::zero(), Vec3::new(5.0, 0.0, 0.0), 1000);
        let state = make_server_state(Vec3::zero(), Vec3::new(200.0, 0.0, 0.0), 1000);
        let result = validate_hit(&claim, &state, &default_config());
        assert!(matches!(result, HitResult::OutOfRange { .. }));
    }

    // --- Priority: target_alive checked first ---

    #[test]
    fn test_target_not_alive_checked_first() {
        // Both timing and range would fail, but target_alive is checked first
        let claim = make_claim(Vec3::zero(), Vec3::new(500.0, 0.0, 0.0), 0);
        let mut state = make_server_state(Vec3::zero(), Vec3::new(500.0, 0.0, 0.0), 10000);
        state.target_alive = false;
        let result = validate_hit(&claim, &state, &default_config());
        assert_eq!(result, HitResult::TargetNotAlive);
    }
}
