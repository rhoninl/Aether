use std::process::Command;

use crate::config::{BuildConfig, BuildProfile};
use crate::{BuildError, BuildOutput};

/// Build for desktop using cargo.
pub fn build(config: &BuildConfig) -> Result<BuildOutput, BuildError> {
    println!("Building for desktop...");

    let mut cmd = Command::new("cargo");
    cmd.arg("build");

    if config.profile == BuildProfile::Release {
        cmd.arg("--release");
    }

    cmd.current_dir(&config.project_dir);

    let status = cmd.status().map_err(|e| BuildError::CompilationFailed {
        stderr: format!("failed to run cargo: {e}"),
    })?;

    if !status.success() {
        return Err(BuildError::CompilationFailed {
            stderr: format!("cargo build exited with status {}", status),
        });
    }

    let profile_dir = match config.profile {
        BuildProfile::Debug => "debug",
        BuildProfile::Release => "release",
    };

    let artifact_path = config.project_dir.join("target").join(profile_dir);

    println!("Desktop build complete: {}", artifact_path.display());

    Ok(BuildOutput {
        artifact_path,
        target_description: format!("desktop ({})", config.profile),
    })
}

/// Build the cargo command arguments (for testing).
pub fn build_cargo_args(config: &BuildConfig) -> Vec<String> {
    let mut args = vec!["build".to_string()];
    if config.profile == BuildProfile::Release {
        args.push("--release".to_string());
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use crate::config::BuildTarget;

    #[test]
    fn build_cargo_args_debug() {
        let config = BuildConfig::new(
            PathBuf::from("/project"),
            BuildTarget::Desktop,
            BuildProfile::Debug,
        );
        let args = build_cargo_args(&config);
        assert_eq!(args, vec!["build"]);
    }

    #[test]
    fn build_cargo_args_release() {
        let config = BuildConfig::new(
            PathBuf::from("/project"),
            BuildTarget::Desktop,
            BuildProfile::Release,
        );
        let args = build_cargo_args(&config);
        assert_eq!(args, vec!["build", "--release"]);
    }
}
