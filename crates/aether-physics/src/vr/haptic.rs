//! Haptic feedback mapping from physics events to vibration intensity.
//!
//! Maps collision forces and contact events to haptic vibration parameters
//! suitable for VR controller output. Supports multiple mapping curves.

/// Minimum collision force (Newtons) that triggers haptic feedback.
const DEFAULT_HAPTIC_MIN_FORCE: f32 = 0.1;

/// Maximum collision force (Newtons) for haptic mapping (maps to intensity 1.0).
const DEFAULT_HAPTIC_MAX_FORCE: f32 = 50.0;

/// Default haptic pulse duration in seconds.
const DEFAULT_HAPTIC_DURATION: f32 = 0.05;

/// Maximum haptic intensity (clamped).
const MAX_INTENSITY: f32 = 1.0;

/// Minimum haptic intensity (below this, no feedback).
const MIN_INTENSITY: f32 = 0.0;

/// Which hand is receiving the haptic event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hand {
    Left,
    Right,
}

/// Curve type for mapping force magnitude to vibration intensity.
#[derive(Debug, Clone, PartialEq)]
pub enum HapticCurve {
    /// Linear mapping from min_force..max_force to 0..1.
    Linear,
    /// Quadratic (ease-in) mapping. Low forces produce low feedback, ramps up fast.
    Quadratic,
    /// Square root (ease-out) mapping. Low forces already produce noticeable feedback.
    SquareRoot,
    /// Step function: below threshold = 0, at or above = fixed intensity.
    Step { threshold: f32, intensity: f32 },
}

/// Configuration for the haptic feedback mapper.
#[derive(Debug, Clone, PartialEq)]
pub struct HapticFeedbackConfig {
    /// Minimum force to trigger any haptic feedback.
    pub min_force: f32,
    /// Force that maps to maximum intensity (1.0).
    pub max_force: f32,
    /// Default duration of haptic pulses in seconds.
    pub default_duration: f32,
    /// The mapping curve from force to intensity.
    pub curve: HapticCurve,
}

impl Default for HapticFeedbackConfig {
    fn default() -> Self {
        Self {
            min_force: DEFAULT_HAPTIC_MIN_FORCE,
            max_force: DEFAULT_HAPTIC_MAX_FORCE,
            default_duration: DEFAULT_HAPTIC_DURATION,
            curve: HapticCurve::Linear,
        }
    }
}

impl HapticFeedbackConfig {
    /// Create a config with a specific curve.
    pub fn with_curve(mut self, curve: HapticCurve) -> Self {
        self.curve = curve;
        self
    }

    /// Set the force range.
    pub fn with_force_range(mut self, min: f32, max: f32) -> Self {
        self.min_force = min;
        self.max_force = max;
        self
    }

    /// Set the default pulse duration.
    pub fn with_duration(mut self, duration: f32) -> Self {
        self.default_duration = duration;
        self
    }
}

/// A haptic event to be sent to a VR controller.
#[derive(Debug, Clone, PartialEq)]
pub struct HapticEvent {
    /// Which hand receives the haptic feedback.
    pub hand: Hand,
    /// Vibration intensity in [0, 1].
    pub intensity: f32,
    /// Duration of the vibration in seconds.
    pub duration: f32,
}

/// Maps physics collision forces to haptic feedback events.
#[derive(Debug, Clone)]
pub struct HapticFeedbackMapper {
    config: HapticFeedbackConfig,
}

impl HapticFeedbackMapper {
    /// Create a mapper with default configuration.
    pub fn new() -> Self {
        Self {
            config: HapticFeedbackConfig::default(),
        }
    }

    /// Create a mapper with a specific configuration.
    pub fn with_config(config: HapticFeedbackConfig) -> Self {
        Self { config }
    }

    /// Get the current configuration.
    pub fn config(&self) -> &HapticFeedbackConfig {
        &self.config
    }

    /// Map a collision force magnitude to a haptic event.
    ///
    /// Returns `None` if the force is below the minimum threshold.
    pub fn map_collision_force(&self, hand: Hand, force_magnitude: f32) -> Option<HapticEvent> {
        if force_magnitude < self.config.min_force {
            return None;
        }

        let intensity = self.compute_intensity(force_magnitude);

        if intensity <= MIN_INTENSITY {
            return None;
        }

        Some(HapticEvent {
            hand,
            intensity,
            duration: self.config.default_duration,
        })
    }

    /// Map a collision force to a haptic event with a custom duration.
    pub fn map_collision_force_with_duration(
        &self,
        hand: Hand,
        force_magnitude: f32,
        duration: f32,
    ) -> Option<HapticEvent> {
        self.map_collision_force(hand, force_magnitude)
            .map(|mut event| {
                event.duration = duration;
                event
            })
    }

