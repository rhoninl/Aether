use std::path::Path;

use crate::manifest;

pub fn check_project(path: &str) -> Result<(), String> {
    let dir = Path::new(path);
    if !dir.is_dir() {
        return Err(format!("'{}' is not a directory", path));
    }

    println!("Checking {}...", dir.display());

    let m = manifest::load_manifest(dir)?;
    let errors = manifest::validate_manifest(dir, &m);

    if errors.is_empty() {
        println!("  {} v{}", m.name, m.version);
        println!("  {} script(s) referenced", m.scripts.len());
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

    #[test]
    fn test_check_valid_project() {
        let tmp = TempDir::new().unwrap();
        let manifest = r#"
name = "test"
version = "0.1.0"
scripts = ["scripts/main.lua"]
"#;
        fs::write(tmp.path().join("world.toml"), manifest).unwrap();
        fs::create_dir_all(tmp.path().join("scripts")).unwrap();
        fs::write(tmp.path().join("scripts/main.lua"), "-- ok").unwrap();

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
    fn test_check_missing_script() {
        let tmp = TempDir::new().unwrap();
        let manifest = r#"
name = "test"
version = "0.1.0"
scripts = ["scripts/missing.lua"]
"#;
        fs::write(tmp.path().join("world.toml"), manifest).unwrap();
        let result = check_project(tmp.path().to_str().unwrap());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("error(s) found"));
    }

    #[test]
    fn test_check_not_a_directory() {
        let result = check_project("/nonexistent/path");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not a directory"));
    }
}
