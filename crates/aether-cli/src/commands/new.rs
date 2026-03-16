use std::fs;
use std::path::Path;

use crate::manifest::WorldManifest;

const STARTER_LUA: &str = r#"-- Main script for the world
-- This runs when the world starts

function on_start()
    print("World started!")
end

function on_tick(dt)
    -- Called every frame
    -- dt is the time delta in seconds
end

function on_player_join(player)
    print("Player joined: " .. player.name)
end
"#;

pub fn create_project(name: &str) -> Result<(), String> {
    let path = Path::new(name);
    if path.exists() {
        return Err(format!("directory '{}' already exists", name));
    }

    fs::create_dir_all(path.join("scripts"))
        .map_err(|e| format!("failed to create project directory: {e}"))?;
    fs::create_dir_all(path.join("assets"))
        .map_err(|e| format!("failed to create assets directory: {e}"))?;

    let manifest = WorldManifest::default_for(name);
    let toml_str = toml::to_string_pretty(&manifest)
        .map_err(|e| format!("failed to serialize manifest: {e}"))?;
    fs::write(path.join("world.toml"), toml_str)
        .map_err(|e| format!("failed to write world.toml: {e}"))?;

    fs::write(path.join("scripts/main.lua"), STARTER_LUA)
        .map_err(|e| format!("failed to write starter script: {e}"))?;

    println!("Created new Aether world '{name}'");
    println!();
    println!("  {name}/");
    println!("  ├── world.toml");
    println!("  ├── scripts/");
    println!("  │   └── main.lua");
    println!("  └── assets/");
    println!();
    println!("Get started:");
    println!("  cd {name}");
    println!("  aether serve");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_create_project() {
        let tmp = TempDir::new().unwrap();
        let project_path = tmp.path().join("test-world");
        let name = project_path.to_str().unwrap();

        create_project(name).unwrap();

        assert!(project_path.join("world.toml").exists());
        assert!(project_path.join("scripts/main.lua").exists());
        assert!(project_path.join("assets").is_dir());

        let manifest_content = fs::read_to_string(project_path.join("world.toml")).unwrap();
        assert!(manifest_content.contains("test-world"));

        let script_content = fs::read_to_string(project_path.join("scripts/main.lua")).unwrap();
        assert!(script_content.contains("on_start"));
        assert!(script_content.contains("on_tick"));
    }

    #[test]
    fn test_create_project_already_exists() {
        let tmp = TempDir::new().unwrap();
        let project_path = tmp.path().join("existing");
        fs::create_dir(&project_path).unwrap();
        let name = project_path.to_str().unwrap();

        let result = create_project(name);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already exists"));
    }
}
