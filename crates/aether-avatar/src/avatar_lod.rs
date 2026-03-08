//! Avatar-specific LOD tier system.
//!
//! Extends the basic LOD profile with four rendering tiers:
//! - FullMesh (<5m): full geometry, all shader features
//! - Simplified (5-30m): reduced geometry, no SSS
//! - Billboard (30-100m): camera-facing quad with baked texture
//! - Dot (100m+): single colored dot/particle

/// Default distance threshold for FullMesh -> Simplified transition.
const DEFAULT_FULL_MESH_MAX_M: f32 = 5.0;
/// Default distance threshold for Simplified -> Billboard transition.
const DEFAULT_SIMPLIFIED_MAX_M: f32 = 30.0;
/// Default distance threshold for Billboard -> Dot transition.
const DEFAULT_BILLBOARD_MAX_M: f32 = 100.0;
/// Maximum render distance. Beyond this the avatar is culled.
const DEFAULT_CULL_DISTANCE_M: f32 = 500.0;
/// Default hysteresis ratio for LOD transitions (percentage of boundary).
const DEFAULT_HYSTERESIS_RATIO: f32 = 0.08;
/// Default transition blend duration in milliseconds.
const DEFAULT_TRANSITION_MS: u32 = 200;

/// Avatar LOD rendering tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AvatarLodTier {
    /// Full mesh with all shader features. Distance < 5m.
    FullMesh = 0,
    /// Simplified mesh, reduced features. Distance 5-30m.
    Simplified = 1,
    /// Camera-facing billboard. Distance 30-100m.
    Billboard = 2,
    /// Single colored dot/particle. Distance 100m+.
    Dot = 3,
    /// Beyond cull distance, not rendered.
    Culled = 4,
}

/// Distance thresholds for LOD tier transitions.
#[derive(Debug, Clone)]
pub struct AvatarLodConfig {
    /// Maximum distance for FullMesh tier (meters).
    pub full_mesh_max_m: f32,
    /// Maximum distance for Simplified tier (meters).
    pub simplified_max_m: f32,
    /// Maximum distance for Billboard tier (meters).
    pub billboard_max_m: f32,
    /// Distance beyond which avatar is culled (meters).
    pub cull_distance_m: f32,
    /// Hysteresis ratio to prevent tier flapping.
    pub hysteresis_ratio: f32,
    /// Duration of LOD transition blend in milliseconds.
    pub transition_duration_ms: u32,
}

impl Default for AvatarLodConfig {
    fn default() -> Self {
        Self {
            full_mesh_max_m: DEFAULT_FULL_MESH_MAX_M,
            simplified_max_m: DEFAULT_SIMPLIFIED_MAX_M,
            billboard_max_m: DEFAULT_BILLBOARD_MAX_M,
            cull_distance_m: DEFAULT_CULL_DISTANCE_M,
            hysteresis_ratio: DEFAULT_HYSTERESIS_RATIO,
            transition_duration_ms: DEFAULT_TRANSITION_MS,
        }
    }
}

impl AvatarLodConfig {
    /// Get the boundary distance for transitioning away from a given tier.
    fn boundary_for_tier(&self, tier: AvatarLodTier) -> f32 {
        match tier {
            AvatarLodTier::FullMesh => self.full_mesh_max_m,
            AvatarLodTier::Simplified => self.simplified_max_m,
            AvatarLodTier::Billboard => self.billboard_max_m,
            AvatarLodTier::Dot => self.cull_distance_m,
            AvatarLodTier::Culled => self.cull_distance_m,
        }
    }
}

