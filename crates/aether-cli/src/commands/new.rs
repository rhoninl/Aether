use std::fs;
use std::path::Path;

use crate::manifest::WorldToml;

const DEFAULT_3D_SCENE: &str = r#"[scene]
name = "Main Scene"
description = ""

[[entities]]
id = "spawn-001"
kind = "SpawnPoint"

[entities.transform]
position = [0.0, 1.0, 0.0]
rotation = [0.0, 0.0, 0.0, 1.0]
scale = [1.0, 1.0, 1.0]
"#;

const DEFAULT_2D_SCENE: &str = r#"[scene]
name = "Main Scene"
description = ""

[[entities]]
id = "spawn-001"
kind = "SpawnPoint"

[entities.transform]
position = [0.0, 0.0]
angle = 0.0
scale = [1.0, 1.0]
"#;

const VERSIONS_HEADER: &str = "# Aether world version history\n";

/// Default world.toml content for 3D worlds.
fn world_toml_3d(name: &str) -> String {
    format!(
        r#"[world]
name = "{name}"
version = "0.1.0"
dimension = "3D"
description = "An Aether world: {name}"

[physics]
gravity = [0.0, -9.81, 0.0]
tick_rate_hz = 60

[players]
max_players = 32
spawn_scene = "main"

[scenes]
default = "main"
list = ["main"]
"#
    )
}

/// Default world.toml content for 2D worlds.
fn world_toml_2d(name: &str) -> String {
    format!(
        r#"[world]
name = "{name}"
version = "0.1.0"
dimension = "2D"
description = "An Aether world: {name}"

[physics]
gravity = [0.0, -9.81]
tick_rate_hz = 60

[camera]
mode = "SideView"
pixels_per_unit = 32

[players]
max_players = 4
spawn_scene = "main"

[scenes]
default = "main"
list = ["main"]
"#
    )
}

/// Extract the short project name from a path (just the last component).
fn short_name(name: &str) -> &str {
    Path::new(name)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(name)
}

pub fn create_project(name: &str, dimension: &str) -> Result<(), String> {
    let path = Path::new(name);
    if path.exists() {
        return Err(format!("directory '{}' already exists", name));
    }

    let project_name = short_name(name);

    // Create common directories
    fs::create_dir_all(path.join("scenes"))
        .map_err(|e| format!("failed to create scenes directory: {e}"))?;
    fs::create_dir_all(path.join("scripts"))
        .map_err(|e| format!("failed to create scripts directory: {e}"))?;
    fs::create_dir_all(path.join("assets/audio"))
        .map_err(|e| format!("failed to create assets/audio directory: {e}"))?;
    fs::create_dir_all(path.join(".aether"))
        .map_err(|e| format!("failed to create .aether directory: {e}"))?;

    // Create dimension-specific directories
    match dimension {
        "3D" => {
            fs::create_dir_all(path.join("assets/meshes"))
                .map_err(|e| format!("failed to create assets/meshes directory: {e}"))?;
            fs::create_dir_all(path.join("assets/textures"))
                .map_err(|e| format!("failed to create assets/textures directory: {e}"))?;
            fs::create_dir_all(path.join("terrain"))
                .map_err(|e| format!("failed to create terrain directory: {e}"))?;
        }
        "2D" => {
            fs::create_dir_all(path.join("assets/sprites"))
                .map_err(|e| format!("failed to create assets/sprites directory: {e}"))?;
            fs::create_dir_all(path.join("assets/tilesets"))
                .map_err(|e| format!("failed to create assets/tilesets directory: {e}"))?;
            fs::create_dir_all(path.join("tilemaps"))
                .map_err(|e| format!("failed to create tilemaps directory: {e}"))?;
        }
        _ => {
            return Err(format!(
                "invalid dimension '{}': must be \"2D\" or \"3D\"",
                dimension
            ))
        }
    }

    // Write world.toml
    let world_toml_content = match dimension {
        "3D" => world_toml_3d(project_name),
        "2D" => world_toml_2d(project_name),
        _ => unreachable!(),
    };
    fs::write(path.join("world.toml"), &world_toml_content)
        .map_err(|e| format!("failed to write world.toml: {e}"))?;

    // Write default scene
    let scene_content = match dimension {
        "3D" => DEFAULT_3D_SCENE,
        "2D" => DEFAULT_2D_SCENE,
        _ => unreachable!(),
    };
    fs::write(path.join("scenes/main.scene.toml"), scene_content)
        .map_err(|e| format!("failed to write default scene: {e}"))?;

    // Write versions.toml
    fs::write(path.join(".aether/versions.toml"), VERSIONS_HEADER)
        .map_err(|e| format!("failed to write versions.toml: {e}"))?;

    // Verify the generated world.toml is valid by parsing it
    let _: WorldToml = toml::from_str(&world_toml_content)
        .map_err(|e| format!("generated invalid world.toml: {e}"))?;

    println!("Created new Aether {dimension} world '{project_name}'");
    println!();
    print_tree(project_name, dimension);
    println!();
    println!("Get started:");
    println!("  cd {name}");
    println!("  aether serve");

    Ok(())
}

