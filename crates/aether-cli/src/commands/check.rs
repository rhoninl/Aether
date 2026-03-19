use std::path::Path;

use crate::manifest;

pub fn check_project(path: &str) -> Result<(), String> {
    let dir = Path::new(path);
    if !dir.is_dir() {
        return Err(format!("'{}' is not a directory", path));
    }

    println!("Checking {}...", dir.display());

    let world_toml = manifest::load_manifest(dir)?;
    let errors = manifest::validate_manifest(dir, &world_toml);

    let m = &world_toml.world;
    if errors.is_empty() {
        println!("  {} v{} ({})", m.name, m.version, m.dimension);
        if let Some(scenes) = &world_toml.scenes {
            println!("  {} scene(s) referenced", scenes.list.len());
        }
        println!("  All checks passed.");
        Ok(())
    } else {
        for err in &errors {
            eprintln!("  error: {err}");
        }
        Err(format!("{} error(s) found", errors.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Helper to create a valid 3D project directory for testing.
    fn create_valid_3d_project(dir: &Path) {
        let manifest = r#"
[world]
name = "test"
version = "0.1.0"
dimension = "3D"
description = "A test world"

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
        fs::write(dir.join("world.toml"), manifest).unwrap();
        fs::create_dir_all(dir.join("scenes")).unwrap();
        fs::create_dir_all(dir.join("assets")).unwrap();
        fs::create_dir_all(dir.join("terrain")).unwrap();
        fs::create_dir_all(dir.join(".aether")).unwrap();
        fs::write(
            dir.join("scenes/main.scene.toml"),
            "[scene]\nname = \"Main\"",
        )
        .unwrap();
        fs::write(
            dir.join(".aether/versions.toml"),
            "# Aether world version history",
        )
        .unwrap();
    }

    /// Helper to create a valid 2D project directory for testing.
    fn create_valid_2d_project(dir: &Path) {
        let manifest = r#"
[world]
name = "test-2d"
version = "0.1.0"
dimension = "2D"
description = "A 2D test world"

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
"#;
        fs::write(dir.join("world.toml"), manifest).unwrap();
        fs::create_dir_all(dir.join("scenes")).unwrap();
        fs::create_dir_all(dir.join("assets")).unwrap();
        fs::create_dir_all(dir.join("tilemaps")).unwrap();
        fs::create_dir_all(dir.join(".aether")).unwrap();
        fs::write(
            dir.join("scenes/main.scene.toml"),
            "[scene]\nname = \"Main\"",
        )
        .unwrap();
        fs::write(
            dir.join(".aether/versions.toml"),
            "# Aether world version history",
        )
        .unwrap();
    }

    #[test]
    fn test_check_valid_3d_project() {
        let tmp = TempDir::new().unwrap();
        create_valid_3d_project(tmp.path());
        let result = check_project(tmp.path().to_str().unwrap());
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_valid_2d_project() {
        let tmp = TempDir::new().unwrap();
        create_valid_2d_project(tmp.path());
        let result = check_project(tmp.path().to_str().unwrap());
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_missing_manifest() {
        let tmp = TempDir::new().unwrap();
        let result = check_project(tmp.path().to_str().unwrap());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_check_missing_scenes_directory() {
        let tmp = TempDir::new().unwrap();
        let manifest = r#"
[world]
name = "test"
version = "0.1.0"
dimension = "3D"

[scenes]
default = "main"
list = ["main"]
"#;
        fs::write(tmp.path().join("world.toml"), manifest).unwrap();
        fs::create_dir_all(tmp.path().join("assets")).unwrap();
        fs::create_dir_all(tmp.path().join("terrain")).unwrap();
        fs::create_dir_all(tmp.path().join(".aether")).unwrap();
        fs::write(
            tmp.path().join(".aether/versions.toml"),
            "# versions",
        )
        .unwrap();

        let result = check_project(tmp.path().to_str().unwrap());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("error(s) found"));
    }

    #[test]
    fn test_check_missing_required_scene_file() {
        let tmp = TempDir::new().unwrap();
        let manifest = r#"
[world]
name = "test"
version = "0.1.0"
dimension = "3D"

[scenes]
default = "main"
list = ["main", "dungeon"]
"#;
        fs::write(tmp.path().join("world.toml"), manifest).unwrap();
        fs::create_dir_all(tmp.path().join("scenes")).unwrap();
        fs::create_dir_all(tmp.path().join("assets")).unwrap();
        fs::create_dir_all(tmp.path().join("terrain")).unwrap();
        fs::create_dir_all(tmp.path().join(".aether")).unwrap();
        fs::write(
            tmp.path().join("scenes/main.scene.toml"),
            "[scene]\nname = \"Main\"",
        )
        .unwrap();
        fs::write(
            tmp.path().join(".aether/versions.toml"),
            "# versions",
        )
        .unwrap();
        // Note: dungeon.scene.toml does NOT exist

        let result = check_project(tmp.path().to_str().unwrap());
        assert!(result.is_err());
    }

    #[test]
    fn test_check_invalid_dimension() {
        let tmp = TempDir::new().unwrap();
        let manifest = r#"
[world]
name = "test"
version = "0.1.0"
dimension = "5D"
"#;
        fs::write(tmp.path().join("world.toml"), manifest).unwrap();
        let result = check_project(tmp.path().to_str().unwrap());
        assert!(result.is_err());
    }

    #[test]
    fn test_check_missing_aether_directory() {
        let tmp = TempDir::new().unwrap();
        let manifest = r#"
[world]
name = "test"
version = "0.1.0"
dimension = "3D"

[scenes]
default = "main"
list = ["main"]
"#;
        fs::write(tmp.path().join("world.toml"), manifest).unwrap();
        fs::create_dir_all(tmp.path().join("scenes")).unwrap();
        fs::create_dir_all(tmp.path().join("assets")).unwrap();
        fs::create_dir_all(tmp.path().join("terrain")).unwrap();
        fs::write(
            tmp.path().join("scenes/main.scene.toml"),
            "[scene]\nname = \"Main\"",
        )
        .unwrap();
        // Note: .aether/ does NOT exist

        let result = check_project(tmp.path().to_str().unwrap());
        assert!(result.is_err());
    }

    #[test]
    fn test_check_not_a_directory() {
        let result = check_project("/nonexistent/path");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not a directory"));
    }
}
