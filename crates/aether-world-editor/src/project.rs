//! World project I/O -- scaffold, load, and save world projects.
//!
//! A world project is a directory on disk containing `world.toml`, scene files,
//! scripts, assets, and version history in `.aether/versions.toml`.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::ProjectError;
use crate::mode::WorldDimension;
use crate::version::{deserialize_version_history, serialize_version_history, VersionHistory};

// ---------------------------------------------------------------------------
// Manifest types
// ---------------------------------------------------------------------------

/// Top-level world project manifest, deserialized from `world.toml`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorldProjectManifest {
    pub world: WorldSection,
    pub physics: PhysicsSection,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub camera: Option<CameraSection>,
    pub players: PlayersSection,
    pub scenes: ScenesSection,
}

/// The `[world]` table.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorldSection {
    pub name: String,
    pub version: String,
    pub dimension: String,
    pub description: String,
}

/// The `[physics]` table.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PhysicsSection {
    pub gravity: Vec<f64>,
    pub tick_rate_hz: u32,
}

/// The `[camera]` table (2D only).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CameraSection {
    pub mode: String,
    pub pixels_per_unit: u32,
}

/// The `[players]` table.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlayersSection {
    pub max_players: u32,
    pub spawn_scene: String,
}

/// The `[scenes]` table.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScenesSection {
    pub default: String,
    pub list: Vec<String>,
}

