//! Viseme and blend shape evaluation for lip sync.
//!
//! Takes `LipSyncFrame` inputs and produces smooth blend shape weights
//! for each viseme, with configurable interpolation.

use crate::lipsync::{LipSyncFrame, VisemeCurve};

/// Number of viseme shapes supported.
const VISEME_COUNT: usize = 7;
/// Default interpolation speed (weight per millisecond).
const DEFAULT_LERP_SPEED: f32 = 0.005;

/// Blend shape weights for all viseme curves.
#[derive(Debug, Clone)]
pub struct VisemeWeights {
    /// Weight for each viseme: [A, E, I, O, U, F, Rest].
    pub weights: [f32; VISEME_COUNT],
}

impl VisemeWeights {
    /// All weights zero.
    pub fn zero() -> Self {
        Self {
            weights: [0.0; VISEME_COUNT],
        }
    }

    /// Rest pose (only Rest viseme active).
    pub fn rest() -> Self {
        let mut w = Self::zero();
        w.weights[viseme_index(&VisemeCurve::Rest)] = 1.0;
        w
    }

    /// Get the weight for a specific viseme.
    pub fn get(&self, viseme: &VisemeCurve) -> f32 {
        self.weights[viseme_index(viseme)]
    }
}

/// Evaluator that smooths viseme transitions over time.
#[derive(Debug, Clone)]
pub struct VisemeEvaluator {
    /// Current blend weights.
    current: VisemeWeights,
    /// Target blend weights.
    target: VisemeWeights,
    /// Interpolation speed (weight per ms).
    lerp_speed: f32,
}

impl VisemeEvaluator {
    /// Create a new evaluator starting at rest.
    pub fn new() -> Self {
        Self {
            current: VisemeWeights::rest(),
            target: VisemeWeights::rest(),
            lerp_speed: DEFAULT_LERP_SPEED,
        }
    }

    /// Create with a custom interpolation speed.
    pub fn with_lerp_speed(lerp_speed: f32) -> Self {
        Self {
            lerp_speed,
            ..Self::new()
        }
    }

    /// Get current weights.
    pub fn current_weights(&self) -> &VisemeWeights {
        &self.current
    }

    /// Set a new target from a lip sync frame.
    ///
    /// The evaluator will smoothly interpolate toward the target viseme.
    pub fn set_target(&mut self, frame: &LipSyncFrame) {
        // Zero out all targets
        self.target = VisemeWeights::zero();
        // Set the target viseme weight based on amplitude
        let idx = viseme_index(&frame.viseme);
        self.target.weights[idx] = frame.amplitude.clamp(0.0, 1.0);
        // Add rest weight for the remainder
        let rest_idx = viseme_index(&VisemeCurve::Rest);
        self.target.weights[rest_idx] = 1.0 - self.target.weights[idx];
    }

    /// Set the target to rest (mouth closed).
    pub fn set_rest(&mut self) {
        self.target = VisemeWeights::rest();
    }

    /// Update interpolation by a time delta.
    ///
    /// # Arguments
    /// * `dt_ms` - Time elapsed in milliseconds
    ///
    /// # Returns
    /// Current blend weights after interpolation.
    pub fn update(&mut self, dt_ms: u64) -> &VisemeWeights {
        let step = self.lerp_speed * dt_ms as f32;
        for i in 0..VISEME_COUNT {
            let diff = self.target.weights[i] - self.current.weights[i];
            if diff.abs() < step {
                self.current.weights[i] = self.target.weights[i];
            } else {
                self.current.weights[i] += diff.signum() * step;
            }
            self.current.weights[i] = self.current.weights[i].clamp(0.0, 1.0);
        }
        &self.current
    }

    /// Immediately snap to target weights (no interpolation).
    pub fn snap_to_target(&mut self) {
        self.current = self.target.clone();
    }
}

impl Default for VisemeEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

