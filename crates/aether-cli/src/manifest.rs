use serde::{Deserialize, Serialize};

const MANIFEST_FILE: &str = "world.toml";

#[derive(Debug, Serialize, Deserialize)]
pub struct WorldManifest {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub scripts: Vec<String>,
}

impl WorldManifest {
    pub fn default_for(name: &str) -> Self {
        Self {
            name: name.to_string(),
            version: "0.1.0".to_string(),
            description: format!("An Aether world: {name}"),
            scripts: vec!["scripts/main.lua".to_string()],
        }
    }
}

/// Load and parse a world.toml from the given directory.
pub fn load_manifest(dir: &std::path::Path) -> Result<WorldManifest, String> {
    let path = dir.join(MANIFEST_FILE);
    if !path.exists() {
        return Err(format!("'{}' not found in {}", MANIFEST_FILE, dir.display()));
    }
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    let manifest: WorldManifest = toml::from_str(&content)
        .map_err(|e| format!("invalid {}: {e}", MANIFEST_FILE))?;
    Ok(manifest)
}

/// Validate a loaded manifest and check that referenced files exist.
pub fn validate_manifest(dir: &std::path::Path, manifest: &WorldManifest) -> Vec<String> {
    let mut errors = Vec::new();
    if manifest.name.is_empty() {
        errors.push("'name' is empty".to_string());
    }
    if manifest.version.is_empty() {
        errors.push("'version' is empty".to_string());
    }
    for script in &manifest.scripts {
        let script_path = dir.join(script);
        if !script_path.exists() {
            errors.push(format!("script '{}' not found", script));
        }
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
    fn test_default_manifest() {
        let m = WorldManifest::default_for("test-world");
        assert_eq!(m.name, "test-world");
        assert_eq!(m.version, "0.1.0");
        assert!(!m.scripts.is_empty());
    }

    #[test]
    fn test_manifest_serialization_roundtrip() {
        let m = WorldManifest::default_for("roundtrip");
        let toml_str = toml::to_string_pretty(&m).unwrap();
        let parsed: WorldManifest = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.name, "roundtrip");
        assert_eq!(parsed.version, "0.1.0");
    }

    #[test]
    fn test_load_manifest_missing() {
        let dir = TempDir::new().unwrap();
        let result = load_manifest(dir.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_load_manifest_valid() {
        let dir = TempDir::new().unwrap();
        let content = r#"
name = "my-world"
version = "0.1.0"
scripts = ["scripts/main.lua"]
"#;
        fs::write(dir.path().join("world.toml"), content).unwrap();
        let m = load_manifest(dir.path()).unwrap();
        assert_eq!(m.name, "my-world");
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
    fn test_validate_manifest_ok() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("scripts")).unwrap();
        fs::write(dir.path().join("scripts/main.lua"), "-- hello").unwrap();
        let m = WorldManifest::default_for("valid");
        let errors = validate_manifest(dir.path(), &m);
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn test_validate_manifest_empty_name() {
        let dir = TempDir::new().unwrap();
        let m = WorldManifest {
            name: String::new(),
            version: "0.1.0".to_string(),
            description: String::new(),
            scripts: vec![],
        };
        let errors = validate_manifest(dir.path(), &m);
        assert!(errors.iter().any(|e| e.contains("name")));
    }

    #[test]
    fn test_validate_manifest_missing_script() {
        let dir = TempDir::new().unwrap();
        let m = WorldManifest::default_for("test");
        let errors = validate_manifest(dir.path(), &m);
        assert!(errors.iter().any(|e| e.contains("scripts/main.lua")));
    }
}