/// Represents a loaded world project.
#[derive(Debug, Clone)]
pub struct WorldProject {
    pub root: PathBuf,
    pub manifest: WorldProjectManifest,
    pub version_history: VersionHistory,
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const WORLD_MANIFEST_FILE: &str = "world.toml";
const VERSION_HISTORY_DIR: &str = ".aether";
const VERSION_HISTORY_FILE: &str = "versions.toml";
const SCENES_DIR: &str = "scenes";
const SCRIPTS_DIR: &str = "scripts";
const ASSETS_DIR: &str = "assets";
const MAIN_SCENE_FILE: &str = "main.scene.toml";

const DEFAULT_TICK_RATE_HZ: u32 = 60;
const DEFAULT_MAX_PLAYERS_3D: u32 = 32;
const DEFAULT_MAX_PLAYERS_2D: u32 = 4;
const DEFAULT_PIXELS_PER_UNIT: u32 = 32;
const DEFAULT_VERSION: &str = "0.1.0";

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Scaffold a new world project directory.
///
/// Creates the full directory tree, `world.toml`, a default scene, and the
/// `.aether/versions.toml` file. Returns an error if the root directory
/// already exists.
pub fn scaffold_project(
    root: &Path,
    name: &str,
    dimension: WorldDimension,
) -> Result<(), ProjectError> {
    if root.exists() {
        return Err(ProjectError::AlreadyExists(root.display().to_string()));
    }

    // Create top-level directory.
    fs::create_dir_all(root)?;

    // Create sub-directories.
    create_directories(root, dimension)?;

    // Write world.toml.
    let manifest = build_default_manifest(name, dimension);
    let toml_str =
        toml::to_string_pretty(&manifest).map_err(|e| ProjectError::Manifest(e.to_string()))?;
    fs::write(root.join(WORLD_MANIFEST_FILE), toml_str)?;

    // Write default scene.
    let scene_content = build_default_scene(name, dimension);
    fs::write(root.join(SCENES_DIR).join(MAIN_SCENE_FILE), scene_content)?;

    // Write empty version history.
    let history = VersionHistory::new();
    let history_toml =
        serialize_version_history(&history).map_err(|e| ProjectError::Manifest(e.to_string()))?;
    fs::write(
        root.join(VERSION_HISTORY_DIR).join(VERSION_HISTORY_FILE),
        history_toml,
    )?;

    Ok(())
}

/// Load a world project from disk.
pub fn load_project(root: &Path) -> Result<WorldProject, ProjectError> {
    let manifest_path = root.join(WORLD_MANIFEST_FILE);
    if !manifest_path.exists() {
        return Err(ProjectError::InvalidProject(format!(
            "missing {} in {}",
            WORLD_MANIFEST_FILE,
            root.display()
        )));
    }

    let manifest_str = fs::read_to_string(&manifest_path)?;
    let manifest: WorldProjectManifest =
        toml::from_str(&manifest_str).map_err(|e| ProjectError::Manifest(e.to_string()))?;

    let version_history = load_version_history(root)?;

    Ok(WorldProject {
        root: root.to_path_buf(),
        manifest,
        version_history,
    })
}

/// Save the world project manifest to `world.toml`.
pub fn save_manifest(project: &WorldProject) -> Result<(), ProjectError> {
    let toml_str = toml::to_string_pretty(&project.manifest)
        .map_err(|e| ProjectError::Manifest(e.to_string()))?;
    fs::write(project.root.join(WORLD_MANIFEST_FILE), toml_str)?;
    Ok(())
}

/// Load version history from `.aether/versions.toml`.
pub fn load_version_history(root: &Path) -> Result<VersionHistory, ProjectError> {
    let path = root.join(VERSION_HISTORY_DIR).join(VERSION_HISTORY_FILE);
    if !path.exists() {
        return Ok(VersionHistory::new());
    }
    let content = fs::read_to_string(&path)?;
    let history = deserialize_version_history(&content)?;
    Ok(history)
}

/// Save version history to `.aether/versions.toml`.
pub fn save_version_history(root: &Path, history: &VersionHistory) -> Result<(), ProjectError> {
    let dir = root.join(VERSION_HISTORY_DIR);
    fs::create_dir_all(&dir)?;
    let toml_str =
        serialize_version_history(history).map_err(|e| ProjectError::Manifest(e.to_string()))?;
    fs::write(dir.join(VERSION_HISTORY_FILE), toml_str)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn create_directories(root: &Path, dimension: WorldDimension) -> Result<(), ProjectError> {
    fs::create_dir_all(root.join(SCENES_DIR))?;
    fs::create_dir_all(root.join(SCRIPTS_DIR))?;
    fs::create_dir_all(root.join(ASSETS_DIR).join("audio"))?;
    fs::create_dir_all(root.join(VERSION_HISTORY_DIR))?;

    match dimension {
        WorldDimension::ThreeD => {
            fs::create_dir_all(root.join(ASSETS_DIR).join("meshes"))?;
            fs::create_dir_all(root.join(ASSETS_DIR).join("textures"))?;
            fs::create_dir_all(root.join("terrain"))?;
        }
        WorldDimension::TwoD => {
            fs::create_dir_all(root.join(ASSETS_DIR).join("sprites"))?;
            fs::create_dir_all(root.join(ASSETS_DIR).join("tilesets"))?;
            fs::create_dir_all(root.join("tilemaps"))?;
        }
    }

    Ok(())
}

fn build_default_manifest(name: &str, dimension: WorldDimension) -> WorldProjectManifest {
    let (dim_str, gravity, camera, max_players) = match dimension {
        WorldDimension::ThreeD => (
            "3D".to_string(),
            vec![0.0, -9.81, 0.0],
            None,
            DEFAULT_MAX_PLAYERS_3D,
        ),
        WorldDimension::TwoD => (
            "2D".to_string(),
            vec![0.0, -9.81],
            Some(CameraSection {
                mode: "SideView".to_string(),
                pixels_per_unit: DEFAULT_PIXELS_PER_UNIT,
            }),
            DEFAULT_MAX_PLAYERS_2D,
        ),
    };

    WorldProjectManifest {
        world: WorldSection {
            name: name.to_string(),
            version: DEFAULT_VERSION.to_string(),
            dimension: dim_str,
            description: String::new(),
        },
        physics: PhysicsSection {
            gravity,
            tick_rate_hz: DEFAULT_TICK_RATE_HZ,
        },
        camera,
        players: PlayersSection {
            max_players,
            spawn_scene: "main".to_string(),
        },
        scenes: ScenesSection {
            default: "main".to_string(),
            list: vec!["main".to_string()],
        },
    }
}

fn build_default_scene(name: &str, dimension: WorldDimension) -> String {
    match dimension {
        WorldDimension::ThreeD => format!(
            r#"[scene]
name = "{name}"
description = "Default scene"

[[entities]]
id = "spawn-main"
kind = "SpawnPoint"

[entities.transform]
position = [0.0, 2.0, 0.0]
rotation = [0.0, 0.0, 0.0, 1.0]
"#,
        ),
        WorldDimension::TwoD => format!(
            r#"[scene]
name = "{name}"
description = "Default scene"

[[entities]]
id = "spawn-main"
kind = "SpawnPoint"

[entities.transform]
position = [0.0, 0.0]
angle = 0.0
"#,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::version::VersionRecord;
    use tempfile::TempDir;

    // -----------------------------------------------------------------------
    // Scaffold tests
    // -----------------------------------------------------------------------

    #[test]
    fn scaffold_3d_creates_correct_dirs() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("my-world-3d");
        scaffold_project(&root, "My World", WorldDimension::ThreeD).unwrap();

        assert!(root.join("world.toml").exists());
        assert!(root.join("scenes/main.scene.toml").exists());
        assert!(root.join("scripts").is_dir());
        assert!(root.join("assets/meshes").is_dir());
        assert!(root.join("assets/textures").is_dir());
        assert!(root.join("assets/audio").is_dir());
        assert!(root.join("terrain").is_dir());
        assert!(root.join(".aether/versions.toml").exists());

        // 2D-specific dirs should NOT exist.
        assert!(!root.join("assets/sprites").exists());
        assert!(!root.join("assets/tilesets").exists());
        assert!(!root.join("tilemaps").exists());
    }

    #[test]
    fn scaffold_2d_creates_correct_dirs() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("my-world-2d");
        scaffold_project(&root, "My Platformer", WorldDimension::TwoD).unwrap();

        assert!(root.join("world.toml").exists());
        assert!(root.join("scenes/main.scene.toml").exists());
        assert!(root.join("scripts").is_dir());
        assert!(root.join("assets/sprites").is_dir());
        assert!(root.join("assets/tilesets").is_dir());
        assert!(root.join("assets/audio").is_dir());
        assert!(root.join("tilemaps").is_dir());
        assert!(root.join(".aether/versions.toml").exists());

        // 3D-specific dirs should NOT exist.
        assert!(!root.join("assets/meshes").exists());
        assert!(!root.join("assets/textures").exists());
        assert!(!root.join("terrain").exists());
    }

    #[test]
    fn scaffold_3d_generates_valid_world_toml() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("test-3d");
        scaffold_project(&root, "Test World", WorldDimension::ThreeD).unwrap();

        let content = fs::read_to_string(root.join("world.toml")).unwrap();
        let manifest: WorldProjectManifest = toml::from_str(&content).unwrap();

        assert_eq!(manifest.world.name, "Test World");
        assert_eq!(manifest.world.dimension, "3D");
        assert_eq!(manifest.world.version, "0.1.0");
        assert_eq!(manifest.physics.gravity, vec![0.0, -9.81, 0.0]);
        assert!(manifest.camera.is_none());
        assert_eq!(manifest.players.max_players, 32);
    }

    #[test]
    fn scaffold_2d_generates_valid_world_toml() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("test-2d");
        scaffold_project(&root, "Test Platformer", WorldDimension::TwoD).unwrap();

        let content = fs::read_to_string(root.join("world.toml")).unwrap();
        let manifest: WorldProjectManifest = toml::from_str(&content).unwrap();

        assert_eq!(manifest.world.name, "Test Platformer");
        assert_eq!(manifest.world.dimension, "2D");
        assert_eq!(manifest.world.version, "0.1.0");
        assert_eq!(manifest.physics.gravity, vec![0.0, -9.81]);

        let camera = manifest.camera.unwrap();
        assert_eq!(camera.mode, "SideView");
        assert_eq!(camera.pixels_per_unit, 32);
        assert_eq!(manifest.players.max_players, 4);
    }