    /// Compute haptic intensity from force magnitude using the configured curve.
    pub fn compute_intensity(&self, force_magnitude: f32) -> f32 {
        let range = self.config.max_force - self.config.min_force;
        if range <= 0.0 {
            return if force_magnitude >= self.config.min_force {
                MAX_INTENSITY
            } else {
                MIN_INTENSITY
            };
        }

        match &self.config.curve {
            HapticCurve::Linear => {
                let t = (force_magnitude - self.config.min_force) / range;
                clamp_intensity(t)
            }
            HapticCurve::Quadratic => {
                let t = (force_magnitude - self.config.min_force) / range;
                clamp_intensity(t * t)
            }
            HapticCurve::SquareRoot => {
                let t = (force_magnitude - self.config.min_force) / range;
                let t_clamped = t.clamp(0.0, 1.0);
                clamp_intensity(t_clamped.sqrt())
            }
            HapticCurve::Step {
                threshold,
                intensity,
            } => {
                if force_magnitude >= *threshold {
                    clamp_intensity(*intensity)
                } else {
                    MIN_INTENSITY
                }
            }
        }
    }

    /// Generate a continuous haptic event (e.g., for sustained contact).
    /// Intensity scales with the force applied over the duration.
    pub fn map_sustained_contact(
        &self,
        hand: Hand,
        force_magnitude: f32,
        contact_duration: f32,
    ) -> Option<HapticEvent> {
        if force_magnitude < self.config.min_force {
            return None;
        }

        let base_intensity = self.compute_intensity(force_magnitude);
        // Sustained contacts have slightly reduced intensity over time
        let decay_factor = 1.0 / (1.0 + contact_duration * 2.0);
        let intensity = clamp_intensity(base_intensity * decay_factor);

        if intensity <= MIN_INTENSITY {
            return None;
        }

        Some(HapticEvent {
            hand,
            intensity,
            duration: self.config.default_duration,
        })
    }
}

impl Default for HapticFeedbackMapper {
    fn default() -> Self {
        Self::new()
    }
}

