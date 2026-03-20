//! Throw detection via velocity tracking over recent frames.
//!
//! Tracks hand velocity samples in a ring buffer and computes a weighted
//! average at release time to estimate a natural throw velocity. More recent
//! samples are weighted more heavily via exponential decay.

use crate::vr::math;

/// Default number of velocity samples to keep in the ring buffer.
const DEFAULT_SAMPLE_COUNT: usize = 10;

/// Default weight decay factor per sample (newer = higher weight).
/// Weight for sample i (0=newest) = decay^i.
const DEFAULT_VELOCITY_WEIGHT_DECAY: f32 = 0.8;

/// Minimum number of samples needed for a valid throw estimate.
const MIN_SAMPLES_FOR_THROW: usize = 2;

/// Maximum throw velocity magnitude (m/s) to clamp unreasonable values.
const MAX_THROW_VELOCITY: f32 = 30.0;

/// A single velocity sample recorded from hand tracking.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VelocitySample {
    /// Linear velocity of the hand (m/s).
    pub velocity: [f32; 3],
    /// Angular velocity of the hand (rad/s).
    pub angular_velocity: [f32; 3],
    /// Timestamp when this sample was recorded (seconds).
    pub timestamp: f32,
}

impl Default for VelocitySample {
    fn default() -> Self {
        Self {
            velocity: [0.0; 3],
            angular_velocity: [0.0; 3],
            timestamp: 0.0,
        }
    }
}

/// Result of throw velocity estimation.
#[derive(Debug, Clone, PartialEq)]
pub struct ThrowResult {
    /// Estimated release linear velocity.
    pub linear_velocity: [f32; 3],
    /// Estimated release angular velocity.
    pub angular_velocity: [f32; 3],
    /// Confidence in the estimate (0 = no data, 1 = highly confident).
    pub confidence: f32,
    /// Speed (magnitude of linear velocity).
    pub speed: f32,
}

/// Configuration for the throw detector.
#[derive(Debug, Clone, PartialEq)]
pub struct ThrowDetectorConfig {
    /// Number of samples in the ring buffer.
    pub sample_count: usize,
    /// Weight decay factor per sample age.
    pub weight_decay: f32,
    /// Maximum allowed throw velocity magnitude.
    pub max_velocity: f32,
}

impl Default for ThrowDetectorConfig {
    fn default() -> Self {
        Self {
            sample_count: DEFAULT_SAMPLE_COUNT,
            weight_decay: DEFAULT_VELOCITY_WEIGHT_DECAY,
            max_velocity: MAX_THROW_VELOCITY,
        }
    }
}

impl ThrowDetectorConfig {
    pub fn with_sample_count(mut self, count: usize) -> Self {
        self.sample_count = count.max(MIN_SAMPLES_FOR_THROW);
        self
    }

    pub fn with_weight_decay(mut self, decay: f32) -> Self {
        self.weight_decay = decay.clamp(0.0, 1.0);
        self
    }

    pub fn with_max_velocity(mut self, max: f32) -> Self {
        self.max_velocity = max.max(0.0);
        self
    }
}

/// Tracks hand velocity samples and estimates throw release velocity.
#[derive(Debug)]
pub struct ThrowDetector {
    config: ThrowDetectorConfig,
    /// Ring buffer of velocity samples.
    samples: Vec<VelocitySample>,
    /// Write index in the ring buffer.
    write_index: usize,
    /// Total number of samples recorded (can exceed buffer size).
    total_recorded: usize,
}

impl ThrowDetector {
    /// Create a new throw detector with default configuration.
    pub fn new() -> Self {
        let config = ThrowDetectorConfig::default();
        let capacity = config.sample_count;
        Self {
            config,
            samples: vec![VelocitySample::default(); capacity],
            write_index: 0,
            total_recorded: 0,
        }
    }

    /// Create a throw detector with custom configuration.
    pub fn with_config(config: ThrowDetectorConfig) -> Self {
        let capacity = config.sample_count;
        Self {
            config,
            samples: vec![VelocitySample::default(); capacity],
            write_index: 0,
            total_recorded: 0,
        }
    }