/// Select the appropriate LOD tier based on distance, with hysteresis.
///
/// Hysteresis prevents rapid switching when an avatar hovers near a
/// boundary. Moving to a higher (farther) tier requires exceeding the
/// boundary by `hysteresis_ratio`, and moving to a lower (nearer) tier
/// requires falling below the boundary by `hysteresis_ratio`.
pub fn select_lod_tier(
    config: &AvatarLodConfig,
    distance_m: f32,
    previous: AvatarLodTier,
) -> AvatarLodTier {
    let raw_tier = raw_tier_for_distance(config, distance_m);

    if raw_tier == previous {
        return previous;
    }

    // Moving to a farther (higher ordinal) tier
    if raw_tier > previous {
        let boundary = config.boundary_for_tier(previous);
        let threshold = boundary * (1.0 + config.hysteresis_ratio);
        if distance_m > threshold {
            return raw_tier;
        }
        return previous;
    }

    // Moving to a nearer (lower ordinal) tier
    let boundary = tier_lower_boundary(config, previous);
    let threshold = boundary * (1.0 - config.hysteresis_ratio);
    if distance_m < threshold {
        return raw_tier;
    }
    previous
}

/// Get the raw tier without hysteresis.
fn raw_tier_for_distance(config: &AvatarLodConfig, distance_m: f32) -> AvatarLodTier {
    if distance_m <= config.full_mesh_max_m {
        AvatarLodTier::FullMesh
    } else if distance_m <= config.simplified_max_m {
        AvatarLodTier::Simplified
    } else if distance_m <= config.billboard_max_m {
        AvatarLodTier::Billboard
    } else if distance_m <= config.cull_distance_m {
        AvatarLodTier::Dot
    } else {
        AvatarLodTier::Culled
    }
}

/// Get the lower boundary distance for a tier (the distance at which
/// transitioning down would happen).
fn tier_lower_boundary(config: &AvatarLodConfig, tier: AvatarLodTier) -> f32 {
    match tier {
        AvatarLodTier::FullMesh => 0.0,
        AvatarLodTier::Simplified => config.full_mesh_max_m,
        AvatarLodTier::Billboard => config.simplified_max_m,
        AvatarLodTier::Dot => config.billboard_max_m,
        AvatarLodTier::Culled => config.cull_distance_m,
    }
}

/// State tracking for an active LOD transition between tiers.
#[derive(Debug, Clone)]
pub struct AvatarLodTransition {
    /// Tier being transitioned from.
    pub from: AvatarLodTier,
    /// Tier being transitioned to.
    pub to: AvatarLodTier,
    /// Elapsed transition time in milliseconds.
    pub elapsed_ms: u32,
    /// Total transition duration in milliseconds.
    pub duration_ms: u32,
}

impl AvatarLodTransition {
    /// Create a new transition.
    pub fn new(from: AvatarLodTier, to: AvatarLodTier, duration_ms: u32) -> Self {
        Self {
            from,
            to,
            elapsed_ms: 0,
            duration_ms,
        }
    }

    /// Advance the transition by a time delta.
    pub fn advance(&mut self, dt_ms: u32) {
        self.elapsed_ms = self.elapsed_ms.saturating_add(dt_ms);
    }

    /// Whether the transition is complete.
    pub fn is_complete(&self) -> bool {
        self.elapsed_ms >= self.duration_ms
    }

    /// Get the blend factor (0.0 = fully "from", 1.0 = fully "to").
    pub fn blend_factor(&self) -> f32 {
        if self.duration_ms == 0 {
            return 1.0;
        }
        let t = self.elapsed_ms as f32 / self.duration_ms as f32;
        t.clamp(0.0, 1.0)
    }
}

/// Rendering hints for each LOD tier.
#[derive(Debug, Clone)]
pub struct AvatarLodRenderHints {
    /// The current LOD tier.
    pub tier: AvatarLodTier,
    /// Whether to enable skinning.
    pub enable_skinning: bool,
    /// Whether to enable blend shapes.
    pub enable_blend_shapes: bool,
    /// Whether to enable SSS.
    pub enable_sss: bool,
    /// Whether to enable eye refraction.
    pub enable_eye_refraction: bool,
    /// Whether to enable shadow casting.
    pub cast_shadows: bool,
    /// Texture resolution scale (1.0 = full, 0.5 = half, etc.).
    pub texture_scale: f32,
}

