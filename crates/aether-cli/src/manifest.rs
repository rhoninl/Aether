use serde::{Deserialize, Serialize};

const MANIFEST_FILE: &str = "world.toml";

/// Top-level world.toml structure with nested sections.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldToml {
    pub world: WorldManifest,
    #[serde(default)]
    pub physics: Option<PhysicsConfig>,
    #[serde(default)]
    pub camera: Option<CameraConfig>,
    #[serde(default)]
    pub players: Option<PlayersConfig>,
    #[serde(default)]
    pub scenes: Option<ScenesConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldManifest {
    pub name: String,
    pub version: String,
    pub dimension: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicsConfig {
    pub gravity: Vec<f64>,
    #[serde(default = "default_tick_rate")]
    pub tick_rate_hz: u32,
}

fn default_tick_rate() -> u32 {
    60
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraConfig {
    pub mode: String,
    pub pixels_per_unit: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayersConfig {
    pub max_players: u32,
    pub spawn_scene: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenesConfig {
    pub default: String,
    pub list: Vec<String>,
}

impl WorldManifest {
    pub fn default_for(name: &str, dimension: &str) -> Self {
        Self {
            name: name.to_string(),
            version: "0.1.0".to_string(),
            dimension: dimension.to_string(),
            description: format!("An Aether world: {name}"),
        }
    }
}

impl WorldToml {
    pub fn default_for(name: &str, dimension: &str) -> Self {
        let world = WorldManifest::default_for(name, dimension);

        let physics = match dimension {
            "3D" => Some(PhysicsConfig {
                gravity: vec![0.0, -9.81, 0.0],
                tick_rate_hz: 60,
            }),
            "2D" => Some(PhysicsConfig {
                gravity: vec![0.0, -9.81],
                tick_rate_hz: 60,
            }),
            _ => None,
        };

        let camera = match dimension {
            "2D" => Some(CameraConfig {
                mode: "SideView".to_string(),
                pixels_per_unit: 32,
            }),
            _ => None,
        };

        let max_players = match dimension {
            "3D" => 32,
            _ => 4,
        };

        let players = Some(PlayersConfig {
            max_players,
            spawn_scene: "main".to_string(),
        });

        let scenes = Some(ScenesConfig {
            default: "main".to_string(),
            list: vec!["main".to_string()],
        });

        Self {
            world,
            physics,
            camera,
            players,
            scenes,
        }
    }
}

/// Load and parse a world.toml from the given directory.
pub fn load_manifest(dir: &std::path::Path) -> Result<WorldToml, String> {
    let path = dir.join(MANIFEST_FILE);
    if !path.exists() {
        return Err(format!(
            "'{}' not found in {}",
            MANIFEST_FILE,
            dir.display()
        ));
    }
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    let world_toml: WorldToml =
        toml::from_str(&content).map_err(|e| format!("invalid {}: {e}", MANIFEST_FILE))?;
    Ok(world_toml)
}

/// Validate a loaded manifest. Returns a list of error messages.
pub fn validate_manifest(dir: &std::path::Path, world_toml: &WorldToml) -> Vec<String> {
    let mut errors = Vec::new();
    let manifest = &world_toml.world;

    if manifest.name.is_empty() {
        errors.push("'name' is empty".to_string());
    }
    if manifest.version.is_empty() {
        errors.push("'version' is empty".to_string());
    }
    if manifest.dimension != "2D" && manifest.dimension != "3D" {
        errors.push(format!(
            "invalid dimension '{}': must be \"2D\" or \"3D\"",
            manifest.dimension
        ));
    }

    // Validate dimension-specific directories
    let dimension = manifest.dimension.as_str();
    let required_dirs: &[&str] = match dimension {
        "3D" => &["scenes", "assets", "terrain"],
        "2D" => &["scenes", "assets", "tilemaps"],
        _ => &[],
    };
    for d in required_dirs {
        if !dir.join(d).is_dir() {
            errors.push(format!("required directory '{}' is missing", d));
        }
    }

    // Validate scene files exist
    if let Some(scenes_config) = &world_toml.scenes {
        for scene_name in &scenes_config.list {
            let scene_file = dir.join("scenes").join(format!("{scene_name}.scene.toml"));
            if !scene_file.exists() {
                errors.push(format!(
                    "scene '{}' listed in scenes.list but 'scenes/{}.scene.toml' not found",
                    scene_name, scene_name
                ));
            }
        }
    }

    // Check .aether directory
    if !dir.join(".aether").join("versions.toml").exists() {
        errors.push("'.aether/versions.toml' is missing".to_string());
    }

    errors
}

pub fn manifest_filename() -> &'static str {
    MANIFEST_FILE
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_default_for_3d_generates_correct_manifest() {
        let m = WorldManifest::default_for("test", "3D");
        assert_eq!(m.name, "test");
        assert_eq!(m.version, "0.1.0");
        assert_eq!(m.dimension, "3D");
        assert_eq!(m.description, "An Aether world: test");
    }

    #[test]
    fn test_default_for_2d_generates_correct_manifest() {
        let m = WorldManifest::default_for("test", "2D");
        assert_eq!(m.name, "test");
        assert_eq!(m.version, "0.1.0");
        assert_eq!(m.dimension, "2D");
        assert_eq!(m.description, "An Aether world: test");
    }

    #[test]
    fn test_world_toml_default_for_3d() {
        let wt = WorldToml::default_for("my-world", "3D");
        assert_eq!(wt.world.dimension, "3D");
        assert!(wt.camera.is_none());
        let physics = wt.physics.unwrap();
        assert_eq!(physics.gravity, vec![0.0, -9.81, 0.0]);
        assert_eq!(physics.tick_rate_hz, 60);
        let players = wt.players.unwrap();
        assert_eq!(players.max_players, 32);
        let scenes = wt.scenes.unwrap();
        assert_eq!(scenes.default, "main");
        assert_eq!(scenes.list, vec!["main"]);
    }

    #[test]
    fn test_world_toml_default_for_2d() {
        let wt = WorldToml::default_for("my-game", "2D");
        assert_eq!(wt.world.dimension, "2D");
        let camera = wt.camera.unwrap();
        assert_eq!(camera.mode, "SideView");
        assert_eq!(camera.pixels_per_unit, 32);
        let physics = wt.physics.unwrap();
        assert_eq!(physics.gravity, vec![0.0, -9.81]);
        let players = wt.players.unwrap();
        assert_eq!(players.max_players, 4);
    }

    #[test]
    fn test_validation_passes_for_valid_3d_manifest() {
        let dir = TempDir::new().unwrap();
        // Create required structure
        fs::create_dir_all(dir.path().join("scenes")).unwrap();
        fs::create_dir_all(dir.path().join("assets")).unwrap();
        fs::create_dir_all(dir.path().join("terrain")).unwrap();
        fs::create_dir_all(dir.path().join(".aether")).unwrap();
        fs::write(
            dir.path().join("scenes/main.scene.toml"),
            "[scene]\nname = \"Main\"",
        )
        .unwrap();
        fs::write(
            dir.path().join(".aether/versions.toml"),
            "# Aether world version history",
        )
        .unwrap();

        let wt = WorldToml::default_for("valid-3d", "3D");
        let errors = validate_manifest(dir.path(), &wt);
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn test_validation_passes_for_valid_2d_manifest() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("scenes")).unwrap();
        fs::create_dir_all(dir.path().join("assets")).unwrap();
        fs::create_dir_all(dir.path().join("tilemaps")).unwrap();
        fs::create_dir_all(dir.path().join(".aether")).unwrap();
        fs::write(
            dir.path().join("scenes/main.scene.toml"),
            "[scene]\nname = \"Main\"",
        )
        .unwrap();
        fs::write(
            dir.path().join(".aether/versions.toml"),
            "# Aether world version history",
        )
        .unwrap();

        let wt = WorldToml::default_for("valid-2d", "2D");
        let errors = validate_manifest(dir.path(), &wt);
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn test_validation_fails_for_invalid_dimension() {
        let dir = TempDir::new().unwrap();
        let wt = WorldToml {
            world: WorldManifest {
                name: "test".to_string(),
                version: "0.1.0".to_string(),
                dimension: "4D".to_string(),
                description: String::new(),
            },
            physics: None,
            camera: None,
            players: None,
            scenes: None,
        };
        let errors = validate_manifest(dir.path(), &wt);
        assert!(errors.iter().any(|e| e.contains("invalid dimension")));
    }

    #[test]
    fn test_validation_fails_for_empty_name() {
        let dir = TempDir::new().unwrap();
        let wt = WorldToml {
            world: WorldManifest {
                name: String::new(),
                version: "0.1.0".to_string(),
                dimension: "3D".to_string(),
                description: String::new(),
            },
            physics: None,
            camera: None,
            players: None,
            scenes: None,
        };
        let errors = validate_manifest(dir.path(), &wt);
        assert!(errors.iter().any(|e| e.contains("name")));
    }

    #[test]
    fn test_serialization_roundtrip_3d() {
        let wt = WorldToml::default_for("roundtrip", "3D");
        let toml_str = toml::to_string_pretty(&wt).unwrap();
        let parsed: WorldToml = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.world.name, "roundtrip");
        assert_eq!(parsed.world.version, "0.1.0");
        assert_eq!(parsed.world.dimension, "3D");
    }

    #[test]
    fn test_serialization_roundtrip_2d() {
        let wt = WorldToml::default_for("roundtrip-2d", "2D");
        let toml_str = toml::to_string_pretty(&wt).unwrap();
        let parsed: WorldToml = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.world.name, "roundtrip-2d");
        assert_eq!(parsed.world.dimension, "2D");
        assert!(parsed.camera.is_some());
    }

    #[test]
    fn test_load_manifest_missing() {
        let dir = TempDir::new().unwrap();
        let result = load_manifest(dir.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_load_manifest_valid_nested_format() {
        let dir = TempDir::new().unwrap();
        let content = r#"
[world]
name = "my-world"
version = "0.1.0"
dimension = "3D"
description = "test"

[physics]
gravity = [0.0, -9.81, 0.0]
tick_rate_hz = 60

[players]
max_players = 32
spawn_scene = "main"

[scenes]
default = "main"
list = ["main"]
"#;
        fs::write(dir.path().join("world.toml"), content).unwrap();
        let wt = load_manifest(dir.path()).unwrap();
        assert_eq!(wt.world.name, "my-world");
        assert_eq!(wt.world.dimension, "3D");
    }

    #[test]
    fn test_load_manifest_invalid_toml() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("world.toml"), "not valid { toml").unwrap();
        let result = load_manifest(dir.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid"));
    }

    #[test]
    fn test_validation_missing_scenes_dir() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("assets")).unwrap();
        fs::create_dir_all(dir.path().join("terrain")).unwrap();
        fs::create_dir_all(dir.path().join(".aether")).unwrap();
        fs::write(dir.path().join(".aether/versions.toml"), "# versions").unwrap();

        let wt = WorldToml::default_for("test", "3D");
        let errors = validate_manifest(dir.path(), &wt);
        assert!(errors.iter().any(|e| e.contains("scenes")));
    }

    #[test]
    fn test_validation_missing_scene_file() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("scenes")).unwrap();
        fs::create_dir_all(dir.path().join("assets")).unwrap();
        fs::create_dir_all(dir.path().join("terrain")).unwrap();
        fs::create_dir_all(dir.path().join(".aether")).unwrap();
        fs::write(dir.path().join(".aether/versions.toml"), "# versions").unwrap();
        // Note: scenes/main.scene.toml does NOT exist

        let wt = WorldToml::default_for("test", "3D");
        let errors = validate_manifest(dir.path(), &wt);
        assert!(errors
            .iter()
            .any(|e| e.contains("main") && e.contains("not found")));
    }

    #[test]
    fn test_validation_missing_aether_dir() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("scenes")).unwrap();
        fs::create_dir_all(dir.path().join("assets")).unwrap();
        fs::create_dir_all(dir.path().join("terrain")).unwrap();
        fs::write(
            dir.path().join("scenes/main.scene.toml"),
            "[scene]\nname = \"Main\"",
        )
        .unwrap();

        let wt = WorldToml::default_for("test", "3D");
        let errors = validate_manifest(dir.path(), &wt);
        assert!(errors.iter().any(|e| e.contains("versions.toml")));
    }
}