    /// Get the configuration.
    pub fn config(&self) -> &ThrowDetectorConfig {
        &self.config
    }

    /// Number of valid samples currently in the buffer.
    pub fn sample_count(&self) -> usize {
        self.total_recorded.min(self.config.sample_count)
    }

    /// Record a new velocity sample.
    pub fn record_sample(
        &mut self,
        velocity: [f32; 3],
        angular_velocity: [f32; 3],
        timestamp: f32,
    ) {
        self.samples[self.write_index] = VelocitySample {
            velocity,
            angular_velocity,
            timestamp,
        };
        self.write_index = (self.write_index + 1) % self.config.sample_count;
        self.total_recorded += 1;
    }

    /// Estimate the release velocity using a weighted average of recent samples.
    ///
    /// Returns `None` if not enough samples have been recorded.
    pub fn estimate_release_velocity(&self) -> Option<ThrowResult> {
        let count = self.sample_count();
        if count < MIN_SAMPLES_FOR_THROW {
            return None;
        }

        let mut weighted_linear = [0.0f32; 3];
        let mut weighted_angular = [0.0f32; 3];
        let mut total_weight = 0.0f32;

        // Iterate from newest to oldest
        for i in 0..count {
            // Index of the (i+1)th newest sample
            let idx = if self.write_index == 0 && i == 0 {
                self.config.sample_count - 1
            } else {
                (self.write_index + self.config.sample_count - 1 - i) % self.config.sample_count
            };

            let weight = self.config.weight_decay.powi(i as i32);
            let sample = &self.samples[idx];

            weighted_linear[0] += sample.velocity[0] * weight;
            weighted_linear[1] += sample.velocity[1] * weight;
            weighted_linear[2] += sample.velocity[2] * weight;

            weighted_angular[0] += sample.angular_velocity[0] * weight;
            weighted_angular[1] += sample.angular_velocity[1] * weight;
            weighted_angular[2] += sample.angular_velocity[2] * weight;

            total_weight += weight;
        }

        if total_weight < f32::EPSILON {
            return None;
        }

        let linear_velocity = [
            weighted_linear[0] / total_weight,
            weighted_linear[1] / total_weight,
            weighted_linear[2] / total_weight,
        ];

        let angular_velocity = [
            weighted_angular[0] / total_weight,
            weighted_angular[1] / total_weight,
            weighted_angular[2] / total_weight,
        ];

        // Clamp velocity magnitude
        let speed = math::length(linear_velocity);
        let clamped_linear = if speed > self.config.max_velocity {
            let factor = self.config.max_velocity / speed;
            math::scale(linear_velocity, factor)
        } else {
            linear_velocity
        };

        let clamped_speed = math::length(clamped_linear);

        // Confidence based on sample count and consistency
        let confidence = (count as f32 / self.config.sample_count as f32).min(1.0);

        Some(ThrowResult {
            linear_velocity: clamped_linear,
            angular_velocity,
            confidence,
            speed: clamped_speed,
        })
    }

    /// Clear all recorded samples.
    pub fn clear(&mut self) {
        self.samples
            .iter_mut()
            .for_each(|s| *s = VelocitySample::default());
        self.write_index = 0;
        self.total_recorded = 0;
    }
}

