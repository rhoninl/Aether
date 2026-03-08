//! Dead zone filtering and sensitivity curve processing for analog inputs.

/// Shape of the dead zone region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeadZoneShape {
    /// Circular dead zone based on magnitude of (x, y).
    Circular,
    /// Square (per-axis) dead zone applied independently to x and y.
    Square,
}

/// Configuration for thumbstick / analog axis dead zones.
#[derive(Debug, Clone, Copy)]
pub struct DeadZoneConfig {
    /// Values below this radius map to zero. Default: 0.1
    pub inner_radius: f32,
    /// Values above this radius map to 1.0. Default: 0.95
    pub outer_radius: f32,
    /// Shape of the dead zone.
    pub shape: DeadZoneShape,
}

impl Default for DeadZoneConfig {
    fn default() -> Self {
        Self {
            inner_radius: 0.1,
            outer_radius: 0.95,
            shape: DeadZoneShape::Circular,
        }
    }
}

/// Sensitivity response curve type.
#[derive(Debug, Clone)]
pub enum SensitivityCurve {
    /// Linear (identity): output = input.
    Linear,
    /// Quadratic: output = input^2 (preserves sign).
    Quadratic,
    /// Cubic: output = input^3.
    Cubic,
    /// S-curve: smooth ease-in/ease-out using 3t^2 - 2t^3.
    SCurve,
    /// Custom lookup table with linear interpolation between points.
    /// Points must be sorted by input value. Each point is (input, output) in [0, 1].
    Custom(Vec<(f32, f32)>),
}

/// Apply dead zone filtering to a 2D axis input.
///
/// Returns the remapped (x, y) after dead zone processing.
/// Both input and output values are in [-1.0, 1.0] range.
pub fn apply_dead_zone(x: f32, y: f32, config: &DeadZoneConfig) -> (f32, f32) {
    match config.shape {
        DeadZoneShape::Circular => apply_circular_dead_zone(x, y, config),
        DeadZoneShape::Square => apply_square_dead_zone(x, y, config),
    }
}

fn apply_circular_dead_zone(x: f32, y: f32, config: &DeadZoneConfig) -> (f32, f32) {
    let magnitude = (x * x + y * y).sqrt();

    if magnitude <= config.inner_radius {
        return (0.0, 0.0);
    }

    if magnitude >= config.outer_radius {
        // Normalize to unit circle, clamped
        let scale = 1.0 / magnitude;
        return (x * scale, y * scale);
    }

    // Linear remap between inner and outer
    let range = config.outer_radius - config.inner_radius;
    if range <= 0.0 {
        return (0.0, 0.0);
    }

    let remapped_magnitude = (magnitude - config.inner_radius) / range;
    let scale = remapped_magnitude / magnitude;
    (x * scale, y * scale)
}

fn apply_square_dead_zone(x: f32, y: f32, config: &DeadZoneConfig) -> (f32, f32) {
    let remap_axis = |v: f32| -> f32 {
        let abs_v = v.abs();
        if abs_v <= config.inner_radius {
            return 0.0;
        }
        if abs_v >= config.outer_radius {
            return v.signum();
        }
        let range = config.outer_radius - config.inner_radius;
        if range <= 0.0 {
            return 0.0;
        }
        let remapped = (abs_v - config.inner_radius) / range;
        remapped * v.signum()
    };

    (remap_axis(x), remap_axis(y))
}

/// Apply a sensitivity curve to a single axis value in [-1.0, 1.0].
///
/// The sign is preserved; the curve is applied to the absolute value.
pub fn apply_sensitivity(value: f32, curve: &SensitivityCurve) -> f32 {
    let sign = value.signum();
    let abs_val = value.abs().clamp(0.0, 1.0);

    let result = match curve {
        SensitivityCurve::Linear => abs_val,
        SensitivityCurve::Quadratic => abs_val * abs_val,
        SensitivityCurve::Cubic => abs_val * abs_val * abs_val,
        SensitivityCurve::SCurve => {
            // Hermite interpolation: 3t^2 - 2t^3
            let t = abs_val;
            3.0 * t * t - 2.0 * t * t * t
        }
        SensitivityCurve::Custom(points) => interpolate_custom(abs_val, points),
    };

    result * sign
}

