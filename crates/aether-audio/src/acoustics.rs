use crate::types::AudioLod;

#[derive(Debug, Clone, Copy)]
pub struct HrtfTransportParams {
    pub azimuth_deg: f32,
    pub elevation_deg: f32,
    pub distance_gain: f32,
    pub occlusion: f32,
    pub reflectivity: f32,
}

#[derive(Debug, Clone)]
pub struct RoomAcoustics {
    pub reverb_mix: f32,
    pub occlusion: f32,
    pub early_reflection_gain: f32,
    pub late_reverb_gain: f32,
    pub room_size_m2: f32,
}

impl RoomAcoustics {
    pub fn dry_gain(&self) -> f32 {
        (1.0 - self.reverb_mix).clamp(0.0, 1.0)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum OcclusionState {
    None,
    Thin,
    Thick,
}

#[derive(Debug, Clone)]
pub struct AcousticsProfile {
    pub room: RoomAcoustics,
    pub max_reflections: u32,
    pub directivity_curve: f32,
}

impl AcousticsProfile {
    pub fn offline() -> Self {
        Self {
            room: RoomAcoustics {
                reverb_mix: 0.15,
                occlusion: 0.2,
                early_reflection_gain: 0.05,
                late_reverb_gain: 0.08,
                room_size_m2: 32.0,
            },
            max_reflections: 12,
            directivity_curve: 0.5,
        }
    }

    pub fn voice_mode() -> Self {
        Self {
            room: RoomAcoustics {
                reverb_mix: 0.03,
                occlusion: 0.08,
                early_reflection_gain: 0.0,
                late_reverb_gain: 0.0,
                room_size_m2: 6.0,
            },
            max_reflections: 4,
            directivity_curve: 0.9,
        }
    }

    pub fn lod_for_distance(meters: f32) -> AudioLod {
        if meters < 2.0 {
            AudioLod::Near
        } else if meters < 8.0 {
            AudioLod::Mid
        } else if meters < 16.0 {
            AudioLod::Far
        } else {
            AudioLod::Distant
        }
    }
}
