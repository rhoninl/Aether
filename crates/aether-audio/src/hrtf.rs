#[derive(Debug, Clone)]
pub struct HrtfSample {
    pub left_delay_ms: f32,
    pub right_delay_ms: f32,
    pub left_gain: f32,
    pub right_gain: f32,
    pub crosstalk_reduction_db: f32,
}

#[derive(Debug, Clone, Copy)]
pub enum HrtfProfile {
    Generic,
    EarShapeA,
    EarShapeB,
    EarShapeC,
}

impl HrtfProfile {
    pub fn has_head_tracking() -> bool {
        true
    }
}

impl HrtfSample {
    pub fn for_profile(profile: HrtfProfile, azimuth_deg: f32) -> Self {
        let spread = azimuth_deg.abs() / 180.0;
        let directional = (1.0 - (spread * 0.2)).clamp(0.0, 1.0);
        let crosstalk = match profile {
            HrtfProfile::Generic => 1.2,
            HrtfProfile::EarShapeA => 1.0,
            HrtfProfile::EarShapeB => 0.9,
            HrtfProfile::EarShapeC => 1.1,
        };
        Self {
            left_delay_ms: spread * 0.04,
            right_delay_ms: (1.0 - spread) * 0.04,
            left_gain: directional,
            right_gain: 1.0 - directional * 0.12,
            crosstalk_reduction_db: crosstalk,
        }
    }
}