fn print_tree(name: &str, dimension: &str) {
    println!("  {name}/");
    println!("  ├── world.toml");
    println!("  ├── scenes/");
    println!("  │   └── main.scene.toml");
    println!("  ├── scripts/");
    println!("  ├── assets/");
    match dimension {
        "3D" => {
            println!("  │   ├── meshes/");
            println!("  │   ├── textures/");
            println!("  │   └── audio/");
            println!("  ├── terrain/");
        }
        "2D" => {
            println!("  │   ├── sprites/");
            println!("  │   ├── tilesets/");
            println!("  │   └── audio/");
            println!("  ├── tilemaps/");
        }
        _ => {}
    }
    println!("  └── .aether/");
    println!("      └── versions.toml");
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_3d_scaffold_creates_all_required_directories() {
        let tmp = TempDir::new().unwrap();
        let project_path = tmp.path().join("test-3d");
        let name = project_path.to_str().unwrap();

        create_project(name, "3D").unwrap();

        assert!(project_path.join("scenes").is_dir());
        assert!(project_path.join("scripts").is_dir());
        assert!(project_path.join("assets").is_dir());
        assert!(project_path.join("assets/meshes").is_dir());
        assert!(project_path.join("assets/textures").is_dir());
        assert!(project_path.join("assets/audio").is_dir());
        assert!(project_path.join("terrain").is_dir());
        assert!(project_path.join(".aether").is_dir());
    }

    #[test]
    fn test_2d_scaffold_creates_all_required_directories() {
        let tmp = TempDir::new().unwrap();
        let project_path = tmp.path().join("test-2d");
        let name = project_path.to_str().unwrap();

        create_project(name, "2D").unwrap();

        assert!(project_path.join("scenes").is_dir());
        assert!(project_path.join("scripts").is_dir());
        assert!(project_path.join("assets").is_dir());
        assert!(project_path.join("assets/sprites").is_dir());
        assert!(project_path.join("assets/tilesets").is_dir());
        assert!(project_path.join("assets/audio").is_dir());
        assert!(project_path.join("tilemaps").is_dir());
        assert!(project_path.join(".aether").is_dir());
    }

    #[test]
    fn test_3d_scaffold_creates_valid_world_toml_with_dimension_3d() {
        let tmp = TempDir::new().unwrap();
        let project_path = tmp.path().join("test-3d-toml");
        let name = project_path.to_str().unwrap();

        create_project(name, "3D").unwrap();

        let content = fs::read_to_string(project_path.join("world.toml")).unwrap();
        let wt: WorldToml = toml::from_str(&content).unwrap();
        assert_eq!(wt.world.name, "test-3d-toml");
        assert_eq!(wt.world.dimension, "3D");
        assert_eq!(wt.world.version, "0.1.0");
        assert!(wt.camera.is_none());
        let physics = wt.physics.unwrap();
        assert_eq!(physics.gravity, vec![0.0, -9.81, 0.0]);
        let players = wt.players.unwrap();
        assert_eq!(players.max_players, 32);
    }

    #[test]
    fn test_2d_scaffold_creates_valid_world_toml_with_dimension_2d() {
        let tmp = TempDir::new().unwrap();
        let project_path = tmp.path().join("test-2d-toml");
        let name = project_path.to_str().unwrap();

        create_project(name, "2D").unwrap();

        let content = fs::read_to_string(project_path.join("world.toml")).unwrap();
        let wt: WorldToml = toml::from_str(&content).unwrap();
        assert_eq!(wt.world.name, "test-2d-toml");
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
    fn test_3d_scaffold_creates_default_3d_scene() {
        let tmp = TempDir::new().unwrap();
        let project_path = tmp.path().join("test-3d-scene");
        let name = project_path.to_str().unwrap();

        create_project(name, "3D").unwrap();

        let scene = fs::read_to_string(project_path.join("scenes/main.scene.toml")).unwrap();
        assert!(scene.contains("Main Scene"));
        assert!(scene.contains("SpawnPoint"));
        // 3D has 3-component position and quaternion rotation
        assert!(scene.contains("position = [0.0, 1.0, 0.0]"));
        assert!(scene.contains("rotation = [0.0, 0.0, 0.0, 1.0]"));
        assert!(scene.contains("scale = [1.0, 1.0, 1.0]"));
    }

    #[test]
    fn test_2d_scaffold_creates_default_2d_scene() {
        let tmp = TempDir::new().unwrap();
        let project_path = tmp.path().join("test-2d-scene");
        let name = project_path.to_str().unwrap();

        create_project(name, "2D").unwrap();

        let scene = fs::read_to_string(project_path.join("scenes/main.scene.toml")).unwrap();
        assert!(scene.contains("Main Scene"));
        assert!(scene.contains("SpawnPoint"));
        // 2D has 2-component position and angle
        assert!(scene.contains("position = [0.0, 0.0]"));
        assert!(scene.contains("angle = 0.0"));
        assert!(scene.contains("scale = [1.0, 1.0]"));
    }

    #[test]
    fn test_scaffold_creates_aether_versions_toml() {
        let tmp = TempDir::new().unwrap();
        let project_path = tmp.path().join("test-versions");
        let name = project_path.to_str().unwrap();

        create_project(name, "3D").unwrap();

        let versions_path = project_path.join(".aether/versions.toml");
        assert!(versions_path.exists());
        let content = fs::read_to_string(versions_path).unwrap();
        assert!(content.contains("Aether world version history"));
    }

    #[test]
    fn test_scaffold_fails_if_directory_already_exists() {
        let tmp = TempDir::new().unwrap();
        let project_path = tmp.path().join("existing");
        fs::create_dir(&project_path).unwrap();
        let name = project_path.to_str().unwrap();

        let result = create_project(name, "3D");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already exists"));
    }

    #[test]
    fn test_scaffold_no_lua_files() {
        let tmp = TempDir::new().unwrap();
        let project_path = tmp.path().join("test-no-lua");
        let name = project_path.to_str().unwrap();

        create_project(name, "3D").unwrap();

        // Verify no .lua files exist anywhere
        assert!(!project_path.join("scripts/main.lua").exists());
    }

    #[test]
    fn test_scaffold_invalid_dimension() {
        let tmp = TempDir::new().unwrap();
        let project_path = tmp.path().join("test-bad-dim");
        let name = project_path.to_str().unwrap();

        let result = create_project(name, "4D");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid dimension"));
    }

    #[test]
    fn test_3d_scaffold_no_tilemaps_dir() {
        let tmp = TempDir::new().unwrap();
        let project_path = tmp.path().join("test-3d-no-tilemaps");
        let name = project_path.to_str().unwrap();

        create_project(name, "3D").unwrap();

        assert!(!project_path.join("tilemaps").exists());
    }

    #[test]
    fn test_2d_scaffold_no_terrain_dir() {
        let tmp = TempDir::new().unwrap();
        let project_path = tmp.path().join("test-2d-no-terrain");
        let name = project_path.to_str().unwrap();

        create_project(name, "2D").unwrap();

        assert!(!project_path.join("terrain").exists());
    }
}
