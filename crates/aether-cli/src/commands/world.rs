use std::fs;
use std::path::Path;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::manifest;

const VERSIONS_FILE: &str = ".aether/versions.toml";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VersionsFile {
    #[serde(default)]
    versions: Vec<VersionEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VersionEntry {
    version: String,
    published_at: String,
    #[serde(default)]
    changelog: String,
}

/// Which component to bump when publishing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BumpLevel {
    Major,
    Minor,
    Patch,
}

/// Bump a semantic version string according to the given level.
fn bump_version(version: &str, level: BumpLevel) -> Result<String, String> {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() != 3 {
        return Err(format!(
            "version '{}' is not valid semver (expected MAJOR.MINOR.PATCH)",
            version
        ));
    }
    let major: u64 = parts[0]
        .parse()
        .map_err(|_| format!("invalid major version: '{}'", parts[0]))?;
    let minor: u64 = parts[1]
        .parse()
        .map_err(|_| format!("invalid minor version: '{}'", parts[1]))?;
    let patch: u64 = parts[2]
        .parse()
        .map_err(|_| format!("invalid patch version: '{}'", parts[2]))?;

    let (new_major, new_minor, new_patch) = match level {
        BumpLevel::Major => (major + 1, 0, 0),
        BumpLevel::Minor => (major, minor + 1, 0),
        BumpLevel::Patch => (major, minor, patch + 1),
    };

    Ok(format!("{new_major}.{new_minor}.{new_patch}"))
}

/// Load the versions file, returning an empty list if the file only has comments.
fn load_versions(dir: &Path) -> Result<Vec<VersionEntry>, String> {
    let path = dir.join(VERSIONS_FILE);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;

    // If the file is just comments or empty, return empty list
    let trimmed = content
        .lines()
        .filter(|line| !line.trim_start().starts_with('#') && !line.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let versions_file: VersionsFile =
        toml::from_str(&content).map_err(|e| format!("invalid {}: {e}", VERSIONS_FILE))?;
    Ok(versions_file.versions)
}

/// Save the versions list back to the versions file.
fn save_versions(dir: &Path, entries: &[VersionEntry]) -> Result<(), String> {
    let versions_file = VersionsFile {
        versions: entries.to_vec(),
    };
    let content = if entries.is_empty() {
        "# Aether world version history\n".to_string()
    } else {
        let mut s = "# Aether world version history\n\n".to_string();
        let serialized = toml::to_string_pretty(&versions_file)
            .map_err(|e| format!("failed to serialize versions: {e}"))?;
        s.push_str(&serialized);
        s
    };
    let path = dir.join(VERSIONS_FILE);
    fs::create_dir_all(path.parent().unwrap())
        .map_err(|e| format!("failed to create .aether directory: {e}"))?;
    fs::write(&path, content).map_err(|e| format!("failed to write {}: {e}", path.display()))?;
    Ok(())
}

/// Update the version field in world.toml.
fn update_world_toml_version(dir: &Path, new_version: &str) -> Result<(), String> {
    let path = dir.join("world.toml");
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("failed to read world.toml: {e}"))?;

    // We need to update the version field in the [world] section.
    // Use simple string replacement to preserve formatting.
    let mut updated = String::new();
    let mut in_world_section = false;
    let mut version_replaced = false;

    for line in content.lines() {
        if line.trim() == "[world]" {
            in_world_section = true;
            updated.push_str(line);
            updated.push('\n');
            continue;
        }
        if line.starts_with('[') && line.trim() != "[world]" {
            in_world_section = false;
        }
        if in_world_section && line.starts_with("version") && !version_replaced {
            updated.push_str(&format!("version = \"{new_version}\""));
            updated.push('\n');
            version_replaced = true;
            continue;
        }
        updated.push_str(line);
        updated.push('\n');
    }

    if !version_replaced {
        return Err("could not find version field in [world] section".to_string());
    }

    fs::write(&path, updated).map_err(|e| format!("failed to write world.toml: {e}"))?;
    Ok(())
}

