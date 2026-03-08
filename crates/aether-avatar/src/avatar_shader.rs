//! Avatar-specific PBR shading configuration.
//!
//! Defines material properties for avatar rendering including subsurface
//! scattering (for skin at close range) and eye rendering with refraction.

/// Default index of refraction for human cornea.
const DEFAULT_CORNEA_IOR: f32 = 1.376;
/// Default pupil radius in millimeters.
const DEFAULT_PUPIL_RADIUS_MM: f32 = 2.0;
/// Default cornea roughness.
const DEFAULT_CORNEA_ROUGHNESS: f32 = 0.05;
/// Default subsurface scatter radius in millimeters.
const DEFAULT_SSS_RADIUS_MM: f32 = 8.0;
/// Default subsurface scatter strength.
const DEFAULT_SSS_STRENGTH: f32 = 0.5;
/// Default PBR roughness for skin.
const DEFAULT_SKIN_ROUGHNESS: f32 = 0.45;
/// Default PBR metallic for skin (non-metallic).
const DEFAULT_SKIN_METALLIC: f32 = 0.0;

/// Subsurface scattering profile type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SssProfile {
    /// Human skin: warm red/orange scatter.
    Skin,
    /// Waxy material: yellow-tinted scatter.
    Wax,
    /// Marble: white/grey scatter.
    Marble,
    /// Custom profile defined by scatter color.
    Custom,
}

/// Subsurface scattering configuration for avatar skin.
#[derive(Debug, Clone)]
pub struct SubsurfaceScatteringConfig {
    /// Whether SSS is enabled.
    pub enabled: bool,
    /// Scatter profile type.
    pub profile: SssProfile,
    /// Scatter radius in millimeters.
    pub radius_mm: f32,
    /// Scatter strength (0.0 = no scatter, 1.0 = full).
    pub strength: f32,
    /// Scatter color tint [R, G, B] in linear space.
    pub scatter_color: [f32; 3],
}

impl Default for SubsurfaceScatteringConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            profile: SssProfile::Skin,
            radius_mm: DEFAULT_SSS_RADIUS_MM,
            strength: DEFAULT_SSS_STRENGTH,
            scatter_color: [1.0, 0.4, 0.25], // warm skin tone
        }
    }
}

impl SubsurfaceScatteringConfig {
    /// Create a disabled SSS config.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Self::default()
        }
    }

    /// Validate that the config has plausible values.
    pub fn is_valid(&self) -> bool {
        self.radius_mm > 0.0
            && self.radius_mm <= 50.0
            && self.strength >= 0.0
            && self.strength <= 1.0
            && self.scatter_color.iter().all(|c| *c >= 0.0 && *c <= 1.0)
    }
}

/// Eye rendering configuration with refraction.
#[derive(Debug, Clone)]
pub struct EyeRefractionConfig {
    /// Whether eye refraction is enabled.
    pub enabled: bool,
    /// Index of refraction for the cornea.
    pub cornea_ior: f32,
    /// Pupil radius in millimeters.
    pub pupil_radius_mm: f32,
    /// Iris texture layer index (for multi-layer eye rendering).
    pub iris_texture_layer: u32,
    /// Cornea surface roughness (0.0 = mirror, 1.0 = diffuse).
    pub cornea_roughness: f32,
    /// Whether to render caustics from the cornea.
    pub caustics_enabled: bool,
    /// Iris color tint [R, G, B] in linear space.
    pub iris_color: [f32; 3],
}

impl Default for EyeRefractionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            cornea_ior: DEFAULT_CORNEA_IOR,
            pupil_radius_mm: DEFAULT_PUPIL_RADIUS_MM,
            iris_texture_layer: 0,
            cornea_roughness: DEFAULT_CORNEA_ROUGHNESS,
            caustics_enabled: false,
            iris_color: [0.4, 0.3, 0.2], // brown
        }
    }
}

impl EyeRefractionConfig {
    /// Create a disabled eye refraction config.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Self::default()
        }
    }

    /// Validate that the config has plausible values.
    pub fn is_valid(&self) -> bool {
        self.cornea_ior >= 1.0
            && self.cornea_ior <= 3.0
            && self.pupil_radius_mm > 0.0
            && self.pupil_radius_mm <= 10.0
            && self.cornea_roughness >= 0.0
            && self.cornea_roughness <= 1.0
    }
}