impl AvatarLodRenderHints {
    /// Get render hints for a given tier.
    pub fn for_tier(tier: AvatarLodTier) -> Self {
        match tier {
            AvatarLodTier::FullMesh => Self {
                tier,
                enable_skinning: true,
                enable_blend_shapes: true,
                enable_sss: true,
                enable_eye_refraction: true,
                cast_shadows: true,
                texture_scale: 1.0,
            },
            AvatarLodTier::Simplified => Self {
                tier,
                enable_skinning: true,
                enable_blend_shapes: false,
                enable_sss: false,
                enable_eye_refraction: false,
                cast_shadows: true,
                texture_scale: 0.5,
            },
            AvatarLodTier::Billboard => Self {
                tier,
                enable_skinning: false,
                enable_blend_shapes: false,
                enable_sss: false,
                enable_eye_refraction: false,
                cast_shadows: false,
                texture_scale: 0.25,
            },
            AvatarLodTier::Dot | AvatarLodTier::Culled => Self {
                tier,
                enable_skinning: false,
                enable_blend_shapes: false,
                enable_sss: false,
                enable_eye_refraction: false,
                cast_shadows: false,
                texture_scale: 0.0,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 0.01;

    #[test]
    fn test_default_config() {
        let config = AvatarLodConfig::default();
        assert!((config.full_mesh_max_m - DEFAULT_FULL_MESH_MAX_M).abs() < EPSILON);
        assert!((config.simplified_max_m - DEFAULT_SIMPLIFIED_MAX_M).abs() < EPSILON);
        assert!((config.billboard_max_m - DEFAULT_BILLBOARD_MAX_M).abs() < EPSILON);
        assert!((config.cull_distance_m - DEFAULT_CULL_DISTANCE_M).abs() < EPSILON);
    }

    #[test]
    fn test_raw_tier_full_mesh() {
        let config = AvatarLodConfig::default();
        assert_eq!(raw_tier_for_distance(&config, 0.0), AvatarLodTier::FullMesh);
        assert_eq!(raw_tier_for_distance(&config, 3.0), AvatarLodTier::FullMesh);
        assert_eq!(raw_tier_for_distance(&config, 5.0), AvatarLodTier::FullMesh);
    }

    #[test]
    fn test_raw_tier_simplified() {
        let config = AvatarLodConfig::default();
        assert_eq!(
            raw_tier_for_distance(&config, 5.1),
            AvatarLodTier::Simplified
        );
        assert_eq!(
            raw_tier_for_distance(&config, 15.0),
            AvatarLodTier::Simplified
        );
        assert_eq!(
            raw_tier_for_distance(&config, 30.0),
            AvatarLodTier::Simplified
        );
    }

    #[test]
    fn test_raw_tier_billboard() {
        let config = AvatarLodConfig::default();
        assert_eq!(
            raw_tier_for_distance(&config, 30.1),
            AvatarLodTier::Billboard
        );
        assert_eq!(
            raw_tier_for_distance(&config, 60.0),
            AvatarLodTier::Billboard
        );
        assert_eq!(
            raw_tier_for_distance(&config, 100.0),
            AvatarLodTier::Billboard
        );
    }

    #[test]
    fn test_raw_tier_dot() {
        let config = AvatarLodConfig::default();
        assert_eq!(raw_tier_for_distance(&config, 100.1), AvatarLodTier::Dot);
        assert_eq!(raw_tier_for_distance(&config, 300.0), AvatarLodTier::Dot);
        assert_eq!(raw_tier_for_distance(&config, 500.0), AvatarLodTier::Dot);
    }

    #[test]
    fn test_raw_tier_culled() {
        let config = AvatarLodConfig::default();
        assert_eq!(
            raw_tier_for_distance(&config, 500.1),
            AvatarLodTier::Culled
        );
        assert_eq!(
            raw_tier_for_distance(&config, 1000.0),
            AvatarLodTier::Culled
        );
    }

    #[test]
    fn test_select_lod_same_tier_no_change() {
        let config = AvatarLodConfig::default();
        let result = select_lod_tier(&config, 3.0, AvatarLodTier::FullMesh);
        assert_eq!(result, AvatarLodTier::FullMesh);
    }

    #[test]
    fn test_select_lod_hysteresis_prevents_upshift() {
        let config = AvatarLodConfig::default();
        // At 5.1m, raw tier is Simplified, but hysteresis should prevent transition
        // threshold = 5.0 * 1.08 = 5.4
        let result = select_lod_tier(&config, 5.2, AvatarLodTier::FullMesh);
        assert_eq!(result, AvatarLodTier::FullMesh);
    }

    #[test]
    fn test_select_lod_upshift_beyond_hysteresis() {
        let config = AvatarLodConfig::default();
        // threshold = 5.0 * 1.08 = 5.4, so 5.5 should trigger
        let result = select_lod_tier(&config, 5.5, AvatarLodTier::FullMesh);
        assert_eq!(result, AvatarLodTier::Simplified);
    }

    #[test]
    fn test_select_lod_hysteresis_prevents_downshift() {
        let config = AvatarLodConfig::default();
        // At 4.9m, raw tier is FullMesh, but hysteresis should prevent
        // threshold = 5.0 * (1 - 0.08) = 4.6
        let result = select_lod_tier(&config, 4.8, AvatarLodTier::Simplified);
        assert_eq!(result, AvatarLodTier::Simplified);
    }

    #[test]
    fn test_select_lod_downshift_beyond_hysteresis() {
        let config = AvatarLodConfig::default();
        // threshold = 5.0 * (1 - 0.08) = 4.6, so 4.5 should trigger
        let result = select_lod_tier(&config, 4.5, AvatarLodTier::Simplified);
        assert_eq!(result, AvatarLodTier::FullMesh);
    }

    #[test]
    fn test_select_lod_billboard_to_dot() {
        let config = AvatarLodConfig::default();
        // threshold = 100.0 * 1.08 = 108.0
        let result = select_lod_tier(&config, 110.0, AvatarLodTier::Billboard);
        assert_eq!(result, AvatarLodTier::Dot);
    }

    #[test]
    fn test_select_lod_dot_to_billboard() {
        let config = AvatarLodConfig::default();
        // threshold = 100.0 * (1 - 0.08) = 92.0
        let result = select_lod_tier(&config, 90.0, AvatarLodTier::Dot);
        assert_eq!(result, AvatarLodTier::Billboard);
    }

    #[test]
    fn test_select_lod_to_culled() {
        let config = AvatarLodConfig::default();
        // threshold = 500.0 * 1.08 = 540.0
        let result = select_lod_tier(&config, 550.0, AvatarLodTier::Dot);
        assert_eq!(result, AvatarLodTier::Culled);
    }

    #[test]
    fn test_transition_new() {
        let t = AvatarLodTransition::new(AvatarLodTier::FullMesh, AvatarLodTier::Simplified, 200);
        assert_eq!(t.from, AvatarLodTier::FullMesh);
        assert_eq!(t.to, AvatarLodTier::Simplified);
        assert_eq!(t.elapsed_ms, 0);
        assert!(!t.is_complete());
    }

    #[test]
    fn test_transition_advance() {
        let mut t =
            AvatarLodTransition::new(AvatarLodTier::FullMesh, AvatarLodTier::Simplified, 200);
        t.advance(100);
        assert_eq!(t.elapsed_ms, 100);
        assert!(!t.is_complete());
    }

    #[test]
    fn test_transition_complete() {
        let mut t =
            AvatarLodTransition::new(AvatarLodTier::FullMesh, AvatarLodTier::Simplified, 200);
        t.advance(200);
        assert!(t.is_complete());
    }

    #[test]
    fn test_transition_advance_past_duration() {
        let mut t =
            AvatarLodTransition::new(AvatarLodTier::FullMesh, AvatarLodTier::Simplified, 200);
        t.advance(500);
        assert!(t.is_complete());
        assert!((t.blend_factor() - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_transition_blend_factor_start() {
        let t = AvatarLodTransition::new(AvatarLodTier::FullMesh, AvatarLodTier::Simplified, 200);
        assert!((t.blend_factor()).abs() < EPSILON);
    }

    #[test]
    fn test_transition_blend_factor_mid() {
        let mut t =
            AvatarLodTransition::new(AvatarLodTier::FullMesh, AvatarLodTier::Simplified, 200);
        t.advance(100);
        assert!((t.blend_factor() - 0.5).abs() < EPSILON);
    }

    #[test]
    fn test_transition_blend_factor_end() {
        let mut t =
            AvatarLodTransition::new(AvatarLodTier::FullMesh, AvatarLodTier::Simplified, 200);
        t.advance(200);
        assert!((t.blend_factor() - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_transition_zero_duration() {
        let t = AvatarLodTransition::new(AvatarLodTier::FullMesh, AvatarLodTier::Simplified, 0);
        assert!((t.blend_factor() - 1.0).abs() < EPSILON);
        assert!(t.is_complete());
    }

    #[test]
    fn test_render_hints_full_mesh() {
        let hints = AvatarLodRenderHints::for_tier(AvatarLodTier::FullMesh);
        assert!(hints.enable_skinning);
        assert!(hints.enable_blend_shapes);
        assert!(hints.enable_sss);
        assert!(hints.enable_eye_refraction);
        assert!(hints.cast_shadows);
        assert!((hints.texture_scale - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_render_hints_simplified() {
        let hints = AvatarLodRenderHints::for_tier(AvatarLodTier::Simplified);
        assert!(hints.enable_skinning);
        assert!(!hints.enable_blend_shapes);
        assert!(!hints.enable_sss);
        assert!(!hints.enable_eye_refraction);
        assert!(hints.cast_shadows);
        assert!((hints.texture_scale - 0.5).abs() < EPSILON);
    }

    #[test]
    fn test_render_hints_billboard() {
        let hints = AvatarLodRenderHints::for_tier(AvatarLodTier::Billboard);
        assert!(!hints.enable_skinning);
        assert!(!hints.enable_blend_shapes);
        assert!(!hints.cast_shadows);
        assert!((hints.texture_scale - 0.25).abs() < EPSILON);
    }

    #[test]
    fn test_render_hints_dot() {
        let hints = AvatarLodRenderHints::for_tier(AvatarLodTier::Dot);
        assert!(!hints.enable_skinning);
        assert!(!hints.enable_blend_shapes);
        assert!(!hints.cast_shadows);
        assert!(hints.texture_scale.abs() < EPSILON);
    }

    #[test]
    fn test_render_hints_culled() {
        let hints = AvatarLodRenderHints::for_tier(AvatarLodTier::Culled);
        assert!(!hints.enable_skinning);
        assert!(hints.texture_scale.abs() < EPSILON);
    }

    #[test]
    fn test_tier_ordering() {
        assert!(AvatarLodTier::FullMesh < AvatarLodTier::Simplified);
        assert!(AvatarLodTier::Simplified < AvatarLodTier::Billboard);
        assert!(AvatarLodTier::Billboard < AvatarLodTier::Dot);
        assert!(AvatarLodTier::Dot < AvatarLodTier::Culled);
    }

    #[test]
    fn test_select_lod_very_close() {
        let config = AvatarLodConfig::default();
        let result = select_lod_tier(&config, 0.1, AvatarLodTier::FullMesh);
        assert_eq!(result, AvatarLodTier::FullMesh);
    }

    #[test]
    fn test_select_lod_negative_distance_stays_full() {
        let config = AvatarLodConfig::default();
        let result = select_lod_tier(&config, -1.0, AvatarLodTier::FullMesh);
        assert_eq!(result, AvatarLodTier::FullMesh);
    }
}
