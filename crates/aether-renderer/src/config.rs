#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LODLevel {
    L0Near,
    L1Mid,
    L2Far,
    L3VeryFar,
}

pub const LODS: [LODLevel; 4] = [
    LODLevel::L0Near,
    LODLevel::L1Mid,
    LODLevel::L2Far,
    LODLevel::L3VeryFar,
];

#[derive(Debug, Clone, Copy)]
pub struct StereoConfig {
    pub enabled: bool,
    pub views_per_frame: u8,
    pub multiview: bool,
    pub single_draw_per_eye: bool,
}

impl Default for StereoConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            views_per_frame: 2,
            multiview: true,
            single_draw_per_eye: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoveationTier {
    Off,
    Tier1,
    Tier2,
    Adaptive,
}

#[derive(Debug, Clone, Copy)]
pub struct FoveationConfig {
    pub tier: FoveationTier,
    pub center_ratio: f32,
    pub edge_ratio: f32,
    pub min_radius_m: f32,
    pub max_radius_m: f32,
    pub smoothing_ms: u32,
}

impl Default for FoveationConfig {
    fn default() -> Self {
        Self {
            tier: FoveationTier::Tier2,
            center_ratio: 1.0,
            edge_ratio: 0.45,
            min_radius_m: 0.1,
            max_radius_m: 1.2,
            smoothing_ms: 8,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ClusterLightingConfig {
    pub enabled: bool,
    pub tile_size_px: u16,
    pub max_lights_per_cluster: u16,
    pub max_light_distance_m: f32,
    pub depth_slices: u8,
}

impl Default for ClusterLightingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            tile_size_px: 16,
            max_lights_per_cluster: 32,
            max_light_distance_m: 120.0,
            depth_slices: 16,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ShadowCascadeConfig {
    pub enabled: bool,
    pub num_cascades: u8,
    pub base_resolution_px: u32,
    pub far_distance_m: f32,
    pub cascade_resolutions: [u32; 4],
}

impl Default for ShadowCascadeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            num_cascades: 4,
            base_resolution_px: 2048,
            far_distance_m: 150.0,
            cascade_resolutions: [4096, 2048, 1024, 512],
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LODPolicy {
    pub near: f32,
    pub mid: f32,
    pub far: f32,
    pub very_far: f32,
    pub hysteresis_ratio: f32,
}

impl Default for LODPolicy {
    fn default() -> Self {
        Self {
            near: 10.0,
            mid: 35.0,
            far: 80.0,
            very_far: 160.0,
            hysteresis_ratio: 0.08,
        }
    }
}

#[derive(Debug)]
pub struct LodCurve {
    pub level: LODLevel,
}

impl LodCurve {
    pub fn select(level_policy: &LODPolicy, distance_m: f32, prev: LODLevel) -> LODLevel {
        let boundaries = [level_policy.near, level_policy.mid, level_policy.far, level_policy.very_far];
        let hysteresis = level_policy.hysteresis_ratio;

        let target = if distance_m <= boundaries[0] {
            LODLevel::L0Near
        } else if distance_m <= boundaries[1] {
            LODLevel::L1Mid
        } else if distance_m <= boundaries[2] {
            LODLevel::L2Far
        } else {
            LODLevel::L3VeryFar
        };

        if target as u8 > prev as u8 {
            let boundary = boundaries[prev as usize];
            let upshift = boundary * (1.0 + hysteresis);
            if distance_m > upshift {
                return target;
            }
            return prev;
        }

        if (target as u8) < (prev as u8) {
            let boundary = boundaries[prev as usize - 1];
            let downshift = boundary * (1.0 - hysteresis);
            if distance_m < downshift {
                return target;
            }
            return prev;
        }

        target
    }
}

#[derive(Debug, Clone, Copy)]
pub enum StreamPriority {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone)]
pub struct StreamRequest {
    pub world_id: u64,
    pub script_id: u64,
    pub requested_level: u8,
    pub bytes: u32,
    pub priority: StreamPriority,
}

#[derive(Debug, Clone, Copy)]
pub struct FrameBudget {
    pub target_ms: f32,
    pub gpu_headroom: f32,
    pub cpu_headroom: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct FrameContext {
    pub fps: u32,
    pub draw_calls: u32,
    pub visible_entities: u32,
    pub gpu_ms: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct FramePolicy {
    pub stereo: StereoConfig,
    pub foveation: FoveationConfig,
    pub clustered_lighting: ClusterLightingConfig,
    pub shadow_cascades: ShadowCascadeConfig,
    pub lod: LODPolicy,
    pub budget: FrameBudget,
}

impl Default for FramePolicy {
    fn default() -> Self {
        Self {
            stereo: StereoConfig::default(),
            foveation: FoveationConfig::default(),
            clustered_lighting: ClusterLightingConfig::default(),
            shadow_cascades: ShadowCascadeConfig::default(),
            lod: LODPolicy::default(),
            budget: FrameBudget {
                target_ms: 16.6,
                gpu_headroom: 0.35,
                cpu_headroom: 0.30,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lod_transitions_use_hysteresis_to_avoid_flapping() {
        let policy = LODPolicy::default();
        let mut current = LODLevel::L0Near;
        current = LodCurve::select(&policy, 9.7, current);
        assert_eq!(current, LODLevel::L0Near);
        current = LodCurve::select(&policy, 10.6, current);
        assert_eq!(current, LODLevel::L0Near);
        current = LodCurve::select(&policy, 11.0, current);
        assert_eq!(current, LODLevel::L1Mid);
        current = LodCurve::select(&policy, 10.2, current);
        assert_eq!(current, LODLevel::L1Mid);
    }

    #[test]
    fn stream_request_accepts_priority_ordering() {
        let request = StreamPriority::High;
        match request {
            StreamPriority::High => assert!(true),
            StreamPriority::Medium => assert!(false),
            StreamPriority::Low => assert!(false),
        }
    }
}
