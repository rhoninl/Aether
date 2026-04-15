#[derive(Debug, Clone, Copy)]
pub struct DistanceBand {
    pub max_distance: f32,
    pub gain: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttenuationCurve {
    Linear,
    Inverse,
    Exponential,
}

#[derive(Debug, Clone, Copy)]
pub struct AttenuationModel {
    pub curve: AttenuationCurve,
    pub min_gain: f32,
    pub max_gain: f32,
    pub max_distance: f32,
}

impl AttenuationModel {
    pub fn from_preset_linear() -> Self {
        Self {
            curve: AttenuationCurve::Linear,
            min_gain: 0.1,
            max_gain: 1.0,
            max_distance: 100.0,
        }
    }

    pub fn from_preset_inverse() -> Self {
        Self {
            curve: AttenuationCurve::Inverse,
            min_gain: 0.02,
            max_gain: 1.0,
            max_distance: 200.0,
        }
    }

    pub fn gain(&self, distance: f32) -> f32 {
        let d = distance.max(0.0).min(self.max_distance);
        let t = if self.max_distance <= f32::EPSILON {
            0.0
        } else {
            d / self.max_distance
        };
        let g = match self.curve {
            AttenuationCurve::Linear => 1.0 - t,
            AttenuationCurve::Inverse => 1.0 / (1.0 + 4.0 * t),
            AttenuationCurve::Exponential => (1.0 - t).powf(2.0),
        };
        (g.clamp(self.min_gain, self.max_gain))
            .max(self.min_gain)
            .min(self.max_gain)
    }

    pub fn band(&self, distance: f32) -> DistanceBand {
        let normalized = if self.max_distance <= f32::EPSILON {
            1.0
        } else {
            distance / self.max_distance
        };
        if normalized <= 0.25 {
            DistanceBand {
                max_distance: self.max_distance,
                gain: 1.0,
            }
        } else if normalized <= 0.5 {
            DistanceBand {
                max_distance: self.max_distance,
                gain: 0.7,
            }
        } else if normalized <= 0.8 {
            DistanceBand {
                max_distance: self.max_distance,
                gain: 0.4,
            }
        } else {
            DistanceBand {
                max_distance: self.max_distance,
                gain: 0.15,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attenuation_is_continuous_near_zero_and_max_range() {
        let model = AttenuationModel::from_preset_linear();
        let n = model.gain(0.0);
        let m = model.gain(model.max_distance);
        assert_eq!(n, model.max_gain);
        assert!(m >= model.min_gain);
    }

    #[test]
    fn inverse_curve_decays_with_distance() {
        let model = AttenuationModel::from_preset_inverse();
        let near = model.gain(1.0);
        let far = model.gain(model.max_distance);
        assert!(near > far);
    }
}
