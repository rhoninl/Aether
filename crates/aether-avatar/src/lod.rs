#[derive(Debug, Clone, Copy)]
pub enum LodLevel {
    Near,
    Mid,
    Far,
    Cull,
}

#[derive(Debug, Clone)]
pub struct LodDistanceBand {
    pub level: LodLevel,
    pub min_distance_m: f32,
    pub max_distance_m: f32,
    pub bone_skip: u8,
    pub texture_downscale: u8,
}

#[derive(Debug, Clone)]
pub struct AvatarLodProfile {
    pub bands: Vec<LodDistanceBand>,
}

impl AvatarLodProfile {
    pub fn default_profile() -> Self {
        Self {
            bands: vec![
                LodDistanceBand {
                    level: LodLevel::Near,
                    min_distance_m: 0.0,
                    max_distance_m: 5.0,
                    bone_skip: 1,
                    texture_downscale: 1,
                },
                LodDistanceBand {
                    level: LodLevel::Mid,
                    min_distance_m: 5.0,
                    max_distance_m: 20.0,
                    bone_skip: 2,
                    texture_downscale: 2,
                },
                LodDistanceBand {
                    level: LodLevel::Far,
                    min_distance_m: 20.0,
                    max_distance_m: 80.0,
                    bone_skip: 4,
                    texture_downscale: 4,
                },
                LodDistanceBand {
                    level: LodLevel::Cull,
                    min_distance_m: 80.0,
                    max_distance_m: 1000.0,
                    bone_skip: 8,
                    texture_downscale: 8,
                },
            ],
        }
    }
}