fn clamp_intensity(value: f32) -> f32 {
    value.clamp(MIN_INTENSITY, MAX_INTENSITY)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-5;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    // --- HapticFeedbackConfig tests ---

    #[test]
    fn config_defaults() {
        let c = HapticFeedbackConfig::default();
        assert_eq!(c.min_force, DEFAULT_HAPTIC_MIN_FORCE);
        assert_eq!(c.max_force, DEFAULT_HAPTIC_MAX_FORCE);
        assert_eq!(c.default_duration, DEFAULT_HAPTIC_DURATION);
        assert_eq!(c.curve, HapticCurve::Linear);
    }

    #[test]
    fn config_with_curve() {
        let c = HapticFeedbackConfig::default().with_curve(HapticCurve::Quadratic);
        assert_eq!(c.curve, HapticCurve::Quadratic);
    }

    #[test]
    fn config_with_force_range() {
        let c = HapticFeedbackConfig::default().with_force_range(1.0, 100.0);
        assert_eq!(c.min_force, 1.0);
        assert_eq!(c.max_force, 100.0);
    }

    #[test]
    fn config_with_duration() {
        let c = HapticFeedbackConfig::default().with_duration(0.1);
        assert_eq!(c.default_duration, 0.1);
    }

    // --- HapticFeedbackMapper - Linear curve tests ---

    #[test]
    fn linear_below_min_returns_none() {
        let mapper = HapticFeedbackMapper::new();
        let result = mapper.map_collision_force(Hand::Left, 0.05);
        assert!(result.is_none());
    }

    #[test]
    fn linear_at_min_returns_zero_intensity() {
        let mapper = HapticFeedbackMapper::new();
        let result = mapper.map_collision_force(Hand::Left, DEFAULT_HAPTIC_MIN_FORCE);
        // At min_force, t=0, intensity=0 -> returns None
        assert!(result.is_none());
    }

    #[test]
    fn linear_at_max_returns_full_intensity() {
        let mapper = HapticFeedbackMapper::new();
        let result = mapper.map_collision_force(Hand::Right, DEFAULT_HAPTIC_MAX_FORCE);
        assert!(result.is_some());
        let event = result.unwrap();
        assert!(approx_eq(event.intensity, 1.0));
        assert_eq!(event.hand, Hand::Right);
        assert_eq!(event.duration, DEFAULT_HAPTIC_DURATION);
    }

    #[test]
    fn linear_mid_range() {
        let mapper = HapticFeedbackMapper::with_config(
            HapticFeedbackConfig::default().with_force_range(0.0, 100.0),
        );
        let result = mapper.map_collision_force(Hand::Left, 50.0);
        assert!(result.is_some());
        assert!(approx_eq(result.unwrap().intensity, 0.5));
    }

    #[test]
    fn linear_above_max_clamps_to_one() {
        let mapper = HapticFeedbackMapper::new();
        let result = mapper.map_collision_force(Hand::Left, 1000.0);
        assert!(result.is_some());
        assert!(approx_eq(result.unwrap().intensity, 1.0));
    }

    // --- Quadratic curve tests ---

    #[test]
    fn quadratic_mid_range() {
        let mapper = HapticFeedbackMapper::with_config(
            HapticFeedbackConfig::default()
                .with_curve(HapticCurve::Quadratic)
                .with_force_range(0.0, 100.0),
        );
        let result = mapper.map_collision_force(Hand::Left, 50.0);
        assert!(result.is_some());
        // t=0.5, quadratic: 0.5^2 = 0.25
        assert!(approx_eq(result.unwrap().intensity, 0.25));
    }

    #[test]
    fn quadratic_at_max() {
        let mapper = HapticFeedbackMapper::with_config(
            HapticFeedbackConfig::default()
                .with_curve(HapticCurve::Quadratic)
                .with_force_range(0.0, 100.0),
        );
        let result = mapper.map_collision_force(Hand::Left, 100.0);
        assert!(result.is_some());
        assert!(approx_eq(result.unwrap().intensity, 1.0));
    }

    // --- SquareRoot curve tests ---

    #[test]
    fn sqrt_mid_range() {
        let mapper = HapticFeedbackMapper::with_config(
            HapticFeedbackConfig::default()
                .with_curve(HapticCurve::SquareRoot)
                .with_force_range(0.0, 100.0),
        );
        let result = mapper.map_collision_force(Hand::Left, 25.0);
        assert!(result.is_some());
        // t=0.25, sqrt(0.25)=0.5
        assert!(approx_eq(result.unwrap().intensity, 0.5));
    }

    // --- Step curve tests ---

    #[test]
    fn step_below_threshold() {
        let mapper = HapticFeedbackMapper::with_config(
            HapticFeedbackConfig::default()
                .with_curve(HapticCurve::Step {
                    threshold: 10.0,
                    intensity: 0.8,
                })
                .with_force_range(0.0, 100.0),
        );
        let result = mapper.map_collision_force(Hand::Left, 5.0);
        assert!(result.is_none());
    }

    #[test]
    fn step_at_threshold() {
        let mapper = HapticFeedbackMapper::with_config(
            HapticFeedbackConfig::default()
                .with_curve(HapticCurve::Step {
                    threshold: 10.0,
                    intensity: 0.8,
                })
                .with_force_range(0.0, 100.0),
        );
        let result = mapper.map_collision_force(Hand::Left, 10.0);
        assert!(result.is_some());
        assert!(approx_eq(result.unwrap().intensity, 0.8));
    }

    // --- Custom duration tests ---

    #[test]
    fn custom_duration() {
        let mapper = HapticFeedbackMapper::new();
        let result =
            mapper.map_collision_force_with_duration(Hand::Right, DEFAULT_HAPTIC_MAX_FORCE, 0.2);
        assert!(result.is_some());
        assert_eq!(result.unwrap().duration, 0.2);
    }

    #[test]
    fn custom_duration_below_threshold_still_none() {
        let mapper = HapticFeedbackMapper::new();
        let result = mapper.map_collision_force_with_duration(Hand::Left, 0.01, 0.2);
        assert!(result.is_none());
    }

    // --- Sustained contact tests ---

    #[test]
    fn sustained_contact_decays_over_time() {
        let mapper = HapticFeedbackMapper::with_config(
            HapticFeedbackConfig::default().with_force_range(0.0, 100.0),
        );

        let event_t0 = mapper.map_sustained_contact(Hand::Left, 50.0, 0.0);
        let event_t1 = mapper.map_sustained_contact(Hand::Left, 50.0, 1.0);

        assert!(event_t0.is_some());
        assert!(event_t1.is_some());
        assert!(event_t0.unwrap().intensity > event_t1.unwrap().intensity);
    }

    #[test]
    fn sustained_contact_below_min_returns_none() {
        let mapper = HapticFeedbackMapper::new();
        let result = mapper.map_sustained_contact(Hand::Left, 0.01, 0.0);
        assert!(result.is_none());
    }

    // --- Edge cases ---

    #[test]
    fn zero_range_config() {
        let mapper = HapticFeedbackMapper::with_config(
            HapticFeedbackConfig::default().with_force_range(10.0, 10.0),
        );
        // At or above min with zero range -> max intensity
        let result = mapper.map_collision_force(Hand::Left, 10.0);
        assert!(result.is_some());
        assert!(approx_eq(result.unwrap().intensity, 1.0));
    }

    #[test]
    fn negative_force_returns_none() {
        let mapper = HapticFeedbackMapper::new();
        let result = mapper.map_collision_force(Hand::Left, -5.0);
        assert!(result.is_none());
    }

    #[test]
    fn hand_enum_equality() {
        assert_eq!(Hand::Left, Hand::Left);
        assert_ne!(Hand::Left, Hand::Right);
    }

    #[test]
    fn compute_intensity_directly() {
        let mapper = HapticFeedbackMapper::with_config(
            HapticFeedbackConfig::default().with_force_range(0.0, 10.0),
        );
        assert!(approx_eq(mapper.compute_intensity(0.0), 0.0));
        assert!(approx_eq(mapper.compute_intensity(5.0), 0.5));
        assert!(approx_eq(mapper.compute_intensity(10.0), 1.0));
        assert!(approx_eq(mapper.compute_intensity(20.0), 1.0)); // clamped
    }
}
