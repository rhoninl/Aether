use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use crate::BuildError;

pub const QUEST_RUST_TARGET: &str = "aarch64-linux-android";
pub const QUEST_ABI: &str = "arm64-v8a";
pub const QUEST_MIN_SDK_VERSION: u32 = 29;
pub const QUEST_TARGET_SDK_VERSION: u32 = 32;
pub const BUILD_OUTPUT_DIR: &str = "target/aether-build";

const ENV_ANDROID_NDK_HOME: &str = "ANDROID_NDK_HOME";
const ENV_ANDROID_HOME: &str = "ANDROID_HOME";
const ENV_ANDROID_SDK_ROOT: &str = "ANDROID_SDK_ROOT";
const ENV_KEYSTORE_PATH: &str = "AETHER_KEYSTORE_PATH";
const ENV_KEYSTORE_PASSWORD: &str = "AETHER_KEYSTORE_PASSWORD";

/// Target platform for the build.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum BuildTarget {
    #[default]
    Desktop,
    Quest,
}

impl fmt::Display for BuildTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuildTarget::Desktop => write!(f, "desktop"),
            BuildTarget::Quest => write!(f, "quest"),
        }
    }
}

impl FromStr for BuildTarget {
    type Err = BuildError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "desktop" => Ok(BuildTarget::Desktop),
            "quest" | "quest3" | "quest_standalone" | "quest-standalone" => Ok(BuildTarget::Quest),
            _ => Err(BuildError::InvalidTarget {
                input: s.to_string(),
            }),
        }
    }
}

/// Build profile (debug or release).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum BuildProfile {
    #[default]
    Debug,
    Release,
}

impl fmt::Display for BuildProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuildProfile::Debug => write!(f, "debug"),
            BuildProfile::Release => write!(f, "release"),
        }
    }
}

const DEFAULT_QUEST_PACKAGE: &str = "quest-debug";

/// Configuration for a build invocation.
#[derive(Debug, Clone)]
pub struct BuildConfig {
    pub target: BuildTarget,
    pub profile: BuildProfile,
    pub project_dir: PathBuf,
    pub app_name: String,
    pub package: Option<String>,
    pub install_after_build: bool,
}

impl BuildConfig {
    pub fn new(project_dir: PathBuf, target: BuildTarget, profile: BuildProfile) -> Self {
        let package = match target {
            BuildTarget::Quest => Some(DEFAULT_QUEST_PACKAGE.to_string()),
            BuildTarget::Desktop => None,
        };
        let app_name = package
            .as_deref()
            .unwrap_or_else(|| {
                project_dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("aether-app")
            })
            .to_string();
        Self {
            target,
            profile,
            project_dir,
            app_name,
            package,
            install_after_build: false,
        }
    }

    pub fn validate(&self) -> Result<(), BuildError> {
        if !self.project_dir.exists() {
            return Err(BuildError::ProjectNotFound {
                path: self.project_dir.display().to_string(),
            });
        }
        Ok(())
    }

    /// Build output directory for this target.
    pub fn output_dir(&self) -> PathBuf {
        self.project_dir
            .join(BUILD_OUTPUT_DIR)
            .join(self.target.to_string())
    }

    /// Intermediate build directory.
    pub fn build_dir(&self) -> PathBuf {
        self.output_dir().join("build")
    }

    /// Final APK path (Quest only).
    pub fn apk_path(&self) -> PathBuf {
        let suffix = match self.profile {
            BuildProfile::Debug => "debug",
            BuildProfile::Release => "release",
        };
        self.output_dir()
            .join(format!("{}-{}.apk", self.app_name, suffix))
    }

    /// Android package name derived from app name.
    pub fn package_name(&self) -> String {
        let sanitized = self
            .app_name
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect::<String>()
            .to_lowercase();
        format!("com.aether.{sanitized}")
    }
}

/// Read the Android NDK home path from environment.
pub fn android_ndk_home() -> Result<PathBuf, BuildError> {
    std::env::var(ENV_ANDROID_NDK_HOME)
        .map(PathBuf::from)
        .map_err(|_| BuildError::ToolchainNotFound {
            tool: "Android NDK".to_string(),
            hint: format!(
                "Set {ENV_ANDROID_NDK_HOME} to your NDK path (e.g., ~/Android/sdk/ndk/27.0.12077973)"
            ),
        })
}