    #[test]
    fn scaffold_3d_default_scene_has_spawn_point() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("scene-test-3d");
        scaffold_project(&root, "Scene Test", WorldDimension::ThreeD).unwrap();

        let scene = fs::read_to_string(root.join("scenes/main.scene.toml")).unwrap();
        assert!(scene.contains("SpawnPoint"));
        assert!(scene.contains("position = [0.0, 2.0, 0.0]"));
        assert!(scene.contains("rotation = [0.0, 0.0, 0.0, 1.0]"));
    }

    #[test]
    fn scaffold_2d_default_scene_has_spawn_point() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("scene-test-2d");
        scaffold_project(&root, "Scene Test", WorldDimension::TwoD).unwrap();

        let scene = fs::read_to_string(root.join("scenes/main.scene.toml")).unwrap();
        assert!(scene.contains("SpawnPoint"));
        assert!(scene.contains("position = [0.0, 0.0]"));
        assert!(scene.contains("angle = 0.0"));
    }

    #[test]
    fn scaffold_existing_dir_returns_error() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("existing");
        fs::create_dir_all(&root).unwrap();

        let result = scaffold_project(&root, "Test", WorldDimension::ThreeD);
        assert!(result.is_err());
        match result.unwrap_err() {
            ProjectError::AlreadyExists(_) => {}
            other => panic!("expected AlreadyExists, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // Load tests
    // -----------------------------------------------------------------------

    #[test]
    fn load_project_reads_manifest() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("load-test");
        scaffold_project(&root, "Load Test", WorldDimension::ThreeD).unwrap();

        let project = load_project(&root).unwrap();
        assert_eq!(project.manifest.world.name, "Load Test");
        assert_eq!(project.manifest.world.dimension, "3D");
        assert_eq!(project.root, root);
    }

    #[test]
    fn load_project_nonexistent_returns_error() {
        let result = load_project(Path::new("/tmp/nonexistent-world-project-12345"));
        assert!(result.is_err());
        match result.unwrap_err() {
            ProjectError::InvalidProject(_) => {}
            other => panic!("expected InvalidProject, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // Save and reload tests
    // -----------------------------------------------------------------------

    #[test]
    fn save_and_reload_manifest_round_trip() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("round-trip");
        scaffold_project(&root, "Round Trip", WorldDimension::TwoD).unwrap();

        let mut project = load_project(&root).unwrap();
        project.manifest.world.description = "Updated description".to_string();
        project.manifest.players.max_players = 8;
        save_manifest(&project).unwrap();

        let reloaded = load_project(&root).unwrap();
        assert_eq!(reloaded.manifest.world.description, "Updated description");
        assert_eq!(reloaded.manifest.players.max_players, 8);
    }

    #[test]
    fn version_history_save_load_round_trip() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("ver-history");
        scaffold_project(&root, "Version Test", WorldDimension::ThreeD).unwrap();

        let mut history = VersionHistory::new();
        history
            .publish(VersionRecord {
                version: "0.1.0".to_string(),
                published_at: "2026-03-19T10:00:00Z".to_string(),
                changelog: "Initial release".to_string(),
                checksum: "sha256:abc".to_string(),
            })
            .unwrap();
        history
            .publish(VersionRecord {
                version: "0.2.0".to_string(),
                published_at: "2026-03-19T12:00:00Z".to_string(),
                changelog: "Second release".to_string(),
                checksum: "sha256:def".to_string(),
            })
            .unwrap();

        save_version_history(&root, &history).unwrap();
        let loaded = load_version_history(&root).unwrap();
        assert_eq!(history, loaded);
    }

    #[test]
    fn load_version_history_missing_file_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let history = load_version_history(tmp.path()).unwrap();
        assert!(history.versions.is_empty());
    }
}