fn interpolate_custom(input: f32, points: &[(f32, f32)]) -> f32 {
    if points.is_empty() {
        return input;
    }
    if points.len() == 1 {
        return points[0].1;
    }

    // Clamp to first/last point
    if input <= points[0].0 {
        return points[0].1;
    }
    if input >= points[points.len() - 1].0 {
        return points[points.len() - 1].1;
    }

    // Find the two surrounding points and interpolate
    for window in points.windows(2) {
        let (x0, y0) = window[0];
        let (x1, y1) = window[1];
        if input >= x0 && input <= x1 {
            let range = x1 - x0;
            if range <= 0.0 {
                return y0;
            }
            let t = (input - x0) / range;
            return y0 + t * (y1 - y0);
        }
    }

    // Fallback (shouldn't reach here with sorted points)
    input
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-5;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    // ---- Dead zone tests ----

    #[test]
    fn circular_dead_zone_inside_inner_returns_zero() {
        let config = DeadZoneConfig {
            inner_radius: 0.2,
            outer_radius: 0.9,
            shape: DeadZoneShape::Circular,
        };
        let (x, y) = apply_dead_zone(0.1, 0.1, &config);
        assert!(approx_eq(x, 0.0), "x={x}");
        assert!(approx_eq(y, 0.0), "y={y}");
    }

    #[test]
    fn circular_dead_zone_at_origin_returns_zero() {
        let config = DeadZoneConfig::default();
        let (x, y) = apply_dead_zone(0.0, 0.0, &config);
        assert!(approx_eq(x, 0.0));
        assert!(approx_eq(y, 0.0));
    }

    #[test]
    fn circular_dead_zone_outside_outer_returns_unit() {
        let config = DeadZoneConfig {
            inner_radius: 0.1,
            outer_radius: 0.9,
            shape: DeadZoneShape::Circular,
        };
        // magnitude = 1.0, which is > 0.9
        let (x, y) = apply_dead_zone(1.0, 0.0, &config);
        assert!(approx_eq(x, 1.0), "x={x}");
        assert!(approx_eq(y, 0.0), "y={y}");
    }

    #[test]
    fn circular_dead_zone_diagonal_outside_outer() {
        let config = DeadZoneConfig {
            inner_radius: 0.1,
            outer_radius: 0.9,
            shape: DeadZoneShape::Circular,
        };
        // magnitude = sqrt(0.5+0.5) = 1.0
        let (x, y) = apply_dead_zone(0.707, 0.707, &config);
        // Should be normalized to unit circle direction
        let mag = (x * x + y * y).sqrt();
        assert!(approx_eq(mag, 1.0), "mag={mag}");
    }

    #[test]
    fn circular_dead_zone_mid_range_interpolates() {
        let config = DeadZoneConfig {
            inner_radius: 0.0,
            outer_radius: 1.0,
            shape: DeadZoneShape::Circular,
        };
        // With inner=0, outer=1, mid-point 0.5 should map to 0.5 magnitude
        let (x, y) = apply_dead_zone(0.5, 0.0, &config);
        assert!(approx_eq(x, 0.5), "x={x}");
        assert!(approx_eq(y, 0.0), "y={y}");
    }

    #[test]
    fn square_dead_zone_inside_inner_returns_zero() {
        let config = DeadZoneConfig {
            inner_radius: 0.2,
            outer_radius: 0.9,
            shape: DeadZoneShape::Square,
        };
        let (x, y) = apply_dead_zone(0.15, -0.1, &config);
        assert!(approx_eq(x, 0.0), "x={x}");
        assert!(approx_eq(y, 0.0), "y={y}");
    }

    #[test]
    fn square_dead_zone_outside_outer_returns_signed_one() {
        let config = DeadZoneConfig {
            inner_radius: 0.1,
            outer_radius: 0.9,
            shape: DeadZoneShape::Square,
        };
        let (x, y) = apply_dead_zone(1.0, -1.0, &config);
        assert!(approx_eq(x, 1.0), "x={x}");
        assert!(approx_eq(y, -1.0), "y={y}");
    }

    #[test]
    fn square_dead_zone_mid_range_per_axis() {
        let config = DeadZoneConfig {
            inner_radius: 0.0,
            outer_radius: 1.0,
            shape: DeadZoneShape::Square,
        };
        let (x, y) = apply_dead_zone(0.5, -0.3, &config);
        assert!(approx_eq(x, 0.5), "x={x}");
        assert!(approx_eq(y, -0.3), "y={y}");
    }

    #[test]
    fn square_dead_zone_negative_axis() {
        let config = DeadZoneConfig {
            inner_radius: 0.2,
            outer_radius: 0.8,
            shape: DeadZoneShape::Square,
        };
        let (x, _y) = apply_dead_zone(-0.5, 0.0, &config);
        // -0.5 abs = 0.5, range = 0.6, remapped = (0.5-0.2)/0.6 = 0.5
        let expected = -((0.5 - 0.2) / (0.8 - 0.2));
        assert!(approx_eq(x, expected), "x={x}, expected={expected}");
    }

    // ---- Sensitivity curve tests ----

    #[test]
    fn linear_sensitivity_is_identity() {
        let curve = SensitivityCurve::Linear;
        assert!(approx_eq(apply_sensitivity(0.0, &curve), 0.0));
        assert!(approx_eq(apply_sensitivity(0.5, &curve), 0.5));
        assert!(approx_eq(apply_sensitivity(1.0, &curve), 1.0));
        assert!(approx_eq(apply_sensitivity(-0.7, &curve), -0.7));
    }

    #[test]
    fn quadratic_sensitivity_squares_input() {
        let curve = SensitivityCurve::Quadratic;
        assert!(approx_eq(apply_sensitivity(0.5, &curve), 0.25));
        assert!(approx_eq(apply_sensitivity(1.0, &curve), 1.0));
        assert!(approx_eq(apply_sensitivity(-0.5, &curve), -0.25));
    }

    #[test]
    fn cubic_sensitivity_cubes_input() {
        let curve = SensitivityCurve::Cubic;
        assert!(approx_eq(apply_sensitivity(0.5, &curve), 0.125));
        assert!(approx_eq(apply_sensitivity(1.0, &curve), 1.0));
        assert!(approx_eq(apply_sensitivity(-1.0, &curve), -1.0));
    }

    #[test]
    fn s_curve_endpoints_and_midpoint() {
        let curve = SensitivityCurve::SCurve;
        assert!(approx_eq(apply_sensitivity(0.0, &curve), 0.0));
        assert!(approx_eq(apply_sensitivity(1.0, &curve), 1.0));
        // At t=0.5: 3*(0.25) - 2*(0.125) = 0.75 - 0.25 = 0.5
        assert!(approx_eq(apply_sensitivity(0.5, &curve), 0.5));
    }

    #[test]
    fn s_curve_preserves_sign() {
        let curve = SensitivityCurve::SCurve;
        let pos = apply_sensitivity(0.3, &curve);
        let neg = apply_sensitivity(-0.3, &curve);
        assert!(pos > 0.0);
        assert!(neg < 0.0);
        assert!(approx_eq(pos, -neg));
    }

    #[test]
    fn custom_curve_interpolation() {
        let curve = SensitivityCurve::Custom(vec![
            (0.0, 0.0),
            (0.5, 0.8),
            (1.0, 1.0),
        ]);
        // At 0.25, interpolate between (0,0) and (0.5,0.8): t=0.5, output=0.4
        assert!(approx_eq(apply_sensitivity(0.25, &curve), 0.4));
        // At 0.0
        assert!(approx_eq(apply_sensitivity(0.0, &curve), 0.0));
        // At 1.0
        assert!(approx_eq(apply_sensitivity(1.0, &curve), 1.0));
    }

    #[test]
    fn custom_curve_empty_is_identity() {
        let curve = SensitivityCurve::Custom(vec![]);
        assert!(approx_eq(apply_sensitivity(0.5, &curve), 0.5));
    }

    #[test]
    fn custom_curve_single_point() {
        let curve = SensitivityCurve::Custom(vec![(0.5, 0.7)]);
        assert!(approx_eq(apply_sensitivity(0.3, &curve), 0.7));
    }

    #[test]
    fn sensitivity_clamps_input() {
        let curve = SensitivityCurve::Linear;
        // Values beyond 1.0 are clamped
        assert!(approx_eq(apply_sensitivity(1.5, &curve), 1.0));
        assert!(approx_eq(apply_sensitivity(-1.5, &curve), -1.0));
    }
}