impl Default for ThrowDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-4;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    // --- ThrowDetectorConfig tests ---

    #[test]
    fn config_defaults() {
        let c = ThrowDetectorConfig::default();
        assert_eq!(c.sample_count, DEFAULT_SAMPLE_COUNT);
        assert_eq!(c.weight_decay, DEFAULT_VELOCITY_WEIGHT_DECAY);
        assert_eq!(c.max_velocity, MAX_THROW_VELOCITY);
    }

    #[test]
    fn config_sample_count_minimum() {
        let c = ThrowDetectorConfig::default().with_sample_count(0);
        assert_eq!(c.sample_count, MIN_SAMPLES_FOR_THROW);
    }

    #[test]
    fn config_weight_decay_clamped() {
        let c1 = ThrowDetectorConfig::default().with_weight_decay(-0.5);
        assert_eq!(c1.weight_decay, 0.0);

        let c2 = ThrowDetectorConfig::default().with_weight_decay(1.5);
        assert_eq!(c2.weight_decay, 1.0);
    }

    #[test]
    fn config_max_velocity() {
        let c = ThrowDetectorConfig::default().with_max_velocity(50.0);
        assert_eq!(c.max_velocity, 50.0);
    }

    // --- VelocitySample tests ---

    #[test]
    fn velocity_sample_default() {
        let s = VelocitySample::default();
        assert_eq!(s.velocity, [0.0; 3]);
        assert_eq!(s.angular_velocity, [0.0; 3]);
        assert_eq!(s.timestamp, 0.0);
    }

    // --- ThrowDetector tests ---

    #[test]
    fn detector_default_empty() {
        let d = ThrowDetector::new();
        assert_eq!(d.sample_count(), 0);
    }

    #[test]
    fn record_samples_increments_count() {
        let mut d = ThrowDetector::new();
        d.record_sample([1.0, 0.0, 0.0], [0.0; 3], 0.0);
        assert_eq!(d.sample_count(), 1);
        d.record_sample([2.0, 0.0, 0.0], [0.0; 3], 0.016);
        assert_eq!(d.sample_count(), 2);
    }

    #[test]
    fn sample_count_caps_at_buffer_size() {
        let mut d = ThrowDetector::with_config(ThrowDetectorConfig::default().with_sample_count(3));

        for i in 0..10 {
            d.record_sample([i as f32, 0.0, 0.0], [0.0; 3], i as f32 * 0.016);
        }

        assert_eq!(d.sample_count(), 3);
    }

    #[test]
    fn estimate_returns_none_with_insufficient_samples() {
        let d = ThrowDetector::new();
        assert!(d.estimate_release_velocity().is_none());

        let mut d2 = ThrowDetector::new();
        d2.record_sample([1.0, 0.0, 0.0], [0.0; 3], 0.0);
        assert!(d2.estimate_release_velocity().is_none()); // only 1 sample < MIN_SAMPLES_FOR_THROW
    }

    #[test]
    fn estimate_constant_velocity() {
        let mut d = ThrowDetector::with_config(
            ThrowDetectorConfig::default()
                .with_sample_count(5)
                .with_weight_decay(1.0), // equal weights
        );

        for i in 0..5 {
            d.record_sample([5.0, 0.0, 0.0], [0.0, 1.0, 0.0], i as f32 * 0.016);
        }

        let result = d.estimate_release_velocity().unwrap();
        assert!(approx_eq(result.linear_velocity[0], 5.0));
        assert!(approx_eq(result.linear_velocity[1], 0.0));
        assert!(approx_eq(result.linear_velocity[2], 0.0));
        assert!(approx_eq(result.angular_velocity[1], 1.0));
        assert!(approx_eq(result.speed, 5.0));
    }

    #[test]
    fn estimate_weighted_average_favors_recent() {
        let mut d = ThrowDetector::with_config(
            ThrowDetectorConfig::default()
                .with_sample_count(3)
                .with_weight_decay(0.5),
        );

        // Oldest: 0, Middle: 0, Newest: 10
        d.record_sample([0.0, 0.0, 0.0], [0.0; 3], 0.0);
        d.record_sample([0.0, 0.0, 0.0], [0.0; 3], 0.016);
        d.record_sample([10.0, 0.0, 0.0], [0.0; 3], 0.032);

        let result = d.estimate_release_velocity().unwrap();
        // Weights: newest=1.0, middle=0.5, oldest=0.25, total=1.75
        // Weighted avg = (10*1.0 + 0*0.5 + 0*0.25) / 1.75 = 10/1.75 ~= 5.714
        assert!(
            result.linear_velocity[0] > 5.0,
            "Should favor recent sample"
        );
    }

    #[test]
    fn estimate_clamps_max_velocity() {
        let mut d = ThrowDetector::with_config(
            ThrowDetectorConfig::default()
                .with_sample_count(3)
                .with_max_velocity(5.0),
        );

        d.record_sample([100.0, 0.0, 0.0], [0.0; 3], 0.0);
        d.record_sample([100.0, 0.0, 0.0], [0.0; 3], 0.016);
        d.record_sample([100.0, 0.0, 0.0], [0.0; 3], 0.032);

        let result = d.estimate_release_velocity().unwrap();
        assert!(
            result.speed <= 5.0 + EPSILON,
            "Speed should be clamped to max: {}",
            result.speed,
        );
    }

    #[test]
    fn estimate_confidence_scales_with_samples() {
        let mut d =
            ThrowDetector::with_config(ThrowDetectorConfig::default().with_sample_count(10));

        d.record_sample([1.0, 0.0, 0.0], [0.0; 3], 0.0);
        d.record_sample([1.0, 0.0, 0.0], [0.0; 3], 0.016);
        let result_low = d.estimate_release_velocity().unwrap();

        for i in 2..10 {
            d.record_sample([1.0, 0.0, 0.0], [0.0; 3], i as f32 * 0.016);
        }
        let result_high = d.estimate_release_velocity().unwrap();

        assert!(result_high.confidence > result_low.confidence);
        assert!(approx_eq(result_high.confidence, 1.0));
    }

    #[test]
    fn clear_resets_state() {
        let mut d = ThrowDetector::new();
        d.record_sample([1.0, 0.0, 0.0], [0.0; 3], 0.0);
        d.record_sample([1.0, 0.0, 0.0], [0.0; 3], 0.016);
        d.record_sample([1.0, 0.0, 0.0], [0.0; 3], 0.032);

        d.clear();
        assert_eq!(d.sample_count(), 0);
        assert!(d.estimate_release_velocity().is_none());
    }

    #[test]
    fn ring_buffer_wraps_correctly() {
        let mut d = ThrowDetector::with_config(
            ThrowDetectorConfig::default()
                .with_sample_count(3)
                .with_weight_decay(1.0), // equal weights
        );

        // Fill buffer
        d.record_sample([1.0, 0.0, 0.0], [0.0; 3], 0.0);
        d.record_sample([2.0, 0.0, 0.0], [0.0; 3], 0.016);
        d.record_sample([3.0, 0.0, 0.0], [0.0; 3], 0.032);

        // Overwrite oldest
        d.record_sample([10.0, 0.0, 0.0], [0.0; 3], 0.048);

        let result = d.estimate_release_velocity().unwrap();
        // Buffer now contains: [10, 2, 3] with 10 being newest (and wrapping)
        // Equal weights: avg = (10 + 3 + 2) / 3 = 5.0
        assert!(approx_eq(result.linear_velocity[0], 5.0));
    }

    #[test]
    fn zero_velocity_throw() {
        let mut d = ThrowDetector::new();
        d.record_sample([0.0; 3], [0.0; 3], 0.0);
        d.record_sample([0.0; 3], [0.0; 3], 0.016);

        let result = d.estimate_release_velocity().unwrap();
        assert!(approx_eq(result.speed, 0.0));
        assert!(approx_eq(result.linear_velocity[0], 0.0));
    }

    #[test]
    fn angular_velocity_tracked() {
        let mut d = ThrowDetector::with_config(
            ThrowDetectorConfig::default()
                .with_sample_count(3)
                .with_weight_decay(1.0),
        );

        d.record_sample([0.0; 3], [0.0, 5.0, 0.0], 0.0);
        d.record_sample([0.0; 3], [0.0, 5.0, 0.0], 0.016);
        d.record_sample([0.0; 3], [0.0, 5.0, 0.0], 0.032);

        let result = d.estimate_release_velocity().unwrap();
        assert!(approx_eq(result.angular_velocity[1], 5.0));
    }

    #[test]
    fn three_dimensional_throw() {
        let mut d = ThrowDetector::with_config(
            ThrowDetectorConfig::default()
                .with_sample_count(3)
                .with_weight_decay(1.0),
        );

        d.record_sample([3.0, 4.0, 0.0], [0.0; 3], 0.0);
        d.record_sample([3.0, 4.0, 0.0], [0.0; 3], 0.016);
        d.record_sample([3.0, 4.0, 0.0], [0.0; 3], 0.032);

        let result = d.estimate_release_velocity().unwrap();
        // speed = sqrt(9+16) = 5
        assert!(approx_eq(result.speed, 5.0));
    }
}