/// Base PBR material properties for an avatar surface.
#[derive(Debug, Clone)]
pub struct AvatarPbrProperties {
    /// Albedo color [R, G, B] in linear space.
    pub albedo: [f32; 3],
    /// Roughness (0.0 = mirror, 1.0 = fully diffuse).
    pub roughness: f32,
    /// Metallic (0.0 = dielectric, 1.0 = metal).
    pub metallic: f32,
    /// Normal map strength multiplier.
    pub normal_strength: f32,
    /// Ambient occlusion strength.
    pub ao_strength: f32,
    /// Emission color [R, G, B] in linear space.
    pub emission: [f32; 3],
    /// Emission intensity multiplier.
    pub emission_intensity: f32,
}

impl Default for AvatarPbrProperties {
    fn default() -> Self {
        Self {
            albedo: [0.8, 0.7, 0.65], // light skin tone
            roughness: DEFAULT_SKIN_ROUGHNESS,
            metallic: DEFAULT_SKIN_METALLIC,
            normal_strength: 1.0,
            ao_strength: 1.0,
            emission: [0.0, 0.0, 0.0],
            emission_intensity: 0.0,
        }
    }
}

/// Complete material configuration for a single avatar surface.
#[derive(Debug, Clone)]
pub struct AvatarMaterialConfig {
    /// Base PBR properties.
    pub pbr: AvatarPbrProperties,
    /// Optional subsurface scattering for skin-like surfaces.
    pub sss: SubsurfaceScatteringConfig,
    /// Optional eye refraction. Only relevant for eye meshes.
    pub eye: EyeRefractionConfig,
}

impl Default for AvatarMaterialConfig {
    fn default() -> Self {
        Self {
            pbr: AvatarPbrProperties::default(),
            sss: SubsurfaceScatteringConfig::default(),
            eye: EyeRefractionConfig::disabled(),
        }
    }
}

impl AvatarMaterialConfig {
    /// Create a material config for an eye mesh.
    pub fn eye_material() -> Self {
        Self {
            pbr: AvatarPbrProperties {
                albedo: [1.0, 1.0, 1.0],
                roughness: DEFAULT_CORNEA_ROUGHNESS,
                metallic: 0.0,
                ..AvatarPbrProperties::default()
            },
            sss: SubsurfaceScatteringConfig::disabled(),
            eye: EyeRefractionConfig::default(),
        }
    }

    /// Create a material config for a clothing surface (no SSS).
    pub fn clothing_material() -> Self {
        Self {
            pbr: AvatarPbrProperties {
                albedo: [0.5, 0.5, 0.5],
                roughness: 0.6,
                metallic: 0.0,
                ..AvatarPbrProperties::default()
            },
            sss: SubsurfaceScatteringConfig::disabled(),
            eye: EyeRefractionConfig::disabled(),
        }
    }
}

/// Feature flags for selecting shader permutations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShaderFeatureFlags {
    /// GPU skinning is active.
    pub skinning: bool,
    /// Blend shapes are active.
    pub blend_shapes: bool,
    /// Subsurface scattering pass is active.
    pub subsurface_scattering: bool,
    /// Eye refraction pass is active.
    pub eye_refraction: bool,
    /// Normal mapping is active.
    pub normal_map: bool,
    /// Emission is active.
    pub emission: bool,
}

impl ShaderFeatureFlags {
    /// Full-featured flags (all enabled).
    pub fn full() -> Self {
        Self {
            skinning: true,
            blend_shapes: true,
            subsurface_scattering: true,
            eye_refraction: false, // eye refraction and SSS are typically mutually exclusive
            normal_map: true,
            emission: false,
        }
    }

    /// Minimal flags (skinning only).
    pub fn minimal() -> Self {
        Self {
            skinning: true,
            blend_shapes: false,
            subsurface_scattering: false,
            eye_refraction: false,
            normal_map: false,
            emission: false,
        }
    }

    /// Compute a bitmask for shader variant selection.
    pub fn to_bitmask(&self) -> u32 {
        let mut mask = 0u32;
        if self.skinning {
            mask |= 1;
        }
        if self.blend_shapes {
            mask |= 1 << 1;
        }
        if self.subsurface_scattering {
            mask |= 1 << 2;
        }
        if self.eye_refraction {
            mask |= 1 << 3;
        }
        if self.normal_map {
            mask |= 1 << 4;
        }
        if self.emission {
            mask |= 1 << 5;
        }
        mask
    }
}

/// A shader permutation selected based on feature flags and LOD.
#[derive(Debug, Clone)]
pub struct AvatarShaderPermutation {
    /// Feature flags controlling which shader stages are active.
    pub features: ShaderFeatureFlags,
    /// A descriptive label for debugging.
    pub label: String,
}

