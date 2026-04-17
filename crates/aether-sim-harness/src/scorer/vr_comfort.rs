//! VR motion-sickness / comfort scorer.
//!
//! Heuristics, loosely calibrated against common XR comfort guidelines
//! (Oculus Store review guidelines, OpenXR best practices). Thresholds
//! are constants and documented — they are intentionally conservative
//! because the score gates agent commits.
//!
//! | Signal                         | Threshold (warn / fail) |
//! |--------------------------------|-------------------------|
//! | Angular velocity (deg/s)       | 90 / 180                |
//! | FOV delta per tick (deg)       | 5 / 10                  |
//! | Locomotion accel (m/s^2)       | 2.0 / 5.0               |
//! | Frame time variance (ms stddev)| 4 / 10                  |

use serde::{Deserialize, Serialize};

use crate::replay::SimState;

pub const ANGULAR_VEL_WARN_DEG_S: f32 = 90.0;
pub const ANGULAR_VEL_FAIL_DEG_S: f32 = 180.0;

pub const FOV_DELTA_WARN_DEG: f32 = 5.0;
pub const FOV_DELTA_FAIL_DEG: f32 = 10.0;

pub const LOCO_ACCEL_WARN: f32 = 2.0;
pub const LOCO_ACCEL_FAIL: f32 = 5.0;

pub const FRAME_TIME_STDDEV_WARN_MS: f32 = 4.0;
pub const FRAME_TIME_STDDEV_FAIL_MS: f32 = 10.0;

/// Score at or above this threshold counts as a pass.
pub const COMFORT_PASS_THRESHOLD: f32 = 0.75;
/// Score at or above this threshold but below pass counts as a warning.
pub const COMFORT_WARN_THRESHOLD: f32 = 0.5;

/// One structured reason contributing to deductions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComfortReason {
    pub code: String,
    pub message: String,
    /// `"warn"` or `"fail"`.
    pub severity: String,
    pub value: f32,
    pub threshold: f32,
}

/// Full comfort score for a simulation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComfortScore {
    pub overall: f32,
    pub reasons: Vec<ComfortReason>,
}

impl ComfortScore {
    pub fn is_pass(&self) -> bool {
        self.overall >= COMFORT_PASS_THRESHOLD
            && !self.reasons.iter().any(|r| r.severity == "fail")
    }

    pub fn is_warn(&self) -> bool {
        !self.is_pass() && self.overall >= COMFORT_WARN_THRESHOLD
    }
}

/// Score the simulation. Always returns a score — if there are no VR
/// samples at all, comfort is trivially `1.0` (no motion, no sickness).
pub fn score(state: &SimState) -> ComfortScore {
    if state.vr_samples.is_empty() {
        return ComfortScore {
            overall: 1.0,
            reasons: Vec::new(),
        };
    }

    let mut reasons = Vec::new();
    let mut deductions: f32 = 0.0;

    let mut max_angular = 0.0f32;
    let mut max_accel = 0.0f32;
    let mut max_fov_delta = 0.0f32;
    let mut prev_fov: Option<f32> = None;
    let mut frame_times: Vec<f32> = Vec::with_capacity(state.vr_samples.len());

    for s in &state.vr_samples {
        let ang_mag = magnitude(&s.angular_velocity_deg_s);
        max_angular = max_angular.max(ang_mag);

        let accel_mag = magnitude(&s.locomotion_accel_m_s2);
        max_accel = max_accel.max(accel_mag);

        if let Some(prev) = prev_fov {
            max_fov_delta = max_fov_delta.max((s.fov_deg - prev).abs());
        }
        prev_fov = Some(s.fov_deg);

        frame_times.push(s.frame_time_ms);
    }

    // Angular velocity.
    if max_angular >= ANGULAR_VEL_FAIL_DEG_S {
        deductions += 0.5;
        reasons.push(ComfortReason {
            code: "angular_velocity.fail".into(),
            message: "Head/world angular velocity exceeds fail threshold".into(),
            severity: "fail".into(),
            value: max_angular,
            threshold: ANGULAR_VEL_FAIL_DEG_S,
        });
    } else if max_angular >= ANGULAR_VEL_WARN_DEG_S {
        deductions += 0.2;
        reasons.push(ComfortReason {
            code: "angular_velocity.warn".into(),
            message: "Head/world angular velocity exceeds warn threshold".into(),
            severity: "warn".into(),
            value: max_angular,
            threshold: ANGULAR_VEL_WARN_DEG_S,
        });
    }

    // FOV rate of change.
    if max_fov_delta >= FOV_DELTA_FAIL_DEG {
        deductions += 0.3;
        reasons.push(ComfortReason {
            code: "fov_delta.fail".into(),
            message: "FOV change per tick exceeds fail threshold".into(),
            severity: "fail".into(),
            value: max_fov_delta,
            threshold: FOV_DELTA_FAIL_DEG,
        });
    } else if max_fov_delta >= FOV_DELTA_WARN_DEG {
        deductions += 0.1;
        reasons.push(ComfortReason {
            code: "fov_delta.warn".into(),
            message: "FOV change per tick exceeds warn threshold".into(),
            severity: "warn".into(),
            value: max_fov_delta,
            threshold: FOV_DELTA_WARN_DEG,
        });
    }

    // Artificial locomotion acceleration.
    if max_accel >= LOCO_ACCEL_FAIL {
        deductions += 0.3;
        reasons.push(ComfortReason {
            code: "locomotion_accel.fail".into(),
            message: "Artificial locomotion acceleration exceeds fail threshold".into(),
            severity: "fail".into(),
            value: max_accel,
            threshold: LOCO_ACCEL_FAIL,
        });
    } else if max_accel >= LOCO_ACCEL_WARN {
        deductions += 0.1;
        reasons.push(ComfortReason {
            code: "locomotion_accel.warn".into(),
            message: "Artificial locomotion acceleration exceeds warn threshold".into(),
            severity: "warn".into(),
            value: max_accel,
            threshold: LOCO_ACCEL_WARN,
        });
    }

    // Frame-time variance.
    let stddev = stddev(&frame_times);
    if stddev >= FRAME_TIME_STDDEV_FAIL_MS {
        deductions += 0.2;
        reasons.push(ComfortReason {
            code: "frame_time_stddev.fail".into(),
            message: "Frame-time standard deviation exceeds fail threshold".into(),
            severity: "fail".into(),
            value: stddev,
            threshold: FRAME_TIME_STDDEV_FAIL_MS,
        });
    } else if stddev >= FRAME_TIME_STDDEV_WARN_MS {
        deductions += 0.1;
        reasons.push(ComfortReason {
            code: "frame_time_stddev.warn".into(),
            message: "Frame-time standard deviation exceeds warn threshold".into(),
            severity: "warn".into(),
            value: stddev,
            threshold: FRAME_TIME_STDDEV_WARN_MS,
        });
    }

    let overall = (1.0 - deductions).clamp(0.0, 1.0);
    ComfortScore { overall, reasons }
}