/// Publish a new version of the world.
pub fn publish(path: &str, level: BumpLevel, changelog: &str) -> Result<(), String> {
    let dir = Path::new(path);
    if !dir.is_dir() {
        return Err(format!("'{}' is not a directory", path));
    }

    let world_toml = manifest::load_manifest(dir)?;
    let current_version = &world_toml.world.version;
    let new_version = bump_version(current_version, level)?;

    // Update world.toml
    update_world_toml_version(dir, &new_version)?;

    // Append to versions.toml
    let mut entries = load_versions(dir)?;
    let entry = VersionEntry {
        version: new_version.clone(),
        published_at: Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        changelog: changelog.to_string(),
    };
    // Insert at the beginning so newest is first
    entries.insert(0, entry);
    save_versions(dir, &entries)?;

    println!(
        "Published '{}' v{} -> v{}",
        world_toml.world.name, current_version, new_version
    );
    if !changelog.is_empty() {
        println!("  Changelog: {changelog}");
    }

    Ok(())
}

/// List version history.
pub fn versions(path: &str) -> Result<(), String> {
    let dir = Path::new(path);
    if !dir.is_dir() {
        return Err(format!("'{}' is not a directory", path));
    }

    let entries = load_versions(dir)?;

    if entries.is_empty() {
        println!("No published versions.");
        return Ok(());
    }

    println!("Version history:");
    for entry in &entries {
        let changelog_suffix = if entry.changelog.is_empty() {
            String::new()
        } else {
            format!(" - {}", entry.changelog)
        };
        println!(
            "  v{}  ({}){changelog_suffix}",
            entry.version, entry.published_at
        );
    }

    Ok(())
}

