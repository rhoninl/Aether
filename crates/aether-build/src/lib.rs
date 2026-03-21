pub mod config;
pub mod desktop;
pub mod quest;
pub mod toolchain;

use std::fmt;
use std::path::PathBuf;

use config::{BuildConfig, BuildTarget};

/// Result of a successful build.
#[derive(Debug)]
pub struct BuildOutput {
    pub artifact_path: PathBuf,
    pub target_description: String,
}

/// Errors that can occur during building.
#[derive(Debug)]
pub enum BuildError {
    ToolchainNotFound {
        tool: String,
        hint: String,
    },
    RustTargetNotInstalled {
        target: String,
    },
    CompilationFailed {
        stderr: String,
    },
    ManifestGenerationFailed {
        reason: String,
    },
    ApkPackagingFailed {
        step: String,
        stderr: String,
    },
    ProjectNotFound {
        path: String,
    },
    InvalidTarget {
        input: String,
    },
    IoError {
        context: String,
        source: std::io::Error,
    },
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuildError::ToolchainNotFound { tool, hint } => {
                write!(f, "{tool} not found. {hint}")
            }
            BuildError::RustTargetNotInstalled { target } => {
                write!(
                    f,
                    "Rust target '{target}' not installed. Run: rustup target add {target}"
                )
            }
            BuildError::CompilationFailed { stderr } => {
                write!(f, "compilation failed: {stderr}")
            }
            BuildError::ManifestGenerationFailed { reason } => {
                write!(f, "manifest generation failed: {reason}")
            }
            BuildError::ApkPackagingFailed { step, stderr } => {
                write!(f, "APK packaging failed at '{step}': {stderr}")
            }
            BuildError::ProjectNotFound { path } => {
                write!(f, "project not found at '{path}'")
            }
            BuildError::InvalidTarget { input } => {
                write!(
                    f,
                    "invalid build target '{input}'. Valid targets: desktop, quest"
                )
            }
            BuildError::IoError { context, source } => {
                write!(f, "I/O error ({context}): {source}")
            }
        }
    }
}

impl std::error::Error for BuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            BuildError::IoError { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Build the project for the specified target.
pub fn build(config: &BuildConfig) -> Result<BuildOutput, BuildError> {
    config.validate()?;

    match config.target {
        BuildTarget::Desktop => desktop::build(config),
        BuildTarget::Quest => quest::build(config),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn build_error_display_toolchain_not_found() {
        let err = BuildError::ToolchainNotFound {
            tool: "Android NDK".to_string(),
            hint: "Set ANDROID_NDK_HOME".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("Android NDK"));
        assert!(msg.contains("ANDROID_NDK_HOME"));
    }

    #[test]
    fn build_error_display_rust_target() {
        let err = BuildError::RustTargetNotInstalled {
            target: "aarch64-linux-android".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("rustup target add"));
    }

    #[test]
    fn build_error_display_invalid_target() {
        let err = BuildError::InvalidTarget {
            input: "nintendo".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("nintendo"));
        assert!(msg.contains("desktop"));
        assert!(msg.contains("quest"));
    }

    #[test]
    fn build_error_display_compilation() {
        let err = BuildError::CompilationFailed {
            stderr: "linker not found".to_string(),
        };
        assert!(err.to_string().contains("linker not found"));
    }

    #[test]
    fn build_error_display_manifest() {
        let err = BuildError::ManifestGenerationFailed {
            reason: "bad template".to_string(),
        };
        assert!(err.to_string().contains("bad template"));
    }

    #[test]
    fn build_error_display_apk() {
        let err = BuildError::ApkPackagingFailed {
            step: "zipalign".to_string(),
            stderr: "invalid APK".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("zipalign"));
        assert!(msg.contains("invalid APK"));
    }

    #[test]
    fn build_error_display_project_not_found() {
        let err = BuildError::ProjectNotFound {
            path: "/bad/path".to_string(),
        };
        assert!(err.to_string().contains("/bad/path"));
    }

    #[test]
    fn build_validates_project_dir() {
        let config = config::BuildConfig::new(
            PathBuf::from("/nonexistent"),
            config::BuildTarget::Desktop,
            config::BuildProfile::Debug,
        );
        let err = build(&config).unwrap_err();
        match err {
            BuildError::ProjectNotFound { .. } => {}
            _ => panic!("expected ProjectNotFound, got: {err}"),
        }
    }
}