impl AvatarShaderPermutation {
    /// Select the appropriate shader permutation from a material config.
    pub fn from_material(material: &AvatarMaterialConfig, is_near: bool) -> Self {
        let features = ShaderFeatureFlags {
            skinning: true,
            blend_shapes: is_near,
            subsurface_scattering: is_near && material.sss.enabled,
            eye_refraction: is_near && material.eye.enabled,
            normal_map: is_near,
            emission: material.pbr.emission_intensity > 0.0,
        };

        let label = format!("avatar_shader_{:#06x}", features.to_bitmask());

        Self { features, label }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-6;

    #[test]
    fn test_sss_config_default() {
        let sss = SubsurfaceScatteringConfig::default();
        assert!(sss.enabled);
        assert_eq!(sss.profile, SssProfile::Skin);
        assert!((sss.radius_mm - DEFAULT_SSS_RADIUS_MM).abs() < EPSILON);
        assert!((sss.strength - DEFAULT_SSS_STRENGTH).abs() < EPSILON);
    }

    #[test]
    fn test_sss_config_disabled() {
        let sss = SubsurfaceScatteringConfig::disabled();
        assert!(!sss.enabled);
    }

    #[test]
    fn test_sss_config_valid() {
        let sss = SubsurfaceScatteringConfig::default();
        assert!(sss.is_valid());
    }

    #[test]
    fn test_sss_config_invalid_radius() {
        let sss = SubsurfaceScatteringConfig {
            radius_mm: -1.0,
            ..SubsurfaceScatteringConfig::default()
        };
        assert!(!sss.is_valid());
    }

    #[test]
    fn test_sss_config_invalid_strength() {
        let sss = SubsurfaceScatteringConfig {
            strength: 1.5,
            ..SubsurfaceScatteringConfig::default()
        };
        assert!(!sss.is_valid());
    }

    #[test]
    fn test_sss_config_invalid_color() {
        let sss = SubsurfaceScatteringConfig {
            scatter_color: [1.5, 0.0, 0.0],
            ..SubsurfaceScatteringConfig::default()
        };
        assert!(!sss.is_valid());
    }

    #[test]
    fn test_sss_config_invalid_radius_too_large() {
        let sss = SubsurfaceScatteringConfig {
            radius_mm: 100.0,
            ..SubsurfaceScatteringConfig::default()
        };
        assert!(!sss.is_valid());
    }

    #[test]
    fn test_eye_config_default() {
        let eye = EyeRefractionConfig::default();
        assert!(eye.enabled);
        assert!((eye.cornea_ior - DEFAULT_CORNEA_IOR).abs() < EPSILON);
        assert!((eye.pupil_radius_mm - DEFAULT_PUPIL_RADIUS_MM).abs() < EPSILON);
        assert!((eye.cornea_roughness - DEFAULT_CORNEA_ROUGHNESS).abs() < EPSILON);
        assert!(!eye.caustics_enabled);
    }

    #[test]
    fn test_eye_config_disabled() {
        let eye = EyeRefractionConfig::disabled();
        assert!(!eye.enabled);
    }

    #[test]
    fn test_eye_config_valid() {
        let eye = EyeRefractionConfig::default();
        assert!(eye.is_valid());
    }

    #[test]
    fn test_eye_config_invalid_ior_low() {
        let eye = EyeRefractionConfig {
            cornea_ior: 0.5,
            ..EyeRefractionConfig::default()
        };
        assert!(!eye.is_valid());
    }

    #[test]
    fn test_eye_config_invalid_ior_high() {
        let eye = EyeRefractionConfig {
            cornea_ior: 5.0,
            ..EyeRefractionConfig::default()
        };
        assert!(!eye.is_valid());
    }

    #[test]
    fn test_eye_config_invalid_pupil() {
        let eye = EyeRefractionConfig {
            pupil_radius_mm: 0.0,
            ..EyeRefractionConfig::default()
        };
        assert!(!eye.is_valid());
    }

    #[test]
    fn test_eye_config_invalid_roughness() {
        let eye = EyeRefractionConfig {
            cornea_roughness: -0.1,
            ..EyeRefractionConfig::default()
        };
        assert!(!eye.is_valid());
    }

    #[test]
    fn test_pbr_default() {
        let pbr = AvatarPbrProperties::default();
        assert!((pbr.roughness - DEFAULT_SKIN_ROUGHNESS).abs() < EPSILON);
        assert!((pbr.metallic - DEFAULT_SKIN_METALLIC).abs() < EPSILON);
    }

    #[test]
    fn test_material_config_default() {
        let mat = AvatarMaterialConfig::default();
        assert!(mat.sss.enabled);
        assert!(!mat.eye.enabled);
    }

    #[test]
    fn test_material_config_eye() {
        let mat = AvatarMaterialConfig::eye_material();
        assert!(!mat.sss.enabled);
        assert!(mat.eye.enabled);
    }

    #[test]
    fn test_material_config_clothing() {
        let mat = AvatarMaterialConfig::clothing_material();
        assert!(!mat.sss.enabled);
        assert!(!mat.eye.enabled);
    }

    #[test]
    fn test_shader_features_full() {
        let f = ShaderFeatureFlags::full();
        assert!(f.skinning);
        assert!(f.blend_shapes);
        assert!(f.subsurface_scattering);
        assert!(!f.eye_refraction);
        assert!(f.normal_map);
    }

    #[test]
    fn test_shader_features_minimal() {
        let f = ShaderFeatureFlags::minimal();
        assert!(f.skinning);
        assert!(!f.blend_shapes);
        assert!(!f.subsurface_scattering);
        assert!(!f.normal_map);
    }

    #[test]
    fn test_shader_features_bitmask() {
        let f = ShaderFeatureFlags {
            skinning: true,
            blend_shapes: false,
            subsurface_scattering: true,
            eye_refraction: false,
            normal_map: true,
            emission: false,
        };
        let mask = f.to_bitmask();
        assert_eq!(mask & 1, 1); // skinning
        assert_eq!(mask & 2, 0); // no blend shapes
        assert_eq!(mask & 4, 4); // sss
        assert_eq!(mask & 8, 0); // no eye
        assert_eq!(mask & 16, 16); // normal map
        assert_eq!(mask & 32, 0); // no emission
    }

    #[test]
    fn test_shader_features_bitmask_all_on() {
        let f = ShaderFeatureFlags {
            skinning: true,
            blend_shapes: true,
            subsurface_scattering: true,
            eye_refraction: true,
            normal_map: true,
            emission: true,
        };
        assert_eq!(f.to_bitmask(), 0b111111);
    }

    #[test]
    fn test_shader_features_bitmask_all_off() {
        let f = ShaderFeatureFlags {
            skinning: false,
            blend_shapes: false,
            subsurface_scattering: false,
            eye_refraction: false,
            normal_map: false,
            emission: false,
        };
        assert_eq!(f.to_bitmask(), 0);
    }

    #[test]
    fn test_shader_permutation_near_skin() {
        let mat = AvatarMaterialConfig::default();
        let perm = AvatarShaderPermutation::from_material(&mat, true);
        assert!(perm.features.skinning);
        assert!(perm.features.blend_shapes);
        assert!(perm.features.subsurface_scattering);
        assert!(!perm.features.eye_refraction);
        assert!(perm.features.normal_map);
    }

    #[test]
    fn test_shader_permutation_far_skin() {
        let mat = AvatarMaterialConfig::default();
        let perm = AvatarShaderPermutation::from_material(&mat, false);
        assert!(perm.features.skinning);
        assert!(!perm.features.blend_shapes);
        assert!(!perm.features.subsurface_scattering);
        assert!(!perm.features.normal_map);
    }

    #[test]
    fn test_shader_permutation_near_eye() {
        let mat = AvatarMaterialConfig::eye_material();
        let perm = AvatarShaderPermutation::from_material(&mat, true);
        assert!(perm.features.eye_refraction);
        assert!(!perm.features.subsurface_scattering);
    }

    #[test]
    fn test_shader_permutation_far_eye() {
        let mat = AvatarMaterialConfig::eye_material();
        let perm = AvatarShaderPermutation::from_material(&mat, false);
        assert!(!perm.features.eye_refraction);
    }

    #[test]
    fn test_shader_permutation_label_format() {
        let mat = AvatarMaterialConfig::default();
        let perm = AvatarShaderPermutation::from_material(&mat, true);
        assert!(perm.label.starts_with("avatar_shader_"));
    }

    #[test]
    fn test_shader_permutation_emission() {
        let mat = AvatarMaterialConfig {
            pbr: AvatarPbrProperties {
                emission: [1.0, 0.0, 0.0],
                emission_intensity: 2.0,
                ..AvatarPbrProperties::default()
            },
            ..AvatarMaterialConfig::default()
        };
        let perm = AvatarShaderPermutation::from_material(&mat, true);
        assert!(perm.features.emission);
    }

    #[test]
    fn test_shader_permutation_no_emission() {
        let mat = AvatarMaterialConfig::default();
        let perm = AvatarShaderPermutation::from_material(&mat, true);
        assert!(!perm.features.emission);
    }

    #[test]
    fn test_sss_profile_variants() {
        assert_eq!(SssProfile::Skin, SssProfile::Skin);
        assert_ne!(SssProfile::Skin, SssProfile::Wax);
        assert_ne!(SssProfile::Marble, SssProfile::Custom);
    }
}