/// Read the Android SDK home path from environment.
pub fn android_sdk_home() -> Result<PathBuf, BuildError> {
    std::env::var(ENV_ANDROID_HOME)
        .or_else(|_| std::env::var(ENV_ANDROID_SDK_ROOT))
        .map(PathBuf::from)
        .map_err(|_| BuildError::ToolchainNotFound {
            tool: "Android SDK".to_string(),
            hint: format!(
                "Set {ENV_ANDROID_HOME} or {ENV_ANDROID_SDK_ROOT} to your SDK path (e.g., ~/Android/sdk)"
            ),
        })
}

/// Read optional keystore configuration for release signing.
pub fn keystore_config() -> Option<(PathBuf, String)> {
    let path = std::env::var(ENV_KEYSTORE_PATH).ok()?;
    let password = std::env::var(ENV_KEYSTORE_PASSWORD).ok()?;
    Some((PathBuf::from(path), password))
}

/// Detect the host NDK prebuilt directory name.
pub fn ndk_host_tag() -> &'static str {
    if cfg!(target_os = "macos") {
        "darwin-x86_64"
    } else if cfg!(target_os = "linux") {
        "linux-x86_64"
    } else if cfg!(target_os = "windows") {
        "windows-x86_64"
    } else {
        "linux-x86_64"
    }
}