/// Read versions and return them as formatted strings (for programmatic use).
pub fn get_version_entries(path: &str) -> Result<Vec<String>, String> {
    let dir = Path::new(path);
    if !dir.is_dir() {
        return Err(format!("'{}' is not a directory", path));
    }

    let entries = load_versions(dir)?;
    Ok(entries.iter().map(|e| e.version.clone()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Helper to create a minimal world project for publish/version tests.
    fn create_minimal_project(dir: &Path, version: &str) {
        let manifest = format!(
            r#"[world]
name = "test"
version = "{version}"
dimension = "3D"
description = "test"

[scenes]
default = "main"
list = ["main"]
"#
        );
        fs::write(dir.join("world.toml"), manifest).unwrap();
        fs::create_dir_all(dir.join(".aether")).unwrap();
        fs::write(
            dir.join(".aether/versions.toml"),
            "# Aether world version history\n",
        )
        .unwrap();
    }

    #[test]
    fn test_bump_version_patch() {
        assert_eq!(bump_version("0.1.0", BumpLevel::Patch).unwrap(), "0.1.1");
        assert_eq!(bump_version("1.2.3", BumpLevel::Patch).unwrap(), "1.2.4");
    }

    #[test]
    fn test_bump_version_minor() {
        assert_eq!(bump_version("0.1.0", BumpLevel::Minor).unwrap(), "0.2.0");
        assert_eq!(bump_version("1.2.3", BumpLevel::Minor).unwrap(), "1.3.0");
    }

    #[test]
    fn test_bump_version_major() {
        assert_eq!(bump_version("0.1.0", BumpLevel::Major).unwrap(), "1.0.0");
        assert_eq!(bump_version("1.2.3", BumpLevel::Major).unwrap(), "2.0.0");
    }

    #[test]
    fn test_bump_version_invalid() {
        assert!(bump_version("invalid", BumpLevel::Patch).is_err());
        assert!(bump_version("1.2", BumpLevel::Patch).is_err());
        assert!(bump_version("1.2.3.4", BumpLevel::Patch).is_err());
    }

    #[test]
    fn test_publish_bumps_patch_version_correctly() {
        let tmp = TempDir::new().unwrap();
        create_minimal_project(tmp.path(), "0.1.0");

        publish(tmp.path().to_str().unwrap(), BumpLevel::Patch, "bug fix").unwrap();

        let wt = manifest::load_manifest(tmp.path()).unwrap();
        assert_eq!(wt.world.version, "0.1.1");
    }

    #[test]
    fn test_publish_bumps_minor_version_correctly() {
        let tmp = TempDir::new().unwrap();
        create_minimal_project(tmp.path(), "0.1.0");

        publish(tmp.path().to_str().unwrap(), BumpLevel::Minor, "new feature").unwrap();

        let wt = manifest::load_manifest(tmp.path()).unwrap();
        assert_eq!(wt.world.version, "0.2.0");
    }

    #[test]
    fn test_publish_bumps_major_version_correctly() {
        let tmp = TempDir::new().unwrap();
        create_minimal_project(tmp.path(), "1.2.3");

        publish(tmp.path().to_str().unwrap(), BumpLevel::Major, "breaking change").unwrap();

        let wt = manifest::load_manifest(tmp.path()).unwrap();
        assert_eq!(wt.world.version, "2.0.0");
    }

    #[test]
    fn test_publish_updates_world_toml_version() {
        let tmp = TempDir::new().unwrap();
        create_minimal_project(tmp.path(), "0.1.0");

        publish(tmp.path().to_str().unwrap(), BumpLevel::Patch, "").unwrap();

        let content = fs::read_to_string(tmp.path().join("world.toml")).unwrap();
        assert!(content.contains("version = \"0.1.1\""));
        assert!(!content.contains("version = \"0.1.0\""));
    }

    #[test]
    fn test_publish_appends_to_versions_toml() {
        let tmp = TempDir::new().unwrap();
        create_minimal_project(tmp.path(), "0.1.0");

        publish(tmp.path().to_str().unwrap(), BumpLevel::Patch, "first release").unwrap();

        let content =
            fs::read_to_string(tmp.path().join(".aether/versions.toml")).unwrap();
        assert!(content.contains("version = \"0.1.1\""));
        assert!(content.contains("first release"));
        assert!(content.contains("published_at"));
    }

    #[test]
    fn test_publish_multiple_appends_in_order() {
        let tmp = TempDir::new().unwrap();
        create_minimal_project(tmp.path(), "0.1.0");

        publish(tmp.path().to_str().unwrap(), BumpLevel::Patch, "fix 1").unwrap();
        publish(tmp.path().to_str().unwrap(), BumpLevel::Patch, "fix 2").unwrap();

        let versions = get_version_entries(tmp.path().to_str().unwrap()).unwrap();
        assert_eq!(versions.len(), 2);
        // Newest first
        assert_eq!(versions[0], "0.1.2");
        assert_eq!(versions[1], "0.1.1");
    }

    #[test]
    fn test_versions_lists_in_reverse_chronological_order() {
        let tmp = TempDir::new().unwrap();
        create_minimal_project(tmp.path(), "0.1.0");

        publish(tmp.path().to_str().unwrap(), BumpLevel::Patch, "first").unwrap();
        publish(tmp.path().to_str().unwrap(), BumpLevel::Minor, "second").unwrap();
        publish(tmp.path().to_str().unwrap(), BumpLevel::Major, "third").unwrap();

        let version_list = get_version_entries(tmp.path().to_str().unwrap()).unwrap();
        assert_eq!(version_list.len(), 3);
        assert_eq!(version_list[0], "1.0.0"); // most recent
        assert_eq!(version_list[1], "0.2.0");
        assert_eq!(version_list[2], "0.1.1"); // oldest
    }

    #[test]
    fn test_versions_on_empty_history_shows_nothing() {
        let tmp = TempDir::new().unwrap();
        create_minimal_project(tmp.path(), "0.1.0");

        let version_list = get_version_entries(tmp.path().to_str().unwrap()).unwrap();
        assert!(version_list.is_empty());
    }

    #[test]
    fn test_versions_on_nonexistent_path() {
        let result = versions("/nonexistent/path");
        assert!(result.is_err());
    }

    #[test]
    fn test_publish_on_nonexistent_path() {
        let result = publish("/nonexistent/path", BumpLevel::Patch, "");
        assert!(result.is_err());
    }

    #[test]
    fn test_publish_preserves_other_world_toml_fields() {
        let tmp = TempDir::new().unwrap();
        create_minimal_project(tmp.path(), "0.1.0");

        publish(tmp.path().to_str().unwrap(), BumpLevel::Patch, "").unwrap();

        let wt = manifest::load_manifest(tmp.path()).unwrap();
        assert_eq!(wt.world.name, "test");
        assert_eq!(wt.world.dimension, "3D");
        assert_eq!(wt.world.description, "test");
    }

    #[test]
    fn test_load_versions_empty_file() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join(".aether")).unwrap();
        fs::write(
            tmp.path().join(".aether/versions.toml"),
            "# Just a comment\n",
        )
        .unwrap();

        let entries = load_versions(tmp.path()).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_load_versions_missing_file() {
        let tmp = TempDir::new().unwrap();
        let entries = load_versions(tmp.path()).unwrap();
        assert!(entries.is_empty());
    }
}