/// Map a `VisemeCurve` to an index in the weights array.
fn viseme_index(viseme: &VisemeCurve) -> usize {
    match viseme {
        VisemeCurve::A => 0,
        VisemeCurve::E => 1,
        VisemeCurve::I => 2,
        VisemeCurve::O => 3,
        VisemeCurve::U => 4,
        VisemeCurve::F => 5,
        VisemeCurve::Rest => 6,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 0.01;

    fn make_frame(viseme: VisemeCurve, amplitude: f32) -> LipSyncFrame {
        LipSyncFrame {
            timestamp_ms: 0,
            viseme,
            amplitude,
            phoneme_id: 0,
        }
    }

    #[test]
    fn test_starts_at_rest() {
        let eval = VisemeEvaluator::new();
        let w = eval.current_weights();
        assert!((w.get(&VisemeCurve::Rest) - 1.0).abs() < EPSILON);
        assert!((w.get(&VisemeCurve::A)).abs() < EPSILON);
    }

    #[test]
    fn test_set_target_viseme() {
        let mut eval = VisemeEvaluator::new();
        eval.set_target(&make_frame(VisemeCurve::A, 0.8));
        eval.snap_to_target();
        let w = eval.current_weights();
        assert!((w.get(&VisemeCurve::A) - 0.8).abs() < EPSILON);
        assert!((w.get(&VisemeCurve::Rest) - 0.2).abs() < EPSILON);
    }

    #[test]
    fn test_snap_to_target() {
        let mut eval = VisemeEvaluator::new();
        eval.set_target(&make_frame(VisemeCurve::O, 1.0));
        eval.snap_to_target();
        let w = eval.current_weights();
        assert!((w.get(&VisemeCurve::O) - 1.0).abs() < EPSILON);
        assert!((w.get(&VisemeCurve::Rest)).abs() < EPSILON);
    }

    #[test]
    fn test_interpolation_moves_toward_target() {
        let mut eval = VisemeEvaluator::with_lerp_speed(0.01);
        eval.set_target(&make_frame(VisemeCurve::E, 1.0));

        // Before update: rest = 1.0, E = 0.0
        let initial_e = eval.current_weights().get(&VisemeCurve::E);
        assert!(initial_e.abs() < EPSILON);

        // After some updates
        eval.update(50);
        let mid_e = eval.current_weights().get(&VisemeCurve::E);
        assert!(mid_e > initial_e, "E weight should increase");
        assert!(mid_e < 1.0, "E weight should not have reached target yet");
    }

    #[test]
    fn test_interpolation_completes() {
        let mut eval = VisemeEvaluator::with_lerp_speed(0.01);
        eval.set_target(&make_frame(VisemeCurve::I, 1.0));

        // Run many updates
        for _ in 0..200 {
            eval.update(16);
        }

        let w = eval.current_weights();
        assert!(
            (w.get(&VisemeCurve::I) - 1.0).abs() < EPSILON,
            "should reach target after enough updates"
        );
    }

    #[test]
    fn test_set_rest_returns_to_rest() {
        let mut eval = VisemeEvaluator::new();
        eval.set_target(&make_frame(VisemeCurve::A, 1.0));
        eval.snap_to_target();

        eval.set_rest();
        eval.snap_to_target();
        let w = eval.current_weights();
        assert!((w.get(&VisemeCurve::Rest) - 1.0).abs() < EPSILON);
        assert!((w.get(&VisemeCurve::A)).abs() < EPSILON);
    }

    #[test]
    fn test_amplitude_clamped() {
        let mut eval = VisemeEvaluator::new();
        eval.set_target(&make_frame(VisemeCurve::U, 2.0)); // over 1.0
        eval.snap_to_target();
        let w = eval.current_weights();
        assert!(w.get(&VisemeCurve::U) <= 1.0 + EPSILON);
    }

    #[test]
    fn test_transition_between_visemes() {
        let mut eval = VisemeEvaluator::new();

        // Set to A
        eval.set_target(&make_frame(VisemeCurve::A, 1.0));
        eval.snap_to_target();
        assert!((eval.current_weights().get(&VisemeCurve::A) - 1.0).abs() < EPSILON);

        // Transition to O
        eval.set_target(&make_frame(VisemeCurve::O, 1.0));
        eval.update(50);

        let w = eval.current_weights();
        // A should be decreasing, O increasing
        assert!(w.get(&VisemeCurve::A) < 1.0);
        assert!(w.get(&VisemeCurve::O) > 0.0);
    }

    #[test]
    fn test_weights_stay_non_negative() {
        let mut eval = VisemeEvaluator::with_lerp_speed(0.1);
        eval.set_target(&make_frame(VisemeCurve::A, 1.0));
        for _ in 0..100 {
            eval.update(16);
        }
        for w in eval.current_weights().weights.iter() {
            assert!(*w >= 0.0, "weight should be non-negative: {}", w);
        }
    }

    #[test]
    fn test_viseme_index_all_variants() {
        assert_eq!(viseme_index(&VisemeCurve::A), 0);
        assert_eq!(viseme_index(&VisemeCurve::E), 1);
        assert_eq!(viseme_index(&VisemeCurve::I), 2);
        assert_eq!(viseme_index(&VisemeCurve::O), 3);
        assert_eq!(viseme_index(&VisemeCurve::U), 4);
        assert_eq!(viseme_index(&VisemeCurve::F), 5);
        assert_eq!(viseme_index(&VisemeCurve::Rest), 6);
    }

    #[test]
    fn test_zero_weights() {
        let w = VisemeWeights::zero();
        for val in w.weights.iter() {
            assert!(val.abs() < EPSILON);
        }
    }

    #[test]
    fn test_rest_weights() {
        let w = VisemeWeights::rest();
        assert!((w.get(&VisemeCurve::Rest) - 1.0).abs() < EPSILON);
        assert!((w.get(&VisemeCurve::A)).abs() < EPSILON);
    }
}