/// Find the highest version build-tools directory in the SDK.
pub fn find_latest_build_tools(sdk_path: &Path) -> Result<PathBuf, BuildError> {
    let build_tools_dir = sdk_path.join("build-tools");
    if !build_tools_dir.is_dir() {
        return Err(BuildError::ToolchainNotFound {
            tool: "Android build-tools".to_string(),
            hint: format!(
                "No build-tools directory found at {}. Install build-tools via sdkmanager.",
                build_tools_dir.display()
            ),
        });
    }

    let mut versions: Vec<_> = std::fs::read_dir(&build_tools_dir)
        .map_err(|e| BuildError::IoError {
            context: "reading build-tools directory".to_string(),
            source: e,
        })?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect();

    versions.sort();

    versions
        .last()
        .map(|v| build_tools_dir.join(v))
        .ok_or_else(|| BuildError::ToolchainNotFound {
            tool: "Android build-tools".to_string(),
            hint: "No build-tools versions found. Install via: sdkmanager \"build-tools;34.0.0\""
                .to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn build_target_from_str_quest() {
        assert_eq!(BuildTarget::from_str("quest").unwrap(), BuildTarget::Quest);
    }

    #[test]
    fn build_target_from_str_quest3() {
        assert_eq!(BuildTarget::from_str("quest3").unwrap(), BuildTarget::Quest);
    }

    #[test]
    fn build_target_from_str_quest_standalone() {
        assert_eq!(
            BuildTarget::from_str("quest_standalone").unwrap(),
            BuildTarget::Quest
        );
        assert_eq!(
            BuildTarget::from_str("quest-standalone").unwrap(),
            BuildTarget::Quest
        );
    }

    #[test]
    fn build_target_from_str_desktop() {
        assert_eq!(
            BuildTarget::from_str("desktop").unwrap(),
            BuildTarget::Desktop
        );
    }

    #[test]
    fn build_target_from_str_case_insensitive() {
        assert_eq!(BuildTarget::from_str("Quest").unwrap(), BuildTarget::Quest);
        assert_eq!(BuildTarget::from_str("QUEST").unwrap(), BuildTarget::Quest);
        assert_eq!(
            BuildTarget::from_str("DESKTOP").unwrap(),
            BuildTarget::Desktop
        );
    }

    #[test]
    fn build_target_from_str_invalid() {
        let err = BuildTarget::from_str("nintendo").unwrap_err();
        match err {
            BuildError::InvalidTarget { input } => assert_eq!(input, "nintendo"),
            _ => panic!("expected InvalidTarget error"),
        }
    }

    #[test]
    fn build_target_default_is_desktop() {
        assert_eq!(BuildTarget::default(), BuildTarget::Desktop);
    }

    #[test]
    fn build_profile_default_is_debug() {
        assert_eq!(BuildProfile::default(), BuildProfile::Debug);
    }

    #[test]
    fn build_target_display() {
        assert_eq!(BuildTarget::Desktop.to_string(), "desktop");
        assert_eq!(BuildTarget::Quest.to_string(), "quest");
    }

    #[test]
    fn build_profile_display() {
        assert_eq!(BuildProfile::Debug.to_string(), "debug");
        assert_eq!(BuildProfile::Release.to_string(), "release");
    }

    #[test]
    fn quest_constants_correct() {
        assert_eq!(QUEST_RUST_TARGET, "aarch64-linux-android");
        assert_eq!(QUEST_ABI, "arm64-v8a");
        assert_eq!(QUEST_MIN_SDK_VERSION, 29);
        assert_eq!(QUEST_TARGET_SDK_VERSION, 32);
    }

    #[test]
    fn build_config_validation_missing_dir() {
        let config = BuildConfig::new(
            PathBuf::from("/nonexistent/path"),
            BuildTarget::Desktop,
            BuildProfile::Debug,
        );
        let err = config.validate().unwrap_err();
        match err {
            BuildError::ProjectNotFound { path } => {
                assert!(path.contains("nonexistent"));
            }
            _ => panic!("expected ProjectNotFound error"),
        }
    }

    #[test]
    fn build_config_validation_passes_for_existing_dir() {
        let tmp = TempDir::new().unwrap();
        let config = BuildConfig::new(
            tmp.path().to_path_buf(),
            BuildTarget::Desktop,
            BuildProfile::Debug,
        );
        assert!(config.validate().is_ok());
    }

    #[test]
    fn build_output_dir_desktop() {
        let config = BuildConfig::new(
            PathBuf::from("/project"),
            BuildTarget::Desktop,
            BuildProfile::Debug,
        );
        let out = config.output_dir();
        assert!(out.ends_with("target/aether-build/desktop"));
    }

    #[test]
    fn build_output_dir_quest() {
        let config = BuildConfig::new(
            PathBuf::from("/project"),
            BuildTarget::Quest,
            BuildProfile::Debug,
        );
        let out = config.output_dir();
        assert!(out.ends_with("target/aether-build/quest"));
    }

    #[test]
    fn apk_path_debug() {
        let mut config = BuildConfig::new(
            PathBuf::from("/project"),
            BuildTarget::Quest,
            BuildProfile::Debug,
        );
        config.app_name = "myapp".to_string();
        let apk = config.apk_path();
        assert!(apk.to_string_lossy().contains("myapp-debug.apk"));
    }

    #[test]
    fn apk_path_release() {
        let mut config = BuildConfig::new(
            PathBuf::from("/project"),
            BuildTarget::Quest,
            BuildProfile::Release,
        );
        config.app_name = "myapp".to_string();
        let apk = config.apk_path();
        assert!(apk.to_string_lossy().contains("myapp-release.apk"));
    }

    #[test]
    fn package_name_format() {
        let mut config = BuildConfig::new(
            PathBuf::from("/project"),
            BuildTarget::Quest,
            BuildProfile::Debug,
        );
        config.app_name = "my-cool-app".to_string();
        assert_eq!(config.package_name(), "com.aether.my_cool_app");
    }

    #[test]
    fn package_name_sanitizes_special_chars() {
        let mut config = BuildConfig::new(
            PathBuf::from("/project"),
            BuildTarget::Quest,
            BuildProfile::Debug,
        );
        config.app_name = "App@2.0!".to_string();
        let name = config.package_name();
        assert!(name.starts_with("com.aether."));
        assert!(!name.contains('@'));
        assert!(!name.contains('!'));
    }

    #[test]
    fn ndk_host_tag_is_valid() {
        let tag = ndk_host_tag();
        assert!(
            tag == "darwin-x86_64" || tag == "linux-x86_64" || tag == "windows-x86_64",
            "unexpected host tag: {tag}"
        );
    }

    #[test]
    fn find_latest_build_tools_picks_highest() {
        let tmp = TempDir::new().unwrap();
        let bt = tmp.path().join("build-tools");
        std::fs::create_dir_all(bt.join("33.0.0")).unwrap();
        std::fs::create_dir_all(bt.join("34.0.0")).unwrap();
        std::fs::create_dir_all(bt.join("31.0.0")).unwrap();

        let result = find_latest_build_tools(tmp.path()).unwrap();
        assert!(result.ends_with("34.0.0"));
    }

    #[test]
    fn find_latest_build_tools_missing_dir() {
        let tmp = TempDir::new().unwrap();
        let err = find_latest_build_tools(tmp.path()).unwrap_err();
        match err {
            BuildError::ToolchainNotFound { tool, .. } => {
                assert!(tool.contains("build-tools"));
            }
            _ => panic!("expected ToolchainNotFound"),
        }
    }

    #[test]
    fn find_latest_build_tools_empty_dir() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("build-tools")).unwrap();
        let err = find_latest_build_tools(tmp.path()).unwrap_err();
        match err {
            BuildError::ToolchainNotFound { tool, .. } => {
                assert!(tool.contains("build-tools"));
            }
            _ => panic!("expected ToolchainNotFound"),
        }
    }
}
