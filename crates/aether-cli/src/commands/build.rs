use std::path::PathBuf;
use std::str::FromStr;

use aether_build::config::{BuildConfig, BuildProfile, BuildTarget};

pub fn build_project(path: &str, target: &str, release: bool, install: bool) -> Result<(), String> {
    let build_target = BuildTarget::from_str(target).map_err(|e| e.to_string())?;

    let profile = if release {
        BuildProfile::Release
    } else {
        BuildProfile::Debug
    };

    let project_dir = PathBuf::from(path)
        .canonicalize()
        .map_err(|e| format!("invalid project path '{}': {}", path, e))?;

    let mut config = BuildConfig::new(project_dir, build_target, profile);
    config.install_after_build = install;

    let output = aether_build::build(&config).map_err(|e| e.to_string())?;

    println!(
        "Build complete ({}) -> {}",
        output.target_description,
        output.artifact_path.display()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_invalid_target_returns_error() {
        let result = build_project(".", "nintendo", false, false);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("invalid build target"));
        assert!(err.contains("nintendo"));
    }

    #[test]
    fn build_target_quest_parses() {
        let target = BuildTarget::from_str("quest");
        assert!(target.is_ok());
        assert_eq!(target.unwrap(), BuildTarget::Quest);
    }

    #[test]
    fn build_target_desktop_is_default() {
        assert_eq!(BuildTarget::default(), BuildTarget::Desktop);
    }

    #[test]
    fn build_nonexistent_path_returns_error() {
        let result = build_project("/nonexistent/path/to/project", "desktop", false, false);
        assert!(result.is_err());
    }
}