fn magnitude(v: &[f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

fn stddev(xs: &[f32]) -> f32 {
    if xs.len() < 2 {
        return 0.0;
    }
    let n = xs.len() as f32;
    let mean = xs.iter().sum::<f32>() / n;
    let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / n;
    var.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::{SimState, VrSample};

    fn mk_state(samples: Vec<VrSample>) -> SimState {
        let mut s = SimState::default();
        s.vr_samples = samples;
        s
    }

    #[test]
    fn empty_vr_samples_is_perfect() {
        let s = score(&SimState::default());
        assert_eq!(s.overall, 1.0);
        assert!(s.is_pass());
    }

    #[test]
    fn low_motion_passes() {
        let state = mk_state(vec![
            VrSample {
                tick: 0,
                angular_velocity_deg_s: [10.0, 0.0, 0.0],
                fov_deg: 90.0,
                locomotion_accel_m_s2: [0.2, 0.0, 0.0],
                frame_time_ms: 11.0,
            };
            10
        ]);
        let s = score(&state);
        assert!(s.is_pass(), "score: {:?}", s);
    }

    #[test]
    fn high_angular_velocity_fails() {
        let state = mk_state(vec![VrSample {
            tick: 0,
            angular_velocity_deg_s: [300.0, 0.0, 0.0],
            fov_deg: 90.0,
            locomotion_accel_m_s2: [0.0; 3],
            frame_time_ms: 11.0,
        }]);
        let s = score(&state);
        assert!(!s.is_pass());
        assert!(s
            .reasons
            .iter()
            .any(|r| r.code == "angular_velocity.fail"));
    }

    #[test]
    fn fov_jump_fails() {
        let state = mk_state(vec![
            VrSample {
                tick: 0,
                angular_velocity_deg_s: [0.0; 3],
                fov_deg: 60.0,
                locomotion_accel_m_s2: [0.0; 3],
                frame_time_ms: 11.0,
            },
            VrSample {
                tick: 1,
                angular_velocity_deg_s: [0.0; 3],
                fov_deg: 120.0,
                locomotion_accel_m_s2: [0.0; 3],
                frame_time_ms: 11.0,
            },
        ]);
        let s = score(&state);
        assert!(s
            .reasons
            .iter()
            .any(|r| r.code == "fov_delta.fail"));
    }

    #[test]
    fn frame_time_jitter_warns() {
        let state = mk_state(vec![
            VrSample {
                tick: 0,
                angular_velocity_deg_s: [0.0; 3],
                fov_deg: 90.0,
                locomotion_accel_m_s2: [0.0; 3],
                frame_time_ms: 8.0,
            },
            VrSample {
                tick: 1,
                angular_velocity_deg_s: [0.0; 3],
                fov_deg: 90.0,
                locomotion_accel_m_s2: [0.0; 3],
                frame_time_ms: 16.0,
            },
        ]);
        let s = score(&state);
        assert!(s
            .reasons
            .iter()
            .any(|r| r.code.starts_with("frame_time_stddev")));
    }
}
