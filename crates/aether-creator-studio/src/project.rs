//! World project manifest with dimension-aware configuration.
//!
//! Defines the `world.toml` manifest format and validation logic.

use serde::{Deserialize, Serialize};

use crate::dimension::WorldDimension;

/// Serialization/deserialization error type.
pub type SerdeError = serde_json::Error;

const MIN_TICK_RATE: u32 = 1;
const MAX_TICK_RATE: u32 = 240;

// ---------------------------------------------------------------------------
// Manifest types
// ---------------------------------------------------------------------------

/// The world.toml manifest with dimension support.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldProjectManifest {
    pub world: WorldInfo,
    pub physics: PhysicsConfig,
    pub players: PlayerConfig,
    pub scenes: SceneConfig,
    pub camera: Option<CameraConfig2D>,
    pub environment: Option<EnvironmentConfig>,
}

/// Core world information.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldInfo {
    pub name: String,
    pub version: String,
    pub dimension: WorldDimension,
    pub description: String,
}

/// Physics configuration. Gravity is [x,y] for 2D or [x,y,z] for 3D.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhysicsConfig {
    pub gravity: Vec<f32>,
    pub tick_rate_hz: u32,
}

/// Player configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerConfig {
    pub max_players: u32,
    pub spawn_scene: String,
}

/// Scene listing configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneConfig {
    pub default: String,
    pub list: Vec<String>,
}

/// Camera configuration for 2D worlds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CameraConfig2D {
    pub mode: CameraMode2D,
    pub pixels_per_unit: u32,
    pub bounds: Option<Bounds2D>,
}

/// 2D camera mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CameraMode2D {
    #[default]
    SideView,
    TopDown,
}

/// 2D camera bounds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bounds2D {
    pub min: [f32; 2],
    pub max: [f32; 2],
}

/// Environment configuration (3D and 2D options).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentConfig {
    /// 3D: skybox asset path.
    pub skybox: Option<String>,
    /// 3D: fog density.
    pub fog_density: Option<f32>,
    /// 2D: background color as [r, g, b].
    pub background_color: Option<[f32; 3]>,
    /// 2D: parallax scrolling layers.
    pub parallax_layers: Option<Vec<ParallaxLayer>>,
}

/// A single parallax scrolling layer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParallaxLayer {
    pub image: String,
    pub speed: f32,
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/// A validation error on a manifest field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
}

impl ValidationError {
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }
}

/// Validate a world project manifest.
///
/// Checks:
/// - name not empty
/// - version is valid semver (major.minor.patch)
/// - gravity dimensions match world dimension (2 for 2D, 3 for 3D)
/// - tick_rate_hz in range 1..=240
/// - max_players > 0
/// - scenes list not empty
/// - default scene is in the scenes list
pub fn validate_manifest(manifest: &WorldProjectManifest) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    // Name must not be empty.
    if manifest.world.name.is_empty() {
        errors.push(ValidationError::new("world.name", "name must not be empty"));
    }

    // Version must be valid semver (major.minor.patch, all numeric).
    if !is_valid_semver(&manifest.world.version) {
        errors.push(ValidationError::new(
            "world.version",
            "version must be valid semver (e.g. \"1.0.0\")",
        ));
    }

    // Gravity dimensions must match world dimension.
    let expected_gravity_len = match manifest.world.dimension {
        WorldDimension::TwoD => 2,
        WorldDimension::ThreeD => 3,
    };
    if manifest.physics.gravity.len() != expected_gravity_len {
        errors.push(ValidationError::new(
            "physics.gravity",
            format!(
                "gravity must have {} components for {} world, got {}",
                expected_gravity_len,
                manifest.world.dimension,
                manifest.physics.gravity.len()
            ),
        ));
    }

    // Tick rate in range.
    if manifest.physics.tick_rate_hz < MIN_TICK_RATE
        || manifest.physics.tick_rate_hz > MAX_TICK_RATE
    {
        errors.push(ValidationError::new(
            "physics.tick_rate_hz",
            format!(
                "tick_rate_hz must be between {} and {}",
                MIN_TICK_RATE, MAX_TICK_RATE
            ),
        ));
    }

    // Max players > 0.
    if manifest.players.max_players == 0 {
        errors.push(ValidationError::new(
            "players.max_players",
            "max_players must be greater than 0",
        ));
    }

    // Scenes list not empty.
    if manifest.scenes.list.is_empty() {
        errors.push(ValidationError::new(
            "scenes.list",
            "scenes list must not be empty",
        ));
    }

    // Default scene in list.
    if !manifest.scenes.list.contains(&manifest.scenes.default) {
        errors.push(ValidationError::new(
            "scenes.default",
            "default scene must be in the scenes list",
        ));
    }

    errors
}

/// Check if a string is valid semver: "major.minor.patch" with all-numeric parts.
fn is_valid_semver(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() != 3 {
        return false;
    }
    parts
        .iter()
        .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

// ---------------------------------------------------------------------------
// Serialization functions
// ---------------------------------------------------------------------------

/// Serialize a world project manifest to a JSON string.
pub fn serialize_manifest(manifest: &WorldProjectManifest) -> Result<String, SerdeError> {
    serde_json::to_string_pretty(manifest)
}

/// Deserialize a world project manifest from a JSON string.
pub fn deserialize_manifest(json: &str) -> Result<WorldProjectManifest, SerdeError> {
    serde_json::from_str(json)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a valid 3D manifest.
    fn valid_3d_manifest() -> WorldProjectManifest {
        WorldProjectManifest {
            world: WorldInfo {
                name: "Test World".to_string(),
                version: "1.0.0".to_string(),
                dimension: WorldDimension::ThreeD,
                description: "A test world".to_string(),
            },
            physics: PhysicsConfig {
                gravity: vec![0.0, -9.81, 0.0],
                tick_rate_hz: 60,
            },
            players: PlayerConfig {
                max_players: 50,
                spawn_scene: "lobby".to_string(),
            },
            scenes: SceneConfig {
                default: "lobby".to_string(),
                list: vec!["lobby".to_string(), "arena".to_string()],
            },
            camera: None,
            environment: Some(EnvironmentConfig {
                skybox: Some("sky_day.hdr".to_string()),
                fog_density: Some(0.02),
                background_color: None,
                parallax_layers: None,
            }),
        }
    }

    /// Helper: build a valid 2D manifest.
    fn valid_2d_manifest() -> WorldProjectManifest {
        WorldProjectManifest {
            world: WorldInfo {
                name: "Pixel Quest".to_string(),
                version: "0.1.0".to_string(),
                dimension: WorldDimension::TwoD,
                description: "A 2D platformer".to_string(),
            },
            physics: PhysicsConfig {
                gravity: vec![0.0, -9.81],
                tick_rate_hz: 60,
            },
            players: PlayerConfig {
                max_players: 4,
                spawn_scene: "level1".to_string(),
            },
            scenes: SceneConfig {
                default: "level1".to_string(),
                list: vec!["level1".to_string(), "level2".to_string()],
            },
            camera: Some(CameraConfig2D {
                mode: CameraMode2D::SideView,
                pixels_per_unit: 16,
                bounds: Some(Bounds2D {
                    min: [0.0, 0.0],
                    max: [1024.0, 768.0],
                }),
            }),
            environment: Some(EnvironmentConfig {
                skybox: None,
                fog_density: None,
                background_color: Some([0.2, 0.3, 0.8]),
                parallax_layers: Some(vec![
                    ParallaxLayer {
                        image: "bg_mountains.png".to_string(),
                        speed: 0.2,
                    },
                    ParallaxLayer {
                        image: "bg_trees.png".to_string(),
                        speed: 0.5,
                    },
                ]),
            }),
        }
    }

    // -- Serialization round-trip --------------------------------------------

    #[test]
    fn test_manifest_3d_round_trip() {
        let m = valid_3d_manifest();
        let json = serialize_manifest(&m).unwrap();
        let back = deserialize_manifest(&json).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn test_manifest_2d_round_trip() {
        let m = valid_2d_manifest();
        let json = serialize_manifest(&m).unwrap();
        let back = deserialize_manifest(&json).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn test_manifest_no_optional_fields() {
        let m = WorldProjectManifest {
            world: WorldInfo {
                name: "Bare".to_string(),
                version: "0.0.1".to_string(),
                dimension: WorldDimension::ThreeD,
                description: String::new(),
            },
            physics: PhysicsConfig {
                gravity: vec![0.0, -9.81, 0.0],
                tick_rate_hz: 30,
            },
            players: PlayerConfig {
                max_players: 1,
                spawn_scene: "main".to_string(),
            },
            scenes: SceneConfig {
                default: "main".to_string(),
                list: vec!["main".to_string()],
            },
            camera: None,
            environment: None,
        };
        let json = serialize_manifest(&m).unwrap();
        let back = deserialize_manifest(&json).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn test_deserialize_manifest_invalid_json() {
        let result = deserialize_manifest("not json at all");
        assert!(result.is_err());
    }

    #[test]
    fn test_serialize_manifest_produces_valid_json() {
        let m = valid_3d_manifest();
        let json = serialize_manifest(&m).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["world"]["name"], "Test World");
    }

    // -- Validation: valid manifests -----------------------------------------

    #[test]
    fn test_validate_valid_3d_manifest() {
        let m = valid_3d_manifest();
        let errors = validate_manifest(&m);
        assert!(errors.is_empty(), "expected no errors, got: {:?}", errors);
    }

    #[test]
    fn test_validate_valid_2d_manifest() {
        let m = valid_2d_manifest();
        let errors = validate_manifest(&m);
        assert!(errors.is_empty(), "expected no errors, got: {:?}", errors);
    }

    // -- Validation: name ----------------------------------------------------

    #[test]
    fn test_validate_empty_name() {
        let mut m = valid_3d_manifest();
        m.world.name = String::new();
        let errors = validate_manifest(&m);
        assert!(errors.iter().any(|e| e.field == "world.name"));
    }

    // -- Validation: version -------------------------------------------------

    #[test]
    fn test_validate_valid_versions() {
        for v in ["0.0.1", "1.0.0", "99.99.99", "0.1.0"] {
            let mut m = valid_3d_manifest();
            m.world.version = v.to_string();
            let errors = validate_manifest(&m);
            assert!(
                !errors.iter().any(|e| e.field == "world.version"),
                "version '{}' should be valid",
                v
            );
        }
    }

    #[test]
    fn test_validate_invalid_versions() {
        for v in ["", "1", "1.0", "1.0.0.0", "a.b.c", "1.0.x", "v1.0.0"] {
            let mut m = valid_3d_manifest();
            m.world.version = v.to_string();
            let errors = validate_manifest(&m);
            assert!(
                errors.iter().any(|e| e.field == "world.version"),
                "version '{}' should be invalid",
                v
            );
        }
    }

    // -- Validation: gravity dimension mismatch ------------------------------

    #[test]
    fn test_validate_gravity_2d_correct() {
        let mut m = valid_2d_manifest();
        m.physics.gravity = vec![0.0, -9.81];
        let errors = validate_manifest(&m);
        assert!(!errors.iter().any(|e| e.field == "physics.gravity"));
    }

    #[test]
    fn test_validate_gravity_2d_wrong_3_components() {
        let mut m = valid_2d_manifest();
        m.physics.gravity = vec![0.0, -9.81, 0.0];
        let errors = validate_manifest(&m);
        assert!(errors.iter().any(|e| e.field == "physics.gravity"));
    }

    #[test]
    fn test_validate_gravity_3d_correct() {
        let mut m = valid_3d_manifest();
        m.physics.gravity = vec![0.0, -9.81, 0.0];
        let errors = validate_manifest(&m);
        assert!(!errors.iter().any(|e| e.field == "physics.gravity"));
    }

    #[test]
    fn test_validate_gravity_3d_wrong_2_components() {
        let mut m = valid_3d_manifest();
        m.physics.gravity = vec![0.0, -9.81];
        let errors = validate_manifest(&m);
        assert!(errors.iter().any(|e| e.field == "physics.gravity"));
    }

    #[test]
    fn test_validate_gravity_empty() {
        let mut m = valid_3d_manifest();
        m.physics.gravity = vec![];
        let errors = validate_manifest(&m);
        assert!(errors.iter().any(|e| e.field == "physics.gravity"));
    }

    #[test]
    fn test_validate_gravity_too_many_components() {
        let mut m = valid_3d_manifest();
        m.physics.gravity = vec![0.0, -9.81, 0.0, 1.0];
        let errors = validate_manifest(&m);
        assert!(errors.iter().any(|e| e.field == "physics.gravity"));
    }

    // -- Validation: tick rate -----------------------------------------------

    #[test]
    fn test_validate_tick_rate_valid_boundaries() {
        for rate in [1, 60, 120, 240] {
            let mut m = valid_3d_manifest();
            m.physics.tick_rate_hz = rate;
            let errors = validate_manifest(&m);
            assert!(
                !errors.iter().any(|e| e.field == "physics.tick_rate_hz"),
                "tick_rate {} should be valid",
                rate
            );
        }
    }

    #[test]
    fn test_validate_tick_rate_zero() {
        let mut m = valid_3d_manifest();
        m.physics.tick_rate_hz = 0;
        let errors = validate_manifest(&m);
        assert!(errors.iter().any(|e| e.field == "physics.tick_rate_hz"));
    }

    #[test]
    fn test_validate_tick_rate_too_high() {
        let mut m = valid_3d_manifest();
        m.physics.tick_rate_hz = 241;
        let errors = validate_manifest(&m);
        assert!(errors.iter().any(|e| e.field == "physics.tick_rate_hz"));
    }

    // -- Validation: max players ---------------------------------------------

    #[test]
    fn test_validate_max_players_zero() {
        let mut m = valid_3d_manifest();
        m.players.max_players = 0;
        let errors = validate_manifest(&m);
        assert!(errors.iter().any(|e| e.field == "players.max_players"));
    }

    #[test]
    fn test_validate_max_players_one() {
        let mut m = valid_3d_manifest();
        m.players.max_players = 1;
        let errors = validate_manifest(&m);
        assert!(!errors.iter().any(|e| e.field == "players.max_players"));
    }

    // -- Validation: scenes list ---------------------------------------------

    #[test]
    fn test_validate_scenes_list_empty() {
        let mut m = valid_3d_manifest();
        m.scenes.list = vec![];
        let errors = validate_manifest(&m);
        assert!(errors.iter().any(|e| e.field == "scenes.list"));
    }

    #[test]
    fn test_validate_default_scene_not_in_list() {
        let mut m = valid_3d_manifest();
        m.scenes.default = "nonexistent".to_string();
        let errors = validate_manifest(&m);
        assert!(errors.iter().any(|e| e.field == "scenes.default"));
    }

    #[test]
    fn test_validate_default_scene_in_list() {
        let m = valid_3d_manifest();
        let errors = validate_manifest(&m);
        assert!(!errors.iter().any(|e| e.field == "scenes.default"));
    }

    // -- Validation: multiple errors at once ---------------------------------

    #[test]
    fn test_validate_multiple_errors() {
        let m = WorldProjectManifest {
            world: WorldInfo {
                name: String::new(),        // empty
                version: "bad".to_string(), // invalid
                dimension: WorldDimension::ThreeD,
                description: String::new(),
            },
            physics: PhysicsConfig {
                gravity: vec![0.0], // wrong for 3D
                tick_rate_hz: 0,    // too low
            },
            players: PlayerConfig {
                max_players: 0, // zero
                spawn_scene: "x".to_string(),
            },
            scenes: SceneConfig {
                default: "missing".to_string(),
                list: vec![], // empty
            },
            camera: None,
            environment: None,
        };
        let errors = validate_manifest(&m);
        // Should have errors for: name, version, gravity, tick_rate, max_players, list, default
        assert!(errors.len() >= 7, "expected >=7 errors, got: {:?}", errors);
    }

    // -- CameraMode2D --------------------------------------------------------

    #[test]
    fn test_camera_mode_default() {
        assert_eq!(CameraMode2D::default(), CameraMode2D::SideView);
    }

    #[test]
    fn test_camera_mode_round_trip() {
        for mode in [CameraMode2D::SideView, CameraMode2D::TopDown] {
            let json = serde_json::to_string(&mode).unwrap();
            let back: CameraMode2D = serde_json::from_str(&json).unwrap();
            assert_eq!(mode, back);
        }
    }

    // -- CameraConfig2D round-trip -------------------------------------------

    #[test]
    fn test_camera_config_round_trip() {
        let c = CameraConfig2D {
            mode: CameraMode2D::TopDown,
            pixels_per_unit: 32,
            bounds: Some(Bounds2D {
                min: [-100.0, -100.0],
                max: [100.0, 100.0],
            }),
        };
        let json = serde_json::to_string(&c).unwrap();
        let back: CameraConfig2D = serde_json::from_str(&json).unwrap();
        assert_eq!(c, back);
    }

    #[test]
    fn test_camera_config_no_bounds() {
        let c = CameraConfig2D {
            mode: CameraMode2D::SideView,
            pixels_per_unit: 16,
            bounds: None,
        };
        let json = serde_json::to_string(&c).unwrap();
        let back: CameraConfig2D = serde_json::from_str(&json).unwrap();
        assert_eq!(c, back);
    }

    // -- EnvironmentConfig round-trip ----------------------------------------

    #[test]
    fn test_environment_config_3d_round_trip() {
        let e = EnvironmentConfig {
            skybox: Some("sunset.hdr".to_string()),
            fog_density: Some(0.05),
            background_color: None,
            parallax_layers: None,
        };
        let json = serde_json::to_string(&e).unwrap();
        let back: EnvironmentConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(e, back);
    }

    #[test]
    fn test_environment_config_2d_round_trip() {
        let e = EnvironmentConfig {
            skybox: None,
            fog_density: None,
            background_color: Some([0.1, 0.2, 0.3]),
            parallax_layers: Some(vec![ParallaxLayer {
                image: "bg.png".to_string(),
                speed: 0.5,
            }]),
        };
        let json = serde_json::to_string(&e).unwrap();
        let back: EnvironmentConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(e, back);
    }

    #[test]
    fn test_environment_config_all_none() {
        let e = EnvironmentConfig {
            skybox: None,
            fog_density: None,
            background_color: None,
            parallax_layers: None,
        };
        let json = serde_json::to_string(&e).unwrap();
        let back: EnvironmentConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(e, back);
    }

    // -- ParallaxLayer -------------------------------------------------------

    #[test]
    fn test_parallax_layer_round_trip() {
        let p = ParallaxLayer {
            image: "clouds.png".to_string(),
            speed: 0.1,
        };
        let json = serde_json::to_string(&p).unwrap();
        let back: ParallaxLayer = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
    }

    // -- Bounds2D ------------------------------------------------------------

    #[test]
    fn test_bounds2d_round_trip() {
        let b = Bounds2D {
            min: [-50.0, -50.0],
            max: [50.0, 50.0],
        };
        let json = serde_json::to_string(&b).unwrap();
        let back: Bounds2D = serde_json::from_str(&json).unwrap();
        assert_eq!(b, back);
    }

    // -- is_valid_semver helper ----------------------------------------------

    #[test]
    fn test_semver_valid() {
        assert!(is_valid_semver("0.0.0"));
        assert!(is_valid_semver("1.2.3"));
        assert!(is_valid_semver("10.20.30"));
    }

    #[test]
    fn test_semver_invalid() {
        assert!(!is_valid_semver(""));
        assert!(!is_valid_semver("1"));
        assert!(!is_valid_semver("1.2"));
        assert!(!is_valid_semver("1.2.3.4"));
        assert!(!is_valid_semver("a.b.c"));
        assert!(!is_valid_semver(".1.2"));
        assert!(!is_valid_semver("1..2"));
    }
}
